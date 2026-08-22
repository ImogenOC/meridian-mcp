[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$DmePath,
    [ValidateSet("debug", "release")]
    [string]$Configuration = "release",
    [string]$BinaryPath,
    [string]$TypePath,
    [string]$ProcName,
    [string]$SearchQuery,
    [ValidateRange(1, 3600)]
    [int]$TimeoutSeconds = 120,
    [switch]$SkipBuild
)

$scriptArguments = @{
    Configuration = $Configuration
    DmePath = $DmePath
    TimeoutSeconds = $TimeoutSeconds
    SkipBuild = $SkipBuild
}
if ($BinaryPath) {
    $scriptArguments.BinaryPath = $BinaryPath
}
if ($TypePath) {
    $scriptArguments.TypePath = $TypePath
}
if ($ProcName) {
    $scriptArguments.ProcName = $ProcName
}
if ($SearchQuery) {
    $scriptArguments.SearchQuery = $SearchQuery
}

& (Join-Path $PSScriptRoot "test_mcp.ps1") @scriptArguments
if ($?) {
    exit 0
}
exit 1
