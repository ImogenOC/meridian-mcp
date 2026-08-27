[CmdletBinding()]
param(
	[Parameter(Mandatory)] [string] $ExperimentName,
	[Parameter(Mandatory)] [string] $Phase,
	[ValidateRange(3, 20)] [int] $ControlCount = 5,
	[ValidateRange(5, 300)] [int] $CaptureSeconds = 30,
	[ValidateRange(0, 600)] [int] $WarmupSeconds = 30,
	[string] $Map,
	[string] $Seed,
	[string] $ConfigurationProfile,
	[string[]] $FeatureSet = @(),
	[string] $Scenario,
	[string] $ExternalRunId,
	[hashtable] $Annotations = @{},
	[string[]] $ZoneKeys = @(),
	[Parameter(Mandatory)] [string] $DmbPath,
	[Parameter(Mandatory)] [string] $BinaryPath,
	[Parameter(Mandatory)] [string] $HelperManifestPath,
	[Parameter(Mandatory)] [string] $DreamMakerPath,
	[Parameter(Mandatory)] [string] $EvidenceDirectory
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$dmb = (Resolve-Path -LiteralPath $DmbPath).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$manifest = (Resolve-Path -LiteralPath $HelperManifestPath).Path
$dreamMaker = (Resolve-Path -LiteralPath $DreamMakerPath).Path
$evidence = [IO.Path]::GetFullPath($EvidenceDirectory)
if (Test-Path -LiteralPath $evidence) { throw 'EvidenceDirectory must be a new owned directory.' }
New-Item -ItemType Directory -Path $evidence | Out-Null
Import-Module (Join-Path $PSScriptRoot 'MeridianMcpSession.psm1') -Force
$systemTemporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$stateDirectory = Join-Path $systemTemporaryRoot ('.meridian-tracy-experiment-state-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stateDirectory | Out-Null
$stateDirectory = (Resolve-Path -LiteralPath $stateDirectory).Path
$relativeStateDirectory = [IO.Path]::GetRelativePath($systemTemporaryRoot, $stateDirectory)
if ($relativeStateDirectory -eq '..' -or $relativeStateDirectory.StartsWith('..' + [IO.Path]::DirectorySeparatorChar)) {
	throw 'Temporary state directory resolved outside the operating-system temporary directory.'
}

function Request([int]$Id, [string]$Name, [hashtable]$Arguments) {
	ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = $Id; method = 'tools/call'; params = [ordered]@{ name = $Name; arguments = $Arguments } })
}

$traces = 1..$ControlCount | ForEach-Object { Join-Path $evidence ("control-{0:D2}.tracy" -f $_) }
$launch = @{ dmb_path = $dmb; experiment_name = $ExperimentName; experiment_directory = $evidence; feature_set = $FeatureSet; annotations = $Annotations }
foreach ($pair in @(@('map',$Map), @('seed',$Seed), @('configuration_profile',$ConfigurationProfile), @('scenario',$Scenario), @('external_run_id',$ExternalRunId))) {
	if (-not [string]::IsNullOrWhiteSpace($pair[1])) { $launch[$pair[0]] = $pair[1] }
}
$requests = @(
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'meridian-tracy-experiment'; version = '1.0' } } })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })),
	(Request 2 'dm_tracy_prepare' @{ dmb_path = $dmb }),
	(Request 3 'dm_tracy_launch' $launch),
	(Request 4 'dm_tracy_status' @{})
)
$nextId = 5
foreach ($index in 1..$ControlCount) {
	$requests += Request $nextId 'dm_tracy_capture' @{ output_path = $traces[$index - 1]; duration_ms = $CaptureSeconds * 1000; memory_limit_mb = 512; phase = $Phase; phase_iteration = $index; capture_network = $true }
	$nextId++
}
$analysisIds = @()
foreach ($trace in $traces) {
	$analysisIds += $nextId
	$requests += Request $nextId 'dm_tracy_frame_stats' @{ trace_path = $trace }
	$nextId++
}
$controlId = $nextId
$requests += Request $nextId 'dm_tracy_control_stats' @{ trace_paths = $traces; frame_percentile = 'p95'; zone_keys = $ZoneKeys }
$nextId++
$stopId = $nextId
$requests += Request $stopId 'dm_tracy_stop' @{}

