# Analysis Correctness and Policy Visibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Authorize explicitly configured linked Git worktrees, expose the immutable effective path policy, and make every proc-analysis tool agree on declaration and implementation ownership.

**Architecture:** Startup expands exact repository authorizations into canonical linked-worktree roots and freezes typed `EffectiveRoot` records inside `PathPolicy`. A new `dm_server_status` tool exposes that state. A snapshot-owned `ProcResolver` becomes the sole source of proc declaration and implementation ownership for exact lookup and indexes.

**Tech Stack:** Rust 2021, Rust 1.95, Tokio, serde/serde_json, rmcp, SpacemanDMM `dreammaker`, fixed non-shell Git subprocesses.

**Spec:** `docs/superpowers/specs/2026-08-27-meridian-mcp-provenance-and-native-evidence-design.md`

## Global Constraints

- `MERIDIAN_MCP_ROOTS` remains an exact-root allowlist.
- `MERIDIAN_MCP_REPOSITORIES` is startup-only and may expand only linked worktrees with the same canonical Git common directory.
- Effective roots are frozen before stdio starts; tool calls cannot authorize paths.
- No Git remote, config, worktree metadata, or repository file may be changed.
- Proc declaration owner and nearest implementation owner are distinct identities.
- Existing `dm_get_proc` compatibility fields remain for one compatibility cycle.
- Public fixtures and documentation must contain no machine-specific paths, account names, or profile segments.
- Use `cargo +1.95.0` for Rust verification and print `rustc +1.95.0 --version` before completion.
- Commit steps are conditional: execute them only if the user explicitly authorizes commits during implementation.

---

## Locked file structure

- Create `src/repository_roots.rs`: fixed Git queries, repository identity, and effective-root expansion.
- Create `src/proc_resolution.rs`: canonical proc declaration/implementation resolution.
- Create `src/tools/server_status.rs`: read-only server and policy status response.
- Modify `src/config.rs`: parse repository authorizations and retain effective roots.
- Modify `src/path_policy.rs`: store typed effective roots and expose policy context.
- Modify `src/analysis_snapshot.rs`: build and retain the proc resolver.
- Modify `src/index/mod.rs`: consume canonical proc records while building language indexes.
- Modify `src/tools/{mod,parse,analysis,language}.rs`: register status and use canonical proc resolution.
- Modify `src/{contracts,lib,server}.rs`: export types, register the tool, and pass immutable configuration.
- Create `tests/repository_roots.rs`: linked-worktree and unrelated-repository policy fixtures.
- Create `tests/proc_resolution.rs`: parent declaration/child override consistency fixture.
- Modify `tests/{config_and_paths,tool_contracts,mcp_conformance}.rs`: configuration, contract, and protocol coverage.

### Task 1: Model and expand effective repository roots

**Files:**
- Create: `src/repository_roots.rs`
- Modify: `src/config.rs`
- Modify: `src/lib.rs`
- Create: `tests/repository_roots.rs`
- Modify: `tests/config_and_paths.rs`

**Interfaces:**
- Consumes: explicit canonical workspace roots and optional canonical repository paths.
- Produces: `RootSource`, `RepositoryIdentity`, `EffectiveRoot`, and `expand_effective_roots(explicit_roots: &[PathBuf], repositories: &[PathBuf]) -> Result<Vec<EffectiveRoot>>`.

- [ ] **Step 1: Write failing explicit-root and linked-worktree tests**

```rust
use meridian_mcp::{expand_effective_roots, RootSource};

#[test]
fn linked_worktrees_expand_only_from_the_authorized_repository() {
	let fixture = GitWorktreeFixture::new();
	let roots = expand_effective_roots(
		&[fixture.primary.clone()],
		&[fixture.primary.clone()],
	)
	.unwrap();

	assert!(roots.iter().any(|root| {
		root.path == fixture.linked.canonicalize().unwrap()
			&& root.source == RootSource::LinkedGitWorktree
	}));
	assert!(!roots.iter().any(|root| root.path == fixture.unrelated.canonicalize().unwrap()));
}
```

