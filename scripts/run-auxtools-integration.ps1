[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$EvidencePath,
	[string]$DmbPath,
	[ValidateSet('interactive', 'headless')][string]$HostMode = 'headless'
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$mcpRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$compiler = (Resolve-Path -LiteralPath $DreamMakerPath).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
Import-Module (Join-Path $PSScriptRoot 'MeridianMcpSession.psm1') -Force

$ownedFixture = [string]::IsNullOrWhiteSpace($DmbPath)
$ownedFixtureArtifacts = @()
if ($ownedFixture) {
	$fixtureRoot = (Resolve-Path -LiteralPath (Join-Path $mcpRoot 'tests/fixtures/auxtools')).Path
	& $compiler (Join-Path $fixtureRoot 'auxtools.dme')
	$dmbCandidate = Join-Path $fixtureRoot 'auxtools.dmb'
	if ($LASTEXITCODE -ne 0 -or -not (Test-Path -LiteralPath $dmbCandidate -PathType Leaf)) {
		throw 'The owned auxtools fixture did not compile.'
	}
	$dmb = (Resolve-Path -LiteralPath $dmbCandidate).Path
	$ownedFixtureArtifacts = @('auxtools.dmb', 'auxtools.rsc', 'auxtools.pdb', 'auxtools.log') | ForEach-Object { Join-Path $fixtureRoot $_ }
} else {
	$dmb = (Resolve-Path -LiteralPath $DmbPath).Path
}
$runtimeRoot = (Split-Path -Parent $dmb)
$hostExecutableName = if ($HostMode -eq 'headless') { 'dreamdaemon.exe' } else { 'dreamseeker.exe' }
$hostExecutable = (Resolve-Path -LiteralPath (Join-Path (Split-Path -Parent $compiler) $hostExecutableName)).Path
$systemTemporaryRoot = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$stateDirectory = Join-Path $systemTemporaryRoot ('.meridian-auxtools-state-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $stateDirectory | Out-Null
$stateDirectory = (Resolve-Path -LiteralPath $stateDirectory).Path
$relativeStateDirectory = [IO.Path]::GetRelativePath($systemTemporaryRoot, $stateDirectory)
if ($relativeStateDirectory -eq '..' -or $relativeStateDirectory.StartsWith('..' + [IO.Path]::DirectorySeparatorChar)) {
	throw 'Temporary state directory resolved outside the operating-system temporary directory.'
}

function Request([int]$Id, [string]$Name, [hashtable]$Arguments) {
	return ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = $Id; method = 'tools/call'; params = [ordered]@{ name = $Name; arguments = $Arguments } })
}
$requests = @(
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'meridian-auxtools-integration'; version = '1.0' } } })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })),
	(ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 2; method = 'tools/list'; params = [ordered]@{} })),
	(Request 3 'dm_debug_launch' @{ dmb_path = $dmb; host_mode = $HostMode; startup_timeout_ms = 60000 }),
	(Request 4 'dm_debug_threads' @{}),
	(Request 5 'dm_debug_set_exception_breakpoints' @{ break_on_runtimes = $true }),
	(Request 6 'dm_debug_stop' @{})
)
$environment = @{
	MERIDIAN_MCP_MODE = 'development'
	MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, (@($mcpRoot, $runtimeRoot) | Select-Object -Unique))
	MERIDIAN_MCP_COMPILERS = $compiler
	MERIDIAN_MCP_STATE_DIR = $stateDirectory
	MERIDIAN_MCP_DEBUGGER = 'auxtools'
}
$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $evidenceFile) | Out-Null
$evidence = [ordered]@{
	schema_version = 2
	overall = 'failed'
	fixture_kind = if ($ownedFixture) { 'owned_protocol_fixture' } else { 'supplied_dmb' }
	host_mode = $HostMode
	host_executable = $hostExecutableName
	host_sha256 = (Get-FileHash -LiteralPath $hostExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
	auxtools_version = 'v2.3.7'
	auxtools_sha256 = 'b188999ac58a0e0171b015c39a403ab7da2f37ddb8ac3817a078f5bce02a8be7'
	dmb_sha256 = (Get-FileHash -LiteralPath $dmb -Algorithm SHA256).Hash.ToLowerInvariant()
	requests_expected = 6
	requests_completed = 0
	mcp_exit_code = $null
	mcp_stderr = $null
	response_timings_ms = $null
	failure_stage = $null
	failure_message = $null
	host_log = $null
	runner_image = [Environment]::GetEnvironmentVariable('ImageOS')
	runner_image_version = [Environment]::GetEnvironmentVariable('ImageVersion')
	os_version = [Environment]::OSVersion.VersionString
	failure = $null
}
$currentStage = 'debugger_session'
$diagnosticRedactions = @{
	$mcpRoot = '<mcp-root>'
	$fixtureRoot = '<fixture-root>'
	$stateDirectory = '<state-directory>'
}
try {
	$session = Invoke-McpSession -BinaryPath $binary -WorkingDirectory $mcpRoot -Environment $environment -Requests $requests -TimeoutMilliseconds 120000
	$evidence.mcp_exit_code = $session.ExitCode
	$evidence.requests_completed = @($session.Responses).Count
	$evidence.response_timings_ms = $session.ResponseTimingsMilliseconds
	if (-not [string]::IsNullOrWhiteSpace($session.Stderr)) {
		$evidence.mcp_stderr = $session.Stderr.Substring(0, [Math]::Min($session.Stderr.Length, 8192))
	}
	if ($session.ExitCode -ne 0) { throw "Debugger MCP session exited with $($session.ExitCode)." }
	foreach ($id in 1..6) {
		$response = Get-McpResponse -Responses $session.Responses -Id $id
		if ($id -ge 3 -and $response.result.isError -eq $true) {
			throw "Debugger request $id failed: $($response.result.content[0].text)"
		}
	}
	$tools = (Get-McpResponse -Responses $session.Responses -Id 2).result.tools.name
	if ($tools -notcontains 'dm_debug_launch' -or $tools -notcontains 'dm_debug_stop') { throw 'Debugger tools were not advertised.' }
	$evidence.overall = 'passed'
} catch {
	$evidence.failure_stage = $currentStage
	$evidence.failure_message = ConvertTo-BoundedDiagnostic -Text $_.Exception.Message -Redactions $diagnosticRedactions
	$evidence.failure = $evidence.failure_message
	throw
} finally {
	$hostLogPath = [IO.Path]::ChangeExtension($dmb, '.log')
	if (Test-Path -LiteralPath $hostLogPath -PathType Leaf) {
		$evidence.host_log = ConvertTo-BoundedDiagnostic -Text (Get-Content -LiteralPath $hostLogPath -Raw) -Redactions $diagnosticRedactions
	}
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 5) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	Remove-Item -LiteralPath $stateDirectory -Recurse -Force -ErrorAction SilentlyContinue
	foreach ($artifact in $ownedFixtureArtifacts) {
		Remove-Item -LiteralPath $artifact -Force -ErrorAction SilentlyContinue
	}
}
