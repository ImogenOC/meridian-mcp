# `dm_parse_environment` Correctness and Performance Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task. The user authorized sequential implementation and review subagents for this plan.

**Goal:** Make parse reuse complete and honest, remove avoidable parse/index and query work, expose actionable timing evidence, and establish a versioned semantic-corpus contract without adding an unapproved embedding model or vector service.

**Architecture:** Preserve atomic `AnalysisSnapshot` replacement. Split search document extraction from postings construction so proc identities are canonicalized before the one BM25 build; enumerate SpacemanDMM's complete registered file list; restrict queries to indexed candidates; and define deterministic semantic chunk records for a future optional dense backend.

**Tech Stack:** Rust 1.95.0, Tokio, Rayon, pinned SpacemanDMM/DreamChecker, SHA-256, serde/serde_json, PowerShell.

**Spec:** `docs/audits/2026-08-30-dm-parse-environment-audit.md`

## Global Constraints

- Preserve the prior snapshot after every failed, timed-out, or panicked reparse.
- Preserve ranking order: descending score, then symbol, file, line, and column.
- Preserve exact/name boosts, partial-symbol matching, filters, and limits.
- Keep analysis local and read-only. Do not add model downloads, embedding calls, or persistence.
- Use SpacemanDMM's file registry; do not write another include parser.
- Call path/length/mtime reuse a metadata fingerprint, not byte identity.
- Use Rust 1.95.0 and PowerShell for Windows gates.
- Create scoped commits after each task passes review. Preserve unrelated changes, and do not push or merge without separate authorization.

## File Map

- `src/analysis_snapshot.rs`: complete file capture and build timing.
- `src/source_fingerprint.rs`: metadata-fingerprint contract.
- `src/source.rs`: reusable line-indexed text.
- `src/search.rs`: document corpus, one BM25 build, candidate indexes, ranking.
- `src/semantic.rs`: schema-v1 chunk/vector identities.
- `src/tools/parse.rs`: validation, failures, timings, response metadata.
- `src/tools/search.rs`: retrieval mode and execution evidence.
- `src/tools/mod.rs` and `src/contracts.rs`: public contract.
- `tests/fixtures/search/*` and `tests/search_relevance.rs`: golden corpus.
- `tests/parse_reuse_scale.rs`: real-corpus evidence.
- `README.md`, `TESTING.md`, `CHANGELOG.md`, `docs/tool-contracts.md`: documentation.
- `docs/audits/2026-08-30-dm-parse-environment-audit.md`: post-update evidence.

---

### Task 1: Track Every SpacemanDMM-Loaded Input

**Files:**
- Modify: `src/tools/parse.rs` tests
- Modify: `src/analysis_snapshot.rs:38-104,289-311`
- Modify: `src/source_fingerprint.rs`

**Produces:** complete `AnalysisContext.file_paths` and `AnalysisSnapshot::source_inputs()`.

- [ ] **Step 1: Add the failing comment-only include test**

```rust
#[tokio::test]
async fn editing_a_comment_only_include_forces_a_reparse() {
    let (directory, dme_path) = write_environment_fixture();
    let comment_only = directory.join("comment_only.dm");
    std::fs::write(
        &dme_path,
        "#include \"fixture.dm\"\n#include \"comment_only.dm\"\n",
    )
    .unwrap();
    std::fs::write(&comment_only, "// first revision\n").unwrap();
    settle(&directory);
    let state = ServerState::new();

    parse_environment(&state, json!({"dme_path": dme_path.clone()}))
        .await
        .unwrap();
    let generation = state.state_generation().await;
    assert!(state.snapshot().await.unwrap().source_inputs()
        .contains(&comment_only.canonicalize().unwrap()));

    std::fs::write(&comment_only, "// second revision\n").unwrap();
    settle(&directory);
    let reparsed = parse_environment(&state, json!({"dme_path": dme_path}))
        .await
        .unwrap();
    let body = result_json(&reparsed);

    assert_eq!(body["reused"], false);
    assert_eq!(body["state_generation"], generation + 1);
    std::fs::remove_dir_all(directory).unwrap();
}
```

This catches a return to declaration-derived input discovery.

- [ ] **Step 2: Verify RED**

