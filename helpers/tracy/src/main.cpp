#include "collector.hpp"
#include "protocol.hpp"
#include "queries.hpp"
#include "session.hpp"
#include "session_json.hpp"

#include <algorithm>
#include <cstdint>
#include <iostream>
#include <iterator>
#include <limits>
#include <mutex>
#include <stdexcept>
#include <string>
#include <thread>
#include <unordered_set>
#include <vector>

namespace
{

using nlohmann::json;
using namespace meridian::tracy;

std::uint64_t bounded_unsigned(const json& params, const char* name, const std::uint64_t minimum, const std::uint64_t maximum)
{
	if(!params.contains(name) || !params[name].is_number_unsigned()) throw ProtocolError("invalid_params", std::string(name) + " must be an unsigned integer.");
	const auto value = params[name].get<std::uint64_t>();
	if(value < minimum || value > maximum) throw ProtocolError("invalid_params", std::string(name) + " is outside the permitted range.");
	return value;
}

std::string bounded_string(const json& params, const char* name, const std::size_t maximum)
{
	if(!params.contains(name) || !params[name].is_string()) throw ProtocolError("invalid_params", std::string(name) + " must be a string.");
	auto value = params[name].get<std::string>();
	if(value.empty() || value.size() > maximum) throw ProtocolError("invalid_params", std::string(name) + " is outside the permitted length.");
	return value;
}

json zone_json(const ZoneStatistics& zone)
{
	return {
		{"name", zone.identity.name}, {"file", zone.identity.file}, {"line", zone.identity.line},
		{"count", zone.count}, {"inclusive_ns", zone.inclusive}, {"self_ns", zone.self},
		{"mean_ns", zone.count == 0 ? 0 : zone.inclusive / static_cast<std::int64_t>(zone.count)},
		{"min_ns", zone.minimum}, {"max_ns", zone.maximum},
		{"p50_ns", zone.p50}, {"p95_ns", zone.p95}, {"p99_ns", zone.p99},
		{"self_p50_ns", zone.self_p50}, {"self_p95_ns", zone.self_p95}, {"self_p99_ns", zone.self_p99},
	};
}

TraceData load_requested_trace(const json& params)
{
	const auto path = bounded_string(params, "trace_path", 32768);
	if(!params.contains("range_begin_ns") && !params.contains("range_end_ns")) return load_trace(path);
	return load_trace(
		path,
		static_cast<std::int64_t>(bounded_unsigned(params, "range_begin_ns", 0, static_cast<std::uint64_t>(INT64_MAX))),
		static_cast<std::int64_t>(bounded_unsigned(params, "range_end_ns", 1, static_cast<std::uint64_t>(INT64_MAX)))
	);
}

json range_counts_json(const RangeCounts& counts)
{
	return {{"raw", counts.raw_total}, {"intersecting", counts.intersecting}, {"complete", counts.complete},
		{"partial_first", counts.partial_left}, {"partial_last", counts.partial_right}, {"spanning", counts.spanning},
		{"invalid", counts.invalid}, {"excluded", counts.partial_left + counts.partial_right + counts.spanning + counts.invalid}, {"analyzed", counts.analyzed}};
}

json dispatch_offline(const Request& request)
{
	const auto& params = request.params;
	switch(request.command)
	{
	case Command::Capture:
	{
		const auto result = capture_trace(
			static_cast<std::uint16_t>(bounded_unsigned(params, "port", 1, 65535)),
			bounded_unsigned(params, "duration_ms", 1, MaximumCaptureDurationMs),
			bounded_unsigned(params, "memory_limit_mb", 16, MaximumResidentMemoryMb),
			bounded_string(params, "output_path", 32768)
		);
		return {{"frame_count", result.frame_count}, {"zone_count", result.zone_count}, {"span_ns", result.span_ns}, {"uncompressed_bytes", result.uncompressed_bytes}, {"compressed_bytes", result.compressed_bytes}};
	}
	case Command::Hotspots:
	{
		auto trace = load_requested_trace(params);
		const auto limit = bounded_unsigned(params, "limit", 1, 1000);
		const auto sort_name = bounded_string(params, "sort", 16);
		const auto sort = sort_name == "inclusive" ? HotspotSort::Inclusive : sort_name == "self" ? HotspotSort::Self : sort_name == "count" ? HotspotSort::Count : sort_name == "max" ? HotspotSort::Maximum : throw ProtocolError("invalid_params", "sort is not supported.");
		const auto selected = select_hotspots(std::move(trace.zones), sort, static_cast<std::size_t>(limit));
		json items = json::array();
		for(const auto& zone : selected.items) items.push_back(zone_json(zone));
		return {{"items", std::move(items)}, {"truncated", selected.truncated}, {"limit", limit}, {"span_ns", trace.span_ns}, {"counts", range_counts_json(trace.zone_counts)}};
	}
	case Command::Zone:
	{
		const auto trace = load_requested_trace(params);
		const auto name = bounded_string(params, "name", 4096);
		const auto limit = bounded_unsigned(params, "limit", 1, 1000);
		json items = json::array();
		for(const auto& zone : trace.zones) if(zone.identity.name == name && items.size() < limit) items.push_back(zone_json(zone));
		return {{"items", std::move(items)}, {"truncated", std::count_if(trace.zones.begin(), trace.zones.end(), [&](const auto& zone) { return zone.identity.name == name; }) > static_cast<std::ptrdiff_t>(limit)}, {"counts", range_counts_json(trace.zone_counts)}};
	}
	case Command::FrameStats:
	{
		auto trace = load_requested_trace(params);
		const auto frames = summarize_frames(std::move(trace.frame_durations));
		if(frames.count == 0) throw ProtocolError("insufficient_complete_samples", "No complete frames exist inside the requested range.");
		return {{"frame_count", frames.count}, {"span_ns", trace.span_ns}, {"mean_ns", frames.mean}, {"min_ns", frames.minimum}, {"max_ns", frames.maximum}, {"p50_ns", frames.p50}, {"p95_ns", frames.p95}, {"p99_ns", frames.p99}, {"counts", range_counts_json(trace.frame_counts)}};
	}
	case Command::Compare:
	{
		const auto ranged = params.contains("baseline_range_begin_ns") || params.contains("current_range_begin_ns");
		const auto baseline = ranged ? load_trace(
			bounded_string(params, "baseline_path", 32768),
			static_cast<std::int64_t>(bounded_unsigned(params, "baseline_range_begin_ns", 0, static_cast<std::uint64_t>(INT64_MAX))),
			static_cast<std::int64_t>(bounded_unsigned(params, "baseline_range_end_ns", 1, static_cast<std::uint64_t>(INT64_MAX)))) : load_trace(bounded_string(params, "baseline_path", 32768));
		const auto current = ranged ? load_trace(
			bounded_string(params, "current_path", 32768),
			static_cast<std::int64_t>(bounded_unsigned(params, "current_range_begin_ns", 0, static_cast<std::uint64_t>(INT64_MAX))),
			static_cast<std::int64_t>(bounded_unsigned(params, "current_range_end_ns", 1, static_cast<std::uint64_t>(INT64_MAX)))) : load_trace(bounded_string(params, "current_path", 32768));
		const auto minimum = bounded_unsigned(params, "minimum_delta_ns", 0, static_cast<std::uint64_t>(INT64_MAX));
		const auto limit = bounded_unsigned(params, "limit", 1, 1000);
		const auto result = compare_zones(baseline.zones, current.zones, static_cast<std::int64_t>(minimum), static_cast<std::size_t>(limit));
		json items = json::array();
		for(const auto& item : result.items) items.push_back({{"name", item.identity.name}, {"file", item.identity.file}, {"line", item.identity.line}, {"inclusive_delta_ns", item.inclusive_delta}, {"self_delta_ns", item.self_delta}, {"count_delta", item.count_delta}});
		return {{"items", std::move(items)}, {"truncated", result.truncated}, {"limit", limit}};
	}
	default: throw ProtocolError("unsupported_command", "Command requires --session mode.");
	}
}

void write_response(const json& response, std::mutex& output_mutex)
{
	auto serialized = response.dump();
	if(serialized.size() > MaximumResponseBytes)
	{
		serialized = error_response(response.value("id", std::uint64_t {0}), "response_too_large", "Response exceeds the fixed protocol limit.").dump();
	}
	std::scoped_lock lock(output_mutex);
	std::cout << serialized << '\n' << std::flush;
}

int run_single_request()
{
	std::string input(std::istreambuf_iterator<char>(std::cin), {});
	if(!input.empty() && input.back() == '\n') input.pop_back();
	if(!input.empty() && input.back() == '\r') input.pop_back();
	std::uint64_t id = 0;
	std::mutex output_mutex;
	try
	{
		if(input.find('\n') != std::string::npos || input.find('\r') != std::string::npos) throw ProtocolError("multiple_requests", "The helper accepts exactly one request outside --session mode.");
		const auto request = parse_request(input);
		id = request.id;
		write_response(success_response(id, dispatch_offline(request)), output_mutex);
		return 0;
	}
	catch(const ProtocolError& error)
	{
		write_response(error_response(id, error.code(), error.what(), error.details()), output_mutex);
		return 2;
	}
	catch(const std::exception& error)
	{
		write_response(error_response(id, "tracy_failure", error.what()), output_mutex);
		return 3;
	}
}

int run_session()
{
	CollectorSession session(make_tracy_collector_backend(), {});
	std::mutex output_mutex;
	std::unordered_set<std::uint64_t> request_ids;
	std::vector<std::thread> capture_threads;
	std::string input;
	while(std::getline(std::cin, input))
	{
		if(!input.empty() && input.back() == '\r') input.pop_back();
		std::uint64_t id = 0;
		try
		{
			const auto request = parse_request(input);
			id = request.id;
			if(!request_ids.insert(id).second) throw ProtocolError("duplicate_id", "Request id has already been used in this session.");
			const auto& params = request.params;
			switch(request.command)
			{
			case Command::SessionStart:
				write_response(success_response(id, session_status_json(session.start({
					bounded_string(params, "host", 255),
					static_cast<std::uint16_t>(bounded_unsigned(params, "port", 1, 65535)),
					bounded_unsigned(params, "connect_timeout_ms", 1, 120'000),
					bounded_unsigned(params, "progress_timeout_ms", 1, 120'000),
				}))), output_mutex);
				break;
			case Command::CaptureWindow:
			{
				const CaptureWindowOptions options {
					bounded_unsigned(params, "duration_ms", 1, MaximumCaptureDurationMs),
					bounded_unsigned(params, "memory_limit_mb", 16, MaximumResidentMemoryMb),
					bounded_string(params, "output_path", 32768),
					bounded_string(params, "phase", 64),
					static_cast<std::uint32_t>(bounded_unsigned(params, "phase_iteration", 1, std::numeric_limits<std::uint32_t>::max())),
				};
				if(!valid_phase_name(options.phase)) throw ProtocolError("invalid_phase", "Phase must contain 1-64 lowercase ASCII letters, digits, underscore, or hyphen.");
				capture_threads.emplace_back([&, id, options] {
					try
					{
						const auto result = session.capture(options);
						if(!result.capture.validation.valid)
						{
							auto response = error_response(id, "invalid_capture", "Capture failed mandatory validation.");
							response["error"]["details"] = validation_json(result.capture.validation);
							response["error"]["details"]["window_started"] = true;
							response["error"]["details"]["collector_recovered"] = result.status.phase == SessionPhase::Draining;
							write_response(response, output_mutex);
						}
						else
						{
							write_response(success_response(id, capture_result_json(result)), output_mutex);
						}
					}
					catch(const ProtocolError& error) { write_response(error_response(id, error.code(), error.what(), error.details()), output_mutex); }
					catch(const std::exception& error) { write_response(error_response(id, "tracy_failure", error.what()), output_mutex); }
				});
				break;
			}
			case Command::SessionStatus: write_response(success_response(id, session_status_json(session.status())), output_mutex); break;
			case Command::Cancel: write_response(success_response(id, session_status_json(session.cancel())), output_mutex); break;
			case Command::SessionStop: write_response(success_response(id, session_status_json(session.stop())), output_mutex); break;
			default: write_response(success_response(id, dispatch_offline(request)), output_mutex); break;
			}
		}
		catch(const ProtocolError& error) { write_response(error_response(id, error.code(), error.what(), error.details()), output_mutex); }
		catch(const std::exception& error) { write_response(error_response(id, "tracy_failure", error.what()), output_mutex); }
	}
	try { static_cast<void>(session.stop()); } catch(...) {}
	for(auto& thread : capture_threads) if(thread.joinable()) thread.join();
	return 0;
}

}

int main(const int argc, char** argv)
{
	if(argc == 1) return run_single_request();
	if(argc == 2 && std::string_view(argv[1]) == "--session") return run_session();
	std::cerr << "usage: meridian-tracy-helper [--session]\n";
	return 64;
}
