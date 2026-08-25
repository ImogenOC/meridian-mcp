# Complete SpacemanDMM Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Upgrade Meridian-MCP to the approved SpacemanDMM revision and expose every relevant parser, checker, language, DMI, DMM, documentation, and restricted auxtools capability through verified MCP-native contracts.

**Architecture:** Execute five independently testable stages. Direct SpacemanDMM adapters and immutable snapshots supply analysis; fixed, hash-verified artifacts cover binary-only dmdoc and auxtools behavior; every public contract remains behind immutable mode, path, effect, and resource policies.

**Tech Stack:** Rust 1.95, `rmcp` 3.1.3, SpacemanDMM revision `351ddc0ffb2439876d4565ce5130bb6b027ee605`, Tokio, serde/schemars, PowerShell, DreamMaker/BYOND 516.1685, GitHub Actions Windows and Ubuntu.

**Spec:** `docs/superpowers/specs/2026-08-24-spacemandmm-complete-integration-design.md`

## Global Constraints

- Read the approved specification and the focused stage plan before editing.
- Pin Rust exactly to `1.95.0`; verify the invoked `rustc`, not only the installed-toolchain list.
- Pin every SpacemanDMM Git dependency exactly to `351ddc0ffb2439876d4565ce5130bb6b027ee605`.
- Preserve all existing public tools and additive response compatibility unless a regression test proves current behavior incorrect.
- No internal LSP subprocess, arbitrary command runner, arbitrary DLL, arbitrary process attach, non-loopback debugger transport, or runtime download.
- DMI work is report-only. Never alter, merge, rename, recolor, redraw, or select human-authored art.
- Use non-creative pixel matrices generated inside tests; do not add game artwork as fixtures.
- Keep `dm_compile`, `rift_compile`, and DreamChecker evidence distinct.
- Use PowerShell for BYOND builds and compatibility scripts.
- Leave changes uncommitted unless the user explicitly authorizes commits. Each task lists a proposed commit message only as a future handoff convenience.
- Do not dispatch subagents unless the user explicitly selects and authorizes the subagent-driven execution option.

---

## Plan set and dependency order

| Stage | Focused plan | Depends on | Independently testable result |
| --- | --- | --- | --- |
| 1 | `2026-08-24-spacemandmm-foundation-language.md` | Approved spec | Rust/upstream upgrade, capability registry, immutable snapshots, language indexes, DreamChecker detail. |
| 2 | `2026-08-24-spacemandmm-dmi-analysis.md` | Stage 1 snapshot/facade | DMI profile, comparison, duplicate scan, icon audit, and mechanical extraction. |
| 3 | `2026-08-24-spacemandmm-maps-docs.md` | Stage 1 snapshot/facade | Complete DMM inspection/diff/render surface and fixed dmdoc helper. |
| 4 | `2026-08-24-spacemandmm-auxtools-debugger.md` | Stage 1 configuration/facade | Opt-in, Windows-only, owned auxtools debugger lifecycle and queries. |
| 5 | `2026-08-24-spacemandmm-compatibility-release.md` | Stages 1-4 | Cross-platform CI, real Meridian-Rift evidence, documentation, promotion, install smoke. |

Stages 2 and 3 may execute in either order after Stage 1. Stage 4 may begin after Stage 1 but must not merge its runtime-state edits over unreviewed Stage 2 or 3 state changes. Stage 5 begins only after all feature stages pass their focused gates.

## Locked file structure

The focused plans create these responsibility boundaries:

```text
src/
  analysis_snapshot.rs          immutable parsed generation and install semantics
  capabilities.rs               checked-in SpacemanDMM capability registry model
  limits.rs                     immutable scan/render/docs/debug limits
  atomic_output.rs              contained temporary output and atomic replacement
  index/
    mod.rs                      language index aggregate
    symbols.rs                  macros and document symbols
    references.rs               canonical symbol identities and reference hits
    implementations.rs          type/proc implementation and override chains
  spaceman/
    mod.rs                      exact upstream revision and facade exports
    language.rs                 parser/checker API adaptation
    dmi/
      mod.rs                    DMI facade and public domain types
      cache.rs                  bounded content-validated decoded-asset cache
      normalize.rs              pixel normalization and transforms
      duplicate.rs              candidate funnel and deterministic clusters
      source_refs.rs            static icon/icon_state correlation
      extract.rs                mechanical state/frame/contact-sheet/GIF output
    dmm.rs                      DMM info/diff/render adaptation
    docs.rs                     fixed dmdoc-helper invocation
    debugger/
      mod.rs                    debugger facade and lifecycle API
      protocol.rs               minimal current auxtools wire protocol
      session.rs                owned process/session state machine
      artifact.rs               fixed DLL discovery and checksum validation
  tools/
    language.rs                 new document/reference/implementation tools
    dmi.rs                      DMI MCP contracts
    docs.rs                     dmdoc MCP contract
    debugger.rs                 restricted debugger MCP contracts

spacemandmm-capabilities.json   exact upstream capability-to-contract inventory
helpers/manifest.json           fixed helper versions and SHA-256 identities
scripts/
  audit-spacemandmm-capabilities.ps1
  build-spacemandmm-helpers.ps1
  fetch-auxtools.ps1
  run-auxtools-integration.ps1
tests/
  dependency_baseline.rs
  capability_registry.rs
  language_capabilities.rs
  dmi_analysis.rs
  map_capabilities.rs
  docs_helper.rs
  debugger_policy.rs
  compatibility/meridian-rift.json
```

