#include "collector.hpp"
#include "session_json.hpp"

#include <atomic>
#ifdef NDEBUG
#undef NDEBUG
#endif
#include <cassert>
#include <chrono>
#include <mutex>
#include <string>
#include <thread>
#include <vector>

using namespace meridian::tracy;

namespace
{

template<typename Function>
void expect_protocol_error(Function&& function, const std::string& code)
{
	try
	{
		function();
		assert(false && "expected protocol error");
	}
	catch(const ProtocolError& error)
	{
		assert(error.code() == code);
	}
}

class FakeBackend final : public CollectorBackend
{
public:
	void attach(const WorkerPurpose purpose) override
	{
		if(purpose == WorkerPurpose::Capture)
		{
			capture_attach_entered.store(true);
			while(block_capture_attach.load()) std::this_thread::sleep_for(std::chrono::milliseconds(1));
		}
		std::scoped_lock lock(mutex);
		if(purpose == WorkerPurpose::Capture && capture_attach_failures > 0)
		{
			--capture_attach_failures;
			++capture_attach_attempts;
			throw ProtocolError("connect_timeout", "transient capture attach failure");
		}
		if(purpose == WorkerPurpose::Capture) ++capture_attach_attempts;
		attachments.push_back(purpose);
		attached = true;
		progress += 10;
	}

	void detach() override
	{
		std::scoped_lock lock(mutex);
		attached = false;
	}

	std::uint64_t producer_progress() const override
	{
		return progress.load();
	}

	std::optional<QueueHealth> health() override
	{
		if(!health_ready.load()) return std::nullopt;
		return QueueHealth {1024, 4, 12, 7, 0, 0, progress.load(), progress.load() - 1, progress.load(), true, true, "516.1687", "fixture-offsets"};
	}

	CaptureResult capture(const CaptureWindowOptions&, const std::atomic_bool& cancelled, std::atomic_bool& window_started) override
	{
		capture_entered.store(true);
		window_started.store(true);
		while(block_capture.load() && !cancelled.load())
		{
			std::this_thread::sleep_for(std::chrono::milliseconds(1));
		}
		if(cancelled.load())
		{
			throw ProtocolError("capture_cancelled", "Capture was cancelled.");
		}
		progress += 100;
		return {2, 3, 1'000'000, 4096, 2048};
	}

	std::vector<WorkerPurpose> attachment_history() const
	{
		std::scoped_lock lock(mutex);
		return attachments;
	}

	void advance_progress(const std::uint64_t amount)
	{
		progress += amount;
	}

	std::atomic_bool block_capture {false};
	std::atomic_bool capture_entered {false};
	std::atomic_bool block_capture_attach {false};
	std::atomic_bool capture_attach_entered {false};
	std::atomic_bool health_ready {true};
	std::uint64_t capture_attach_failures = 0;
	std::uint64_t capture_attach_attempts = 0;

private:
	mutable std::mutex mutex;
	std::vector<WorkerPurpose> attachments;
	std::atomic_uint64_t progress {0};
	bool attached = false;
};

}

