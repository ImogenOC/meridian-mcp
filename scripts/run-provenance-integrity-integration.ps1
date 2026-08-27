[CmdletBinding()]
param(
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$EvidencePath,
	[string]$ExpectedByondVersion = '516.1687'
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$mcpRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
$dreamMaker = (Resolve-Path -LiteralPath $DreamMakerPath).Path
$binary = (Resolve-Path -LiteralPath $BinaryPath).Path
$dreamMakerVersion = (Get-Item -LiteralPath $dreamMaker).VersionInfo.FileVersion
if ($dreamMakerVersion -notmatch [regex]::Escape($ExpectedByondVersion)) { throw "DreamMaker version $dreamMakerVersion does not match required $ExpectedByondVersion." }
$evidenceFile = [IO.Path]::GetFullPath($EvidencePath)
Import-Module (Join-Path $PSScriptRoot 'MeridianMcpSession.psm1') -Force

function New-Call([int]$Id, [string]$Name, [hashtable]$Arguments) {
	ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = $Id; method = 'tools/call'; params = [ordered]@{ name = $Name; arguments = $Arguments } })
}

function Get-ToolPayload([object]$Session, [int]$Id) {
	$response = Get-McpResponse -Responses $Session.Responses -Id $Id
	if ($response.result.isError -eq $true) { return [pscustomobject]@{ is_error = $true; text = [string]$response.result.content[0].text } }
	$text = [string]$response.result.content[0].text
	try { return [pscustomobject]@{ is_error = $false; value = ($text | ConvertFrom-Json); text = $text } } catch { return [pscustomobject]@{ is_error = $false; value = $null; text = $text } }
}

function Invoke-FixtureSession([string[]]$Calls, [string]$FixtureRoot, [string]$StateRoot) {
	$requests = [System.Collections.Generic.List[string]]::new()
	$requests.Add((ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; id = 1; method = 'initialize'; params = [ordered]@{ protocolVersion = '2024-11-05'; capabilities = [ordered]@{}; clientInfo = [ordered]@{ name = 'meridian-provenance-integrity'; version = '1.0' } } })))
	$requests.Add((ConvertTo-McpJsonLine ([ordered]@{ jsonrpc = '2.0'; method = 'notifications/initialized'; params = [ordered]@{} })))
	foreach ($call in $Calls) { $requests.Add($call) }
	$environment = @{
		MERIDIAN_MCP_MODE = 'development'
		MERIDIAN_MCP_ROOTS = [string]::Join([IO.Path]::PathSeparator, @($mcpRoot, $FixtureRoot))
		MERIDIAN_MCP_COMPILERS = $dreamMaker
		MERIDIAN_MCP_STATE_DIR = $StateRoot
	}
	Invoke-McpSession -BinaryPath $binary -WorkingDirectory $mcpRoot -Environment $environment -Requests $requests.ToArray() -TimeoutMilliseconds 900000
}

$runId = [Guid]::NewGuid().ToString('N')
$temporaryRoot = Join-Path ([IO.Path]::GetTempPath()) "meridian-provenance-$runId"
$fixtureRoot = Join-Path $temporaryRoot 'fixture'
$stateRoot = Join-Path $temporaryRoot 'private-state'
$evidence = [ordered]@{
	schema_version = 1
	overall = 'failed'
	fixture_id = 'owned-provenance-fixture'
	byond_version = $ExpectedByondVersion
	mcp_build_id = $null
	steps = [System.Collections.Generic.List[object]]::new()
	state_journal_finalized = $false
	owned_processes_remaining = 0
}

