# SpacemanDMM Foundation and Language Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the exact Rust/SpacemanDMM baseline, capability inventory, immutable analysis snapshot, and complete MCP-native language and DreamChecker query surface.

**Architecture:** Parse with the pinned `dreammaker` pipeline using proc bodies and preprocessor history, extract owned macro records before dropping the non-`Send` history, build all language indexes before atomically installing an `Arc<AnalysisSnapshot>`, and let tools clone that snapshot without holding the server state lock during expensive work. A checked-in capability registry makes every upstream mapping or exclusion auditable.

**Tech Stack:** Rust 1.95, Tokio `RwLock`/`Mutex`, serde/schemars, `dreammaker`, `dreamchecker`, `dmi`, `dmm-tools`, PowerShell, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-24-spacemandmm-complete-integration-design.md`

## Global Constraints

- Apply every constraint from `2026-08-24-spacemandmm-complete-integration.md`.
- Use `dreammaker::Preprocessor`, `Parser::enable_procs`, `Parser::parse_object_tree_2`, and `Preprocessor::finalize` so macros and proc bodies come from one parse.
- Do not copy an LSP transport or hold editor document buffers.
- Build an MCP-native reference/implementation index from canonical on-disk source.
- Failed parsing must preserve the complete prior generation.
- Leave changes uncommitted absent explicit authorization.

---

### Task 1: Pin Rust 1.95 and the approved upstream revision

**Files:**
- Create: `tests/dependency_baseline.rs`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `rust-toolchain.toml`
- Modify: `.github/workflows/ci.yml`
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `TESTING.md`
- Modify: `docs/dependency-policy.md`

**Interfaces:**
- Consumes: Existing Cargo package and workflow definitions.
- Produces: Exact `RUST_VERSION = "1.95.0"` and `SPACEMANDMM_REVISION = "351ddc0ffb2439876d4565ce5130bb6b027ee605"` baselines enforced by tests.

- [ ] **Step 1: Write the failing baseline tests**

Create `tests/dependency_baseline.rs` with checks that read repository files rather than shell state:

```rust
const REVISION: &str = "351ddc0ffb2439876d4565ce5130bb6b027ee605";

#[test]
fn rust_toolchain_and_manifest_require_1_95() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let toolchain = std::fs::read_to_string(root.join("rust-toolchain.toml")).unwrap();
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(toolchain.contains("channel = \"1.95.0\""));
    assert!(manifest.contains("rust-version = \"1.95\""));
}

#[test]
fn every_spacemandmm_dependency_uses_the_approved_revision() {
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .unwrap();
    for package in ["dreammaker", "dreamchecker", "dmi", "dmm-tools"] {
        let line = manifest
            .lines()
            .find(|line| line.starts_with(package))
            .unwrap_or_else(|| panic!("missing {package} dependency"));
        assert!(line.contains(REVISION), "{package} is not pinned to {REVISION}");
    }
}
```

- [ ] **Step 2: Run the baseline test and confirm the old pins fail**

Run: `cargo test --test dependency_baseline`

Expected: failure naming Rust 1.88 and/or the old SpacemanDMM revision.

- [ ] **Step 3: Update manifest, toolchain, and workflows**

Set:

```toml
[package]
rust-version = "1.95"

[dependencies]
dreammaker = { version = "=0.1.0", git = "https://github.com/SpaceManiac/SpacemanDMM", rev = "351ddc0ffb2439876d4565ce5130bb6b027ee605" }
dreamchecker = { version = "=1.11.0", git = "https://github.com/SpaceManiac/SpacemanDMM", rev = "351ddc0ffb2439876d4565ce5130bb6b027ee605" }
dmi = { version = "=0.1.0", git = "https://github.com/SpaceManiac/SpacemanDMM", rev = "351ddc0ffb2439876d4565ce5130bb6b027ee605" }
dmm-tools = { version = "=0.1.0", git = "https://github.com/SpaceManiac/SpacemanDMM", rev = "351ddc0ffb2439876d4565ce5130bb6b027ee605", features = ["png", "gif"] }
```

Set `rust-toolchain.toml` to `channel = "1.95.0"`. Change both workflow actions to `dtolnay/rust-toolchain@1.95.0`; keep `rustfmt` and `clippy` components in the Rust workflow.

- [ ] **Step 4: Regenerate and inspect the lockfile**

Run:

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc cargo update -p dreammaker@0.1.0
rustup run 1.95.0-x86_64-pc-windows-msvc cargo check --all-features
git diff -- Cargo.lock
```

