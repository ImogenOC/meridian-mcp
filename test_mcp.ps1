[CmdletBinding()]
param(
    [ValidateSet("debug", "release")]
    [string]$Configuration = "release",
    [string]$BinaryPath,
    [string]$DmePath,
    [string]$CompileDmePath,
    [switch]$ExpectCompileFailure,
    [string]$TypePath,
    [string]$ProcName,
    [ValidateRange(1, 3600)]
    [int]$TimeoutSeconds = 30,
    [switch]$SkipBuild
)

$ErrorActionPreference = "Stop"
$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path

if (-not $BinaryPath) {
    $binaryName = if ($IsWindows -or $env:OS -eq "Windows_NT") { "meridian-mcp.exe" } else { "meridian-mcp" }
    $binaryDirectory = Join-Path $repoRoot (Join-Path "target" $Configuration)
    $BinaryPath = Join-Path $binaryDirectory $binaryName
}

if (-not $SkipBuild) {
    $cargoArguments = @("build")
    if ($Configuration -eq "release") {
        $cargoArguments += "--release"
    }

    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }
}

if (-not (Test-Path -LiteralPath $BinaryPath)) {
    throw "meridian-mcp binary not found: $BinaryPath"
}

function ConvertTo-JsonRpcLine {
    param([hashtable]$Request)

    return ($Request | ConvertTo-Json -Compress -Depth 20)
}

function Invoke-McpSession {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Requests,
        [Parameter(Mandatory = $true)]
        [int]$TimeoutMilliseconds
    )

    $startInfo = [System.Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = (Resolve-Path -LiteralPath $BinaryPath).Path
    $startInfo.WorkingDirectory = $repoRoot
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true

    $process = [System.Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "Failed to start meridian-mcp: $BinaryPath"
    }

    try {
        # Start both readers before sending requests so a noisy server cannot fill either OS pipe
        # while this harness is still writing stdin.
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        foreach ($request in $Requests) {
            $process.StandardInput.WriteLine($request)
        }
        $process.StandardInput.Close()

        if (-not $process.WaitForExit($TimeoutMilliseconds)) {
            if (-not $process.HasExited) {
                $process.Kill()
            }
            $process.WaitForExit()
            throw "meridian-mcp did not exit within $TimeoutMilliseconds ms"
        }

        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        $responses = @()
        foreach ($line in ($stdout -split "`r?`n" | Where-Object { $_.Trim().Length -gt 0 })) {
            try {
                $responses += $line | ConvertFrom-Json
            } catch {
                throw "meridian-mcp emitted invalid JSON-RPC output: $line`n(stderr: $stderr)"
            }
        }

        [pscustomobject]@{
            Responses = $responses
            Stdout = $stdout
            Stderr = $stderr
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

function Assert-Response {
    param(
        [Parameter(Mandatory = $true)]
        [object[]]$Responses,
        [Parameter(Mandatory = $true)]
        [int]$Id
    )

    $response = @($Responses | Where-Object { $_.id -eq $Id })
    if ($response.Count -ne 1) {
        throw "Expected exactly one JSON-RPC response with id $Id, received $($response.Count)"
    }
    if ($null -ne $response[0].error) {
        throw "JSON-RPC request $Id failed: $($response[0].error.message)"
    }
    return $response[0]
}

$initialize = ConvertTo-JsonRpcLine ([ordered]@{
    jsonrpc = "2.0"
    id = 1
    method = "initialize"
    params = [ordered]@{
        protocolVersion = "2024-11-05"
        capabilities = [ordered]@{}
        clientInfo = [ordered]@{ name = "meridian-mcp-smoke-test"; version = "1.0" }
    }
})
$list = ConvertTo-JsonRpcLine ([ordered]@{
    jsonrpc = "2.0"
    id = 2
    method = "tools/list"
    params = [ordered]@{}
})
$status = ConvertTo-JsonRpcLine ([ordered]@{
    jsonrpc = "2.0"
    id = 6
    method = "tools/call"
    params = [ordered]@{
        name = "dm_status"
        arguments = [ordered]@{}
    }
})
$waitWithoutProcess = ConvertTo-JsonRpcLine ([ordered]@{
    jsonrpc = "2.0"
    id = 7
    method = "tools/call"
    params = [ordered]@{
        name = "dm_wait_for_output"
        arguments = [ordered]@{ pattern = "this process does not exist"; timeout_ms = 10 }
    }
})

$requests = @($initialize, $list, $status, $waitWithoutProcess)
if ($DmePath) {
    if (-not (Test-Path -LiteralPath $DmePath -PathType Leaf)) {
        throw "DME file not found: $DmePath"
    }

    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 3
        method = "tools/call"
        params = [ordered]@{
            name = "dm_parse_environment"
            arguments = [ordered]@{ dme_path = (Resolve-Path -LiteralPath $DmePath).Path }
        }
    })
}

if ($CompileDmePath) {
    if (-not (Test-Path -LiteralPath $CompileDmePath -PathType Leaf)) {
        throw "Compile DME file not found: $CompileDmePath"
    }

    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 8
        method = "tools/call"
        params = [ordered]@{
            name = "dm_compile"
            arguments = [ordered]@{ dme_path = (Resolve-Path -LiteralPath $CompileDmePath).Path }
        }
    })
}

