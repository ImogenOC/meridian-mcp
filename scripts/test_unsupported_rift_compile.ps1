[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$BinaryPath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Import-Module (Join-Path $repoRoot 'scripts\MeridianMcpSession.psm1') -Force

function New-JsonLine {
	param([Parameter(Mandatory)][System.Collections.IDictionary]$Value)
	return ConvertTo-McpJsonLine $Value
}

$requests = @(
	(New-JsonLine ([ordered]@{
		jsonrpc = '2.0'; id = 1; method = 'initialize'
		params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'unsupported-rift-test'; version = '1.0' } }
	})),
	(New-JsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })),
	(New-JsonLine ([ordered]@{
		jsonrpc = '2.0'; id = 2; method = 'tools/call'
		params = [ordered]@{ name = 'rift_compile'; arguments = [ordered]@{} }
	}))
)
$session = Invoke-McpSession `
	-BinaryPath $BinaryPath `
	-WorkingDirectory $repoRoot `
	-Environment @{
		MERIDIAN_MCP_MODE = 'development'
		MERIDIAN_MCP_ROOTS = $repoRoot
		MERIDIAN_MCP_RIFT_BUILD = 'network'
	} `
	-Requests $requests `
	-TimeoutMilliseconds 30000

if ($session.ExitCode -ne 0) {
	throw "meridian-mcp exited with $($session.ExitCode): $($session.Stderr)"
}
$response = Get-McpResponse -Responses $session.Responses -Id 2
if ($response.result.isError -ne $true) {
	throw 'A stale-schema rift_compile call unexpectedly succeeded on a non-Windows platform.'
}
$payload = $response.result.content[0].text | ConvertFrom-Json
if ($payload.code -ne 'unsupported_platform') {
	throw "Expected unsupported_platform, received: $($response.result.content[0].text)"
}
Write-Output 'Non-Windows stale-schema rift_compile rejection passed.'