try {
	New-Item -ItemType Directory -Force -Path $fixtureRoot, $stateRoot, (Split-Path -Parent $evidenceFile) | Out-Null
	Get-ChildItem -LiteralPath (Join-Path $mcpRoot 'tests/fixtures/provenance') | Copy-Item -Destination $fixtureRoot -Recurse
	& git -C $fixtureRoot init --quiet
	& git -C $fixtureRoot config core.autocrlf false
	& git -C $fixtureRoot config user.email fixture@example.invalid
	& git -C $fixtureRoot config user.name 'Owned Fixture'
	& git -C $fixtureRoot add --all
	& git -C $fixtureRoot commit --quiet -m fixture
	if ($LASTEXITCODE -ne 0) { throw 'Could not initialize the owned Git fixture.' }
	$originalBindings = [IO.File]::ReadAllBytes((Join-Path $fixtureRoot 'generated_bindings.dm'))
	$dme = Join-Path $fixtureRoot 'fixture.dme'
	$dmb = Join-Path $fixtureRoot 'fixture.dmb'
	$manifest = Join-Path $fixtureRoot 'fixture-manifest.json'

	$initial = Invoke-FixtureSession @(
		(New-Call 2 'dm_server_status' @{}),
		(New-Call 3 'dm_parse_environment' @{ dme_path = $dme }),
		(New-Call 4 'dm_check_fixture_sync' @{ fixture_manifest_path = $manifest }),
		(New-Call 5 'dm_compile' @{ dme_path = $dme; compiler_path = $dreamMaker; fixture_manifest_path = $manifest }),
		(New-Call 6 'dm_run' @{ dmb_path = $dmb; require_verified_provenance = $true }),
		(New-Call 7 'dm_wait_for_output' @{ pattern = 'MERIDIAN_INTEGRITY_PHASE_COMPLETE'; timeout_ms = 60000 }),
		(New-Call 8 'dm_stop' @{})
	) $fixtureRoot $stateRoot
	foreach ($id in 2..8) {
		$payload = Get-ToolPayload $initial $id
		if ($payload.is_error) { throw "Initial fixture request $id failed: $($payload.text)" }
	}
	$status = (Get-ToolPayload $initial 2).value
	$compile = (Get-ToolPayload $initial 5).value
	$stop = (Get-ToolPayload $initial 8).value
	$evidence.mcp_build_id = $status.mcp_build.build_id
	$initialText = $initial.Responses | ConvertTo-Json -Depth 30 -Compress
	if ($initialText -notmatch 'source_integrity_warning') { throw 'Runtime did not report source_integrity_warning.' }
	$evidence.steps.Add([ordered]@{ id = 'fresh_compile_launch'; classification = 'passed'; dmb_updated = [bool]$compile.dmb_updated; build_record_id = $compile.build_record_id; process_stopped = [bool]$stop.process_stopped; warning_code = 'source_integrity_warning' })
	$evidence.state_journal_finalized = ([string]$stop.integrity.status).StartsWith('finalized_')

	[IO.File]::WriteAllText((Join-Path $fixtureRoot 'generated_bindings.dm'), "this is not valid DreamMaker source`n", [Text.UTF8Encoding]::new($false))
	$stale = Invoke-FixtureSession @(
		(New-Call 2 'dm_parse_environment' @{ dme_path = $dme }),
		(New-Call 3 'dm_check_fixture_sync' @{ fixture_manifest_path = $manifest }),
		(New-Call 4 'dm_run' @{ dmb_path = $dmb; require_verified_provenance = $true }),
		(New-Call 5 'dm_compile' @{ dme_path = $dme; compiler_path = $dreamMaker; fixture_manifest_path = $manifest })
	) $fixtureRoot $stateRoot
	$staleText = $stale.Responses | ConvertTo-Json -Depth 30 -Compress
	if ($staleText -notmatch 'stale_build_artifact') { throw 'Changed input did not produce stale_build_artifact.' }
	if ($staleText -notmatch 'dmb_updated') { throw 'Failed compile did not report dmb_updated.' }
	$evidence.steps.Add([ordered]@{ id = 'changed_input'; classification = 'stale_build_artifact'; dmb_updated = $false })

	$restart = Invoke-FixtureSession @((New-Call 2 'dm_run' @{ dmb_path = $dmb; require_verified_provenance = $true })) $fixtureRoot $stateRoot
	if (($restart.Responses | ConvertTo-Json -Depth 20 -Compress) -notmatch 'stale_build_artifact') { throw 'Restart did not retain stale build rejection.' }
	$evidence.steps.Add([ordered]@{ id = 'restart_stale_retention'; classification = 'stale_build_artifact' })

	[IO.File]::WriteAllBytes((Join-Path $fixtureRoot 'generated_bindings.dm'), $originalBindings)
	[IO.File]::WriteAllText((Join-Path $fixtureRoot 'tracked-runtime-artifact.txt'), "unchanged before runtime`n", [Text.UTF8Encoding]::new($false))
	$restored = Invoke-FixtureSession @(
		(New-Call 2 'dm_parse_environment' @{ dme_path = $dme }),
		(New-Call 3 'dm_check_fixture_sync' @{ fixture_manifest_path = $manifest }),
		(New-Call 4 'dm_compile' @{ dme_path = $dme; compiler_path = $dreamMaker; fixture_manifest_path = $manifest }),
		(New-Call 5 'dm_run' @{ dmb_path = $dmb; require_verified_provenance = $true }),
		(New-Call 6 'dm_wait_for_output' @{ pattern = 'MERIDIAN_INTEGRITY_PHASE_COMPLETE'; timeout_ms = 60000 }),
		(New-Call 7 'dm_stop' @{})
	) $fixtureRoot $stateRoot
	foreach ($id in 2..7) {
		$payload = Get-ToolPayload $restored $id
		if ($payload.is_error) { throw "Restored fixture request $id failed: $($payload.text)" }
	}
	$evidence.steps.Add([ordered]@{ id = 'restored_compile_launch'; classification = 'passed'; process_stopped = $true })
	$restoredStop = (Get-ToolPayload $restored 7).value
	$evidence.state_journal_finalized = $evidence.state_journal_finalized -and ([string]$restoredStop.integrity.status).StartsWith('finalized_')
	$evidence.overall = 'passed'
} catch {
	$evidence.steps.Add([ordered]@{ id = 'failure'; classification = 'failed'; code = $_.Exception.GetType().Name })
	throw
} finally {
	[IO.File]::WriteAllText($evidenceFile, (($evidence | ConvertTo-Json -Depth 12) + [Environment]::NewLine), [Text.UTF8Encoding]::new($false))
	& (Join-Path $PSScriptRoot 'test-provenance-evidence-validation.ps1') -EvidencePath $evidenceFile
	if (Test-Path -LiteralPath $temporaryRoot) { Remove-Item -LiteralPath $temporaryRoot -Recurse -Force }
}
