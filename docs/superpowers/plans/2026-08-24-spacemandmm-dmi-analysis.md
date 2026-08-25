# SpacemanDMM DMI Analysis Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add contained DMI profiling, state comparison, repository-scale duplicate discovery, static icon-reference auditing, and mechanical extraction without modifying source art.

**Architecture:** Decode DMI metadata and pixels through pinned `dmi`/`dmm-tools`, normalize frames into deterministic fingerprints, use hash buckets before bounded detailed comparisons, and correlate results with the immutable DreamMaker snapshot. A separately invalidated bounded cache prevents needless decoding while content hashes preserve correctness.

**Tech Stack:** Rust 1.95, `dmi`, `dmm-tools` PNG/GIF features, SHA-256, Tokio blocking workers, serde/schemars, generated technical test matrices.

**Spec:** `docs/superpowers/specs/2026-08-24-spacemandmm-complete-integration-design.md`

## Global Constraints

- Complete the foundation/language plan first.
- All DMI tools are read-only except explicit development-mode extraction outputs.
- Never mutate, rewrite, rename, delete, merge, recolor, redraw, or select a canonical source DMI/state.
- Do not add human-facing sprites as fixtures; generate technical matrices inside test code.
- Normalize hidden RGB only when alpha is zero.
- Preserve DMI direction semantics when applying transforms.
- Label unused-state evidence best-effort when any dynamic icon expression exists.
- Leave changes uncommitted absent explicit authorization.

---

### Task 1: Add DMI domain types, limits, and content-validated cache

**Files:**
- Create: `src/spaceman/dmi/mod.rs`
- Create: `src/spaceman/dmi/cache.rs`
- Create: `src/spaceman/dmi/test_support.rs`
- Create: `tests/dmi_analysis.rs`
- Modify: `src/spaceman/mod.rs`
- Modify: `src/limits.rs`
- Modify: `src/state.rs`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: `PathPolicy::read_path`, `ServerLimits`, pinned `dmm_tools::IconFile` and `dmi::{Metadata, State, StateIndex, Dir}`.
- Produces:

```rust
pub struct DmiAssetId {
    pub path: std::path::PathBuf,
    pub sha256: String,
    pub size: u64,
    pub modified: Option<std::time::SystemTime>,
}

pub struct DecodedDmi {
    pub identity: DmiAssetId,
    pub icon: std::sync::Arc<dmm_tools::IconFile>,
    pub asset_generation: u64,
}

pub struct DmiCache {
    entries: std::collections::HashMap<std::path::PathBuf, CacheEntry>,
    decoded_bytes: usize,
    next_generation: u64,
}

impl DmiCache {
    pub fn load(&mut self, path: &std::path::Path, limits: &ServerLimits) -> Result<DecodedDmi, DmiError>;
}
```

- [ ] **Step 1: Write the technical DMI generator and failing cache tests**

Under `#[cfg(test)]`, implement `write_test_dmi(path, width, height, states, pixels)` with `png::Encoder`, RGBA8 output, and a PNG `Description` text chunk containing valid DMI metadata. Use solid technical colors and coordinate patterns only.

Write tests that load a DMI twice, replace it with same-size bytes while restoring the original modification time when the platform permits, and require a changed SHA-256 and asset generation.

```rust
#[test]
fn cache_revalidates_content_identity() {
    let fixture = TestDmi::one_state([[255, 0, 0, 255]]);
    let mut cache = DmiCache::default();
    let first = cache.load(fixture.path(), &ServerLimits::default()).unwrap();
    fixture.replace_pixels([[0, 0, 255, 255]]);
    let second = cache.load(fixture.path(), &ServerLimits::default()).unwrap();
    assert_ne!(first.identity.sha256, second.identity.sha256);
    assert!(second.asset_generation > first.asset_generation);
}
```

- [ ] **Step 2: Run the focused test and confirm the DMI module is absent**

Run: `cargo test dmi::cache --all-features`

Expected: compile failure for missing `DmiCache` and test helper.

- [ ] **Step 3: Extend immutable server limits**

Add:

