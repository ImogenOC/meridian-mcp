param(
	[Parameter(Mandatory)][string]$Child,
	[Parameter(Mandatory)][string]$Marker
)

$ErrorActionPreference = 'Stop'
& (Join-Path $PSHOME 'powershell.exe') -NoLogo -NoProfile -ExecutionPolicy Bypass -File $Child -Marker $Marker
exit $LASTEXITCODE
