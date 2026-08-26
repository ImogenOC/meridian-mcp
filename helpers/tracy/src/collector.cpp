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
	const auto initial_progress = backend->producer_progress();
	try
	{
		attach(WorkerPurpose::Drain);
	}
	catch(...)
	{
		std::scoped_lock lock(mutex);
		phase = SessionPhase::Failed;
		throw;
	}
	const auto deadline = std::chrono::steady_clock::now() + std::chrono::milliseconds(options.progress_timeout_ms);
	while(backend->producer_progress() <= initial_progress)
	{
		if(std::chrono::steady_clock::now() >= deadline)
		{
			backend->detach();
			std::scoped_lock lock(mutex);
			phase = SessionPhase::Failed;
			throw ProtocolError("producer_stalled", "Producer progress did not advance during collector readiness.");
		}
		std::this_thread::sleep_for(std::chrono::milliseconds(1));
	}
	std::scoped_lock lock(mutex);
	producer_progress = backend->producer_progress();
	phase = SessionPhase::Draining;
	return status_locked();
}

CaptureWindowResult CollectorSession::capture(const CaptureWindowOptions& options)
{
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
		phase = SessionPhase::Capturing;
	}

	try
	{
		backend->detach();
		attach(WorkerPurpose::Capture);
		auto result = backend->capture(options, cancelled);
		if(result.compressed_bytes > limits.maximum_trace_bytes)
		{
			throw ProtocolError("session_limit_reached", "Trace exceeds the fixed session byte limit.");
		}
		{
			std::scoped_lock lock(mutex);
			phase = SessionPhase::Validating;
		}
		backend->detach();
		attach(WorkerPurpose::Drain);
		std::scoped_lock lock(mutex);
		producer_progress = backend->producer_progress();
		++capture_count;
		capture_active = false;
		phase = SessionPhase::Draining;
		idle_condition.notify_all();
		return {result, status_locked()};
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

SessionStatus CollectorSession::status() const
{
	std::scoped_lock lock(mutex);
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
	phase = SessionPhase::Stopped;
	return status_locked();
}

SessionStatus CollectorSession::status_locked() const
{
	return {phase, worker_generation, producer_progress, capture_count};
}

void CollectorSession::attach(const WorkerPurpose purpose)
{
	backend->attach(purpose);
	std::scoped_lock lock(mutex);
	++worker_generation;
}

bool CollectorSession::restore_drain_worker() noexcept
{
	try
	{
		backend->detach();
		attach(WorkerPurpose::Drain);
		return true;
	}
	catch(...)
	{
		return false;
	}
}

}
