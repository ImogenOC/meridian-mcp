[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$HelperManifestPath,
	[Parameter(Mandatory)][string]$AuxtoolsRoot,
	[Parameter(Mandatory)][string]$DestinationRoot,
	[string]$InstalledName = 'meridian-mcp-spacemandmm-20260824.exe'
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$manifestPath = (Resolve-Path -LiteralPath $HelperManifestPath).Path
$auxRoot = (Resolve-Path -LiteralPath $AuxtoolsRoot).Path
$destination = [IO.Path]::GetFullPath($DestinationRoot)
New-Item -ItemType Directory -Force -Path $destination | Out-Null
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$platform = if ($IsWindows) { 'windows-x86_64' } elseif ($IsLinux) { 'linux-x86_64' } else { throw 'Unsupported installation platform.' }
$helper = @($manifest.helpers | Where-Object platform -eq $platform)
if ($helper.Count -ne 1) { throw "Expected exactly one $platform dmdoc helper." }
$helperSource = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $manifestPath) $helper[0].path))
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $helperSource).Hash.ToLowerInvariant() -ne $helper[0].sha256) { throw 'Source dmdoc helper hash mismatch.' }

function Install-File([string]$Source, [string]$Target) {
	$parent = Split-Path -Parent $Target
	New-Item -ItemType Directory -Force -Path $parent | Out-Null
	$temporary = Join-Path $parent ('.install-' + [Guid]::NewGuid().ToString('N') + '.tmp')
	try {
		Copy-Item -LiteralPath $Source -Destination $temporary
		$sourceHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $Source).Hash
		$temporaryHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $temporary).Hash
		if ($sourceHash -ne $temporaryHash) { throw "Staged hash mismatch for $Target" }
		Move-Item -LiteralPath $temporary -Destination $Target -Force
	} finally {
		Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
	}
}

$installedBinary = Join-Path $destination $InstalledName
Install-File $binary $installedBinary
$helperTarget = Join-Path $destination "helpers/bin/$platform/$([IO.Path]::GetFileName($helperSource))"
Install-File $helperSource $helperTarget
$auxSource = Join-Path $auxRoot 'helpers/auxtools/v2.3.7/debug_server.dll'
$auxTarget = Join-Path $destination 'helpers/auxtools/v2.3.7/debug_server.dll'
Install-File $auxSource $auxTarget
if ((Get-FileHash -Algorithm SHA256 -LiteralPath $auxTarget).Hash.ToLowerInvariant() -ne 'b188999ac58a0e0171b015c39a403ab7da2f37ddb8ac3817a078f5bce02a8be7') { throw 'Installed auxtools hash mismatch.' }

$installedManifestPath = Join-Path $destination 'helpers/manifest.json'
$installedManifest = [ordered]@{ schema_version = 1; helpers = @([ordered]@{ platform = $platform; path = "bin/$platform/$([IO.Path]::GetFileName($helperTarget))"; sha256 = $helper[0].sha256; source_revision = $helper[0].source_revision }) }
$manifestTemporary = $installedManifestPath + '.tmp-' + [Guid]::NewGuid().ToString('N')
try {
	[IO.File]::WriteAllText($manifestTemporary, (($installedManifest | ConvertTo-Json -Depth 5) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	Move-Item -LiteralPath $manifestTemporary -Destination $installedManifestPath -Force
} finally {
	Remove-Item -LiteralPath $manifestTemporary -Force -ErrorAction SilentlyContinue
}
[pscustomobject]@{ binary = $installedBinary; helper_manifest = $installedManifestPath; auxtools = $auxTarget; binary_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $installedBinary).Hash.ToLowerInvariant() } | ConvertTo-Json
