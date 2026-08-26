#include "protocol.hpp"

#ifdef NDEBUG
#undef NDEBUG
#endif
#include <cassert>
#include <string>

using meridian::tracy::Command;
using meridian::tracy::ProtocolError;
using meridian::tracy::parse_request;

template<typename Function>
void expect_error(Function&& function, const std::string& code)
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

int main()
{
	const auto request = parse_request(R"({"schema_version":2,"id":7,"command":"frame_stats","params":{"trace_path":"trace.tracy"}})");
	assert(request.id == 7);
	assert(request.command == Command::FrameStats);
	assert(request.params.at("trace_path") == "trace.tracy");
	assert(parse_request(R"({"schema_version":2,"id":8,"command":"frame_stats","params":{"trace_path":"trace.tracy","range_begin_ns":1,"range_end_ns":2}})").command == Command::FrameStats);

	const auto start = parse_request(R"({"schema_version":2,"id":9,"command":"session_start","params":{"host":"127.0.0.1","port":8086,"connect_timeout_ms":15000,"progress_timeout_ms":15000}})");
	assert(start.command == Command::SessionStart);
	assert(parse_request(R"({"schema_version":2,"id":9,"command":"capture_window","params":{"duration_ms":30000,"memory_limit_mb":512,"output_path":"capture.tracy","phase":"steady_state","phase_iteration":1}})").command == Command::CaptureWindow);
	assert(parse_request(R"({"schema_version":2,"id":10,"command":"session_status","params":{}})").command == Command::SessionStatus);
	assert(parse_request(R"({"schema_version":2,"id":11,"command":"session_stop","params":{}})").command == Command::SessionStop);
	assert(parse_request(R"({"schema_version":2,"id":12,"command":"cancel","params":{}})").command == Command::Cancel);

	expect_error([] { static_cast<void>(parse_request("not json")); }, "invalid_json");
	expect_error([] { static_cast<void>(parse_request(R"({"schema_version":1,"id":1,"command":"frame_stats","params":{}})")); }, "unsupported_schema");
	expect_error([] { static_cast<void>(parse_request(R"({"schema_version":2,"id":1,"command":"eval","params":{}})")); }, "unsupported_command");
	expect_error([] { static_cast<void>(parse_request(R"({"schema_version":2,"id":1,"command":"frame_stats","params":[]})")); }, "invalid_params");
	expect_error([] { static_cast<void>(parse_request(R"({"schema_version":2,"id":1,"command":"session_status","params":{},"extra":true})")); }, "unknown_field");
	expect_error([] { static_cast<void>(parse_request(R"({"schema_version":2,"id":1,"command":"session_status","params":{"extra":true}})")); }, "unknown_param");
	expect_error([] { static_cast<void>(parse_request(std::string(1024 * 1024 + 1, 'x'))); }, "request_too_large");

	const auto success = meridian::tracy::success_response(9, {{"frame_count", 42}});
	assert(success.at("schema_version") == 2);
	assert(success.at("id") == 9);
	assert(success.at("ok") == true);
	assert(success.at("result").at("frame_count") == 42);

	const auto failure = meridian::tracy::error_response(10, "bad_trace", "Trace is invalid.");
	assert(failure.at("ok") == false);
	assert(failure.at("error").at("code") == "bad_trace");
	return 0;
}
