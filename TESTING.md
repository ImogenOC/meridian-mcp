# Testing Meridian-MCP

Run checks from the repository root with PowerShell 7 on Windows or Linux.

## Rust and contract gates

The checked-in `rust-toolchain.toml` pins Rust 1.95.0 with rustfmt and Clippy, matching CI and the pinned SpacemanDMM workspace MSRV. Do not override that toolchain when reproducing a CI failure; confirm `rustc --version` reports 1.95.0 before trusting a local green result.

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
cargo deny --all-features check
```

The suite covers owned DreamMaker and DMM fixtures, parse generations, exact lookup and source excerpts, ranked search, map coordinates and PNG output, path containment, executable allowlisting, overwrite policy, mode inventories, runtime buffering, readiness, `Topic()` framing, generated contract drift, and documentation links.

## Tracy native gates

Check out Tracy and byond-tracy at the exact revisions recorded in `tracy-capabilities.json`, then build from those local sources. The builder never downloads source and merges its schema-v2 entries into an existing dmdoc manifest.

```powershell
./scripts/build-tracy-helpers.ps1 `
    -TracyPath C:\path\to\tracy `
    -ByondTracyPath C:\path\to\byond-tracy `
    -OutputDirectory ./target/tracy-package `
    -ManifestPath ./target/tracy-package/helpers/manifest.json
```

This runs only the Meridian-owned CTests, builds the host x86_64 fixed-command helper and x86 BYOND hook, records SHA-256 hashes, and copies both licenses. Windows uses x86 MSVC; Ubuntu requires CMake, a C++20 compiler, PowerShell, and `gcc-multilib`. Native helper build success is not live BYOND compatibility evidence.

For an installed opt-in session, pass `-EnableTracy` to both installation/configuration scripts, start Codex after configuration, then exercise `prepare`, `launch`, `capture`, offline analysis, `status`, and `stop` against the pinned BYOND fixture. A real Meridian-Rift smoke remains a separate named gate and must not modify `BUILD.cmd`.

Run installed sessions with analysis/disabled, development/disabled, development/offline, and development/network startup configurations when changing contract visibility or build policy. The startup ceiling is immutable; `network_mode=allow` must fail under an offline ceiling.

## Installed stdio gate

```powershell
$binaryPath = if ($IsWindows) { ".\target\release\meridian-mcp.exe" } else { "./target/release/meridian-mcp" }

./test_mcp.ps1 -SkipBuild -ServerPath $binaryPath -Mode development
./test_mcp.ps1 -SkipBuild -ServerPath $binaryPath -Mode analysis `
    -DmePath ./tests/fixtures/language/fixture.dme `
    -SearchQuery "return supplied value"
```

These two installed-binary checks run on Windows and Ubuntu 24.04 in per-change CI. The harness sets immutable roots for every supplied fixture, negotiates through the official SDK, validates JSON-only stdout, checks the exact mode inventory and schemas, exercises caller-visible error paths, and parses and searches an owned DreamMaker fixture. `-ServerPath` and `-BinaryPath` are aliases.

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

The named BYOND 516.1687 workflow also verifies the x86 MSVC runtime required by auxtools and starts a generated world with 65,537 unique prototype paths:

```powershell
.\scripts\install-auxtools-runtime.ps1
.\scripts\run-large-prototype-integration.ps1 `
    -DreamMakerPath 'C:\path\to\BYOND\bin\dm.exe' `
    -EvidencePath .\integration\evidence\large-prototype-compatibility.json
```

The large fixture is generated in a temporary directory and removed after the gate. Its successful DreamDaemon readiness marker is the evidence for BYOND's post-64K startup behavior; a successful DreamMaker compile alone is insufficient.

## Meridian-Rift full corpus

```powershell
.\test_mcp.ps1 -SkipBuild -Mode analysis `
    -DmePath C:\path\to\Meridian-Rift\tgstation.dme `
    -SearchQuery "storage navigation exit button" `
    -TimeoutSeconds 300
```

Parsing/search is MCP evidence. Run Meridian-Rift's own PowerShell and `BUILD.cmd` gates before claiming game-code completion. A focused fixture or query is iteration evidence, not the full acceptance matrix.

The named Windows integration uses:

```powershell
.\scripts\run-meridian-compatibility.ps1 `
    -BinaryPath .\target\release\meridian-mcp.exe `
    -MeridianRiftRoot C:\path\to\Meridian-Rift `
    -DreamMakerPath 'C:\Program Files (x86)\BYOND\bin\dm.exe' `
    -EvidencePath .\integration\evidence\meridian-compatibility.json
```

This destructive integration gate is intended for a disposable checkout: it removes only root `tgstation.dmb` and `tgstation.rsc` between gates. It runs the manifest, direct `dm_compile`, forced network `rift_compile`, warm human `BUILD.cmd`, forced offline `rift_compile`, and negative policy/state sessions. Evidence is written even on failure. Offline controls are cooperative preflight, not a firewall, and network samples are always incomplete.

Ubuntu CI runs `scripts/test_unsupported_rift_compile.ps1` against the release binary. It validates portable stale-schema behavior only and must not install BYOND or attempt the Windows wrapper.
