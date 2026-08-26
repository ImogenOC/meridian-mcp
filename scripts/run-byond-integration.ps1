[CmdletBinding()]
param(
    [string]$MeridianRiftRoot,
    [string]$DreamMakerPath = "C:\Program Files (x86)\BYOND\bin\dm.exe",
    [string]$BinaryPath,
    [string]$EvidencePath,
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
    $MeridianRiftRoot = (Resolve-Path -LiteralPath $MeridianRiftRoot).Path
    $dme = Join-Path $MeridianRiftRoot "tgstation.dme"
    if (-not (Test-Path -LiteralPath $dme -PathType Leaf)) { throw "Meridian-Rift DME not found: $dme" }
    if (-not $BinaryPath -or -not $EvidencePath) {
        throw '-MeridianRiftRoot requires -BinaryPath and -EvidencePath for the full compatibility gate.'
    }

    $dependencies = Get-Content -LiteralPath (Join-Path $MeridianRiftRoot 'dependencies.sh')
    $major = ($dependencies | Select-String '^export BYOND_MAJOR=([0-9]+)$').Matches.Groups[1].Value
    $minor = ($dependencies | Select-String '^export BYOND_MINOR=([0-9]+)$').Matches.Groups[1].Value
    if ("$major.$minor" -ne '516.1687') {
        throw "Unsupported Meridian-Rift BYOND pin: $major.$minor"
    }
    $mcpSha = (& git -C $repoRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the Meridian-MCP commit SHA.' }
    $riftSha = (& git -C $MeridianRiftRoot rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the Meridian-Rift commit SHA.' }
    Write-Host "Meridian-MCP SHA: $mcpSha"
    Write-Host "Meridian-Rift SHA: $riftSha"
    Write-Host "BYOND version: $major.$minor"

    & (Join-Path $repoRoot 'scripts\run-meridian-compatibility.ps1') `
        -BinaryPath $BinaryPath `
        -MeridianRiftRoot $MeridianRiftRoot `
        -DreamMakerPath $DreamMakerPath `
        -EvidencePath $EvidencePath `
        -MeridianMcpSha $mcpSha `
        -MeridianRiftSha $riftSha
    if ($LASTEXITCODE -ne 0) {
        throw "Meridian-Rift compatibility gate failed with exit code $LASTEXITCODE"
    }
}
