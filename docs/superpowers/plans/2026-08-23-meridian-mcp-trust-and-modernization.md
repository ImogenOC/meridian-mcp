# Meridian-MCP Trust and Modernization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver an evidence-backed, workspace-contained DreamMaker MCP server with accurate documentation, typed tool contracts, independent fixtures, pinned dependencies, official Rust MCP transport, and no inherited unverified BYOND client protocol.

**Architecture:** Preserve the existing SpacemanDMM-backed analysis and proven runtime adapters while adding explicit configuration, path policy, project profiles, and typed contracts around them. Establish safety and behavior tests before replacing the hand-written JSON-RPC loop with `rmcp`; keep each migration boundary independently testable.

**Tech Stack:** Rust 2021, Rust 1.88 minimum, Tokio, serde, schemars, SpacemanDMM crates pinned by git revision, `rmcp` 3.1.3 with server and stdio features, PowerShell integration harnesses, DreamMaker 516.1685 for Meridian integration.

**Spec:** `docs/superpowers/specs/2026-08-23-meridian-mcp-trust-and-modernization-design.md`

## Global Constraints

- Default server mode is `analysis`; only installation or launch configuration may enable `development`.
- Windows is the verified platform. Linux is best-effort only when exercised by CI. macOS remains unsupported.
- DreamMaker is the language acceptance authority; SpacemanDMM findings remain analysis evidence.
- `dm_compile` is a raw DreamMaker compiler gate, not a tgstation full build.
- Reads and writes resolve inside configured workspace roots; writes may also use a server-managed temporary root.
- Caller-selected executables are rejected unless present in the configured allowlist.
- Network operations are loopback-only and only server-owned DreamDaemon processes may be managed.
- The inherited BYOND client protocol is removed; the audit found no evidenced consumer or independent compatibility proof.
- Existing `dm_*` tool names remain stable unless a tool is removed for unsupported behavior.
- Changes remain uncommitted for maintainer review. End each task with a working-tree review checkpoint, not a commit.
- Preserve unrelated working-tree changes and never reset or overwrite them.

---

## File map

- `src/config.rs`: capability mode, workspace roots, executable allowlist, environment parsing, and startup validation.
- `src/lib.rs`: reusable crate boundary for integration tests and the binary entry point.
- `src/path_policy.rs`: canonical read/write/DMB/output containment and overwrite rules.
- `src/project.rs`: generic project profile plus Meridian discovery of `.dme`, `SpacemanDMM.toml`, `dependencies.sh`, and full-build entry point.
- `src/parameters.rs`: serde/schemars parameter types for every MCP tool.
- `src/contracts.rs`: effect metadata, support labels, tool inventory, active-tool set, and generated Markdown reference.
- `src/result.rs`: transport-independent domain tool result and stable error payload.
- `src/server.rs`: cloneable `MeridianServer`, synchronized `ServerState`, tool wrappers, and capability filtering.
- `src/mcp.rs`: official `rmcp` handler and stdio service startup after migration.
- `src/state.rs`: parsed-environment generation and server-owned runtime state.
- `src/tools/*.rs`: domain adapters; no MCP transport types after migration.
- `tests/fixtures/`: fresh DreamMaker, DMM/TGM, runtime, and malformed-input fixtures.
- `tests/common/mod.rs`: temporary-root, fixture-path, result-assertion, and test-configuration helpers.
- `tests/`: contract, containment, fixture, installed-binary, and documentation tests.
- `.github/workflows/ci.yml`: per-change Rust/MCP gates.
- `.github/workflows/byond-integration.yml`: scheduled/manual Windows BYOND gates.
- `docs/*.md`, `README.md`, `TESTING.md`, `SECURITY.md`, `CONTRIBUTING.md`, `CHANGELOG.md`: public trust and operating documentation.

### Task 1: Provenance and truthful support baseline

**Files:**
- Create: `docs/provenance.md`
- Create: `docs/source-authority.md`
- Create: `docs/compatibility.md`
- Create: `docs/dependency-policy.md`
- Create: `SECURITY.md`
- Create: `tests/documentation.rs`
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `CONTRIBUTING.md`

**Interfaces:**
- Consumes: accepted trust classifications in the design specification.
- Produces: stable capability labels and source/provenance vocabulary used by every later task.

- [ ] **Step 1: Add failing documentation-presence and claim tests**

