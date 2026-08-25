#include "protocol.hpp"
#include "queries.hpp"
#include "session.hpp"

#include <algorithm>
#include <cstdint>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>

namespace
{

using nlohmann::json;
using namespace meridian::tracy;

std::uint64_t bounded_unsigned(const json& params, const char* name, const std::uint64_t minimum, const std::uint64_t maximum)
{
	if(!params.contains(name) || !params[name].is_number_unsigned())
	{
		throw ProtocolError("invalid_params", std::string(name) + " must be an unsigned integer.");
	}
	const auto value = params[name].get<std::uint64_t>();
	if(value < minimum || value > maximum)
	{
		throw ProtocolError("invalid_params", std::string(name) + " is outside the permitted range.");
	}
	return value;
}

std::string bounded_string(const json& params, const char* name, const std::size_t maximum)
{
	if(!params.contains(name) || !params[name].is_string())
	{
		throw ProtocolError("invalid_params", std::string(name) + " must be a string.");
	}
	auto value = params[name].get<std::string>();
	if(value.empty() || value.size() > maximum)
	{
		throw ProtocolError("invalid_params", std::string(name) + " is outside the permitted length.");
	}
	return value;
}

json zone_json(const ZoneStatistics& zone)
{
	return {
		{"name", zone.identity.name}, {"file", zone.identity.file}, {"line", zone.identity.line},
		{"count", zone.count}, {"inclusive_ns", zone.inclusive}, {"self_ns", zone.self},
		{"mean_ns", zone.count == 0 ? 0 : zone.inclusive / static_cast<std::int64_t>(zone.count)},
		{"min_ns", zone.minimum}, {"max_ns", zone.maximum},
	};
}

json dispatch(const Request& request)
{
	const auto& params = request.params;
	switch(request.command)
	{
	case Command::Capture:
	{
		const auto result = capture_trace(
			static_cast<std::uint16_t>(bounded_unsigned(params, "port", 1, 65535)),
			bounded_unsigned(params, "duration_ms", 1, 300000),
			bounded_unsigned(params, "memory_limit_mb", 16, 4096),
			bounded_string(params, "output_path", 32768)
		);
		return {{"frame_count", result.frame_count}, {"zone_count", result.zone_count}, {"span_ns", result.span_ns}, {"uncompressed_bytes", result.uncompressed_bytes}, {"compressed_bytes", result.compressed_bytes}};
	}
	case Command::Hotspots:
	{
		auto trace = load_trace(bounded_string(params, "trace_path", 32768));
		const auto limit = bounded_unsigned(params, "limit", 1, 1000);
		const auto sort_name = bounded_string(params, "sort", 16);
		const auto sort = sort_name == "inclusive" ? HotspotSort::Inclusive : sort_name == "self" ? HotspotSort::Self : sort_name == "count" ? HotspotSort::Count : sort_name == "max" ? HotspotSort::Maximum : throw ProtocolError("invalid_params", "sort is not supported.");
		const auto selected = select_hotspots(std::move(trace.zones), sort, static_cast<std::size_t>(limit));
		json items = json::array();
		for(const auto& zone : selected.items) items.push_back(zone_json(zone));
		return {{"items", std::move(items)}, {"truncated", selected.truncated}, {"limit", limit}, {"span_ns", trace.span_ns}};
	}
	case Command::Zone:
	{
		const auto trace = load_trace(bounded_string(params, "trace_path", 32768));
		const auto name = bounded_string(params, "name", 4096);
		const auto limit = bounded_unsigned(params, "limit", 1, 1000);
		json items = json::array();
		for(const auto& zone : trace.zones)
		{
			if(zone.identity.name == name && items.size() < limit) items.push_back(zone_json(zone));
		}
		return {{"items", std::move(items)}, {"truncated", std::count_if(trace.zones.begin(), trace.zones.end(), [&](const auto& zone) { return zone.identity.name == name; }) > static_cast<std::ptrdiff_t>(limit)}};
	}
	case Command::FrameStats:
	{
		auto trace = load_trace(bounded_string(params, "trace_path", 32768));
		const auto frames = summarize_frames(std::move(trace.frame_durations));
		return {{"frame_count", frames.count}, {"span_ns", trace.span_ns}, {"mean_ns", frames.mean}, {"min_ns", frames.minimum}, {"max_ns", frames.maximum}, {"p50_ns", frames.p50}, {"p95_ns", frames.p95}, {"p99_ns", frames.p99}};
	}
	case Command::Compare:
	{
		const auto baseline = load_trace(bounded_string(params, "baseline_path", 32768));
		const auto current = load_trace(bounded_string(params, "current_path", 32768));
		const auto minimum = bounded_unsigned(params, "minimum_delta_ns", 0, static_cast<std::uint64_t>(INT64_MAX));
		const auto limit = bounded_unsigned(params, "limit", 1, 1000);
		const auto result = compare_zones(baseline.zones, current.zones, static_cast<std::int64_t>(minimum), static_cast<std::size_t>(limit));
		json items = json::array();
		for(const auto& item : result.items)
		{
			items.push_back({{"name", item.identity.name}, {"file", item.identity.file}, {"line", item.identity.line}, {"inclusive_delta_ns", item.inclusive_delta}, {"self_delta_ns", item.self_delta}, {"count_delta", item.count_delta}});
		}
		return {{"items", std::move(items)}, {"truncated", result.truncated}, {"limit", limit}};
	}
	}
	throw ProtocolError("unsupported_command", "Request command is not supported.");
}

}

int main()
{
	std::string input(std::istreambuf_iterator<char>(std::cin), {});
	if(!input.empty() && input.back() == '\n') input.pop_back();
	if(!input.empty() && input.back() == '\r') input.pop_back();
	std::uint64_t id = 0;
	try
	{
		if(input.find('\n') != std::string::npos || input.find('\r') != std::string::npos)
		{
			throw ProtocolError("multiple_requests", "The helper accepts exactly one request.");
		}
		const auto request = meridian::tracy::parse_request(input);
		id = request.id;
		std::cout << meridian::tracy::success_response(id, dispatch(request)).dump() << '\n';
		return 0;
	}
	catch(const meridian::tracy::ProtocolError& error)
	{
		std::cout << meridian::tracy::error_response(id, error.code(), error.what()).dump() << '\n';
		return 2;
	}
	catch(const std::exception& error)
	{
		std::cout << meridian::tracy::error_response(id, "tracy_failure", error.what()).dump() << '\n';
		return 3;
	}
}