if ($TypePath) {
    if (-not $DmePath) {
        throw '-TypePath requires -DmePath'
    }
    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 4
        method = "tools/call"
        params = [ordered]@{
            name = "dm_get_type"
            arguments = [ordered]@{ type_path = $TypePath }
        }
    })
    if ($ProcName) {
        $requests += ConvertTo-JsonRpcLine ([ordered]@{
            jsonrpc = "2.0"
            id = 5
            method = "tools/call"
            params = [ordered]@{
                name = "dm_get_proc"
                arguments = [ordered]@{ type_path = $TypePath; proc_name = $ProcName }
            }
        })
    }
}

$session = Invoke-McpSession -Requests $requests -TimeoutMilliseconds ($TimeoutSeconds * 1000)
$sessionExitCode = $session.ExitCode
if ($sessionExitCode -ne 0) {
    throw "meridian-mcp exited with code $sessionExitCode`n(stderr: $($session.Stderr))"
}
$initializeResponse = Assert-Response -Responses $session.Responses -Id 1
$toolsResponse = Assert-Response -Responses $session.Responses -Id 2
$statusResponse = Assert-Response -Responses $session.Responses -Id 6
$waitResponse = Assert-Response -Responses $session.Responses -Id 7

if ($initializeResponse.result.protocolVersion -ne "2024-11-05") {
    throw "Unexpected MCP protocol version: $($initializeResponse.result.protocolVersion)"
}
if ($initializeResponse.result.serverInfo.name -ne "meridian-mcp") {
    throw "Unexpected MCP server name: $($initializeResponse.result.serverInfo.name)"
}

$tools = @($toolsResponse.result.tools)
if ($tools.Count -eq 0) {
    throw "tools/list returned no tools"
}
if ($statusResponse.result.isError -eq $true) {
    throw "dm_status returned an MCP tool error in a fresh session: $($statusResponse.result.content[0].text)"
}
if ($waitResponse.result.isError -ne $true) {
    throw "dm_wait_for_output did not report the expected no-process tool error"
}

