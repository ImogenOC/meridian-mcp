# Compile and Fixture Provenance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist hash-bound build and fixture identity, detect stale managed artifacts after input changes or failed compilation, and refuse stale managed launches.

**Architecture:** A startup-owned private state store persists atomic schema-versioned records outside game workspaces. A declarative fixture manifest supplements the parsed DreamMaker source closure with generated bindings, native modules, services, and required proc contracts. Compile tools record verified successes and failed attempts; every runtime launcher uses one provenance gate immediately before process spawn.

**Tech Stack:** Rust 2021, Rust 1.95, serde/serde_json, SHA-256, SpacemanDMM analysis snapshots, existing atomic output and process runner, PowerShell for live BYOND gates.

**Spec:** `docs/superpowers/specs/2026-08-27-meridian-mcp-provenance-and-native-evidence-design.md`

## Global Constraints

- Development mode requires an existing writable `MERIDIAN_MCP_STATE_DIR` outside all game workspace roots.
- State records are local, atomic, bounded, schema-versioned, non-executable, and excluded from published evidence.
- A verified general build requires an active successful snapshot for the same canonical DME.
- Fixture manifests are declarative, contained, relative-path-only, and cannot contain globs, URLs, commands, arguments, or environment variables.
- A failed compile never deletes or overwrites the previous DMB.
- Known stale managed artifacts are refused without a tool-call override.
- Unmanaged human-built DMBs remain launchable with `provenance_status: unverified`; `require_verified_provenance=true` rejects them.
- Revalidate inputs and output immediately before spawn.
- Use PowerShell for all live BYOND compilation and runtime testing.
- Commit steps are conditional on explicit user authorization at execution time.

---

## Locked file structure

- Create `src/private_state.rs`: private state layout, locking, atomic JSON reads/writes, and startup recovery scan.
- Create `src/fixture_manifest.rs`: schema-1 manifest model, containment, canonicalization, hashing, and sync checks.
- Create `src/build_provenance.rs`: build records, failed-attempt records, stale comparison, and launch decisions.
- Create `src/tools/fixture.rs`: `dm_check_fixture_sync` adapter.
- Modify `src/config.rs`: require and expose development state directory.
- Modify `src/tools/{mod,compile,rift,runtime,debugger,tracy}.rs`: record builds and enforce launch decisions.
- Modify `src/{analysis_snapshot,artifact,contracts,parameters,server,state,lib}.rs`: source closure, artifact identity, schemas, and owned store.
- Create `tests/private_state.rs`, `tests/fixture_manifest.rs`, and `tests/build_provenance.rs`.
- Create `tests/support/mod.rs`: development-config fixture with an external private state lifetime.
- Modify every existing integration test that constructs development `ServerConfig` directly:
  `tests/{active_tool_policy,analysis_snapshot,compiler_runner,config_and_paths,dmi_analysis,fixture_corpus,language_capabilities,map_capabilities,mcp_conformance}.rs`.
- Modify `tests/{compiler_runner,rift_compile,runtime_tools,tracy_tools,active_tool_policy,mcp_conformance}.rs`.
- Create `tests/fixtures/provenance/fixture-manifest.json` and fixture source/generated/native/service files.

### Task 1: Add the development private state store

**Files:**
- Create: `src/private_state.rs`
- Modify: `src/config.rs`
- Modify: `src/server.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/lib.rs`
- Create: `tests/private_state.rs`
- Modify: `tests/config_and_paths.rs`
- Create: `tests/support/mod.rs`
- Modify: `tests/active_tool_policy.rs`
- Modify: `tests/analysis_snapshot.rs`
- Modify: `tests/compiler_runner.rs`
- Modify: `tests/dmi_analysis.rs`
- Modify: `tests/fixture_corpus.rs`
- Modify: `tests/language_capabilities.rs`
- Modify: `tests/map_capabilities.rs`
- Modify: `tests/mcp_conformance.rs`

**Interfaces:**
- Consumes: startup mode, effective workspace roots, and `MERIDIAN_MCP_STATE_DIR`.
- Produces: `PrivateStateStore::open(path: &Path, workspace_roots: &[EffectiveRoot]) -> Result<Self>`, `read_json<T>`, `write_json_atomic<T>`, and bounded record enumeration.

- [ ] **Step 1: Write failing state-boundary tests**

