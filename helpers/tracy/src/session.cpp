#include "session.hpp"

#include "collector.hpp"

#include <atomic>
#include <chrono>
#include <cmath>
#include <iomanip>
#include <iostream>
#include <memory>
#include <numeric>
#include <set>
#include <sstream>
#include <stdexcept>
#include <thread>

#include "TracyFileRead.hpp"
#include "TracyFileWrite.hpp"
#include "TracyProtocol.hpp"
#include "TracyWorker.hpp"

namespace meridian::tracy
{

namespace
{

inline constexpr std::uint64_t MinimumValidTraceBytes = 1024;

std::int64_t checked_duration_sum(const std::vector<std::int64_t>& durations)
{
	std::int64_t total = 0;
	for(const auto duration : durations)
	{
		if(duration < 0 || duration > std::numeric_limits<std::int64_t>::max() - total)
		{
			throw ProtocolError("statistics_overflow", "Duration accumulator exceeded the signed 64-bit result range.");
		}
		total += duration;
	}
	return total;
}

void wait_for_live_connection(::tracy::Worker& worker, const std::chrono::milliseconds timeout)
{
	const auto deadline = std::chrono::steady_clock::now() + timeout;
	while(!worker.HasData() || !worker.IsConnected())
	{
		switch(worker.GetHandshakeStatus())
		{
		case ::tracy::HandshakeProtocolMismatch: throw ProtocolError("protocol_mismatch", "Tracy protocol mismatch.");
		case ::tracy::HandshakeNotAvailable: throw ProtocolError("profiler_busy", "Tracy client already has a profiler connection.");
		case ::tracy::HandshakeDropped: throw ProtocolError("handshake_dropped", "Tracy client dropped the handshake.");
		default: break;
		}
		if(std::chrono::steady_clock::now() >= deadline)
		{
			throw ProtocolError("connect_timeout", "Timed out connecting to the Tracy client.");
		}
		std::this_thread::sleep_for(std::chrono::milliseconds(25));
	}
}

void wait_for_statistics(::tracy::Worker& worker)
{
	const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(30);
	while(!worker.AreSourceLocationZonesReady())
	{
		if(std::chrono::steady_clock::now() >= deadline)
		{
			throw ProtocolError("statistics_timeout", "Timed out building Tracy source-location statistics.");
		}
		std::this_thread::sleep_for(std::chrono::milliseconds(10));
	}
}

TraceData extract_trace(
	::tracy::Worker& worker,
	const std::int64_t range_begin,
	const std::int64_t range_end,
	const double nanoseconds_per_tick = 1.0,
	const std::uint64_t raw_base = 0
)
{
	wait_for_statistics(worker);
	TraceData output {};
	std::vector<FrameInterval> trace_frame_intervals;
	output.span_ns = worker.GetLastTime() - worker.GetFirstTime();
	std::set<std::string> source_files;
	for(const auto& [source_id, statistics] : worker.GetSourceLocationZones())
	{
		if(statistics.zones.empty()) continue;
		const auto& source = worker.GetSourceLocation(source_id);
		const auto* file_text = worker.GetString(source.file);
		const std::string file = file_text == nullptr ? std::string {} : std::string(file_text);
		if(!file.empty()) source_files.emplace(file);
		std::vector<std::int64_t> inclusive_durations;
		std::vector<std::int64_t> self_durations;
		for(const auto& thread_zone : statistics.zones)
		{
			const auto* zone = thread_zone.Zone();
			++output.zone_counts.raw_total;
			if(!zone->IsEndValid() || zone->End() <= zone->Start())
			{
				++output.zone_counts.invalid;
				continue;
			}
			const FrameInterval interval {static_cast<std::uint64_t>(zone->Start()), static_cast<std::uint64_t>(zone->End())};
			const auto classification = classify_frame(interval, static_cast<std::uint64_t>(range_begin), static_cast<std::uint64_t>(range_end));
			if(!classification.has_value()) continue;
			++output.zone_counts.intersecting;
			if(*classification == FrameClass::Complete)
			{
				++output.zone_counts.complete;
				++output.zone_counts.analyzed;
				++output.complete_zone_count;
				const auto duration = zone->End() - zone->Start();
				auto self_duration = duration;
				if(zone->HasChildren())
				{
					const auto& children = worker.GetZoneChildren(zone->Child());
					if(children.is_magic())
					{
						for(const auto& child : *reinterpret_cast<const ::tracy::Vector<::tracy::ZoneEvent>*>(&children)) self_duration -= std::max<std::int64_t>(0, child.End() - child.Start());
					}
					else
					{
						for(const auto& child : children) if(child.get() != nullptr) self_duration -= std::max<std::int64_t>(0, child->End() - child->Start());
					}
				}
				inclusive_durations.push_back(duration);
				self_durations.push_back(std::max<std::int64_t>(0, self_duration));
			}
			else if(*classification == FrameClass::LeftBoundary) ++output.zone_counts.partial_left;
			else if(*classification == FrameClass::RightBoundary) ++output.zone_counts.partial_right;
			else ++output.zone_counts.spanning;
		}
		if(inclusive_durations.empty()) continue;
		const auto inclusive_total = checked_duration_sum(inclusive_durations);
		const auto self_total = checked_duration_sum(self_durations);
		const auto inclusive = summarize_frames(inclusive_durations);
		const auto self = summarize_frames(self_durations);
		output.zones.push_back({
			{
				worker.GetZoneName(source),
				file,
				source.line,
			},
			inclusive.count,
			inclusive_total,
			self_total,
			inclusive.minimum,
			inclusive.maximum,
			inclusive.p50,
			inclusive.p95,
			inclusive.p99,
			self.p50,
			self.p95,
			self.p99,
		});
	}
	output.source_file_count = source_files.size();
	if(const auto* frames = worker.GetFramesBase())
	{
		const auto count = worker.GetFrameCount(*frames);
		const auto first_time = worker.GetFirstTime();
		const auto first_raw = raw_base + static_cast<std::uint64_t>(static_cast<double>(std::max<std::int64_t>(0, first_time)) / nanoseconds_per_tick);
		output.frame_durations.reserve(count);
		output.frame_intervals.reserve(count);
		for(std::size_t index = 0; index < count; ++index)
		{
			const auto begin = worker.GetFrameBegin(*frames, index);
			const auto end = worker.GetFrameEnd(*frames, index);
			if(begin >= 0 && end >= 0)
			{
				const FrameInterval trace_interval {static_cast<std::uint64_t>(begin), static_cast<std::uint64_t>(end)};
				trace_frame_intervals.push_back(trace_interval);
				const FrameInterval interval {
					raw_base + static_cast<std::uint64_t>(static_cast<double>(begin) / nanoseconds_per_tick),
					raw_base + static_cast<std::uint64_t>(static_cast<double>(end) / nanoseconds_per_tick),
				};
				const auto has_observed_end = frames->continuous ? index + 1 < count : frames->frames[index].end >= 0;
				if(should_validate_frame(interval, first_raw, has_observed_end))
				{
					output.frame_intervals.push_back(interval);
					const auto classification = classify_frame(trace_interval, static_cast<std::uint64_t>(range_begin), static_cast<std::uint64_t>(range_end));
					if(classification == FrameClass::Complete) output.frame_durations.push_back(worker.GetFrameTime(*frames, index));
				}
			}
		}
		output.frame_counts = count_range(trace_frame_intervals, static_cast<std::uint64_t>(range_begin), static_cast<std::uint64_t>(range_end));
	}
	return output;
}

double latest_plot_value(::tracy::Worker& worker, const std::string& expected_name, const double fallback)
{
	for(const auto* plot : worker.GetPlots())
	{
		const auto* name = worker.GetString(plot->name);
		if(name != nullptr && expected_name == name && !plot->data.empty())
		{
			return plot->data.back().val;
		}
	}
	return fallback;
}

QueueHealth extract_queue_health(::tracy::Worker& worker)
{
	auto lock = worker.ObtainLockForMainThread();
	const auto metric = [&](const char* name, const double fallback = 0.0) {
		return static_cast<std::uint64_t>(std::max(0.0, latest_plot_value(worker, name, fallback)));
	};
	const auto hook_installed = metric("meridian.hook.proc_execution.installed") == 1 &&
		metric("meridian.hook.server_tick.installed") == 1 && metric("meridian.hook.map_send.installed") == 1;
	const auto prologue_validated = metric("meridian.hook.proc_execution.prologue_validated") == 1 &&
		metric("meridian.hook.server_tick.prologue_validated") == 1 && metric("meridian.hook.map_send.prologue_validated") == 1;
	std::ostringstream offset_identity;
	offset_identity << std::hex << metric("meridian.offset_table.identity");
	return {
		metric("meridian.queue.capacity"),
		metric("meridian.queue.depth"),
		metric("meridian.queue.high_water"),
		metric("meridian.queue.saturation_count"),
		metric("meridian.queue.dropped_events"),
		metric("meridian.queue.produced_events"),
		metric("meridian.queue.consumed_events"),
		metric("meridian.producer.last_progress_raw"),
		hook_installed,
		prologue_validated,
		std::to_string(metric("meridian.byond.build")),
		offset_identity.str(),
	};
}

QueueHealth wait_for_queue_health(::tracy::Worker& worker, const std::chrono::milliseconds timeout)
{
	const auto deadline = std::chrono::steady_clock::now() + timeout;
	for(;;)
	{
		auto health = extract_queue_health(worker);
		if(health.capacity > 0 && health.last_producer_progress_raw > 0)
		{
			return health;
		}
		if(!worker.IsConnected())
		{
			throw ProtocolError("client_disconnected", "Tracy client disconnected before health telemetry was ready.");
		}
		if(std::chrono::steady_clock::now() >= deadline)
		{
			throw ProtocolError("health_timeout", "Timed out waiting for named byond-tracy health telemetry.");
		}
		std::this_thread::sleep_for(std::chrono::milliseconds(25));
	}
}

CaptureResult write_trace(
	::tracy::Worker& worker,
	const CaptureWindowOptions& options,
	const std::filesystem::path& output,
	const std::int64_t active_begin,
	const std::int64_t active_end,
	const std::uint64_t raw_begin,
	const std::uint64_t raw_end,
	const double nanoseconds_per_tick,
	const std::uint64_t raw_base,
	const double measured_wall_seconds,
	const QueueHealth& queue_start,
	const QueueHealth& queue_end
)
{
	auto file = std::unique_ptr<::tracy::FileWrite>(::tracy::FileWrite::Open(output.string().c_str(), ::tracy::FileCompression::Zstd, 3, 4));
	if(!file)
	{
		throw ProtocolError("trace_open_failed", "Could not open the trace output file.");
	}
	worker.Write(*file, false);
	file->Finish();
	const auto compression = file->GetCompressionStatistics();
	file.reset();

	bool reopened = false;
	TraceData trace {};
	try
	{
		auto input = std::unique_ptr<::tracy::FileRead>(::tracy::FileRead::Open(output.string().c_str()));
		if(input)
		{
			::tracy::Worker reopened_worker(*input);
			trace = extract_trace(reopened_worker, active_begin, active_end, nanoseconds_per_tick, raw_base);
			reopened = true;
		}
	}
	catch(const std::exception&)
	{
		reopened = false;
	}

	CaptureObservation observation {
		raw_begin,
		raw_end,
		active_begin,
		active_end,
		nanoseconds_per_tick,
		static_cast<double>(options.duration_ms) / 1000.0,
		measured_wall_seconds,
		trace.frame_intervals,
		trace.complete_zone_count,
		static_cast<std::uint64_t>(trace.zones.size()),
		trace.source_file_count,
		std::filesystem::exists(output) ? std::filesystem::file_size(output) : 0,
		MinimumValidTraceBytes,
		reopened,
		queue_start,
		queue_end,
	};
	auto validation = validate_capture(observation);
	return {
		static_cast<std::uint64_t>(trace.frame_intervals.size()),
		static_cast<std::uint64_t>(trace.zones.size()),
		active_end - active_begin,
		static_cast<std::uint64_t>(compression.first),
		static_cast<std::uint64_t>(compression.second),
		std::move(validation),
		options.phase,
		options.phase_iteration,
	};
}

class TracyCollectorBackend final : public CollectorBackend
{
public:
	void configure(const SessionStartOptions& options) override
	{
		host = options.host;
		port = options.port;
		connect_timeout = std::chrono::milliseconds(options.connect_timeout_ms);
	}

