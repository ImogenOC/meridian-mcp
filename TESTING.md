# meridian-mcp testing

Run these commands from the meridian-mcp checkout. The scripts resolve the binary from the invoking
repository's `target/<configuration>` directory by default, or from an explicit caller-supplied
binary path, so they do not depend on a developer-specific path.

## Rust checks

```powershell
cargo test
cargo build --release
```

`cargo test` covers the bounded DreamDaemon output log and log-file follower, BYOND Topic framing,
literal and regular-expression matching, exact map parsing/search and PNG output, inspection metadata,
source excerpt boundaries and limits, parser-backed symbol metadata, BM25 relevance and exact-symbol
boosts, search filters and deterministic ordering, MCP search schema/lifecycle behavior, and compiler
diagnostic parsing. The release build is the binary used by the protocol smoke tests.

## MCP protocol smoke test

```powershell
pwsh -NoProfile -File .\test_mcp.ps1
pwsh -NoProfile -File .\test_mcp.ps1 -SkipBuild

# Exercise DreamDaemon readiness, Topic framing, the experimental handshake diagnostic, and stop.
pwsh -NoProfile -File .\test_mcp.ps1 -SkipBuild `
    -RuntimeDmbPath ..\path\to\fixture.dmb `
    -RuntimeReadyMarker "FIXTURE_READY" `
    -RuntimeTopic ping -ExpectedTopicResponse pong

# Exercise parser-backed map statistics, coordinates, and PNG rendering.
pwsh -NoProfile -File .\test_mcp.ps1 -SkipBuild `
    -DmePath ..\path\to\project.dme `
    -MapDmmPath ..\path\to\map.dmm `
    -MapTypePath /obj/machinery/door `
    -MapRenderOutputPath .\map-smoke.png `
    -RequireVisibleMapPixels

# Assert that a DME rejected by DreamMaker is not reported as success. The assertion accepts either
# structured compiler diagnostics or an explicit bounded idle/timeout classification.
pwsh -NoProfile -File .\test_mcp.ps1 -SkipBuild `
    -BinaryPath .\target\release\meridian-mcp.exe `
    -CompileDmePath ..\path\to\project.dme -ExpectCompileFailure -TimeoutSeconds 300
```

The PowerShell smoke test starts a fresh stdio session, drains both output streams before sending
requests, validates JSON-RPC response IDs and exit status, discovers the tool list dynamically,
checks required tools, asserts `serverInfo.name == "meridian-mcp"`, checks the advertised client
workflow instructions, verifies that the `dm_compile` and `dm_search_context` schemas match their
implementations, and keeps the `dm_*` tool names as an intentional
compatibility contract. `-BinaryPath` can point to a separately built binary, and
`-TimeoutSeconds` controls the whole session.

## Source-backed project smoke test

Pass a project path relative to the meridian-mcp checkout or use an absolute path supplied by the local
caller. The source query is optional; when supplied, it verifies that parser locations resolve to
readable source excerpts.

```powershell
pwsh -NoProfile -File .\test_parse.ps1 `
    -DmePath ..\path\to\project.dme `
    -TypePath /turf/open `
    -ProcName AfterChange `
    -SearchQuery "turf air temperature reset"
```

`-SearchQuery` requires `-DmePath`. It checks that parsing produced a non-empty index, ranked search
returns at least one result, and the top result contains score, symbol kind, canonical symbol, file,
and line metadata. `-TypePath` and `-ProcName` remain optional exact-navigation checks.

The shell wrapper provides the same protocol check on Unix-like systems and delegates to the
PowerShell implementation on Windows:

```bash
./test-mcp.sh --skip-build
./test-mcp.sh --skip-build --binary ./target/release/meridian-mcp --dme ../path/to/project.dme
```

The shell path requires `jq` and `timeout` on Unix-like systems. On Windows it requires PowerShell
and uses the PowerShell assertions, avoiding a second implementation of the protocol harness.

## Runtime diagnostics

`dm_run` continuously drains DreamDaemon stdout and stderr and follows newly appended content in the
DMB-adjacent `-logself` file into a bounded 500-line ring buffer. The file follower starts at the
pre-launch file length, so output retained from earlier runs is not reported as current readiness.
It accepts optional `working_directory` and `daemon_args` values, so a relative test DMB can be
run from its game checkout with arguments such as `-close`, `-params`, and `log-directory=ci`.
The daemon is started from the DMB's parent directory, preventing the MCP installation directory
from becoming the game's implicit working directory.
On Windows, the spawn path is converted back from Rust's `\\?\` canonical form before it is passed
to DreamDaemon; DreamDaemon can remain alive but idle when given the verbatim path directly.
The drain reads fixed 8 KiB chunks, truncates any retained line beyond 16 KiB with a
`... [truncated]` suffix, and evicts old lines when retained line bytes exceed 1 MiB. Use `wait_for`
plus `startup_timeout_ms` when starting a server, or call `dm_wait_for_output` with a literal marker
or regular expression afterward. Output remains searchable after the process exits, which makes
post-crash markers and exit codes available through `dm_status` and the wait result.
If a requested `wait_for` marker is not observed, `dm_run` returns an MCP error and stops the child;
it does not report a successful start with an unverified readiness result.

`dm_compile` treats parsed compiler errors as failure even when DreamMaker exits with code 0. BYOND
can save a DMB after reporting resource-cache errors, but that artifact is not a valid boot result.
It also samples DreamMaker CPU time on Windows while draining both output streams. A process that is
silent but CPU-active is allowed to finish; a process with no output and no CPU progress fails after
`idle_timeout_ms` (default 45 seconds), with the partial stdout/stderr included in the error result.
The total `timeout_ms` remains the outer deadline. This prevents a modal or stale DreamMaker launch
from consuming the full compile timeout while preserving long, legitimately quiet compiles.

When replacing a live release binary on Windows, stop the client-owned MCP process before building
because the executable is locked. A client that does not automatically reconnect a deliberately
closed stdio transport must relaunch its configured bridge after the build. Validate the replacement
with `test_mcp.ps1 -SkipBuild` before using it for a game run; this distinguishes a healthy binary
from a stale or disconnected client process.
