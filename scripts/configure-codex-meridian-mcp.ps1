[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$ConfigPath,
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$HelperManifestPath,
	[string]$ServerName = 'meridian-mcp',
	[switch]$EnableTracy
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$config = (Resolve-Path -LiteralPath $ConfigPath).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$helperManifest = (Resolve-Path -LiteralPath $HelperManifestPath).Path
$serverNamePattern = '^[A-Za-z0-9_-]+$'
if ($ServerName -notmatch $serverNamePattern) { throw 'ServerName may contain only letters, numbers, underscores, and hyphens.' }
$escapedServerName = [regex]::Escape($ServerName)
$text = [IO.File]::ReadAllText($config)
$serverPattern = '(?ms)(\[mcp_servers\.' + $escapedServerName + '\]\s*\r?\n)command\s*=\s*[^\r\n]+'
if ([regex]::Matches($text, $serverPattern).Count -ne 1) { throw "Expected one $ServerName server command." }
$text = [regex]::Replace($text, $serverPattern, ('$1command = ''' + $binary + ''''))
$environmentPattern = '(?ms)(\[mcp_servers\.' + $escapedServerName + '\.env\]\s*\r?\n)(.*?)(?=\r?\n\[|\z)'
$match = [regex]::Match($text, $environmentPattern)
if (-not $match.Success) { throw "Expected one $ServerName environment section." }
$body = $match.Groups[2].Value
$entries = [ordered]@{ MERIDIAN_MCP_HELPER_MANIFEST = $helperManifest; MERIDIAN_MCP_DEBUGGER = 'auxtools' }
if ($EnableTracy) { $entries.MERIDIAN_MCP_TRACY = 'byond' }
$body = [regex]::Replace($body, '(?m)^MERIDIAN_MCP_TRACY\s*=.*(?:\r?\n)?', '')
foreach ($entry in $entries.GetEnumerator()) {
	$linePattern = '(?m)^' + [regex]::Escape($entry.Key) + '\s*=.*$'
	$line = $entry.Key + " = '" + $entry.Value + "'"
	if ([regex]::IsMatch($body, $linePattern)) { $body = [regex]::Replace($body, $linePattern, $line) } else { $body = $body.TrimEnd() + [Environment]::NewLine + $line + [Environment]::NewLine }
}
$text = $text.Substring(0, $match.Groups[2].Index) + $body + $text.Substring($match.Groups[2].Index + $match.Groups[2].Length)
$temporary = $config + '.tmp-' + [Guid]::NewGuid().ToString('N')
try {
	[IO.File]::WriteAllText($temporary, $text, [Text.UTF8Encoding]::new($false))
	Move-Item -LiteralPath $temporary -Destination $config -Force
} finally {
	Remove-Item -LiteralPath $temporary -Force -ErrorAction SilentlyContinue
}