```rust
#[test]
fn development_state_must_be_outside_workspace_and_writable() {
	let fixture = StateFixture::new();
	assert!(PrivateStateStore::open(&fixture.state, &fixture.roots).is_ok());
	assert!(PrivateStateStore::open(&fixture.workspace.join("state"), &fixture.roots).is_err());
}

#[test]
fn atomic_records_survive_reopen() {
	let fixture = StateFixture::new();
	let store = PrivateStateStore::open(&fixture.state, &fixture.roots).unwrap();
	store.write_json_atomic("builds/example.json", &json!({"schema": 1})).unwrap();
	drop(store);
	let reopened = PrivateStateStore::open(&fixture.state, &fixture.roots).unwrap();
	assert_eq!(reopened.read_json::<Value>("builds/example.json").unwrap()["schema"], 1);
}
```

- [ ] **Step 2: Run focused tests and confirm the store is missing**

```powershell
cargo +1.95.0 test --test private_state --test config_and_paths
```

Expected: compilation fails because `PrivateStateStore` is undefined.

- [ ] **Step 3: Implement the private state layout and atomic writer**

```rust
pub struct PrivateStateStore {
	root: PathBuf,
}

impl PrivateStateStore {
	pub fn write_json_atomic<T: Serialize>(&self, relative: &str, value: &T) -> Result<PathBuf>;
	pub fn read_json<T: DeserializeOwned>(&self, relative: &str) -> Result<T>;
	pub fn list_records(&self, namespace: &str, max_entries: usize) -> Result<Vec<PathBuf>>;
}
```

Reject absolute record names, `..`, symlinks/reparse-point escapes, non-regular existing records, more
than 100,000 records, and serialized documents over 8 MiB. Write to an exclusive same-directory
temporary file, flush, close, rename, reopen, and deserialize before reporting success.

- [ ] **Step 4: Add configuration without breaking analysis mode**

`ServerConfig::from_env()` requires `MERIDIAN_MCP_STATE_DIR` only when mode is `development`.
Analysis mode accepts no state directory and performs no state writes. Add
`ServerConfig::from_values_with_state` for explicit test construction. Change development-mode tests
to use one shared fixture that creates a unique state directory outside every workspace root and
removes only that exact owned directory in `Drop` after the server/config lifetime ends.

```rust
let state_directory = match mode {
	CapabilityMode::Development => Some(required_state_directory_from_env()?),
	CapabilityMode::Analysis => None,
};
```

```rust
pub struct DevelopmentConfigFixture {
	pub config: ServerConfig,
	state_directory: PathBuf,
}

pub fn development_config(workspace_roots: Vec<PathBuf>) -> DevelopmentConfigFixture;
```

- [ ] **Step 5: Store the opened state handle in `ToolExecutionContext`**

Use `Arc<PrivateStateStore>` and expose `private_state() -> Option<&PrivateStateStore>`. Do not put
the state store in `ServerState`; it is immutable configuration, not session state.

- [ ] **Step 6: Run focused tests**

```powershell
cargo +1.95.0 test --test private_state --test config_and_paths --test active_tool_policy
```

