[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$EvidencePath,
	[string]$PrerequisiteEvidencePath,
	[ValidateRange(65537, 100000)][int]$PrototypeCount = 65537,
	[ValidateRange(1024, 65535)][int]$GamePort = 14570,
	[ValidateRange(30, 900)][int]$CompileTimeoutSeconds = 300,
	[ValidateRange(10, 300)][int]$RuntimeTimeoutSeconds = 60
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
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
$compileStdout = Join-Path $fixtureRoot 'dreammaker.stdout.log'
$compileStderr = Join-Path $fixtureRoot 'dreammaker.stderr.log'
$daemonStdout = Join-Path $fixtureRoot 'dreamdaemon.stdout.log'
$daemonStderr = Join-Path $fixtureRoot 'dreamdaemon.stderr.log'
$existingDaemonIds = @()
$ownedDaemonIds = [Collections.Generic.HashSet[int]]::new()
$compile = $null
$runtime = $null
$markerReady = $false
$failure = $null
$startedAtUtc = [DateTime]::UtcNow.ToString('O')
$retainedFixtureId = "byond-516.1687-over64-$PrototypeCount"

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

$prerequisiteEvidence = [ordered]@{ status = 'unavailable' }
if (-not [string]::IsNullOrWhiteSpace($PrerequisiteEvidencePath)) {
	$resolvedPrerequisiteEvidence = [IO.Path]::GetFullPath($PrerequisiteEvidencePath)
	if (Test-Path -LiteralPath $resolvedPrerequisiteEvidence -PathType Leaf) {
		$prerequisiteEvidence = Get-Content -Raw -LiteralPath $resolvedPrerequisiteEvidence | ConvertFrom-Json -AsHashtable
	}
}

$evidence = [ordered]@{
	schema_version = 2
	overall = 'failed'
	byond = '516.1687'
	prototype_count = $PrototypeCount
	game_port = $GamePort
	started_at_utc = $startedAtUtc
	finished_at_utc = $null
	retained_fixture_id = $retainedFixtureId
	prerequisites = $prerequisiteEvidence
	dreammaker = [ordered]@{
		exit_code_signed = $null
		exit_code_hex = $null
		timed_out = $false
		dmb_created = $false
		stdout_log = 'large-prototype-logs/dreammaker.stdout.log'
		stderr_log = 'large-prototype-logs/dreammaker.stderr.log'
	}
	dreamdaemon = [ordered]@{
		launcher_exit_code_signed = $null
		launcher_exit_code_hex = $null
		timed_out = $false
		stdout_log = 'large-prototype-logs/dreamdaemon.stdout.log'
		stderr_log = 'large-prototype-logs/dreamdaemon.stderr.log'
	}
	launcher_exit_code_signed = $null
	launcher_exit_code_hex = $null
	marker_state = 'not_started'
	owned_processes = @()
	dmb_sha256 = $null
	failure = $null
}

try {
	& (Join-Path $PSScriptRoot 'new-large-prototype-fixture.ps1') -OutputDirectory $fixtureRoot -PrototypeCount $PrototypeCount | Out-Null
	$dme = Join-Path $fixtureRoot 'large_prototypes.dme'
	$dmb = Join-Path $fixtureRoot 'large_prototypes.dmb'
	$marker = Join-Path $fixtureRoot 'startup.marker'
	$compileParameters = @{
		FilePath = $compiler
		ArgumentList = @($dme)
		WorkingDirectory = $fixtureRoot
		PassThru = $true
		RedirectStandardOutput = $compileStdout
		RedirectStandardError = $compileStderr
	}
	if ($IsWindows) { $compileParameters.WindowStyle = 'Hidden' }
	$compile = Start-Process @compileParameters
	if (-not $compile.WaitForExit($CompileTimeoutSeconds * 1000)) {
		$evidence.dreammaker.timed_out = $true
		Stop-Process -Id $compile.Id -Force -ErrorAction SilentlyContinue
		throw "The over-64K prototype fixture compile exceeded $CompileTimeoutSeconds seconds."
	}
	$evidence.dreammaker.exit_code_signed = [int32]$compile.ExitCode
	$evidence.dreammaker.exit_code_hex = Convert-ExitCodeHex $compile.ExitCode
	$evidence.dreammaker.dmb_created = Test-Path -LiteralPath $dmb -PathType Leaf
	if ($compile.ExitCode -ne 0 -or -not $evidence.dreammaker.dmb_created) {
		throw "The over-64K prototype fixture did not compile successfully; exit code $($compile.ExitCode)."
	}

	$existingDaemonIds = @(Get-Process -Name 'DreamDaemon' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
	$runtimeParameters = @{
		FilePath = $daemon
		ArgumentList = @($dmb, $GamePort, '-trusted', '-logself', '-close', '-verbose')
		WorkingDirectory = $fixtureRoot
		PassThru = $true
		RedirectStandardOutput = $daemonStdout
		RedirectStandardError = $daemonStderr
	}
	if ($IsWindows) { $runtimeParameters.WindowStyle = 'Hidden' }
	$runtime = Start-Process @runtimeParameters
	if (-not $runtime.WaitForExit($RuntimeTimeoutSeconds * 1000)) {
		$evidence.dreamdaemon.timed_out = $true
		Stop-Process -Id $runtime.Id -Force -ErrorAction SilentlyContinue
		throw "The DreamDaemon launcher did not exit within $RuntimeTimeoutSeconds seconds."
	}
	$evidence.dreamdaemon.launcher_exit_code_signed = [int32]$runtime.ExitCode
	$evidence.dreamdaemon.launcher_exit_code_hex = Convert-ExitCodeHex $runtime.ExitCode
	$evidence.launcher_exit_code_signed = [int32]$runtime.ExitCode
	$evidence.launcher_exit_code_hex = Convert-ExitCodeHex $runtime.ExitCode
	if ($IsWindows -and $runtime.ExitCode -gt 0 -and $existingDaemonIds -notcontains $runtime.ExitCode) {
		[void]$ownedDaemonIds.Add($runtime.ExitCode)
	}

	$readiness = [Diagnostics.Stopwatch]::StartNew()
	do {
		foreach ($process in @(Get-Process -Name 'DreamDaemon' -ErrorAction SilentlyContinue)) {
			if ($existingDaemonIds -notcontains $process.Id) { [void]$ownedDaemonIds.Add($process.Id) }
		}
		if (Test-Path -LiteralPath $marker -PathType Leaf) {
			$markerReady = (Get-Content -Raw -LiteralPath $marker).TrimEnd() -eq 'MERIDIAN_LARGE_PROTOTYPE_READY'
			if ($markerReady) { break }
		}
		Start-Sleep -Milliseconds 100
	} while ($readiness.Elapsed.TotalSeconds -lt $RuntimeTimeoutSeconds)
	$readiness.Stop()
	$evidence.marker_state = if ($markerReady) { 'ready' } else { 'missing' }
	if (-not $markerReady) {
		throw "DreamDaemon did not emit the over-64K readiness marker within $RuntimeTimeoutSeconds seconds."
	}

	$evidence.dmb_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $dmb).Hash.ToLowerInvariant()
	$evidence.overall = 'passed'
} catch {
	$failure = $_
	$evidence.failure = [ordered]@{
		message = $_.Exception.Message.Replace($fixtureRoot, '<fixture>').Replace($compilerDirectory, '<byond-bin>')
		category = $_.CategoryInfo.Category.ToString()
	}
} finally {
	foreach ($processId in $ownedDaemonIds) {
		Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
	}
	$evidence.owned_processes = @($ownedDaemonIds | Sort-Object | ForEach-Object { [ordered]@{ role = 'dreamdaemon'; pid = $_; terminated = $true } })
	[void](Write-OwnedLog $compileStdout 'dreammaker.stdout.log')
	[void](Write-OwnedLog $compileStderr 'dreammaker.stderr.log')
	[void](Write-OwnedLog $daemonStdout 'dreamdaemon.stdout.log')
	[void](Write-OwnedLog $daemonStderr 'dreamdaemon.stderr.log')
	$evidence.finished_at_utc = [DateTime]::UtcNow.ToString('O')
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 10) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	if ($null -eq $failure -and (Test-Path -LiteralPath $fixtureRoot)) {
		Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
	} elseif (Test-Path -LiteralPath $fixtureRoot) {
		Write-Warning 'Preserved the failed over-64K prototype fixture under the runner temporary directory.'
	}
}

if ($null -ne $failure) { throw $failure }
