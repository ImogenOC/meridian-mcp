[CmdletBinding()]
param(
	[string]$RuntimeDirectory = $(if ($IsWindows) { Join-Path $env:WINDIR 'SysWOW64' } else { '' }),
	[string]$InstallerPath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$requiredRuntimeFiles = @('MSVCP140.dll', 'VCRUNTIME140.dll')

function Get-MissingRuntimeFiles([string]$Directory) {
	return @($requiredRuntimeFiles | Where-Object { -not (Test-Path -LiteralPath (Join-Path $Directory $_) -PathType Leaf) })
}

if ([string]::IsNullOrWhiteSpace($RuntimeDirectory)) {
	throw 'An x86 Windows runtime directory is required.'
}
$runtimeRoot = [IO.Path]::GetFullPath($RuntimeDirectory)
$missing = @(Get-MissingRuntimeFiles $runtimeRoot)
if ($missing.Count -eq 0) {
	Write-Host "Verified auxtools x86 MSVC runtime in $runtimeRoot"
	return
}
if (-not $IsWindows) {
	throw "Missing auxtools x86 MSVC runtime files: $([string]::Join(', ', $missing)). Automatic installation is Windows-only."
}

if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
	$redistRoot = 'C:\Program Files (x86)\Microsoft Visual Studio\2022'
	$installer = Get-ChildItem -LiteralPath $redistRoot -Filter 'vc_redist.x86.exe' -File -Recurse -ErrorAction SilentlyContinue |
		Sort-Object -Property FullName -Descending |
		Select-Object -First 1
	if ($null -ne $installer) {
		$InstallerPath = $installer.FullName
	}
}
if ([string]::IsNullOrWhiteSpace($InstallerPath) -or -not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
	throw "Missing auxtools x86 MSVC runtime files: $([string]::Join(', ', $missing)). Could not find vc_redist.x86.exe."
}

$process = Start-Process -FilePath ([IO.Path]::GetFullPath($InstallerPath)) -ArgumentList @('/install', '/quiet', '/norestart') -Wait -PassThru -WindowStyle Hidden
if ($process.ExitCode -notin @(0, 1638, 3010)) {
	throw "The x86 MSVC redistributable installer failed with exit code $($process.ExitCode)."
}
$missing = @(Get-MissingRuntimeFiles $runtimeRoot)
if ($missing.Count -ne 0) {
	throw "The x86 MSVC redistributable completed but these auxtools runtime files remain missing: $([string]::Join(', ', $missing))."
}
Write-Host "Installed and verified auxtools x86 MSVC runtime in $runtimeRoot"
