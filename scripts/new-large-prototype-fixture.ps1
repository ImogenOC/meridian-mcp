[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$OutputDirectory,
	[ValidateRange(65537, 100000)][int]$PrototypeCount = 65537
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $root | Out-Null
$sourcePath = Join-Path $root 'large_prototypes.dm'
$environmentPath = Join-Path $root 'large_prototypes.dme'
$writer = [IO.StreamWriter]::new($sourcePath, $false, [Text.UTF8Encoding]::new($false))
try {
	$writer.WriteLine('/world/New()')
	$writer.WriteLine("`t..()")
	$writer.WriteLine("`ttext2file(`"MERIDIAN_LARGE_PROTOTYPE_READY`", `"startup.marker`")")
	$writer.WriteLine("`tworld.log << `"MERIDIAN_LARGE_PROTOTYPE_READY`"")
	$writer.WriteLine("`tshutdown()")
	$writer.WriteLine()
	$writer.WriteLine('/datum/meridian_large_prototype')
	for ($index = 0; $index -lt $PrototypeCount; $index++) {
		$bucket = [int][Math]::Floor($index / 256)
		if (($index % 256) -eq 0) {
			$writer.WriteLine(('/datum/meridian_large_prototype/b{0:D4}' -f $bucket))
		}
		$writer.WriteLine(('/datum/meridian_large_prototype/b{0:D4}/p{1:D5}' -f $bucket, $index))
	}
} finally {
	$writer.Dispose()
}
[IO.File]::WriteAllText($environmentPath, '#include "large_prototypes.dm"' + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))

[pscustomobject]@{
	prototype_count = $PrototypeCount
	source = $sourcePath
	environment = $environmentPath
} | ConvertTo-Json -Compress
