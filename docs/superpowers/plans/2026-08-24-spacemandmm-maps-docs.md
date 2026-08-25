# SpacemanDMM Maps and Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Complete SpacemanDMM DMM inspection, map diffing, render-pass, bounded single/batch rendering, and dmdoc behavior through contained typed MCP contracts while preserving existing `dm_find_on_map` lookup behavior.

**Architecture:** Use direct `dmm-tools` library adapters for maps and rendering against the immutable analysis snapshot. Build dmdoc as an exact-revision packaging helper, embed its per-platform hash manifest into the Meridian-MCP release, and advertise documentation generation only when the verified helper is present.

**Tech Stack:** Rust 1.95, `dmm-tools`, `dreammaker`, PNG output, fixed dmdoc binary, Tokio bounded blocking workers, PowerShell packaging and stdio tests.

**Spec:** `docs/superpowers/specs/2026-08-24-spacemandmm-complete-integration-design.md`

## Global Constraints

- Complete the foundation/language plan first.
- Reuse `Arc<AnalysisSnapshot>` and the shared `write_atomic` helper.
- Map inspection/diff tools are read-only; rendering and dmdoc output are development-only.
- Never expose the raw `dmm-tools` CLI, raw `RenderMany` JSON, arbitrary helper arguments, or external PNG optimizers.
- Bounds are one-indexed and inclusive at the MCP boundary.
- Leave changes uncommitted absent explicit authorization.

---

### Task 1: Complete map information and structured map differences

**Files:**
- Create: `src/spaceman/dmm.rs`
- Create: `tests/map_capabilities.rs`
- Modify: `src/spaceman/mod.rs`
- Modify: `src/tools/map.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `src/tools/mod.rs`
- Modify: `tests/fixtures/maps/fixture.dmm`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: `dmm_tools::dmm::Map`, contained DMM/TGM files, result limits.
- Produces:

```rust
pub struct MapProfile {
    pub path: std::path::PathBuf,
    pub format: String,
    pub dimensions: [u32; 3],
    pub bounds: MapBounds,
    pub dictionary_entries: usize,
    pub unique_models: usize,
    pub model_use_counts: Vec<ModelUseCount>,
    pub warnings: Vec<MapWarning>,
}

pub struct MapDifference {
    pub coordinates: Vec<CoordinateDifference>,
    pub left_dimensions: [u32; 3],
    pub right_dimensions: [u32; 3],
    pub truncated: bool,
}

pub fn profile_map(path: &std::path::Path, limit: usize) -> Result<MapProfile, DmmError>;
pub fn diff_maps(left: &std::path::Path, right: &std::path::Path, limit: usize) -> Result<MapDifference, DmmError>;
```

- [ ] **Step 1: Add a second map fixture and failing diff tests**

Create a second textual DMM fixture by copying the technical fixture map structure and changing one dictionary model, one coordinate, and one dimension. Assert the exact coordinate and before/after model strings while allowing dictionary keys to differ.

- [ ] **Step 2: Run focused tests and confirm `dm_diff_maps` is absent**

Run: `cargo test --test map_capabilities map_info_and_diff --all-features`

Expected: missing diff tool and missing enhanced map-profile fields.

- [ ] **Step 3: Implement the direct DMM adapter**

Parse each file once. Derive canonical `[max_x, max_y, max_z]`, min/max bounds, dictionary/model counts, and deterministic model-use counts sorted by descending count then model string. Return parser warnings instead of printing them.

For differences, compare coordinate models after resolving each file's independent dictionary keys. Report `left: null` or `right: null` for coordinates added or removed by dimension changes. Stop at the requested limit clamped to the server maximum.

- [ ] **Step 4: Add `dm_diff_maps` and enhance `dm_map_info` additively**

Use:

```rust
pub struct DiffMapsParams {
    pub left_dmm_path: PathBuf,
    pub right_dmm_path: PathBuf,
    pub limit: Option<usize>,
}
```

Canonicalize both inputs. Keep every current `dm_map_info` response field and add `bounds`, `dictionary_entries`, `unique_models`, `model_use_counts`, `warnings`, and upstream revision.

- [ ] **Step 5: Run map, contract, and path tests**

Run:

```powershell
cargo test --test map_capabilities map_info_and_diff --all-features
cargo test --test config_and_paths --all-features
cargo test --test tool_contracts --all-features
```

Expected: all pass and both tools are analysis/read-only.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: add structured DMM differences`.