```rust
pub max_dmi_files: usize,          // 20_000
pub max_dmi_input_bytes: u64,      // 2 GiB aggregate per scan
pub max_dmi_file_bytes: u64,       // 64 MiB
pub max_dmi_decoded_pixels: u64,   // 64 million per file
pub max_dmi_states: usize,         // 100_000 aggregate
pub max_dmi_frames: usize,         // 1_000_000 aggregate
pub max_dmi_cache_entries: usize,  // 128
pub max_dmi_cache_bytes: usize,    // 512 MiB
pub max_dmi_matches: usize,        // 10_000
pub max_dmi_candidates: usize,     // 2_000_000
```

These are server ceilings. Tool parameters can request lower limits only.

- [ ] **Step 4: Implement load, validation, and LRU eviction**

Read bytes once, reject size/pixel/state/frame limits, compute SHA-256, and decode with `IconFile::from_bytes`. Key identity by canonical path plus the measured metadata and hash. Reuse only when the current content hash equals the cached hash. Evict least-recently-used entries until both cache ceilings hold; break ties by canonical path for deterministic tests.

- [ ] **Step 5: Attach the cache to server state**

Add `assets: tokio::sync::Mutex<DmiCache>` to `ServerState` with a method that locks only around lookup/insert. Do file reads and decoding in `spawn_blocking`; do not hold the analysis `RwLock`.

- [ ] **Step 6: Run cache and full regressions**

Run:

```powershell
cargo test dmi::cache --all-features
cargo test --all-features
```

Expected: cache tests and all existing tests pass.

- [ ] **Step 7: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: add content-validated DMI cache`.

---

### Task 2: Implement complete DMI profiling

**Files:**
- Modify: `src/spaceman/dmi/mod.rs`
- Create: `src/tools/dmi.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `tests/dmi_analysis.rs`
- Modify: `tests/active_tool_policy.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: `DecodedDmi`, `Metadata`, ordered `State` values, sheet rectangles, RGBA pixels.
- Produces:

```rust
pub struct DmiProfile {
    pub identity: DmiAssetId,
    pub sheet_width: u32,
    pub sheet_height: u32,
    pub cell_width: u32,
    pub cell_height: u32,
    pub total_frames: usize,
    pub states: Vec<DmiStateProfile>,
    pub warnings: Vec<DmiWarning>,
}

pub fn profile_dmi(asset: &DecodedDmi, limits: &ServerLimits) -> Result<DmiProfile, DmiError>;
```

- [ ] **Step 1: Write a failing profile test**

Generate a two-state DMI with duplicate state names, four directions, two delays, movement/loop/rewind metadata, transparent pixels, and a translucent pixel. Assert state order, duplicate indices, rectangles, delay list, alpha bounds, pixel counts, and per-frame hash.

- [ ] **Step 2: Run the test and confirm profiling is absent**

Run: `cargo test --test dmi_analysis dmi_profile --all-features`

Expected: compile failure or missing `dm_dmi_info` contract.

- [ ] **Step 3: Implement profile traversal**

Iterate `metadata.states` in sheet order. For every direction/frame index, obtain `rect_of_index`, copy the bounded cell pixels, normalize fully transparent RGB to zero for hashing, and calculate:

```rust
pub struct PixelCounts { pub opaque: u64, pub translucent: u64, pub transparent: u64 }
pub struct AlphaBounds { pub min_x: u32, pub min_y: u32, pub max_x: u32, pub max_y: u32 }
```

If no alpha is nonzero, return `alpha_bounds: None`. Add a warning `{ code: "hotspot_unsupported" }` when metadata contains a hotspot line or the parser cannot expose hotspot semantics.

- [ ] **Step 4: Add `dm_dmi_info` schema and containment**

Use:

```rust
pub struct DmiInfoParams { pub dmi_path: std::path::PathBuf }
```

Register it as analysis/read-only, canonicalize through `PathPolicy`, and include file hash, asset generation, upstream revision, and truncation fields in the result.

- [ ] **Step 5: Run profile, contract, and protocol tests**

Run:

```powershell
cargo test --test dmi_analysis dmi_profile --all-features
cargo test --test active_tool_policy --all-features
cargo test --test tool_contracts --all-features
```

Expected: all pass and `dm_dmi_info` is visible in both modes.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: profile DMI metadata and frames`.

---

### Task 3: Implement normalized comparison primitives

**Files:**
- Create: `src/spaceman/dmi/normalize.rs`
- Modify: `src/spaceman/dmi/mod.rs`
- Modify: `tests/dmi_analysis.rs`