Expected: atomic reopen, workspace exclusion, traversal rejection, and mode requirements pass.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/private_state.rs src/config.rs src/server.rs src/tools/mod.rs src/lib.rs tests/support/mod.rs tests/private_state.rs tests/config_and_paths.rs tests/active_tool_policy.rs tests/analysis_snapshot.rs tests/compiler_runner.rs tests/dmi_analysis.rs tests/fixture_corpus.rs tests/language_capabilities.rs tests/map_capabilities.rs tests/mcp_conformance.rs
git commit -m "feat: add private Meridian-MCP state store"
```

### Task 2: Parse and validate fixture manifests

**Files:**
- Create: `src/fixture_manifest.rs`
- Modify: `src/lib.rs`
- Create: `tests/fixture_manifest.rs`
- Create: `tests/fixtures/provenance/fixture-manifest.json`
- Create: `tests/fixtures/provenance/fixture.dm`
- Create: `tests/fixtures/provenance/generated_bindings.dm`
- Create: `tests/fixtures/provenance/native_module.bin`
- Create: `tests/fixtures/provenance/service.bin`

**Interfaces:**
- Consumes: contained manifest path and `PathPolicy`.
- Produces: `FixtureManifest::load(policy: &PathPolicy, path: &Path) -> Result<VerifiedFixtureManifest, FixtureManifestError>` and exact input records.

- [ ] **Step 1: Write the schema-1 fixture and failing validation tests**

```json
{
  "schema": 1,
  "fixture_id": "owned-provenance-fixture",
  "dme_path": "fixture.dme",
  "dmb_path": "fixture.dmb",
  "inputs": [
    {"path": "fixture.dm", "role": "source"},
    {"path": "generated_bindings.dm", "role": "generated_binding"},
    {"path": "native_module.bin", "role": "native_module"},
    {"path": "service.bin", "role": "service_executable"}
  ],
  "required_procs": [
    {"path": "/proc/meridian_fixture_state_batch", "arguments": ["payload"]}
  ],
  "required_tokens": ["#define MERIDIAN_FIXTURE_PROTOCOL 4"]
}
```

Tests must reject `../escape`, absolute paths, duplicate normalized paths, unknown roles, unknown
fields, missing regular files, symlinks, glob characters, URLs, and more than the fixed entry limit.

- [ ] **Step 2: Run the manifest tests and confirm failure**

```powershell
cargo +1.95.0 test --test fixture_manifest
```

Expected: compilation fails because the manifest module is absent.

- [ ] **Step 3: Implement strict manifest types**

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FixtureManifestDocument {
	pub schema: u32,
	pub fixture_id: String,
	pub dme_path: String,
	pub dmb_path: String,
	pub rsc_path: Option<String>,
	pub inputs: Vec<FixtureInputDocument>,
	pub required_procs: Vec<RequiredProcDocument>,
	pub required_tokens: Vec<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum FixtureInputRole {
	Source,
	GeneratedBinding,
	NativeModule,
	ServiceExecutable,
	Configuration,
}
```

Cap fixture IDs at 128 bytes, path strings at 4,096 bytes, inputs at 10,000, required procs at 1,000,
required tokens at 1,000, token length at 4,096, and manifest bytes at 4 MiB.

- [ ] **Step 4: Canonicalize and hash exact inputs**

```rust
pub struct VerifiedFixtureInput {
	pub relative_path: String,
	pub canonical_path: PathBuf,
	pub role: FixtureInputRole,
	pub size: u64,
	pub sha256: String,
}
```

Resolve each forward-slash relative path below the canonical manifest directory, then call
`PathPolicy::read_path`. Sort canonical records by `(role, relative_path)` before hashing the manifest
identity.

- [ ] **Step 5: Run fixture-manifest tests**

```powershell
cargo +1.95.0 test --test fixture_manifest
```

Expected: valid manifest is deterministic; all escape and ambiguity fixtures fail closed.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/fixture_manifest.rs src/lib.rs tests/fixture_manifest.rs tests/fixtures/provenance
git commit -m "feat: validate fixture provenance manifests"
```

### Task 3: Implement durable build records and stale decisions

**Files:**
- Create: `src/build_provenance.rs`
- Modify: `src/artifact.rs`
- Modify: `src/analysis_snapshot.rs`
- Modify: `src/lib.rs`
- Create: `tests/build_provenance.rs`

**Interfaces:**
- Consumes: `PrivateStateStore`, matching `AnalysisSnapshot`, optional `VerifiedFixtureManifest`, compiler identity, and `ArtifactSnapshot` outputs.
- Produces: `BuildProvenanceStore`, `BuildRecord`, `BuildAttempt`, `ProvenanceStatus`, `LaunchDecision`, and stable stale reasons.

- [ ] **Step 1: Write failing record/reopen/staleness tests**

```rust
#[test]
fn failed_attempt_makes_the_last_success_stale_across_reopen() {
	let fixture = ProvenanceFixture::new();
	let store = fixture.store();
	store.record_success(fixture.success()).unwrap();
	store.record_failure(fixture.failure("compiler_failed")).unwrap();
	drop(store);

	let decision = fixture.reopen().evaluate_launch(&fixture.dmb).unwrap();
	assert_eq!(decision.status, ProvenanceStatus::Stale);
	assert!(decision.reasons.iter().any(|reason| reason.code == "later_compile_failed"));
}
```

Add one test for each changed role plus changed DMB, RSC, manifest, and repository identity.

- [ ] **Step 2: Run the tests and confirm missing provenance types**

```powershell
cargo +1.95.0 test --test build_provenance
```

Expected: compilation fails because `BuildProvenanceStore` is undefined.

- [ ] **Step 3: Define schema-1 success and attempt records**

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildRecord {
	pub schema: u32,
	pub record_id: String,
	pub artifact_key: String,
	pub mcp_build: BuildIdentity,
	pub compiler: FileIdentity,
	pub project: ProjectBuildIdentity,
	pub inputs: Vec<BuildInputIdentity>,
	pub dmb: FileIdentity,
	pub rsc: Option<FileIdentity>,
	pub fixture_manifest_sha256: Option<String>,
	pub created_at_unix_ms: u128,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct BuildAttempt {
	pub schema: u32,
	pub attempt_id: String,
	pub artifact_key: String,
	pub outcome: BuildAttemptOutcome,
	pub observed_inputs: Vec<BuildInputIdentity>,
	pub retained_dmb_sha256: Option<String>,
	pub created_at_unix_ms: u128,
}
```