	void attach(const WorkerPurpose purpose) override
	{
		std::cerr << "collector.attach.begin purpose=" << (purpose == WorkerPurpose::Drain ? "drain" : "capture") << std::endl;
		if(host != "127.0.0.1")
		{
			throw ProtocolError("invalid_host", "Collector host must be the fixed loopback address.");
		}
		worker = std::make_unique<::tracy::Worker>(host.c_str(), port, static_cast<std::int64_t>(MaximumResidentMemoryMb * 1024 * 1024));
		wait_for_live_connection(*worker, connect_timeout);
		current_purpose = purpose;
		std::cerr << "collector.attach.ready" << std::endl;
	}

	void detach() override
	{
		if(!worker) return;
		std::cerr << "collector.detach.begin" << std::endl;
		worker->Disconnect();
		while(worker->IsConnected()) std::this_thread::sleep_for(std::chrono::milliseconds(10));
		std::cerr << "collector.detach.disconnected" << std::endl;
		worker.reset();
		std::cerr << "collector.detach.complete" << std::endl;
	}

	std::uint64_t producer_progress() const override
	{
		if(!worker) return last_progress;
		auto lock = worker->ObtainLockForMainThread();
		std::uint64_t frames = 0;
		if(const auto* frame_set = worker->GetFramesBase()) frames = worker->GetFrameCount(*frame_set);
		last_progress = worker->GetZoneCount() + frames;
		return last_progress;
	}

