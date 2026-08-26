#include "protocol.hpp"

#include <unordered_map>
#include <unordered_set>

namespace meridian::tracy
{

ProtocolError::ProtocolError(std::string code, std::string message)
	: std::runtime_error(std::move(message)),
	  error_code(std::move(code))
{
}

const std::string& ProtocolError::code() const noexcept
{
	return error_code;
}

Request parse_request(const std::string_view input)
{
	if(input.size() > MaximumRequestBytes)
	{
		throw ProtocolError("request_too_large", "Request exceeds the fixed protocol limit.");
	}

	nlohmann::json document;
	try
	{
		document = nlohmann::json::parse(input);
	}
	catch(const nlohmann::json::exception&)
	{
		throw ProtocolError("invalid_json", "Request is not valid JSON.");
	}
	if(!document.is_object())
	{
		throw ProtocolError("invalid_request", "Request must be a JSON object.");
	}
	static const std::unordered_set<std::string> TopLevelFields {"schema_version", "id", "command", "params"};
	for(const auto& [name, _] : document.items())
	{
		if(!TopLevelFields.contains(name))
		{
			throw ProtocolError("unknown_field", "Request contains an unknown top-level field.");
		}
	}
	if(!document.contains("schema_version") || !document["schema_version"].is_number_unsigned() || document["schema_version"] != ProtocolSchemaVersion)
	{
		throw ProtocolError("unsupported_schema", "Request schema is not supported.");
	}
	if(!document.contains("id") || !document["id"].is_number_unsigned())
	{
		throw ProtocolError("invalid_id", "Request id must be an unsigned integer.");
	}
	if(!document.contains("command") || !document["command"].is_string())
	{
		throw ProtocolError("invalid_command", "Request command must be a string.");
	}
	if(!document.contains("params") || !document["params"].is_object())
	{
		throw ProtocolError("invalid_params", "Request params must be an object.");
	}

	static const std::unordered_map<std::string, Command> Commands {
		{"capture", Command::Capture},
		{"session_start", Command::SessionStart},
		{"capture_window", Command::CaptureWindow},
		{"session_status", Command::SessionStatus},
		{"session_stop", Command::SessionStop},
		{"cancel", Command::Cancel},
		{"hotspots", Command::Hotspots},
		{"zone", Command::Zone},
		{"frame_stats", Command::FrameStats},
		{"compare", Command::Compare},
	};
	const auto command_name = document["command"].get<std::string>();
	const auto command = Commands.find(command_name);
	if(command == Commands.end())
	{
		throw ProtocolError("unsupported_command", "Request command is not supported.");
	}
	const auto validate_params = [&](const std::unordered_set<std::string>& allowed) {
		for(const auto& [name, _] : document["params"].items())
		{
			if(!allowed.contains(name))
			{
				throw ProtocolError("unknown_param", "Request contains an unknown command parameter.");
			}
		}
	};
	switch(command->second)
	{
	case Command::Capture: validate_params({"port", "duration_ms", "memory_limit_mb", "output_path"}); break;
	case Command::SessionStart: validate_params({"host", "port", "connect_timeout_ms", "progress_timeout_ms"}); break;
	case Command::CaptureWindow: validate_params({"duration_ms", "memory_limit_mb", "output_path", "phase", "phase_iteration"}); break;
	case Command::SessionStatus:
	case Command::SessionStop:
	case Command::Cancel: validate_params({}); break;
	case Command::Hotspots: validate_params({"trace_path", "limit", "sort", "range_begin_ns", "range_end_ns"}); break;
	case Command::Zone: validate_params({"trace_path", "name", "limit", "range_begin_ns", "range_end_ns"}); break;
	case Command::FrameStats: validate_params({"trace_path", "range_begin_ns", "range_end_ns"}); break;
	case Command::Compare: validate_params({"baseline_path", "current_path", "minimum_delta_ns", "limit", "baseline_range_begin_ns", "baseline_range_end_ns", "current_range_begin_ns", "current_range_end_ns"}); break;
	}

	return Request {
		document["id"].get<std::uint64_t>(),
		command->second,
		document["params"],
	};
}

nlohmann::json success_response(const std::uint64_t id, nlohmann::json result)
{
	return {
		{"schema_version", ProtocolSchemaVersion},
		{"id", id},
		{"ok", true},
		{"result", std::move(result)},
	};
}

nlohmann::json error_response(const std::uint64_t id, std::string code, std::string message)
{
	return {
		{"schema_version", ProtocolSchemaVersion},
		{"id", id},
		{"ok", false},
		{"error", {
			{"code", std::move(code)},
			{"message", std::move(message)},
		}},
	};
}

}
