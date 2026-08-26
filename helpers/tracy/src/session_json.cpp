#include "session_json.hpp"

namespace meridian::tracy
{

const char* phase_name(const SessionPhase phase) noexcept
{
	switch(phase)
	{
	case SessionPhase::Stopped: return "stopped";
	case SessionPhase::Starting: return "starting";
	case SessionPhase::Draining: return "draining";
	case SessionPhase::Capturing: return "capturing";
	case SessionPhase::Validating: return "validating";
	case SessionPhase::Stopping: return "stopping";
	case SessionPhase::Failed: return "failed";
	}
	return "unknown";
}

nlohmann::json session_status_json(const SessionStatus& status)
{
	return {
		{"state", phase_name(status.phase)},
		{"worker_generation", status.worker_generation},
		{"producer_progress", status.producer_progress},
		{"capture_count", status.capture_count},
	};
}

namespace
{

nlohmann::json queue_health_json(const QueueHealth& health)
{
	return {
		{"capacity", health.capacity},
		{"depth", health.depth},
		{"high_water", health.high_water},
		{"saturation_count", health.saturation_count},
		{"dropped_events", health.dropped_events},
		{"produced_events", health.produced_events},
		{"consumed_events", health.consumed_events},
		{"last_producer_progress_raw", health.last_producer_progress_raw},
		{"hook_installed", health.hook_installed},
		{"prologue_validated", health.prologue_validated},
		{"byond_build", health.byond_build},
		{"offset_table_identity", health.offset_table_identity},
	};
}

}

nlohmann::json validation_json(const CaptureValidation& validation)
{
	return {
		{"valid", validation.valid},
		{"raw_begin", validation.raw_begin},
		{"raw_end", validation.raw_end},
		{"trace_begin_ns", validation.trace_begin_ns},
		{"trace_end_ns", validation.trace_end_ns},
		{"nanoseconds_per_tick", validation.nanoseconds_per_tick},
		{"wall_span_seconds", validation.wall_span_seconds},
		{"complete_frames", validation.complete_frames},
		{"partial_frames", validation.partial_frames},
		{"zones", validation.zones},
		{"source_files", validation.source_files},
		{"queue", queue_health_json(validation.queue)},
		{"error_codes", validation.error_codes},
	};
}

nlohmann::json capture_result_json(const CaptureWindowResult& result)
{
	return {
		{"frame_count", result.capture.frame_count},
		{"zone_count", result.capture.zone_count},
		{"span_ns", result.capture.span_ns},
		{"uncompressed_bytes", result.capture.uncompressed_bytes},
		{"compressed_bytes", result.capture.compressed_bytes},
		{"validation", validation_json(result.capture.validation)},
		{"phase", result.capture.phase},
		{"phase_iteration", result.capture.phase_iteration},
		{"session", session_status_json(result.status)},
	};
}

}
