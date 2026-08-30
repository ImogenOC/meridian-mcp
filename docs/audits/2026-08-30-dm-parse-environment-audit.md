# `dm_parse_environment` Functional and Performance Audit

Date: 2026-08-30

## Scope

This audit covers the `dm_parse_environment` request path, immutable analysis snapshot construction,
source-change detection, the ranked context-search index built from a parse, and the maintenance and
evaluation requirements for adding dense-vector retrieval. It does not treat the existing lexical
index as a vector database: the repository currently contains no embedding model, dense vectors,
approximate-nearest-neighbor index, or persistent vector store.

The checkout was clean on `main` at `fe7515c`. Verification used the checked-in Rust 1.95.0
toolchain and the current pinned SpacemanDMM revision.

## Current architecture

`dm_parse_environment` serializes parses, runs SpacemanDMM preprocessing and object-tree parsing in a
blocking worker, runs DreamChecker, builds a BM25-like in-memory `SearchIndex`, builds the remaining
language/reference/icon/proc indexes, fingerprints selected source inputs, and atomically installs a
new `AnalysisSnapshot`. A failed parse preserves the prior snapshot. A repeated request can reuse the
active snapshot when its metadata fingerprint matches.

`dm_search_context` is lexical retrieval. It tokenizes names, canonical symbols, type paths,
documentation, parameters, parent paths, file paths, and bounded source excerpts into weighted term
frequencies. It has exact-symbol, exact-name, and substring boosts, but it has no embedding or semantic
vector stage.

## Measured baseline

The release-mode scale gate against the current Meridian-Rift environment reported:

| Measurement | Result |
| --- | ---: |
| Parsed types | 64,855 |
| Indexed symbol documents | 450,258 |
| Fingerprinted inputs | 10,389 |
| Cold parse and index | 34,386 ms |
| Unchanged snapshot reuse | 451 ms |
| Post-parse working set | 1,737.2 MiB |
| Post-parse private memory | 1,895.8 MiB |

The reuse path was 76 times faster than the cold parse in this run. This is a useful optimization, but
it is process-local: restarting the MCP requires a fresh parse. Memory was sampled immediately after
snapshot installation in a separate release-mode run on the same host; that run completed in 32,659
ms, so its memory values are a post-install baseline rather than a peak-allocation measurement.

Ten real-corpus `dm_search_context` requests took 106-255 ms each. Exact identifiers and literal
feature language ranked well. Conceptual queries did not. For example, `dogmos` and the exact mapping
subsystem path ranked their intended declarations first, while `native dog library health detection`
ranked unrelated dog health variables and `find references to icon state` ranked unrelated symbols.
The latter intent is better served by an exact reference tool, but the result also demonstrates that
the current search is lexical rather than semantic.

## Findings

### Critical: snapshot reuse does not cover every loaded source file

`AnalysisContext::extract` discovers paths by walking object-tree declarations, macro history, and
diagnostics. `build_source_inputs` fingerprints that derived map. The pinned SpacemanDMM `Context`
already exposes its complete registered file list, but the current implementation does not use it.
An included file that produces no retained declaration, macro, or diagnostic can therefore be absent
from the fingerprint. Editing such a file can incorrectly reuse the prior snapshot.

The fingerprint must be built from every file registered by the parser, plus the environment and
configuration inputs. Reuse must fail closed when a registered input cannot be resolved or inspected.

### High: the search postings are constructed twice

`SearchIndex::from_object_tree` collects documents and immediately constructs all postings and
document-length statistics. `AnalysisBuild::from_parse` later canonicalizes proc ownership through
`with_proc_resolver`, which calls `SearchIndex::new` again and rebuilds the complete postings set.
The first build is discarded. At 450,258 documents this is material redundant CPU and allocation.

Document extraction, proc canonicalization, and postings construction should be distinct stages.
Postings should be constructed once, after canonical proc identities are known.

### High: source extraction repeats whole-file line indexing

`SourceCache` caches file text, but every proc excerpt calls `extract_source_from_text`, which collects
the entire file into a new `Vec<&str>` before selecting at most 80 lines. Files containing many procs
therefore repeat the same whole-file line scan and allocation many times. Cache line offsets or line
slices once per file and reuse them for every declaration.

### High: every query scans and sorts the entire document set

Search allocates a score vector sized to all documents, then iterates all 450,258 documents, allocates
lowercase copies of several fields while filtering/boosting, collects every positive hit, fully sorts
them, and truncates to at most 50. This explains the observed 106-255 ms latency even though an
inverted index already identifies the term-matching candidates.

Search should accumulate only posting candidates, use pre-normalized filter/exact-match fields, and
maintain a bounded top-k heap. Prefix/substring behavior must use an explicit auxiliary index or a
bounded fallback so the optimization does not silently change established lookup behavior.

### High: adding dense vectors without an evaluation set would be ungoverned

There is no checked-in golden query set, relevance label set, recall/MRR/NDCG gate, embedding-model
identity, chunk-schema version, or vector-index recall test. Adding an ANN engine now would make
latency measurable while leaving relevance and migration correctness unmeasured.

