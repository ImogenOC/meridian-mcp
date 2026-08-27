# Standard Runtime Integrity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Detect, journal, and report tracked workspace mutations produced during ordinary `dm_run` sessions without reverting them or obscuring process-stop success.

**Architecture:** Generalized integrity identities establish a launch baseline. A focused `RuntimeIntegritySession` owns the private journal, five-second bounded monitor, exact owned-path exemptions, and mutation observations. Standard runtime tools refresh and finalize that session at explicit lifecycle boundaries while preserving the existing DreamDaemon child ownership model.

**Tech Stack:** Rust 2021, Rust 1.95, Tokio tasks/watch channels, fixed Git subprocesses, SHA-256, existing private state and atomic JSON store, PowerShell for BYOND integration.

**Spec:** `docs/superpowers/specs/2026-08-27-meridian-mcp-provenance-and-native-evidence-design.md`

## Global Constraints

- Capture the integrity baseline before spawning DreamDaemon.
- Existing dirty state is permitted but remains protected against further session changes.
- Git workspaces report Git object identity plus working-tree SHA-256 and size.
- Non-Git workspaces retain the bounded manifest fallback.
- The fixed monitor interval is five seconds and is not caller-configurable initially.
- Every mutation is associated with the first observation and nearest preceding output sequence.
- Exact MCP-owned paths may be exempted; directories, globs, and retroactive exemptions are forbidden.
- `dm_stop` always attempts owned process termination before returning integrity failure or warning.
- Never restore, checkout, delete, rewrite, or stage a workspace file.
- Journal files live only in `MERIDIAN_MCP_STATE_DIR`.
- Commit steps require explicit user authorization during execution.

---

## Locked file structure

- Modify `src/workspace_integrity.rs`: richer file identities, pure delta classification, reusable journal document.
- Create `src/runtime_integrity.rs`: standard runtime session, monitor, checkpoint, finalization, and recovery.
- Modify `src/state.rs`: timestamped output observations and standard integrity session ownership.
- Modify `src/tools/runtime.rs`: baseline-before-spawn, lifecycle refresh, structured results.
- Modify `src/contracts.rs`: accurately mark standard runtime status/wait/stop state writes.
- Modify `src/server.rs`: startup recovery inventory.
- Create `tests/runtime_integrity.rs`: pure and async session coverage.
- Modify `tests/{workspace_integrity,runtime_tools,tool_contracts}.rs`.
- Modify `tests/fixtures/runtime/runtime.dm`: optional tracked mutation and phase markers.

### Task 1: Generalize integrity file identities and deltas

**Files:**
- Modify: `src/workspace_integrity.rs`
- Modify: `src/lib.rs`
- Modify: `tests/workspace_integrity.rs`

**Interfaces:**
- Consumes: canonical protected root and exact owned paths.
- Produces: `FileIdentity`, `WorkspaceSnapshot`, `IntegrityDelta`, and `compare_snapshots` independent of Tracy.

- [ ] **Step 1: Write failing modified-dirty and deletion tests**

```rust
#[test]
fn a_preexisting_dirty_file_changed_again_is_a_session_mutation() {
	let fixture = GitIntegrityFixture::new();
	fixture.write("tracked.txt", "dirty before launch\n");
	let baseline = WorkspaceSnapshot::capture(&fixture.root).unwrap();
	fixture.write("tracked.txt", "changed during runtime\n");
	let current = WorkspaceSnapshot::capture(&fixture.root).unwrap();
	let delta = compare_snapshots(&baseline, &current, &[]).unwrap();
	assert_eq!(delta.modified[0].relative_path, "tracked.txt");
	assert_ne!(delta.modified[0].before.sha256, delta.modified[0].after.sha256);
}
```

Add separate tests for tracked deletion, rename representation, untracked addition, exact owned-file
exemption, and rejection of directory exemptions.

- [ ] **Step 2: Run the integrity tests and confirm the richer model is missing**

```powershell
cargo +1.95.0 test --test workspace_integrity
```

