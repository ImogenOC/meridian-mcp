# Testing Meridian-MCP

Run checks from the repository root with PowerShell 7 on Windows or Linux.

## Rust and contract gates

The checked-in `rust-toolchain.toml` pins Rust 1.95.0 with rustfmt and Clippy, matching CI and the pinned SpacemanDMM workspace MSRV. Do not override that toolchain when reproducing a CI failure; confirm the verbose compiler identity before trusting a local green result.

```powershell
rustc +1.95.0 --version --verbose
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.95.0 test --locked --all-features
cargo +1.95.0 build --locked --release
cargo +1.95.0 deny check
```

The suite covers owned DreamMaker and DMM fixtures, parse generations, exact lookup and source excerpts, ranked search, map coordinates and PNG output, path containment, executable allowlisting, overwrite policy, mode inventories, runtime buffering, readiness, `Topic()` framing, generated contract drift, and documentation links.

The repository's checked-in cross-platform text policy is LF, including PowerShell and owned DreamMaker fixtures. Do not rewrite files to CRLF to satisfy a Windows-only observation. Parsers and contract tests that consume external text must accept both LF and CRLF explicitly; use `git diff --check` and the checked-in `.gitattributes` as the repository authority rather than a developer's `core.autocrlf` setting.

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

The managed provenance and runtime-integrity fixture is separate. It compiles only the owned fixture, proves changed-input and failed-compile stale rejection across an MCP restart, restores the original bytes explicitly, then requires a fresh verified launch and reports the tracked runtime mutation without reverting it:

```powershell
./scripts/run-provenance-integrity-integration.ps1 `
    -DreamMakerPath 'C:\Program Files (x86)\BYOND\bin\dm.exe' `
    -BinaryPath ./target/release/meridian-mcp.exe `
    -EvidencePath ./integration/evidence/local-provenance-integrity.json

./scripts/test-provenance-evidence-validation.ps1 `
    -EvidencePath ./integration/evidence/local-provenance-integrity.json
```

The evidence document is schema 1 and bounded to 256 KiB. It omits raw stdout/stderr, absolute profile paths, private-state records, source copies, DMB/RSC files, and player/account identifiers.

The named BYOND 516.1687 workflow separates parser, synthetic runtime, and real Windows product evidence. Verify the parser boundary independently through the release MCP:

```powershell
.\scripts\run-large-prototype-parser-integration.ps1 `
    -BinaryPath .\target\release\meridian-mcp.exe `
    -EvidencePath .\integration\evidence\large-prototype-parser.json
```

The parser gate uses a compact flat declaration layout and resolves the first, boundary, and last declared paths. It does not start DreamMaker or DreamDaemon. The synthetic runtime uses 256-child buckets so it exceeds 64K total declarations without hitting DreamMaker's separate direct-child limit. Run a bucketed control before the boundary fixture:

```powershell
.\scripts\install-auxtools-runtime.ps1
.\scripts\run-large-prototype-integration.ps1 `
    -DreamMakerPath 'C:\path\to\BYOND\bin\dm.exe' `
    -PrototypeCount 50000 -RuntimeCase control `
    -EvidencePath .\integration\evidence\prototype-control.json

.\scripts\run-large-prototype-integration.ps1 `
    -DreamMakerPath 'C:\path\to\BYOND\bin\dm.exe' `
    -PrototypeCount 65537 -RuntimeCase boundary `
    -ControlEvidencePath .\integration\evidence\prototype-control.json `
    -EvidencePath .\integration\evidence\prototype-boundary.json
```

The fixtures are generated in temporary directories and removed after successful gates. DreamDaemon uses an ephemeral port bound to `127.0.0.1`; the synthetic world is never intentionally exposed beyond the runner. A readiness marker is required; a successful DreamMaker compile or live process alone is insufficient. Evidence classifies `passed`, `compile_failure`, `environment_failure`, `boundary_regression`, or `inconclusive_timeout` and retains bounded process metrics, logs, events, version provenance, and cleanup state.

The hosted workflow uses Ubuntu as the required synthetic BYOND engine lane. Windows synthetic startup remains diagnostic until three consecutive scheduled or manual runs pass. The Windows job independently requires real Meridian-Rift compatibility, the owned auxtools protocol fixture, and the owned Tracy live fixture; it has no dependency on the synthetic jobs.

Run the auxtools protocol gate without a full-game boot:

```powershell
.\scripts\run-auxtools-integration.ps1 `
    -DreamMakerPath 'C:\path\to\BYOND\bin\dm.exe' `
    -BinaryPath .\target\release\meridian-mcp.exe `
    -EvidencePath .\integration\evidence\auxtools-compatibility.json `
    -HostMode headless
```

Supplying `-DmbPath` remains an explicit full-game diagnostic. It is not the required native protocol gate because repository initialization time is unrelated to auxtools wire compatibility.

To run the hosted check without opening a pull request, open the repository's **Actions** tab, select **BYOND integration**, choose **Run workflow**, supply the intended Meridian-Rift ref, and start the run. The artifacts are `windows-meridian-compatibility-evidence`, `prototype-parser-windows-evidence`, `prototype-parser-ubuntu-evidence`, `prototype-runtime-windows-evidence`, `prototype-runtime-ubuntu-evidence`, and `tracy-linux-compatibility-evidence`.

Before testing an installation/configuration change, run the parser and private temporary-file round trips:

```powershell
Get-ChildItem ./scripts -Filter *.ps1 | ForEach-Object {
    $null = [scriptblock]::Create((Get-Content -LiteralPath $_.FullName -Raw))
}
./scripts/test-configure-codex-meridian-mcp.ps1
./scripts/test-meridian-evidence-validation.ps1
./scripts/test-provenance-evidence-validation.ps1
```

## Meridian-Rift full corpus

```powershell
.\test_mcp.ps1 -SkipBuild -Mode analysis `
    -DmePath C:\path\to\Meridian-Rift\tgstation.dme `
    -SearchQuery "storage navigation exit button" `
    -TimeoutSeconds 300
```

Parsing/search is MCP evidence. Run Meridian-Rift's own PowerShell and `BUILD.cmd` gates before claiming game-code completion. A focused fixture or query is iteration evidence, not the full acceptance matrix.

The snapshot-reuse gate measures the `dm_parse_environment` short-circuit at real scale. It is ignored by default because it needs a full checkout, and it only reads that checkout:

```powershell
$env:MERIDIAN_SCALE_DME = 'C:\path\to\Meridian-Rift\tgstation.dme'
cargo +1.95.0 test --locked --release --test parse_reuse_scale -- --ignored --nocapture
```

Run it in release; a debug parse is slow enough to obscure the comparison. It asserts that reusing an unchanged environment is decisively cheaper than parsing it and that reuse does not install a new state generation, and it prints type, symbol, input, and diagnostic counts alongside both timings. Cache invalidation on edit is proved at fixture scale by the unit tests, which do not need to mutate a real checkout. Reuse timings are filesystem-bound and vary with input count and cache warmth; treat a single run as iteration evidence.

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
