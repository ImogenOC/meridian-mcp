#include "protocol.hpp"

#include <unordered_map>

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
