[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$EvidencePath,
	[string]$PrerequisiteEvidencePath,
	[ValidateRange(1, 100000)][int]$PrototypeCount = 65537,
	[ValidateSet('control', 'boundary')][string]$RuntimeCase = 'boundary',
	[string]$ControlEvidencePath,
	[ValidatePattern('^[0-9]+\.[0-9]+$')][string]$ExpectedByondVersion = '516.1687',
	[ValidateRange(30, 900)][int]$CompileTimeoutSeconds = 300,
	[ValidateRange(10, 900)][int]$RuntimeTimeoutSeconds = 300
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
Import-Module -Force (Join-Path $PSScriptRoot 'process-readiness.psm1')
$compiler = (Resolve-Path -LiteralPath $DreamMakerPath).Path
$compilerDirectory = Split-Path -Parent $compiler
$daemonName = if ($IsWindows) { 'DreamDaemon.exe' } else { 'DreamDaemon' }
$daemon = Join-Path $compilerDirectory $daemonName
if (-not (Test-Path -LiteralPath $daemon -PathType Leaf)) { throw 'DreamDaemon was not found beside DreamMaker.' }

$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
$evidenceRoot = Split-Path -Parent $evidenceFile
New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null
$logRoot = Join-Path $evidenceRoot 'large-prototype-logs'
New-Item -ItemType Directory -Force -Path $logRoot | Out-Null

$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('meridian-large-prototypes-' + [Guid]::NewGuid().ToString('N'))
$dme = Join-Path $fixtureRoot 'large_prototypes.dme'
$dmb = Join-Path $fixtureRoot 'large_prototypes.dmb'
$source = Join-Path $fixtureRoot 'large_prototypes.dm'
$marker = Join-Path $fixtureRoot 'startup.marker'
$compileStdout = Join-Path $fixtureRoot 'dreammaker.stdout.log'
$compileStderr = Join-Path $fixtureRoot 'dreammaker.stderr.log'
$daemonStdout = Join-Path $fixtureRoot 'dreamdaemon.stdout.log'
$daemonStderr = Join-Path $fixtureRoot 'dreamdaemon.stderr.log'
$daemonWorldLog = Join-Path $fixtureRoot 'large_prototypes.log'
$existingDaemonIds = @()
$ownedDaemonIds = [Collections.Generic.HashSet[int]]::new()
$compile = $null
$runtime = $null
$fixtureMetadata = $null
$compileSucceeded = $false
$markerReady = $false
$readiness = $null
$controlPassed = $false
$failure = $null
$startedAtUtc = [DateTime]::UtcNow.ToString('O')
$reportedFileVersion = (Get-Item -LiteralPath $compiler).VersionInfo.FileVersion
$byondIdentity = Get-ByondVersionIdentity -ExpectedVersion $ExpectedByondVersion -ReportedFileVersion $reportedFileVersion
$byondVersion = $byondIdentity.version
$retainedFixtureId = "byond-$byondVersion-$RuntimeCase-$PrototypeCount"
$daemonArguments = New-PrototypeDreamDaemonArguments -DmbPath $dmb

function Convert-ExitCodeHex([int]$ExitCode) {
	$unsigned = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32]$ExitCode), 0)
	return ('0x{0:X8}' -f $unsigned)
}

function Get-BoundedRedactedLog([string]$Path) {
	if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return '' }
	$text = Get-Content -Raw -LiteralPath $Path -ErrorAction SilentlyContinue
	if ($null -eq $text) { return '' }
	if ($text.Length -gt 65536) { $text = $text.Substring($text.Length - 65536) }
	return $text.Replace($fixtureRoot, '<fixture>').Replace($compilerDirectory, '<byond-bin>')
}

function Write-OwnedLog([string]$Source, [string]$Name) {
	$destination = Join-Path $logRoot $Name
	[IO.File]::WriteAllText($destination, (Get-BoundedRedactedLog $Source), [Text.UTF8Encoding]::new($false))
	return "large-prototype-logs/$Name"
}

function Get-FileIdentity([string]$Path) {
	if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) { return $null }
	$item = Get-Item -LiteralPath $Path
	return [ordered]@{
		bytes = $item.Length
		sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLowerInvariant()
	}
}