---

### Task 2: Add render-pass inventory and bounded single-map rendering

**Files:**
- Modify: `src/spaceman/dmm.rs`
- Modify: `src/tools/map.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `src/limits.rs`
- Modify: `tests/map_capabilities.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: `dmm_tools::render_passes::RENDER_PASSES`, `minimap::Context`, `IconCache`, snapshot object tree, atomic output.
- Produces:

```rust
pub struct RenderPassRecord { pub name: String, pub description: String, pub default_enabled: bool }
pub struct MapRenderRequest { pub dmm_path: PathBuf, pub z_level: usize, pub bounds: Option<MapBounds>, pub enable_passes: Vec<String>, pub disable_passes: Vec<String> }
pub fn render_passes() -> Vec<RenderPassRecord>;
pub fn render_map(snapshot: &AnalysisSnapshot, request: &MapRenderRequest, limits: &ServerLimits) -> Result<dmm_tools::dmi::Image, DmmError>;
```

- [ ] **Step 1: Write failing pass and bounds tests**

Require the pass list to match `RENDER_PASSES` name/description/default values, unknown pass rejection, one-indexed inclusive bounds conversion, out-of-map rejection, and a bounded render whose PNG dimensions equal the selected cells times icon dimensions.

- [ ] **Step 2: Run the focused tests and confirm the fields/tool are absent**

Run: `cargo test --test map_capabilities render_passes_and_bounds --all-features`

Expected: missing `dm_list_render_passes` and old `dm_render_map` schema lacks bounds/pass fields.

- [ ] **Step 3: Extend render limits**

Add `max_render_pixels = 268_435_456`, `max_render_output_bytes = 512 * 1024 * 1024`, `max_render_files = 128`, and `max_render_chunks = 512` to `ServerLimits`. Reject requests before allocating an image whose calculated pixel count exceeds the ceiling.

- [ ] **Step 4: Implement pass selection**

Start from upstream defaults and config values, apply explicit disable entries, then enable entries. Reject unknown or duplicate-conflicting names. Keep deterministic applied-pass order matching `RENDER_PASSES`.

- [ ] **Step 5: Extend `dm_render_map` and add `dm_list_render_passes`**

Extend the existing params additively:

```rust
pub struct RenderMapParams {
    pub dmm_path: PathBuf,
    pub z_level: Option<usize>,
    pub min: Option<[u32; 3]>,
    pub max: Option<[u32; 3]>,
    pub enable_passes: Option<Vec<String>>,
    pub disable_passes: Option<Vec<String>>,
    pub output_path: Option<PathBuf>,
    pub overwrite: bool,
}
```

`dm_list_render_passes` is analysis/read-only and does not require a parse. `dm_render_map` remains development-only and uses `write_atomic`.

- [ ] **Step 6: Run renderer and policy tests**

Run:

```powershell
cargo test --test map_capabilities render_passes_and_bounds --all-features
cargo test --test active_tool_policy --all-features
cargo test --test config_and_paths --all-features
```

Expected: all pass; pass inventory appears in both modes and rendering only in development.

