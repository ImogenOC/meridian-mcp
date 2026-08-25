#include "protocol.hpp"

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
	const auto request = parse_request(R"({"schema_version":1,"id":7,"command":"frame_stats","params":{"trace_path":"trace.tracy"}})");
	assert(request.id == 7);
	assert(request.command == Command::FrameStats);
	assert(request.params.at("trace_path") == "trace.tracy");

	expect_error([] { static_cast<void>(parse_request("not json")); }, "invalid_json");
	expect_error([] { static_cast<void>(parse_request(R"({"schema_version":2,"id":1,"command":"frame_stats","params":{}})")); }, "unsupported_schema");
	expect_error([] { static_cast<void>(parse_request(R"({"schema_version":1,"id":1,"command":"eval","params":{}})")); }, "unsupported_command");
	expect_error([] { static_cast<void>(parse_request(R"({"schema_version":1,"id":1,"command":"frame_stats","params":[]})")); }, "invalid_params");
	expect_error([] { static_cast<void>(parse_request(std::string(1024 * 1024 + 1, 'x'))); }, "request_too_large");

	const auto success = meridian::tracy::success_response(9, {{"frame_count", 42}});
	assert(success.at("schema_version") == 1);
	assert(success.at("id") == 9);
	assert(success.at("ok") == true);
	assert(success.at("result").at("frame_count") == 42);

	const auto failure = meridian::tracy::error_response(10, "bad_trace", "Trace is invalid.");
	assert(failure.at("ok") == false);
	assert(failure.at("error").at("code") == "bad_trace");
	return 0;
}
