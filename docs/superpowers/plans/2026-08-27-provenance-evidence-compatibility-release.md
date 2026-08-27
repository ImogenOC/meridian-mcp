# Provenance and Evidence Compatibility Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Configure the new trust boundaries, qualify managed compile/run integrity on Windows BYOND, verify portable behavior on Ubuntu, and publish complete tool and compatibility documentation.

**Architecture:** Installation/configuration scripts create and register the private state and explicit repository authorizations. An owned PowerShell fixture exercises parse, sync, compile, stale rejection, fresh launch, tracked mutation reporting, and stop using BYOND 516.1687. Standard Rust CI owns cross-platform policy, parser, persistence, integrity, and evidence-reader gates; compatibility documents promote only the platform evidence that actually passes.

**Tech Stack:** PowerShell 7, GitHub Actions, Rust 1.95, BYOND 516.1687, existing Meridian-MCP installer/configurator, generated tool contracts and compatibility manifests.

**Spec:** `docs/superpowers/specs/2026-08-27-meridian-mcp-provenance-and-native-evidence-design.md`

## Global Constraints

- Do not modify Meridian-Rift's human-authored `BUILD.cmd` or other human build entry points.
- Use PowerShell for every BYOND build and live runtime gate.
- Use the existing verified BYOND 516.1687 installer and pinned archive hash.
- Rebuild the exact release binary used by the MCP session before recording evidence.
- CI prints the exact Rust compiler version and uses `--locked` for all Cargo build/test/Clippy commands.
- Windows and Ubuntu retain independent compatibility states.
- Fixture success is not production performance acceptance.
- Raw logs, profiles, traces, private state records, and machine paths are not published.
- Failed live runs retain bounded technical evidence and never promote compatibility status.
- Installation changes preserve unrelated existing MCP configuration fields.
- Commit and push steps require explicit user authorization during execution.

---

## Locked file structure

- Modify `scripts/install-meridian-mcp.ps1`: create private state and pass repository authorizations.
- Modify `scripts/configure-codex-meridian-mcp.ps1`: update exact environment keys without replacing unrelated settings.
- Create `scripts/run-provenance-integrity-integration.ps1`: owned Windows live fixture.
- Create `scripts/test-provenance-evidence-validation.ps1`: evidence privacy/schema validation.
- Modify `.github/workflows/{ci,byond-integration}.yml`: portable and Windows live gates.
- Modify `tests/workflow_contract.rs`: enforce exact workflow commands and evidence retention.
- Modify `tests/documentation.rs` and `tests/compatibility_manifest.rs`.
- Modify `README.md`, `TESTING.md`, `docs/{architecture,security,compatibility,provenance,native-evidence}.md`.
- Modify `tests/compatibility/meridian-rift.json`, `spacemandmm-capabilities.json`, and generated `docs/tool-contracts.md` only from verified implementation state.

### Task 1: Configure repositories and private state safely

**Files:**
- Modify: `scripts/install-meridian-mcp.ps1`
- Modify: `scripts/configure-codex-meridian-mcp.ps1`
- Modify: `tests/documentation.rs`
- Modify: `tests/workflow_contract.rs`

**Interfaces:**
- Consumes: installed binary, existing Codex config, explicit workspace roots, explicit repository roots, and optional private state path.
- Produces: server environment containing `MERIDIAN_MCP_ROOTS`, `MERIDIAN_MCP_REPOSITORIES`, and development-only `MERIDIAN_MCP_STATE_DIR` without rewriting unrelated configuration.

- [ ] **Step 1: Write failing script-contract assertions**

```rust
for required in [
	"[string[]]$RepositoryRoots",
	"[string]$StateDirectory",
	"MERIDIAN_MCP_REPOSITORIES",
	"MERIDIAN_MCP_STATE_DIR",
	"Test-Path -LiteralPath",
] {
	assert!(configure.contains(required), "configure script is missing {required}");
}
```

Also assert the scripts do not contain `Remove-Item -Recurse` against workspace, state, or profile
roots and do not replace the entire `mcp_servers` table.

- [ ] **Step 2: Run documentation/workflow tests and confirm missing parameters**

```powershell
cargo +1.95.0 test --test documentation --test workflow_contract
```

Expected: assertions fail because the new configuration keys are absent.

- [ ] **Step 3: Add explicit installer parameters and validation**

```powershell
param(
	[string[]]$WorkspaceRoots,
	[string[]]$RepositoryRoots = @(),
	[string]$StateDirectory,
	[switch]$Development
)
```