Existing files remain the authority for transport, path policy, contracts, compilation, runtime, and compatibility orchestration. Focused plans name every modification to them.

---

### Task 1: Execute the foundation and language stage

**Files:**
- Plan: `docs/superpowers/plans/2026-08-24-spacemandmm-foundation-language.md`
- Verify: `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `src/analysis_snapshot.rs`, `src/capabilities.rs`, `src/spaceman/language.rs`

**Interfaces:**
- Consumes: Existing `ServerConfig`, `PathPolicy`, `ServerState`, `ToolContract`, and stdio MCP transport.
- Produces: `Arc<AnalysisSnapshot>`, `LanguageIndex`, exact revision constant, capability registry loader, and language/DreamChecker tools used by every later stage.

- [ ] **Step 1: Execute every checkbox in the focused Stage 1 plan**

Run each red/green test exactly as written in `2026-08-24-spacemandmm-foundation-language.md`.

- [ ] **Step 2: Run the Stage 1 aggregate gate**

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc rustc --version
rustup run 1.95.0-x86_64-pc-windows-msvc cargo fmt --all -- --check
rustup run 1.95.0-x86_64-pc-windows-msvc cargo clippy --all-targets --all-features -- -D warnings
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --all-features
```

Expected: Rust reports 1.95.0; formatting, Clippy, and tests exit 0.

- [ ] **Step 3: Record the review checkpoint**

Run `git diff --check` and record the exact commands and counts. Proposed commit message if later authorized: `feat: upgrade SpacemanDMM language foundation`.

---

### Task 2: Execute the DMI analysis stage

**Files:**
- Plan: `docs/superpowers/plans/2026-08-24-spacemandmm-dmi-analysis.md`
- Verify: `src/spaceman/dmi/`, `src/tools/dmi.rs`, `tests/dmi_analysis.rs`

**Interfaces:**
- Consumes: `Arc<AnalysisSnapshot>`, `PathPolicy`, `ServerLimits`, capability registry, atomic-output API.
- Produces: `DmiCache`, profile/comparison/cluster models, icon-reference audit, and five DMI tools.

- [ ] **Step 1: Execute every checkbox in the focused Stage 2 plan**

Run the normalization, cluster, audit, extraction, policy, and protocol red/green cycles in order.

- [ ] **Step 2: Run the Stage 2 aggregate gate**

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --all-features dmi
rustup run 1.95.0-x86_64-pc-windows-msvc cargo clippy --all-targets --all-features -- -D warnings
```

Expected: all DMI-focused tests and Clippy exit 0; extraction tests prove source hashes unchanged.

- [ ] **Step 3: Record the review checkpoint**

Run `git diff --check`. Proposed commit message if later authorized: `feat: add DMI profiling and duplicate audits`.

---

### Task 3: Execute the maps and documentation stage

**Files:**
- Plan: `docs/superpowers/plans/2026-08-24-spacemandmm-maps-docs.md`
- Verify: `src/spaceman/dmm.rs`, `src/spaceman/docs.rs`, `src/tools/map.rs`, `src/tools/docs.rs`

**Interfaces:**
- Consumes: `Arc<AnalysisSnapshot>`, `PathPolicy`, `ServerLimits`, atomic-output API, helper manifest.
- Produces: DMM info/diff/pass/render/batch tools and contained dmdoc generation.

- [ ] **Step 1: Execute every checkbox in the focused Stage 3 plan**

Build helper artifacts only through the fixed helper script and verify their manifest before tool tests.

- [ ] **Step 2: Run the Stage 3 aggregate gate**

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --all-features map
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --all-features docs
rustup run 1.95.0-x86_64-pc-windows-msvc cargo clippy --all-targets --all-features -- -D warnings
```

