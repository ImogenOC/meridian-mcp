[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$EvidencePath,
	[Parameter(Mandatory)][string]$DmbPath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$mcpRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$compiler = (Resolve-Path -LiteralPath $DreamMakerPath).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
Import-Module (Join-Path $PSScriptRoot 'MeridianMcpSession.psm1') -Force

$dmb = (Resolve-Path -LiteralPath $DmbPath).Path
$runtimeRoot = (Split-Path -Parent $dmb)

function Request([int]$Id, [string]$Name, [hashtable]$Arguments) {
	return ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = $Id; method = 'tools/call'; params = [ordered]@{ name = $Name; arguments = $Arguments } })
}
$requests = @(
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'meridian-auxtools-integration'; version = '1.0' } } })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 2; method = 'tools/list'; params = [ordered]@{} })),
	(Request 3 'dm_debug_launch' @{ dmb_path = $dmb; startup_timeout_ms = 60000 }),
	(Request 4 'dm_debug_threads' @{}),
	(Request 5 'dm_debug_set_exception_breakpoints' @{ break_on_runtimes = $true }),
	(Request 6 'dm_debug_stop' @{})
)
$environment = @{
	MERIDIAN_MCP_MODE = 'development'
	MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, (@($mcpRoot, $runtimeRoot) | Select-Object -Unique))
	MERIDIAN_MCP_COMPILERS = $compiler
	MERIDIAN_MCP_DEBUGGER = 'auxtools'
}
$session = Invoke-McpSession -BinaryPath $binary -WorkingDirectory $mcpRoot -Environment $environment -Requests $requests -TimeoutMilliseconds 120000
if ($session.ExitCode -ne 0) { throw "Debugger MCP session exited with $($session.ExitCode)." }
foreach ($id in 1..6) {
	$response = Get-McpResponse -Responses $session.Responses -Id $id
	if ($id -ge 3 -and $response.result.isError -eq $true) { throw "Debugger request $id failed: $($response.result.content[0].text)" }
}
$tools = (Get-McpResponse -Responses $session.Responses -Id 2).result.tools.name
if ($tools -notcontains 'dm_debug_launch' -or $tools -notcontains 'dm_debug_stop') { throw 'Debugger tools were not advertised.' }
$evidence = [ordered]@{ schema_version = 1; overall = 'passed'; auxtools_version = 'v2.3.7'; auxtools_sha256 = 'b188999ac58a0e0171b015c39a403ab7da2f37ddb8ac3817a078f5bce02a8be7'; dmb_sha256 = (Get-FileHash -LiteralPath $dmb -Algorithm SHA256).Hash.ToLowerInvariant(); requests = 6 }
$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $evidenceFile) | Out-Null
[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 5) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