Use a cryptographically random record ID and Unix milliseconds only as metadata. The deterministic artifact
key is SHA-256 over local repository identity plus normalized root-relative DMB path.

- [ ] **Step 4: Expose the parsed source closure**

Add `AnalysisSnapshot::source_inputs() -> &[PathBuf]` containing sorted canonical parsed DM/DME and
configuration files. Ensure every path is below the snapshot project root. Do not infer DLL or service
paths from source strings.

- [ ] **Step 5: Implement launch evaluation**

```rust
pub enum ProvenanceStatus {
	Verified,
	Unverified,
	Stale,
}

pub struct LaunchDecision {
	pub status: ProvenanceStatus,
	pub record_id: Option<String>,
	pub reasons: Vec<ProvenanceReason>,
}

pub fn evaluate_launch(
	&self,
	dmb_path: &Path,
	require_verified: bool,
) -> Result<LaunchDecision, BuildProvenanceError>;
```

Construct `BuildProvenanceStore::new(Arc<PrivateStateStore>, PathPolicy)` during server startup and
retain it as `Arc<BuildProvenanceStore>` in `ToolExecutionContext`. Expose a crate-private
`build_provenance()` getter for compile, fixture, runtime, debugger, Tracy, and native-evidence tools.

Return `Unverified` for no record unless `require_verified` is true. Return `Stale` for any known
managed mismatch regardless of `require_verified`. Re-hash current files during every evaluation.

- [ ] **Step 6: Run provenance tests**

```powershell
cargo +1.95.0 test --test build_provenance --test analysis_snapshot
```

Expected: success, failure, changed-input, changed-output, unmanaged, and restart cases pass.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/build_provenance.rs src/artifact.rs src/analysis_snapshot.rs src/lib.rs tests/build_provenance.rs
git commit -m "feat: persist managed build provenance"
```

### Task 4: Record direct and Rift compile outcomes

**Files:**
- Modify: `src/tools/compile.rs`
- Modify: `src/tools/rift.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/parameters.rs`
- Modify: `tests/compiler_runner.rs`
- Modify: `tests/rift_compile.rs`

**Interfaces:**
- Consumes: matching snapshot, optional `fixture_manifest_path`, and `BuildProvenanceStore` from Task 3.
- Produces: verified success record or failed-attempt record in every managed compilation result.

- [ ] **Step 1: Write failing compile-result tests**

```rust
assert_eq!(payload["provenance_status"], "verified");
assert!(payload["build_record_id"].as_str().is_some());
assert_eq!(payload["dmb_updated"], true);
```

For a deliberate compiler failure with a retained old DMB, require `provenance_status: stale`,
`dmb_updated: false`, `retained_dmb_sha256`, and a persisted `later_compile_failed` reason.

- [ ] **Step 2: Run compile tests and confirm missing provenance fields**

```powershell
cargo +1.95.0 test --test compiler_runner --test rift_compile
```

Expected: assertions fail because compile tools do not record provenance.

- [ ] **Step 3: Add contained optional fixture-manifest parameters**

```rust
#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CompileParams {
	pub dme_path: PathBuf,
	pub compiler_path: Option<PathBuf>,
	pub working_directory: Option<PathBuf>,
	pub timeout_ms: Option<u64>,
	pub fixture_manifest_path: Option<PathBuf>,
}
```

Add the same optional manifest field to `RiftCompileParams`. Canonicalize it through
`PathPolicy::read_path` in `contain_arguments`.

- [ ] **Step 4: Pass context and state into direct compile**

Change dispatch from `compile::compile(args)` to:

```rust
compile::compile(context, state, args).await
```

Capture the matching snapshot before spawning. On success, verify fresh DMB/RSC snapshots and write
the success record. On compiler failure, timeout, idle timeout, or missing/frozen artifact, write a
failed attempt whenever a prior artifact key exists or a fixture manifest identifies the expected
artifact.

- [ ] **Step 5: Record Rift build evidence after semantic classification**

Only `BuildEvidence::FreshArtifacts` and `BuildEvidence::ValidCacheHit` may create a successful build
record. Cache hits still re-hash the current source closure and output artifacts. Every failure class
records a failed attempt without modifying outputs.

- [ ] **Step 6: Run compile tests**

```powershell
cargo +1.95.0 test --test compiler_runner --test rift_compile --test active_tool_policy
```

Expected: verified success, valid cache hit, unverified missing snapshot, and stale failed compile all
pass.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/tools/compile.rs src/tools/rift.rs src/tools/mod.rs src/parameters.rs tests/compiler_runner.rs tests/rift_compile.rs
git commit -m "feat: bind compile results to provenance"
```

