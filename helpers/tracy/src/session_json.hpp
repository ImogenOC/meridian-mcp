#pragma once

#include "collector.hpp"

#include <nlohmann/json.hpp>

namespace meridian::tracy
{

[[nodiscard]] const char* phase_name(SessionPhase phase) noexcept;
[[nodiscard]] nlohmann::json session_status_json(const SessionStatus& status);
[[nodiscard]] nlohmann::json validation_json(const CaptureValidation& validation);
[[nodiscard]] nlohmann::json capture_result_json(const CaptureWindowResult& result);

}
