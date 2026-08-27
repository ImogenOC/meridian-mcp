[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$ConfigPath,
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$HelperManifestPath,
	[string]$ServerName = 'meridian-mcp',
	[string[]]$WorkspaceRoots = @(),
	[string[]]$RepositoryRoots = @(),
	[string]$StateDirectory,
	[switch]$Development,
	[switch]$EnableTracy
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$config = (Resolve-Path -LiteralPath $ConfigPath).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$helperManifest = (Resolve-Path -LiteralPath $HelperManifestPath).Path
function Resolve-ExistingDirectoryList([string[]]$Paths, [string]$Name) {
	@($Paths | ForEach-Object {
		if (-not (Test-Path -LiteralPath $_ -PathType Container)) { throw "$Name directory does not exist: $_" }
		$resolved = (Resolve-Path -LiteralPath $_).Path
		if ($resolved.Contains("'") -or $resolved.Contains("`n") -or $resolved.Contains("`r")) { throw "$Name contains characters that cannot be written safely to TOML." }
		$resolved
	} | Select-Object -Unique)
}
$resolvedWorkspaceRoots = @(Resolve-ExistingDirectoryList $WorkspaceRoots 'Workspace root')
$resolvedRepositoryRoots = @(Resolve-ExistingDirectoryList $RepositoryRoots 'Repository root')
$resolvedStateDirectory = $null
if ($Development) {
	if ([string]::IsNullOrWhiteSpace($StateDirectory) -or -not (Test-Path -LiteralPath $StateDirectory -PathType Container)) { throw 'Development mode requires an existing StateDirectory.' }
	$resolvedStateDirectory = (Resolve-Path -LiteralPath $StateDirectory).Path
	foreach ($root in $resolvedWorkspaceRoots) {
		$relative = [IO.Path]::GetRelativePath($root, $resolvedStateDirectory)
		if ($relative -eq '.' -or (-not $relative.StartsWith('..' + [IO.Path]::DirectorySeparatorChar) -and $relative -ne '..')) { throw 'StateDirectory must be outside every workspace root.' }
	}
}
$serverNamePattern = '^[A-Za-z0-9_-]+$'
if ($ServerName -notmatch $serverNamePattern) { throw 'ServerName may contain only letters, numbers, underscores, and hyphens.' }
$escapedServerName = [regex]::Escape($ServerName)
$text = [IO.File]::ReadAllText($config)
$serverPattern = '(?ms)(\[mcp_servers\.' + $escapedServerName + '\]\s*\r?\n)(.*?)(?=\r?\n\[|\z)'
$serverMatch = [regex]::Match($text, $serverPattern)
if (-not $serverMatch.Success) { throw "Expected one $ServerName server table." }
$serverBody = $serverMatch.Groups[2].Value
$commandPattern = '(?m)^command\s*=\s*[^\r\n]+$'
if ([regex]::Matches($serverBody, $commandPattern).Count -ne 1) { throw "Expected one $ServerName server command." }
$serverBody = [regex]::Replace($serverBody, $commandPattern, ("command = '" + $binary + "'"))
$text = $text.Substring(0, $serverMatch.Groups[2].Index) + $serverBody + $text.Substring($serverMatch.Groups[2].Index + $serverMatch.Groups[2].Length)
$environmentPattern = '(?ms)(\[mcp_servers\.' + $escapedServerName + '\.env\]\s*\r?\n)(.*?)(?=\r?\n\[|\z)'
$match = [regex]::Match($text, $environmentPattern)
if (-not $match.Success) { throw "Expected one $ServerName environment section." }
$body = $match.Groups[2].Value
$entries = [ordered]@{ MERIDIAN_MCP_HELPER_MANIFEST = $helperManifest; MERIDIAN_MCP_DEBUGGER = 'auxtools' }
if ($resolvedWorkspaceRoots.Count -gt 0) { $entries.MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, $resolvedWorkspaceRoots) }
if ($resolvedRepositoryRoots.Count -gt 0) { $entries.MERIDIAN_MCP_REPOSITORIES = [string]::Join([IO.Path]::PathSeparator, $resolvedRepositoryRoots) }
if ($Development) {
	$entries.MERIDIAN_MCP_MODE = 'development'
	$entries.MERIDIAN_MCP_STATE_DIR = $resolvedStateDirectory
}
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
