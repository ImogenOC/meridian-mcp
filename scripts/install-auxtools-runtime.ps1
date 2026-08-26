[CmdletBinding()]
param(
	[string]$RuntimeDirectory = $(if ($IsWindows) { Join-Path $env:WINDIR 'SysWOW64' } else { '' }),
	[string]$InstallerPath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

if ([string]::IsNullOrWhiteSpace($RuntimeDirectory)) {
	throw 'An x86 Windows runtime directory is required.'
}
$runtimeRoot = [IO.Path]::GetFullPath($RuntimeDirectory)
$runtimeParameters = @{
	ApplicationDirectory = $runtimeRoot
	System32Directory = $runtimeRoot
	SkipDirectX = $true
}
if (-not [string]::IsNullOrWhiteSpace($InstallerPath)) {
	$runtimeParameters.InstallerPath = $InstallerPath
}
& (Join-Path $PSScriptRoot 'install-byond-runtime.ps1') @runtimeParameters
