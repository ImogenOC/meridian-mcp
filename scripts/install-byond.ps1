[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^[0-9]+\.[0-9]+$')]
    [string]$Version,
    [Parameter(Mandatory)]
    [string]$ArchivePath,
    [Parameter(Mandatory)]
    [string]$DestinationPath,
    [Parameter(Mandatory)]
    [ValidatePattern('^[0-9a-fA-F]{64}$')]
    [string]$ExpectedSha256,
    [ValidateRange(1, 5)]
    [int]$MaxAttempts = 3
)

$ErrorActionPreference = 'Stop'
$majorVersion = $Version.Split('.')[0]
$downloadSources = @(
    "https://byond-builds.dm-lang.org/$majorVersion/${Version}_byond.zip",
    "http://www.byond.com/download/build/$majorVersion/${Version}_byond.zip"
)
$headers = @{ 'User-Agent' = 'tgstation/1.0 CI Script' }
$lastFailure = $null

foreach ($downloadUri in $downloadSources) {
    for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
        try {
            Write-Host "Downloading BYOND $Version from $downloadUri (attempt $attempt of $MaxAttempts)"
            Invoke-WebRequest -UseBasicParsing -Headers $headers -Uri $downloadUri -OutFile $ArchivePath

            $archive = Get-Item -LiteralPath $ArchivePath
            if ($archive.Length -lt 4) {
                throw "BYOND download was only $($archive.Length) bytes"
            }

            $stream = [IO.File]::OpenRead($archive.FullName)
            try {
                $signature = [byte[]]::new(4)
                if ($stream.Read($signature, 0, $signature.Length) -ne $signature.Length) {
                    throw 'Could not read the BYOND archive signature'
                }
            } finally {
                $stream.Dispose()
            }
            if ($signature[0] -ne 0x50 -or $signature[1] -ne 0x4b) {
                throw 'BYOND download is not a ZIP archive'
            }

            $actualSha256 = (Get-FileHash -LiteralPath $ArchivePath -Algorithm SHA256).Hash
            if ($actualSha256 -ne $ExpectedSha256) {
                throw "BYOND archive SHA-256 mismatch: expected $ExpectedSha256, received $actualSha256"
            }

            Expand-Archive -LiteralPath $ArchivePath -DestinationPath $DestinationPath -Force
            $dreamMakerPath = Join-Path $DestinationPath 'byond\bin\dm.exe'
            if (-not (Test-Path -LiteralPath $dreamMakerPath -PathType Leaf)) {
                throw "BYOND archive did not contain DreamMaker: $dreamMakerPath"
            }

            Write-Host "Installed BYOND $Version from $($archive.Length) verified bytes"
            return
        } catch {
            $lastFailure = $_.Exception.Message
            Write-Warning "BYOND installation attempt $attempt from $downloadUri failed: $lastFailure"
            if ($attempt -lt $MaxAttempts) {
                Start-Sleep -Seconds (2 * $attempt)
            }
        }
    }
}

throw "Could not install BYOND $Version from any configured source. Last failure: $lastFailure"