Resolve each path with `GetFullPath`, require workspace/repository directories to exist, and reject a
state directory below any workspace root. Create only the exact state directory with `New-Item
-ItemType Directory -Force`; do not recursively clean an existing directory.

- [ ] **Step 4: Update one named Codex server entry**

Use the script's existing TOML editing strategy. Preserve all unrelated servers and unrelated fields
in the selected server. Set `MERIDIAN_MCP_STATE_DIR` only for development mode. Remove stale legacy
values for these exact three keys only when the caller explicitly supplies replacement values.

- [ ] **Step 5: Add a script-level round-trip fixture**

Start from a temporary TOML containing another MCP server and extra selected-server environment keys.
Run the configurator, parse the result, and assert the other server/keys remain byte-equivalent while
the three Meridian keys contain platform path-list separators.

- [ ] **Step 6: Run script-contract tests**

```powershell
cargo +1.95.0 test --test documentation --test workflow_contract
```

Expected: configuration keys, preservation, path validation, and no-destructive-cleanup assertions
pass.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add scripts/install-meridian-mcp.ps1 scripts/configure-codex-meridian-mcp.ps1 tests/documentation.rs tests/workflow_contract.rs
git commit -m "feat: configure Meridian-MCP provenance state"
```

### Task 2: Build the owned Windows provenance and integrity live fixture

**Files:**
- Create: `scripts/run-provenance-integrity-integration.ps1`
- Create: `scripts/test-provenance-evidence-validation.ps1`
- Modify: `tests/fixtures/provenance/fixture.dm`
- Create: `tests/fixtures/provenance/fixture.dme`
- Modify: `tests/fixtures/provenance/generated_bindings.dm`
- Modify: `tests/fixtures/provenance/fixture-manifest.json`
- Modify: `tests/workflow_contract.rs`

**Interfaces:**
- Consumes: exact release binary, BYOND 516.1687 DreamMaker path, owned fixture copy, and new private state directory.
- Produces: schema-1 bounded JSON evidence for sync, initial compile, changed-input stale rejection, failed compile stale retention, restored compile, launch, tracked mutation, and stop.

- [ ] **Step 1: Write failing workflow-contract requirements**

```rust
for required in [
	"dm_check_fixture_sync",
	"dm_compile",
	"stale_build_artifact",
	"require_verified_provenance",
	"source_integrity_warning",
	"process_stopped",
	"state_journal_finalized",
	"owned_processes_remaining",
] {
	assert!(script.contains(required), "live script is missing {required}");
}
```

Require the validator to reject absolute profile paths, credential-like keys, raw player identifiers,
and an evidence schema other than 1.

- [ ] **Step 2: Run the workflow test and confirm the script is absent**

```powershell
cargo +1.95.0 test --test workflow_contract
```

Expected: missing-script failure.

- [ ] **Step 3: Implement the isolated fixture sequence**

The PowerShell script creates one temporary owned directory and copies only
`tests/fixtures/provenance`. It initializes a local Git repository and commits the fixture so runtime
mutation is tracked. It creates a separate exact private state directory outside the fixture root.

Run one MCP stdio session with development mode, exact roots, state directory, and compiler allowlist.
Send requests in this order:

```text
dm_parse_environment
dm_check_fixture_sync
dm_compile
dm_run(require_verified_provenance=true)
dm_wait_for_output(MERIDIAN_INTEGRITY_PHASE_COMPLETE)
dm_stop
```

Require fresh build provenance, runtime warning naming the tracked artifact, and process stop success.

- [ ] **Step 4: Add changed-input and failed-compile stale sequences**

After the first compile, change the generated binding while leaving the DMB. Require fixture sync to
report the missing proc and `dm_run` to return `stale_build_artifact`. Then call `dm_compile`, require
compiler failure and `dmb_updated: false`, restart the MCP with the same private state, and require the
same stale rejection.

- [ ] **Step 5: Restore and verify a fresh compile can launch**

Restore the owned fixture copy from the script's in-memory original bytes, reparse, require sync
verified, compile successfully, launch, match the complete marker, and stop. Do not use `git checkout`
or reset even inside the fixture; the test must exercise explicit byte restoration.

- [ ] **Step 6: Write bounded evidence and validate privacy**

Evidence includes logical fixture ID, BYOND version, MCP build ID, step classifications, hashes,
warning codes, process exit/stop state, and state-record recovery outcome. It excludes raw stdout,
absolute paths, environment dumps, and private state documents. Run the validator before returning
success.

- [ ] **Step 7: Run PowerShell parser and owned-script validation without BYOND**

```powershell
$null = [scriptblock]::Create((Get-Content -LiteralPath ./scripts/run-provenance-integrity-integration.ps1 -Raw))
./scripts/test-provenance-evidence-validation.ps1
cargo +1.95.0 test --test workflow_contract
```

Expected: scripts parse, malicious evidence fixtures are rejected, and workflow contracts pass.

- [ ] **Step 8: Record the checkpoint if commits are authorized**

```powershell
git add scripts/run-provenance-integrity-integration.ps1 scripts/test-provenance-evidence-validation.ps1 tests/fixtures/provenance tests/workflow_contract.rs
git commit -m "test: add provenance and integrity live fixture"
```

### Task 3: Add portable Ubuntu gates and exact Rust commands

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `scripts/run-meridian-analysis-compatibility.ps1`
- Modify: `tests/workflow_contract.rs`

**Interfaces:**
- Consumes: Rust fixtures from Plans 1-4 and the existing real Meridian-Rift analysis checkout.
- Produces: required Ubuntu policy/proc/provenance/integrity/native-evidence gates without claiming live BYOND runtime support.

- [ ] **Step 1: Write failing exact-command workflow assertions**

```rust
for required in [
	"rustc --version --verbose",
	"cargo fmt --all -- --check",
	"cargo clippy --locked --all-targets --all-features -- -D warnings",
	"cargo test --locked --all-features",
	"cargo build --locked --release",
	"native_evidence_readers",
	"repository_roots",
	"build_provenance",
	"runtime_integrity",
] {
	assert!(workflow.contains(required), "CI workflow is missing {required}");
}
```

- [ ] **Step 2: Run workflow tests and identify current command differences**

```powershell
cargo +1.95.0 test --test workflow_contract
```

Expected: failures for current unqualified/unlocked command strings and missing named gates.

- [ ] **Step 3: Update standard CI commands**

Use repository `rust-toolchain.toml` as the installed authority and print `rustc --version --verbose`
immediately before commands. Add `--locked` to Clippy, tests, and release build. Keep cargo-deny on its
existing Linux job.

- [ ] **Step 4: Add explicit portable test steps**

The Ubuntu job runs the four named integration test binaries so failures identify the subsystem even
when the later all-feature suite also fails. Supply temporary `MERIDIAN_MCP_STATE_DIR` only to
development MCP subprocess fixtures, not globally to unrelated tests.

- [ ] **Step 5: Extend real Meridian-Rift analysis compatibility**

Have `run-meridian-analysis-compatibility.ps1` call `dm_server_status` before parse and record logical
effective-root sources, then exercise the child override fixture/query chosen for MMCP-PROF-018.
Normalize returned paths relative to the checked-out repository before writing public evidence.

- [ ] **Step 6: Run local workflow contracts and portable Rust tests**

```powershell
cargo +1.95.0 test --test workflow_contract --test repository_roots --test proc_resolution --test build_provenance --test runtime_integrity --test native_evidence_readers --test native_evidence_timeline --test native_evidence_summary --test native_evidence_comparison
```

Expected: every command exits 0 locally; hosted Ubuntu remains the platform authority.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add .github/workflows/ci.yml scripts/run-meridian-analysis-compatibility.ps1 tests/workflow_contract.rs
git commit -m "ci: verify provenance and evidence on Ubuntu"
```