### Task 5: Add `dm_check_fixture_sync`

**Files:**
- Create: `src/tools/fixture.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/contracts.rs`
- Modify: `src/parameters.rs`
- Modify: `spacemandmm-capabilities.json`
- Modify: `tests/fixture_manifest.rs`
- Modify: `tests/tool_contracts.rs`
- Modify: `tests/mcp_conformance.rs`

**Interfaces:**
- Consumes: contained fixture manifest, active snapshot when matching, fixture-only parse otherwise, and durable build record.
- Produces: analysis-mode read-only `dm_check_fixture_sync` with `verified`, `stale`, or `invalid` classification.

- [ ] **Step 1: Write the failing missing-proc test**

Remove `/proc/meridian_fixture_state_batch` from a copied generated binding and require:

```rust
assert_eq!(payload["classification"], "invalid");
assert_eq!(payload["issues"][0]["code"], "required_proc_missing");
assert_eq!(payload["issues"][0]["path"], "/proc/meridian_fixture_state_batch");
```

Also require the tool in analysis `tools/list` and in the capability registry.

- [ ] **Step 2: Run tests and confirm the tool is absent**

```powershell
cargo +1.95.0 test --test fixture_manifest --test tool_contracts --test mcp_conformance --test capability_registry
```

Expected: missing tool/contract/mapping failures.

- [ ] **Step 3: Implement fixed fixture checks**

```rust
pub async fn check_sync(
	context: &ToolExecutionContext,
	state: &ServerState,
	args: Value,
) -> Result<ToolResult>;
```

Use the active snapshot only when its DME canonical path equals the manifest DME. Otherwise construct
a bounded `AnalysisBuild` locally and discard it after the call. Resolve required procs through the
Plan 1 canonical resolver and compare exact ordered argument names. Search required tokens only in
declared text inputs with normalized LF/CRLF line handling.

- [ ] **Step 4: Add contract and schema**

```rust
json!({
	"type": "object",
	"properties": {"fixture_manifest_path": {"type": "string"}},
	"required": ["fixture_manifest_path"],
	"additionalProperties": false
})
```

Register it as analysis `READ`, `Experimental`, with 1 MiB maximum output.

- [ ] **Step 5: Run fixture and protocol tests**

```powershell
cargo +1.95.0 test --test fixture_manifest --test tool_contracts --test mcp_conformance --test capability_registry
```

Expected: matching fixture verifies; missing proc/token/file and stale build status are structured.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/tools/fixture.rs src/tools/mod.rs src/contracts.rs src/parameters.rs spacemandmm-capabilities.json tests/fixture_manifest.rs tests/tool_contracts.rs tests/mcp_conformance.rs
git commit -m "feat: check fixture synchronization"
```

### Task 6: Enforce one launch provenance gate

**Files:**
- Modify: `src/build_provenance.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/debugger.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `src/parameters.rs`
- Modify: `src/state.rs`
- Modify: `tests/build_provenance.rs`
- Modify: `tests/runtime_tools.rs`
- Modify: `tests/tracy_tools.rs`