```powershell
cargo +1.95.0 test --locked --lib tools::parse::tests::editing_a_comment_only_include_forces_a_reparse -- --nocapture
```

Expected: FAIL because `comment_only.dm` is absent.

- [ ] **Step 3: Enumerate the parser registry**

Replace declaration/macro/diagnostic path capture in `AnalysisContext::extract`:

```rust
let project_root = environment_path.parent().unwrap_or_else(|| Path::new("."));
let mut file_paths = HashMap::new();
context.file_list().for_each(|reported| {
    let Some(file_id) = context.get_file(reported) else {
        return;
    };
    let resolved = if reported.is_absolute() {
        reported.to_path_buf()
    } else {
        project_root.join(reported)
    };
    file_paths.insert(file_id, resolved);
});
```

Retain DME/config additions, canonicalization, containment, sorting, and deduplication. Do not crawl the repository.

- [ ] **Step 4: Correct fingerprint docs**

State that `SourceFingerprint` compares path, length, and settled mtime. Do not add hashing.

- [ ] **Step 5: Verify GREEN**

```powershell
cargo +1.95.0 test --locked --lib tools::parse::tests -- --nocapture
cargo +1.95.0 test --locked --test analysis_snapshot -- --nocapture
git diff --check
```

### Task 2: Classify Failures and Bound the Contract

**Files:**
- Modify: `src/tools/parse.rs`
- Modify: `src/tools/mod.rs:173-195`
- Modify: `src/contracts.rs:133-141`
- Modify: `tests/tool_contracts.rs`
- Regenerate: `docs/tool-contracts.md`

**Produces:** code-aware `parse_failure` and timeout range `1..=1_800_000` ms.

- [ ] **Step 1: Add failing tests**

```rust
#[tokio::test]
async fn parse_failure_preserves_the_requested_error_code() {
    let result = parse_failure(
        &ServerState::new(),
        None,
        ToolErrorCode::TimedOut,
        "parse exceeded 1 ms".to_owned(),
        Some("Wait for the active parser worker to finish, then retry.".to_owned()),
    )
    .await
    .unwrap();
    assert_eq!(result_json(&result)["code"], "timed_out");
}

#[test]
fn parse_timeout_rejects_values_outside_the_contract() {
    assert!(validated_parse_timeout(&json!({"timeout_ms": 0})).is_err());
    assert!(validated_parse_timeout(&json!({"timeout_ms": 1_800_001})).is_err());
    assert_eq!(
        validated_parse_timeout(&json!({"timeout_ms": 1_800_000})).unwrap(),
        Duration::from_millis(1_800_000),
    );
}
```

In `tests/tool_contracts.rs`:

```rust
let parse = all_contracts().iter()
    .find(|contract| contract.name == "dm_parse_environment")
    .unwrap();
assert_eq!(parse.timeout_ms, Some(1_800_000));
```

- [ ] **Step 2: Verify RED**

```powershell
cargo +1.95.0 test --locked --lib parse_failure_preserves_the_requested_error_code
cargo +1.95.0 test --locked --lib parse_timeout_rejects_values_outside_the_contract
cargo +1.95.0 test --locked --test tool_contracts
```

- [ ] **Step 3: Add one validator**

```rust
const MAX_PARSE_TIMEOUT_MS: u64 = 1_800_000;

fn validated_parse_timeout(args: &Value) -> Result<Duration> {
    let timeout_ms = args.get("timeout_ms")
        .map(|value| value.as_u64()
            .ok_or_else(|| anyhow!("timeout_ms must be an integer")))
        .transpose()?
        .unwrap_or(DEFAULT_PARSE_TIMEOUT_MS);
    if !(1..=MAX_PARSE_TIMEOUT_MS).contains(&timeout_ms) {
        return Err(anyhow!(
            "timeout_ms must be between 1 and {MAX_PARSE_TIMEOUT_MS}"
        ));
    }
    Ok(Duration::from_millis(timeout_ms))
}
```

Return structured `invalid_input` before starting a worker.

- [ ] **Step 4: Pass code and recovery explicitly**

```rust
async fn parse_failure(
    state: &ServerState,
    prior_environment: Option<&Path>,
    code: ToolErrorCode,
    message: String,
    recovery: Option<String>,
) -> Result<ToolResult>
```

