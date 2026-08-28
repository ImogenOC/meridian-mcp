#pragma once

#include <cstdint>
#include <optional>
#include <string>
#include <string_view>
#include <vector>

namespace meridian::tracy
{

enum class FrameClass
{
	Complete,
	LeftBoundary,
	RightBoundary,
	Spanning,
};

struct FrameInterval
{
	std::uint64_t begin;
	std::uint64_t end;
};

struct QueryRange
{
	std::uint64_t raw_begin;
	std::uint64_t raw_end;
	std::string phase;
	std::uint32_t phase_iteration;
};

struct RangeCounts
{
	std::uint64_t raw_total;
	std::uint64_t intersecting;
	std::uint64_t complete;
	std::uint64_t partial_left;
	std::uint64_t partial_right;
	std::uint64_t spanning;
	std::uint64_t analyzed;
	std::uint64_t invalid = 0;
};

struct QueueHealth
{
	std::uint64_t capacity;
	std::uint64_t depth;
	std::uint64_t high_water;
	std::uint64_t tail_refresh_count;
	std::uint64_t saturation_count;
	std::uint64_t dropped_events;
	std::uint64_t produced_events;
	std::uint64_t consumed_events;
	std::uint64_t last_producer_progress_raw;
	bool hook_installed;
	bool prologue_validated;
	std::string byond_build;
	std::string offset_table_identity;
};

struct CaptureObservation
{
	std::uint64_t raw_begin;
	std::uint64_t raw_end;
	std::int64_t trace_begin_ns;
	std::int64_t trace_end_ns;
	double nanoseconds_per_tick;
	double requested_seconds;
	double measured_wall_seconds;
	std::vector<FrameInterval> frames;
	std::uint64_t complete_zone_count;
	std::uint64_t zone_count;
	std::uint64_t source_file_count;
	std::uint64_t trace_bytes;
	std::uint64_t minimum_trace_bytes;
	bool trace_reopened;
	QueueHealth queue_start;
	QueueHealth queue_end;
};

struct CaptureValidation
{
	bool valid;
	std::uint64_t raw_begin;
	std::uint64_t raw_end;
	std::int64_t trace_begin_ns;
	std::int64_t trace_end_ns;
	double nanoseconds_per_tick;
	double wall_span_seconds;
	double requested_wall_seconds;
	double measured_wall_seconds;
	double wall_tolerance_seconds;
	double producer_progress_shortfall_seconds;
	std::uint64_t complete_frames;
	std::uint64_t partial_frames;
	std::uint64_t zones;
	std::uint64_t source_files;
	QueueHealth queue;
	std::vector<std::string> error_codes;
	std::vector<std::string> warning_codes;
};

[[nodiscard]] std::optional<FrameClass> classify_frame(
	const FrameInterval& frame,
	std::uint64_t range_begin,
	std::uint64_t range_end
);
[[nodiscard]] bool should_validate_frame(
	const FrameInterval& frame,
	std::uint64_t trace_first_time,
	bool has_observed_end
);
[[nodiscard]] bool valid_phase_name(std::string_view phase) noexcept;
[[nodiscard]] RangeCounts count_range(const std::vector<FrameInterval>& intervals, std::uint64_t range_begin, std::uint64_t range_end);
[[nodiscard]] std::optional<std::int64_t> trace_time_from_raw(
	std::uint64_t raw_time,
	std::uint64_t raw_base,
	double nanoseconds_per_tick
);
[[nodiscard]] CaptureValidation validate_capture(const CaptureObservation& observation);

}
