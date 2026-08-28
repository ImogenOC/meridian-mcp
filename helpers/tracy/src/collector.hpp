#pragma once

#include "protocol.hpp"
#include "session.hpp"

#include <atomic>
#include <chrono>
#include <condition_variable>
#include <cstdint>
#include <memory>
#include <mutex>
#include <string>

namespace meridian::tracy
{

enum class WorkerPurpose
{
	Drain,
	Capture,
};

enum class SessionPhase
{
	Stopped,
	Starting,
	Draining,
	CaptureConnecting,
	Capturing,
	Validating,
	DrainRestoring,
	Stopping,
	Failed,
};

struct SessionLimits
{
	std::uint64_t maximum_session_seconds = MaximumSessionSeconds;
	std::uint64_t maximum_capture_count = MaximumCaptureCount;
	std::uint64_t maximum_capture_duration_ms = MaximumCaptureDurationMs;
	std::uint64_t maximum_memory_mb = MaximumResidentMemoryMb;
	std::uint64_t maximum_trace_bytes = MaximumTraceBytes;
	std::chrono::milliseconds stop_timeout {5'000};
	std::uint64_t maximum_attach_attempts = 2;
	std::chrono::milliseconds transition_retry_delay {250};
};

struct SessionStartOptions
{
	std::string host;
	std::uint16_t port;
	std::uint64_t connect_timeout_ms;
	std::uint64_t progress_timeout_ms;
};

struct CaptureWindowOptions
{
	std::uint64_t duration_ms;
	std::uint64_t memory_limit_mb;
	std::string output_path;
	std::string phase;
	std::uint32_t phase_iteration;
};

struct SessionStatus
{
	SessionPhase phase;
	std::uint64_t worker_generation;
	std::uint64_t producer_progress;
	std::uint64_t capture_count;
	WorkerPurpose worker_purpose;
	bool worker_attached;
	std::uint64_t transition_retry_count;
	std::string last_transition_error;
	bool recovery_required;
	std::optional<QueueHealth> queue_health;
};

struct CaptureWindowResult
{
	CaptureResult capture;
	SessionStatus status;
};

class CollectorBackend
{
public:
	virtual ~CollectorBackend() = default;
	virtual void configure(const SessionStartOptions&) {}
	virtual void attach(WorkerPurpose purpose) = 0;
	virtual void detach() = 0;
	[[nodiscard]] virtual std::uint64_t producer_progress() const = 0;
	[[nodiscard]] virtual std::optional<QueueHealth> health() = 0;
	[[nodiscard]] virtual CaptureResult capture(
		const CaptureWindowOptions& options,
		const std::atomic_bool& cancelled,
		std::atomic_bool& window_started
	) = 0;
};

class CollectorSession
{
public:
	explicit CollectorSession(std::unique_ptr<CollectorBackend> backend, SessionLimits limits);

	[[nodiscard]] SessionStatus start(const SessionStartOptions& options);
	[[nodiscard]] CaptureWindowResult capture(const CaptureWindowOptions& options);
	[[nodiscard]] SessionStatus status();
	[[nodiscard]] SessionStatus cancel();
	[[nodiscard]] SessionStatus stop();

private:
	[[nodiscard]] SessionStatus status_locked() const;
	[[nodiscard]] QueueHealth wait_for_queue_health(std::chrono::steady_clock::time_point deadline);
	void attach(WorkerPurpose purpose);
	bool restore_drain_worker() noexcept;

	std::unique_ptr<CollectorBackend> backend;
	SessionLimits limits;
	mutable std::mutex mutex;
	std::condition_variable idle_condition;
	std::atomic_bool cancelled {false};
	SessionPhase phase = SessionPhase::Stopped;
	std::uint64_t worker_generation = 0;
	std::uint64_t producer_progress = 0;
	std::uint64_t capture_count = 0;
	WorkerPurpose worker_purpose = WorkerPurpose::Drain;
	bool worker_attached = false;
	std::uint64_t transition_retry_count = 0;
	std::string last_transition_error;
	bool recovery_required = false;
	std::optional<QueueHealth> queue_health;
	bool capture_active = false;
	std::chrono::steady_clock::time_point started_at {};
	std::chrono::milliseconds readiness_timeout {100};
};

}
