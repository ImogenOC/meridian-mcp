# SpacemanDMM Compatibility and Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Verify the complete SpacemanDMM integration on Ubuntu and Windows against owned fixtures and real Meridian-Rift, publish detailed per-tool documentation and evidence, then install and smoke-test the exact Codex binary before requesting restart.

**Architecture:** Expand existing generated contracts, stdio harnesses, compatibility manifest, and GitHub workflows instead of creating a parallel test system. Promote support per tool only from a named evidence record; package fixed helpers beside the exact release binary and validate their hashes during the installed smoke.

**Tech Stack:** Rust 1.95, Cargo/Clippy/rustfmt/cargo-deny, PowerShell 7, GitHub Actions Ubuntu 24.04 and Windows, BYOND 516.1685, Meridian-Rift, rmcp stdio protocol.

**Spec:** `docs/superpowers/specs/2026-08-24-spacemandmm-complete-integration-design.md`

## Global Constraints

- Complete every feature-stage plan first.
- Reproduce CI with exact Rust 1.95.0; never substitute plain newer `cargo` evidence.
- Test the release binary through stdio MCP, not only Rust function calls.
- Record exact Meridian-MCP, Meridian-Rift, SpacemanDMM, Rust, BYOND, dmdoc, and auxtools identities.
- Ubuntu makes no DreamMaker, DreamDaemon, DreamSeeker, `rift_compile`, or auxtools claim.
- A focused or truncated result cannot promote a broader capability.
- Preserve human-authored `BUILD.cmd`; continue through the existing separate `RIFT_BUILD.cmd` integration.
- Do not commit generated art/map render/doc output; retain only bounded machine-readable evidence.
- Leave changes uncommitted absent explicit authorization.

---

### Task 1: Make per-change Windows and Ubuntu CI enforce the complete portable surface

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `tests/workflow_contract.rs`
- Modify: `test_mcp.ps1`
- Create: `scripts/run-portable-spacemandmm-integration.ps1`
- Modify: `TESTING.md`

**Interfaces:**
- Consumes: Exact toolchain, helper packaging script, fixture corpus, release binary.
- Produces: Equivalent portable gates on Windows and Ubuntu plus Linux-specific unsupported debugger/Rift evidence.

- [ ] **Step 1: Write failing workflow-contract assertions**

Require both matrix platforms to run exact Rust 1.95.0, helper packaging, formatting, Clippy with `-D warnings`, all-feature tests, release build, capability audit, generated docs check, installed stdio smoke, and portable SpacemanDMM integration. Require cargo-deny only on Linux.

- [ ] **Step 2: Run tests and confirm workflows lack the new gates**

Run: `cargo test --test workflow_contract --all-features`

Expected: failure naming 1.88 and missing helper/capability/portable steps.

- [ ] **Step 3: Extend the installed stdio smoke harness**

Add switches for generated technical DMI fixture, second map, document-symbol/reference/implementation queries, DMI profile/duplicate/audit, map diff/pass/batch render, and dmdoc output. The script creates all outputs below a temporary contained root and removes them in `finally` after validating hashes/signatures.

- [ ] **Step 4: Implement the portable integration script**

The script starts one analysis session and one development session. It must exercise every non-BYOND tool through `initialize`, `tools/list`, and `tools/call`, assert tool schemas/effects, check deterministic repeated results, and verify stale-schema responses for unavailable Windows-only tools. It writes one bounded JSON result and removes generated files.

- [ ] **Step 5: Update the matrix workflow**

Use `dtolnay/rust-toolchain@1.95.0`. Check out SpacemanDMM at the exact revision into `integration/SpacemanDMM`, build dmdoc through `build-spacemandmm-helpers.ps1`, set `MERIDIAN_MCP_HELPER_MANIFEST`, then run all gates. On Ubuntu run `cargo deny check --all-features`; do not run the container action on Windows.

- [ ] **Step 6: Run local workflow and portable tests**

