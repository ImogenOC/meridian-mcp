[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$EvidencePath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$mcpRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$fixtureRoot = (Resolve-Path -LiteralPath (Join-Path $mcpRoot 'tests/fixtures/runtime')).Path
$compiler = (Resolve-Path -LiteralPath $DreamMakerPath).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
Import-Module (Join-Path $PSScriptRoot 'MeridianMcpSession.psm1') -Force

$dmb = Join-Path $fixtureRoot 'runtime.dmb'
foreach ($artifact in @('runtime.dmb', 'runtime.rsc', 'runtime.pdb')) {
	Remove-Item -LiteralPath (Join-Path $fixtureRoot $artifact) -Force -ErrorAction SilentlyContinue
}

function Request([int]$Id, [string]$Name, [hashtable]$Arguments) {
	return ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = $Id; method = 'tools/call'; params = [ordered]@{ name = $Name; arguments = $Arguments } })
}
$requests = @(
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'meridian-auxtools-integration'; version = '1.0' } } })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 2; method = 'tools/list'; params = [ordered]@{} })),
	(Request 3 'dm_parse_environment' @{ dme_path = (Join-Path $fixtureRoot 'runtime.dme') }),
	(Request 4 'dm_compile' @{ dme_path = (Join-Path $fixtureRoot 'runtime.dme'); compiler_path = $compiler; working_directory = $fixtureRoot; timeout_ms = 120000; idle_timeout_ms = 60000 }),
	(Request 5 'dm_debug_launch' @{ dmb_path = $dmb; startup_timeout_ms = 60000 }),
	(Request 6 'dm_debug_threads' @{}),
	(Request 7 'dm_debug_set_exception_breakpoints' @{ break_on_runtimes = $true }),
	(Request 8 'dm_debug_stop' @{})
)
$environment = @{
	MERIDIAN_MCP_MODE = 'development'
	MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, @($mcpRoot, $fixtureRoot))
	MERIDIAN_MCP_COMPILERS = $compiler
	MERIDIAN_MCP_DEBUGGER = 'auxtools'
}
try {
	$session = Invoke-McpSession -BinaryPath $binary -WorkingDirectory $mcpRoot -Environment $environment -Requests $requests -TimeoutMilliseconds 120000
	if ($session.ExitCode -ne 0) { throw "Debugger MCP session exited with $($session.ExitCode)." }
	foreach ($id in 1..8) {
		$response = Get-McpResponse -Responses $session.Responses -Id $id
		if ($id -ge 3 -and $response.result.isError -eq $true) { throw "Debugger request $id failed: $($response.result.content[0].text)" }
	}
	$tools = (Get-McpResponse -Responses $session.Responses -Id 2).result.tools.name
	if ($tools -notcontains 'dm_debug_launch' -or $tools -notcontains 'dm_debug_stop') { throw 'Debugger tools were not advertised.' }
	$evidence = [ordered]@{ schema_version = 1; overall = 'passed'; auxtools_version = 'v2.3.7'; auxtools_sha256 = 'b188999ac58a0e0171b015c39a403ab7da2f37ddb8ac3817a078f5bce02a8be7'; requests = 8 }
	$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
	New-Item -ItemType Directory -Force -Path (Split-Path -Parent $evidenceFile) | Out-Null
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 5) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
} finally {
	foreach ($artifact in @('runtime.dmb', 'runtime.rsc', 'runtime.pdb')) {
		Remove-Item -LiteralPath (Join-Path $fixtureRoot $artifact) -Force -ErrorAction SilentlyContinue
	}
}
