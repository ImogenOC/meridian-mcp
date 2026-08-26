[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$EvidencePath,
	[string]$HelperManifestPath,
	[ValidateRange(1, 100000)][int]$PrototypeCount = 65537,
	[ValidateRange(10, 900)][int]$TimeoutSeconds = 300
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$mcpRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
$evidenceRoot = Split-Path -Parent $evidenceFile
New-Item -ItemType Directory -Force -Path $evidenceRoot | Out-Null
$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ('meridian-large-parser-' + [Guid]::NewGuid().ToString('N'))
$spacemanRevision = '351ddc0ffb2439876d4565ce5130bb6b027ee605'
$failure = $null
Import-Module -Force (Join-Path $PSScriptRoot 'MeridianMcpSession.psm1')
Import-Module -Force (Join-Path $PSScriptRoot 'process-readiness.psm1')

function New-ToolCall([int]$Id, [string]$Name, [hashtable]$Arguments) {
	return ConvertTo-McpJsonLine ([ordered]@{
		jsonrpc = '2.0'
		id = $Id
		method = 'tools/call'
		params = [ordered]@{ name = $Name; arguments = $Arguments }
	})
}

function Get-ToolPayload([object[]]$Responses, [int]$Id, [string]$Stage) {
	$response = Get-McpResponse -Responses $Responses -Id $Id
	if ($response.result.isError -eq $true) {
		throw "$Stage returned an MCP tool error: $($response.result.content[0].text)"
	}
	return $response.result.content[0].text | ConvertFrom-Json
}

function Get-HelperEvidence([string]$ManifestPath) {
	if ([string]::IsNullOrWhiteSpace($ManifestPath)) {
		return [ordered]@{ status = 'not_provided' }
	}
	$resolvedManifest = (Resolve-Path -LiteralPath $ManifestPath).Path
	$manifest = Get-Content -Raw -LiteralPath $resolvedManifest | ConvertFrom-Json
	$platform = if ($IsWindows) { 'windows' } elseif ($IsLinux) { 'linux' } else { throw 'Unsupported parser integration platform.' }
	$helper = @($manifest.helpers | Where-Object { $_.id -eq 'dmdoc' -and $_.platform -eq $platform })
	if ($helper.Count -ne 1) { throw "The helper manifest does not contain one $platform dmdoc entry." }
	if ($helper[0].source_revision -ne $spacemanRevision) { throw 'The dmdoc helper does not use the pinned SpacemanDMM revision.' }
	$helperPath = [IO.Path]::GetFullPath((Join-Path (Split-Path -Parent $resolvedManifest) $helper[0].path))
	$actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $helperPath).Hash.ToLowerInvariant()
	if ($actualHash -ne $helper[0].sha256) { throw 'The dmdoc helper hash does not match its manifest.' }
	return [ordered]@{
		status = 'verified'
		id = 'dmdoc'
		platform = $helper[0].platform
		target_arch = $helper[0].target_arch
		sha256 = $actualHash
		source_revision = $helper[0].source_revision
		manifest_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $resolvedManifest).Hash.ToLowerInvariant()
	}
}

$evidence = [ordered]@{
	schema_version = 1
	overall = 'failed'
	started_at_utc = [DateTime]::UtcNow.ToString('O')
	finished_at_utc = $null
	platform = [ordered]@{
		os = [Runtime.InteropServices.RuntimeInformation]::OSDescription
		architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
		runner_image = $env:ImageOS
	}
	binary = [ordered]@{
		sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $binary).Hash.ToLowerInvariant()
		meridian_mcp_revision = (& git -C $mcpRoot rev-parse HEAD).Trim()
	}
	spacemandmm = [ordered]@{
		source_revision = $spacemanRevision
		helper = $null
	}
	fixture = $null
	parse = $null
	lookups = @()
	timings_milliseconds = [ordered]@{}
	failure = $null
}

