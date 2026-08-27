[CmdletBinding()]
param([Parameter(Mandatory)] [string] $EvidenceDirectory)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$root = (Resolve-Path -LiteralPath $EvidenceDirectory).Path
$traces = @(Get-ChildItem -LiteralPath $root -Filter '*.tracy' -File)
if ($traces.Count -lt 3 -or $traces.Count -gt 20) { throw 'Evidence must contain 3-20 local control traces.' }
$experimentId = $null
$phase = $null
$buildId = $null
$validated = @()
$iterations = [Collections.Generic.HashSet[int]]::new()
foreach ($trace in $traces) {
	$sidecarPath = "$($trace.FullName).meridian.json"
	if (-not (Test-Path -LiteralPath $sidecarPath -PathType Leaf)) { throw "Missing sidecar: $sidecarPath" }
	$sidecar = Get-Content -LiteralPath $sidecarPath -Raw | ConvertFrom-Json
	$hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $trace.FullName).Hash.ToLowerInvariant()
	if ($sidecar.schema -ne 2 -or $sidecar.trace_sha256 -ne $hash) { throw "Trace identity failed: $($trace.Name)" }
	if (-not $sidecar.capture.validation.valid -or $sidecar.capture.validation.complete_frames -lt 3 -or $sidecar.capture.validation.trace_end_ns -le $sidecar.capture.validation.trace_begin_ns -or $sidecar.capture.validation.queue.saturation_count -ne 0 -or $sidecar.capture.validation.queue.dropped_events -ne 0) { throw "Capture validation failed: $($trace.Name)" }
	if (-not $sidecar.meridian_mcp_build.complete -or -not $sidecar.meridian_mcp_build.build_id -or $sidecar.meridian_mcp_build.executable_sha256.Length -ne 64) { throw "Meridian-MCP build identity is incomplete: $($trace.Name)" }
	if ($null -eq $experimentId) { $experimentId = $sidecar.experiment_identity.experiment_id; $phase = $sidecar.phase }
	if ($null -eq $buildId) { $buildId = $sidecar.meridian_mcp_build.build_id }
	if ($sidecar.experiment_identity.experiment_id -ne $experimentId -or $sidecar.phase -ne $phase) { throw "Control identity mismatch: $($trace.Name)" }
	if ($sidecar.meridian_mcp_build.build_id -ne $buildId) { throw "Meridian-MCP build identity mismatch: $($trace.Name)" }
	$roles = @($sidecar.memory_series | ForEach-Object { $_.identity.role } | Sort-Object -Unique)
	if ($roles.Count -ne 2 -or $roles -notcontains 'dream_daemon' -or $roles -notcontains 'collector') { throw "Role-specific memory series are incomplete: $($trace.Name)" }
	if (@($sidecar.memory_series | Where-Object { @($_.samples).Count -eq 0 }).Count -ne 0) { throw "Role-specific memory samples are empty: $($trace.Name)" }
	if ($sidecar.network_evidence.network_isolation_confirmed -ne $false -or $sidecar.network_evidence.capture_complete -ne $false) { throw "Network evidence overclaims completeness: $($trace.Name)" }
	if (-not $iterations.Add([int]$sidecar.phase_iteration)) { throw "Duplicate phase iteration: $($sidecar.phase_iteration)" }
	$validated += [ordered]@{ trace = $trace.Name; sha256 = $hash; phase_iteration = $sidecar.phase_iteration; complete_frames = $sidecar.capture.validation.complete_frames }
}
if (-not (Test-Path -LiteralPath (Join-Path $root 'experiment.json') -PathType Leaf)) { throw 'Complete experiment manifest is missing.' }
$journal = Get-Content -LiteralPath (Join-Path $root '.meridian-tracy-session.json') -Raw | ConvertFrom-Json
if ($journal.status -ne 'finalized' -or $journal.last_action -ne 'finalized') { throw 'Integrity journal is unfinished.' }
[pscustomobject]@{ schema = 2; status = 'passed'; experiment_id = $experimentId; meridian_mcp_build_id = $buildId; phase = $phase; controls = $validated } | ConvertTo-Json -Depth 5
