[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$HelperManifestPath,
	[Parameter(Mandatory)][string]$EvidencePath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$mcpRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$fixtureRoot = (Resolve-Path -LiteralPath (Join-Path $mcpRoot 'tests/fixtures/tracy')).Path
$compiler = (Resolve-Path -LiteralPath $DreamMakerPath).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$manifest = (Resolve-Path -LiteralPath $HelperManifestPath).Path
Import-Module (Join-Path $PSScriptRoot 'MeridianMcpSession.psm1') -Force

function Request([int]$Id, [string]$Name, [hashtable]$Arguments) {
	ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = $Id; method = 'tools/call'; params = [ordered]@{ name = $Name; arguments = $Arguments } })
}

$dmb = Join-Path $fixtureRoot 'tracy.dmb'
$trace = Join-Path $fixtureRoot 'meridian-owned.tracy'
foreach ($artifact in @('tracy.dmb', 'tracy.rsc', 'tracy.pdb', 'tracy.log', 'prof.dll', 'libprof.so', 'meridian-owned.tracy')) {
	Remove-Item -LiteralPath (Join-Path $fixtureRoot $artifact) -Force -ErrorAction SilentlyContinue
}

& $compiler (Join-Path $fixtureRoot 'tracy.dme')
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $dmb -PathType Leaf)) { throw 'The Tracy fixture did not compile.' }

$requests = @(
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'meridian-tracy-integration'; version = '1.0' } } })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 2; method = 'tools/list'; params = [ordered]@{} })),
	(Request 3 'dm_tracy_prepare' @{ dmb_path = $dmb }),
	(Request 4 'dm_tracy_launch' @{ dmb_path = $dmb; game_port = 14569 }),
	(Request 5 'dm_tracy_capture' @{ output_path = $trace; duration_ms = 5000; memory_limit_mb = 256; capture_network = $true }),
	(Request 6 'dm_tracy_hotspots' @{ trace_path = $trace; limit = 100; sort = 'inclusive' }),
	(Request 7 'dm_tracy_zone' @{ trace_path = $trace; name = '/proc/meridian_profile_work'; limit = 10 }),
	(Request 8 'dm_tracy_frame_stats' @{ trace_path = $trace }),
	(Request 9 'dm_tracy_compare' @{ baseline_path = $trace; current_path = $trace; limit = 100; minimum_delta_ns = 0 }),
	(Request 10 'dm_tracy_status' @{}),
	(Request 11 'dm_tracy_stop' @{})
)
$environment = @{
	MERIDIAN_MCP_MODE = 'development'
	MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, @($mcpRoot, $fixtureRoot))
	MERIDIAN_MCP_COMPILERS = $compiler
	MERIDIAN_MCP_HELPER_MANIFEST = $manifest
	MERIDIAN_MCP_TRACY = 'byond'
}

try {
	$session = Invoke-McpSession -BinaryPath $binary -WorkingDirectory $mcpRoot -Environment $environment -Requests $requests -TimeoutMilliseconds 240000
	if ($session.ExitCode -ne 0) { throw "Tracy MCP session exited with $($session.ExitCode)." }
	$failures = @()
	foreach ($id in 1..11) {
		$response = Get-McpResponse -Responses $session.Responses -Id $id
		if ($id -ge 3 -and $response.result.isError -eq $true) { $failures += "request $id failed: $($response.result.content[0].text)" }
	}
	if ($failures.Count -ne 0) {
		$statusText = (Get-McpResponse -Responses $session.Responses -Id 10).result.content[0].text
		throw "$([string]::Join('; ', $failures)) Status: $statusText"
	}
	$tools = (Get-McpResponse -Responses $session.Responses -Id 2).result.tools.name
	foreach ($tool in @('dm_tracy_prepare', 'dm_tracy_launch', 'dm_tracy_capture', 'dm_tracy_hotspots', 'dm_tracy_zone', 'dm_tracy_frame_stats', 'dm_tracy_compare', 'dm_tracy_status', 'dm_tracy_stop')) {
		if ($tools -notcontains $tool) { throw "Tracy tool was not advertised: $tool" }
	}
	if (-not (Test-Path -LiteralPath $trace -PathType Leaf) -or (Get-Item -LiteralPath $trace).Length -eq 0) { throw 'The live trace artifact is missing or empty.' }
	$hotspots = (Get-McpResponse -Responses $session.Responses -Id 6).result.content[0].text | ConvertFrom-Json
	if (@($hotspots.items | Where-Object name -like '*meridian_profile_work*').Count -eq 0) { throw "The trace did not contain the known fixture proc. Hotspots: $($hotspots | ConvertTo-Json -Compress -Depth 5)" }
	$zone = (Get-McpResponse -Responses $session.Responses -Id 7).result.content[0].text | ConvertFrom-Json
	if (@($zone.items).Count -lt 1) { throw 'The exact fixture proc query returned no recorded source identity.' }
	$frames = (Get-McpResponse -Responses $session.Responses -Id 8).result.content[0].text | ConvertFrom-Json
	if ($frames.frame_count -lt 1) { throw 'The trace did not contain ServerTick frame evidence.' }
	$comparison = (Get-McpResponse -Responses $session.Responses -Id 9).result.content[0].text | ConvertFrom-Json
	if (@($comparison.items | Where-Object { $_.inclusive_delta_ns -ne 0 -or $_.self_delta_ns -ne 0 -or $_.count_delta -ne 0 }).Count -ne 0) { throw 'A trace compared with itself produced non-zero deltas.' }
	$evidence = [ordered]@{ schema_version = 1; overall = 'passed'; byond = '516.1685'; tracy_protocol = 82; trace_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $trace).Hash.ToLowerInvariant(); trace_bytes = (Get-Item -LiteralPath $trace).Length; known_proc = $true; frame_count = $frames.frame_count }
	$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
	New-Item -ItemType Directory -Force -Path (Split-Path -Parent $evidenceFile) | Out-Null
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 5) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
} finally {
	foreach ($artifact in @('tracy.dmb', 'tracy.rsc', 'tracy.pdb', 'tracy.log', 'prof.dll', 'libprof.so', 'meridian-owned.tracy')) {
		Remove-Item -LiteralPath (Join-Path $fixtureRoot $artifact) -Force -ErrorAction SilentlyContinue
	}
}
