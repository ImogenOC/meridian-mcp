#pragma once

#include "queries.hpp"
#include "validation.hpp"

#include <cstdint>
#include <filesystem>
#include <memory>
#include <string>
#include <vector>

namespace meridian::tracy
{

struct CaptureResult
{
	std::uint64_t frame_count;
	std::uint64_t zone_count;
	std::int64_t span_ns;
	std::uint64_t uncompressed_bytes;
	std::uint64_t compressed_bytes;
	CaptureValidation validation {};
	std::string phase;
	std::uint32_t phase_iteration;
};

struct TraceData
{
	std::vector<ZoneStatistics> zones;
	std::vector<std::int64_t> frame_durations;
	std::vector<FrameInterval> frame_intervals;
	std::int64_t span_ns;
	std::uint64_t complete_zone_count;
	std::uint64_t source_file_count;
	RangeCounts frame_counts {};
	RangeCounts zone_counts {};
};

class CollectorBackend;

[[nodiscard]] CaptureResult capture_trace(
	std::uint16_t port,
	std::uint64_t duration_ms,
	std::uint64_t memory_limit_mb,
	const std::filesystem::path& output
);
[[nodiscard]] TraceData load_trace(const std::filesystem::path& path);
[[nodiscard]] TraceData load_trace(const std::filesystem::path& path, std::int64_t range_begin, std::int64_t range_end);
[[nodiscard]] std::unique_ptr<CollectorBackend> make_tracy_collector_backend();

}

