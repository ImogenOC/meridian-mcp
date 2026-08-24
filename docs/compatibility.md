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
| Rust | 1.88 minimum | Verified | Declared in Cargo and configured in CI. |
| BYOND | 516.1685 Meridian-Rift pin | Provisional | The named Windows workflow now records direct compiler, network full-build, warm human-build, and offline full-build evidence. No promotion occurs until that workflow is green. |
| SpacemanDMM | `7fdd00d8e9b7f7583df4960b5ed38269685ec432` | Provisional | Parser, search, map, and diagnostic tests. |
| MCP transport | `rmcp` 3.1.3 | Verified | Official SDK tests and installed stdio negotiation/tool smoke passed. |

Update this table only from fresh, reproducible evidence. Never infer platform support from another operating system.

## Named Meridian-Rift gate

The scheduled/manual Windows workflow drives the release binary through stdio MCP, parses the real `tgstation.dme`, runs the versioned lookup/definition/search manifest, checks deterministic ranked retrieval, and records direct and full-build artifact evidence. It records exact Meridian-MCP and Meridian-Rift SHAs plus BYOND `516.1685`. `rift_compile` must produce fresh artifacts in both `network_mode=allow` and forced `network_mode=offline`, with a successful warm human `BUILD.cmd` between them. Endpoint observations remain best-effort and always state `capture_complete: false`.

Ubuntu 24.04 verifies portable Rust, contracts, installed stdio, owned parsing/search, and the stable stale-schema `unsupported_platform` response. It does not install BYOND or attempt `RIFT_BUILD.cmd`, and supplies no DreamMaker or Meridian-Rift full-build evidence.

## Deferred verification matrix

| Capability | Owned fixture | Named-platform/real-repository gate | Required semantic evidence | Current blocker | Status |
| --- | --- | --- | --- | --- | --- |
| DreamChecker | Language fixture with known diagnostics | Real Meridian-Rift diagnostic corpus on Windows and Ubuntu | Stable structured severities, files, and messages without treating SpacemanDMM as DreamMaker | No versioned full-corpus diagnostic assertions | Provisional |
| Map inspection | DMM fixture with known dimensions and coordinates | Representative Meridian-Rift DMM/TGM files on Windows and Ubuntu | Exact format, dimensions, descendant matches, and coordinates | No versioned real-map corpus | Provisional |
| PNG rendering | Owned DMM/DMI render fixture | Named Windows render gate using parsed real metadata | Valid PNG, expected dimensions, and non-transparent semantic pixels | No real-repository render artifact assertion | Provisional |
| DreamDaemon lifecycle | Owned runtime fixture | BYOND 516.1685 Windows process-context gate | Readiness marker, bounded output, running status, and owned-tree stop | Reliable hosted-runner runtime gate not yet recorded | Provisional |
| `Topic()` | Owned loopback ping/pong handler | BYOND 516.1685 Windows runtime gate | Exact decoded response over server-owned loopback DreamDaemon | Depends on the missing lifecycle gate | Provisional |
