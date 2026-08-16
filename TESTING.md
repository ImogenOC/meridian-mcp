# meridian-mcp testing

Run these commands from the meridian-mcp checkout. The scripts resolve the binary from the invoking
repository's `target/<configuration>` directory by default, or from an explicit caller-supplied
binary path, so they do not depend on a developer-specific path.

## Rust checks

```powershell
cargo test
cargo build --release
```

`cargo test` covers the bounded DreamDaemon output log, literal and regular-expression matching,
source excerpt boundaries, source excerpt limits, and compiler diagnostic parsing. The release
build is the binary used by the protocol smoke tests.

## MCP protocol smoke test

```powershell
pwsh -NoProfile -File .\test_mcp.ps1
pwsh -NoProfile -File .\test_mcp.ps1 -SkipBuild
```

The PowerShell smoke test starts a fresh stdio session, drains both output streams before sending
requests, validates JSON-RPC response IDs and exit status, discovers the tool list dynamically,
checks required tools, asserts `serverInfo.name == "meridian-mcp"`, verifies that the advertised
`dm_compile` options match its implementation, and keeps the `dm_*` tool names as an intentional
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
    -ProcName AfterChange
```

The shell wrapper provides the same protocol check on Unix-like systems and delegates to the
PowerShell implementation on Windows:

```bash
./test-mcp.sh --skip-build
./test-mcp.sh --skip-build --binary ./target/release/meridian-mcp --dme ../path/to/project.dme
```

The shell path requires `jq` and `timeout` on Unix-like systems. On Windows it requires PowerShell
and uses the PowerShell assertions, avoiding a second implementation of the protocol harness.

## Runtime diagnostics

`dm_run` continuously drains DreamDaemon stdout and stderr into a bounded 500-line ring buffer.
The drain reads fixed 8 KiB chunks, truncates any retained line beyond 16 KiB with a
`... [truncated]` suffix, and evicts old lines when retained line bytes exceed 1 MiB. Use `wait_for`
plus `startup_timeout_ms` when starting a server, or call `dm_wait_for_output` with a literal marker
or regular expression afterward. Output remains searchable after the process exits, which makes
post-crash markers and exit codes available through `dm_status` and the wait result.

## Meridian/Dogmos integration checks

The generic MCP does not encode Dogmos policy. From the Meridian checkout, use its own harness:

```powershell
pwsh -NoProfile -File .\tools\dogmos\test_compile_check.ps1
pwsh -NoProfile -File .\tools\dogmos\run_tests.ps1 -Focus /datum/unit_test/example
pwsh -NoProfile -File .\tools\dogmos\run_tests.ps1
```

The focused run is an iteration aid, not a replacement for the full suite. The compile check is
also bounded: a hung DreamMaker process fails with a timeout instead of blocking indefinitely.
