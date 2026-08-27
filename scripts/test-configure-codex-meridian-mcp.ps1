[CmdletBinding()]
param()

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) ('meridian-config-' + [Guid]::NewGuid().ToString('N'))
try {
	$workspace = Join-Path $temporaryRoot 'workspace'
	$repository = Join-Path $temporaryRoot 'repository'
	$state = Join-Path $temporaryRoot 'state'
	New-Item -ItemType Directory -Force -Path $workspace, $repository, $state | Out-Null
	$config = Join-Path $temporaryRoot 'config.toml'
	$helper = Join-Path $temporaryRoot 'manifest.json'
	$binary = (Get-Process -Id $PID).Path
	[IO.File]::WriteAllText($helper, '{}', [Text.UTF8Encoding]::new($false))
	$originalOther = "[mcp_servers.other]`ncommand = 'untouched'`n[mcp_servers.other.env]`nKEEP = 'byte-equivalent'"
	$source = "$originalOther`n`n[mcp_servers.meridian-mcp]`nenabled = true`nstartup_timeout_sec = 30`ncommand = 'old'`ntool_timeout_sec = 1900`n[mcp_servers.meridian-mcp.env]`nUNRELATED = 'preserve-me'`nMERIDIAN_MCP_ROOTS = 'old-root'`n"
	[IO.File]::WriteAllText($config, $source, [Text.UTF8Encoding]::new($false))
	& (Join-Path $PSScriptRoot 'configure-codex-meridian-mcp.ps1') -ConfigPath $config -BinaryPath $binary -HelperManifestPath $helper -WorkspaceRoots $workspace -RepositoryRoots $repository -StateDirectory $state -Development
	$result = [IO.File]::ReadAllText($config)
	if (-not $result.Contains($originalOther)) { throw 'The unrelated MCP server was modified.' }
	foreach ($required in @("UNRELATED = 'preserve-me'", 'enabled = true', 'startup_timeout_sec = 30', 'tool_timeout_sec = 1900', 'MERIDIAN_MCP_ROOTS', 'MERIDIAN_MCP_REPOSITORIES', 'MERIDIAN_MCP_STATE_DIR')) {
		if (-not $result.Contains($required)) { throw "Configured TOML is missing $required" }
	}
} finally {
	if (Test-Path -LiteralPath $temporaryRoot) { Remove-Item -LiteralPath $temporaryRoot -Recurse -Force }
}

Write-Output 'Codex Meridian-MCP configuration round-trip passed.'
