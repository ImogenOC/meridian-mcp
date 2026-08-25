#pragma once

#include <cstdint>
#include <stdexcept>
#include <string>
#include <string_view>

#include <nlohmann/json.hpp>

namespace meridian::tracy
{

inline constexpr std::uint32_t ProtocolSchemaVersion = 1;
inline constexpr std::size_t MaximumRequestBytes = 1024 * 1024;

enum class Command
{
	Capture,
	Hotspots,
	Zone,
	FrameStats,
	Compare,
};

struct Request
{
	std::uint64_t id;
	Command command;
	nlohmann::json params;
};

class ProtocolError final : public std::runtime_error
{
public:
	ProtocolError(std::string code, std::string message);

	[[nodiscard]] const std::string& code() const noexcept;

private:
	std::string error_code;
};

[[nodiscard]] Request parse_request(std::string_view input);
[[nodiscard]] nlohmann::json success_response(std::uint64_t id, nlohmann::json result);
[[nodiscard]] nlohmann::json error_response(std::uint64_t id, std::string code, std::string message);

}