Expected: all SpacemanDMM packages resolve to the approved Git revision; unrelated direct dependencies do not change without a documented compatibility requirement.

- [ ] **Step 5: Run the baseline test and exact compiler check**

Run:

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc rustc --version
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --test dependency_baseline
```

Expected: `rustc 1.95.0` and two passing tests.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `build: upgrade Rust and SpacemanDMM pins`.

---

### Task 2: Add the exact capability registry and audit command

**Files:**
- Create: `spacemandmm-capabilities.json`
- Create: `src/capabilities.rs`
- Create: `tests/capability_registry.rs`
- Create: `scripts/audit-spacemandmm-capabilities.ps1`
- Modify: `src/lib.rs`
- Modify: `tests/workflow_contract.rs`

**Interfaces:**
- Consumes: `src/contracts.rs::all_contracts()` and the exact revision constant.
- Produces:

```rust
pub const SPACEMANDMM_REVISION: &str;
pub enum CapabilityDisposition { Direct, McpNative, FixedHelper, Superseded, Excluded }
pub struct CapabilityRecord {
    pub id: String,
    pub category: String,
    pub upstream_component: String,
    pub disposition: CapabilityDisposition,
    pub target: String,
    pub platforms: Vec<String>,
    pub verification: String,
    pub rationale: Option<String>,
}
pub fn capability_registry() -> Result<CapabilityRegistry, CapabilityRegistryError>;
pub fn validate_capability_registry(registry: &CapabilityRegistry) -> Result<(), Vec<String>>;
```

- [ ] **Step 1: Write failing registry validation tests**

Create tests that require the approved revision, unique IDs, nonempty verification, valid tool targets, and rationale for every exclusion:

```rust
#[test]
fn checked_in_registry_is_complete_and_consistent() {
    let registry = meridian_mcp::capabilities::capability_registry().unwrap();
    assert_eq!(registry.spacemandmm_revision, meridian_mcp::capabilities::SPACEMANDMM_REVISION);
    assert_eq!(
        meridian_mcp::capabilities::validate_capability_registry(&registry),
        Ok(())
    );
}

#[test]
fn every_public_tool_has_at_least_one_registry_mapping() {
    let registry = meridian_mcp::capabilities::capability_registry().unwrap();
    for contract in meridian_mcp::all_contracts() {
        assert!(
            registry.capabilities.iter().any(|record| record.target == contract.name),
            "{} has no capability mapping",
            contract.name
        );
    }
}
```

- [ ] **Step 2: Run the registry test and confirm the module is missing**

Run: `cargo test --test capability_registry`

Expected: compile failure because `meridian_mcp::capabilities` does not exist.

- [ ] **Step 3: Implement the registry model and checked-in inventory**

Use `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/spacemandmm-capabilities.json"))` and `serde_json::from_str`. Populate rows for every capability listed in the approved spec, including internal crates, LSP behavior, DMM CLI commands, DMI behavior, debugger requests, fixed helpers, superseded MCP equivalents, and explicit exclusions.

Validation must reject:

```rust
if record.verification.trim().is_empty() {
    errors.push(format!("{} has no verification", record.id));
}
if record.disposition == CapabilityDisposition::Excluded
    && record.rationale.as_deref().is_none_or(str::is_empty)
{
    errors.push(format!("{} has no exclusion rationale", record.id));
}
```

- [ ] **Step 4: Implement the PowerShell audit**

`scripts/audit-spacemandmm-capabilities.ps1` accepts `-Check` and optional `-UpstreamPath`. It must:

```powershell
param(
	[switch]$Check,
	[string]$UpstreamPath
)

$expectedRevision = '351ddc0ffb2439876d4565ce5130bb6b027ee605'
$registry = Get-Content -LiteralPath "$PSScriptRoot\..\spacemandmm-capabilities.json" -Raw | ConvertFrom-Json
if ($registry.spacemandmm_revision -ne $expectedRevision) { throw 'Capability registry revision mismatch.' }
if ($UpstreamPath) {
	$actualRevision = git -C $UpstreamPath rev-parse HEAD
	if ($actualRevision -ne $expectedRevision) { throw "Upstream checkout is $actualRevision." }
}
```