```rust
#[test]
fn required_trust_documents_exist() {
	for path in [
		"docs/provenance.md",
		"docs/source-authority.md",
		"docs/compatibility.md",
		"docs/dependency-policy.md",
		"SECURITY.md",
	] {
		assert!(std::path::Path::new(path).is_file(), "missing {path}");
	}
}

#[test]
fn readme_does_not_claim_unverified_full_support() {
	let readme = std::fs::read_to_string("README.md").unwrap();
	for forbidden in [
		"full access to DreamMaker language tooling",
		"Full BYOND client protocol implementation",
		"macOS | Untested | Should work",
	] {
		assert!(!readme.contains(forbidden), "unverified claim remains: {forbidden}");
	}
}
```

- [ ] **Step 2: Run the documentation test and confirm it fails**

Run: `cargo test --test documentation -- --nocapture`

Expected: failure because the trust documents do not exist and the README still contains unsupported claims.

- [ ] **Step 3: Write the provenance and authority records**

Record the original source commit `6a739a4278b53e86b430abaf011467f22c9dd2ec`, the non-ancestral import, inherited file groups, current local commits, SpacemanDMM lock revision, and the verified/provisional/experimental/unsupported definitions. State that BYOND/compiler behavior is authoritative for language acceptance and that the original dm-mcp source is provenance only.

- [ ] **Step 4: Rewrite the README capability and platform tables**

Use explicit support labels. Mark Windows verified only where current evidence exists, Linux best-effort, macOS unsupported, and the client-login protocol experimental/off by default. Link all trust documents and distinguish `dm_compile` from Meridian-Rift's full build.

- [ ] **Step 5: Replace generic contribution and changelog text**

Document repository-specific Rust gates, fixture expectations, source-authority rules, dependency update procedure, and the missing `dm_search_context` changelog entry. Do not claim the SDK migration or containment work is complete before its tasks pass.

- [ ] **Step 6: Run the documentation and whitespace gates**

Run: `cargo test --test documentation -- --nocapture`

Expected: all documentation tests pass.

Run: `git diff --check`

Expected: no whitespace errors.

- [ ] **Step 7: Review checkpoint**

Run: `git status --short`

Expected: only the approved specification, this plan, and Task 1 documentation/test files are new or modified in this repository.

### Task 2: Startup configuration and workspace containment

**Files:**
- Create: `src/config.rs`
- Create: `src/lib.rs`
- Create: `src/path_policy.rs`
- Create: `tests/common/mod.rs`
- Create: `tests/config_and_paths.rs`
- Modify: `src/main.rs`
- Modify: `src/state.rs`

**Interfaces:**
- Consumes: `MERIDIAN_MCP_MODE`, `MERIDIAN_MCP_ROOTS`, `MERIDIAN_MCP_COMPILERS`.
- Produces: `CapabilityMode`, `ServerConfig::from_env()`, and `PathPolicy::{read_path, new_output_path, existing_output_path, executable, runtime_dmb}`.

`tests/common/mod.rs` produces `fixture_root()`, `fixture_policy()`, `development_context()`, `existing_fixture_file()`, `outside_file()`, `state_with_fixture_environment()`, and `assert_tool_error_code()` for later integration tests.

- [ ] **Step 1: Write failing configuration tests**

```rust
#[test]
fn analysis_is_the_default_mode() {
	let config = ServerConfig::from_values(None, vec![fixture_root()], Vec::new()).unwrap();
	assert_eq!(config.mode(), CapabilityMode::Analysis);
}

#[test]
fn development_requires_an_explicit_value() {
	let config = ServerConfig::from_values(
		Some("development"),
		vec![fixture_root()],
		vec![fixture_root().join("dm.exe")],
	).unwrap();
	assert_eq!(config.mode(), CapabilityMode::Development);
}
```

- [ ] **Step 2: Write failing path-policy tests**

```rust
#[test]
fn traversal_outside_a_workspace_is_rejected() {
	let policy = fixture_policy();
	let error = policy.read_path(fixture_root().join("..\\secret.dm")).unwrap_err();
	assert_eq!(error.code(), "path_outside_workspace");
}

#[test]
fn existing_outputs_require_explicit_overwrite() {
	let policy = fixture_policy();
	let output = existing_fixture_file("map.png");
	assert_eq!(
		policy.output_path(&output, false).unwrap_err().code(),
		"output_exists"
	);
}

#[test]
fn caller_selected_executables_must_be_allowlisted() {
	let policy = fixture_policy();
	let error = policy.executable(fixture_root().join("other.exe")).unwrap_err();
	assert_eq!(error.code(), "executable_not_allowed");
}
```

