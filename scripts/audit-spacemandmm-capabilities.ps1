[CmdletBinding()]
param(
	[switch]$Check,
	[string]$UpstreamPath
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'
$expectedRevision = '351ddc0ffb2439876d4565ce5130bb6b027ee605'
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$registryPath = Join-Path $repoRoot 'spacemandmm-capabilities.json'
$registry = Get-Content -LiteralPath $registryPath -Raw | ConvertFrom-Json
$errors = [System.Collections.Generic.List[string]]::new()

function Add-AuditError {
	param([Parameter(Mandatory)][string]$Message)
	$errors.Add($Message)
}

if ($registry.schema_version -ne 1) {
	Add-AuditError "Unsupported registry schema $($registry.schema_version)."
}
if ($registry.spacemandmm_revision -ne $expectedRevision) {
	Add-AuditError "Capability registry revision $($registry.spacemandmm_revision) does not match $expectedRevision."
}

$identities = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
foreach ($record in $registry.capabilities) {
	if (-not $identities.Add([string]$record.id)) {
		Add-AuditError "Duplicate capability id $($record.id)."
	}
	if ([string]::IsNullOrWhiteSpace([string]$record.verification)) {
		Add-AuditError "$($record.id) has no verification gate."
	}
	if ($record.disposition -eq 'excluded' -and [string]::IsNullOrWhiteSpace([string]$record.rationale)) {
		Add-AuditError "$($record.id) has no exclusion rationale."
	}
}

$evidence = [System.Collections.Generic.HashSet[string]]::new([System.StringComparer]::Ordinal)
foreach ($record in $registry.capabilities) {
	foreach ($item in $record.evidence) {
		[void]$evidence.Add([string]$item)
	}
}

if ($UpstreamPath) {
	$resolvedUpstream = (Resolve-Path -LiteralPath $UpstreamPath).Path
	$safeDirectory = $resolvedUpstream.Replace('\', '/')
	$actualRevision = (& git -c "safe.directory=$safeDirectory" -C $resolvedUpstream rev-parse HEAD)
	if ($LASTEXITCODE -ne 0) {
		throw "Unable to read the upstream revision at $resolvedUpstream."
	}
	$actualRevision = $actualRevision.Trim()
	if ($actualRevision -ne $expectedRevision) {
		Add-AuditError "Upstream checkout is $actualRevision, expected $expectedRevision."
	}

	$workspaceManifest = Get-Content -LiteralPath (Join-Path $resolvedUpstream 'Cargo.toml')
	$workspaceMembers = foreach ($line in $workspaceManifest) {
		if ($line.TrimStart().StartsWith('#')) {
			continue
		}
		if ($line -match '"crates/([^\"]+)"') {
			$Matches[1]
		}
	}
	foreach ($member in $workspaceMembers) {
		$token = "workspace:$member"
		if (-not $evidence.Contains($token)) {
			Add-AuditError "Unmapped active workspace member $member; expected evidence $token."
		}
	}

	$languageServer = Get-Content -LiteralPath (Join-Path $resolvedUpstream 'crates\dm-langserver\src\main.rs') -Raw
	$languageCapabilities = [ordered]@{
		'definition_provider' = 'language:definition'
		'workspace_symbol_provider' = 'language:workspace_symbol'
		'document_symbol_provider' = 'language:document_symbol'
		'references_provider' = 'language:references'
		'implementation_provider' = 'language:implementation'
		'type_definition_provider' = 'language:type_definition'
		'document_link_provider' = 'language:document_link'
		'color_provider' = 'language:document_color'
		'folding_range_provider' = 'language:folding_range'
	}
	foreach ($capability in $languageCapabilities.GetEnumerator()) {
		if ($languageServer -match [regex]::Escape($capability.Key) -and -not $evidence.Contains($capability.Value)) {
			Add-AuditError "Unmapped language capability $($capability.Key); expected evidence $($capability.Value)."
		}
	}

	$dmmCli = Get-Content -LiteralPath (Join-Path $resolvedUpstream 'crates\dmm-tools-cli\src\main.rs') -Raw
	foreach ($command in @('list-passes', 'minimap', 'diff-maps', 'map-info', 'RenderMany')) {
		if ($dmmCli -notmatch [regex]::Escape($command)) {
			Add-AuditError "Expected DMM CLI command $command is absent from the pinned source."
			continue
		}
		$token = "dmm-cli:$command"
		if (-not $evidence.Contains($token)) {
			Add-AuditError "Unmapped DMM CLI command $command; expected evidence $token."
		}
	}

	$auxtoolsTypes = Get-Content -LiteralPath (Join-Path $resolvedUpstream 'crates\dm-langserver\src\debugger\auxtools_types.rs') -Raw
	foreach ($request in @(
		'Disconnect', 'Configured', 'StdDef', 'Eval', 'CurrentInstruction', 'BreakpointSet',
		'BreakpointUnset', 'CatchRuntimes', 'LineNumber', 'Offset', 'Stacks', 'StackFrames',
		'Scopes', 'Variables', 'Continue', 'Pause'
	)) {
		if ($auxtoolsTypes -notmatch "(?m)^\s{4}$request(?:\s*\{|,)") {
			Add-AuditError "Expected auxtools request $request is absent from the pinned source."
			continue
		}
		$token = "debugger-request:$request"
		if (-not $evidence.Contains($token)) {
			Add-AuditError "Unmapped auxtools request $request; expected evidence $token."
		}
	}
}

if ($errors.Count -gt 0) {
	throw "SpacemanDMM capability audit failed:`n$($errors -join "`n")"
}

if ($Check) {
	Write-Output "Capability audit passed for $($registry.capabilities.Count) records at $expectedRevision."
}
