function Wait-ProcessReadiness {
	[CmdletBinding()]
	param(
		[Parameter(Mandatory)][Diagnostics.Process]$Process,
		[Parameter(Mandatory)][string]$MarkerPath,
		[Parameter(Mandatory)][string]$ExpectedMarker,
		[Parameter(Mandatory)][ValidateRange(1, 900)][int]$TimeoutSeconds
	)

	$stopwatch = [Diagnostics.Stopwatch]::StartNew()
	$processExited = $false
	do {
		if (Test-Path -LiteralPath $MarkerPath -PathType Leaf) {
			$marker = Get-Content -Raw -LiteralPath $MarkerPath -ErrorAction SilentlyContinue
			if ($null -ne $marker -and $marker.TrimEnd() -eq $ExpectedMarker) {
				$stopwatch.Stop()
				return [pscustomobject]@{
					status = 'ready'
					elapsed_seconds = $stopwatch.Elapsed.TotalSeconds
				}
			}
		}

		$Process.Refresh()
		$processExited = $processExited -or $Process.HasExited
		Start-Sleep -Milliseconds 100
	} while ($stopwatch.Elapsed.TotalSeconds -lt $TimeoutSeconds)

	$stopwatch.Stop()
	return [pscustomobject]@{
		status = if ($processExited) { 'process_exited' } else { 'timed_out' }
		elapsed_seconds = $stopwatch.Elapsed.TotalSeconds
	}
}

Export-ModuleMember -Function Wait-ProcessReadiness
