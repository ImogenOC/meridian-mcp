#include "session.hpp"

#include <chrono>
#include <memory>
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

void wait_for_live_connection(::tracy::Worker& worker)
{
	const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(15);
	while(!worker.HasData() || !worker.IsConnected())
	{
		switch(worker.GetHandshakeStatus())
		{
		case ::tracy::HandshakeProtocolMismatch: throw std::runtime_error("Tracy protocol mismatch.");
		case ::tracy::HandshakeNotAvailable: throw std::runtime_error("Tracy client already has a profiler connection.");
		case ::tracy::HandshakeDropped: throw std::runtime_error("Tracy client dropped the handshake.");
		default: break;
		}
		if(std::chrono::steady_clock::now() >= deadline)
		{
			throw std::runtime_error("Timed out connecting to the Tracy client.");
		}
		std::this_thread::sleep_for(std::chrono::milliseconds(50));
	}
}

void wait_for_statistics(::tracy::Worker& worker)
{
	const auto deadline = std::chrono::steady_clock::now() + std::chrono::seconds(30);
	while(!worker.AreSourceLocationZonesReady())
	{
		if(std::chrono::steady_clock::now() >= deadline)
		{
			throw std::runtime_error("Timed out building Tracy source-location statistics.");
		}
		std::this_thread::sleep_for(std::chrono::milliseconds(10));
	}
}

TraceData extract_trace(::tracy::Worker& worker)
{
	wait_for_statistics(worker);
	TraceData output;
	output.span_ns = worker.GetLastTime() - worker.GetFirstTime();
	for(const auto& [source_id, statistics] : worker.GetSourceLocationZones())
	{
		if(statistics.zones.empty()) continue;
		const auto& source = worker.GetSourceLocation(source_id);
		output.zones.push_back({
			{
				worker.GetZoneName(source),
				worker.GetString(source.file),
				source.line,
			},
			static_cast<std::uint64_t>(statistics.zones.size()),
			statistics.total,
			statistics.selfTotal,
			statistics.min,
			statistics.max,
		});
	}
	if(const auto* frames = worker.GetFramesBase())
	{
		const auto count = worker.GetFullFrameCount(*frames);
		output.frame_durations.reserve(count);
		for(std::size_t index = 0; index < count; ++index)
		{
			output.frame_durations.push_back(worker.GetFrameTime(*frames, index));
		}
	}
	return output;
}

}

CaptureResult capture_trace(
	const std::uint16_t port,
	const std::uint64_t duration_ms,
	const std::uint64_t memory_limit_mb,
	const std::filesystem::path& output
)
{
	const auto memory_limit = static_cast<std::int64_t>(memory_limit_mb * 1024 * 1024);
	::tracy::Worker worker("127.0.0.1", port, memory_limit);
	wait_for_live_connection(worker);
	const auto started = std::chrono::steady_clock::now();
	while(worker.IsConnected() && std::chrono::steady_clock::now() - started < std::chrono::milliseconds(duration_ms))
	{
		std::this_thread::sleep_for(std::chrono::milliseconds(50));
	}
	const auto elapsed = std::chrono::steady_clock::now() - started;
	if(!worker.IsConnected() && elapsed + std::chrono::milliseconds(100) < std::chrono::milliseconds(duration_ms))
	{
		throw std::runtime_error("Tracy client disconnected before the requested capture duration.");
	}
	worker.Disconnect();
	while(worker.IsConnected())
	{
		std::this_thread::sleep_for(std::chrono::milliseconds(25));
	}

	auto file = std::unique_ptr<::tracy::FileWrite>(::tracy::FileWrite::Open(output.string().c_str(), ::tracy::FileCompression::Zstd, 3, 4));
	if(!file)
	{
		throw std::runtime_error("Could not open the trace output file.");
	}
	worker.Write(*file, false);
	file->Finish();
	const auto compression = file->GetCompressionStatistics();
	const auto* frames = worker.GetFramesBase();
	return {
		frames ? static_cast<std::uint64_t>(worker.GetFrameCount(*frames)) : 0,
		worker.GetZoneCount(),
		worker.GetLastTime() - worker.GetFirstTime(),
		static_cast<std::uint64_t>(compression.first),
		static_cast<std::uint64_t>(compression.second),
	};
}

TraceData load_trace(const std::filesystem::path& path)
{
	auto file = std::unique_ptr<::tracy::FileRead>(::tracy::FileRead::Open(path.string().c_str()));
	if(!file)
	{
		throw std::runtime_error("Could not open the Tracy trace.");
	}
	::tracy::Worker worker(*file);
	return extract_trace(worker);
}

}
