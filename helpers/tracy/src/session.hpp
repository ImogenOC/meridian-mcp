#pragma once

#include "queries.hpp"

#include <cstdint>
#include <filesystem>
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
};

struct TraceData
{
	std::vector<ZoneStatistics> zones;
	std::vector<std::int64_t> frame_durations;
	std::int64_t span_ns;
};

[[nodiscard]] CaptureResult capture_trace(
	std::uint16_t port,
	std::uint64_t duration_ms,
	std::uint64_t memory_limit_mb,
	const std::filesystem::path& output
);
[[nodiscard]] TraceData load_trace(const std::filesystem::path& path);

}

