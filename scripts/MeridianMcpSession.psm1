Set-StrictMode -Version 2.0

function ConvertTo-McpJsonLine {
	param([Parameter(Mandatory)][System.Collections.IDictionary]$Request)

	return ($Request | ConvertTo-Json -Compress -Depth 20)
}

function Invoke-McpSession {
	[CmdletBinding()]
	param(
		[Parameter(Mandatory)][string]$BinaryPath,
		[Parameter(Mandatory)][string]$WorkingDirectory,
		[Parameter(Mandatory)][System.Collections.IDictionary]$Environment,
		[Parameter(Mandatory)][string[]]$Requests,
		[Parameter(Mandatory)][int]$TimeoutMilliseconds,
		[scriptblock]$AfterResponse
	)

	$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
	$startInfo.FileName = (Resolve-Path -LiteralPath $BinaryPath).Path
	$startInfo.WorkingDirectory = (Resolve-Path -LiteralPath $WorkingDirectory).Path
	$startInfo.UseShellExecute = $false
	$startInfo.CreateNoWindow = $true
	$startInfo.RedirectStandardInput = $true
	$startInfo.RedirectStandardOutput = $true
	$startInfo.RedirectStandardError = $true
	foreach ($entry in $Environment.GetEnumerator()) {
		$startInfo.Environment[[string]$entry.Key] = [string]$entry.Value
	}

	$process = [System.Diagnostics.Process]::new()
	$process.StartInfo = $startInfo
	if (-not $process.Start()) {
		throw "Failed to start meridian-mcp: $BinaryPath"
	}

	try {
		$stderrTask = $process.StandardError.ReadToEndAsync()
		$responses = @()
		$stdoutLines = @()
		$responseTimings = [ordered]@{}
		foreach ($request in $Requests) {
			$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
			$process.StandardInput.WriteLine($request)
			$process.StandardInput.Flush()
			$requestObject = $request | ConvertFrom-Json
			$idProperty = $requestObject.PSObject.Properties['id']
			if ($null -eq $idProperty) {
				continue
			}
			$lineTask = $process.StandardOutput.ReadLineAsync()
			if (-not $lineTask.Wait($TimeoutMilliseconds)) {
				throw "meridian-mcp did not respond to request $($idProperty.Value) within $TimeoutMilliseconds ms"
			}
			$line = $lineTask.GetAwaiter().GetResult()
			if ($null -eq $line) {
				throw "meridian-mcp closed stdout before responding to request $($idProperty.Value)"
			}
			$stdoutLines += $line
			try {
				$response = $line | ConvertFrom-Json
				$responses += $response
			} catch {
				throw "meridian-mcp emitted invalid JSON-RPC output: $line"
			}
			$stopwatch.Stop()
			$responseTimings[[string]$idProperty.Value] = $stopwatch.ElapsedMilliseconds
			if ($AfterResponse) {
				& $AfterResponse $requestObject $response
			}
		}
		$process.StandardInput.Close()

		if (-not $process.WaitForExit($TimeoutMilliseconds)) {
			if (-not $process.HasExited) {
				$process.Kill()
			}
			$process.WaitForExit()
			throw "meridian-mcp did not exit within $TimeoutMilliseconds ms"
		}

		[pscustomobject]@{
			Responses = $responses
			ResponseTimingsMilliseconds = $responseTimings
			Stdout = [string]::Join([Environment]::NewLine, $stdoutLines)
			Stderr = $stderrTask.GetAwaiter().GetResult()
			ExitCode = $process.ExitCode
		}
	} finally {
		if (-not $process.HasExited) {
			$process.Kill()
			$process.WaitForExit()
		}
		$process.Dispose()
	}
}

function Get-McpResponse {
	param(
		[Parameter(Mandatory)][object[]]$Responses,
		[Parameter(Mandatory)][int]$Id
	)

	$response = @($Responses | Where-Object { $_.id -eq $Id })
	if ($response.Count -ne 1) {
		throw "Expected exactly one JSON-RPC response with id $Id, received $($response.Count)"
	}
	$errorProperty = $response[0].PSObject.Properties['error']
	if ($null -ne $errorProperty -and $null -ne $errorProperty.Value) {
		throw "JSON-RPC request $Id failed: $($errorProperty.Value.message)"
	}
	return $response[0]
}

Export-ModuleMember -Function ConvertTo-McpJsonLine, Invoke-McpSession, Get-McpResponse