Map bad path/timeout/parser input to `InvalidInput`, elapsed timeout to `TimedOut`, and worker `JoinError` to `Internal`. Retain `state_preserved`, active environment, and generation.

- [ ] **Step 5: Close and bound the schema**

Add `maximum: 1800000`, `additionalProperties: false`, and `Some(1_800_000)` to the contract registry.

- [ ] **Step 6: Verify GREEN**

```powershell
cargo +1.95.0 test --locked --lib tools::parse::tests -- --nocapture
cargo +1.95.0 run --locked --bin render_tool_docs -- docs/tool-contracts.md
cargo +1.95.0 test --locked --test tool_contracts -- --nocapture
```

### Task 3: Build Postings Once, Cache Lines Once, Expose Timings

**Files:**
- Modify: `src/source.rs`
- Modify: `src/search.rs`
- Modify: `src/analysis_snapshot.rs`
- Modify: `src/tools/parse.rs`

**Produces:** `IndexedSource`, `SearchDocuments`, `AnalysisBuildTimings`, and `timings_ms`.

- [ ] **Step 1: Add failing tests**

```rust
#[test]
fn indexed_source_serves_multiple_declarations_from_one_line_table() {
    let indexed = IndexedSource::new(
        "/proc/one()\n\treturn 1\n/proc/two()\n\treturn 2\n".to_owned(),
    );
    assert_eq!(indexed.line(1), Some("/proc/one()"));
    assert_eq!(indexed.declaration(1, 80).as_deref(),
        Some("/proc/one()\n\treturn 1"));
    assert_eq!(indexed.declaration(3, 80).as_deref(),
        Some("/proc/two()\n\treturn 2"));
    assert_eq!(indexed.line_count(), 4);
}
```

In the successful parse test require `queue_wait`, `preprocess_parse`, `dreamchecker`, `search_documents`, `analysis_indexes`, `fingerprint`, and `total` under `timings_ms`.

- [ ] **Step 2: Verify RED**

```powershell
cargo +1.95.0 test --locked --lib indexed_source_serves_multiple_declarations_from_one_line_table
cargo +1.95.0 test --locked --lib a_successful_parse_reports_diagnostic_counts_and_duration
```

- [ ] **Step 3: Implement line-indexed text**

```rust
pub(crate) struct IndexedSource {
    text: String,
    line_starts: Vec<usize>,
}

impl IndexedSource {
    pub(crate) fn new(text: String) -> Self;
    pub(crate) fn read(path: &Path) -> std::io::Result<Self>;
    pub(crate) fn line(&self, one_based_line: u32) -> Option<&str>;
    pub(crate) fn declaration(
        &self,
        one_based_line: u32,
        max_lines: usize,
    ) -> Option<String>;
    pub(crate) fn line_count(&self) -> usize;
}
```

Compute offsets once, preserve LF/CRLF behavior, and make extraction helpers delegate to it. Cache `Option<IndexedSource>` by path without a second full copy.

- [ ] **Step 4: Separate documents from postings**

```rust
pub(crate) struct SearchDocuments {
    documents: Vec<SearchDocument>,
}

impl SearchDocuments {
    pub(crate) fn from_object_tree(
        objtree: &ObjectTree,
        context: &Context,
        environment_path: &Path,
    ) -> Self;
    pub(crate) fn canonicalize_procs(mut self, resolver: &ProcResolver) -> Self;
    pub(crate) fn into_index(self) -> SearchIndex {
        SearchIndex::new(self.documents)
    }
}
```

Move the object-tree walk into `SearchDocuments` and proc canonicalization from `SearchIndex::with_proc_resolver` into `canonicalize_procs`. Delete both early-build APIs.

- [ ] **Step 5: Time immutable build stages**

```rust
#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct AnalysisBuildTimings {
    pub analysis_indexes: u64,
    pub fingerprint: u64,
}
```

Make `AnalysisBuild::from_parse` consume `SearchDocuments` and return `(AnalysisBuild, AnalysisBuildTimings)`. Preserve Rayon ordering and call `into_index` once after proc resolution.

- [ ] **Step 6: Time the outer request**

