$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$scriptPath = Join-Path $PSScriptRoot 'run-meridian-compatibility.ps1'
$scriptSource = Get-Content -Raw -LiteralPath $scriptPath
$tokens = $null
$parseErrors = $null
$syntaxTree = [System.Management.Automation.Language.Parser]::ParseFile(
	$scriptPath,
	[ref]$tokens,
	[ref]$parseErrors
)
if ($parseErrors.Count -ne 0) {
	throw "Compatibility harness has PowerShell parse errors: $($parseErrors[0].Message)"
}
$validator = $syntaxTree.Find({
	param($node)
	return $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -ceq 'Assert-NoSensitiveEvidenceKeys'
}, $true)
if ($null -eq $validator) {
	throw 'Compatibility harness does not define Assert-NoSensitiveEvidenceKeys.'
}
Invoke-Expression $validator.Extent.Text

try {
	Assert-NoSensitiveEvidenceKeys -Value ([ordered]@{
		artifact = [ordered]@{ sha256 = $null; modified_unix_ms = $null }
		warnings = @($null)
	})
} catch {
	throw "Legitimate null evidence was rejected: $($_.Exception.Message)"
}

$forbiddenKeyRejected = $false
try {
	Assert-NoSensitiveEvidenceKeys -Value ([ordered]@{ nested = [ordered]@{ token = $null } })
} catch {
	if ($_.Exception.Message -match 'forbidden key') {
		$forbiddenKeyRejected = $true
	} else {
		throw
	}
}
if (-not $forbiddenKeyRejected) {
	throw 'A forbidden evidence key was accepted.'
}

if ($scriptSource -notmatch [regex]::Escape('$badParse.details.state_preserved') -or $scriptSource -notmatch [regex]::Escape('$badParse.details.state_generation')) {
	throw 'Failed-reparse validation does not read the structured error details object.'
}

Write-Host 'PASS Meridian compatibility evidence validation'
