[CmdletBinding()]
param([Parameter(Mandatory)][string]$DestinationRoot)

$ErrorActionPreference = 'Stop'
$url = 'https://github.com/willox/auxtools/releases/download/v2.3.7/debug_server.dll'
$expected = 'b188999ac58a0e0171b015c39a403ab7da2f37ddb8ac3817a078f5bce02a8be7'
$root = [IO.Path]::GetFullPath($DestinationRoot)
$destination = Join-Path $root 'helpers/auxtools/v2.3.7/debug_server.dll'
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination) | Out-Null
$temporary = Join-Path (Split-Path -Parent $destination) ('.download-' + [Guid]::NewGuid().ToString('N') + '.tmp')
try {
	Invoke-WebRequest -Uri $url -OutFile $temporary -UseBasicParsing
	$actual = (Get-FileHash -Algorithm SHA256 -LiteralPath $temporary).Hash.ToLowerInvariant()
	if ($actual -ne $expected) { throw "auxtools SHA-256 mismatch: $actual" }
	if ($IsWindows) { Unblock-File -LiteralPath $temporary }
	Move-Item -LiteralPath $temporary -Destination $destination -Force
} finally {
	if (Test-Path -LiteralPath $temporary) { Remove-Item -LiteralPath $temporary -Force }
}