$environment = @{ MERIDIAN_MCP_MODE = 'development'; MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, @($root, $evidence, (Split-Path -Parent $dmb))); MERIDIAN_MCP_HELPER_MANIFEST = $manifest; MERIDIAN_MCP_TRACY = 'byond'; MERIDIAN_MCP_COMPILERS = $dreamMaker; MERIDIAN_MCP_STATE_DIR = $stateDirectory; PATH = ((Split-Path -Parent $dreamMaker) + [IO.Path]::PathSeparator + $env:PATH) }
$status = 'failed'
$failure = $null
$completedSteps = [Collections.Generic.List[string]]::new()
$cleanup = [ordered]@{ stop_requested = $false; stop_succeeded = $false }
try {
	$afterResponse = {
		param($request, $response)
		if ($request.id -eq 4 -and $WarmupSeconds -gt 0) {
			Write-Host "Tracy experiment warmup: excluding $WarmupSeconds seconds of boot activity from the first authoritative phase window."
			Start-Sleep -Seconds $WarmupSeconds
		}
	}
	$session = Invoke-McpSession -BinaryPath $binary -WorkingDirectory $root -Environment $environment -Requests $requests -TimeoutMilliseconds (($ControlCount * $CaptureSeconds + $WarmupSeconds + 300) * 1000) -AfterResponse $afterResponse
	if ($session.ExitCode -ne 0) { throw "MCP session exited with $($session.ExitCode)." }
	foreach ($id in @(2..$stopId)) {
		$response = Get-McpResponse -Responses $session.Responses -Id $id
		if ($response.result.isError -eq $true) { throw "Request $id failed: $($response.result.content[0].text)" }
	}
	$completedSteps.Add('prepare')
	$completedSteps.Add('launch')
	$completedSteps.Add('status')
	$completedSteps.Add('captures')
	$completedSteps.Add('analysis')
	$completedSteps.Add('control_stats')
	$cleanup.stop_requested = $true
	$cleanup.stop_succeeded = $true
	$completedSteps.Add('stop')
	$summaries = Join-Path $evidence 'summaries'
	New-Item -ItemType Directory -Path $summaries | Out-Null
	for ($offset = 0; $offset -lt $analysisIds.Count; $offset++) {
		$analysis = (Get-McpResponse -Responses $session.Responses -Id $analysisIds[$offset]).result.content[0].text | ConvertFrom-Json
		[IO.File]::WriteAllText((Join-Path $summaries ("control-{0:D2}-frames.json" -f ($offset + 1))), (($analysis | ConvertTo-Json -Depth 20) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	}
	$control = (Get-McpResponse -Responses $session.Responses -Id $controlId).result.content[0].text | ConvertFrom-Json
	[IO.File]::WriteAllText((Join-Path $evidence 'control-stats.json'), (($control | ConvertTo-Json -Depth 20) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	$stop = (Get-McpResponse -Responses $session.Responses -Id $stopId).result.content[0].text | ConvertFrom-Json
	if (-not $stop.experiment_manifest.path) { throw 'Stop response omitted the complete experiment manifest.' }
	$journal = Get-Content -LiteralPath (Join-Path $evidence '.meridian-tracy-session.json') -Raw | ConvertFrom-Json
	if ($journal.status -ne 'finalized' -or $journal.last_action -ne 'finalized') { throw 'The experiment integrity journal was not finalized.' }
	Copy-Item -LiteralPath $stop.experiment_manifest.path -Destination (Join-Path $evidence 'experiment.json')
	$validation = & (Join-Path $PSScriptRoot 'validate-tracy-evidence.ps1') -EvidenceDirectory $evidence
	[IO.File]::WriteAllText((Join-Path $evidence 'validation.json'), (($validation | Out-String).Trim() + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	$completedSteps.Add('independent_validation')
	$status = 'passed'
} catch {
	$failure = $_.Exception.Message
	throw
} finally {
	$buildIdentity = if (Test-Path -LiteralPath ($traces[0] + '.meridian.json')) { (Get-Content -LiteralPath ($traces[0] + '.meridian.json') -Raw | ConvertFrom-Json).meridian_mcp_build } else { $null }
	$index = [ordered]@{ schema = 3; status = $status; experiment_name = $ExperimentName; phase = $Phase; control_count = $ControlCount; capture_seconds = $CaptureSeconds; warmup_seconds = $WarmupSeconds; raw_traces_local_only = $true; meridian_mcp_build = $buildIdentity; completed_steps = @($completedSteps); cleanup = $cleanup; traces = @($traces | ForEach-Object { [ordered]@{ file = Split-Path -Leaf $_; sha256 = if (Test-Path -LiteralPath $_) { (Get-FileHash -Algorithm SHA256 -LiteralPath $_).Hash.ToLowerInvariant() } else { $null }; sidecar = (Split-Path -Leaf $_) + '.meridian.json' } }); failure = $failure }
	[IO.File]::WriteAllText((Join-Path $evidence 'evidence-index.json'), (($index | ConvertTo-Json -Depth 8) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	Remove-Item -LiteralPath $stateDirectory -Recurse -Force -ErrorAction SilentlyContinue
}
