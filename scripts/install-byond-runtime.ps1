[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$ApplicationDirectory,
	[string]$System32Directory = $(if ($IsWindows) { Join-Path $env:WINDIR 'SysWOW64' } else { '' }),
	[string]$DownloadDirectory = $(Join-Path ([IO.Path]::GetTempPath()) 'meridian-byond-runtime'),
	[string]$InstallerPath,
	[string]$EvidencePath,
	[switch]$CheckOnly,
	[switch]$SkipDirectX
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$dxsdkPackage = 'Microsoft.DXSDK.D3DX'
$dxsdkVersion = '9.29.952.8'
$dxsdkSha256 = 'ead0906ae8a26c18a7525da7490127a2110f7c58f18293738283e30e97c6ea4b'
$dxsdkUri = 'https://api.nuget.org/v3-flatcontainer/microsoft.dxsdk.d3dx/9.29.952.8/microsoft.dxsdk.d3dx.9.29.952.8.nupkg'
$dxsdkDllEntry = 'build/native/release/bin/x86/D3DX9_43.dll'
$requiredVcRuntimeFiles = @('MSVCP140.dll', 'VCRUNTIME140.dll', 'mfc140u.dll')
$requiredApplicationFiles = if ($SkipDirectX) { @() } else { @('D3DX9_43.dll') }

function Get-PeArchitecture([string]$Path) {
	try {
		$stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
		try {
			$reader = [IO.BinaryReader]::new($stream)
			if ($stream.Length -lt 64 -or $reader.ReadUInt16() -ne 0x5a4d) { return 'invalid' }
			$stream.Position = 0x3c
			$peOffset = $reader.ReadUInt32()
			if ($peOffset -gt ($stream.Length - 6)) { return 'invalid' }
			$stream.Position = $peOffset
			if ($reader.ReadUInt32() -ne 0x00004550) { return 'invalid' }
			switch ($reader.ReadUInt16()) {
				0x014c { return 'x86' }
				0x8664 { return 'x64' }
				0xaa64 { return 'arm64' }
				default { return 'unknown' }
			}
		} finally {
			$stream.Dispose()
		}
	} catch {
		return 'unreadable'
	}
}

function Get-RuntimeCheck([string]$Directory, [string]$Name, [string]$Scope) {
	$path = Join-Path $Directory $Name
	$present = Test-Path -LiteralPath $path -PathType Leaf
	$architecture = if ($present) { Get-PeArchitecture $path } else { $null }
	return [ordered]@{
		name = $Name
		scope = $Scope
		present = $present
		architecture = $architecture
		sha256 = if ($present) { (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant() } else { $null }
	}
}

function Get-RuntimeChecks([string]$RuntimeRoot, [string]$ApplicationRoot) {
	$checks = [Collections.Generic.List[object]]::new()
	foreach ($name in $requiredVcRuntimeFiles) {
		$checks.Add((Get-RuntimeCheck $RuntimeRoot $name 'x86_system_runtime'))
	}
	foreach ($name in $requiredApplicationFiles) {
		$checks.Add((Get-RuntimeCheck $ApplicationRoot $name 'byond_application'))
	}
	return @($checks)
}

function Get-MissingNames([object[]]$Checks) {
	return @($Checks | Where-Object { -not $_.present -or $_.architecture -ne 'x86' } | ForEach-Object { $_.name })
}

function Write-Result([hashtable]$Result) {
	$json = ($Result | ConvertTo-Json -Depth 8)
	if (-not [string]::IsNullOrWhiteSpace($EvidencePath)) {
		$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
		$evidenceParent = Split-Path -Parent $evidenceFile
		if (-not [string]::IsNullOrWhiteSpace($evidenceParent)) {
			New-Item -ItemType Directory -Force -Path $evidenceParent | Out-Null
		}
		[IO.File]::WriteAllText($evidenceFile, ($json + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	}
	Write-Output $json
}

if ([string]::IsNullOrWhiteSpace($System32Directory)) {
	throw 'An x86 Windows system runtime directory is required.'
}
$runtimeRoot = [IO.Path]::GetFullPath($System32Directory)
$applicationRoot = [IO.Path]::GetFullPath($ApplicationDirectory)
if (-not (Test-Path -LiteralPath $runtimeRoot -PathType Container)) {
	throw "The x86 Windows system runtime directory does not exist: $runtimeRoot"
}
if (-not (Test-Path -LiteralPath $applicationRoot -PathType Container)) {
	if ($CheckOnly) { throw "The BYOND application directory does not exist: $applicationRoot" }
	New-Item -ItemType Directory -Force -Path $applicationRoot | Out-Null
}

$checks = @(Get-RuntimeChecks $runtimeRoot $applicationRoot)
$missing = @(Get-MissingNames $checks)
$result = [ordered]@{
	schema = 1
	status = if ($missing.Count -eq 0) { 'present' } else { 'missing' }
	runner_image = [ordered]@{
		name = $env:ImageOS
		version = $env:ImageVersion
	}
	checked = $checks
	missing = $missing
	installed = @()
	redistributable = $null
	package = [ordered]@{
		id = $dxsdkPackage
		version = $dxsdkVersion
		sha256 = $dxsdkSha256
	}
	licenses = @()
}

if ($CheckOnly) {
	Write-Result $result
	if ($missing.Count -ne 0) {
		throw "Missing or non-x86 BYOND runtime prerequisites: $([string]::Join(', ', $missing))."
	}
	return
}

$missingVc = @($checks | Where-Object { $_.scope -eq 'x86_system_runtime' -and (-not $_.present -or $_.architecture -ne 'x86') })
if ($missingVc.Count -ne 0) {
	if (-not $IsWindows) {
		$result.status = 'failed'
		Write-Result $result
		throw "Missing x86 Windows runtime files: $([string]::Join(', ', @($missingVc.name))). Automatic installation is Windows-only."
	}
	if ([string]::IsNullOrWhiteSpace($InstallerPath)) {
		$redistRoot = 'C:\Program Files (x86)\Microsoft Visual Studio\2022'
		$installer = Get-ChildItem -LiteralPath $redistRoot -Filter 'vc_redist.x86.exe' -File -Recurse -ErrorAction SilentlyContinue |
			Sort-Object -Property FullName -Descending |
			Select-Object -First 1
		if ($null -ne $installer) { $InstallerPath = $installer.FullName }
	}
	if ([string]::IsNullOrWhiteSpace($InstallerPath) -or -not (Test-Path -LiteralPath $InstallerPath -PathType Leaf)) {
		$result.status = 'failed'
		Write-Result $result
		throw "Missing x86 Windows runtime files: $([string]::Join(', ', @($missingVc.name))). Could not find vc_redist.x86.exe."
	}
	$resolvedInstaller = [IO.Path]::GetFullPath($InstallerPath)
	$signature = Get-AuthenticodeSignature -LiteralPath $resolvedInstaller
	if ($signature.Status -ne [Management.Automation.SignatureStatus]::Valid -or $signature.SignerCertificate.Subject -notmatch 'Microsoft') {
		$result.status = 'failed'
		Write-Result $result
		throw 'The x86 VC redistributable does not have a valid Microsoft Authenticode signature.'
	}
	$result.redistributable = [ordered]@{
		sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $resolvedInstaller).Hash.ToLowerInvariant()
		signature_status = $signature.Status.ToString()
		signer = $signature.SignerCertificate.Subject
	}
	$process = Start-Process -FilePath $resolvedInstaller -ArgumentList @('/install', '/quiet', '/norestart') -Wait -PassThru -WindowStyle Hidden
	if ($process.ExitCode -notin @(0, 1638, 3010)) {
		$result.status = 'failed'
		Write-Result $result
		throw "The x86 MSVC redistributable installer failed with exit code $($process.ExitCode)."
	}
}

$directXCheck = $checks | Where-Object { $_.name -eq 'D3DX9_43.dll' } | Select-Object -First 1
if ($null -ne $directXCheck -and (-not $directXCheck.present -or $directXCheck.architecture -ne 'x86')) {
	if ($directXCheck.present) {
		$result.status = 'failed'
		Write-Result $result
		throw 'The existing application-local D3DX9_43.dll is not x86; refusing to replace it automatically.'
	}
	$downloadRoot = [IO.Path]::GetFullPath($DownloadDirectory)
	New-Item -ItemType Directory -Force -Path $downloadRoot | Out-Null
	$packagePath = Join-Path $downloadRoot 'Microsoft.DXSDK.D3DX.9.29.952.8.nupkg'
	Invoke-WebRequest -Uri $dxsdkUri -OutFile $packagePath
	$observedPackageHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $packagePath).Hash.ToLowerInvariant()
	if ($observedPackageHash -ne $dxsdkSha256) {
		$result.status = 'failed'
		Write-Result $result
		throw "Microsoft.DXSDK.D3DX package hash mismatch: expected $dxsdkSha256, got $observedPackageHash."
	}
	Add-Type -AssemblyName System.IO.Compression.FileSystem
	$archive = [IO.Compression.ZipFile]::OpenRead($packagePath)
	try {
		$dllEntry = $archive.GetEntry($dxsdkDllEntry)
		if ($null -eq $dllEntry) { throw "The pinned package does not contain $dxsdkDllEntry." }
		$dllPath = Join-Path $applicationRoot 'D3DX9_43.dll'
		$dllStream = [IO.File]::Create($dllPath)
		try {
			$entryStream = $dllEntry.Open()
			try { $entryStream.CopyTo($dllStream) } finally { $entryStream.Dispose() }
		} finally {
			$dllStream.Dispose()
		}
		foreach ($licenseName in @('LICENSE.txt', 'NOTICE.md')) {
			$licenseEntry = $archive.GetEntry($licenseName)
			if ($null -eq $licenseEntry) { throw "The pinned package does not contain $licenseName." }
			$licensePath = Join-Path $downloadRoot $licenseName
			$licenseStream = [IO.File]::Create($licensePath)
			try {
				$entryStream = $licenseEntry.Open()
				try { $entryStream.CopyTo($licenseStream) } finally { $entryStream.Dispose() }
			} finally {
				$licenseStream.Dispose()
			}
			$result.licenses += [ordered]@{
				name = $licenseName
				sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $licensePath).Hash.ToLowerInvariant()
			}
		}
	} finally {
		$archive.Dispose()
	}
}

$finalChecks = @(Get-RuntimeChecks $runtimeRoot $applicationRoot)
$finalMissing = @(Get-MissingNames $finalChecks)
$result.checked = $finalChecks
$result.missing = $finalMissing
$result.installed = @($checks | Where-Object { -not $_.present } | ForEach-Object { $_.name })
if ($finalMissing.Count -ne 0) {
	$result.status = 'failed'
	Write-Result $result
	throw "BYOND runtime installation completed but prerequisites remain missing or non-x86: $([string]::Join(', ', $finalMissing))."
}
$result.status = 'ready'
Write-Result $result