Expected: compilation fails because `WorkspaceSnapshot` and `compare_snapshots` do not exist.

- [ ] **Step 3: Define stable identities and delta records**

```rust
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FileIdentity {
	pub tracked: bool,
	pub git_object_kind: Option<String>,
	pub git_object_id: Option<String>,
	pub sha256: Option<String>,
	pub size: Option<u64>,
	pub status: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PathMutation {
	pub relative_path: String,
	pub change_kind: MutationKind,
	pub before: FileIdentity,
	pub after: FileIdentity,
}
```

Use `git ls-files -s -z` for tracked index identities and fixed `git status --porcelain=v2 -z
--untracked-files=all` for state. Hash worktree bytes with the existing 64 KiB streaming buffer. Keep
the current 250,000-entry and 16 GiB fallback limits.

- [ ] **Step 4: Implement pure comparison and compatibility adapters**

```rust
pub fn compare_snapshots(
	baseline: &WorkspaceSnapshot,
	current: &WorkspaceSnapshot,
	owned_paths: &[PathBuf],
) -> Result<IntegrityDelta, IntegrityError>;
```

Adapt `IntegrityBaseline::capture` and `checkpoint` to delegate to the new snapshot/delta types so the
existing Tracy call sites retain behavior. Do not change Tracy journal schema in this task.

- [ ] **Step 5: Run integrity and Tracy artifact tests**

```powershell
cargo +1.95.0 test --test workspace_integrity --test tracy_tools --test tracy_artifacts
```

Expected: richer standard identities pass and Tracy compatibility remains green.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/workspace_integrity.rs src/lib.rs tests/workspace_integrity.rs
git commit -m "refactor: generalize workspace integrity identities"
```

### Task 2: Add timestamped runtime output observations

**Files:**
- Modify: `src/state.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `tests/runtime_tools.rs`

**Interfaces:**
- Consumes: captured stdout, stderr, and log-file lines.
- Produces: `RuntimeOutputEntry { sequence, monotonic_offset_ms, text }`, compatibility `recent_output`, and `nearest_output_before`.

- [ ] **Step 1: Write failing sequence and nearest-marker tests**

```rust
push_output_line_at(&log, 100, "phase-start".to_owned());
push_output_line_at(&log, 250, "phase-complete".to_owned());
let entry = nearest_output_before(&log, 200).unwrap();
assert_eq!(entry.sequence, 1);
assert_eq!(entry.text, "phase-start");
```

Retain tests proving the 500-line, 16 KiB-per-line, and 1 MiB-total limits.

- [ ] **Step 2: Run runtime tests and confirm output entries are plain strings**

```powershell
cargo +1.95.0 test --test runtime_tools
```

Expected: compilation fails because timestamped output APIs are missing.

- [ ] **Step 3: Replace the internal log element while preserving response shape**

```rust
#[derive(Clone, Debug, Serialize)]
pub struct RuntimeOutputEntry {
	pub sequence: u64,
	pub monotonic_offset_ms: u64,
	pub text: String,
}
```

Replace the alias with `OutputLog = Arc<StdMutex<RuntimeOutputBuffer>>`. The buffer owns its bounded
entry deque, session start instant, and next sequence number so every cloned capture-task handle can
append a complete entry atomically. Existing `recent_output(count)` still returns `Vec<String>` for
protocol compatibility. Add `recent_output_entries(count)` for integrity evidence.

```rust
pub struct RuntimeOutputBuffer {
	entries: VecDeque<RuntimeOutputEntry>,
	started_at: Instant,
	next_sequence: u64,
}
```

- [ ] **Step 4: Route all three capture paths through one entry function**

Pass the session start instant into stdout, stderr, and log-file tasks. Preserve LF/CRLF normalization
and truncation. The same line observed through two different channels remains two observations; do not
attempt content deduplication.

- [ ] **Step 5: Run runtime and state tests**

```powershell
cargo +1.95.0 test --test runtime_tools
cargo +1.95.0 test state::tests
```