When `-UpstreamPath` is supplied, additionally enumerate the workspace members, language-server capability fields, DMM CLI command variants, and debugger request table from the pinned source and compare them with registry `evidence` values. The script reports each unmapped identifier and exits 1.

- [ ] **Step 5: Run registry and script tests**

Run:

```powershell
rustup run 1.95.0-x86_64-pc-windows-msvc cargo test --test capability_registry
.\scripts\audit-spacemandmm-capabilities.ps1 -Check
```

Expected: all registry tests pass and the script exits 0.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: add SpacemanDMM capability registry`.

---

### Task 3: Add shared atomic output handling

**Files:**
- Create: `src/atomic_output.rs`
- Create: `tests/atomic_output.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: `PathPolicy::output_path` and an existing contained output parent.
- Produces:

```rust
pub struct OutputArtifact {
    pub path: std::path::PathBuf,
    pub bytes: u64,
    pub sha256: String,
}

pub fn write_atomic<F>(
    policy: &crate::PathPolicy,
    output: &std::path::Path,
    overwrite: bool,
    write: F,
) -> Result<OutputArtifact, AtomicOutputError>
where
    F: FnOnce(&mut std::fs::File) -> Result<(), AtomicOutputError>;
```

- [ ] **Step 1: Write failing containment and cleanup tests**

Test rejection outside roots, rejection of an existing output without `overwrite`, successful new output, successful replacement, original restoration after replacement failure, and temporary-file cleanup after writer failure.

- [ ] **Step 2: Run the test and confirm the helper is absent**

Run: `cargo test --test atomic_output --all-features`

Expected: compile failure because `meridian_mcp::atomic_output` does not exist.

- [ ] **Step 3: Implement contained temporary output**

Validate the final path first. Create a random, non-client-controlled temporary filename in the same existing parent, call the writer, flush and close, calculate SHA-256, then rename into place. On Windows replacement, move the existing output to a contained backup, move the temporary file to the final path, and restore the backup if the second move fails. Remove the backup only after success.

- [ ] **Step 4: Run the output and path-policy tests**

Run:

```powershell
cargo test --test atomic_output --all-features
cargo test --test config_and_paths --all-features
```

Expected: all output and containment tests pass with no retained temporary files.

- [ ] **Step 5: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: add contained atomic outputs`.

---

### Task 4: Standardize tool provenance, truncation, and structured errors

**Files:**
- Create: `tests/tool_results.rs`
- Modify: `src/result.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/contracts.rs`
- Modify: `src/lib.rs`

**Interfaces:**
- Consumes: Existing `ToolResult` construction, tool contracts, and server version constants.
- Produces:

```rust
pub struct ToolMetadata {
    pub meridian_mcp_version: &'static str,
    pub spacemandmm_revision: &'static str,
    pub state_generation: Option<u64>,
    pub asset_generation: Option<u64>,
    pub truncated: bool,
    pub truncation_reasons: Vec<String>,
}

pub enum ToolErrorCode {
    InvalidInput,
    PathOutsideWorkspace,
    ParseRequired,
    StaleGeneration,
    NotFound,
    AmbiguousSymbol,
    UnsupportedUpstream,
    LimitExceeded,
    TimedOut,
    PartialEvidence,
    HelperFailure,
    HelperChecksumMismatch,
    ExternalToolFailure,
    ToolNotAvailable,
    Internal,
}

