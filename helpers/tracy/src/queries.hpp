#pragma once

#include <cstddef>
#include <cstdint>
#include <string>
#include <vector>

namespace meridian::tracy
{

struct ZoneIdentity
{
	std::string name;
	std::string file;
	std::uint32_t line;

	[[nodiscard]] bool operator==(const ZoneIdentity&) const = default;
	[[nodiscard]] bool operator<(const ZoneIdentity& other) const;
};

struct ZoneStatistics
{
	ZoneIdentity identity;
	std::uint64_t count;
	std::int64_t inclusive;
	std::int64_t self;
	std::int64_t minimum;
	std::int64_t maximum;
};

enum class HotspotSort
{
	Inclusive,
	Self,
	Count,
	Maximum,
};

struct HotspotResult
{
	std::vector<ZoneStatistics> items;
	bool truncated;
};

struct FrameStatistics
{
	std::uint64_t count;
	std::int64_t minimum;
	std::int64_t maximum;
	std::int64_t mean;
	std::int64_t p50;
	std::int64_t p95;
	std::int64_t p99;
};

struct ZoneComparison
{
	ZoneIdentity identity;
	std::int64_t inclusive_delta;
	std::int64_t self_delta;
	std::int64_t count_delta;
};

struct ComparisonResult
{
	std::vector<ZoneComparison> items;
	bool truncated;
};

[[nodiscard]] HotspotResult select_hotspots(std::vector<ZoneStatistics> zones, HotspotSort sort, std::size_t limit);
[[nodiscard]] FrameStatistics summarize_frames(std::vector<std::int64_t> durations);
[[nodiscard]] ComparisonResult compare_zones(
	const std::vector<ZoneStatistics>& baseline,
	const std::vector<ZoneStatistics>& current,
	std::int64_t minimum_absolute_delta,
	std::size_t limit
);

}

