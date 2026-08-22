# DreamMaker Context Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add deterministic repository-scale DreamMaker context retrieval to `meridian-mcp` and validate it against Meridian-Rift.

**Architecture:** Build a weighted in-memory BM25 index from SpacemanDMM's parsed object tree. Cache it in `ServerState`, expose it as `dm_search_context`, and retain exact-tool workflows for follow-up verification.

**Tech Stack:** Rust, SpacemanDMM `dreammaker`, serde/serde_json, tokio, PowerShell smoke tests.

**Spec:** `docs/superpowers/specs/2026-08-22-dreammaker-context-search-design.md`

**Global Constraints:** Work in the existing clean checkout, keep changes uncommitted, use `apply_patch` for file edits, and preserve all existing behavior.

---

### Task 1: Share source extraction

**Files:**
- Create: `src/source.rs`
- Modify: `src/main.rs`
- Modify: `src/tools/parse.rs`

1. Move the tested source-declaration extraction helper and its four tests from `tools/parse.rs` into `source.rs`.
2. Keep `dm_get_proc` behavior unchanged by calling the shared helper.
3. Run the focused source tests, then the whole suite.

### Task 2: Implement the BM25 retrieval core

**Files:**
- Create: `src/search.rs`
- Modify: `src/main.rs`

1. Write failing tests for behavioral relevance, exact-symbol boosts, filters/limits, and deterministic tie order.
2. Define `SymbolKind`, `SearchDocument`, `SearchRequest`, `SearchHit`, and `SearchIndex`.
3. Implement weighted token collection, inverted postings, BM25 scoring, exact-match boosts, filters, and stable ordering.
4. Run `cargo test search::tests` and confirm all new tests pass.

Representative API:

```rust
pub(crate) struct SearchRequest<'a> {
	pub(crate) query: &'a str,
	pub(crate) kind: Option<SymbolKind>,
	pub(crate) type_prefix: Option<&'a str>,
	pub(crate) file_filter: Option<&'a str>,
	pub(crate) limit: usize,
}

impl SearchIndex {
	pub(crate) fn search(&self, request: &SearchRequest<'_>) -> Vec<SearchHit<'_>>;
}
```

### Task 3: Build the parser-backed symbol index

**Files:**
- Modify: `src/search.rs`
- Modify: `src/source.rs`

1. Write a failing fixture test that parses a temporary `.dme` and searches a documented proc.
2. Add a source-file cache rooted at the parsed environment directory.
3. Walk object-tree types, variables, procs, and overrides while excluding builtins.
4. Populate exact locations, parent types, parameters, documentation, source excerpts, and override metadata.
5. Make unreadable source non-fatal and rerun focused tests.

### Task 4: Integrate state and parsing

**Files:**
- Modify: `src/state.rs`
- Modify: `src/tools/parse.rs`

1. Add a failing state test proving cached search is cleared with parser state.
2. Add `search_index: Option<Arc<SearchIndex>>` to `ServerState`.
3. Build the index after successful parsing, then atomically publish the environment path, object tree, context, and index.
4. Include the indexed-document count in `dm_parse_environment` output.

### Task 5: Expose `dm_search_context`

**Files:**
- Create: `src/tools/search.rs`
- Modify: `src/tools/mod.rs`

1. Add failing tests for tool registration/schema and the pre-parse error.
2. Define the MCP input schema and argument validation.
3. Serialize ranked results, optionally omitting or line-truncating source excerpts.
4. Register and route `dm_search_context`.

### Task 6: Add client workflow instructions

**Files:**
- Modify: `src/mcp.rs`

1. Extend the initialization test to require concise parse/search/exact-inspection guidance.
2. Add MCP `instructions` to `InitializeResult`.
3. Keep the instructions self-contained and within the protocol's practical size limits.

### Task 7: Extend smoke tests and documentation

**Files:**
- Modify: `test_mcp.ps1`
- Modify: `test_parse.ps1`
- Modify: `README.md`
- Modify: `TESTING.md`

1. Require `dm_search_context` and validate its schema in protocol smoke tests.
2. Add an optional `SearchQuery` that performs a real ranked-search assertion after parsing.
3. Document the parse-search-inspect workflow, filters, result metadata, refresh behavior, and deterministic/local design.

### Task 8: Full verification

1. Run `cargo fmt --check`.
2. Run `cargo test`.
3. Run `cargo build --release`.
4. Run the PowerShell protocol smoke test against the release binary.
5. Run the PowerShell parse/search smoke test against `C:\Users\Zoe\Documents\GitHub\Meridian-Rift\tgstation.dme`.
6. Inspect `git diff --check`, `git status --short`, and the final diff. Do not commit.