Record receipt before `parse_permit`. Cold `timings_ms` contains all named stages; reused timing contains `queue_wait`, `reuse_validation`, and `total` only. Keep the detached timeout waiter holding the permit.

- [ ] **Step 7: Verify GREEN**

```powershell
cargo +1.95.0 test --locked --lib source::tests -- --nocapture
cargo +1.95.0 test --locked --lib search::tests -- --nocapture
cargo +1.95.0 test --locked --lib tools::parse::tests -- --nocapture
cargo +1.95.0 test --locked --test analysis_snapshot -- --nocapture
```

### Task 4: Search Only Indexed Candidates

**Files:**
- Modify: `src/search.rs`
- Modify: `src/tools/search.rs`
- Modify: `src/process_metrics.rs`
- Modify: `tests/process_metrics.rs`
- Modify: `tests/parse_reuse_scale.rs`

**Produces:** `SearchExecution` with hits and candidate counters.

- [ ] **Step 1: Add failing candidate tests**

```rust
#[test]
fn partial_symbol_queries_use_substring_candidates() {
    let index = SearchIndex::new(vec![
        document(SymbolKind::Proc, "/datum/dogmos_kennel/proc/status",
            "status", "/datum/dogmos_kennel", "code/dogmos.dm", "", "return"),
        document(SymbolKind::Proc, "/datum/unrelated/proc/status",
            "status", "/datum/unrelated", "code/other.dm", "", "return"),
    ]);
    let execution = index.search(&request("dogmos_kenn"));
    assert_eq!(execution.hits[0].document.symbol,
        "/datum/dogmos_kennel/proc/status");
}

#[test]
fn absent_terms_do_not_score_the_whole_corpus() {
    let index = SearchIndex::new((0..10_000)
        .map(|number| document(
            SymbolKind::Type,
            &format!("/datum/noise_{number}"),
            &format!("noise_{number}"),
            "/datum",
            "code/noise.dm",
            "unrelated fixture",
            "return",
        ))
        .collect());
    let execution = index.search(&request("term_that_is_not_present"));
    assert!(execution.hits.is_empty());
    assert_eq!(execution.documents_scored, 0);
}
```

Retain exact, filter, and tie-order tests; add a filtered partial-symbol case.

- [ ] **Step 2: Verify RED**

```powershell
cargo +1.95.0 test --locked --lib search::tests -- --nocapture
```

- [ ] **Step 3: Build candidate indexes once**

Extend `SearchIndex`:

```rust
exact_symbols: HashMap<String, Vec<usize>>,
exact_names: HashMap<String, Vec<usize>>,
```

Use lowercase exact keys. Normal term queries take candidates from BM25 postings and exact maps. When neither produces a candidate, run the existing full symbol-substring scan as the compatibility fallback for partial identifiers. This keeps partial-symbol behavior without adding a high-cardinality trigram structure whose memory could rival the lexical postings.

- [ ] **Step 4: Rank candidate IDs only**

```rust
pub(crate) struct SearchExecution<'a> {
    pub(crate) hits: Vec<SearchHit<'a>>,
    pub(crate) candidates_considered: usize,
    pub(crate) documents_scored: usize,
}
```

Union BM25 posting IDs and exact IDs. Apply filters/boosts to that union, or to the verified substring fallback when the union is empty. Use one deterministic comparator. When over limit, use `select_nth_unstable_by`, truncate, then sort retained top-k.

- [ ] **Step 5: Expose retrieval evidence**

Keep existing fields and add:

```json
"retrieval": {
  "mode": "lexical",
  "algorithm": "bm25",
  "candidates_considered": 123,
  "documents_scored": 120
}
```

- [ ] **Step 6: Extend the ignored scale gate**

Run the audit's ten queries after reuse in the same state. Print each latency, median, maximum, candidates, and top symbols. Assert exact `dogmos` and mapping queries rank first; keep timing thresholds manual to avoid machine-speed CI flakiness.

Add `ProcessRole::MeridianMcp`, cover its snake-case serialization in `tests/process_metrics.rs`, and sample the release test process immediately before parsing and after snapshot installation:

```rust
let identity = process_identity(std::process::id(), ProcessRole::MeridianMcp).unwrap();
let before = sample_process(&identity, 0).unwrap();
// Run the cold parse.
let after = sample_process(&identity, cold_elapsed.as_millis() as u64).unwrap();
```