function Get-ProcessCreationIdentity([Diagnostics.Process]$Process) {
	try {
		$Process.Refresh()
		$identity = "dreamdaemon|$($Process.Id)|$($Process.StartTime.ToUniversalTime().Ticks)"
		$bytes = [Text.Encoding]::UTF8.GetBytes($identity)
		$hash = [Security.Cryptography.SHA256]::HashData($bytes)
		return [Convert]::ToHexString($hash).ToLowerInvariant()
	} catch {
		return $null
	}
}

function Get-RedactedApplicationEvents([DateTime]$SinceUtc) {
	if (-not $IsWindows) { return @() }
	try {
		return @(Get-WinEvent -FilterHashtable @{ LogName = 'Application'; StartTime = $SinceUtc.ToLocalTime() } -ErrorAction Stop |
			Where-Object { $_.ProviderName -match 'BYOND|DreamDaemon|Application Error' -or $_.Message -match 'BYOND|DreamDaemon' } |
			Select-Object -First 20 |
			ForEach-Object {
				$message = if ($null -eq $_.Message) { '' } else { [string]$_.Message }
				if ($message.Length -gt 4096) { $message = $message.Substring($message.Length - 4096) }
				[ordered]@{
					provider = $_.ProviderName
					event_id = $_.Id
					level = $_.LevelDisplayName
					message_tail = $message.Replace($fixtureRoot, '<fixture>').Replace($compilerDirectory, '<byond-bin>')
				}
			})
	} catch {
		return @([ordered]@{ unavailable = $true; reason = $_.Exception.Message })
	}
}

$prerequisiteEvidence = [ordered]@{ status = 'unavailable' }
if (-not [string]::IsNullOrWhiteSpace($PrerequisiteEvidencePath)) {
	$resolvedPrerequisiteEvidence = [IO.Path]::GetFullPath($PrerequisiteEvidencePath)
	if (Test-Path -LiteralPath $resolvedPrerequisiteEvidence -PathType Leaf) {
		$prerequisiteEvidence = Get-Content -Raw -LiteralPath $resolvedPrerequisiteEvidence | ConvertFrom-Json -AsHashtable
	}
}

if ($RuntimeCase -eq 'boundary' -and -not [string]::IsNullOrWhiteSpace($ControlEvidencePath)) {
	$resolvedControlEvidence = [IO.Path]::GetFullPath($ControlEvidencePath)
	if (Test-Path -LiteralPath $resolvedControlEvidence -PathType Leaf) {
		$controlEvidence = Get-Content -Raw -LiteralPath $resolvedControlEvidence | ConvertFrom-Json
		$controlPassed = $controlEvidence.overall -eq 'passed'
	}
}

$evidence = [ordered]@{
	schema_version = 3
	overall = 'failed'
	classification = 'environment_failure'
	byond = $byondVersion
	byond_version_verification = $byondIdentity.verification
	runtime_case = $RuntimeCase
	fixture = $null
	game_port = 0
	platform = [ordered]@{
		os = [Runtime.InteropServices.RuntimeInformation]::OSDescription
		architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
		runner_image = $env:ImageOS
	}
	started_at_utc = $startedAtUtc
	finished_at_utc = $null
	retained_fixture_id = $retainedFixtureId
	prerequisites = $prerequisiteEvidence
	dreammaker = [ordered]@{
		exit_code_signed = $null
		exit_code_hex = $null
		timed_out = $false
		dmb_created = $false
		elapsed_milliseconds = $null
		source = $null
		dmb = $null
		stdout_log = 'large-prototype-logs/dreammaker.stdout.log'
		stderr_log = 'large-prototype-logs/dreammaker.stderr.log'
	}
	dreamdaemon = [ordered]@{
		launcher_exit_code_signed = $null
		launcher_exit_code_hex = $null
		timed_out = $false
		elapsed_milliseconds = $null
		arguments = @('<dmb>') + @($daemonArguments | Select-Object -Skip 1)
		process_samples = @()
		last_progress_milliseconds = $null
		process_exit_code = $null
		stdout_log = 'large-prototype-logs/dreamdaemon.stdout.log'
		stderr_log = 'large-prototype-logs/dreamdaemon.stderr.log'
		world_log = 'large-prototype-logs/dreamdaemon.world.log'
	}
	launcher_exit_code_signed = $null
	launcher_exit_code_hex = $null
	marker_state = 'not_started'
	marker_observed_milliseconds = $null
	owned_processes = @()
	dmb_sha256 = $null
	application_events = @()
	failure = $null
}