Qdrant's current guidance separates ANN recall from retrieval relevance and recommends a labeled
query set with metrics chosen for actual top-k usage. Its ANN guidance compares approximate results
against exact k-nearest-neighbor results. These two gates are both required: high ANN recall does not
prove that the embedding model retrieves the right DreamMaker symbols.

### Medium: timeout and worker failures use the wrong error class

All parse failures currently flow through `parse_failure`, which emits `invalid_input`. A user-supplied
bad path or blocking parser diagnostic can be invalid input, but an elapsed timeout should be
`timed_out` and a failed blocking worker should be `internal`. The response should preserve the prior
generation in all cases while reporting a recovery appropriate to the actual cause.

The input schema permits any positive timeout and omits `additionalProperties: false`. The server
should enforce the same bounded maximum advertised by its contract and reject unknown fields.

### Medium: the response does not expose actionable stage timings

Only total cold-parse duration is returned. Queue wait, preprocessing/parsing, DreamChecker,
document extraction, canonicalization, secondary-index construction, fingerprinting, and atomic
installation are not separately observable. Optimization work therefore cannot attribute changes,
and a regression can move between stages while the total remains noisy.

Stage timings and input/document counts should be emitted in the successful response and structured
logs. They are diagnostics, not a promise of deterministic timing.

### Medium: the documentation overstates fingerprint strength

The README says the reuse check proves sources are byte-for-byte identical. The implementation stores
path, length, and modification time, with a two-second settle window; it does not hash contents. This
is a fast metadata identity check, not byte equality. Either the documentation must state the actual
contract or a content digest must be introduced with measured cost. The recommended immediate change
is honest documentation plus complete parser-file coverage; content hashing should be evaluated as a
separate strict mode because hashing a station-sized corpus on every defensive parse can erase much of
the 451 ms reuse benefit.

### Medium: dense-vector lifecycle requirements are absent

If dense retrieval is added, every vector record needs a stable chunk identity and payload containing
at least environment identity, source-relative path, symbol kind, canonical symbol, declaration
location, parse generation, chunk-schema version, embedding provider/model, dimensions, and content
digest. A model or schema change requires a new collection/index generation, background rebuild, and
atomic swap. Qdrant documents atomic alias swaps and named-vector model migrations; the existing
`AnalysisSnapshot` atomic install provides the corresponding in-process consistency model.

Metadata fields used for `kind`, `type_prefix`, file, and generation filtering need their own filter
indexes. Vector-only indexes do not make filtered search efficient. Qdrant explicitly recommends
payload indexes for selective fields, and pgvector documents that ANN filtering can reduce returned
results unless the search expands candidates or uses iterative scans.

### Low: comments and tool descriptions blur lexical and semantic retrieval

Repository text describes ranked retrieval as semantic in several contexts. The current implementation
is a deterministic weighted lexical index. Documentation and response metadata should name the active
retrieval modes explicitly so clients can decide when to use exact symbol/reference tools, lexical
search, or a future hybrid search.

## Web research conclusions

The following practices are directly applicable:

