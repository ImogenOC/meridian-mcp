[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$EvidencePath,
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
if (-not (Test-Path -LiteralPath $daemon -PathType Leaf)) { throw "DreamDaemon not found beside DreamMaker: $daemon" }
$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('meridian-large-prototypes-' + [Guid]::NewGuid().ToString('N'))
$stdout = Join-Path $fixtureRoot 'dreamdaemon.stdout.log'
$stderr = Join-Path $fixtureRoot 'dreamdaemon.stderr.log'
$existingDaemonIds = @()
$ownedDaemonIds = [Collections.Generic.HashSet[int]]::new()
$succeeded = $false

try {
	& (Join-Path $PSScriptRoot 'new-large-prototype-fixture.ps1') -OutputDirectory $fixtureRoot -PrototypeCount $PrototypeCount | Out-Host
	$dme = Join-Path $fixtureRoot 'large_prototypes.dme'
	$dmb = Join-Path $fixtureRoot 'large_prototypes.dmb'
	$marker = Join-Path $fixtureRoot 'startup.marker'
	$compile = Start-Process -FilePath $compiler -ArgumentList @($dme) -WorkingDirectory $fixtureRoot -PassThru -WindowStyle Hidden
	if (-not $compile.WaitForExit($CompileTimeoutSeconds * 1000)) {
		Stop-Process -Id $compile.Id -Force -ErrorAction SilentlyContinue
		throw "The over-64K prototype fixture compile exceeded $CompileTimeoutSeconds seconds."
	}
	if ($compile.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $dmb -PathType Leaf)) {
		throw "The over-64K prototype fixture did not compile successfully; exit code $($compile.ExitCode)."
	}

	$existingDaemonIds = @(Get-Process -Name 'DreamDaemon' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Id)
	$runtimeArguments = @($dmb, $GamePort, '-trusted', '-logself', '-close', '-verbose')
	$runtimeParameters = @{
		FilePath = $daemon
		ArgumentList = $runtimeArguments
		WorkingDirectory = $fixtureRoot
		PassThru = $true
		RedirectStandardOutput = $stdout
		RedirectStandardError = $stderr
	}
	if ($IsWindows) { $runtimeParameters.WindowStyle = 'Hidden' }
	$runtime = Start-Process @runtimeParameters
	if (-not $runtime.WaitForExit($RuntimeTimeoutSeconds * 1000)) {
		Stop-Process -Id $runtime.Id -Force -ErrorAction SilentlyContinue
		throw "The DreamDaemon launcher did not exit within $RuntimeTimeoutSeconds seconds."
	}
	if ($IsWindows -and $runtime.ExitCode -gt 0 -and $existingDaemonIds -notcontains $runtime.ExitCode) {
		[void]$ownedDaemonIds.Add($runtime.ExitCode)
	}
	$readiness = [Diagnostics.Stopwatch]::StartNew()
	do {
		foreach ($process in @(Get-Process -Name 'DreamDaemon' -ErrorAction SilentlyContinue)) {
			if ($existingDaemonIds -notcontains $process.Id) { [void]$ownedDaemonIds.Add($process.Id) }
		}
		if ((Test-Path -LiteralPath $marker -PathType Leaf) -and (Get-Content -Raw -LiteralPath $marker) -eq 'MERIDIAN_LARGE_PROTOTYPE_READY') {
			break
		}
		Start-Sleep -Milliseconds 100
	} while ($readiness.Elapsed.TotalSeconds -lt $RuntimeTimeoutSeconds)
	$readiness.Stop()
	$runtimeOutput = ((Get-Content -Raw -LiteralPath $stdout -ErrorAction SilentlyContinue) + (Get-Content -Raw -LiteralPath $stderr -ErrorAction SilentlyContinue))
	if (-not (Test-Path -LiteralPath $marker -PathType Leaf) -or (Get-Content -Raw -LiteralPath $marker) -ne 'MERIDIAN_LARGE_PROTOTYPE_READY') {
		throw "DreamDaemon did not emit the over-64K readiness marker within $RuntimeTimeoutSeconds seconds; launcher exit code $($runtime.ExitCode): $runtimeOutput"
	}

	$evidence = [ordered]@{
		schema_version = 1
		overall = 'passed'
		byond = '516.1687'
		prototype_count = $PrototypeCount
		game_port = $GamePort
		dmb_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $dmb).Hash.ToLowerInvariant()
		readiness_marker = 'MERIDIAN_LARGE_PROTOTYPE_READY'
		dreamdaemon_launcher_exit_code = $runtime.ExitCode
		owned_dreamdaemon_processes = $ownedDaemonIds.Count
	}
	$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
	New-Item -ItemType Directory -Force -Path (Split-Path -Parent $evidenceFile) | Out-Null
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 5) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	$succeeded = $true
} finally {
	foreach ($processId in $ownedDaemonIds) {
		Stop-Process -Id $processId -Force -ErrorAction SilentlyContinue
	}
	if ($succeeded -and (Test-Path -LiteralPath $fixtureRoot)) {
		Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
	} elseif (Test-Path -LiteralPath $fixtureRoot) {
		Write-Warning "Preserved failed over-64K prototype fixture at $fixtureRoot"
	}
}