Expected: map, renderer, documentation-helper, and Clippy gates exit 0.

- [ ] **Step 3: Record the review checkpoint**

Run `git diff --check`. Proposed commit message if later authorized: `feat: complete SpacemanDMM maps and docs adapters`.

---

### Task 4: Execute the restricted auxtools debugger stage

**Files:**
- Plan: `docs/superpowers/plans/2026-08-24-spacemandmm-auxtools-debugger.md`
- Verify: `src/spaceman/debugger/`, `src/tools/debugger.rs`, `scripts/fetch-auxtools.ps1`, `tests/debugger_policy.rs`

**Interfaces:**
- Consumes: `ServerConfig`, compiler/process allowlists, bounded process runner, runtime output model.
- Produces: `DebuggerAccess`, `DebuggerSession`, exact auxtools artifact validation, and restricted debugger tools.

- [ ] **Step 1: Execute every checkbox in the focused Stage 4 plan**

Do not run the live debugger test until the fixed DLL and BYOND installation pass preflight.

- [ ] **Step 2: Run the portable policy gate**

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --all-features debugger
rustup run 1.95.0-x86_64-pc-windows-msvc cargo clippy --all-targets --all-features -- -D warnings
```

Expected: state-machine, protocol, checksum, containment, and schema tests exit 0.

- [ ] **Step 3: Run the Windows live gate**

```powershell
.\scripts\run-auxtools-integration.ps1 `
  -BinaryPath .\target\release\meridian-mcp.exe `
  -FixtureDme .\tests\fixtures\runtime\runtime.dme
```

Expected: compile, launch, breakpoint, stack/scope/variable query, evaluation, continue, and clean stop all pass with no owned process left running.

- [ ] **Step 4: Record the review checkpoint**

Run `git diff --check`. Proposed commit message if later authorized: `feat: add restricted auxtools debugging`.

---

### Task 5: Execute compatibility, documentation, and release promotion

**Files:**
- Plan: `docs/superpowers/plans/2026-08-24-spacemandmm-compatibility-release.md`
- Verify: workflows, compatibility scripts/manifest, README, TESTING, architecture/security/provenance docs, installed binary.

**Interfaces:**
- Consumes: Every tool and support level from Stages 1-4.
- Produces: Windows/Ubuntu evidence, detailed public tool reference, per-tool support promotion, installed Codex binary smoke evidence.

- [ ] **Step 1: Execute every checkbox in the focused Stage 5 plan**

Do not mark a tool `Verified` until its named evidence gate has passed.

- [ ] **Step 2: Run the complete local Rust gate**

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc rustc --version
rustup run 1.95.0-x86_64-pc-windows-msvc cargo fmt --all -- --check
rustup run 1.95.0-x86_64-pc-windows-msvc cargo clippy --all-targets --all-features -- -D warnings
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --all-features
rustup run 1.95.0-x86_64-pc-windows-msvc cargo build --release
.\test_mcp.ps1 -SkipBuild -BinaryPath .\target\release\meridian-mcp.exe -Mode development
```

Expected: exact Rust 1.95.0, all Rust gates exit 0, and the release-binary MCP smoke initializes, lists tools, invokes a fixture query, and shuts down cleanly.

- [ ] **Step 3: Run dependency and generated-contract gates**

```powershell
cargo deny check --all-features
rustup run 1.95.0-x86_64-pc-windows-msvc cargo run --bin render_tool_docs -- --check
.\scripts\audit-spacemandmm-capabilities.ps1 -Check
```

Expected: dependency policy, checked-in tool docs, and capability registry all pass.

- [ ] **Step 4: Run the real Meridian-Rift Windows gate**

Use the exact command in the focused plan with contained Meridian-Rift and DreamMaker paths. Expected: one evidence JSON file with `overall: passed`, recorded repository SHAs, exact versions, and no sensitive keys.

- [ ] **Step 5: Install and smoke-test before restart**

Copy only the verified release binary and fixed helper artifacts through the documented installer path, start that exact installed binary with the configured Codex environment, and run initialize plus `tools/list`. Compare its SHA-256 to the built artifact.

- [ ] **Step 6: Request the Codex restart**

Request restart only after Step 5 passes. Proposed commit message if later authorized: `docs: record complete SpacemanDMM compatibility`.