### Task 4: Add the Windows live gate and retain failure evidence

**Files:**
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `tests/workflow_contract.rs`
- Modify: `scripts/run-meridian-compatibility.ps1`

**Interfaces:**
- Consumes: installed BYOND 516.1687, rebuilt release MCP, and the owned PowerShell fixture from Task 2.
- Produces: required Windows provenance/integrity evidence artifact and bounded failure diagnostics.

- [ ] **Step 1: Write failing BYOND workflow assertions**

Require one step named `Run managed provenance and integrity gate`, an evidence path under
`integration/evidence`, `if: always()` artifact upload, and inclusion in the existing final Windows
failure aggregator.

- [ ] **Step 2: Run workflow tests and confirm the lane is missing**

```powershell
cargo +1.95.0 test --test workflow_contract
```

Expected: missing lane and artifact assertions fail.

- [ ] **Step 3: Invoke the owned live script after rebuilding release**

```powershell
./scripts/run-provenance-integrity-integration.ps1 `
	-DreamMakerPath "$env:RUNNER_TEMP\byond\byond\bin\dm.exe" `
	-BinaryPath ./target/release/meridian-mcp.exe `
	-EvidencePath ./integration/evidence/provenance-integrity.json
```

Do not edit or call Meridian-Rift's human `BUILD.cmd` for this fixture. Keep the existing real
Meridian-Rift full-build gate separate.

- [ ] **Step 4: Retain bounded evidence on every failure**

The script writes a failure document in `finally`. Upload the JSON plus bounded compiler/runtime text
diagnostics, excluding DMB/RSC/native binaries, raw private state, and copied source. Add its pass/fail
state to the final Windows gate aggregator.

- [ ] **Step 5: Run workflow contracts and PowerShell validation**

```powershell
cargo +1.95.0 test --test workflow_contract
./scripts/test-provenance-evidence-validation.ps1
```

Expected: workflow structure and evidence boundaries pass locally.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add .github/workflows/byond-integration.yml tests/workflow_contract.rs scripts/run-meridian-compatibility.ps1
git commit -m "ci: qualify managed DreamMaker provenance"
```

