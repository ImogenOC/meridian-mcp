#include "session_json.hpp"

namespace meridian::tracy
{

namespace
{

nlohmann::json queue_health_json(const QueueHealth& health);

}

const char* phase_name(const SessionPhase phase) noexcept
{
	switch(phase)
	{
	case SessionPhase::Stopped: return "stopped";
	case SessionPhase::Starting: return "starting";
	case SessionPhase::Draining: return "draining";
	case SessionPhase::CaptureConnecting: return "capture_connecting";
	case SessionPhase::Capturing: return "capturing";
	case SessionPhase::Validating: return "validating";
	case SessionPhase::DrainRestoring: return "drain_restoring";
	case SessionPhase::Stopping: return "stopping";
	case SessionPhase::Failed: return "failed";
	}
	return "unknown";
}

nlohmann::json session_status_json(const SessionStatus& status)
{
	auto output = nlohmann::json {
		{"state", phase_name(status.phase)},
		{"worker_generation", status.worker_generation},
		{"producer_progress", status.producer_progress},
		{"capture_count", status.capture_count},
		{"worker_attached", status.worker_attached},
		{"worker_purpose", status.worker_attached ? nlohmann::json(status.worker_purpose == WorkerPurpose::Drain ? "drain" : "capture") : nlohmann::json(nullptr)},
		{"transition_retry_count", status.transition_retry_count},
		{"last_transition_error", status.last_transition_error.empty() ? nlohmann::json(nullptr) : nlohmann::json(status.last_transition_error)},
		{"recovery_required", status.recovery_required},
		{"queue_health", status.queue_health.has_value() ? queue_health_json(*status.queue_health) : nlohmann::json(nullptr)},
	};
	return output;
}

namespace
{

nlohmann::json queue_health_json(const QueueHealth& health)
{
	return {
		{"capacity", health.capacity},
		{"depth", health.depth},
		{"high_water", health.high_water},
		{"tail_refresh_count", health.tail_refresh_count},
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
		{"requested_wall_seconds", validation.requested_wall_seconds},
		{"measured_wall_seconds", validation.measured_wall_seconds},
		{"wall_tolerance_seconds", validation.wall_tolerance_seconds},
		{"producer_progress_shortfall_seconds", validation.producer_progress_shortfall_seconds},
		{"complete_frames", validation.complete_frames},
		{"partial_frames", validation.partial_frames},
		{"zones", validation.zones},
		{"source_files", validation.source_files},
		{"queue", queue_health_json(validation.queue)},
		{"error_codes", validation.error_codes},
		{"warning_codes", validation.warning_codes},
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
