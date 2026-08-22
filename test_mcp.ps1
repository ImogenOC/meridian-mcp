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
    [string]$SearchQuery,
    [string]$RuntimeDmbPath,
    [string]$RuntimeReadyMarker,
    [string]$RuntimeTopic,
    [string]$ExpectedTopicResponse,
    [string]$MapDmmPath,
    [string]$MapTypePath,
    [string]$MapRenderOutputPath,
    [switch]$RequireVisibleMapPixels,
    [ValidateRange(1, 65535)]
    [int]$RuntimePort = 14567,
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

if ($SearchQuery) {
    if (-not $DmePath) {
        throw '-SearchQuery requires -DmePath'
    }
    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 9
        method = "tools/call"
        params = [ordered]@{
            name = "dm_search_context"
            arguments = [ordered]@{
                query = $SearchQuery
                limit = 5
                include_source = $true
                max_source_lines = 20
            }
        }
    })
}

if ($RuntimeDmbPath) {
    if (-not (Test-Path -LiteralPath $RuntimeDmbPath -PathType Leaf)) {
        throw "Runtime DMB file not found: $RuntimeDmbPath"
    }
    if (-not $RuntimeReadyMarker) {
        throw '-RuntimeDmbPath requires -RuntimeReadyMarker'
    }

    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 10
        method = "tools/call"
        params = [ordered]@{
            name = "dm_run"
            arguments = [ordered]@{
                dmb_path = (Resolve-Path -LiteralPath $RuntimeDmbPath).Path
                port = $RuntimePort
                wait_for = $RuntimeReadyMarker
                startup_timeout_ms = 15000
            }
        }
    })
    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 11
        method = "tools/call"
        params = [ordered]@{ name = "dm_status"; arguments = [ordered]@{} }
    })
    if ($RuntimeTopic) {
        $requests += ConvertTo-JsonRpcLine ([ordered]@{
            jsonrpc = "2.0"
            id = 12
            method = "tools/call"
            params = [ordered]@{
                name = "dm_topic"
                arguments = [ordered]@{ topic = $RuntimeTopic; timeout_ms = 5000 }
            }
        })
    }
    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 13
        method = "tools/call"
        params = [ordered]@{
            name = "dm_connect_test"
            arguments = [ordered]@{ timeout_secs = 1 }
        }
    })
    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 14
        method = "tools/call"
        params = [ordered]@{ name = "dm_stop"; arguments = [ordered]@{} }
    })
    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 15
        method = "tools/call"
        params = [ordered]@{ name = "dm_status"; arguments = [ordered]@{} }
    })
}

