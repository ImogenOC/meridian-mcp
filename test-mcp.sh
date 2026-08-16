#!/usr/bin/env bash
# Cross-platform Meridian MCP protocol smoke test.

set -euo pipefail

script_dir="${BASH_SOURCE[0]}"
if [[ "$script_dir" != */* ]]; then
    script_dir="."
else
    script_dir="${script_dir%/*}"
fi
script_dir="$(cd -- "$script_dir" && pwd)"
cd "$script_dir"

configuration="release"
dme_path="${DM_MCP_DME:-}"
binary_path="${DM_MCP_BINARY:-}"
skip_build="${DM_MCP_SKIP_BUILD:-0}"
timeout_seconds="${DM_MCP_TIMEOUT_SECONDS:-30}"

while (($# > 0)); do
    case "$1" in
        --dme)
            dme_path="$2"
            shift 2
            ;;
        --binary)
            binary_path="$2"
            shift 2
            ;;
        --debug)
            configuration="debug"
            shift
            ;;
        --skip-build)
            skip_build=1
            shift
            ;;
        *)
            printf 'Unknown argument: %s\n' "$1" >&2
            exit 2
            ;;
    esac
done

if [[ "${OS:-}" == "Windows_NT" ]]; then
    if command -v pwsh >/dev/null 2>&1; then
        powershell_command="$(command -v pwsh)"
    elif command -v powershell.exe >/dev/null 2>&1; then
        powershell_command="$(command -v powershell.exe)"
    else
        printf 'PowerShell is required on Windows for the Meridian MCP smoke test\n' >&2
        exit 1
    fi

    if command -v cygpath >/dev/null 2>&1; then
        powershell_script="$(cygpath -w "$script_dir/test_mcp.ps1")"
    elif [[ "$script_dir" =~ ^/[a-zA-Z]/ ]]; then
        drive_letter="${script_dir:1:1}"
        drive_letter="${drive_letter^^}"
        powershell_script="${drive_letter}:${script_dir:2}/test_mcp.ps1"
    else
        powershell_script="$script_dir/test_mcp.ps1"
    fi
    powershell_args=(-NoProfile -File "$powershell_script" -Configuration "$configuration" -TimeoutSeconds "$timeout_seconds")
    if [[ "$skip_build" == "1" ]]; then
        powershell_args+=(-SkipBuild)
    fi
    if [[ -n "$binary_path" ]]; then
        powershell_args+=(-BinaryPath "$binary_path")
    fi
    if [[ -n "$dme_path" ]]; then
        powershell_args+=(-DmePath "$dme_path")
    fi
    exec "$powershell_command" "${powershell_args[@]}"
fi

binary_name="meridian-mcp"
if [[ "${OS:-}" == "Windows_NT" ]]; then
    binary_name="meridian-mcp.exe"
fi
if [[ -n "$binary_path" ]]; then
    binary="$binary_path"
else
    binary="$script_dir/target/$configuration/$binary_name"
fi

if [[ "$skip_build" != "1" ]]; then
    build_args=(build)
    if [[ "$configuration" == "release" ]]; then
        build_args+=(--release)
    fi
    cargo "${build_args[@]}"
fi

if [[ ! -x "$binary" && ! -f "$binary" ]]; then
    printf 'meridian-mcp binary not found: %s\n' "$binary" >&2
    exit 1
fi
if ! command -v jq >/dev/null 2>&1; then
    printf 'jq is required for the protocol smoke test\n' >&2
    exit 1
fi

run_session() {
    local input="$1"
    timeout "$timeout_seconds" "$binary" <<< "$input"
}

initialize_request='{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"meridian-mcp-smoke-test","version":"1.0"}}}'
list_request='{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}'

protocol_result="$(run_session "${initialize_request}
${list_request}")"
if ! jq -s -e 'any(.[]; .id == 1 and .result.protocolVersion == "2024-11-05")' <<< "$protocol_result" >/dev/null; then
    printf 'initialize failed:\n%s\n' "$protocol_result" >&2
    exit 1
fi
if ! jq -s -e 'any(.[]; .id == 1 and .result.serverInfo.name == "meridian-mcp")' <<< "$protocol_result" >/dev/null; then
    printf 'initialize returned unexpected serverInfo.name:\n%s\n' "$protocol_result" >&2
    exit 1
fi

tool_count="$(jq -s 'map(select(.id == 2))[0].result.tools | length' <<< "$protocol_result")"
if [[ "$tool_count" -le 0 ]]; then
    printf 'tools/list returned no tools:\n%s\n' "$protocol_result" >&2
    exit 1
fi
for required_tool in dm_parse_environment dm_compile dm_run dm_wait_for_output; do
    if ! jq -s -e --arg name "$required_tool" 'map(select(.id == 2))[0].result.tools[] | select(.name == $name)' <<< "$protocol_result" >/dev/null; then
        printf 'tools/list is missing required tool: %s\n' "$required_tool" >&2
        exit 1
    fi
done
printf 'MCP smoke test passed: protocol 2024-11-05, %s tools\n' "$tool_count"

if [[ -n "$dme_path" ]]; then
    if [[ ! -f "$dme_path" ]]; then
        printf 'DME file not found: %s\n' "$dme_path" >&2
        exit 1
    fi
    absolute_dme_path="$(realpath "$dme_path")"
    parse_request="$(jq -cn --arg path "$absolute_dme_path" '{jsonrpc:"2.0",id:3,method:"tools/call",params:{name:"dm_parse_environment",arguments:{dme_path:$path}}}')"
    parse_result="$(run_session "${initialize_request}
${parse_request}")"
    if ! jq -s -e 'any(.[]; .id == 3 and .result.isError != true)' <<< "$parse_result" >/dev/null; then
        printf 'dm_parse_environment failed:\n%s\n' "$parse_result" >&2
        exit 1
    fi
    printf 'DME parse smoke test passed: %s\n' "$dme_path"
fi