Run:

```powershell
cargo test --test workflow_contract --all-features
cargo build --release
.\scripts\run-portable-spacemandmm-integration.ps1 `
  -BinaryPath .\target\release\meridian-mcp.exe `
  -EvidencePath .\target\portable-spacemandmm-evidence.json
```

Expected: workflow tests pass and portable evidence reports every expected tool passed.

- [ ] **Step 7: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `ci: verify portable SpacemanDMM surface`.

---

### Task 2: Expand the real Meridian-Rift compatibility manifest and Windows gate

**Files:**
- Modify: `tests/compatibility/meridian-rift.json`
- Modify: `tests/compatibility_manifest.rs`
- Modify: `scripts/run-meridian-compatibility.ps1`
- Modify: `scripts/run-byond-integration.ps1`
- Modify: `.github/workflows/byond-integration.yml`
- Create: `docs/evidence/spacemandmm-compatibility.schema.json`
- Modify: `tests/workflow_contract.rs`

**Interfaces:**
- Consumes: Real Meridian-Rift checkout, exact BYOND, fixed dmdoc/auxtools artifacts, full tool surface.
- Produces: One schema-validated evidence bundle covering language, DMI, DMM, docs, direct compile, full build, runtime, and debugger.

- [ ] **Step 1: Write failing manifest-schema tests**

Bump `schema_version` to 2 and require sections for `document_symbols`, `references`, `implementations`, `diagnostics`, `dmis`, `dmi_scan`, `maps`, `map_diff`, `render`, `docs`, and `debugger` in addition to existing exact lookup/search/build sections.

- [ ] **Step 2: Run the manifest tests and confirm version 1 fails**

Run: `cargo test --test compatibility_manifest --all-features`

Expected: failure naming missing schema-version-2 sections.

- [ ] **Step 3: Add stable real-repository cases**

Use these contained paths as the initial named corpus:

```json
{
  "dmis": ["icons/mob/vatgrowing.dmi", "icons/ui_icons/minimap/minimap.dmi"],
  "dmi_scan": { "scope": "icons/ui_icons/minimap", "glob": "**/*.dmi", "maximum_files": 100 },
  "maps": ["_maps/virtual_domains/test_only.dmm", "_maps/virtual_domains/xeno_nest.dmm"],
  "map_diff": {
    "left": "_maps/virtual_domains/test_only.dmm",
    "right": "_maps/virtual_domains/xeno_nest.dmm"
  }
}
```

Keep current exact type/proc/var cases. Add document symbols for `code/controllers/subsystem.dm`, references to `/datum/controller/subsystem/var/next_fire`, and implementations for `/datum/controller/subsystem/proc/fire`. Assert membership and contained source suffixes, never absolute line numbers or whole-repository counts.

- [ ] **Step 4: Extend the compatibility session**

After parse, call every language/DreamChecker tool, profile both DMIs, scan the bounded icon scope, audit icons with `include_unused=false`, inspect/diff both maps, list passes, render a 10x10 bounded chunk, batch-render two 5x5 chunks, and generate dmdoc output. Validate PNG signatures, dimensions, output hashes, dmdoc `index.html`, and at least one valid crosslink.

Run DMI extraction in a second request after selecting the first state returned by `dm_dmi_info`; validate output then delete it. Do not assert a duplicate cluster exists; assert the scan completed within its declared scope or reported explicit truncation.

- [ ] **Step 5: Integrate existing compile/build/runtime and new debugger evidence**

Retain direct `dm_compile`, network `rift_compile`, warm human `BUILD.cmd`, and offline `rift_compile` ordering. Add the existing DreamDaemon readiness/Topic/stop fixture evidence and invoke `run-auxtools-integration.ps1` for the owned DreamSeeker debugger. Keep those process identities separate in JSON.

- [ ] **Step 6: Validate evidence safety and schema**