**Interfaces:**
- Consumes: One decoded frame as width, height, and row-major RGBA8 pixels.
- Produces:

```rust
pub enum GeometricTransform { Identity, MirrorHorizontal, MirrorVertical, Rotate90, Rotate180, Rotate270 }
pub struct NormalizedFrame { pub width: u32, pub height: u32, pub pixels: Vec<[u8; 4]>, pub alpha_bounds: Option<AlphaBounds> }
pub struct FrameFingerprint { pub exact: [u8; 32], pub cropped: [u8; 32], pub palette: [u8; 32], pub perceptual: u64 }
pub struct FrameComparison { pub kind: MatchKind, pub transform: GeometricTransform, pub offset: (i32, i32), pub similarity: f32, pub changed_pixels: u64, pub max_channel_delta: u8 }
```

- [ ] **Step 1: Write the normalization test matrix**

Generate raw 3x3 and 4x2 matrices for:

- Same visible pixels with different RGB below alpha zero.
- Horizontal/vertical mirror.
- 90/180/270-degree rotation.
- One-pixel transparent padding/translation.
- Same alpha and canonical color topology with two different palettes.
- One changed visible pixel.

Require the exact approved classification and reject unrelated same-size matrices.

- [ ] **Step 2: Run the tests and confirm comparison APIs are absent**

Run: `cargo test dmi::normalize --all-features`

Expected: compile failure for the new types/functions.

- [ ] **Step 3: Implement normalized pixels and transforms**

Set RGB to zero only when alpha equals zero. Implement transforms with explicit coordinate functions. Return `dimension_mismatch` for 90/270 transforms whose rotated dimensions cannot map to the comparison canvas.

Direction mapping must use:

```rust
pub fn transform_direction(dir: dmi::Dir, transform: GeometricTransform) -> dmi::Dir;
```

Map all eight BYOND directions; test every transform and inverse.

- [ ] **Step 4: Implement exact, cropped, palette, and perceptual fingerprints**

Exact hashes include dimensions and normalized RGBA. Cropped hashes include cropped dimensions and alpha-bounded pixels. Palette topology assigns each new nontransparent RGBA value a monotonically increasing color index in row-major order while hashing the per-pixel alpha and index. The perceptual signature downsamples premultiplied luminance and alpha to an 8x8 bit signature used only for candidate bucketing.

- [ ] **Step 5: Implement final near-match scoring**

After the best legal alignment/transform, compute premultiplied RGBA absolute channel differences. Define:

```rust
similarity = 1.0 - total_absolute_delta as f32 / (pixel_count as f32 * 4.0 * 255.0)
```

Count a changed pixel when any normalized channel differs. The caller supplies a minimum similarity clamped to `[0.90, 1.0]`; the default is `0.985`.

- [ ] **Step 6: Run primitive and property tests**

Run: `cargo test dmi::normalize --all-features`

Expected: every transform round-trip, classification, and unrelated-image rejection passes.

- [ ] **Step 7: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: add deterministic DMI frame comparison`.

---

### Task 4: Add state comparison and repository duplicate clustering

**Files:**
- Create: `src/spaceman/dmi/duplicate.rs`
- Modify: `src/tools/dmi.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `src/tools/mod.rs`
- Modify: `tests/dmi_analysis.rs`
- Modify: `spacemandmm-capabilities.json`
- Modify: `Cargo.toml`

**Interfaces:**
- Consumes: Profiles, normalized fingerprints, direction transformation, cache, `PathPolicy`, scan limits.
- Produces:

```rust
pub struct StateLocator { pub dmi_path: std::path::PathBuf, pub state: String, pub duplicate_index: u32 }
pub struct StateComparison { pub left: StateLocator, pub right: StateLocator, pub image_match: MatchKind, pub metadata_differences: Vec<MetadataDifference>, pub frames: Vec<FrameComparison> }
pub struct DuplicateCluster { pub cluster_id: String, pub confidence: Confidence, pub members: Vec<StateLocator>, pub pair_evidence: Vec<StateComparison> }
pub fn compare_states(left: &DecodedDmi, left_state: &dmi::StateIndex, right: &DecodedDmi, right_state: &dmi::StateIndex, options: &ComparisonOptions) -> Result<StateComparison, DmiError>;
pub fn scan_duplicates(request: DuplicateScanRequest, cache: &DmiCache, limits: &ServerLimits) -> Result<DuplicateScanResult, DmiError>;
```

