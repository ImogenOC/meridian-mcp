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
$traces = @((Join-Path $fixtureRoot 'meridian-owned-immediate.tracy'))
$traces += @(1..3 | ForEach-Object { Join-Path $fixtureRoot "meridian-owned-$_.tracy" })
$duration_ms = 30000
if ($env:MERIDIAN_TRACY_TEST_DURATION_MS) { $duration_ms = [int]$env:MERIDIAN_TRACY_TEST_DURATION_MS }
$delay_seconds = 120
if ($env:MERIDIAN_TRACY_TEST_DELAY_SECONDS) { $delay_seconds = [int]$env:MERIDIAN_TRACY_TEST_DELAY_SECONDS }
$ownedArtifacts = @('tracy.dmb', 'tracy.rsc', 'tracy.pdb', 'tracy.log', 'prof.dll', 'libprof.so', '.meridian-tracy-session.json', 'experiment-launch.meridian.json', 'experiment-identity.meridian.json', 'experiment-complete.meridian.json', 'meridian-owned-immediate.tracy', 'meridian-owned-immediate.tracy.meridian.json') + @(1..3 | ForEach-Object { "meridian-owned-$_.tracy"; "meridian-owned-$_.tracy.meridian.json" })
foreach ($artifact in $ownedArtifacts) {
	Remove-Item -LiteralPath (Join-Path $fixtureRoot $artifact) -Force -ErrorAction SilentlyContinue
}

& $compiler (Join-Path $fixtureRoot 'tracy.dme')
if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $dmb -PathType Leaf)) { throw 'The Tracy fixture did not compile.' }

$requests = @(
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'meridian-tracy-integration'; version = '1.0' } } })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 2; method = 'tools/list'; params = [ordered]@{} })),
	(Request 3 'dm_tracy_prepare' @{ dmb_path = $dmb }),
	(Request 4 'dm_tracy_launch' @{ dmb_path = $dmb; game_port = 14569; experiment_directory = $fixtureRoot }),
	(Request 5 'dm_tracy_status' @{}),
	(Request 6 'dm_tracy_capture' @{ output_path = $traces[0]; duration_ms = $duration_ms; memory_limit_mb = 256; capture_network = $true; phase = 'immediate'; phase_iteration = 1 }),
	(Request 7 'dm_tracy_capture' @{ output_path = $traces[1]; duration_ms = $duration_ms; memory_limit_mb = 256; capture_network = $true; phase = 'steady_state'; phase_iteration = 1 }),
	(Request 8 'dm_tracy_capture' @{ output_path = $traces[2]; duration_ms = $duration_ms; memory_limit_mb = 256; capture_network = $true; phase = 'steady_state'; phase_iteration = 2 }),
	(Request 9 'dm_tracy_capture' @{ output_path = $traces[3]; duration_ms = $duration_ms; memory_limit_mb = 256; capture_network = $true; phase = 'steady_state'; phase_iteration = 3 }),
	(Request 10 'dm_tracy_hotspots' @{ trace_path = $traces[3]; limit = 100; sort = 'inclusive' }),
	(Request 11 'dm_tracy_zone' @{ trace_path = $traces[3]; name = '/proc/meridian_profile_work'; limit = 10 }),
	(Request 12 'dm_tracy_frame_stats' @{ trace_path = $traces[3] }),
	(Request 13 'dm_tracy_compare' @{ baseline_path = $traces[3]; current_path = $traces[3]; limit = 100; minimum_delta_ns = 0 }),
	(Request 14 'dm_tracy_status' @{}),
	(Request 15 'dm_tracy_stop' @{})
)
$environment = @{
	MERIDIAN_MCP_MODE = 'development'
	MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, @($mcpRoot, $fixtureRoot))
	MERIDIAN_MCP_COMPILERS = $compiler
	MERIDIAN_MCP_HELPER_MANIFEST = $manifest
	MERIDIAN_MCP_TRACY = 'byond'
	PATH = ([IO.Path]::GetDirectoryName($compiler) + [IO.Path]::PathSeparator + $env:PATH)
}

