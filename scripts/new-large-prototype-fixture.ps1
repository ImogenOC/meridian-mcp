[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$OutputDirectory,
	[ValidateRange(1, 100000)][int]$PrototypeCount = 65537,
	[ValidateSet('flat', 'bucketed')][string]$Layout = 'flat'
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
	$writer.WriteLine('/datum/mlp')
	for ($index = 0; $index -lt $PrototypeCount; $index++) {
		if ($Layout -eq 'bucketed') {
			$bucket = [int][Math]::Floor($index / 256)
			if (($index % 256) -eq 0) {
				$writer.WriteLine(('/datum/mlp/b{0:D4}' -f $bucket))
			}
			$writer.WriteLine(('/datum/mlp/b{0:D4}/p{1:D5}' -f $bucket, $index))
		} else {
			$writer.WriteLine(('/datum/mlp/p{0:D5}' -f $index))
		}
	}
} finally {
	$writer.Dispose()
}
[IO.File]::WriteAllText($environmentPath, '#include "large_prototypes.dm"' + [Environment]::NewLine, [Text.UTF8Encoding]::new($false))

$bucketCount = if ($Layout -eq 'bucketed') { [int][Math]::Ceiling($PrototypeCount / 256.0) } else { 0 }
$declaredParentCount = 1 + $bucketCount
$pathFor = {
	param([int]$Index)
	if ($Layout -eq 'bucketed') {
		$bucket = [int][Math]::Floor($Index / 256)
		return ('/datum/mlp/b{0:D4}/p{1:D5}' -f $bucket, $Index)
	}
	return ('/datum/mlp/p{0:D5}' -f $Index)
}

[pscustomobject]@{
	layout = $Layout
	declared_leaf_count = $PrototypeCount
	declared_parent_count = $declaredParentCount
	declared_type_count = $PrototypeCount + $declaredParentCount
	first_path = & $pathFor 0
	boundary_path = if ($PrototypeCount -gt 65535) { & $pathFor 65535 } else { $null }
	last_path = & $pathFor ($PrototypeCount - 1)
	source = $sourcePath
	environment = $environmentPath
} | ConvertTo-Json -Compress