int main()
{
	{
		auto backend = std::make_unique<FakeBackend>();
		backend->health_ready.store(false);
		CollectorSession session(std::move(backend), {.maximum_capture_count = 3});
		expect_protocol_error([&] { static_cast<void>(session.start({"127.0.0.1", 8086, 5, 100})); }, "health_timeout");
	}

	auto backend = std::make_unique<FakeBackend>();
	auto* fake = backend.get();
	CollectorSession session(std::move(backend), {.maximum_capture_count = 3});
	const auto started = session.start({"127.0.0.1", 8086, 100, 100});
	assert(started.phase == SessionPhase::Draining);
	assert(started.worker_generation == 1);
	assert(started.producer_progress > 0);
	const auto started_json = session_status_json(started);
	assert(started_json.at("state") == "draining");
	assert(started_json.at("worker_generation") == 1);
	assert(started_json.at("worker_attached") == true);
	assert(started_json.at("queue_health").at("capacity") == 1024);
	assert(started_json.at("queue_health").at("tail_refresh_count") == 7);
	assert(started_json.at("queue_health").at("hook_installed") == true);
	assert(started_json.at("queue_health").at("prologue_validated") == true);
	fake->advance_progress(5);
	const auto refreshed = session.status();
	assert(refreshed.producer_progress == started.producer_progress + 5);
	assert(refreshed.queue_health.has_value());
	assert(refreshed.queue_health->last_producer_progress_raw == refreshed.producer_progress);
	expect_protocol_error([&] { static_cast<void>(session.start({"127.0.0.1", 8086, 100, 100})); }, "session_already_started");

	for(std::uint64_t capture = 1; capture <= 3; ++capture)
	{
		std::thread restore_health;
		if(capture == 1)
		{
			fake->health_ready.store(false);
			restore_health = std::thread([&] {
				while(fake->attachment_history().size() < 3) std::this_thread::sleep_for(std::chrono::milliseconds(1));
				std::this_thread::sleep_for(std::chrono::milliseconds(5));
				fake->health_ready.store(true);
			});
		}
		const auto result = session.capture({1, 64, "capture.tracy", "steady_state", static_cast<std::uint32_t>(capture)});
		if(restore_health.joinable()) restore_health.join();
		assert(result.capture.frame_count == 2);
		assert(result.capture.zone_count == 3);
		assert(result.status.phase == SessionPhase::Draining);
		assert(result.status.capture_count == capture);
		assert(result.status.worker_generation == 1 + capture * 2);
		assert(result.status.queue_health.has_value());
		assert(result.status.queue_health->capacity > 0);
	}
	expect_protocol_error([&] { static_cast<void>(session.capture({1, 64, "capture.tracy", "steady_state", 4})); }, "session_limit_reached");

	const auto history = fake->attachment_history();
	assert(history.size() == 7);
	assert(history.front() == WorkerPurpose::Drain);
	for(std::size_t index = 1; index < history.size(); index += 2)
	{
		assert(history[index] == WorkerPurpose::Capture);
		assert(history[index + 1] == WorkerPurpose::Drain);
	}
	assert(session.stop().phase == SessionPhase::Stopped);
	assert(session.stop().phase == SessionPhase::Stopped);

	auto cancel_backend = std::make_unique<FakeBackend>();
	auto* cancel_fake = cancel_backend.get();
	cancel_fake->block_capture.store(true);
	CollectorSession cancellable(std::move(cancel_backend), {});
	static_cast<void>(cancellable.start({"127.0.0.1", 8086, 100, 100}));
	std::atomic_bool cancelled_error {false};
	std::thread capture_thread([&] {
		try
		{
			static_cast<void>(cancellable.capture({30'000, 64, "capture.tracy"}));
		}
		catch(const ProtocolError& error)
		{
			cancelled_error.store(
				error.code() == "capture_cancelled" &&
				error.details().at("window_started") == true &&
				error.details().at("collector_recovered") == true
			);
		}
	});
	while(!cancel_fake->capture_entered.load()) std::this_thread::sleep_for(std::chrono::milliseconds(1));
	assert(cancellable.status().phase == SessionPhase::Capturing);
	expect_protocol_error([&] { static_cast<void>(cancellable.capture({1, 64, "other.tracy"})); }, "capture_busy");
	assert(cancellable.cancel().phase == SessionPhase::Capturing);
	capture_thread.join();
	assert(cancelled_error.load());
	assert(cancellable.status().phase == SessionPhase::Draining);
	assert(cancellable.status().worker_generation == 3);

	auto retry_backend = std::make_unique<FakeBackend>();
	auto* retry_fake = retry_backend.get();
	retry_fake->capture_attach_failures = 1;
	SessionLimits retry_limits;
	retry_limits.transition_retry_delay = std::chrono::milliseconds(0);
	CollectorSession retrying(std::move(retry_backend), retry_limits);
	static_cast<void>(retrying.start({"127.0.0.1", 8086, 100, 100}));
	const auto retried = retrying.capture({1, 64, "capture.tracy", "steady_state", 1});
	assert(retried.capture.frame_count == 2);
	assert(retry_fake->capture_attach_attempts == 2);
	assert(retried.status.transition_retry_count == 1);
	assert(retried.status.worker_purpose == WorkerPurpose::Drain);
	const auto retried_json = session_status_json(retried.status);
	assert(retried_json.at("worker_purpose") == "drain");
	assert(retried_json.at("transition_retry_count") == 1);
	assert(retried_json.at("recovery_required") == false);
	static_cast<void>(retrying.stop());

	auto transition_backend = std::make_unique<FakeBackend>();
	auto* transition_fake = transition_backend.get();
	transition_fake->block_capture_attach.store(true);
	CollectorSession transitioning(std::move(transition_backend), {});
	static_cast<void>(transitioning.start({"127.0.0.1", 8086, 100, 100}));
	std::thread transition_thread([&] { static_cast<void>(transitioning.capture({1, 64, "capture.tracy", "steady_state", 1})); });
	while(!transition_fake->capture_attach_entered.load()) std::this_thread::sleep_for(std::chrono::milliseconds(1));
	const auto connecting = transitioning.status();
	assert(connecting.phase == SessionPhase::CaptureConnecting);
	assert(!connecting.worker_attached);
	assert(session_status_json(connecting).at("state") == "capture_connecting");
	transition_fake->block_capture_attach.store(false);
	transition_thread.join();
	static_cast<void>(transitioning.stop());
	return 0;
}
