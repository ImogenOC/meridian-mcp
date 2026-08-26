#include "queries.hpp"

#include <algorithm>
#include <cmath>
#include <map>
#include <limits>
#include <stdexcept>
#include <tuple>

namespace meridian::tracy
{

bool ZoneIdentity::operator<(const ZoneIdentity& other) const
{
	return std::tie(name, file, line) < std::tie(other.name, other.file, other.line);
}

HotspotResult select_hotspots(std::vector<ZoneStatistics> zones, const HotspotSort sort, const std::size_t limit)
{
	const auto metric = [sort](const ZoneStatistics& zone) -> std::int64_t {
		switch(sort)
		{
		case HotspotSort::Inclusive: return zone.inclusive;
		case HotspotSort::Self: return zone.self;
		case HotspotSort::Count: return static_cast<std::int64_t>(zone.count);
		case HotspotSort::Maximum: return zone.maximum;
		}
		return 0;
	};
	std::sort(zones.begin(), zones.end(), [&](const ZoneStatistics& left, const ZoneStatistics& right) {
		const auto left_metric = metric(left);
		const auto right_metric = metric(right);
		return left_metric != right_metric ? left_metric > right_metric : left.identity < right.identity;
	});
	const auto truncated = zones.size() > limit;
	if(truncated)
	{
		zones.resize(limit);
	}
	return {std::move(zones), truncated};
}

FrameStatistics summarize_frames(std::vector<std::int64_t> durations)
{
	if(durations.empty())
	{
		return {};
	}
	std::sort(durations.begin(), durations.end());
	std::int64_t total = 0;
	for(const auto duration : durations)
	{
		if(duration < 0 || duration > std::numeric_limits<std::int64_t>::max() - total)
		{
			throw std::overflow_error("statistics duration accumulator overflow");
		}
		total += duration;
	}
	const auto percentile = [&](const double fraction) {
		const auto rank = static_cast<std::size_t>(std::ceil(fraction * static_cast<double>(durations.size())));
		return durations[std::max<std::size_t>(1, rank) - 1];
	};
	return {
		static_cast<std::uint64_t>(durations.size()),
		durations.front(),
		durations.back(),
		total / static_cast<std::int64_t>(durations.size()),
		percentile(0.50),
		percentile(0.95),
		percentile(0.99),
	};
}

ComparisonResult compare_zones(
	const std::vector<ZoneStatistics>& baseline,
	const std::vector<ZoneStatistics>& current,
	const std::int64_t minimum_absolute_delta,
	const std::size_t limit
)
{
	std::map<ZoneIdentity, ZoneStatistics> baseline_by_identity;
	std::map<ZoneIdentity, ZoneStatistics> current_by_identity;
	for(const auto& zone : baseline)
	{
		baseline_by_identity.emplace(zone.identity, zone);
	}
	for(const auto& zone : current)
	{
		current_by_identity.emplace(zone.identity, zone);
	}

	std::map<ZoneIdentity, bool> identities;
	for(const auto& [identity, _] : baseline_by_identity) identities.emplace(identity, true);
	for(const auto& [identity, _] : current_by_identity) identities.emplace(identity, true);

	std::vector<ZoneComparison> output;
	for(const auto& [identity, _] : identities)
	{
		const auto baseline_entry = baseline_by_identity.find(identity);
		const auto current_entry = current_by_identity.find(identity);
		const ZoneStatistics empty {identity, 0, 0, 0, 0, 0};
		const auto& before = baseline_entry == baseline_by_identity.end() ? empty : baseline_entry->second;
		const auto& after = current_entry == current_by_identity.end() ? empty : current_entry->second;
		const auto delta = after.inclusive - before.inclusive;
		if(std::abs(delta) < minimum_absolute_delta)
		{
			continue;
		}
		output.push_back({
			identity,
			delta,
			after.self - before.self,
			static_cast<std::int64_t>(after.count) - static_cast<std::int64_t>(before.count),
		});
	}
	std::sort(output.begin(), output.end(), [](const ZoneComparison& left, const ZoneComparison& right) {
		const auto left_magnitude = std::abs(left.inclusive_delta);
		const auto right_magnitude = std::abs(right.inclusive_delta);
		return left_magnitude != right_magnitude ? left_magnitude > right_magnitude : left.identity < right.identity;
	});
	const auto truncated = output.size() > limit;
	if(truncated)
	{
		output.resize(limit);
	}
	return {std::move(output), truncated};
}

}