Report working-set/private bytes on Windows and RSS/virtual bytes on Linux. Do not add a memory threshold until baseline and updated values exist on the same host.

- [ ] **Step 7: Verify GREEN**

```powershell
cargo +1.95.0 test --locked --lib search::tests -- --nocapture
cargo +1.95.0 test --locked --test proc_resolution -- --nocapture
cargo +1.95.0 test --locked --test fixture_corpus -- --nocapture
```

### Task 5: Add Golden Retrieval Judgments and Semantic Chunk Schema

**Files:**
- Create: `src/semantic.rs`
- Modify: `src/lib.rs` and `src/search.rs`
- Create: `tests/fixtures/search/fixture.dme`
- Create: `tests/fixtures/search/fixture.dm`
- Create: `tests/fixtures/search/relevance.json`
- Create: `tests/search_relevance.rs`
- Modify: `src/tools/parse.rs`

**Produces:** schema-v1 stable chunks and fixed recall/MRR gates; no embeddings.

- [ ] **Step 1: Create the owned fixture**

`fixture.dme`:

```dm
#include "fixture.dm"
```

`fixture.dm`:

```dm
/** Coordinate native canine simulation health and ABI compatibility. */
/datum/controller/subsystem/dogmos_fixture

/** Report whether the native canine library is healthy. */
/datum/controller/subsystem/dogmos_fixture/proc/library_status()
	return TRUE

/** Move an atom through the subsystem-managed path queue. */
/datum/move_manager_fixture/proc/queue_path(atom/movable/target)
	return target

/** Store a personal item in a bluespace cache. */
/datum/personal_cache_fixture/proc/store_item(obj/item/stored_item)
	return stored_item

/** Unrelated dog health distractor. */
/mob/living/basic/dog_fixture
	var/health = 100
```

Create schema-1 JSON judgments for the exact proc and these natural queries: `native canine library health compatibility`, `move atom through managed path queue`, and `personal bluespace item storage cache`. Store literal relevant symbols and optional `required_first`.

- [ ] **Step 2: Add the failing evaluator**

Parse via `call_tool`, run each query with limit 10, calculate from JSON labels, and assert:

```rust
assert_eq!(exact_identifier_mrr, 1.0);
assert_eq!(natural_language_recall_at_10, 1.0);
assert_eq!(body["retrieval"]["mode"], "lexical");
```

- [ ] **Step 3: Verify RED**

```powershell
cargo +1.95.0 test --locked --test search_relevance -- --nocapture
```

Improve fixture documentation/index behavior if a judgment fails; do not weaken labels.

- [ ] **Step 4: Define semantic records**

```rust
pub const SEMANTIC_CHUNK_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct SemanticChunkRecord {
    pub schema_version: u32,
    pub chunk_id: String,
    pub document_id: String,
    pub content_digest: String,
    pub chunk_index: u32,
    pub kind: String,
    pub symbol: String,
    pub implementation_owner: Option<String>,
    pub declaration_owner: Option<String>,
    pub repository_relative_file: String,
    pub line: u32,
    pub column: u32,
    pub text: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize)]
pub struct VectorIndexIdentity {
    pub chunk_schema_version: u32,
    pub embedding_provider: String,
    pub embedding_model: String,
    pub dimensions: usize,
    pub distance: String,
}
```

Use lowercase SHA-256. `document_id` covers schema, symbol/implementation identities, relative file, location, and override index; `chunk_id` adds chunk index; `content_digest` covers exact text. Exclude absolute paths, generations, models, and timestamps. Produce 40-line chunks with five-line overlap. Do not retain duplicate chunks in the snapshot.

- [ ] **Step 5: Test identity semantics**

Prove rebuild stability, unchanged document ID but changed digest after a body edit, repository-relative paths, and model identity independence from chunks.

- [ ] **Step 6: Report honest readiness**

Add to cold and reused parse results:

```json
"retrieval": {
  "lexical": {"status": "ready", "algorithm": "bm25", "documents": 450258},
  "dense": {"status": "not_configured"},
  "semantic_chunk_schema_version": 1
}
```

- [ ] **Step 7: Verify GREEN**

