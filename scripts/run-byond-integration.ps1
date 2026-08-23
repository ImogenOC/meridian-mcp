[CmdletBinding()]
param(
    [string]$MeridianRiftRoot,
    [string]$DreamMakerPath = "C:\Program Files (x86)\BYOND\bin\dm.exe",
    [ValidateRange(10, 600)]
    [int]$TimeoutSeconds = 120
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not (Test-Path -LiteralPath $DreamMakerPath -PathType Leaf)) {
    throw "DreamMaker not found: $DreamMakerPath"
}

Push-Location (Join-Path $repoRoot "tests\fixtures\runtime")
try {
    $process = Start-Process -FilePath $DreamMakerPath -ArgumentList @("runtime.dme") -PassThru -WindowStyle Hidden
    if (-not $process.WaitForExit($TimeoutSeconds * 1000)) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
        throw "DreamMaker fixture compile exceeded $TimeoutSeconds seconds"
    }
    if ($process.ExitCode -ne 0) { throw "DreamMaker fixture compile failed with exit code $($process.ExitCode)" }
    if (-not (Test-Path -LiteralPath "runtime.dmb" -PathType Leaf)) { throw "DreamMaker exited without producing runtime.dmb" }
} finally {
    Pop-Location
}

if ($MeridianRiftRoot) {
    $dme = Join-Path $MeridianRiftRoot "tgstation.dme"
    if (-not (Test-Path -LiteralPath $dme -PathType Leaf)) { throw "Meridian-Rift DME not found: $dme" }
    Write-Host "Meridian-Rift full-corpus input available: $dme"
}
