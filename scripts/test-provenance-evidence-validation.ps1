[CmdletBinding()]
param([string]$EvidencePath)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

function Test-ProvenanceEvidence([object]$Document) {
	if ($Document.schema_version -ne 1) { throw 'Provenance evidence schema_version must be 1.' }
	$json = $Document | ConvertTo-Json -Depth 20 -Compress
	foreach ($pattern in @('[A-Za-z]:\\\\Users\\\\', '/home/[^/]+/', '/Users/[^/]+/', '"(?:password|secret|credential|token|player|ckey|account|raw_stdout|raw_stderr)"\s*:')) {
		if ($json -match $pattern) { throw "Provenance evidence contains forbidden private data matching $pattern" }
	}
	if ($json.Length -gt 262144) { throw 'Provenance evidence exceeds the fixed 256 KiB limit.' }
}

if ($EvidencePath) {
	$document = Get-Content -LiteralPath $EvidencePath -Raw | ConvertFrom-Json
	Test-ProvenanceEvidence $document
}

$valid = [pscustomobject]@{ schema_version = 1; overall = 'passed'; fixture_id = 'owned-provenance-fixture'; steps = @() }
Test-ProvenanceEvidence $valid
foreach ($invalid in @(
	[pscustomobject]@{ schema_version = 2; overall = 'passed' },
	[pscustomobject]@{ schema_version = 1; path = 'C:\Users\Example\private' },
	[pscustomobject]@{ schema_version = 1; player = 'private-user' }
)) {
	$rejected = $false
	try { Test-ProvenanceEvidence $invalid } catch { $rejected = $true }
	if (-not $rejected) { throw 'A malicious provenance evidence fixture was accepted.' }
}

Write-Output 'Provenance evidence validation passed.'
