#pragma once

#include <cstdint>
#include <stdexcept>
#include <string>
#include <string_view>

#include <nlohmann/json.hpp>

namespace meridian::tracy
{

inline constexpr std::uint32_t ProtocolSchemaVersion = 2;
inline constexpr std::size_t MaximumRequestBytes = 1024 * 1024;
inline constexpr std::size_t MaximumResponseBytes = 4 * 1024 * 1024;
inline constexpr std::uint64_t MaximumSessionSeconds = 30 * 60;
inline constexpr std::uint64_t MaximumResidentMemoryMb = 4096;
inline constexpr std::uint64_t MaximumTraceBytes = 2ULL * 1024 * 1024 * 1024;
inline constexpr std::uint64_t MaximumCaptureDurationMs = 300'000;
inline constexpr std::uint64_t MaximumCaptureCount = 64;

enum class Command
{
	Capture,
	SessionStart,
	CaptureWindow,
	SessionStatus,
	SessionStop,
	Cancel,
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