Reject keys matching token/secret/password/authorization/cookie, paths outside the two checkouts and workflow temp directory, output above 10 MiB, or missing version/SHA fields. Evidence schema requires `capture_complete: false` for endpoint audits and explicit `truncated`/`complete` flags for scans.

- [ ] **Step 7: Run manifest and fixture-level tests**

Run:

```powershell
cargo test --test compatibility_manifest --all-features
cargo test --test workflow_contract --all-features
```

Expected: schema and workflow contracts pass without needing BYOND.

- [ ] **Step 8: Run the real Windows gate**

Run:

```powershell
.\scripts\run-byond-integration.ps1 `
  -MeridianRiftRoot C:\path\to\Meridian-Rift `
  -DreamMakerPath 'C:\Program Files (x86)\BYOND\bin\dm.exe' `
  -BinaryPath .\target\release\meridian-mcp.exe `
  -EvidencePath .\integration\evidence\meridian-compatibility.json
```

Expected: exit 0 and evidence `overall: passed` with exact repository/tool/helper identities.

- [ ] **Step 9: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `test: cover full Meridian-Rift SpacemanDMM integration`.

---

### Task 3: Add real Meridian-Rift analysis on Ubuntu

**Files:**
- Create: `scripts/run-meridian-analysis-compatibility.ps1`
- Modify: `.github/workflows/ci.yml`
- Modify: `tests/workflow_contract.rs`
- Modify: `docs/compatibility.md`
- Modify: `TESTING.md`

**Interfaces:**
- Consumes: Ubuntu release binary, exact dmdoc helper, Meridian-Rift checkout, schema-version-2 analysis manifest.
- Produces: Named Ubuntu evidence for parser/language/DreamChecker/DMI/DMM/render/docs behavior without BYOND claims.

- [ ] **Step 1: Write failing Ubuntu workflow assertions**

Require a dedicated Ubuntu job to check out Meridian-Rift, record its SHA, build the exact dmdoc helper, and run the analysis-only compatibility script. Explicitly reject steps invoking DreamMaker, DreamDaemon, DreamSeeker, `BUILD.cmd`, `RIFT_BUILD.cmd`, or the auxtools fetch script.

- [ ] **Step 2: Run the workflow test and confirm the job is absent**

Run: `cargo test --test workflow_contract ubuntu_meridian_analysis --all-features`

Expected: failure for missing job/script.

- [ ] **Step 3: Implement the analysis-only script**

Reuse the schema-version-2 manifest and stdio session helpers, but execute only parse, language, DreamChecker, DMI, DMM, render, and dmdoc sections. Require Windows-only tools absent. Record OS/architecture, both SHAs, Rust, MCP, helper, and upstream revision.

- [ ] **Step 4: Add the Ubuntu job**

Run on `ubuntu-24.04`, build with exact Rust 1.95.0, and upload evidence even on failure. Use a 30-minute job timeout and bounded tool timeouts. This job may follow Meridian-Rift's default branch but records the resolved SHA.

- [ ] **Step 5: Run script syntax and workflow tests locally**

Run:

```powershell
$errors = $null
[System.Management.Automation.Language.Parser]::ParseFile(
  (Resolve-Path .\scripts\run-meridian-analysis-compatibility.ps1),
  [ref]$null,
  [ref]$errors
) | Out-Null
if ($errors.Count) { throw ($errors | Out-String) }
cargo test --test workflow_contract ubuntu_meridian_analysis --all-features
```

Expected: no PowerShell parse errors and workflow tests pass.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `ci: verify Meridian-Rift analysis on Ubuntu`.

---

### Task 4: Generate detailed per-tool documentation and security/provenance updates

**Files:**
- Modify: `README.md`
- Modify: `src/contracts.rs`
- Modify: `src/bin/render_tool_docs.rs`
- Regenerate: `docs/tool-contracts.md`
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Modify: `docs/compatibility.md`
- Modify: `docs/dependency-policy.md`
- Modify: `docs/provenance.md`
- Modify: `TESTING.md`
- Modify: `SECURITY.md`
- Modify: `CHANGELOG.md`
- Modify: `tests/documentation.rs`
- Modify: `tests/tool_contracts.rs`