- [ ] **Step 1: Write failing whole-state and cross-DMI tests**

Generate multiple temporary DMIs covering renamed exact copy, metadata-only difference, horizontally mirrored four-direction state, padded copy, palette swap, one-pixel near copy, and an unrelated state. Require correct clusters and stable ordering across two runs.

- [ ] **Step 2: Run tests and confirm comparison/scan tools are absent**

Run: `cargo test --test dmi_analysis duplicate_scan --all-features`

Expected: failure for missing `dm_compare_dmi_states` and `dm_find_dmi_duplicates`.

- [ ] **Step 3: Implement bounded contained file discovery**

Add `globset = "0.4"`. Walk with `std::fs::read_dir`, never follow directory symlinks outside the canonical scope, and skip fixed components `.git`, `target`, and `.meridian-mcp-cache`. Match `**/*.dmi` by default. Stop at file/input-byte ceilings and set explicit truncation reasons.

- [ ] **Step 4: Implement candidate buckets and whole-state verification**

Bucket by exact, transformed, cropped, palette, then perceptual signatures. Increment `candidate_comparisons` before detailed work and stop at the server ceiling. Whole-state matching requires identical direction/frame cardinality after transform and consistent direction remapping for every frame. Record delays, movement, loop, rewind, dirs, and frame-count differences separately.

- [ ] **Step 5: Implement stable clusters**

Sort locators by canonical path, state name, and duplicate index. Union verified pairs. Derive `cluster_id` as SHA-256 over the sorted member identities and match class. Sort clusters by highest confidence, then first member. Never label a member canonical.

- [ ] **Step 6: Add typed tools**

Use:

```rust
pub struct CompareDmiStatesParams {
    pub left_dmi_path: PathBuf,
    pub left_state: String,
    pub left_duplicate_index: Option<u32>,
    pub right_dmi_path: PathBuf,
    pub right_state: String,
    pub right_duplicate_index: Option<u32>,
    pub minimum_similarity: Option<f32>,
}

pub struct FindDmiDuplicatesParams {
    pub scope_path: Option<PathBuf>,
    pub include_glob: Option<String>,
    pub minimum_similarity: Option<f32>,
    pub include_frame_matches: Option<bool>,
    pub max_matches: Option<usize>,
}
```

Both are analysis/read-only. A missing `scope_path` uses the active parsed root; before parsing it returns `parse_required` rather than scanning every configured root.

- [ ] **Step 7: Run cluster, policy, and full tests**

Run:

```powershell
cargo test --test dmi_analysis duplicate_scan --all-features
cargo test --test config_and_paths --all-features
cargo test --test active_tool_policy --all-features
cargo test --all-features
```

Expected: exact and lazy-change classes pass; unrelated states remain separate; all scans report limits/statistics.

- [ ] **Step 8: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: find cross-DMI duplicate states`.

---

### Task 5: Correlate static icon references and add `dm_audit_icons`

**Files:**
- Create: `src/spaceman/dmi/source_refs.rs`
- Modify: `src/index/mod.rs`
- Modify: `src/analysis_snapshot.rs`
- Modify: `src/spaceman/dmi/duplicate.rs`
- Modify: `src/tools/dmi.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `tests/dmi_analysis.rs`
- Modify: `tests/fixtures/language/fixture.dm`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: Object-tree values/expressions, source locations, DMI profiles, duplicate clusters.
- Produces:

```rust
pub enum IconReferenceResolution {
    Static { dmi_path: PathBuf, state: Option<String> },
    Dynamic { reason: String },
}
pub struct IconReference { pub type_path: String, pub file: String, pub line: u32, pub resolution: IconReferenceResolution }
pub struct IconAuditResult { pub missing_files: Vec<MissingIconFile>, pub missing_states: Vec<MissingIconState>, pub duplicates: Vec<DuplicateCluster>, pub unused_states: Vec<BestEffortUnusedState>, pub dynamic_references: Vec<IconReference>, pub complete: bool }
```

- [ ] **Step 1: Add static/dynamic fixture cases and failing audit test**

Add fixture types with a literal resource icon and state, a missing state, and a computed `icon_state`. Generate the referenced DMI during the test. Require the literal reference to resolve, missing state to report, and dynamic expression to set `complete: false`.