try {
	$fixtureMetadata = & (Join-Path $PSScriptRoot 'new-large-prototype-fixture.ps1') -OutputDirectory $fixtureRoot -PrototypeCount $PrototypeCount -Layout bucketed | ConvertFrom-Json -AsHashtable
	$evidence.fixture = ConvertTo-PublicPrototypeFixtureEvidence $fixtureMetadata
	$evidence.dreammaker.source = Get-FileIdentity $source
	$compileParameters = @{
		FilePath = $compiler
		ArgumentList = @($dme)
		WorkingDirectory = $fixtureRoot
		PassThru = $true
		RedirectStandardOutput = $compileStdout
		RedirectStandardError = $compileStderr
	}
	if ($IsWindows) { $compileParameters.WindowStyle = 'Hidden' }
	$compileStopwatch = [Diagnostics.Stopwatch]::StartNew()
	$compile = Start-Process @compileParameters
	if (-not $compile.WaitForExit($CompileTimeoutSeconds * 1000)) {
		$compileStopwatch.Stop()
		$evidence.dreammaker.timed_out = $true
		$evidence.dreammaker.elapsed_milliseconds = [int64]$compileStopwatch.Elapsed.TotalMilliseconds
		Stop-Process -Id $compile.Id -Force -ErrorAction SilentlyContinue
		throw "The over-64K prototype fixture compile exceeded $CompileTimeoutSeconds seconds."
	}
	$compileStopwatch.Stop()
	$evidence.dreammaker.elapsed_milliseconds = [int64]$compileStopwatch.Elapsed.TotalMilliseconds
	$evidence.dreammaker.exit_code_signed = [int32]$compile.ExitCode
	$evidence.dreammaker.exit_code_hex = Convert-ExitCodeHex $compile.ExitCode
	$evidence.dreammaker.dmb_created = Test-Path -LiteralPath $dmb -PathType Leaf
	if ($compile.ExitCode -ne 0 -or -not $evidence.dreammaker.dmb_created) {
		throw "The over-64K prototype fixture did not compile successfully; exit code $($compile.ExitCode)."
	}
	$compileSucceeded = $true
	$evidence.dreammaker.dmb = Get-FileIdentity $dmb
	$evidence.dmb_sha256 = $evidence.dreammaker.dmb.sha256

	$existingDaemonIds = @(Get-Process -Name 'DreamDaemon' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
	$runtimeParameters = @{
		FilePath = $daemon
		ArgumentList = $daemonArguments
		WorkingDirectory = $fixtureRoot
		PassThru = $true
		RedirectStandardOutput = $daemonStdout
		RedirectStandardError = $daemonStderr
	}
	if ($IsWindows) { $runtimeParameters.WindowStyle = 'Hidden' }
	$runtime = Start-Process @runtimeParameters
	if ($existingDaemonIds -notcontains $runtime.Id) { [void]$ownedDaemonIds.Add($runtime.Id) }
	$readiness = Wait-ProcessReadiness -Process $runtime -MarkerPath $marker -ExpectedMarker 'MERIDIAN_LARGE_PROTOTYPE_READY' -TimeoutSeconds $RuntimeTimeoutSeconds
	$evidence.dreamdaemon.elapsed_milliseconds = $readiness.elapsed_milliseconds
	$evidence.dreamdaemon.process_samples = @($readiness.samples)
	$evidence.dreamdaemon.last_progress_milliseconds = $readiness.last_progress_milliseconds
	$evidence.dreamdaemon.process_exit_code = $readiness.process_exit_code
	$runtime.Refresh()
	if ($runtime.HasExited) {
		$evidence.dreamdaemon.launcher_exit_code_signed = [int32]$runtime.ExitCode
		$evidence.dreamdaemon.launcher_exit_code_hex = Convert-ExitCodeHex $runtime.ExitCode
		$evidence.launcher_exit_code_signed = [int32]$runtime.ExitCode
		$evidence.launcher_exit_code_hex = Convert-ExitCodeHex $runtime.ExitCode
		if ($IsWindows -and $runtime.ExitCode -gt 0 -and $existingDaemonIds -notcontains $runtime.ExitCode) {
			[void]$ownedDaemonIds.Add($runtime.ExitCode)
		}
	}
	foreach ($process in @(Get-Process -Name 'DreamDaemon' -ErrorAction SilentlyContinue)) {
		if ($existingDaemonIds -notcontains $process.Id) { [void]$ownedDaemonIds.Add($process.Id) }
	}
	$markerReady = $readiness.status -eq 'ready'
	$evidence.marker_state = if ($markerReady) { 'ready' } else { 'missing' }
	if ($markerReady) { $evidence.marker_observed_milliseconds = $readiness.elapsed_milliseconds }
	$hasProcessProgress = $readiness.last_progress_milliseconds -gt 0
	$evidence.classification = Get-PrototypeRuntimeClassification -RuntimeCase $RuntimeCase -CompileSucceeded $compileSucceeded -MarkerReady $markerReady -ReadinessStatus $readiness.status -HasProcessProgress $hasProcessProgress -ControlPassed $controlPassed
	if (-not $markerReady) {
		$evidence.dreamdaemon.timed_out = $readiness.status -eq 'timed_out'
		if ($readiness.status -eq 'process_exited') {
			throw 'DreamDaemon exited before emitting the over-64K readiness marker.'
		}
		throw "DreamDaemon did not emit the over-64K readiness marker within $RuntimeTimeoutSeconds seconds."
	}

	$evidence.overall = 'passed'
} catch {
	$failure = $_
	if ($evidence.classification -eq 'environment_failure' -and -not $compileSucceeded) {
		$evidence.classification = 'compile_failure'
	}
	$evidence.failure = [ordered]@{
		message = $_.Exception.Message.Replace($fixtureRoot, '<fixture>').Replace($compilerDirectory, '<byond-bin>')
		category = $_.CategoryInfo.Category.ToString()
	}
} finally {
	$ownedProcessEvidence = [Collections.Generic.List[object]]::new()
	foreach ($processId in $ownedDaemonIds) {
		$ownedProcess = Get-Process -Id $processId -ErrorAction SilentlyContinue
		$creationIdentity = if ($null -ne $ownedProcess) { Get-ProcessCreationIdentity $ownedProcess } else { $null }
		if ($null -ne $ownedProcess) { Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue }
		$terminated = $null -eq (Get-Process -Id $processId -ErrorAction SilentlyContinue)
		$ownedProcessEvidence.Add([ordered]@{
			role = 'dreamdaemon'
			pid = $processId
			creation_identity = $creationIdentity
			terminated = $terminated
		})
	}
	$evidence.owned_processes = @($ownedProcessEvidence | Sort-Object { $_.pid })
	$evidence.application_events = @(Get-RedactedApplicationEvents ([DateTime]::Parse($startedAtUtc).ToUniversalTime()))
	[void](Write-OwnedLog $compileStdout 'dreammaker.stdout.log')
	[void](Write-OwnedLog $compileStderr 'dreammaker.stderr.log')
	[void](Write-OwnedLog $daemonStdout 'dreamdaemon.stdout.log')
	[void](Write-OwnedLog $daemonStderr 'dreamdaemon.stderr.log')
	[void](Write-OwnedLog $daemonWorldLog 'dreamdaemon.world.log')
	$evidence.finished_at_utc = [DateTime]::UtcNow.ToString('O')
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 10) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	if ($null -eq $failure -and (Test-Path -LiteralPath $fixtureRoot)) {
		Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
	} elseif (Test-Path -LiteralPath $fixtureRoot) {
		Write-Warning 'Preserved the failed over-64K prototype fixture under the runner temporary directory.'
	}
}

if ($null -ne $failure) { throw $failure }