pub fn json_success<T: serde::Serialize>(metadata: ToolMetadata, data: T) -> ToolResult;
pub fn structured_error(
    code: ToolErrorCode,
    message: impl Into<String>,
    recovery: Option<String>,
    details: serde_json::Value,
) -> ToolResult;
```

- [ ] **Step 1: Write failing result-contract tests**

Create tests that require every `ToolErrorCode` to serialize as stable `snake_case`, every success result to contain version and truncation metadata, and every structured error to contain `code`, `message`, `recovery`, and object-valued `details`. Add compatibility assertions that current tool-specific payload fields remain available.

- [ ] **Step 2: Run the result tests and confirm the shared model is absent**

Run: `cargo test --test tool_results --all-features`

Expected: compile failure for `ToolMetadata`, `ToolErrorCode`, `json_success`, and `structured_error`.

- [ ] **Step 3: Implement the shared metadata and error serializers**

Use `#[serde(rename_all = "snake_case")]` for `ToolErrorCode`. Serialize success responses as an additive object merge so existing top-level payload keys are retained. Reject non-object success payloads at construction time. Always include `truncated` and `truncation_reasons`; use empty reasons when the result is complete.

- [ ] **Step 4: Migrate common dispatch and policy failures**

Convert unknown-tool, mode-policy, missing-parse, invalid-parameter, path-containment, stale-generation, and internal-dispatch failures to the shared shape. Preserve current human-readable text and legacy fields where clients already consume them. Individual feature stages must use these constructors instead of introducing incompatible error envelopes.

- [ ] **Step 5: Run result, conformance, and full regression tests**

Run:

```powershell
cargo test --test tool_results --all-features
cargo test --test mcp_conformance --all-features
cargo test --test active_tool_policy --all-features
cargo test --all-features
```

Expected: stable structured codes, complete metadata, and no removed current response fields.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: standardize MCP tool results`.

---

### Task 5: Refactor shared state around immutable analysis snapshots

**Files:**
- Create: `src/analysis_snapshot.rs`
- Create: `src/limits.rs`
- Modify: `src/state.rs`
- Modify: `src/server.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/parse.rs`
- Modify: `src/tools/analysis.rs`
- Modify: `src/tools/search.rs`
- Modify: `src/tools/map.rs`
- Modify: `src/tools/rift.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/lib.rs`
- Modify: `tests/fixture_corpus.rs`

**Interfaces:**
- Consumes: Parsed `Context`, `ObjectTree`, `SearchIndex`, and `ProjectProfile`.
- Produces:

```rust
pub struct AnalysisSnapshot {
    pub environment_path: std::path::PathBuf,
    pub context: std::sync::Arc<crate::analysis_snapshot::AnalysisContext>,
    pub objtree: std::sync::Arc<dreammaker::objtree::ObjectTree>,
    pub search_index: std::sync::Arc<crate::search::SearchIndex>,
    pub language_index: std::sync::Arc<crate::index::LanguageIndex>,
    pub project_profile: Option<crate::ProjectProfile>,
    pub generation: u64,
    pub spacemandmm_revision: &'static str,
}

pub struct AnalysisContext {
    pub config: dreammaker::config::Config,
    pub file_paths: std::collections::HashMap<dreammaker::FileId, std::path::PathBuf>,
}

pub struct ServerState {
    analysis: tokio::sync::RwLock<AnalysisState>,
    runtime: tokio::sync::Mutex<RuntimeState>,
}

impl ServerState {
    pub async fn snapshot(&self) -> Result<std::sync::Arc<AnalysisSnapshot>, StateError>;
    pub async fn install_analysis(&self, build: AnalysisBuild) -> std::sync::Arc<AnalysisSnapshot>;
}
```

- [ ] **Step 1: Write failing atomic-generation and concurrency tests**

Add tests proving that snapshot handles survive a replacement and failed build:

```rust
#[tokio::test]
async fn installed_snapshot_remains_valid_after_next_generation() {
    let state = ServerState::new();
    let first = state.install_analysis(test_analysis_build("one.dme")).await;
    let held = state.snapshot().await.unwrap();
    let second = state.install_analysis(test_analysis_build("two.dme")).await;
    assert_eq!(held.generation, first.generation);
    assert_eq!(second.generation, first.generation + 1);
    assert!(held.environment_path.ends_with("one.dme"));
}
```

Extend `tests/fixture_corpus.rs` to launch a deliberately slow read from a cloned snapshot while a second parse installs a generation; both operations must finish without deadlock.

- [ ] **Step 2: Run the focused tests and confirm the new APIs are absent**

Run:

```powershell
cargo test --all-features state::tests
cargo test --test fixture_corpus --all-features
```

Expected: compile failure for `install_analysis`, `snapshot`, and `AnalysisSnapshot`.

- [ ] **Step 3: Implement `ServerLimits` and snapshot domain types**

Create immutable defaults with named constants rather than client-controlled maxima:

```rust
#[derive(Clone, Debug)]
pub struct ServerLimits {
    pub max_result_bytes: usize,
    pub max_blocking_jobs: usize,
    pub max_reference_results: usize,
    pub max_document_symbols: usize,
}