- [ ] **Step 3: Run the tests and confirm missing interfaces**

Run: `cargo test --test config_and_paths -- --nocapture`

Expected: compilation failure because `ServerConfig` and `PathPolicy` do not exist.

- [ ] **Step 4: Implement immutable startup configuration**

Define:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityMode { Analysis, Development }

#[derive(Clone, Debug)]
pub struct ServerConfig {
	mode: CapabilityMode,
	workspace_roots: Vec<std::path::PathBuf>,
	compiler_allowlist: Vec<std::path::PathBuf>,
}
```

Canonicalize configured roots during startup, reject an empty root list, reject unknown modes, and never expose a tool that changes these values at runtime.

Create `src/lib.rs` and expose only the types required by integration tests: `CapabilityMode`, `ServerConfig`, `PathPolicy`, `PolicyError`, `ProjectProfile`, tool contracts, parameter types, and the server constructor. Keep implementation details crate-private. Change `src/main.rs` into a thin binary that calls the library startup function.

- [ ] **Step 5: Implement canonical containment checks**

Return a typed `PolicyError { code: &'static str, path: PathBuf, message: String }`. Existing read paths must canonicalize successfully. New outputs canonicalize their existing parent before joining the filename. On Windows, reject reparse-point escapes after canonicalization. Reject existing output unless `overwrite` is true.

- [ ] **Step 6: Load configuration before starting stdio**

Change `main()` to call `ServerConfig::from_env()?`, log only mode and root count to stderr, and pass the configuration into the server constructor. Do not print workspace paths or arguments to stdout.

- [ ] **Step 7: Run focused and full Rust gates**

Run: `cargo test --test config_and_paths -- --nocapture`

Expected: all containment tests pass.

Run: `cargo test`

Expected: all existing and new tests pass.

- [ ] **Step 8: Review checkpoint**

Run: `git diff --check` and `git status --short`.

Expected: no whitespace errors and no unrelated changes.

### Task 3: Project profiles and parsed-state generation

**Files:**
- Create: `src/project.rs`
- Create: `tests/project_profile.rs`
- Modify: `src/main.rs`
- Modify: `src/state.rs`
- Modify: `src/tools/parse.rs`

**Interfaces:**
- Consumes: a workspace-contained `.dme` path and nearby repository configuration.
- Produces: `ProjectProfile::discover(&PathPolicy, &Path)`, `ParsedEnvironment`, and monotonically increasing `state_generation`.

- [ ] **Step 1: Write project-discovery fixtures and failing tests**

Create a temporary fixture tree containing `tgstation.dme`, `SpacemanDMM.toml`, `dependencies.sh` with `BYOND_MAJOR=516` and `BYOND_MINOR=1685`, and `BUILD.cmd`.

```rust
#[test]
fn meridian_profile_discovers_checked_in_project_configuration() {
	let profile = ProjectProfile::discover(&fixture_policy(), &fixture_dme()).unwrap();
	assert_eq!(profile.byond_version().as_deref(), Some("516.1685"));
	assert!(profile.spaceman_config().unwrap().ends_with("SpacemanDMM.toml"));
	assert!(profile.full_build_entrypoint().unwrap().ends_with("BUILD.cmd"));
}
```

- [ ] **Step 2: Write failing atomic-state tests**

```rust
#[test]
fn failed_parse_preserves_the_last_valid_generation() {
	let mut state = state_with_fixture_environment(7);
	let before = state.environment_path.clone();
	state.record_parse_failure();
	assert_eq!(state.state_generation, 7);
	assert_eq!(state.environment_path, before);
}
```

- [ ] **Step 3: Run the focused tests and confirm failure**

Run: `cargo test --test project_profile -- --nocapture`

Expected: compilation failure because project-profile and generation interfaces do not exist.

- [ ] **Step 4: Implement `ProjectProfile`**

Use a generic profile with optional Meridian fields. Parse only the literal exported values needed from `dependencies.sh`; do not execute the script. Discovery never leaves the `.dme` workspace root.

- [ ] **Step 5: Make parse replacement atomic**

Build `Context`, `ObjectTree`, and `SearchIndex` into local variables. Only after all required stages succeed, call:

```rust
state.replace_environment(ParsedEnvironment {
	path: dme_path,
	context,
	objtree,
	search_index,
	profile,
});
```

On failure, return JSON with `state_preserved: true`, the active environment path, and unchanged generation. On success, return the new generation.

- [ ] **Step 6: Run parse-state and regression tests**

Run: `cargo test --test project_profile -- --nocapture`

Expected: all tests pass.

Run: `cargo test tools::parse`

Run: `cargo test state::tests`

Expected: parse and state tests pass.

- [ ] **Step 7: Review checkpoint**

Run: `git diff --check` and `git status --short`.

Expected: only Task 3 files plus previously approved work are changed.

### Task 4: Typed parameters, tool contracts, and generated reference

**Files:**
- Create: `src/parameters.rs`
- Create: `src/contracts.rs`
- Create: `src/result.rs`
- Create: `src/bin/render_tool_docs.rs`
- Create: `tests/tool_contracts.rs`
- Create: `docs/tool-contracts.md`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/analysis.rs`
- Modify: `src/tools/compile.rs`
- Modify: `src/tools/map.rs`
- Modify: `src/tools/parse.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/search.rs`

**Interfaces:**
- Consumes: existing 19 `dm_*` tool names and argument behavior.
- Produces: `all_contracts() -> &'static [ToolContract]`, `active_tool_names()`, typed parameter structs, `DomainToolResult`, and `render_tool_reference()`.

- [ ] **Step 1: Define the failing contract invariants**

```rust
#[test]
fn every_tool_has_unique_effect_and_support_metadata() {
	let contracts = all_contracts();
	let names: std::collections::HashSet<_> = contracts.iter().map(|c| c.name).collect();
	assert_eq!(names.len(), contracts.len());
	assert!(contracts.iter().all(|c| !c.summary.is_empty()));
	assert!(contracts.iter().all(|c| c.max_output_bytes > 0));
}

#[test]
fn analysis_mode_contains_no_active_tools() {
	assert!(contracts_for(CapabilityMode::Analysis)
		.iter()
		.all(|contract| !contract.effects.spawns_process
			&& !contract.effects.writes_files
			&& !contract.effects.network_loopback));
}

#[test]
fn checked_in_reference_matches_generated_contracts() {
	let expected = render_tool_reference(all_contracts());
	let actual = std::fs::read_to_string("docs/tool-contracts.md").unwrap();
	assert_eq!(actual, expected);
}
```

- [ ] **Step 2: Run the contract tests and confirm failure**

Run: `cargo test --test tool_contracts -- --nocapture`

Expected: compilation failure because contract types and generated reference do not exist.

- [ ] **Step 3: Add typed parameter structs**

Define one `#[derive(Debug, Deserialize, JsonSchema)]` type per tool. Preserve current names and defaults. Add `overwrite: bool` to map rendering. Remove unrestricted semantics from `compiler_path`; document it as selecting a configured allowlisted compiler.

- [ ] **Step 4: Add contract metadata**

```rust
pub struct ToolEffects {
	pub reads_files: bool,
	pub writes_files: bool,
	pub spawns_process: bool,
	pub network_loopback: bool,
}

pub enum SupportLevel { Verified, Provisional, Experimental, Unsupported }

pub struct ToolContract {
	pub name: &'static str,
	pub summary: &'static str,
	pub mode: CapabilityMode,
	pub effects: ToolEffects,
	pub support: SupportLevel,
	pub timeout_ms: Option<u64>,
	pub max_output_bytes: usize,
}
```

Classify `dm_connect_test` as experimental and development-only. Classify compile, render, run, wait, stop, status, Topic, and connect-test as development-mode tools. Record file and process effects exactly.

Move `ToolResult` and `ToolContent` out of `src/mcp.rs` into transport-independent `DomainToolResult` and `DomainContent` types in `src/result.rs`. Domain adapters return these types; only `src/server.rs` later converts them into SDK result models.

- [ ] **Step 5: Generate `docs/tool-contracts.md` deterministically**

Add a small `src/bin/render_tool_docs.rs` binary or a test helper that writes the table in tool-name order. The checked-in document includes inputs, mode, effects, support, bounds, state requirements, and failure guidance.

- [ ] **Step 6: Run contract and documentation gates**

Run: `cargo run --bin render_tool_docs -- docs/tool-contracts.md`

Expected: the reference is generated successfully.

Run: `cargo test --test tool_contracts -- --nocapture`

Expected: all invariants and drift checks pass.

- [ ] **Step 7: Review checkpoint**

Run: `git diff --check` and `git status --short`.

Expected: no unrelated changes.

### Task 5: Apply containment to compiler, map, and runtime tools

**Files:**
- Create: `tests/active_tool_policy.rs`
- Modify: `src/tools/compile.rs`
- Modify: `src/tools/map.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/state.rs`

**Interfaces:**
- Consumes: `ServerConfig`, `PathPolicy`, typed parameters, and server-owned process state.
- Produces: active tools that cannot escape configured roots, overwrite implicitly, or manage unowned processes.

- [ ] **Step 1: Write failing active-tool policy tests**

```rust
#[tokio::test]
async fn compile_rejects_an_unlisted_compiler() {
	let result = compile(&development_context(), CompileParams {
		dme_path: fixture_dme(),
		compiler_path: Some(fixture_root().join("arbitrary.exe")),
		working_directory: None,
		defines: Vec::new(),
		timeout_ms: Some(10_000),
		idle_timeout_ms: Some(2_000),
	}).await.unwrap();
	assert_tool_error_code(result, "executable_not_allowed");
}

#[tokio::test]
async fn render_refuses_an_existing_output_without_overwrite() {
	let result = render_map(&development_context(), RenderMapParams {
		dmm_path: fixture_map(),
		output_path: Some(existing_fixture_file("map.png")),
		overwrite: false,
		z_level: 1,
	}).await.unwrap();
	assert_tool_error_code(result, "output_exists");
}

#[tokio::test]
async fn run_rejects_a_dmb_outside_the_workspace() {
	let result = run(&mut development_context(), RunParams {
		dmb_path: outside_file("game.dmb"),
		port: 1337,
		working_directory: None,
		daemon_args: Vec::new(),
		wait_for: None,
		wait_regex: false,
		startup_timeout_ms: 30_000,
	})
		.await.unwrap();
	assert_tool_error_code(result, "path_outside_workspace");
}
```

- [ ] **Step 2: Run the focused tests and confirm failure**

Run: `cargo test --test active_tool_policy -- --nocapture`

Expected: tests fail because current tools accept unrestricted paths or lack `overwrite`.

- [ ] **Step 3: Pass a tool execution context into active adapters**

Define `ToolContext<'a> { config: &'a ServerConfig, paths: &'a PathPolicy }`. Resolve every DME, DMM, DMB, working directory, output, and executable through it before invoking filesystem or process APIs.

- [ ] **Step 4: Enforce active-operation rules**

Compilation uses only an allowlisted compiler. Map rendering uses `output_path(..., overwrite)`. DreamDaemon launch accepts only a contained DMB and allowlisted discovered daemon. Topic remains fixed to `127.0.0.1` and the owned process port. Stop and wait continue to use only `ServerState.game_process`.

- [ ] **Step 5: Bound arguments and outputs**

Keep existing output-log bounds. Cap user-supplied daemon arguments at 64 entries and 4 KiB total, Topic strings at 60 KiB, search/list results at contract maxima, and timeouts at their existing documented caps. Return stable error codes in JSON tool errors.

- [ ] **Step 6: Run active policy and regression tests**

Run: `cargo test --test active_tool_policy -- --nocapture`

Expected: all policy tests pass.

Run: `cargo test tools::`

Run: `cargo test state::tests`

Expected: all focused regression tests pass.

- [ ] **Step 7: Review checkpoint**

Run: `git diff --check` and `git status --short`.

Expected: no unrelated changes.

### Task 6: Remove the unsupported BYOND client protocol

**Files:**
- Modify: `Cargo.toml`
- Modify: `src/main.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/contracts.rs`
- Modify: `docs/tool-contracts.md`
- Modify: `test_mcp.ps1`
- Create: `tests/client_protocol_removed.rs`
- Delete: `src/client/mod.rs`
- Delete: `src/client/crypto.rs`
- Delete: `src/client/packets.rs`
- Delete: `src/client/protocol.rs`

**Interfaces:**
- Consumes: the completed provenance audit showing no evidenced consumer or independent protocol validation.
- Produces: normal builds with no `dm_connect_test`, client packet parser, RUNSUB crypto, or unused crypto dependency.

- [ ] **Step 1: Write a failing default-build exposure test**

```rust
#[test]
fn tool_inventory_excludes_removed_connect_test() {
	let names: Vec<_> = all_contracts().iter().map(|contract| contract.name).collect();
	assert!(!names.contains(&"dm_connect_test"));
}

#[test]
fn runtime_source_does_not_import_the_removed_client() {
	let source = std::fs::read_to_string("src/tools/runtime.rs").unwrap();
	assert!(!source.contains("BYONDClient"));
}
```

- [ ] **Step 2: Run default and feature tests before implementation**

Run: `cargo test --test client_protocol_removed -- --nocapture`

Expected: failure because the default inventory still contains `dm_connect_test`.

- [ ] **Step 3: Remove client-login dispatch and implementation**

Delete `src/client/`, the root `mod client`, `connect_test`, its tool definition/dispatch, and the `rand` dependency, whose only current consumer is `src/client/protocol.rs`. Preserve the independently validated loopback `world.Topic()` implementation in `src/tools/runtime.rs`.

Remove the experimental contract and regenerate `docs/tool-contracts.md` so public inventory matches the normal binary.

- [ ] **Step 4: Remove the default smoke-test expectation**

Make `test_mcp.ps1` assert that `tools/list` omits `dm_connect_test` and remove the client-handshake call from runtime smoke sessions.

- [ ] **Step 5: Run removal and full tests**

Run: `cargo test`

Expected: all tests pass without compiling/exposing the client protocol.

- [ ] **Step 6: Review checkpoint**

Run: `git diff --check` and `git status --short`.

Expected: only the unsupported client-protocol removal and prior planned files changed.

### Task 7: Independent DreamMaker, map, runtime, and performance fixtures

**Files:**
- Create: `tests/fixtures/language/fixture.dme`
- Create: `tests/fixtures/language/types.dm`
- Create: `tests/fixtures/language/diagnostics.dm`
- Create: `tests/fixtures/maps/fixture.dmm`
- Create: `tests/fixtures/maps/fixture.tgm`
- Create: `tests/fixtures/runtime/runtime_fixture.dme`
- Create: `tests/fixtures/runtime/runtime_fixture.dm`
- Create: `tests/dreammaker_fixtures.rs`
- Create: `tests/map_fixtures.rs`
- Create: `tests/runtime_fixtures.rs`
- Create: `tests/performance.rs`
- Create: `scripts/run-byond-integration.ps1`
- Modify: `TESTING.md`

**Interfaces:**
- Consumes: installed DreamMaker/DreamDaemon and optional `MERIDIAN_RIFT_ROOT`.
- Produces: fresh fixture evidence and a recorded full-corpus baseline.

- [ ] **Step 1: Author language fixtures and expected assertions**

Cover absolute/relative paths, inheritance, proc overrides, vars, defines, conditionals, AUTODOC, warnings, and one intentional compiler error isolated behind a define. Tests assert canonical symbol locations and diagnostic severities.

- [ ] **Step 2: Author map fixtures and expected assertions**

Use purpose-written tiles on two z-levels. Assert exact dimensions, known type coordinates, and at least one nontransparent rendered pixel. Include one missing-resource fixture that must return a structured diagnostic.

- [ ] **Step 3: Author the runtime fixture**

The fixture logs `MERIDIAN_MCP_RUNTIME_READY`, implements `world/Topic()` responses for string, float, and null/empty cases, and has a controlled crash command. It contains no tgstation code.

- [ ] **Step 4: Write integration tests behind environment guards**

```rust
struct ParseMetrics {
	type_count: usize,
	symbol_count: usize,
	elapsed: std::time::Duration,
}

#[test]
fn meridian_full_corpus_baseline_is_within_guardrail() {
	let Some(root) = std::env::var_os("MERIDIAN_RIFT_ROOT") else {
		eprintln!("skipped: MERIDIAN_RIFT_ROOT is not configured");
		return;
	};
	let metrics = parse_metrics(std::path::Path::new(&root).join("tgstation.dme")).unwrap();
	assert!(metrics.type_count > 50_000);
	assert!(metrics.symbol_count > 300_000);
	assert!(metrics.elapsed <= accepted_baseline() * 2);
}
```

Implement `parse_metrics(PathBuf) -> anyhow::Result<ParseMetrics>` by calling the same parser/index constructors used by `dm_parse_environment`, timing them with `Instant`, and counting the completed object tree and search documents. Store the accepted baseline in `tests/fixtures/performance-baseline.json` with repository commit, tool revision, duration, counts, and measurement date.

- [ ] **Step 5: Implement the PowerShell integration driver**

The script discovers `dm.exe` in standard Windows locations, reads the Meridian BYOND version, compiles the fresh fixtures, parses diagnostics, launches DreamDaemon, waits for the readiness marker, exercises Topic values, stops cleanly, and optionally runs the full Meridian parse. It fails if artifacts are stale or no readiness marker appears.

- [ ] **Step 6: Run Rust fixture tests**

Run: `cargo test --tests -- --nocapture`

Expected: all tests that do not require BYOND pass; guarded integration tests state why they skipped when environment variables are absent.

- [ ] **Step 7: Run the Windows BYOND fixture gate**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-byond-integration.ps1`

Expected: zero DreamMaker errors for clean fixtures, intentional diagnostic classification for the failing fixture, visible map output, readiness marker, correct Topic responses, and clean shutdown.

- [ ] **Step 8: Run the optional Meridian full-corpus gate**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-byond-integration.ps1 -MeridianRiftRoot 'C:\Users\Zoe\Documents\GitHub\Meridian-Rift'`

Expected: successful parse/index metrics and no regression greater than two times the accepted baseline.

- [ ] **Step 9: Review checkpoint**

Run: `git diff --check` and `git status --short`.

Expected: fixtures are small, authored here, and no generated DMB/RSC/PNG files are tracked unintentionally.

### Task 8: Pin dependencies and add CI gates

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `deny.toml`
- Create: `.github/workflows/ci.yml`
- Create: `.github/workflows/byond-integration.yml`
- Modify: `docs/dependency-policy.md`
- Modify: `docs/compatibility.md`

**Interfaces:**
- Consumes: Rust 1.88, exact SpacemanDMM commit, fixture scripts.
- Produces: reproducible dependency resolution and split per-change/scheduled CI.

- [ ] **Step 1: Declare the Rust minimum and exact dependency revisions**

Set `rust-version = "1.88"`. Replace each SpacemanDMM `branch = "master"` dependency with the same exact `rev` already selected in `Cargo.lock`, then refresh the lockfile without upgrading unrelated packages.

- [ ] **Step 2: Add advisory and license policy**

Configure `cargo-deny` to check advisories, bans, sources, and licenses. Allow only licenses demonstrated by the resolved dependency graph and document any exception with package, version, reason, and expiry/review condition.

- [ ] **Step 3: Add the per-change workflow**

Run on Windows and Linux where supported:

```text
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo deny check
cargo run --bin render_tool_docs -- docs/tool-contracts.md
git diff --exit-code -- docs/tool-contracts.md
```

- [ ] **Step 4: Add the scheduled/manual BYOND workflow**

Use a Windows runner, install the exact Meridian BYOND version, run `scripts/run-byond-integration.ps1`, upload bounded logs on failure, and avoid making this a per-change required gate until reliability and cost are accepted.

- [ ] **Step 5: Run the local reproducibility gates**

Run: `cargo update -p dreammaker --precise 7fdd00d8e9b7f7583df4960b5ed38269685ec432`

Expected: the locked SpacemanDMM packages remain on the intended revision.

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Run: `cargo test --all-features`

Expected: all commands pass.

- [ ] **Step 6: Review checkpoint**

Run: `git diff --check` and `git status --short`.

Expected: lockfile changes are limited to intentional dependency declarations or SDK preparation.

### Task 9: Migrate stdio transport to official `rmcp`

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Create: `src/server.rs`
- Modify: `src/mcp.rs`
- Modify: `src/main.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/*.rs`
- Modify: `test_mcp.ps1`
- Create: `tests/mcp_conformance.rs`

**Interfaces:**
- Consumes: typed parameters, contract metadata, capability mode, and existing domain adapters.
- Produces: `MeridianServer::new(ServerConfig)`, `MeridianServer::into_router()`, an `rmcp::ServerHandler`, and standards-compatible stdio lifecycle.

- [ ] **Step 1: Pin the official SDK**

Add:

```toml
rmcp = { version = "=3.1.3", features = ["server", "transport-io", "schemars"] }
schemars = "1"
```

Use the exact stable release in the lockfile. Do not follow the SDK `main` branch.

- [ ] **Step 2: Write failing in-process MCP tests**

Use the SDK's worker/in-process test transport or duplex Tokio streams to initialize a client, list tools in analysis mode, call `dm_parse_environment`, and verify an active tool is absent. Run the same test in development mode and verify active tools are present.

- [ ] **Step 3: Implement cloneable server state**

```rust
#[derive(Clone)]
pub struct MeridianServer {
	config: std::sync::Arc<ServerConfig>,
	state: std::sync::Arc<tokio::sync::Mutex<ServerState>>,
}
```

Tool wrappers deserialize typed parameters, acquire state only for the duration required, call domain adapters, and convert domain `ToolResult` into `rmcp::model::CallToolResult`.

- [ ] **Step 4: Implement typed tool routes**

Use `#[tool_router]`, `#[tool]`, and `Parameters<T>`. Implement `ServerHandler` explicitly for server metadata. `MeridianServer::into_router()` obtains the generated `ToolRouter`, disables every development-only route in analysis mode, and returns `rmcp::handler::server::router::Router<Self>`. Metadata instructions preserve the parse/search/exact-verification workflow and state that MCP does not replace repository builds.

- [ ] **Step 5: Replace the hand-written stdio loop**

`mcp::run_server(config)` must call:

```rust
let service = MeridianServer::new(config)
	.into_router()
	.serve(rmcp::transport::stdio())
	.await?;
service.waiting().await?;
```

Delete private hand-written JSON-RPC request/response/framing types after the SDK-backed tests pass.

- [ ] **Step 6: Update the PowerShell smoke harness**

Stop asserting a hard-coded `2024-11-05` response. Exercise the lifecycle negotiated by the installed SDK/client, list exact expected tool sets per mode, call parse/search/error paths, and fail on stdout contamination.

- [ ] **Step 7: Run focused and full MCP gates**

Run: `cargo test --test mcp_conformance -- --nocapture`

Expected: analysis/development inventory, initialization compatibility, schema, calls, and errors pass.

Run: `cargo build --release`

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\test_mcp.ps1 -ServerPath .\target\release\meridian-mcp.exe`

Expected: installed-style stdio smoke test passes with no protocol bytes written to stderr and no logs written to stdout.

- [ ] **Step 8: Review checkpoint**

Run: `git diff --check` and `git status --short`.

Expected: hand-written transport is gone; DreamMaker domain adapters remain independently testable.

### Task 10: Final public documentation and installed validation

**Files:**
- Create: `docs/architecture.md`
- Create: `docs/security.md`
- Modify: `README.md`
- Modify: `TESTING.md`
- Modify: `SECURITY.md`
- Modify: `CONTRIBUTING.md`
- Modify: `CHANGELOG.md`
- Modify: `docs/compatibility.md`
- Modify: `docs/provenance.md`
- Modify: `docs/tool-contracts.md`

**Interfaces:**
- Consumes: completed implementation and fresh test evidence.
- Produces: public documentation whose support claims exactly match verified behavior.

- [ ] **Step 1: Document final component and state boundaries**

Describe transport, server, contracts, project profiles, path policy, analysis engine, active operations, parse generations, runtime ownership, and error taxonomy. Include the two capability modes and configuration variables.

- [ ] **Step 2: Update evidence labels from fresh results only**

Promote a capability to verified only when its committed tests and named integration gate passed. Record BYOND 516.1685, Rust 1.88, `rmcp` 3.1.3, the SpacemanDMM revision, Windows version, and Codex test date. Leave unrun platforms unsupported or best-effort.

- [ ] **Step 3: Run all local automated gates**

Run: `cargo fmt --all -- --check`

Run: `cargo clippy --all-targets --all-features -- -D warnings`

Run: `cargo test --all-features`

Run: `cargo build --release`

Run: `cargo deny check`

Expected: every command passes.

- [ ] **Step 4: Run installed-binary and real-project gates**

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\test_mcp.ps1 -ServerPath .\target\release\meridian-mcp.exe -DmePath 'C:\Users\Zoe\Documents\GitHub\Meridian-Rift\tgstation.dme' -SearchQuery 'storage navigation exit button'`

Run: `powershell -NoProfile -ExecutionPolicy Bypass -File .\scripts\run-byond-integration.ps1 -MeridianRiftRoot 'C:\Users\Zoe\Documents\GitHub\Meridian-Rift'`

Expected: stdio, repository parse/search, fresh fixtures, runtime readiness, Topic, map, and performance gates pass.

- [ ] **Step 5: Validate through Codex after installation**

Install or point the Codex MCP configuration at the release binary in analysis mode, restart Codex, call parse/search/exact-definition tools against Meridian-Rift, then repeat in explicitly enabled development mode for a safe fixture checkout. Record the tested client version and date.

- [ ] **Step 6: Final working-tree and documentation review**

Run: `git diff --check`

Run: `git status --short`

Expected: all changes are intentional, no credentials or generated runtime artifacts are tracked, and the work remains uncommitted for maintainer review.
