[CmdletBinding()]
param(
    [Parameter(Mandatory)]
    [ValidatePattern('^[0-9]+\.[0-9]+$')]
    [string]$Version,
    [Parameter(Mandatory)]
    [string]$ArchivePath,
    [Parameter(Mandatory)]
    [string]$DestinationPath,
    [ValidateRange(1, 5)]
    [int]$MaxAttempts = 3
)

$ErrorActionPreference = 'Stop'
$majorVersion = $Version.Split('.')[0]
$downloadUri = "https://www.byond.com/download/build/$majorVersion/${Version}_byond.zip"

for ($attempt = 1; $attempt -le $MaxAttempts; $attempt++) {
    try {
        Write-Host "Downloading BYOND $Version (attempt $attempt of $MaxAttempts)"
        Invoke-WebRequest -UseBasicParsing -Uri $downloadUri -OutFile $ArchivePath

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

        Expand-Archive -LiteralPath $ArchivePath -DestinationPath $DestinationPath -Force
        $dreamMakerPath = Join-Path $DestinationPath 'byond\bin\dm.exe'
        if (-not (Test-Path -LiteralPath $dreamMakerPath -PathType Leaf)) {
            throw "BYOND archive did not contain DreamMaker: $dreamMakerPath"
        }

        Write-Host "Installed BYOND $Version from $($archive.Length) downloaded bytes"
        return
    } catch {
        if ($attempt -eq $MaxAttempts) {
            throw
        }
        Write-Warning "BYOND installation attempt $attempt failed: $($_.Exception.Message)"
        Start-Sleep -Seconds (2 * $attempt)
    }
}