$toolNames = @($tools | ForEach-Object { $_.name })
$requiredTools = @("dm_parse_environment", "dm_compile", "dm_run", "dm_wait_for_output")
foreach ($requiredTool in $requiredTools) {
    if ($toolNames -notcontains $requiredTool) {
        throw "tools/list is missing required tool: $requiredTool"
    }
}
$compileTool = @($tools | Where-Object { $_.name -eq "dm_compile" })
$compileProperties = @($compileTool[0].inputSchema.properties.PSObject.Properties.Name)
foreach ($compileProperty in @("compiler_path", "working_directory", "defines", "timeout_ms", "idle_timeout_ms")) {
    if ($compileProperties -notcontains $compileProperty) {
        throw "dm_compile schema is missing implemented property: $compileProperty"
    }
}
$runTool = @($tools | Where-Object { $_.name -eq "dm_run" })
$runProperties = @($runTool[0].inputSchema.properties.PSObject.Properties.Name)
foreach ($runProperty in @("working_directory", "daemon_args", "wait_for", "startup_timeout_ms")) {
    if ($runProperties -notcontains $runProperty) {
        throw "dm_run schema is missing implemented property: $runProperty"
    }
}

if ($DmePath) {
    $parseResponse = Assert-Response -Responses $session.Responses -Id 3
    if ($parseResponse.result.isError -eq $true) {
        $message = $parseResponse.result.content[0].text
        throw "dm_parse_environment returned an MCP tool error: $message"
    }
}

if ($TypePath) {
    $typeResponse = Assert-Response -Responses $session.Responses -Id 4
    if ($typeResponse.result.isError -eq $true) {
        throw "dm_get_type returned an MCP tool error: $($typeResponse.result.content[0].text)"
    }
    $typePayload = $typeResponse.result.content[0].text | ConvertFrom-Json
    if ($typePayload.path -ne $TypePath) {
        throw "dm_get_type returned an unexpected type: $($typePayload.path)"
    }
}
if ($ProcName) {
    $procResponse = Assert-Response -Responses $session.Responses -Id 5
    if ($procResponse.result.isError -eq $true) {
        throw "dm_get_proc returned an MCP tool error: $($procResponse.result.content[0].text)"
    }
    $procPayload = $procResponse.result.content[0].text | ConvertFrom-Json
    if (-not $procPayload.overrides) {
        throw "dm_get_proc returned no overrides for $TypePath/$ProcName"
    }
    if (-not (@($procPayload.overrides | Where-Object { $_.source }).Count -gt 0)) {
        throw "dm_get_proc returned no source excerpts for $TypePath/$ProcName"
    }
}

if ($CompileDmePath) {
    $compileResponse = Assert-Response -Responses $session.Responses -Id 8
    $compileFailed = $compileResponse.result.isError -eq $true
    if ($ExpectCompileFailure -and -not $compileFailed) {
        throw "dm_compile accepted a compile that was expected to contain errors"
    }
    if ($ExpectCompileFailure) {
        $compilePayload = $compileResponse.result.content[0].text | ConvertFrom-Json
        $hasStructuredDiagnostics = @($compilePayload.errors).Count -gt 0
        $hasBoundedWatchdogFailure = $compilePayload.idle -eq $true -or $compilePayload.timed_out -eq $true
        if (-not $hasStructuredDiagnostics -and -not $hasBoundedWatchdogFailure) {
            throw "dm_compile failed without structured diagnostics or a bounded watchdog classification"
        }
    }
    if (-not $ExpectCompileFailure -and $compileFailed) {
        throw "dm_compile returned an MCP tool error: $($compileResponse.result.content[0].text)"
    }
}

Write-Output ("MCP smoke test passed: protocol 2024-11-05, {0} tools, exit code {1}" -f $tools.Count, $sessionExitCode)
if ($DmePath) {
    Write-Output "DME parse smoke test passed: $DmePath"
}
if ($TypePath) {
    Write-Output "Symbol smoke test passed: $TypePath"
}
if ($ProcName) {
    Write-Output "Proc/source smoke test passed: $TypePath/$ProcName"
}

if ($CompileDmePath) {
    $compileResult = if ($ExpectCompileFailure) { "expected compile rejection" } else { "clean" }
    Write-Output "Compile smoke test passed: $CompileDmePath ($compileResult)"
}