if ($MapDmmPath) {
    if (-not (Test-Path -LiteralPath $MapDmmPath -PathType Leaf)) {
        throw "Map DMM file not found: $MapDmmPath"
    }
    $requests += ConvertTo-JsonRpcLine ([ordered]@{
        jsonrpc = "2.0"
        id = 16
        method = "tools/call"
        params = [ordered]@{
            name = "dm_map_info"
            arguments = [ordered]@{ dmm_path = (Resolve-Path -LiteralPath $MapDmmPath).Path }
        }
    })
    if ($MapTypePath) {
        $requests += ConvertTo-JsonRpcLine ([ordered]@{
            jsonrpc = "2.0"
            id = 17
            method = "tools/call"
            params = [ordered]@{
                name = "dm_find_on_map"
                arguments = [ordered]@{
                    dmm_path = (Resolve-Path -LiteralPath $MapDmmPath).Path
                    type_path = $MapTypePath
                }
            }
        })
    }
    if ($MapRenderOutputPath) {
        if (-not $DmePath) {
            throw '-MapRenderOutputPath requires -DmePath for icon and type metadata'
        }
        $requests += ConvertTo-JsonRpcLine ([ordered]@{
            jsonrpc = "2.0"
            id = 18
            method = "tools/call"
            params = [ordered]@{
                name = "dm_render_map"
                arguments = [ordered]@{
                    dmm_path = (Resolve-Path -LiteralPath $MapDmmPath).Path
                    output_path = $MapRenderOutputPath
                    z_level = 1
                }
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
if (-not $initializeResponse.result.instructions.Contains("dm_search_context")) {
    throw "MCP initialization instructions do not describe context search"
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
$requiredTools = @("dm_parse_environment", "dm_search_context", "dm_compile", "dm_run", "dm_wait_for_output")
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
$searchTool = @($tools | Where-Object { $_.name -eq "dm_search_context" })
$searchProperties = @($searchTool[0].inputSchema.properties.PSObject.Properties.Name)
foreach ($searchProperty in @("query", "kind", "type_prefix", "file_filter", "limit", "include_source", "max_source_lines")) {
    if ($searchProperties -notcontains $searchProperty) {
        throw "dm_search_context schema is missing implemented property: $searchProperty"
    }
}
if (@($searchTool[0].inputSchema.required) -notcontains "query") {
    throw "dm_search_context schema does not require query"
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

if ($SearchQuery) {
    $searchResponse = Assert-Response -Responses $session.Responses -Id 9
    if ($searchResponse.result.isError -eq $true) {
        throw "dm_search_context returned an MCP tool error: $($searchResponse.result.content[0].text)"
    }
    $searchPayload = $searchResponse.result.content[0].text | ConvertFrom-Json
    if ($searchPayload.indexed_documents -le 0) {
        throw "dm_search_context reported an empty index"
    }
    if ($searchPayload.count -le 0 -or @($searchPayload.results).Count -le 0) {
        throw "dm_search_context returned no results for: $SearchQuery"
    }
    $firstSearchResult = @($searchPayload.results)[0]
    foreach ($requiredResultProperty in @("score", "kind", "symbol", "file", "line")) {
        if ($firstSearchResult.PSObject.Properties.Name -notcontains $requiredResultProperty) {
            throw "dm_search_context result is missing property: $requiredResultProperty"
        }
    }
}

if ($RuntimeDmbPath) {
    $runResponse = Assert-Response -Responses $session.Responses -Id 10
    if ($runResponse.result.isError) {
        throw "dm_run returned an MCP tool error: $($runResponse.result.content[0].text)"
    }
    $runPayload = $runResponse.result.content[0].text | ConvertFrom-Json
    if (-not $runPayload.success -or -not $runPayload.readiness.matched) {
        throw "dm_run did not verify readiness: $($runResponse.result.content[0].text)"
    }

    $runningResponse = Assert-Response -Responses $session.Responses -Id 11
    $runningPayload = $runningResponse.result.content[0].text | ConvertFrom-Json
    if (-not $runningPayload.running) {
        throw "dm_status did not report the fixture as running"
    }

    if ($RuntimeTopic) {
        $topicResponse = Assert-Response -Responses $session.Responses -Id 12
        if ($topicResponse.result.isError) {
            throw "dm_topic returned an MCP tool error: $($topicResponse.result.content[0].text)"
        }
        $topicPayload = $topicResponse.result.content[0].text | ConvertFrom-Json
        if ($PSBoundParameters.ContainsKey('ExpectedTopicResponse') -and
            $topicPayload.response -ne $ExpectedTopicResponse) {
            throw "dm_topic returned '$($topicPayload.response)', expected '$ExpectedTopicResponse'"
        }
    }

    $connectResponse = Assert-Response -Responses $session.Responses -Id 13
    if ($connectResponse.result.isError) {
        $connectPayload = $connectResponse.result.content[0].text | ConvertFrom-Json
        if (-not $connectPayload.experimental) {
            throw "dm_connect_test failure was not classified as experimental"
        }
    }

    $stopResponse = Assert-Response -Responses $session.Responses -Id 14
    if ($stopResponse.result.isError) {
        throw "dm_stop returned an MCP tool error: $($stopResponse.result.content[0].text)"
    }
    $stoppedResponse = Assert-Response -Responses $session.Responses -Id 15
    $stoppedPayload = $stoppedResponse.result.content[0].text | ConvertFrom-Json
    if ($stoppedPayload.running) {
        throw "dm_status still reported the fixture as running after dm_stop"
    }
}

if ($MapDmmPath) {
    $mapInfoResponse = Assert-Response -Responses $session.Responses -Id 16
    if ($mapInfoResponse.result.isError) {
        throw "dm_map_info returned an MCP tool error: $($mapInfoResponse.result.content[0].text)"
    }
    $mapInfoPayload = $mapInfoResponse.result.content[0].text | ConvertFrom-Json
    if ($mapInfoPayload.dimensions.x -le 0 -or $mapInfoPayload.dimensions.y -le 0 -or
        $mapInfoPayload.dimensions.z -le 0) {
        throw "dm_map_info returned invalid dimensions"
    }

    if ($MapTypePath) {
        $findResponse = Assert-Response -Responses $session.Responses -Id 17
        if ($findResponse.result.isError) {
            throw "dm_find_on_map returned an MCP tool error: $($findResponse.result.content[0].text)"
        }
        $findPayload = $findResponse.result.content[0].text | ConvertFrom-Json
        if ($findPayload.PSObject.Properties.Name -notcontains 'coordinates') {
            throw "dm_find_on_map did not return exact coordinates"
        }
    }

    if ($MapRenderOutputPath) {
        $renderResponse = Assert-Response -Responses $session.Responses -Id 18
        if ($renderResponse.result.isError) {
            throw "dm_render_map returned an MCP tool error: $($renderResponse.result.content[0].text)"
        }
        $renderPayload = $renderResponse.result.content[0].text | ConvertFrom-Json
        if (-not $renderPayload.success -or -not (Test-Path -LiteralPath $MapRenderOutputPath -PathType Leaf)) {
            throw "dm_render_map did not create its PNG output"
        }
        if ($renderPayload.PSObject.Properties.Name -notcontains 'non_transparent_pixels') {
            throw "dm_render_map did not report whether the PNG contains visible pixels"
        }
        if ($RequireVisibleMapPixels -and $renderPayload.non_transparent_pixels -le 0) {
            throw "dm_render_map produced a fully transparent PNG when visible pixels were required"
        }
        [byte[]]$signature = Get-Content -LiteralPath $MapRenderOutputPath -AsByteStream -TotalCount 8
        if (($signature | ForEach-Object { $_.ToString('X2') }) -join '' -ne '89504E470D0A1A0A') {
            throw "dm_render_map output is not a PNG"
        }
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
if ($SearchQuery) {
    Write-Output "Ranked context search smoke test passed: $SearchQuery"
}
if ($RuntimeDmbPath) {
    Write-Output "DreamDaemon runtime smoke test passed: readiness, Topic, handshake classification, and stop"
}
if ($MapDmmPath) {
    Write-Output "Map inspection smoke test passed: $MapDmmPath"
}
if ($MapRenderOutputPath) {
    Write-Output "Map render smoke test passed: $MapRenderOutputPath ($($renderPayload.non_transparent_pixels) visible pixels)"
}

if ($CompileDmePath) {
    $compileResult = if ($ExpectCompileFailure) { "expected compile rejection" } else { "clean" }
    Write-Output "Compile smoke test passed: $CompileDmePath ($compileResult)"
}