The fixture initializes one repository, commits a file with local test-only Git identity arguments,
adds a linked worktree, and initializes an unrelated repository under the same temporary parent.

- [ ] **Step 2: Run the focused tests and confirm the API is missing**

```powershell
cargo +1.95.0 test --test repository_roots --test config_and_paths
```

Expected: compilation fails because `expand_effective_roots` and `RootSource` do not exist.

- [ ] **Step 3: Implement fixed Git command execution and typed root records**

```rust
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RootSource {
	ExplicitRoot,
	LinkedGitWorktree,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct RepositoryIdentity {
	pub kind: &'static str,
	pub digest: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct EffectiveRoot {
	pub path: PathBuf,
	pub source: RootSource,
	pub repository_identity: Option<RepositoryIdentity>,
	pub head_revision: Option<String>,
	pub dirty: Option<bool>,
}
```

Implement `run_git(path, args)` with `std::process::Command::new("git")`, `.args(args)`, and
`.current_dir(path)`. Do not invoke a shell. Parse `git worktree list --porcelain -z` as NUL-delimited
records, canonicalize every `worktree` value, and verify each candidate by resolving its own
`--git-common-dir`. Hash the canonical common-directory bytes with SHA-256 for the local identity.

- [ ] **Step 4: Parse `MERIDIAN_MCP_REPOSITORIES` and freeze expanded roots**

```rust
let repositories = std::env::var_os("MERIDIAN_MCP_REPOSITORIES")
	.map(|value| std::env::split_paths(&value).collect::<Vec<_>>())
	.unwrap_or_default();
let effective_roots = expand_effective_roots(&workspace_roots, &repositories)?;
```

Keep existing public constructors compatible by treating their `workspace_roots` as explicit roots
and their repository list as empty. Add a test-only `ServerConfig::from_values_with_repositories`
constructor rather than adding positional arguments to every existing constructor.

- [ ] **Step 5: Run focused tests**

```powershell
cargo +1.95.0 test --test repository_roots --test config_and_paths
```