Expected: compatibility strings, structured entries, bounds, and nearest-marker selection pass.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/state.rs src/tools/runtime.rs tests/runtime_tools.rs
git commit -m "feat: timestamp owned runtime output"
```

### Task 3: Implement the standard runtime integrity session

**Files:**
- Create: `src/runtime_integrity.rs`
- Modify: `src/lib.rs`
- Create: `tests/runtime_integrity.rs`

**Interfaces:**
- Consumes: `PrivateStateStore`, protected root, launch provenance, output log, and exact owned paths.
- Produces: `RuntimeIntegritySession::create`, `spawn_monitor`, `checkpoint`, `finalize`, and `recover_unfinished`.

- [ ] **Step 1: Write failing async mutation-observation tests**

```rust
#[tokio::test]
async fn monitor_records_first_mutation_and_nearest_marker() {
	let fixture = RuntimeIntegrityFixture::new();
	let mut session = fixture.start_session().unwrap();
	fixture.output(400, "runtime phase: preview generation");
	fixture.write_tracked("tracked.dmi", b"changed");
	session.observe_now().await.unwrap();

	let event = &session.summary().events[0];
	assert_eq!(event.relative_path, "tracked.dmi");
	assert_eq!(event.nearest_output.as_ref().unwrap().text, "runtime phase: preview generation");
}
```

Also test exact owned output exclusion, later changes preserving the first observation, clean
finalization, active-journal recovery, and no filesystem repair.

- [ ] **Step 2: Run the focused test and confirm the session is missing**

```powershell
cargo +1.95.0 test --test runtime_integrity
```

Expected: compilation fails because `RuntimeIntegritySession` does not exist.

- [ ] **Step 3: Define the journal and event schema**

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeIntegrityEvent {
	pub relative_path: String,
	pub change_kind: MutationKind,
	pub before: FileIdentity,
	pub after: FileIdentity,
	pub first_observed_offset_ms: u64,
	pub nearest_output: Option<RuntimeOutputEntry>,
	pub session_id: String,
	pub process_id: Option<u32>,
	pub build_record_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RuntimeIntegrityJournal {
	pub schema: u32,
	pub session_id: String,
	pub status: RuntimeIntegrityStatus,
	pub protected_root: PathBuf,
	pub baseline: WorkspaceSnapshot,
	pub events: Vec<RuntimeIntegrityEvent>,
	pub last_action: String,
}
```

Cap journal bytes at 32 MiB and events at 10,000. Store under
`runtime-integrity/<session-id>.json` in private state.

- [ ] **Step 4: Implement bounded observation and monitor control**

```rust
pub fn spawn_monitor(
	session: Arc<Mutex<RuntimeIntegritySession>>,
	mut stop: watch::Receiver<bool>,
) -> JoinHandle<()>;
```

Use a fixed five-second Tokio interval with missed-tick behavior `Delay`. `observe_now` runs blocking
Git/file work in `spawn_blocking`. Record only the first event per `(relative_path, change_kind)` and
update current identity separately for the final summary.

- [ ] **Step 5: Implement finalization and recovery**

`finalize` stops the monitor, performs one last observation, sets `finalized_clean`,
`finalized_with_changes`, or `finalized_with_violation`, atomically writes, reopens, and returns a
summary. `recover_unfinished` never attributes post-MCP changes to the old process; it marks them
`observed_during_recovery`.

- [ ] **Step 6: Run runtime-integrity tests**

```powershell
cargo +1.95.0 test --test runtime_integrity --test workspace_integrity
```

Expected: observation, finalization, recovery, limits, and no-repair assertions pass.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/runtime_integrity.rs src/lib.rs tests/runtime_integrity.rs
git commit -m "feat: journal standard runtime integrity"
```

### Task 4: Capture the baseline before standard runtime spawn

**Files:**
- Modify: `src/state.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `tests/runtime_tools.rs`