```powershell
cargo +1.95.0 test --locked --lib semantic::tests -- --nocapture
cargo +1.95.0 test --locked --test search_relevance -- --nocapture
cargo +1.95.0 test --locked --test fixture_corpus -- --nocapture
```

### Task 6: Update Contracts and Maintenance Guidance

**Files:**
- Modify: `README.md`, `TESTING.md`, `CHANGELOG.md`
- Modify: `src/tools/mod.rs` descriptions
- Regenerate: `docs/tool-contracts.md`
- Modify: `docs/audits/2026-08-30-dm-parse-environment-audit.md`

- [ ] **Step 1: Correct public wording**

Document registered-file metadata reuse, fail-closed inputs, cold/reused timing maps, lexical BM25, exact-tool routing, `dense.status = not_configured`, and that chunk schema readiness is not dense retrieval.

- [ ] **Step 2: Add maintenance/testing guidance**

Document the relevance gate and scale evidence. State that ANN recall, model migration, filter indexes, and atomic vector-generation swaps become required only when dense retrieval is implemented.

- [ ] **Step 3: Update changelog and audit**

Record changes under `Unreleased`. Append `Implementation evidence` after Task 7 while preserving the original baseline.

- [ ] **Step 4: Regenerate and verify**

```powershell
cargo +1.95.0 run --locked --bin render_tool_docs -- docs/tool-contracts.md
cargo +1.95.0 test --locked --test documentation -- --nocapture
cargo +1.95.0 test --locked --test tool_contracts -- --nocapture
git diff --check
```

### Task 7: Run the Pinned Acceptance Matrix

- [ ] **Step 1: Confirm compiler identity**

```powershell
rustc +1.95.0 --version --verbose
```

Require release 1.95.0.

- [ ] **Step 2: Run focused gates**

```powershell
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 test --locked --lib source::tests -- --nocapture
cargo +1.95.0 test --locked --lib search::tests -- --nocapture
cargo +1.95.0 test --locked --lib tools::parse::tests -- --nocapture
cargo +1.95.0 test --locked --test analysis_snapshot -- --nocapture
cargo +1.95.0 test --locked --test search_relevance -- --nocapture
cargo +1.95.0 test --locked --test proc_resolution -- --nocapture
cargo +1.95.0 test --locked --test tool_contracts -- --nocapture
cargo +1.95.0 test --locked --test documentation -- --nocapture
```

- [ ] **Step 3: Run full gates**

```powershell
cargo +1.95.0 test --locked --all-features
cargo +1.95.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.95.0 build --locked --release
cargo +1.95.0 deny check
```

Record existing duplicate-dependency warnings separately from failures.

- [ ] **Step 4: Run installed release smoke**

```powershell
./test_mcp.ps1 -SkipBuild -ServerPath ./target/release/meridian-mcp.exe -Mode analysis -DmePath ./tests/fixtures/language/fixture.dme -SearchQuery "return supplied value" -TimeoutSeconds 300
```

- [ ] **Step 5: Run real-corpus evidence**

```powershell
$env:MERIDIAN_SCALE_DME = 'C:\path\to\Meridian-Rift\tgstation.dme'
cargo +1.95.0 test --locked --release --test parse_reuse_scale -- --ignored --nocapture
```

Use the current contained checkout but never record its absolute path. Acceptance on the audit host:
- cold no worse than 34,386 ms; target 15% faster;
- reuse below 750 ms;
- median query below 50 ms and maximum below 100 ms;
- post-install working set no more than 1,824 MiB and private memory no more than 1,991 MiB (5% over the measured baseline);
- exact `dogmos` and mapping results first;
- exact MRR 1.0 and natural-language recall@10 unchanged.

If a target fails, return to its responsible task with stage evidence. Do not weaken thresholds.

- [ ] **Step 6: Append evidence and recheck**

```powershell
cargo +1.95.0 test --locked --test documentation -- --nocapture
git diff --check
```

- [ ] **Step 7: Inspect final scope**

```powershell
git status --short
git diff --stat
git diff -- src tests README.md TESTING.md CHANGELOG.md docs
```

Check every audit target against fresh output. Commit reviewed task changes locally, do not push or merge, and report unavailable CI/platform gates.
