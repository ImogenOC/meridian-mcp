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
		std::scoped_lock lock(mutex);
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

	CaptureResult capture(const CaptureWindowOptions&, const std::atomic_bool& cancelled) override
	{
		capture_entered.store(true);
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

	std::atomic_bool block_capture {false};
	std::atomic_bool capture_entered {false};

private:
	mutable std::mutex mutex;
	std::vector<WorkerPurpose> attachments;
	std::atomic_uint64_t progress {0};
	bool attached = false;
};

}

int main()
{
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
	expect_protocol_error([&] { static_cast<void>(session.start({"127.0.0.1", 8086, 100, 100})); }, "session_already_started");

	for(std::uint64_t capture = 1; capture <= 3; ++capture)
	{
		const auto result = session.capture({1, 64, "capture.tracy", "steady_state", static_cast<std::uint32_t>(capture)});
		assert(result.capture.frame_count == 2);
		assert(result.capture.zone_count == 3);
		assert(result.status.phase == SessionPhase::Draining);
		assert(result.status.capture_count == capture);
		assert(result.status.worker_generation == 1 + capture * 2);
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
			cancelled_error.store(error.code() == "capture_cancelled");
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
	return 0;
}