- [ ] **Step 7: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: expose bounded map render passes`.

---

### Task 3: Add typed batch rendering equivalent to `RenderMany`

**Files:**
- Modify: `src/spaceman/dmm.rs`
- Modify: `src/tools/map.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `src/tools/mod.rs`
- Modify: `tests/map_capabilities.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: Single-map rendering adapter, atomic output, batch limits.
- Produces:

```rust
pub struct RenderMapBatchItem {
    pub dmm_path: PathBuf,
    pub chunks: Vec<RenderChunk>,
}
pub struct RenderMapsParams {
    pub files: Vec<RenderMapBatchItem>,
    pub enable_passes: Option<Vec<String>>,
    pub disable_passes: Option<Vec<String>>,
    pub overwrite: bool,
}
pub struct BatchRenderResult { pub files: Vec<BatchFileResult>, pub completed: usize, pub failed: usize, pub truncated: bool }
```

- [ ] **Step 1: Write failing batch and partial-failure tests**

Create two bounded chunks and one invalid chunk. Require deterministic per-item results, no output for the invalid item, completed/failed counts, and cleanup of temporary files. Require rejection when files/chunks exceed server ceilings.

- [ ] **Step 2: Run tests and confirm `dm_render_maps` is absent**

Run: `cargo test --test map_capabilities batch_render --all-features`

Expected: missing batch tool.

- [ ] **Step 3: Implement typed batch orchestration**

Validate every input and output path before starting any write. Reuse parsed maps and icon cache within the call. Process chunks in request order on the bounded blocking pool. An invalid request fails before writes; a runtime encoder failure returns per-item failure after cleaning that item's temporary output.

- [ ] **Step 4: Register `dm_render_maps`**

Mark it development/read-write with explicit maximum output bytes. Do not accept raw upstream `RenderManyCommand` JSON or derive output paths from untrusted dictionary keys.

- [ ] **Step 5: Run batch, contract, and full tests**

Run:

```powershell
cargo test --test map_capabilities batch_render --all-features
cargo test --test tool_contracts --all-features
cargo test --all-features
```

Expected: all pass and current single-map rendering remains compatible.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: add typed batch map rendering`.

---

### Task 4: Build and embed a fixed dmdoc helper manifest

**Files:**
- Create: `build.rs`
- Create: `helpers/manifest.json`
- Create: `scripts/build-spacemandmm-helpers.ps1`
- Create: `src/spaceman/docs.rs`
- Create: `tests/docs_helper.rs`
- Modify: `.gitignore`
- Modify: `Cargo.toml`
- Modify: `src/spaceman/mod.rs`
- Modify: `src/config.rs`
- Modify: `.github/workflows/ci.yml`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: Exact SpacemanDMM revision, fixed helper source, current executable directory.
- Produces:

```rust
pub struct HelperArtifact { pub name: String, pub platform: String, pub relative_path: PathBuf, pub sha256: String, pub source_revision: String }
pub struct HelperRegistry { pub artifacts: Vec<HelperArtifact> }
pub fn verified_dmdoc_helper() -> Result<PathBuf, DocsHelperError>;
```

- [ ] **Step 1: Write failing helper-manifest tests**

Require the embedded manifest revision to match `SPACEMANDMM_REVISION`, reject an absent helper, reject a changed helper hash, and accept only the exact current platform entry. Tests use a compile-time test manifest and technical fake bytes; they do not execute the fake helper.

- [ ] **Step 2: Run tests and confirm helper support is absent**

Run: `cargo test --test docs_helper helper_manifest --all-features`

Expected: missing helper registry and verification function.

- [ ] **Step 3: Implement the packaging script**

The script accepts `-UpstreamPath`, `-OutputDirectory`, and `-ManifestPath`. It verifies `git -C $UpstreamPath rev-parse HEAD`, runs the exact Rust 1.95 release build for `dmdoc`, copies only `dmdoc(.exe)` to `helpers/bin/<platform>/`, hashes it, and writes a sorted manifest containing platform, relative path, SHA-256, and source revision. It accepts no remote URL or arbitrary Cargo package name.

- [ ] **Step 4: Embed the packaging manifest without runtime downloads**

