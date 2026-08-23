# Testing Meridian-MCP

Run checks from the repository root with PowerShell on Windows.

## Rust and contract gates

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
cargo deny check
```

The suite covers owned DreamMaker and DMM fixtures, parse generations, exact lookup and source excerpts, ranked search, map coordinates and PNG output, path containment, executable allowlisting, overwrite policy, mode inventories, runtime buffering, readiness, `Topic()` framing, generated contract drift, and documentation links.

## Installed stdio gate

```powershell
.\test_mcp.ps1 -SkipBuild -ServerPath .\target\release\meridian-mcp.exe -Mode development
.\test_mcp.ps1 -SkipBuild -ServerPath .\target\release\meridian-mcp.exe -Mode analysis `
    -DmePath .\tests\fixtures\language\fixture.dme `
    -SearchQuery "return supplied value"
```

The harness sets immutable roots for every supplied fixture, negotiates through the official SDK, validates JSON-only stdout, checks the exact mode inventory and schemas, and exercises caller-visible error paths. `-ServerPath` and `-BinaryPath` are aliases.

## BYOND fixture gate

```powershell
.\scripts\run-byond-integration.ps1
```

This compiles the purpose-written runtime fixture with DreamMaker. To exercise the runtime, `Topic()`, map output, and clean shutdown through the installed binary:

```powershell
.\test_mcp.ps1 -SkipBuild -Mode development `
    -DmePath .\tests\fixtures\language\fixture.dme `
    -RuntimeDmbPath .\tests\fixtures\runtime\runtime.dmb `
    -RuntimeReadyMarker MERIDIAN_MCP_READY `
    -RuntimeTopic ping -ExpectedTopicResponse pong `
    -MapDmmPath .\tests\fixtures\maps\fixture.dmm
```

## Meridian-Rift full corpus

```powershell
.\test_mcp.ps1 -SkipBuild -Mode analysis `
    -DmePath C:\path\to\Meridian-Rift\tgstation.dme `
    -SearchQuery "storage navigation exit button" `
    -TimeoutSeconds 300
```

Parsing/search is MCP evidence. Run Meridian-Rift's own PowerShell and `BUILD.cmd` gates before claiming game-code completion. A focused fixture or query is iteration evidence, not the full acceptance matrix.