**Interfaces:**
- Consumes: launch provenance from Plan 2, matching analysis project root, private state, output log.
- Produces: standard runtime session with active integrity monitor before DreamDaemon starts.

- [ ] **Step 1: Write failing ordering and scope tests**

Instrument a test spawn adapter to assert this call order:

```text
provenance_revalidated
integrity_baseline_captured
journal_persisted
process_spawned
monitor_started
```

Also require matching snapshot root selection and DMB-parent fallback for unmanaged fixtures.

- [ ] **Step 2: Run runtime tests and confirm there is no pre-spawn journal**

```powershell
cargo +1.95.0 test --test runtime_tools --test runtime_integrity
```

Expected: ordering assertion fails.

- [ ] **Step 3: Add standard integrity state to `RuntimeState`**

```rust
pub(crate) integrity: Option<Arc<Mutex<RuntimeIntegritySession>>>,
pub(crate) integrity_stop: Option<watch::Sender<bool>>,
pub(crate) integrity_task: Option<JoinHandle<()>>,
pub(crate) launch_provenance: Option<LaunchProvenance>,
```

Clear these only after finalization. `clear_runtime_diagnostics` must not erase an unfinished prior
journal without recovery.

- [ ] **Step 4: Create the session before `Command::spawn`**

Select the active project root only when its DME output corresponds to the canonical DMB. Otherwise
use the DMB parent. Capture and persist the baseline before constructing/spawning the child. If scope
capture fails, return `integrity_scope_too_large` or the specific I/O error and do not spawn.

- [ ] **Step 5: Checkpoint launch readiness**

After the 250 ms process check and optional output readiness wait, call `observe_now`, record
`launch_ready` or `launch_failed`, and include `integrity_session_id` and current summary in the
result. On launch failure, stop the process first, then finalize integrity.

- [ ] **Step 6: Run ordering and runtime tests**

```powershell
cargo +1.95.0 test --test runtime_tools --test runtime_integrity
```