### Task 5: Complete user, agent, and tool documentation

**Files:**
- Modify: `README.md`
- Modify: `TESTING.md`
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Modify: `docs/compatibility.md`
- Modify: `docs/provenance.md`
- Modify: `docs/native-evidence.md`
- Modify: `tests/documentation.rs`
- Regenerate: `docs/tool-contracts.md`

**Interfaces:**
- Consumes: final schemas and behavior from Plans 1-4.
- Produces: complete individual tool documentation and honest compatibility guidance.

- [ ] **Step 1: Add failing documentation coverage for every new/changed tool**

```rust
for required in [
	"dm_server_status",
	"dm_check_fixture_sync",
	"dm_native_evidence_summary",
	"dm_native_evidence_compare",
	"require_verified_provenance",
	"source_integrity_warning",
	"pre_game_cumulative",
] {
	assert!(readme.contains(required), "README is missing {required}");
}
```

The existing test requiring one table row per public tool remains authoritative.

- [ ] **Step 2: Expand README individual tool descriptions**

Each tool row explains accepted inputs, result purpose, mutations/process effects, support status,
and the required preceding call. Add concise workflows for worktree parsing, managed fixture compile,
standard runtime integrity, one-run evidence summary, and repeated-run comparison.

- [ ] **Step 3: Document operational and security boundaries**

`TESTING.md` contains exact local Rust, PowerShell parser, owned BYOND, and manual workflow-dispatch
commands. Security and architecture docs explain startup-only roots, state privacy, stale managed
launch refusal, unmanaged warnings, mutation/no-revert behavior, evidence limits, and redaction limits.

- [ ] **Step 4: Regenerate the checked-in tool reference**

```powershell
cargo +1.95.0 run --locked --bin render_tool_docs -- docs/tool-contracts.md
```

- [ ] **Step 5: Run documentation and link tests**

```powershell
cargo +1.95.0 test --test documentation --test tool_contracts --test capability_registry
```