**Interfaces:**
- Consumes: Final schemas, effects, limits, capability registry, evidence statuses.
- Produces: Detailed human-readable description of every individual tool and generated contract consistency.

- [ ] **Step 1: Write failing documentation-contract tests**

For every `all_contracts()` entry, require exactly one README table row, a generated contract row, mode/effect/support language, and at least one capability-registry mapping. Require configuration docs for `MERIDIAN_MCP_DEBUGGER`, fixed helper installation, resource limits, and no creative-asset mutation.

- [ ] **Step 2: Run documentation tests and confirm missing rows**

Run: `cargo test --test documentation --all-features`

Expected: failure listing every undocumented new tool/configuration.

- [ ] **Step 3: Replace broad capability prose with individual tool descriptions**

Keep analysis and development sections. For each tool document purpose, required prior parse/session state, important inputs, returned evidence, side effects, limits/truncation, platform/mode/feature gate, and the key limitation. Explicitly state that DMI tools report/extract only and never alter art.

- [ ] **Step 4: Update architecture, security, dependency, and provenance**

Document immutable snapshots, bounded asset cache, direct-vs-helper facade, capability audit, exact revision, Rust 1.95, dmdoc packaging, auxtools v2.3.7 hash, DreamSeeker ownership, evaluation risk, loopback transport, and GPL distribution review. Preserve the statement that engineering documentation is not legal advice.

- [ ] **Step 5: Generate and verify contracts**

Run:

```powershell
cargo run --bin render_tool_docs
cargo run --bin render_tool_docs -- --check
cargo test --test documentation --all-features
cargo test --test tool_contracts --all-features
```

Expected: generated docs are current and all documentation tests pass.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `docs: describe complete SpacemanDMM tool surface`.

---

### Task 5: Promote only evidenced support levels

**Files:**
- Create: `docs/evidence/spacemandmm-compatibility.json`
- Modify: `src/contracts.rs`
- Modify: `docs/compatibility.md`
- Modify: `README.md`
- Modify: `spacemandmm-capabilities.json`
- Modify: `tests/tool_contracts.rs`
- Modify: `tests/documentation.rs`

**Interfaces:**
- Consumes: Fresh green Windows and Ubuntu evidence tied to exact commits.
- Produces: Per-tool `Verified`, `Provisional`, or `Experimental` status with named evidence.

- [ ] **Step 1: Validate fresh evidence before changing status**

Require evidence schema success, exact current Meridian-MCP commit, recorded Meridian-Rift commits, Rust 1.95.0, approved SpacemanDMM revision, Windows BYOND 516.1685, and no failed/truncated required stage. If the current working tree differs from the evidenced commit, keep affected tools provisional and record why.

- [ ] **Step 2: Create a bounded checked-in evidence summary**

Copy only the schema-validated summary fields and assertion results into `docs/evidence/spacemandmm-compatibility.json`; omit generated HTML/PNG/GIF, raw unbounded logs, and environment values.

- [ ] **Step 3: Promote per tool**

Set parser/language/DreamChecker/DMI/DMM/render/dmdoc tools `Verified` only when both owned tests and their named Windows/Ubuntu real-repository evidence pass. Keep debugger tools `Experimental` because they are opt-in active debugging even after the live Windows gate; document the green platform evidence separately. Keep BYOND-only tools at their independently evidenced status.

- [ ] **Step 4: Run support/evidence consistency tests**

Run:

```powershell
cargo test --test tool_contracts --all-features
cargo test --test documentation --all-features
.\scripts\audit-spacemandmm-capabilities.ps1 -Check
```

Expected: every promoted tool names a passing evidence assertion; no unsupported or unevidenced tool is marked Verified.