	CaptureResult capture(const CaptureWindowOptions& options, const std::atomic_bool& cancelled) override
	{
		std::cerr << "collector.capture.begin" << std::endl;
		if(!worker || current_purpose != WorkerPurpose::Capture)
		{
			throw ProtocolError("session_not_ready", "Capture worker is not attached.");
		}
		const auto nanoseconds_per_tick = worker->GetTimerMultiplier();
		const auto raw_base = worker->GetBaseTime();
		if(!std::isfinite(nanoseconds_per_tick) || nanoseconds_per_tick <= 0.0)
		{
			throw ProtocolError("invalid_clock_conversion", "Tracy reported an invalid timer multiplier.");
		}
		const auto queue_start = wait_for_queue_health(*worker, connect_timeout);
		const auto raw_begin = queue_start.last_producer_progress_raw;
		const auto active_begin = trace_time_from_raw(raw_begin, raw_base, nanoseconds_per_tick);
		if(!active_begin.has_value())
		{
			throw ProtocolError("invalid_clock_conversion", "Could not convert the producer start clock into Tracy time.");
		}
		const auto wall_begin = std::chrono::steady_clock::now();
		const auto deadline = wall_begin + std::chrono::milliseconds(options.duration_ms);
		while(worker->IsConnected() && std::chrono::steady_clock::now() < deadline)
		{
			if(cancelled.load())
			{
				throw ProtocolError("capture_cancelled", "Capture was cancelled.");
			}
			std::this_thread::sleep_for(std::chrono::milliseconds(25));
		}
		const auto wall_end = std::chrono::steady_clock::now();
		if(!worker->IsConnected() && wall_end + std::chrono::milliseconds(100) < deadline)
		{
			throw ProtocolError("client_disconnected", "Tracy client disconnected before the requested capture duration.");
		}
		const auto queue_end = extract_queue_health(*worker);
		const auto raw_end = queue_end.last_producer_progress_raw;
		const auto active_end = trace_time_from_raw(raw_end, raw_base, nanoseconds_per_tick);
		if(!active_end.has_value())
		{
			throw ProtocolError("invalid_clock_conversion", "Could not convert the producer end clock into Tracy time.");
		}
		worker->Disconnect();
		while(worker->IsConnected()) std::this_thread::sleep_for(std::chrono::milliseconds(10));
		return write_trace(
			*worker,
			options,
			options.output_path,
			*active_begin,
			*active_end,
			raw_begin,
			raw_end,
			nanoseconds_per_tick,
			raw_base,
			std::chrono::duration<double>(wall_end - wall_begin).count(),
			queue_start,
			queue_end
		);
	}

private:
	std::string host;
	std::uint16_t port = 0;
	std::chrono::milliseconds connect_timeout {15'000};
	std::unique_ptr<::tracy::Worker> worker;
	WorkerPurpose current_purpose = WorkerPurpose::Drain;
	mutable std::uint64_t last_progress = 0;
};

}

CaptureResult capture_trace(
	const std::uint16_t port,
	const std::uint64_t duration_ms,
	const std::uint64_t memory_limit_mb,
	const std::filesystem::path& output
)
{
	TracyCollectorBackend backend;
	backend.configure({"127.0.0.1", port, 15'000, 15'000});
	backend.attach(WorkerPurpose::Capture);
	std::atomic_bool cancelled {false};
	auto result = backend.capture({duration_ms, memory_limit_mb, output.string(), "legacy", 1}, cancelled);
	backend.detach();
	return result;
}

TraceData load_trace(const std::filesystem::path& path)
{
	auto file = std::unique_ptr<::tracy::FileRead>(::tracy::FileRead::Open(path.string().c_str()));
	if(!file)
	{
		throw ProtocolError("trace_open_failed", "Could not open the Tracy trace.");
	}
	::tracy::Worker worker(*file);
	return extract_trace(worker, worker.GetFirstTime(), worker.GetLastTime());
}

TraceData load_trace(const std::filesystem::path& path, const std::int64_t range_begin, const std::int64_t range_end)
{
	if(range_end <= range_begin || range_begin < 0) throw ProtocolError("invalid_range", "Trace range must be increasing and non-negative.");
	auto file = std::unique_ptr<::tracy::FileRead>(::tracy::FileRead::Open(path.string().c_str()));
	if(!file) throw ProtocolError("trace_open_failed", "Could not open the Tracy trace.");
	::tracy::Worker worker(*file);
	return extract_trace(worker, range_begin, range_end);
}

std::unique_ptr<CollectorBackend> make_tracy_collector_backend()
{
	return std::make_unique<TracyCollectorBackend>();
}

}