Expected: every tool has an individual README row, all local links resolve, and generated contracts
match source.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add README.md TESTING.md docs/architecture.md docs/security.md docs/compatibility.md docs/provenance.md docs/native-evidence.md docs/tool-contracts.md tests/documentation.rs
git commit -m "docs: complete provenance and evidence guidance"
```

### Task 6: Promote only evidence-backed compatibility status

**Files:**
- Modify: `tests/compatibility/meridian-rift.json`
- Modify: `docs/compatibility.md`
- Modify: `spacemandmm-capabilities.json`
- Modify: `tests/compatibility_manifest.rs`
- Modify: `tests/capability_registry.rs`

**Interfaces:**
- Consumes: completed hosted Windows and Ubuntu run evidence from Tasks 3-4.
- Produces: exact platform-scoped status without converting fixture success into production acceptance.

- [ ] **Step 1: Add failing compatibility-manifest assertions**

Require separate records for:

```text
analysis_policy_and_proc_ownership / windows / verified
analysis_policy_and_proc_ownership / ubuntu / verified
managed_compile_run_provenance / windows / verified
managed_compile_run_provenance / ubuntu / fixture_verified
standard_runtime_integrity / windows / verified
standard_runtime_integrity / ubuntu / fixture_verified
native_evidence_readers / windows / fixture_verified
native_evidence_readers / ubuntu / fixture_verified
```

- [ ] **Step 2: Run manifest tests and confirm the records are absent**

```powershell
cargo +1.95.0 test --test compatibility_manifest --test capability_registry
```

Expected: missing capability/platform records.

- [ ] **Step 3: Apply the promotion table only after hosted evidence is green**

Windows live records require the BYOND workflow artifact. Ubuntu live BYOND fields remain
`fixture_verified` unless the Ubuntu live runtime lane independently passes the same semantic gate.
Native evidence remains `Experimental` even with parser fixtures because representative production
measurement acceptance is outside this implementation.

- [ ] **Step 4: Run compatibility and registry tests**

```powershell
cargo +1.95.0 test --test compatibility_manifest --test capability_registry --test documentation
```

Expected: exact independent status records pass with no broad “fully compatible” claim.

- [ ] **Step 5: Record the checkpoint if commits are authorized**

```powershell
git add tests/compatibility/meridian-rift.json docs/compatibility.md spacemandmm-capabilities.json tests/compatibility_manifest.rs tests/capability_registry.rs
git commit -m "docs: record provenance compatibility evidence"
```

### Task 7: Run final verification and hand off for restart

**Files:**
- Verify all changed files from Plans 1-5.
- Do not modify unrelated files during this task.

**Interfaces:**
- Consumes: the complete implementation.
- Produces: evidence-backed readiness for user commit/restart and hosted CI execution.

- [ ] **Step 1: Print exact toolchain and repository state**

```powershell
rustc +1.95.0 --version --verbose
git status --short --branch
```

- [ ] **Step 2: Run the full exact Rust and dependency gate**

```powershell
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.95.0 test --locked --all-features
cargo +1.95.0 build --locked --release
cargo +1.95.0 deny check --all-features
```

Expected: every command exits 0 under Rust 1.95.

- [ ] **Step 3: Run PowerShell syntax and evidence validation**

```powershell
Get-ChildItem ./scripts -Filter *.ps1 | ForEach-Object {
	$null = [scriptblock]::Create((Get-Content -LiteralPath $_.FullName -Raw))
}
./scripts/test-meridian-evidence-validation.ps1
./scripts/test-provenance-evidence-validation.ps1
```

Expected: every script parses and both validators pass.

- [ ] **Step 4: Run the local owned Windows BYOND gate when BYOND 516.1687 is installed**

```powershell
./scripts/run-provenance-integrity-integration.ps1 `
	-DreamMakerPath "C:\Program Files (x86)\BYOND\bin\dm.exe" `
	-BinaryPath ./target/release/meridian-mcp.exe `
	-EvidencePath ./integration/evidence/local-provenance-integrity.json
```

Expected: sync, compile, stale rejection across restart, fresh launch, mutation warning, and stop all
pass. If BYOND is unavailable, report this exact gate untested rather than inferring success.

- [ ] **Step 5: Run generated-file and hygiene gates**

```powershell
cargo +1.95.0 run --locked --bin render_tool_docs -- docs/tool-contracts.md
cargo +1.95.0 test --test tool_contracts --test workflow_contract --test documentation --test compatibility_manifest --test capability_registry
git diff --check
git grep -n -I -E "[A-Za-z]:\\\\Users\\\\|/home/[^/]+|/Users/[^/]+" -- README.md TESTING.md docs scripts tests src
```

Expected: generated docs are unchanged after regeneration, tests pass, whitespace is clean, and no
machine profile path appears.

- [ ] **Step 6: Inspect all remaining changes without committing**

```powershell
git status --short
git diff --stat
git diff --name-only
```

Expected: only approved implementation, tests, workflows, docs, spec, and plans remain. Leave them in
the working tree unless the user explicitly asks for commits.

- [ ] **Step 7: If commits were explicitly authorized, record the approved design and plan set**

```powershell
git add docs/superpowers/specs/2026-08-27-meridian-mcp-provenance-and-native-evidence-design.md
git add docs/superpowers/plans/2026-08-27-analysis-correctness-and-policy-visibility.md
git add docs/superpowers/plans/2026-08-27-compile-fixture-provenance.md
git add docs/superpowers/plans/2026-08-27-standard-runtime-integrity.md
git add docs/superpowers/plans/2026-08-27-native-evidence-ingestion.md
git add docs/superpowers/plans/2026-08-27-provenance-evidence-compatibility-release.md
git commit -m "docs: plan provenance and evidence qualification"
```

Do not push without separate explicit authorization. Tell the user to restart Codex only after the
release binary is rebuilt and the active MCP configuration contains the new state and repository
environment values.