- [ ] **Step 2: Run the audit test and confirm the tool is missing**

Run: `cargo test --test dmi_analysis icon_audit --all-features`

Expected: missing `dm_audit_icons`.

- [ ] **Step 3: Build static reference records during snapshot construction**

For type `icon` and `icon_state` values, accept only upstream constants that resolve to a contained resource path and literal string. Pair inherited defaults using object-tree resolution. Record any expression/nonconstant assignment as dynamic with its source location; never guess constructed strings.

- [ ] **Step 4: Implement audit composition**

Resolve static files relative to the parsed root, profile existing files, test named states including duplicate index 0, run the bounded duplicate scan, then identify profile states with no static reference. Mark every unused result `{ "best_effort": true }`. Set top-level `complete` false for dynamic references or truncation.

- [ ] **Step 5: Add and register `dm_audit_icons`**

Use a scope/glob/near-threshold schema consistent with duplicate scanning plus `include_unused: bool`. Return source locations with every reference-related finding.

- [ ] **Step 6: Run audit and full regressions**

Run:

```powershell
cargo test --test dmi_analysis icon_audit --all-features
cargo test --test language_capabilities --all-features
cargo test --all-features
```

Expected: all pass; dynamic input never yields definitive unused claims.

- [ ] **Step 7: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: audit DreamMaker icon references`.

---

### Task 6: Add contained mechanical extraction

**Files:**
- Modify: `src/atomic_output.rs`
- Create: `src/spaceman/dmi/extract.rs`
- Modify: `src/spaceman/dmi/mod.rs`
- Modify: `src/tools/dmi.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `src/tools/mod.rs`
- Modify: `tests/dmi_analysis.rs`
- Modify: `tests/config_and_paths.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: `IconRenderer`, selected `StateIndex`, `PathPolicy::output_path`, source hash.
- Produces:

```rust
pub enum DmiExtractionKind { Auto, Png, Gif, ContactSheet, Frame }
pub fn extract_dmi(asset: &DecodedDmi, request: &ExtractionRequest, output: &mut std::fs::File) -> Result<ExtractionReport, DmiError>;
```

- [ ] **Step 1: Write failing output-safety tests**

Require rejection outside roots, rejection of existing output without overwrite, successful atomic replacement with overwrite, cleanup after encoder failure, and unchanged source DMI SHA-256.

- [ ] **Step 2: Run the tests and confirm extraction is absent**

Run: `cargo test --test dmi_analysis extraction --all-features`

Expected: missing `dm_extract_dmi` and the DMI extraction adapter.

- [ ] **Step 3: Reuse the contained atomic output helper**

Call the foundation stage's `write_atomic` only after resolving the output with `PathPolicy`. Keep encoder logic inside the closure so any encoder failure exercises the shared cleanup and restoration behavior. Do not add a DMI-specific rename path.

- [ ] **Step 4: Implement extraction modes**

Use `IconRenderer::prepare_render_state` for auto PNG/GIF behavior and `render_to_images` for contact sheets. Frame mode writes one exact direction/frame cell. Validate state plus duplicate index and all frame/direction bounds. Output extension must match the selected encoder; reject mismatches rather than silently changing the path.

- [ ] **Step 5: Add development-only `dm_extract_dmi`**

Use:

```rust
pub struct ExtractDmiParams {
    pub dmi_path: PathBuf,
    pub state: String,
    pub duplicate_index: Option<u32>,
    pub kind: DmiExtractionKind,
    pub direction: Option<String>,
    pub frame: Option<u32>,
    pub output_path: PathBuf,
    pub overwrite: bool,
}
```

The result includes source and output hashes, encoder, dimensions, bytes, asset generation, and upstream revision.

- [ ] **Step 6: Run extraction, policy, and stdio tests**

Run:

```powershell
cargo test --test dmi_analysis extraction --all-features
cargo test --test config_and_paths --all-features
cargo test --test active_tool_policy --all-features
cargo build --release
.\test_mcp.ps1 -SkipBuild -BinaryPath .\target\release\meridian-mcp.exe -Mode development
```

Expected: extraction appears only in development mode; source hashes are unchanged; all temporary outputs are cleaned.

- [ ] **Step 7: Run the Stage 2 aggregate gate**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
```

Expected: every command exits 0.

- [ ] **Step 8: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: complete DMI analysis and extraction`.
