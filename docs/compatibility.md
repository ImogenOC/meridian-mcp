# Compatibility and evidence

- **Verified:** committed automated tests and a documented integration gate pass on a named platform and version.
- **Provisional:** useful automated coverage exists, but part of the integration matrix remains untested.
- **Experimental:** behavior may change or fail and is disabled by default when it crosses a security or protocol boundary.
- **Unsupported:** not exposed in normal operation or removed.

| Component | Target | Status | Evidence |
| --- | --- | --- | --- |
| Windows | Current host, 2026-08-24 | Verified for existing owned gates | Rust and installed stdio fixture evidence exists. New full-corpus analysis and `rift_compile` capabilities remain provisional until the scheduled workflow records a green run against exact repository SHAs. |
| Linux | Ubuntu 24.04 GitHub-hosted runner | Provisional | Per-change all-feature Rust checks, release build, installed stdio protocol/tool smoke, and owned-fixture parse/search are configured. Promote after the first green Ubuntu run; no BYOND claim. |
| macOS | Any | Unsupported | No test evidence. |
| Rust | 1.95.0 exact project toolchain | Verified | Declared in Cargo, `rust-toolchain.toml`, and CI. |
| BYOND | 516.1687 Meridian-Rift pin | Provisional | The named Windows workflow records an over-64K prototype startup plus direct compiler, network full-build, warm human-build, and offline full-build evidence. No promotion occurs until that workflow is green. |
| SpacemanDMM | `351ddc0ffb2439876d4565ce5130bb6b027ee605` | Provisional | Parser, search, map, and diagnostic tests. |
| Tracy native helper | Tracy `099df3de` (v0.14.0), protocol 82, x86_64 Windows | Provisional | Release helper, schema-2 protocol, query, strict validation, rotation, and cancellation CTests pass locally on Windows; hosted Windows and Ubuntu results remain independent. Live BYOND evidence is still required. |
| byond-tracy hook | `d1ec4047`, x86, BYOND 516.1685-1687 | Experimental | Exact-revision x86 Windows artifact built and hash-manifested locally. A local BYOND 516.1687 technical fixture completed five compatible 30-second controls; hosted live evidence remains required. |
| MCP transport | `rmcp` 3.1.3 | Verified | Official SDK tests and installed stdio negotiation/tool smoke passed. |

Update this table only from fresh, reproducible evidence. Never infer platform support from another operating system.

## Named Meridian-Rift gate

The scheduled/manual Windows workflow drives the release binary through stdio MCP, parses the real `tgstation.dme`, runs the versioned lookup/definition/search manifest, checks deterministic ranked retrieval, and records direct and full-build artifact evidence. It records exact Meridian-MCP and Meridian-Rift SHAs plus BYOND `516.1687`. Before the repository gate, a generated world with 65,537 unique prototype paths must compile, start in DreamDaemon, emit its readiness marker, and shut down cleanly. `rift_compile` must produce fresh artifacts in both `network_mode=allow` and forced `network_mode=offline`, with a successful warm human `BUILD.cmd` between them. Endpoint observations remain best-effort and always state `capture_complete: false`.

Ubuntu 24.04 verifies portable Rust, contracts, installed stdio, owned parsing/search, and the stable stale-schema `unsupported_platform` response. It does not install BYOND or attempt `RIFT_BUILD.cmd`, and supplies no DreamMaker or Meridian-Rift full-build evidence.

## Deferred verification matrix

| Capability | Owned fixture | Named-platform/real-repository gate | Required semantic evidence | Current blocker | Status |
| --- | --- | --- | --- | --- | --- |
| DreamChecker | Language fixture with known diagnostics | Real Meridian-Rift diagnostic corpus on Windows and Ubuntu | Stable structured severities, files, and messages without treating SpacemanDMM as DreamMaker | No versioned full-corpus diagnostic assertions | Provisional |
| Map inspection | DMM fixture with known dimensions and coordinates | Representative Meridian-Rift DMM/TGM files on Windows and Ubuntu | Exact format, dimensions, descendant matches, and coordinates | No versioned real-map corpus | Provisional |
| PNG rendering | Owned DMM/DMI render fixture | Named Windows render gate using parsed real metadata | Valid PNG, expected dimensions, and non-transparent semantic pixels | No real-repository render artifact assertion | Provisional |
| DreamDaemon lifecycle | Owned runtime fixture | BYOND 516.1687 Windows process-context and over-64K prototype gates | Readiness marker, bounded output, running status, and owned-tree stop | Fresh hosted-runner evidence is required | Provisional |
| `Topic()` | Owned loopback ping/pong handler | BYOND 516.1687 Windows runtime gate | Exact decoded response over server-owned loopback DreamDaemon | Depends on the named lifecycle gate | Provisional |
| Tracy fixed helper | Native protocol/query/validation/session fixtures | Windows and Ubuntu native build jobs | Bounded multiplexed envelopes, deterministic queries, raw-clock/trace validation, drain/capture rotation, exact revisions and ordered patches, dual architecture, and packaged licenses | Fresh hosted runs not yet recorded | Provisional |
| Tracy live capture | Technical BYOND fixture using repository-supported `-params tracy` | BYOND 516.1687 Windows, then Ubuntu independently | Prepared hook hash, loopback endpoint, known proc/file/line, `ServerTick` frames, bounded atomic trace, cancellation, and clean stop | Local Windows acceptance passed on 2026-08-26; fresh hosted Windows and Ubuntu evidence remains required | Experimental |
| Tracy offline analysis | Native query fixtures and malformed-trace errors | Packaged helper on Windows and Ubuntu | Hotspots, zone aggregates, frame percentiles, comparison identity, deterministic truncation, optional source correlation | No checked-in representative trace fixture because binary trace stability/licensing must be reviewed | Provisional |
| Tracy experiment controls | Deterministic identity, range, memory-role, network-honesty, and noise fixtures | Five 30-second BYOND 516.1687 controls on each claimed live host | Same immutable experiment/phase, complete-only statistics, separate role memory, fixed noise envelope, redacted summaries, and raw traces retained locally | Local Windows 5x30 acceptance passed with 299 complete frames per trace and a non-noisy baseline; hosted platform evidence has not completed | Experimental |
