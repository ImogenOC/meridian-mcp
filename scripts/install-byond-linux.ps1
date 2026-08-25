[CmdletBinding()]
param(
	[Parameter(Mandatory)][ValidatePattern('^[0-9]+\.[0-9]+$')][string]$Version,
	[Parameter(Mandatory)][string]$ArchivePath,
	[Parameter(Mandatory)][string]$DestinationPath,
	[Parameter(Mandatory)][ValidatePattern('^[0-9a-fA-F]{64}$')][string]$ExpectedSha256,
	[ValidateRange(1, 5)][int]$MaxAttempts = 3
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
if (-not $IsLinux) { throw 'The Linux BYOND installer only runs on Linux.' }
$majorVersion = $Version.Split('.')[0]
$downloadSources = @(
	"https://byond-builds.dm-lang.org/$majorVersion/${Version}_byond_linux.zip",
	"http://www.byond.com/download/build/$majorVersion/${Version}_byond_linux.zip"
)
$headers = @{ 'User-Agent' = 'tgstation/1.0 CI Script' }
$lastFailure = $null

foreach ($downloadUri in $downloadSources) {
	for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
		try {
			Invoke-WebRequest -UseBasicParsing -Headers $headers -Uri $downloadUri -OutFile $ArchivePath
			$archive = Get-Item -LiteralPath $ArchivePath
			if ($archive.Length -lt 4) { throw "BYOND download was only $($archive.Length) bytes." }
			if ((Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash -ne $ExpectedSha256) { throw 'BYOND archive SHA-256 mismatch.' }
			Expand-Archive -LiteralPath $ArchivePath -DestinationPath $DestinationPath -Force
			$byondRoot = Join-Path $DestinationPath 'byond'
			& make -C $byondRoot here
			if ($LASTEXITCODE -ne 0) { throw 'BYOND make here failed.' }
			foreach ($name in @('DreamMaker', 'DreamDaemon')) {
				$executable = Join-Path $byondRoot "bin/$name"
				if (-not (Test-Path -LiteralPath $executable -PathType Leaf)) { throw "BYOND archive did not contain $name." }
				& chmod +x $executable
			}
			return
		} catch {
			$lastFailure = $_.Exception.Message
			if ($attempt -lt $MaxAttempts) { Start-Sleep -Seconds (2 * $attempt) }
		}
	}
}
throw "Could not install Linux BYOND $Version. Last failure: $lastFailure"