try {
	$afterResponse = {
		param($request, $response)
		if ($request.id -eq 6) {
			Write-Host "immediate-capture-complete marker: retaining the drain worker for $delay_seconds seconds before steady-state capture"
			Start-Sleep -Seconds $delay_seconds
		}
	}
	$session = Invoke-McpSession -BinaryPath $binary -WorkingDirectory $mcpRoot -Environment $environment -Requests $requests -TimeoutMilliseconds 480000 -AfterResponse $afterResponse
	if ($session.ExitCode -ne 0) { throw "Tracy MCP session exited with $($session.ExitCode)." }
	$failures = @()
	foreach ($id in 1..15) {
		$response = Get-McpResponse -Responses $session.Responses -Id $id
		if ($id -ge 3 -and $response.result.isError -eq $true) { $failures += "request $id failed: $($response.result.content[0].text)" }
	}
	if ($failures.Count -ne 0) {
		$statusText = (Get-McpResponse -Responses $session.Responses -Id 14).result.content[0].text
		throw "$([string]::Join('; ', $failures)) Status: $statusText"
	}
	$tools = (Get-McpResponse -Responses $session.Responses -Id 2).result.tools.name
	foreach ($tool in @('dm_tracy_prepare', 'dm_tracy_launch', 'dm_tracy_capture', 'dm_tracy_hotspots', 'dm_tracy_zone', 'dm_tracy_frame_stats', 'dm_tracy_compare', 'dm_tracy_status', 'dm_tracy_stop')) {
		if ($tools -notcontains $tool) { throw "Tracy tool was not advertised: $tool" }
	}
	$binarySha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $binary).Hash.ToLowerInvariant()
	for ($traceIndex = 0; $traceIndex -lt $traces.Count; $traceIndex++) {
		$trace = $traces[$traceIndex]
		$sidecar = "$trace.meridian.json"
		if (-not (Test-Path -LiteralPath $trace -PathType Leaf) -or (Get-Item -LiteralPath $trace).Length -eq 0) { throw "The live trace artifact is missing or empty: $trace" }
		if (-not (Test-Path -LiteralPath $sidecar -PathType Leaf)) { throw "The schema-2 sidecar is missing: $sidecar" }
		$metadata = Get-Content -LiteralPath $sidecar -Raw | ConvertFrom-Json
		if ($metadata.schema -ne 2 -or $metadata.trace_sha256 -ne (Get-FileHash -Algorithm SHA256 -LiteralPath $trace).Hash.ToLowerInvariant()) { throw "Trace sidecar validation failed: $sidecar" }
		if (-not $metadata.experiment_identity.experiment_id -or -not $metadata.launch_manifest_sha256 -or -not $metadata.experiment_manifest_sha256) { throw "Trace sidecar omitted immutable experiment identity: $sidecar" }
		$expectedPhase = if ($traceIndex -eq 0) { 'immediate' } else { 'steady_state' }
		$expectedIteration = if ($traceIndex -eq 0) { 1 } else { $traceIndex }
		if ($metadata.phase -ne $expectedPhase -or $metadata.phase_iteration -ne $expectedIteration) { throw "Trace sidecar phase identity is invalid: $sidecar" }
		if (-not $metadata.capture.validation.valid -or $metadata.capture.validation.raw_end -le $metadata.capture.validation.raw_begin -or $metadata.capture.validation.trace_end_ns -le $metadata.capture.validation.trace_begin_ns -or $metadata.capture.validation.complete_frames -lt 3 -or $metadata.capture.validation.zones -lt 1) { throw "Trace failed mandatory validity checks: $sidecar" }
		if (-not $metadata.meridian_mcp_build.complete -or $metadata.meridian_mcp_build.executable_sha256 -ne $binarySha256 -or -not $metadata.meridian_mcp_build.build_id) { throw "Trace omitted the exact Meridian-MCP build identity: $sidecar" }
		if ($metadata.capture.validation.queue.saturation_count -ne 0 -or $metadata.capture.validation.queue.dropped_events -ne 0) { throw "Trace recorded queue saturation or drops: $sidecar" }
	}
	$initialStatus = (Get-McpResponse -Responses $session.Responses -Id 5).result.content[0].text | ConvertFrom-Json
	if ($initialStatus.collector_status.queue_health.capacity -lt 1 -or -not $initialStatus.collector_status.queue_health.hook_installed -or -not $initialStatus.collector_status.queue_health.prologue_validated) { throw 'Initial Tracy status omitted ready hook and queue health.' }
	$finalStatus = (Get-McpResponse -Responses $session.Responses -Id 14).result.content[0].text | ConvertFrom-Json
	if ($finalStatus.collector_status.worker_purpose -ne 'drain' -or -not $finalStatus.collector_status.worker_attached -or $finalStatus.collector_status.queue_health.capacity -lt 1 -or -not $finalStatus.collector_status.queue_health.hook_installed -or -not $finalStatus.collector_status.queue_health.prologue_validated) { throw 'Restored Tracy status omitted ready drain-worker queue health.' }
	$hotspots = (Get-McpResponse -Responses $session.Responses -Id 10).result.content[0].text | ConvertFrom-Json
	if (@($hotspots.items | Where-Object name -like '*meridian_profile_work*').Count -eq 0) { throw "The trace did not contain the known fixture proc. Hotspots: $($hotspots | ConvertTo-Json -Compress -Depth 5)" }
	$zone = (Get-McpResponse -Responses $session.Responses -Id 11).result.content[0].text | ConvertFrom-Json
	if (@($zone.items).Count -lt 1) { throw 'The exact fixture proc query returned no recorded source identity.' }
	$frames = (Get-McpResponse -Responses $session.Responses -Id 12).result.content[0].text | ConvertFrom-Json
	if ($frames.frame_count -lt 1) { throw 'The trace did not contain ServerTick frame evidence.' }
	$comparison = (Get-McpResponse -Responses $session.Responses -Id 13).result.content[0].text | ConvertFrom-Json
	if (@($comparison.items | Where-Object { $_.inclusive_delta_ns -ne 0 -or $_.self_delta_ns -ne 0 -or $_.count_delta -ne 0 }).Count -ne 0) { throw 'A trace compared with itself produced non-zero deltas.' }
	$journal = Get-Content -LiteralPath (Join-Path $fixtureRoot '.meridian-tracy-session.json') -Raw | ConvertFrom-Json
	if ($journal.status -ne 'finalized' -or $journal.last_action -ne 'finalized') { throw 'Tracy integrity journal was not finalized after stop.' }
	$evidence = [ordered]@{ schema_version = 3; overall = 'passed'; byond = '516.1687'; tracy_protocol = 82; delayed_first_capture_seconds = $delay_seconds; capture_duration_ms = $duration_ms; capture_count = 4; immediate_capture = $true; meridian_mcp_build_id = $metadata.meridian_mcp_build.build_id; captures = @($traces | ForEach-Object { [ordered]@{ trace_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $_).Hash.ToLowerInvariant(); trace_bytes = (Get-Item -LiteralPath $_).Length } }); known_proc = $true; frame_count = $frames.frame_count; repository_integrity = 'verified_by_mcp'; integrity_journal = 'finalized' }
	$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
	New-Item -ItemType Directory -Force -Path (Split-Path -Parent $evidenceFile) | Out-Null
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 5) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
} finally {
	foreach ($artifact in $ownedArtifacts) {
		Remove-Item -LiteralPath (Join-Path $fixtureRoot $artifact) -Force -ErrorAction SilentlyContinue
	}
}
