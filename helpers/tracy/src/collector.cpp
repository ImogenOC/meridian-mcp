#include "collector.hpp"

#include <thread>

namespace meridian::tracy
{

CollectorSession::CollectorSession(std::unique_ptr<CollectorBackend> backend, const SessionLimits limits)
	: backend(std::move(backend)),
	  limits(limits)
{
	if(!this->backend)
	{
		throw ProtocolError("invalid_backend", "Collector backend is required.");
	}
}

SessionStatus CollectorSession::start(const SessionStartOptions& options)
{
	{
		std::scoped_lock lock(mutex);
		if(phase != SessionPhase::Stopped)
		{
			throw ProtocolError("session_already_started", "Collector session has already started.");
		}
		phase = SessionPhase::Starting;
		started_at = std::chrono::steady_clock::now();
	}
	backend->configure(options);
	readiness_timeout = std::chrono::milliseconds(options.progress_timeout_ms);
	const auto initial_progress = backend->producer_progress();
	try
	{
		attach(WorkerPurpose::Drain, limits.maximum_memory_mb);
	}
	catch(...)
	{
		std::scoped_lock lock(mutex);
		phase = SessionPhase::Failed;
		throw;
	}
	const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(options.progress_timeout_ms);
	std::optional<QueueHealth> ready_health;
	while(true)
	{
		const auto progress_advanced = backend->producer_progress() > initial_progress;
		ready_health = backend->health();
		const auto health_ready = ready_health.has_value() && ready_health->capacity > 0 && ready_health->last_producer_progress_raw > 0 && ready_health->hook_installed && ready_health->prologue_validated;
		if(progress_advanced && health_ready) break;
		if(std::chrono::steady_clock::now() >= deadline)
		{
			backend->detach();
			std::scoped_lock lock(mutex);
			phase = SessionPhase::Failed;
			if(progress_advanced)
			{
				throw ProtocolError("health_timeout", "Queue health did not become valid during collector readiness.");
			}
			throw ProtocolError("producer_stalled", "Producer progress did not advance during collector readiness.");
		}
		std::this_thread::sleep_for(std::chrono::milliseconds(1));
	}
	std::scoped_lock lock(mutex);
	producer_progress = backend->producer_progress();
	queue_health = std::move(ready_health);
	phase = SessionPhase::Draining;
	return status_locked();
}

CaptureWindowResult CollectorSession::capture(const CaptureWindowOptions& options)
{
	std::atomic_bool window_started {false};
	{
		std::scoped_lock lock(mutex);
		if(capture_active)
		{
			throw ProtocolError("capture_busy", "A capture is already active.");
		}
		if(phase != SessionPhase::Draining)
		{
			throw ProtocolError("session_not_ready", "Collector session is not ready to capture.");
		}
		if(capture_count >= limits.maximum_capture_count || options.duration_ms > limits.maximum_capture_duration_ms || options.memory_limit_mb > limits.maximum_memory_mb || std::chrono::steady_clock::now() - started_at > std::chrono::seconds(limits.maximum_session_seconds))
		{
			throw ProtocolError("session_limit_reached", "Capture would exceed a fixed session limit.");
		}
		capture_active = true;
		cancelled.store(false);
		phase = SessionPhase::CaptureConnecting;
	}

	try
	{
		backend->detach();
		{
			std::scoped_lock lock(mutex);
			worker_attached = false;
		}
		attach(WorkerPurpose::Capture, options.memory_limit_mb);
		{
			std::scoped_lock lock(mutex);
			phase = SessionPhase::Capturing;
		}
		auto result = backend->capture(options, cancelled, window_started);
		if(result.compressed_bytes > limits.maximum_trace_bytes)
		{
			throw ProtocolError("session_limit_reached", "Trace exceeds the fixed session byte limit.");
		}
		{
			std::scoped_lock lock(mutex);
			phase = SessionPhase::Validating;
		}
		backend->detach();
		{
			std::scoped_lock lock(mutex);
			worker_attached = false;
			phase = SessionPhase::DrainRestoring;
		}
		attach(WorkerPurpose::Drain, limits.maximum_memory_mb);
		const auto restored_queue_health = wait_for_queue_health(std::chrono::steady_clock::now() + readiness_timeout);
		std::scoped_lock lock(mutex);
		producer_progress = backend->producer_progress();
		queue_health = restored_queue_health;
		++capture_count;
		capture_active = false;
		phase = SessionPhase::Draining;
		idle_condition.notify_all();
		return {result, status_locked()};
	}
	catch(const ProtocolError& error)
	{
		const auto restored = restore_drain_worker();
		{
			std::scoped_lock lock(mutex);
			producer_progress = backend->producer_progress();
			capture_active = false;
			phase = restored ? SessionPhase::Draining : SessionPhase::Failed;
			idle_condition.notify_all();
		}
		auto details = error.details().is_object() ? error.details() : nlohmann::json::object();
		details["window_started"] = window_started.load();
		details["collector_recovered"] = restored;
		throw ProtocolError(error.code(), error.what(), std::move(details));
	}
	catch(...)
	{
		const auto restored = restore_drain_worker();
		std::scoped_lock lock(mutex);
		producer_progress = backend->producer_progress();
		capture_active = false;
		phase = restored ? SessionPhase::Draining : SessionPhase::Failed;
		idle_condition.notify_all();
		throw;
	}
}

SessionStatus CollectorSession::status()
{
	std::scoped_lock lock(mutex);
	if(worker_attached && !capture_active)
	{
		producer_progress = backend->producer_progress();
		if(auto current_health = backend->health(); current_health.has_value())
		{
			queue_health = std::move(current_health);
		}
	}
	return status_locked();
}

SessionStatus CollectorSession::cancel()
{
	std::scoped_lock lock(mutex);
	if(capture_active)
	{
		cancelled.store(true);
	}
	return status_locked();
}

SessionStatus CollectorSession::stop()
{
	std::unique_lock lock(mutex);
	if(phase == SessionPhase::Stopped) return status_locked();
	phase = SessionPhase::Stopping;
	cancelled.store(true);
	if(!idle_condition.wait_for(lock, limits.stop_timeout, [&] { return !capture_active; }))
	{
		phase = SessionPhase::Failed;
		throw ProtocolError("stop_timeout", "Active capture did not stop within the fixed timeout.");
	}
	lock.unlock();
	backend->detach();
	lock.lock();
	worker_attached = false;
	phase = SessionPhase::Stopped;
	return status_locked();
}

SessionStatus CollectorSession::status_locked() const
{
	return {
		phase,
		worker_generation,
		producer_progress,
		capture_count,
		worker_purpose,
		worker_attached,
		transition_retry_count,
		last_transition_error,
		recovery_required,
		queue_health,
	};
}

QueueHealth CollectorSession::wait_for_queue_health(const std::chrono::steady_clock::time_point deadline)
{
	while(true)
	{
		const auto health = backend->health();
		if(health.has_value() && health->capacity > 0 && health->last_producer_progress_raw > 0 && health->hook_installed && health->prologue_validated)
		{
			return *health;
		}
		if(std::chrono::steady_clock::now() >= deadline)
		{
			throw ProtocolError("health_timeout", "Queue health did not become valid during worker readiness.");
		}
		std::this_thread::sleep_for(std::chrono::milliseconds(1));
	}
}

void CollectorSession::attach(const WorkerPurpose purpose, const std::uint64_t memory_limit_mb)
{
	for(std::uint64_t attempt = 1; attempt <= limits.maximum_attach_attempts; ++attempt)
	{
		try
		{
			backend->attach(purpose, memory_limit_mb);
			std::scoped_lock lock(mutex);
			++worker_generation;
			worker_purpose = purpose;
			worker_attached = true;
			return;
		}
		catch(const ProtocolError& error)
		{
			{
				std::scoped_lock lock(mutex);
				worker_attached = false;
				last_transition_error = error.what();
			}
			const auto transient = error.code() == "connect_timeout" || error.code() == "handshake_dropped" || error.code() == "client_disconnected" || error.code() == "profiler_busy";
			if(!transient || attempt == limits.maximum_attach_attempts) throw;
			backend->detach();
			{
				std::scoped_lock lock(mutex);
				++transition_retry_count;
			}
			std::this_thread::sleep_for(limits.transition_retry_delay);
		}
	}
}

bool CollectorSession::restore_drain_worker() noexcept
{
	try
	{
		backend->detach();
		{
			std::scoped_lock lock(mutex);
			worker_attached = false;
			phase = SessionPhase::DrainRestoring;
		}
		attach(WorkerPurpose::Drain, limits.maximum_memory_mb);
		const auto restored_queue_health = wait_for_queue_health(std::chrono::steady_clock::now() + readiness_timeout);
		{
			std::scoped_lock lock(mutex);
			producer_progress = backend->producer_progress();
			queue_health = restored_queue_health;
		}
		return true;
	}
	catch(...)
	{
		std::scoped_lock lock(mutex);
		recovery_required = true;
		return false;
	}
}

}