1. Keep lexical and dense retrieval together. Exact DM paths, proc names, defines, and uncommon
   identifiers are first-class queries. [pgvector recommends hybrid full-text/vector search and RRF or
   a cross-encoder](https://github.com/pgvector/pgvector#hybrid-search), while
   [Elastic recommends reciprocal-rank fusion](https://www.elastic.co/docs/solutions/search/hybrid-search)
   to combine score domains without fragile score normalization.
2. Chunk on semantic code boundaries. One vector should represent a declaration or bounded proc body,
   not an arbitrary fixed character window. Long declarations still need bounded token-aware chunks;
   [OpenAI's embedding guidance](https://github.com/openai/openai-cookbook/blob/main/examples/Embedding_long_inputs.ipynb)
   recommends chunking rather than silently discarding over-limit content and notes that paragraph or
   sentence boundaries can preserve meaning. For DM, declaration/body boundaries are stronger.
3. Preserve structured payload and index selective filters. [Qdrant payload guidance](https://qdrant.tech/documentation/concepts/payload/)
   recommends indexing fields that constrain results most, and its
   [indexing guidance](https://qdrant.tech/documentation/manage-data/indexing/) distinguishes vector
   indexes from payload indexes used for efficient filtered retrieval.
4. Build immutable generations and swap atomically. [Qdrant collection aliases](https://qdrant.tech/documentation/manage-data/collections/#collection-aliases)
   support atomic model/index changes. Meridian-MCP should retain its existing rule that incomplete
   generations never replace the active snapshot.
5. Bulk-build before enabling ANN maintenance. pgvector notes that initial indexes build faster after
   loading data, and Qdrant documents disabling indexing during bulk upload and enabling it after the
   collection is populated. Parse output is naturally a bulk immutable generation, so per-document
   online index maintenance is unnecessary.
6. Tune speed only at a fixed quality target. [Qdrant's ANN recall guide](https://qdrant.tech/documentation/tutorials-search-engineering/ann-recall/)
   compares ANN against exact kNN, while its
   [retrieval-relevance guide](https://qdrant.tech/documentation/improve-search/retrieval-relevance/)
   uses labeled queries and recall@k, MRR, and NDCG@k. Both belong in the acceptance matrix.
7. Treat quantization as an evaluated tradeoff. [Qdrant quantization guidance](https://qdrant.tech/documentation/manage-data/quantization/)
   describes lower memory and latency at the cost of approximation error. It should not be enabled
   until exact-vector and unquantized baselines exist.
8. Maintain lexical indexes deliberately if persistence is introduced. SQLite FTS5 documents prefix
   indexes, field weighting, and explicit `optimize`/incremental `merge` maintenance. Its
   [FTS5 documentation](https://www.sqlite.org/fts5.html) is relevant if the in-memory BM25 index is
   later replaced with an embedded persistent lexical store.

## Design alternatives

### A. Correct and optimize the current lexical snapshot first, then add a provider-neutral hybrid seam

This is the recommended approach. It fixes proven correctness and performance defects, adds stage
metrics and a golden retrieval corpus, and defines stable chunk/payload identities. The first update
keeps analysis fully local and deterministic. A dense backend is enabled only when its model and store
are explicitly configured; lexical and exact tools remain available without it.

Advantages: immediate measurable gains, no unreviewed model download or service dependency, stable
offline behavior, and an evidence base for choosing an ANN engine. Disadvantage: the first milestone
does not pretend to provide dense semantic search.

### B. Embed a local model and ANN engine in Meridian-MCP immediately

Use a local code-capable embedding model and an embedded HNSW engine. This offers offline semantic
queries after model provisioning, but it adds a large model/runtime, packaging and CPU-architecture
concerns, model-license and update policy, cold-start cost, and a second substantial memory resident
index. The 450k-document corpus makes a careless per-symbol dense representation expensive.

This is appropriate only with an approved model artifact, redistribution policy, memory ceiling, and
quality/latency targets.

### C. Require an external embedding and vector service

Stream stable parse chunks and metadata into a configured Qdrant or PostgreSQL/pgvector service, then
fuse returned ranks with local BM25 results. This provides mature persistence, filtering, HNSW, model
migration, and operational tooling. It also makes source analysis dependent on credentials, network
policy, service lifecycle, and source-code disclosure decisions.

This conflicts with Meridian-MCP's useful local/offline analysis behavior unless it remains strictly
optional and fails open to lexical retrieval.

## Recommended update boundary

Implement alternative A in this change:

- correct complete source-input tracking;
- classify failures and bound the public contract;
- expose parse/index stage metrics;
- eliminate duplicate postings construction and repeated file-line indexing;
- replace full-result sorting and avoid unnecessary whole-corpus query work while preserving exact,
  prefix/substring, filter, and deterministic tie behavior;
- add a checked-in DreamMaker retrieval golden set with exact-identifier, natural-language, negative,
  and filtered queries;
- define stable semantic chunk records and version metadata, but do not generate fake embeddings;
- document the active retrieval mode and dense-backend requirements;
- measure release cold parse, reuse latency, lexical query latency, memory, and golden-set quality
  before and after.

Dense implementation should be a separately accepted milestone after selecting local versus external
inference and setting a source-disclosure policy. The interface and evaluation assets from this update
make that decision reversible and measurable.

## Acceptance targets for the recommended update

- Every file registered by SpacemanDMM participates in reuse invalidation.
- Failed reparses never replace the prior snapshot.
- Timeout, invalid input, and internal worker failures have distinct structured codes.
- BM25 postings are built once per successful generation.
- Source line indexing is built at most once per loaded file.
- Search preserves deterministic ranking and established filters while avoiding a full sort of all
  positive documents.
- Real-corpus release cold parse is no slower than the 34.386 s baseline and should improve by at least
  15% if the duplicate-build and line-cache findings dominate as code inspection indicates.
- Unchanged reuse remains below 750 ms on the same checkout.
- Median lexical query latency over the audit query set is below 50 ms, with no query above 100 ms on
  the same machine.
- Golden-set exact-identifier MRR is 1.0 and natural-language recall@10 does not regress.
- Full pinned tests, formatting, Clippy, release build, dependency policy, installed MCP fixture smoke,
  and real-corpus scale gates pass.

## Implementation status

Alternative A is implemented on the audit branch. Complete registered-input tracking, structured
failure codes, bounded input schema, one-pass postings construction, cached source line offsets,
stage timings, candidate-bounded ranking, retrieval counters, the owned relevance corpus, and the
schema-1 semantic chunk API are covered by focused tests. Parse results report lexical BM25 as ready
and dense retrieval as `not_configured`; no embedding model, vector index, source upload, or duplicate
semantic corpus was added to the active snapshot.

Final release-scale timings, post-install memory, installed MCP smoke results, and the full pinned gate
matrix are recorded below after Task 7. Until those fresh results exist, the original baseline and
acceptance targets above remain the comparison authority.