impl Default for ServerLimits {
    fn default() -> Self {
        Self {
            max_result_bytes: 1_048_576,
            max_blocking_jobs: 4,
            max_reference_results: 10_000,
            max_document_symbols: 20_000,
        }
    }
}
```

Later stages extend the same struct with DMI, rendering, helper, and debugger maxima.

- [ ] **Step 4: Remove the outer server lock**

Change `MeridianServer` to store `Arc<ServerState>`. Change dispatch to:

```rust
let result = tools::call_tool(
    &self.execution,
    self.state.as_ref(),
    &request.name,
    arguments,
)
.await
.unwrap_or_else(|error| DomainToolResult::error(error.to_string()));
```

Change tool functions to accept `&ServerState`. Analysis tools call `state.snapshot().await?` and perform work after the lock is released. Runtime methods acquire only `state.runtime()`.

- [ ] **Step 5: Parse off-lock and install atomically**

`dm_parse_environment` canonicalizes first, then runs this shape inside `tokio::task::spawn_blocking`:

```rust
let context = dreammaker::Context::default();
context.autodetect_config(&dme_path);
let mut preprocessor = dreammaker::Preprocessor::new(&context, dme_path.clone())?;
let mut parser = dreammaker::Parser::new(&context, &mut preprocessor);
parser.enable_procs();
let (fatal, objtree) = parser.parse_object_tree_2();
let defines = preprocessor.finalize();
AnalysisBuild::from_parse(dme_path, context, objtree, defines, fatal)
```

Build every index before `install_analysis`. If the blocking task returns an I/O failure, return `state_preserved: true` and the active generation without acquiring a write lock.

- [ ] **Step 6: Run state, fixture, and full regression tests**

Run:

```powershell
cargo test state::tests --all-features
cargo test --test fixture_corpus --all-features
cargo test --all-features
```

Expected: all tests pass and existing runtime/compile behavior remains unchanged.

- [ ] **Step 7: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `refactor: install immutable analysis snapshots`.

---

### Task 6: Index macros and document symbols

**Files:**
- Create: `src/index/mod.rs`
- Create: `src/index/symbols.rs`
- Create: `src/spaceman/mod.rs`
- Create: `src/spaceman/language.rs`
- Create: `src/tools/language.rs`
- Create: `tests/language_capabilities.rs`
- Modify: `src/analysis_snapshot.rs`
- Modify: `src/lib.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `tests/fixtures/language/fixture.dm`

**Interfaces:**
- Consumes: owned macro records extracted from `DefineHistory`, `ObjectTree`, immutable context data, and snapshot generation. Never retain `DefineHistory`; its documentation uses `Rc` and is not `Send` or `Sync`.
- Produces:

```rust
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum SymbolId {
    Type(String),
    Proc { owner: String, name: String, override_index: usize },
    Var { owner: String, name: String },
    Macro { name: String, file: String, line: u32 },
}

pub struct DocumentSymbol {
    pub id: SymbolId,
    pub name: String,
    pub kind: SymbolKind,
    pub owner: Option<String>,
    pub file: String,
    pub line: u32,
    pub column: u16,
}

pub fn document_symbols(snapshot: &AnalysisSnapshot, file: &std::path::Path) -> Vec<DocumentSymbol>;
```

- [ ] **Step 1: Extend the fixture and write failing symbol tests**

Add a documented macro and a nested type to `fixture.dm`:

```dm
/// Return a fixed fixture value.
#define MERIDIAN_FIXTURE_VALUE 7

/datum/fixture_symbol_parent
	var/value = MERIDIAN_FIXTURE_VALUE

/datum/fixture_symbol_parent/child
```

Write a test that calls `dm_search_symbols` with `kind: "macro"` and `dm_document_symbols` for the fixture file. Require deterministic order and positive source locations.

