[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$TracyPath,
	[Parameter(Mandatory)][string]$ByondTracyPath,
	[string]$BuildRoot,
	[string]$CpmSourceCache,
	[string]$OutputDirectory,
	[string]$ManifestPath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$evidenceRoot = if ([string]::IsNullOrWhiteSpace($OutputDirectory)) { Join-Path ([IO.Path]::GetTempPath()) ("meridian-tracy-native-" + [Guid]::NewGuid().ToString('N')) } else { [IO.Path]::GetFullPath($OutputDirectory) }
$manifest = if ([string]::IsNullOrWhiteSpace($ManifestPath)) { Join-Path $evidenceRoot 'helpers/manifest.json' } else { [IO.Path]::GetFullPath($ManifestPath) }
$arguments = @{
	TracyPath = $TracyPath
	ByondTracyPath = $ByondTracyPath
	OutputDirectory = $evidenceRoot
	ManifestPath = $manifest
}
if (-not [string]::IsNullOrWhiteSpace($BuildRoot)) { $arguments.BuildRoot = $BuildRoot }
if (-not [string]::IsNullOrWhiteSpace($CpmSourceCache)) { $arguments.CpmSourceCache = $CpmSourceCache }

& (Join-Path $PSScriptRoot 'build-tracy-helpers.ps1') @arguments
if ($LASTEXITCODE -ne 0) { throw 'Portable Tracy native verification failed.' }
$document = Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
foreach ($id in @('tracy-server-helper', 'byond-tracy')) {
	if (@($document.helpers | Where-Object id -eq $id).Count -ne 1) { throw "Native verification omitted $id." }
}
[pscustomobject]@{ schema = 1; status = 'passed'; platform = if ($IsWindows) { 'windows' } else { 'ubuntu' }; manifest = $manifest } | ConvertTo-Json