**Interfaces:**
- Consumes: canonical DMB path, `require_verified_provenance`, and `BuildProvenanceStore::evaluate_launch`.
- Produces: `require_launchable_artifact(context, dmb_path, require_verified) -> Result<LaunchProvenance, ToolResult>` used by all launchers immediately before spawn.

- [ ] **Step 1: Write failing managed/unmanaged launch-decision tests**

```rust
assert_launch_rejected(stale_managed, "stale_build_artifact");
assert_launch_allowed(unmanaged_default, "unverified");
assert_launch_rejected(unmanaged_required, "build_provenance_unavailable");
assert_launch_allowed(fresh_managed, "verified");
```

Require identical decisions from standard, debugger, and Tracy launch adapters without spawning a
real process.

- [ ] **Step 2: Run focused tests and confirm launchers ignore provenance**

```powershell
cargo +1.95.0 test --test build_provenance --test runtime_tools --test tracy_tools
```

Expected: stale launch assertions fail.

- [ ] **Step 3: Add the shared gate and launch schema field**

```rust
pub struct LaunchProvenance {
	pub status: ProvenanceStatus,
	pub build_record_id: Option<String>,
	pub dmb_sha256: String,
	pub warnings: Vec<ProvenanceReason>,
}
```

Add `require_verified_provenance` with default `false` to `dm_run`, `dm_debug_launch`, and
`dm_tracy_launch`. Call the shared gate after path containment and immediately before constructing the
process command. Re-hash again inside the gate; do not reuse the earlier argument-containment metadata.

- [ ] **Step 4: Retain launch identity in runtime/debugger/Tracy state**

Store `LaunchProvenance` in the owned session state and return it from launch/status/stop. Tracy
experiment identity must include its verified build record ID when available. Existing DMB/RSC hash
checks remain and must agree with the launch provenance.

- [ ] **Step 5: Run launch policy tests**

```powershell
cargo +1.95.0 test --test build_provenance --test runtime_tools --test tracy_tools --test active_tool_policy
```

Expected: stale managed artifacts never reach a spawn helper; unmanaged compatibility remains
explicit.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/build_provenance.rs src/tools/mod.rs src/tools/runtime.rs src/tools/debugger.rs src/tools/tracy.rs src/parameters.rs src/state.rs tests/build_provenance.rs tests/runtime_tools.rs tests/tracy_tools.rs
git commit -m "feat: reject stale managed DreamMaker artifacts"
```

### Task 7: Verify Plan 2 and prepare its review checkpoint

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Modify: `README.md`
- Modify: `tests/documentation.rs`
- Regenerate: `docs/tool-contracts.md`

**Interfaces:**
- Consumes: all Plan 2 components.
- Produces: fixture-verified MMCP-PROF-021 implementation ready for live Plan 5 qualification.

- [ ] **Step 1: Add failing documentation assertions**

Require README coverage for `MERIDIAN_MCP_STATE_DIR`, `dm_check_fixture_sync`, managed versus
unmanaged artifacts, `require_verified_provenance`, failed-compile stale behavior, and local state
privacy.

- [ ] **Step 2: Update documentation and regenerate tool contracts**

```powershell
cargo +1.95.0 run --locked --bin render_tool_docs -- docs/tool-contracts.md
```

- [ ] **Step 3: Run the Plan 2 gate**

```powershell
rustc +1.95.0 --version
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.95.0 test --locked --test private_state --test fixture_manifest --test build_provenance --test compiler_runner --test rift_compile --test runtime_tools --test tracy_tools --test active_tool_policy --test capability_registry --test documentation --test tool_contracts --test mcp_conformance
git diff --check
```

Expected: every command exits 0 and restart tests preserve stale decisions.

- [ ] **Step 4: Inspect state privacy and working-tree scope**

```powershell
git grep -n -I -E "[A-Za-z]:\\\\Users\\\\|/home/[^/]+|/Users/[^/]+" -- README.md docs tests scripts src
git status --short
git diff --stat
```

Expected: no machine profile path is present; only approved Plan 1/2 and documentation files differ.

- [ ] **Step 5: Record the Plan 2 checkpoint if commits are authorized**

```powershell
git add README.md docs/architecture.md docs/security.md docs/tool-contracts.md tests/documentation.rs
git commit -m "docs: explain build provenance and fixture sync"
```