- [ ] **Step 2: Run the test and confirm macro/document-symbol support fails**

Run: `cargo test --test language_capabilities document_symbols --all-features`

Expected: missing tool or missing `macro` schema kind.

- [ ] **Step 3: Build the symbol index from parse artifacts**

Iterate `DefineHistory::iter()` for macro records and `ObjectTree::iter_types()` for types, vars, and proc values. Normalize file paths through `Context::file_path`. Store `BTreeMap<PathBuf, Vec<DocumentSymbol>>`; sort each vector by `(line, column, kind, name)`.

- [ ] **Step 4: Add the MCP contract**

Add `DocumentSymbolsParams { file_path: PathBuf, limit: Option<usize> }`, canonicalize `file_path` through `PathPolicy::read_path`, enforce `ServerLimits::max_document_symbols`, and return:

```json
{
  "state_generation": 1,
  "spacemandmm_revision": "351ddc0ffb2439876d4565ce5130bb6b027ee605",
  "count": 2,
  "truncated": false,
  "symbols": []
}
```

Extend `dm_search_symbols` kind enumeration to `type`, `proc`, `var`, `macro`, and `all`.

- [ ] **Step 5: Run focused and contract tests**

Run:

```powershell
cargo test --test language_capabilities document_symbols --all-features
cargo test --test tool_contracts --all-features
cargo test --test active_tool_policy --all-features
```

Expected: all pass; `dm_document_symbols` appears in analysis and development modes.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: index macros and document symbols`.

---

### Task 7: Add reference and implementation indexes

**Files:**
- Create: `src/index/references.rs`
- Create: `src/index/implementations.rs`
- Modify: `src/index/mod.rs`
- Modify: `src/spaceman/language.rs`
- Modify: `src/tools/language.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `src/tools/mod.rs`
- Modify: `tests/language_capabilities.rs`
- Modify: `tests/fixtures/language/fixture.dm`

**Interfaces:**
- Consumes: `SymbolId`, proc-body AST blocks retained by `Parser::enable_procs`, and the `ObjectTree` inheritance graph.
- Produces:

```rust
pub enum ReferenceKind { Call, Read, Write, TypePath, MacroExpansion }
pub struct ReferenceHit {
    pub symbol: SymbolId,
    pub kind: ReferenceKind,
    pub file: String,
    pub line: u32,
    pub column: u16,
}
pub struct ImplementationHit {
    pub symbol: SymbolId,
    pub declared_in: String,
    pub inherited_from: Option<String>,
    pub file: String,
    pub line: u32,
    pub column: u16,
}
pub fn references_for(&self, symbol: &SymbolId) -> &[ReferenceHit];
pub fn implementations_for(&self, owner: &str, member: Option<&str>) -> Vec<ImplementationHit>;
```

- [ ] **Step 1: Add fixture relationships and failing tests**

Add a parent proc, child override, variable read/write, and call:

```dm
/datum/fixture_symbol_parent/proc/compute(input)
	value = input
	return value

/datum/fixture_symbol_parent/child/compute(input)
	return ..(input)
```

Require `dm_find_references` for `value` to return at least one write and one read, and `dm_find_implementations` for `compute` to return parent then child in deterministic inheritance order.

- [ ] **Step 2: Run the focused tests and confirm missing tools**

Run: `cargo test --test language_capabilities references_and_implementations --all-features`

Expected: failure because the two tool contracts do not exist.

- [ ] **Step 3: Implement canonical symbol resolution**

Resolve requested tool parameters into one `SymbolId`; reject ambiguous names with a structured `ambiguous_symbol` result listing canonical candidates. Use exact type path plus member kind/name, never a free-form cursor position.

- [ ] **Step 4: Build references from retained proc ASTs**

Adapt the pinned `dm-langserver/src/find_references.rs` resolution rules into a transport-independent visitor. Record only references resolved to a canonical `SymbolId`; classify unresolved dynamic expressions as skipped statistics, not guessed references. Sort hits by `(file, line, column, kind)` and deduplicate exact tuples.

- [ ] **Step 5: Build implementations from the object tree**