- [ ] **Step 5: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `docs: record SpacemanDMM compatibility evidence`.

---

### Task 6: Package, install, and smoke-test the exact Codex binary

**Files:**
- Create: `scripts/install-meridian-mcp.ps1`
- Modify: `README.md`
- Modify: `TESTING.md`
- Modify: `tests/workflow_contract.rs`
- External install target: `C:\Users\Zoe\AppData\Local\meridian-mcp\`
- Read-only configuration check: `C:\Users\Zoe\.codex\config.toml`

**Interfaces:**
- Consumes: Verified release binary, embedded helper manifest, dmdoc helper, fixed auxtools DLL, existing Codex MCP configuration.
- Produces: Atomic local installation with manifest and exact installed-binary stdio evidence.

- [ ] **Step 1: Write failing installer contract tests**

Require explicit source binary/install root, SHA-256 manifest, sibling helper layout, temporary staging, backup/restore on failure, no configuration mutation, and no network access. The installer must reject a source binary whose embedded helper manifest does not match supplied helpers.

- [ ] **Step 2: Run workflow tests and confirm installer is absent**

Run: `cargo test --test workflow_contract installer --all-features`

Expected: failure for missing installer.

- [ ] **Step 3: Implement atomic installation**

The script accepts `-BinaryPath`, `-DmdocHelperPath`, optional `-AuxtoolsDllPath`, and `-InstallRoot`. It verifies all hashes against the release manifest, stages below the install root, stops without touching a running process, atomically swaps the installed directory, and writes `installation-manifest.json`. It never edits Codex configuration.

- [ ] **Step 4: Build and run all final local gates**

Run:

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc rustc --version
rustup run 1.95.0-x86_64-pc-windows-msvc cargo fmt --all -- --check
rustup run 1.95.0-x86_64-pc-windows-msvc cargo clippy --all-targets --all-features -- -D warnings
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --all-features
rustup run 1.95.0-x86_64-pc-windows-msvc cargo build --release
cargo deny check --all-features
cargo run --bin render_tool_docs -- --check
.\scripts\audit-spacemandmm-capabilities.ps1 -Check
```

Expected: exact Rust 1.95.0 and every gate exits 0.

- [ ] **Step 5: Install to the existing local MCP root**

Run the installer with `C:\Users\Zoe\AppData\Local\meridian-mcp` as `-InstallRoot` and the versioned destination name `meridian-mcp-spacemandmm-20260824.exe`. Preserve unrelated files and the prior installation backup until the smoke passes.

- [ ] **Step 6: Verify Codex configuration without changing it**

Read `[mcp_servers.dm-mcp]` from `C:\Users\Zoe\.codex\config.toml`. The current command is `C:\Users\Zoe\AppData\Local\meridian-mcp\meridian-mcp-20260824.exe`; confirm roots, mode, compiler, and Rift settings remain intentional, then obtain explicit approval before changing the command to the new versioned binary or adding `MERIDIAN_MCP_DEBUGGER=auxtools`.

- [ ] **Step 7: Smoke-test the exact installed binary**

Compare installed and built SHA-256 values, then run:

```powershell
.\test_mcp.ps1 `
  -SkipBuild `
  -BinaryPath C:\Users\Zoe\AppData\Local\meridian-mcp\meridian-mcp-spacemandmm-20260824.exe `
  -Mode development `
  -DmePath .\tests\fixtures\language\fixture.dme `
  -SearchQuery 'fixture compute'
```

Expected: initialize, tool inventory, parse, language query, capability metadata, and shutdown all pass through the installed executable.

- [ ] **Step 8: Request restart only after installed smoke passes**

Report exact binary/helper hashes, configuration path, enabled feature gates, and verification commands. Then tell the user to restart Codex. Do not claim the running Codex process has loaded the new server before that restart.

- [ ] **Step 9: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `release: package complete Meridian-MCP integration`.