try {
	$evidence.spacemandmm.helper = Get-HelperEvidence $HelperManifestPath
	$fixtureMetadata = & (Join-Path $PSScriptRoot 'new-large-prototype-fixture.ps1') -OutputDirectory $fixtureRoot -PrototypeCount $PrototypeCount | ConvertFrom-Json -AsHashtable
	$evidence.fixture = ConvertTo-PublicPrototypeFixtureEvidence $fixtureMetadata

	$paths = [Collections.Generic.List[string]]::new()
	$paths.Add([string]$fixtureMetadata.first_path)
	if ($null -ne $fixtureMetadata.boundary_path) { $paths.Add([string]$fixtureMetadata.boundary_path) }
	if ($paths -notcontains [string]$fixtureMetadata.last_path) { $paths.Add([string]$fixtureMetadata.last_path) }

	$requests = [Collections.Generic.List[string]]::new()
	$requests.Add((ConvertTo-McpJsonLine ([ordered]@{
		jsonrpc = '2.0'; id = 1; method = 'initialize'
		params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'large-prototype-parser'; version = '1.0' } }
	})))
	$requests.Add((ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })))
	$requests.Add((New-ToolCall 2 'dm_parse_environment' @{ dme_path = $fixtureMetadata.environment }))
	$requestId = 10
	foreach ($path in $paths) {
		$requests.Add((New-ToolCall $requestId 'dm_get_type' @{ type_path = $path }))
		$requestId++
	}

	$environment = @{
		MERIDIAN_MCP_MODE = 'analysis'
		MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, @($mcpRoot, $fixtureRoot))
	}
	if (-not [string]::IsNullOrWhiteSpace($HelperManifestPath)) {
		$environment.MERIDIAN_MCP_HELPER_MANIFEST = (Resolve-Path -LiteralPath $HelperManifestPath).Path
	}
	$session = Invoke-McpSession -BinaryPath $binary -WorkingDirectory $mcpRoot -Environment $environment -Requests $requests.ToArray() -TimeoutMilliseconds ($TimeoutSeconds * 1000)
	if ($session.ExitCode -ne 0) { throw "Meridian-MCP exited with $($session.ExitCode)." }

	$parse = Get-ToolPayload $session.Responses 2 'dm_parse_environment'
	if ($parse.success -ne $true) { throw 'dm_parse_environment did not report success.' }
	if ([int64]$parse.total_types -lt [int64]$fixtureMetadata.declared_type_count) { throw 'The parser indexed fewer types than the fixture declares.' }
	if ([int64]$parse.indexed_symbols -le 0) { throw 'The parser did not index any symbols.' }
	$evidence.parse = [ordered]@{
		total_types = [int64]$parse.total_types
		indexed_symbols = [int64]$parse.indexed_symbols
		state_generation = $parse.state_generation
	}
	$evidence.timings_milliseconds.parse = [int64]$session.ResponseTimingsMilliseconds['2']

	$requestId = 10
	foreach ($expectedPath in $paths) {
		$lookup = Get-ToolPayload $session.Responses $requestId 'dm_get_type'
		if ($lookup.path -ne $expectedPath) { throw "dm_get_type returned $($lookup.path), expected $expectedPath." }
		$evidence.lookups += [ordered]@{
			path = $lookup.path
			duration_milliseconds = [int64]$session.ResponseTimingsMilliseconds[[string]$requestId]
		}
		$requestId++
	}
	$evidence.overall = 'passed'
} catch {
	$failure = $_
	$message = $_.Exception.Message.Replace($fixtureRoot, '<fixture>').Replace($mcpRoot, '<meridian-mcp>')
	$evidence.failure = [ordered]@{ message = $message; category = $_.CategoryInfo.Category.ToString() }
} finally {
	$evidence.finished_at_utc = [DateTime]::UtcNow.ToString('O')
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 10) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	if (Test-Path -LiteralPath $fixtureRoot -PathType Container) {
		Remove-Item -LiteralPath $fixtureRoot -Recurse -Force
	}
}

if ($null -ne $failure) { throw $failure }