For types, walk descendants with declarations. For procs, collect every concrete proc value and override owner; report inherited resolution separately. Do not treat an inherited lookup as a new declaration.

- [ ] **Step 6: Add schemas and bounded tool results**

Use:

```rust
pub struct FindReferencesParams {
    pub type_path: String,
    pub member_name: Option<String>,
    pub kind: Option<String>,
    pub include_declaration: Option<bool>,
    pub limit: Option<usize>,
}

pub struct FindImplementationsParams {
    pub type_path: String,
    pub member_name: Option<String>,
    pub limit: Option<usize>,
}
```

Clamp limits to server maxima and return `truncated`, `skipped_dynamic`, and generation metadata.

- [ ] **Step 7: Run focused, snapshot, and full tests**

Run:

```powershell
cargo test --test language_capabilities --all-features
cargo test --test fixture_corpus --all-features
cargo test --all-features
```

Expected: all pass and reference ordering repeats exactly across identical parses.

- [ ] **Step 8: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: add DreamMaker references and implementations`.

---

### Task 8: Enrich DreamChecker and exact inspection results

**Files:**
- Modify: `src/spaceman/language.rs`
- Modify: `src/tools/analysis.rs`
- Modify: `src/tools/parse.rs`
- Modify: `src/search.rs`
- Modify: `tests/language_capabilities.rs`
- Modify: `tests/compatibility/meridian-rift.json`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: `DMError::{errortype, component, description, notes, severity, location}`, `Context::config`, and the immutable snapshot.
- Produces:

```rust
pub struct DiagnosticRecord {
    pub rule: Option<String>,
    pub severity: String,
    pub component: String,
    pub message: String,
    pub file: String,
    pub line: u32,
    pub column: u16,
    pub notes: Vec<DiagnosticNoteRecord>,
    pub configured: bool,
}
```

- [ ] **Step 1: Write a failing detailed-diagnostic test**

Add a temporary fixture with one stable DreamChecker warning and assert that `dm_check_errors` includes `rule`, `component`, `notes`, `state_generation`, and `spacemandmm_revision` keys without removing current fields.

- [ ] **Step 2: Run the test and confirm missing detail**

Run: `cargo test --test language_capabilities dreamchecker_details --all-features`

Expected: failure for missing diagnostic keys.

- [ ] **Step 3: Run DreamChecker without mutating shared snapshot diagnostics**

Because `dreamchecker::run` registers into `Context`, build checker diagnostics during snapshot construction before installation, or run against the private parse context before extracting immutable context data. Never retain or share upstream `Context`: its `RefCell` internals are not `Sync`. Store the resulting `Arc<[DiagnosticRecord]>` in `AnalysisSnapshot`.

- [ ] **Step 4: Add rule/configuration metadata**

Map `errortype()` to `rule`, `component()` to a stable lowercase string, `notes()` to source-located records, and configuration discovery to the contained `SpacemanDMM.toml` path when present. `configured` is true only when a rule identifier exists and the loaded configuration contains an override for it; do not infer suppression records for diagnostics that upstream removed.

- [ ] **Step 5: Enrich existing inspection tools additively**

Add structured `file`, `line`, `column`, `state_generation`, and `spacemandmm_revision` fields while retaining current `location` strings for compatibility. Apply the common provenance and truncation envelope to `dm_search_context`, `dm_list_types`, `dm_get_type`, and `dm_get_var`; do not change their established payload keys or lookup semantics. `dm_get_proc` retains parameters and source; `dm_get_definition` adds `declaration_kind` and resolved type owner.

- [ ] **Step 6: Run all language and compatibility-manifest tests**

Run:

```powershell
cargo test --test language_capabilities --all-features
cargo test --test compatibility_manifest --all-features
cargo test --all-features
```

Expected: all pass with no removed response keys.

- [ ] **Step 7: Run the Stage 1 aggregate gate**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
.\test_mcp.ps1 -SkipBuild -BinaryPath .\target\release\meridian-mcp.exe -Mode analysis -DmePath .\tests\fixtures\language\fixture.dme -SearchQuery 'fixture compute'
```

Expected: every command exits 0 and the shipped stdio path advertises and invokes the new analysis tools.

- [ ] **Step 8: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: complete SpacemanDMM language analysis`.