Expected: process spawn is impossible before journal persistence.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/state.rs src/tools/runtime.rs tests/runtime_tools.rs
git commit -m "feat: guard standard runtime launch integrity"
```

### Task 5: Surface integrity from wait, status, stop, and recovery

**Files:**
- Modify: `src/tools/runtime.rs`
- Modify: `src/state.rs`
- Modify: `src/server.rs`
- Modify: `src/contracts.rs`
- Modify: `tests/runtime_tools.rs`
- Modify: `tests/runtime_integrity.rs`
- Modify: `tests/tool_contracts.rs`

**Interfaces:**
- Consumes: active or recoverable `RuntimeIntegritySession`.
- Produces: consistent `integrity` summaries and warnings from every standard runtime lifecycle tool.

- [ ] **Step 1: Write failing stop-warning and deletion-violation tests**

```rust
assert_eq!(payload["success"], true);
assert_eq!(payload["process_stopped"], true);
assert_eq!(payload["warnings"][0]["code"], "source_integrity_warning");
assert_eq!(payload["warnings"][0]["relative_path"], "tracked.dmi");
```

For deletion, require a tool error containing `workspace_integrity_violation` and
`process_stopped: true`. Verify the file remains deleted after the response.

- [ ] **Step 2: Run runtime tests and confirm mutations are absent**

```powershell
cargo +1.95.0 test --test runtime_tools --test runtime_integrity --test tool_contracts
```

Expected: response assertions and effect-contract assertions fail.

- [ ] **Step 3: Add one lifecycle refresh helper**

```rust
async fn refresh_standard_runtime(
	state: &mut RuntimeState,
	action: &'static str,
) -> Result<RuntimeIntegritySummary>;
```

If the child exited naturally, stop output tasks, stop the monitor, finalize once, retain the exit
code and final summary, and clear only active handles. Repeated refresh returns the retained terminal
summary.

- [ ] **Step 4: Update tool response semantics**

`dm_wait_for_output` and `dm_status` include current integrity warnings. `dm_stop` terminates first,
then finalizes. Modified/added tracked paths are successful warnings; deletion, rename loss, or
unowned cleanup attempts make the result an error after termination. Every response includes session
and launch provenance identity.

- [ ] **Step 5: Correct contract effects**

Introduce a `RUNTIME_STATE` effect with file reads/writes but no process spawn or network. Apply it to
`dm_wait_for_output`, `dm_stop`, and `dm_status` because any can finalize a durable journal. Keep
`dm_run` as `RUNTIME`.

- [ ] **Step 6: Inventory unfinished journals at server startup**

Load bounded unfinished standard journals into a recovery summary during `MeridianServer::new`.
Expose them through `dm_server_status`. Run `recover_unfinished` only when the protected root remains
an effective root; otherwise report `integrity_recovery_required` without reading outside policy.

- [ ] **Step 7: Run lifecycle and contract tests**

```powershell
cargo +1.95.0 test --test runtime_tools --test runtime_integrity --test tool_contracts --test mcp_conformance
```

Expected: natural exit, explicit stop, repeated stop, warning, violation, and recovery all pass.

- [ ] **Step 8: Record the checkpoint if commits are authorized**

```powershell
git add src/tools/runtime.rs src/state.rs src/server.rs src/contracts.rs tests/runtime_tools.rs tests/runtime_integrity.rs tests/tool_contracts.rs
git commit -m "feat: surface standard runtime integrity"
```

### Task 6: Add the owned mutation fixture and verify Plan 3

**Files:**
- Modify: `tests/fixtures/runtime/runtime.dm`
- Modify: `tests/fixtures/runtime/runtime.dme`
- Modify: `tests/runtime_tools.rs`
- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Modify: `tests/documentation.rs`
- Regenerate: `docs/tool-contracts.md`

**Interfaces:**
- Consumes: all Plan 3 behavior.
- Produces: fixture-verified MMCP-PROF-019 implementation ready for the Plan 5 live gate.

- [ ] **Step 1: Add deterministic runtime markers and mutation behavior**

```dm
/world/New()
	. = ..()
	world.log << "MERIDIAN_INTEGRITY_PHASE_START"
	text2file("changed during runtime", "tracked-runtime-artifact.txt")
	world.log << "MERIDIAN_INTEGRITY_PHASE_COMPLETE"
```

The Rust/PowerShell fixture setup initializes and commits `tracked-runtime-artifact.txt` before
launch. The DM fixture is technical test content, not production game content.

- [ ] **Step 2: Run the fixture test and require the closest marker**

```powershell
cargo +1.95.0 test --test runtime_tools runtime_mutation_reports_nearest_marker
```

Expected: the event names `tracked-runtime-artifact.txt` and the preceding phase-start marker.

- [ ] **Step 3: Update documentation and contract reference**

Document journal location, mutation versus violation semantics, no-revert rule, exact owned paths,
five-second observation granularity, natural-exit recovery, and privacy boundaries.

```powershell
cargo +1.95.0 run --locked --bin render_tool_docs -- docs/tool-contracts.md
```

- [ ] **Step 4: Run the exact Plan 3 gate**

```powershell
rustc +1.95.0 --version
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.95.0 test --locked --test workspace_integrity --test runtime_integrity --test runtime_tools --test tracy_tools --test tracy_artifacts --test tool_contracts --test mcp_conformance --test documentation
git diff --check
```

Expected: every command exits 0 and existing Tracy integrity behavior remains green.

- [ ] **Step 5: Confirm no test repaired or staged its mutation target**

```powershell
git status --short
git diff --stat
```

Expected: only planned source, test, plan, and documentation changes exist.

- [ ] **Step 6: Record the Plan 3 checkpoint if commits are authorized**

```powershell
git add tests/fixtures/runtime/runtime.dm tests/fixtures/runtime/runtime.dme tests/runtime_tools.rs README.md docs/architecture.md docs/security.md docs/tool-contracts.md tests/documentation.rs
git commit -m "test: verify standard runtime integrity reporting"
```