`build.rs` reads `MERIDIAN_MCP_HELPER_MANIFEST` when set, validates that it is a file, and emits a generated `helper_manifest.rs` into `OUT_DIR`. Without the variable it emits an empty registry so ordinary offline builds succeed and `dm_generate_docs` is not advertised. Add `/helpers/bin/` to `.gitignore`; keep the source manifest schema/example tracked.

- [ ] **Step 5: Verify the helper next to the installed MCP binary**

Resolve only the embedded relative path below `current_exe().parent()`, canonicalize it, and compare SHA-256 before every process launch. No environment variable or tool argument can replace the path or hash at runtime.

- [ ] **Step 6: Add cross-platform helper build to CI**

Check out the exact upstream revision into the job workspace, run the packaging script, set `MERIDIAN_MCP_HELPER_MANIFEST` for the Meridian-MCP release build/tests, and retain the helper only as a build artifact. Do not download during the MCP build script.

- [ ] **Step 7: Run helper and workflow-contract tests**

Run:

```powershell
cargo test --test docs_helper helper_manifest --all-features
cargo test --test workflow_contract --all-features
```

Expected: manifest validation passes; absent/mismatched helpers fail closed.

- [ ] **Step 8: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `build: package exact dmdoc helper`.

---

### Task 5: Add contained dmdoc generation

**Files:**
- Create: `src/tools/docs.rs`
- Modify: `src/spaceman/docs.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `src/limits.rs`
- Modify: `tests/docs_helper.rs`
- Modify: `tests/active_tool_policy.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: Verified fixed helper, active parsed project root, atomic directory replacement, bounded process runner.
- Produces:

```rust
pub struct GenerateDocsParams {
    pub output_directory: PathBuf,
    pub overwrite: bool,
}
pub struct DocsGenerationResult {
    pub output_directory: PathBuf,
    pub files: usize,
    pub bytes: u64,
    pub duration_ms: u64,
    pub helper_sha256: String,
    pub spacemandmm_revision: String,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
}
```

- [ ] **Step 1: Write failing tool and containment tests**

Use a test-only executable fixture whose command is injected through a `DocsProcess` trait available only to tests. Require fixed arguments, canonical project working directory, contained temporary output, explicit overwrite, bounded logs, and cleanup on nonzero exit.

- [ ] **Step 2: Run tests and confirm `dm_generate_docs` is absent**

Run: `cargo test --test docs_helper generate_docs --all-features`

Expected: missing tool and adapter.

- [ ] **Step 3: Implement fixed command construction**

Invoke only:

```text
<verified dmdoc helper> -e <active-environment-path> --output <temporary-contained-directory>
```

Lock that exact argument order in one command-construction test. Do not accept client arguments, `--index`, `--dry-run`, templates, repositories, URLs, or environment maps. The active contained `SpacemanDMM.toml` remains the authority for dmdoc index and module-directory configuration.

- [ ] **Step 4: Enforce directory output limits and replacement**

Add `max_docs_files = 100_000`, `max_docs_output_bytes = 1 GiB`, and `max_docs_duration_ms = 600_000`. After helper success, walk the temporary directory, reject symlinks/escapes and ceilings, then replace the final contained directory. Restore the previous directory if replacement fails.

- [ ] **Step 5: Advertise only when usable**

`dm_generate_docs` appears only in development mode when the embedded current-platform helper entry exists and the installed helper hash passes startup validation. A stale client call returns `tool_not_available` or `helper_checksum_mismatch` without launching a process.

- [ ] **Step 6: Run docs, policy, protocol, and full tests**

Run:

```powershell
cargo test --test docs_helper --all-features
cargo test --test active_tool_policy --all-features
cargo test --test mcp_conformance --all-features
cargo test --all-features
```

Expected: all pass; documentation output never escapes or partially replaces the final directory.

- [ ] **Step 7: Run the Stage 3 aggregate gate**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Expected: every command exits 0 on the current platform with the expected helper configuration.

- [ ] **Step 8: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: complete maps and dmdoc integration`.