Expected: linked and explicit roots are present once, unrelated roots are absent, malformed Git
output fails closed, and existing config tests pass.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/repository_roots.rs src/config.rs src/lib.rs tests/repository_roots.rs tests/config_and_paths.rs
git commit -m "feat: authorize linked repository worktrees"
```

### Task 2: Carry effective policy context through path failures

**Files:**
- Modify: `src/path_policy.rs`
- Modify: `src/server.rs`
- Modify: `src/tools/mod.rs`
- Modify: `tests/config_and_paths.rs`
- Modify: `tests/tool_results.rs`

**Interfaces:**
- Consumes: `Vec<EffectiveRoot>` from Task 1.
- Produces: `PathPolicy::from_effective_roots`, `PathPolicy::status`, and serializable `PolicyContext` attached to `PolicyError`.

- [ ] **Step 1: Write failing policy-context tests**

```rust
let error = policy.read_path(&outside).unwrap_err();
assert_eq!(error.code(), "path_outside_workspace");
assert_eq!(error.context().containment_mode, "immutable_startup_roots");
assert_eq!(error.context().policy_source, "server_startup_configuration");
assert_eq!(error.context().effective_roots.len(), 2);
```

Also update the tool-result fixture to require `details.containment_mode`, `details.policy_source`,
and `details.effective_roots` in a contained-path failure.

- [ ] **Step 2: Run the focused tests and confirm missing context**

```powershell
cargo +1.95.0 test --test config_and_paths --test tool_results
```

Expected: compilation fails because `PolicyError::context` is absent.

- [ ] **Step 3: Implement typed policy status and errors**

```rust
#[derive(Clone, Debug, serde::Serialize)]
pub struct PolicyContext {
	pub containment_mode: &'static str,
	pub policy_source: &'static str,
	pub effective_roots: Vec<EffectiveRoot>,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct PathPolicyStatus {
	pub containment_mode: &'static str,
	pub policy_source: &'static str,
	pub effective_roots: Vec<EffectiveRoot>,
	pub compiler_allowlist: Vec<PathBuf>,
}
```

Store effective roots in `PathPolicy`. Preserve `PathPolicy::new(Vec<PathBuf>, Vec<PathBuf>)` by
converting paths to `ExplicitRoot` records. Add `from_effective_roots` for server startup.

- [ ] **Step 4: Serialize policy context in the centralized tool error**

```rust
json!({
	"path": error.path().display().to_string(),
	"policy_code": error.code(),
	"containment_mode": error.context().containment_mode,
	"policy_source": error.context().policy_source,
	"effective_roots": error.context().effective_roots,
})
```

Change recovery text to distinguish selecting an existing effective root from restarting with an
explicit repository authorization. Do not expose Git remotes or environment dumps.

- [ ] **Step 5: Run policy and result tests**

```powershell
cargo +1.95.0 test --test config_and_paths --test tool_results --test active_tool_policy
```

Expected: all pass and unrelated containment behavior remains unchanged.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/path_policy.rs src/server.rs src/tools/mod.rs tests/config_and_paths.rs tests/tool_results.rs
git commit -m "feat: report effective path policy"
```

### Task 3: Add `dm_server_status`

**Files:**
- Create: `src/tools/server_status.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/contracts.rs`
- Modify: `src/server.rs`
- Modify: `tests/tool_contracts.rs`
- Modify: `tests/mcp_conformance.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: `ToolExecutionContext`, `ServerState`, `PathPolicyStatus`, current analysis snapshot, runtime state, and `current_build_identity()`.
- Produces: analysis-mode read-only tool `dm_server_status` with an empty object schema.

- [ ] **Step 1: Write failing contract and result tests**

```rust
#[tokio::test]
async fn server_status_reports_policy_and_analysis_generation() {
	let result = call_tool(&context, &state, "dm_server_status", json!({})).await.unwrap();
	let payload = payload(&result);
	assert_eq!(payload["containment"]["mode"], "immutable_startup_roots");
	assert!(payload["containment"]["effective_roots"].is_array());
	assert_eq!(payload["analysis"]["state_generation"], 0);
}
```

Require `dm_server_status` in analysis and development `tools/list`, and add one capability-registry
mapping with an owned Rust fixture gate.

- [ ] **Step 2: Run the tests and confirm the tool is absent**

```powershell
cargo +1.95.0 test --test tool_contracts --test mcp_conformance --test capability_registry
```

Expected: failure because the contract and registry mapping are absent.

- [ ] **Step 3: Implement the status adapter**

```rust
pub async fn status(
	context: &ToolExecutionContext,
	state: &ServerState,
) -> anyhow::Result<ToolResult> {
	let snapshot = state.snapshot().await.ok();
	let runtime = state.runtime().await;
	Ok(ToolResult::text(json!({
		"mcp_build": crate::build_identity::current_build_identity(),
		"mode": context.mode_name(),
		"containment": context.policy().status(),
		"analysis": analysis_status(snapshot.as_deref()),
		"runtime": runtime.status_summary(),
	}).to_string()))
}
```

Add narrow getters on `ToolExecutionContext` rather than making its fields public. The status call may
refresh an exited child but must not spawn, stop, parse, write, or access Git remotes.

- [ ] **Step 4: Register schema, dispatch, contract, and capability mapping**

```rust
tools.push(ToolDefinition {
	name: "dm_server_status".into(),
	description: "Report immutable startup policy, build identity, analysis generation, and owned runtime summary.".into(),
	input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
});
```

Use the analysis `MEMORY` effect and `Provisional` support. Ensure the capability mapping targets
exactly `dm_server_status`.

- [ ] **Step 5: Run focused tests**

```powershell
cargo +1.95.0 test --test tool_contracts --test mcp_conformance --test capability_registry
```

Expected: status is advertised in both modes and returns bounded policy state.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/tools/server_status.rs src/tools/mod.rs src/contracts.rs src/server.rs tests/tool_contracts.rs tests/mcp_conformance.rs spacemandmm-capabilities.json
git commit -m "feat: expose Meridian-MCP server status"
```

### Task 4: Build the canonical proc resolver

**Files:**
- Create: `src/proc_resolution.rs`
- Modify: `src/analysis_snapshot.rs`
- Modify: `src/lib.rs`
- Create: `tests/proc_resolution.rs`
- Modify: `tests/fixtures/language/fixture.dm`

**Interfaces:**
- Consumes: immutable `AnalysisSnapshot` object tree, source mapping, and language symbols.
- Produces: `ProcResolver`, `ProcResolution`, `ResolvedProcImplementation`, `ProcResolutionKind`, and `ProcResolutionError`.

- [ ] **Step 1: Add a parent declaration and child override fixture**

```dm
/datum/proc/meridian_resolution_fixture(value)
	return "parent [value]"

/datum/meridian_resolution_child/meridian_resolution_fixture(value)
	return "child [value]"
```

Write a Rust test requiring implementation owner `/datum/meridian_resolution_child`, declaration
owner `/datum`, and resolution kind `local_implementation`.

- [ ] **Step 2: Run the fixture test and reproduce inherited-parent selection**

```powershell
cargo +1.95.0 test --test proc_resolution
```

Expected: compilation fails because the resolver API does not exist.

- [ ] **Step 3: Define stable resolution records**

```rust
#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcResolutionKind {
	LocalImplementation,
	InheritedImplementation,
	NotFound,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ResolvedProcImplementation {
	pub owner: String,
	pub override_index: usize,
	pub location: SourceLocation,
	pub has_body: bool,
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct ProcResolution {
	pub requested_type_path: String,
	pub proc_name: String,
	pub implementation_owner: String,
	pub declaration_owner: String,
	pub resolution_kind: ProcResolutionKind,
	pub implementations: Vec<ResolvedProcImplementation>,
}
```

Keep `SourceLocation` repository-owned and serializable so no `dreammaker` internal reference escapes
the snapshot.

- [ ] **Step 4: Resolve implementation independently from declaration**

Walk from requested type toward ancestors. Select the first proc entry with a local value/body as
the implementation owner. Continue only to find the nearest declaration metadata. Build the bounded
implementation chain in requested-to-ancestor order. Do not overwrite a selected child implementation
when an ancestor declaration is found.

- [ ] **Step 5: Build the resolver during snapshot construction**

```rust
let proc_resolver = ProcResolver::build(&extracted_context, &objtree);
```

Store it as `Arc<ProcResolver>` in `AnalysisSnapshot`. The resolver is immutable and shares the
snapshot generation lifetime.

- [ ] **Step 6: Run resolution tests**

```powershell
cargo +1.95.0 test --test proc_resolution --test analysis_snapshot
```

Expected: local override, inherited implementation, missing proc, and repeated snapshot-build tests
pass deterministically.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/proc_resolution.rs src/analysis_snapshot.rs src/lib.rs tests/proc_resolution.rs tests/fixtures/language/fixture.dm
git commit -m "feat: resolve canonical proc ownership"
```

### Task 5: Make exact lookup and language indexes consume the resolver

**Files:**
- Modify: `src/tools/parse.rs`
- Modify: `src/tools/analysis.rs`
- Modify: `src/tools/language.rs`
- Modify: `src/index/mod.rs`
- Modify: `src/search.rs`
- Modify: `tests/proc_resolution.rs`
- Modify: `tests/language_capabilities.rs`
- Modify: `tests/fixture_corpus.rs`

**Interfaces:**
- Consumes: `AnalysisSnapshot::proc_resolver()` and `ProcResolution` from Task 4.
- Produces: consistent owner fields across `dm_get_proc`, definition, search, document symbols, and implementations.

- [ ] **Step 1: Extend the failing fixture test across every consumer**

```rust
for tool in [
	"dm_get_proc",
	"dm_get_definition",
	"dm_search_symbols",
	"dm_search_context",
	"dm_document_symbols",
	"dm_find_implementations",
] {
	let payload = call_fixture_tool(tool).await;
	assert_reports_implementation_owner(&payload, "/datum/meridian_resolution_child");
}
```

Require `dm_get_proc` to preserve `type_path`, `declared`, and `overrides` while adding
`requested_type_path`, `implementation_owner`, `declaration_owner`, `resolution_kind`, and bounded
`resolution_diagnostics` for inherited selection.

- [ ] **Step 2: Run the consistency tests and observe current disagreement**

```powershell
cargo +1.95.0 test --test proc_resolution --test language_capabilities --test fixture_corpus
```

Expected: child-owner assertions fail for current `dm_get_proc` and index metadata.

- [ ] **Step 3: Replace the ad hoc `dm_get_proc` ancestor loop**

```rust
let resolution = snapshot.proc_resolver().resolve(type_path, proc_name)?;
let result = json!({
	"name": proc_name,
	"type_path": type_path,
	"requested_type_path": resolution.requested_type_path,
	"implementation_owner": resolution.implementation_owner,
	"declaration_owner": resolution.declaration_owner,
	"resolution_kind": resolution.resolution_kind,
	"declared": resolution.implementation_owner == type_path,
	"overrides": render_implementations(&snapshot, &resolution),
	"resolution_diagnostics": resolution.diagnostics(),
});
```

Map `ProcResolutionError::NotFound` to structured `symbol_not_found` with the searched type chain and
bounded same-name candidates.

- [ ] **Step 4: Build language-index proc entries from canonical implementations**

Iterate the resolver's canonical implementation records instead of deriving owner from declaration
metadata. Keep `SymbolId::Proc { owner, name, override_index }`; set `owner` to the implementation
owner. Store the declaration owner as a separate `declared_in` field only where the response schema
requires it.

- [ ] **Step 5: Route definition, context search, and implementations through canonical IDs**

When a query names an exact owner/member, resolve first and then query the index with the returned
implementation owner. If the resolver and index disagree, return `symbol_index_inconsistent` with
the analysis generation rather than falling back.

- [ ] **Step 6: Run focused analysis tests**

```powershell
cargo +1.95.0 test --test proc_resolution --test language_capabilities --test fixture_corpus --test analysis_snapshot
```

Expected: every tool reports the same child implementation and parent declaration.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/tools/parse.rs src/tools/analysis.rs src/tools/language.rs src/index/mod.rs src/search.rs tests/proc_resolution.rs tests/language_capabilities.rs tests/fixture_corpus.rs
git commit -m "fix: unify proc ownership across analysis tools"
```

### Task 6: Verify Plan 1 and prepare its review checkpoint

**Files:**
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Modify: `README.md`
- Modify: `tests/documentation.rs`
- Regenerate: `docs/tool-contracts.md`

**Interfaces:**
- Consumes: all Plan 1 interfaces.
- Produces: independently reviewable MMCP-PROF-017 and MMCP-PROF-018 implementation.

- [ ] **Step 1: Write failing documentation assertions**

```rust
for required in [
	"MERIDIAN_MCP_REPOSITORIES",
	"dm_server_status",
	"immutable_startup_roots",
	"implementation owner",
	"declaration owner",
] {
	assert!(readme.contains(required), "README is missing {required}");
}
```

- [ ] **Step 2: Update documentation and regenerate contracts**

Document exact linked-worktree startup behavior, restart requirements, local repository identity,
policy error fields, status fields, and proc ownership semantics. Regenerate with:

```powershell
cargo +1.95.0 run --locked --bin render_tool_docs -- docs/tool-contracts.md
```

- [ ] **Step 3: Run the exact Plan 1 gate**

```powershell
rustc +1.95.0 --version
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.95.0 test --locked --test repository_roots --test config_and_paths --test tool_results --test proc_resolution --test language_capabilities --test fixture_corpus --test capability_registry --test documentation --test tool_contracts --test mcp_conformance
git diff --check
```

Expected: every command exits 0; no unrelated path is authorized; every proc consumer agrees.

- [ ] **Step 4: Inspect the working tree and preserve unrelated changes**

```powershell
git status --short
git diff --stat
```

Expected: only Plan 1 files plus the approved spec/plan documents are changed.

- [ ] **Step 5: Record the Plan 1 checkpoint if commits are authorized**

```powershell
git add README.md docs/architecture.md docs/security.md docs/tool-contracts.md tests/documentation.rs
git commit -m "docs: explain policy and proc ownership"
```
