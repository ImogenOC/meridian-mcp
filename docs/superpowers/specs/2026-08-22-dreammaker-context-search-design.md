# DreamMaker Context Search Design

## Objective

Add repository-scale, natural-language code retrieval to `meridian-mcp` without requiring a hosted embedding model, vector database, or separate indexing service. Retrieval must use the already-parsed SpacemanDMM object tree so results retain exact DreamMaker symbol identity and source locations.

## User workflow

1. Call `dm_parse_environment` for the target `.dme`.
2. Call `dm_search_context` with a behavioral or symbolic query.
3. Use `dm_get_type`, `dm_get_proc`, `dm_get_var`, or `dm_get_definition` to inspect and verify exact results.
4. Re-run `dm_parse_environment` after source changes to refresh both parser state and the search index.

The MCP initialization response will describe this workflow so clients do not need repository-specific prompt text to discover it.

## Indexed units

The index contains one document for each non-builtin:

- DreamMaker type
- declared type variable
- proc implementation, including each override separately

Every document carries structured metadata: kind, canonical symbol, short name, owning type, parent type where applicable, source file, line and column, documentation, parameters, source excerpt, and proc override position/count.

## Retrieval

The first implementation uses an in-memory BM25 inverted index. Tokens are lower-cased Unicode alphanumeric runs, so paths and snake_case identifiers naturally split into useful terms. Field weights prioritize symbol names and canonical paths, then documentation and parameters, then source and file names. Exact name and canonical-symbol matches receive explicit boosts.

This choice is deterministic, local, inexpensive to refresh, easy to test, and operationally simpler than Claude Context or another external vector stack. The index API stays isolated in `src/search.rs`, allowing hybrid semantic retrieval to be added later without changing the MCP contract.

## MCP contract

`dm_search_context` accepts:

- `query` (required string)
- `kind` (`all`, `type`, `proc`, or `var`)
- `type_prefix` (optional canonical path prefix)
- `file_filter` (optional case-insensitive path substring)
- `limit` (default 10, maximum 50)
- `include_source` (default true)
- `max_source_lines` (default 40, maximum 200)

It returns JSON containing the normalized query terms, total indexed document count, result count, and ranked results with scores and structured symbol metadata.

## Lifecycle and failure behavior

The index is built only after a `.dme` parses successfully and is cached alongside the object tree in `ServerState`. Parsing another environment atomically replaces the previous parser state and index. Clearing parser state also clears the index. Calling search before parsing returns a direct instruction to call `dm_parse_environment` first.

Source files are read through a per-build cache and source excerpts are bounded. Failure to read one source file does not invalidate the parsed object tree; the affected documents remain searchable by symbol and metadata.

## Verification

Verification includes unit tests for tokenization, ranking, boosts, filters, deterministic ties, source extraction, and parser-backed metadata; MCP schema and initialization tests; existing Rust tests; a release build; protocol smoke tests; and a real parse/search against Meridian-Rift's `tgstation.dme`.

## Constraints

- No external service, model credential, or persistent database.
- No commit or push; changes remain in the working tree.
- Existing exact inspection tools remain the verification authority.
- Compiler/runtime verification remains distinct from parser/search results.
