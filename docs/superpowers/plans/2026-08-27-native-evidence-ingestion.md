# Native Evidence Ingestion Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Validate, phase-align, redact, summarize, and compare bounded BYOND and application-native evidence without confusing cumulative startup data with live interval measurements.

**Architecture:** A focused `native_evidence` module streams explicit artifact kinds into one normalized run model. Separate timeline, redaction, and statistics units operate only on bounded typed records. Read-only summary and comparison tools bind every result to artifact hashes and verified managed build identity; comparison recomputes inputs and rejects identity mismatches before statistics.

**Tech Stack:** Rust 2021, Rust 1.95, serde/serde_json, `csv` crate for RFC-compliant CSV, `time` crate for RFC3339 timestamps, SHA-256, existing path policy and build provenance.

**Spec:** `docs/superpowers/specs/2026-08-27-meridian-mcp-provenance-and-native-evidence-design.md`

## Global Constraints

- Supported kinds are explicit: `byond_proc_profile_json`, `byond_sendmaps_json`, `performance_csv`, `runtime_jsonl`, and `event_jsonl`.
- No format autodetection, URLs, parser plugins, scripts, expressions, JSONPath, wildcards, or transformations.
- Preserve wall time, BYOND world time, and artifact-local indexes as separate domains.
- Classify artifacts as cumulative snapshot, interval series, or event stream from fixed format semantics.
- Unaligned data remains `unassigned`; never stretch or proportionally assign it.
- Default protected identifier fields cannot be disabled.
- Raw artifacts remain unchanged and local.
- Unverified summaries may be inspected but cannot enter verified comparison.
- Different verified build identities are rejected before metric calculation.
- Every reader has fixed per-file, total, row, column, depth, line, string, group, and output bounds.
- Add LF and CRLF fixtures where the format supports both.
- Commit steps require explicit authorization during execution.

---

## Locked file structure

- Create `src/native_evidence/mod.rs`: public orchestration and shared errors.
- Create `src/native_evidence/model.rs`: descriptors, normalized records, identity, phases, and result schemas.
- Create `src/native_evidence/reader.rs`: bounded file opening, streaming hashing, line/byte limits.
- Create `src/native_evidence/byond.rs`: BYOND proc-profile and sendmaps JSON readers.
- Create `src/native_evidence/csv.rs`: performance CSV reader.
- Create `src/native_evidence/jsonl.rs`: runtime and mapped event JSONL readers.
- Create `src/native_evidence/timeline.rs`: clock anchors, phase validation, and record assignment.
- Create `src/native_evidence/redaction.rs`: fixed protected names and bounded text sanitization.
- Create `src/native_evidence/statistics.rs`: deterministic descriptive statistics and type-7 percentiles.
- Create `src/tools/native_evidence.rs`: summary and comparison MCP adapters.
- Modify `Cargo.toml`, `Cargo.lock`, `deny.toml`, `src/{lib,limits,contracts,parameters}.rs`, and `src/tools/mod.rs`.
- Create `tests/native_evidence_{readers,timeline,summary,comparison}.rs` and `tests/fixtures/evidence/**`.

### Task 1: Lock dependencies, limits, and normalized models

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `deny.toml`
- Modify: `src/limits.rs`
- Create: `src/native_evidence/mod.rs`
- Create: `src/native_evidence/model.rs`
- Modify: `src/lib.rs`
- Create: `tests/native_evidence_readers.rs`

**Interfaces:**
- Consumes: contained artifact paths, optional DMB path, workload identity, named phases, and kind-specific descriptors.
- Produces: strict `NativeEvidenceRequest`, `ArtifactDescriptor`, `ArtifactKind`, `NormalizedRun`, and central `NativeEvidenceLimits`.

- [ ] **Step 1: Add failing descriptor-validation tests**

```rust
#[test]
fn descriptors_reject_unknown_fields_and_unsupported_kinds() {
	assert!(serde_json::from_value::<NativeEvidenceRequest>(json!({
		"artifacts": [{"kind": "auto", "path": "evidence.json"}],
		"surprise": true
	})).is_err());
}
```

Add tests for more than 32 artifacts, duplicate canonical paths, more than 64 phases, invalid
half-open ranges, more than 64 selected metrics, and event field names over 256 bytes.

- [ ] **Step 2: Add pinned parser dependencies and run the failing test**

```toml
csv = "1.3"
time = { version = "0.3", features = ["parsing", "formatting"] }
```

```powershell
cargo +1.95.0 update -p csv -p time
cargo +1.95.0 test --test native_evidence_readers
```

Expected: compile fails because the model types are absent. Review the resolved dependency graph and
license policy before retaining the lockfile change.

- [ ] **Step 3: Define strict request and descriptor types**

```rust
#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
	ByondProcProfileJson,
	ByondSendmapsJson,
	PerformanceCsv,
	RuntimeJsonl,
	EventJsonl,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtifactDescriptor {
	pub kind: ArtifactKind,
	pub path: PathBuf,
	pub options: Option<ArtifactOptions>,
}

#[derive(Clone, Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NativeEvidenceRequest {
	pub artifacts: Vec<ArtifactDescriptor>,
	pub dmb_path: Option<PathBuf>,
	pub workload: Option<WorkloadIdentityInput>,
	pub phases: Vec<PhaseInput>,
}
```

Use tagged `ArtifactOptions` variants so an event mapping cannot be supplied to a proc profile.

- [ ] **Step 4: Add fixed evidence limits**

```rust
pub const MAX_EVIDENCE_ARTIFACTS: usize = 32;
pub const MAX_EVIDENCE_FILE_BYTES: u64 = 256 * 1024 * 1024;
pub const MAX_EVIDENCE_TOTAL_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_EVIDENCE_ROWS: usize = 5_000_000;
pub const MAX_EVIDENCE_COLUMNS: usize = 512;
pub const MAX_EVIDENCE_LINE_BYTES: usize = 1024 * 1024;
pub const MAX_EVIDENCE_STRING_BYTES: usize = 64 * 1024;
pub const MAX_EVIDENCE_GROUPS: usize = 100_000;
pub const MAX_EVIDENCE_PHASES: usize = 64;
```

Also cap JSON depth at 64, selected metrics at 64, returned groups at 1,000, and comparison runs at 20.

- [ ] **Step 5: Define normalized output records**

```rust
pub struct ArtifactIdentity {
	pub relative_path: String,
	pub kind: ArtifactKind,
	pub bytes: u64,
	pub sha256: String,
}

pub enum EvidenceSemantics {
	CumulativeSnapshot,
	IntervalSeries,
	EventStream,
}

pub struct NormalizedRun {
	pub identity: NativeRunIdentity,
	pub artifacts: Vec<ArtifactIdentity>,
	pub phases: Vec<NormalizedPhase>,
	pub datasets: Vec<NormalizedDataset>,
	pub redaction: RedactionSummary,
	pub warnings: Vec<EvidenceWarning>,
}
```

- [ ] **Step 6: Run model tests and dependency policy**

```powershell
cargo +1.95.0 test --test native_evidence_readers
cargo +1.95.0 deny check --all-features
```

Expected: strict descriptors pass and dependency policy accepts the exact lockfile.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add Cargo.toml Cargo.lock deny.toml src/limits.rs src/native_evidence/mod.rs src/native_evidence/model.rs src/lib.rs tests/native_evidence_readers.rs
git commit -m "feat: define native evidence contracts"
```

### Task 2: Implement bounded readers for all five formats

**Files:**
- Create: `src/native_evidence/reader.rs`
- Create: `src/native_evidence/byond.rs`
- Create: `src/native_evidence/csv.rs`
- Create: `src/native_evidence/jsonl.rs`
- Modify: `src/native_evidence/mod.rs`
- Modify: `tests/native_evidence_readers.rs`
- Create: `tests/fixtures/evidence/proc-profile.json`
- Create: `tests/fixtures/evidence/sendmaps.json`
- Create: `tests/fixtures/evidence/performance-lf.csv`
- Create: `tests/fixtures/evidence/performance-crlf.csv`
- Create: `tests/fixtures/evidence/runtime-lf.jsonl`
- Create: `tests/fixtures/evidence/runtime-crlf.jsonl`
- Create: `tests/fixtures/evidence/events.jsonl`

**Interfaces:**
- Consumes: one validated descriptor, `PathPolicy`, and remaining total-budget counter.
- Produces: `ParsedArtifact` with identity, semantics, represented range, normalized rows/events, rejected counts, and unavailable metrics.

- [ ] **Step 1: Write one failing golden test per format**

```rust
let parsed = parse_artifact(&policy, descriptor("performance-lf.csv"), &limits).unwrap();
assert_eq!(parsed.semantics, EvidenceSemantics::IntervalSeries);
assert_eq!(parsed.accepted_records, 3);
assert_eq!(parsed.metrics["tick_usage"], vec![10.0, 20.0, 30.0]);
```

Require LF and CRLF CSV/JSONL fixtures to produce identical normalized values and hashes appropriate
to their actual bytes. Add malformed-row, overlong-line, deep-JSON, oversized-file, extra-column, and
unique-group-limit tests.

- [ ] **Step 2: Run reader tests and confirm parsers are missing**

```powershell
cargo +1.95.0 test --test native_evidence_readers
```

Expected: compilation fails because `parse_artifact` is undefined.

- [ ] **Step 3: Implement one bounded reader wrapper**

```rust
pub struct BoundedEvidenceReader<R> {
	inner: R,
	bytes_read: u64,
	file_limit: u64,
	shared_total: Arc<AtomicU64>,
}
```

Hash the exact bytes while reading. Reject before allocating from a declared file length over the
limit. Enforce the live byte limit as well because metadata can change. Return the canonical
root-relative path, never a host profile prefix.

- [ ] **Step 4: Parse BYOND proc and sendmaps JSON with fixed field adapters**

Accept only documented object/array shapes represented in owned fixtures. Preserve fields that are
actually present: proc/source identity, calls/samples, total/average duration, map-send counts, and
represented timestamp. Classify both as `CumulativeSnapshot`. Unknown scalar fields are ignored;
unknown nested structures count toward depth/size limits but do not become output.

- [ ] **Step 5: Parse performance CSV with the `csv` crate**

Use `ReaderBuilder::new().flexible(false).trim(Trim::All)`. Require unique headers and the fixed
maximum column count. Parse only caller-selected numeric columns plus configured wall/world time
columns. Malformed numeric cells increment missing/rejected counts according to descriptor policy;
they do not become zero.

- [ ] **Step 6: Parse runtime and event JSONL line by line**

`runtime_jsonl` extracts fixed technical fields such as timestamp, category, proc/exception, source
file, source line, and message. `event_jsonl` resolves only simple dotted object fields declared in
`EventJsonlOptions`; arrays are rejected at a mapped path. Each line is one bounded JSON value and
trailing non-whitespace content is invalid.

- [ ] **Step 7: Run all reader fixtures**

```powershell
cargo +1.95.0 test --test native_evidence_readers
```

Expected: every format, line ending, malformed input, and bound behaves deterministically.

- [ ] **Step 8: Record the checkpoint if commits are authorized**

```powershell
git add src/native_evidence/reader.rs src/native_evidence/byond.rs src/native_evidence/csv.rs src/native_evidence/jsonl.rs src/native_evidence/mod.rs tests/native_evidence_readers.rs tests/fixtures/evidence
git commit -m "feat: read bounded native evidence"
```

### Task 3: Implement timeline alignment and default redaction

**Files:**
- Create: `src/native_evidence/timeline.rs`
- Create: `src/native_evidence/redaction.rs`
- Modify: `src/native_evidence/model.rs`
- Modify: `src/native_evidence/mod.rs`
- Create: `tests/native_evidence_timeline.rs`

**Interfaces:**
- Consumes: parsed artifact records, explicit phase ranges, optional wall/world anchors, and extra protected field names.
- Produces: validated `NormalizedTimeline`, assigned phase IDs, pre-game cumulative classification, and `RedactionSummary`.

- [ ] **Step 1: Write failing phase and redaction tests**

```rust
assert_eq!(summary.datasets[0].classification, "pre_game_cumulative");
assert_eq!(summary.datasets[0].assigned_phase, None);
assert_eq!(summary.redaction.values_redacted, 2);
assert!(!serde_json::to_string(&summary).unwrap().contains("example_player"));
```

Add tests for overlapping phase ranges, reversed ranges, conflicting anchors, boundary timestamps in
half-open ranges, unassigned events, protected field aliases, and protected `key=value` free text.

- [ ] **Step 2: Run timeline tests and confirm missing behavior**

```powershell
cargo +1.95.0 test --test native_evidence_timeline
```

Expected: compilation fails because timeline/redaction modules are absent.

- [ ] **Step 3: Implement separate time domains**

```rust
pub struct NormalizedTimestamp {
	pub wall_utc: Option<OffsetDateTime>,
	pub world_deciseconds: Option<i64>,
	pub sample_index: u64,
}

pub struct TimeAnchor {
	pub wall_utc: OffsetDateTime,
	pub world_deciseconds: i64,
	pub source_artifact: usize,
	pub source_record: u64,
}
```

Validate anchors by comparing implied offsets. Any disagreement above the fixed one-decisecond
tolerance returns `timeline_conflict`. Do not fit, average, or repair conflicting clocks.

- [ ] **Step 4: Assign half-open phases and classify cumulative data**

Assign a record only when its available domain falls within exactly one phase. If both domains are
present they must agree on the same phase. A cumulative profile whose latest represented timestamp is
before the named `game_start` phase becomes `pre_game_cumulative` and cannot populate later phase
metrics.

- [ ] **Step 5: Implement non-disableable protected fields**

```rust
const PROTECTED_FIELDS: &[&str] = &[
	"player", "player_id", "client", "client_id", "account", "account_id",
	"key", "ckey", "mob", "mob_id", "discord", "discord_id",
];
```

Compare field names case-insensitively after ASCII underscore/hyphen normalization. Replace values
with `<redacted>`, exclude them from group keys, and sanitize explicit protected `name=value` segments
from returned message samples. Cap sanitization work by the existing string length.

- [ ] **Step 6: Run timeline and redaction tests**

```powershell
cargo +1.95.0 test --test native_evidence_timeline --test native_evidence_readers
```

Expected: phase boundaries, conflicts, pre-game classification, and redaction all pass without
modifying fixture files.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/native_evidence/timeline.rs src/native_evidence/redaction.rs src/native_evidence/model.rs src/native_evidence/mod.rs tests/native_evidence_timeline.rs
git commit -m "feat: align and redact native evidence"
```

### Task 4: Add deterministic statistics and technical grouping

**Files:**
- Create: `src/native_evidence/statistics.rs`
- Modify: `src/native_evidence/mod.rs`
- Modify: `src/native_evidence/model.rs`
- Create: `tests/native_evidence_summary.rs`

**Interfaces:**
- Consumes: accepted numeric samples and redacted normalized records grouped by phase.
- Produces: `NumericSummary`, proc/sendmaps contributors, runtime signatures, event groups, and explicit unavailable metrics.

- [ ] **Step 1: Write failing exact-statistics tests**

```rust
let stats = NumericSummary::from_samples(&[1.0, 2.0, 3.0, 4.0]).unwrap();
assert_eq!(stats.count, 4);
assert_eq!(stats.mean, 2.5);
assert_eq!(stats.p50, 2.5);
assert_eq!(stats.p95, 3.85);
assert_eq!(stats.sample_standard_deviation, 1.290_994_448_735_805_6);
```

Add empty, one-sample, NaN/infinity rejection, stable sort, missing metric, cumulative contributor,
runtime signature, protected grouping field, and group truncation tests.

- [ ] **Step 2: Run summary tests and confirm missing statistics**

```powershell
cargo +1.95.0 test --test native_evidence_summary
```

Expected: compilation fails because `NumericSummary` is undefined.

- [ ] **Step 3: Implement deterministic type-7 percentiles**

```rust
fn percentile_type7(sorted: &[f64], probability: f64) -> Option<f64> {
	let h = (sorted.len() - 1) as f64 * probability;
	let lower = h.floor() as usize;
	let upper = h.ceil() as usize;
	Some(sorted[lower] + (h - lower as f64) * (sorted[upper] - sorted[lower]))
}
```

Reject non-finite values before sorting. Use compensated summation for mean and sample variance.

- [ ] **Step 4: Build fixed technical signatures**

Runtime signature order is category, exception/proc, normalized root-relative source file, source
line, and redacted message template. Event groups contain only caller-selected non-protected scalar
fields. Sort groups by descending count then canonical key, and return no more than 1,000.

- [ ] **Step 5: Summarize per phase and full represented interval**

For interval metrics return the full series plus each assigned named phase. For cumulative profiles
return present cumulative totals and top contributors only. Populate `unavailable_metrics` rather
than deriving missing durations or rates.

- [ ] **Step 6: Run summary tests**

```powershell
cargo +1.95.0 test --test native_evidence_summary --test native_evidence_timeline --test native_evidence_readers
```

Expected: exact percentiles, grouping, cumulative semantics, and unavailable metrics pass.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/native_evidence/statistics.rs src/native_evidence/mod.rs src/native_evidence/model.rs tests/native_evidence_summary.rs
git commit -m "feat: summarize native performance evidence"
```

### Task 5: Add `dm_native_evidence_summary`

**Files:**
- Create: `src/tools/native_evidence.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/contracts.rs`
- Modify: `src/parameters.rs`
- Modify: `spacemandmm-capabilities.json`
- Modify: `tests/native_evidence_summary.rs`
- Modify: `tests/tool_contracts.rs`
- Modify: `tests/mcp_conformance.rs`

**Interfaces:**
- Consumes: `NativeEvidenceRequest`, path policy, and optional verified build record for `dmb_path`.
- Produces: read-only analysis tool `dm_native_evidence_summary` and bounded schema-1 summary.

- [ ] **Step 1: Write failing tool registration and identity tests**

```rust
assert!(analysis_tools.contains("dm_native_evidence_summary"));
assert_eq!(payload["schema"], 1);
assert_eq!(payload["artifacts"][0]["sha256"].as_str().unwrap().len(), 64);
assert_eq!(payload["identity_verification"], "verified");
```

Add an unmanaged DMB case returning `identity_verification: unavailable` while still providing a
local summary.

- [ ] **Step 2: Run protocol and summary tests and confirm missing tool**

```powershell
cargo +1.95.0 test --test native_evidence_summary --test tool_contracts --test mcp_conformance --test capability_registry
```

Expected: registration, mapping, and dispatch fail.

- [ ] **Step 3: Implement the summary orchestration**

```rust
pub async fn summary(
	context: &ToolExecutionContext,
	state: &ServerState,
	args: Value,
) -> Result<ToolResult> {
	let request: NativeEvidenceRequest = serde_json::from_value(args)?;
	let result = tokio::task::spawn_blocking({
		let evidence = NativeEvidenceContext {
			policy: context.policy().clone(),
			provenance: context.build_provenance().cloned(),
		};
		move || summarize_run(&evidence, request)
	}).await??;
	Ok(ToolResult::text(serde_json::to_string(&result)?))
}
```

Define `NativeEvidenceContext { policy: PathPolicy, provenance:
Option<Arc<BuildProvenanceStore>> }` in `src/native_evidence/mod.rs`. It contains immutable handles
only and is safe to move into `spawn_blocking`.

The tool reads no active analysis snapshot unless source-root normalization needs the matching project
identity. It never changes analysis generation or writes a cached summary.

- [ ] **Step 4: Register accurate contract and schema**

Register analysis `READ`, `Experimental`, 120-second timeout, and 1 MiB output. Canonicalize every
artifact and optional DMB path in `contain_arguments`. Map the capability as Meridian-owned native
evidence rather than an upstream SpacemanDMM feature.

- [ ] **Step 5: Run summary tool tests**

```powershell
cargo +1.95.0 test --test native_evidence_summary --test tool_contracts --test mcp_conformance --test capability_registry
```

Expected: verified and unverified summaries, policy failures, bounds, and tool annotations pass.

- [ ] **Step 6: Record the checkpoint if commits are authorized**

```powershell
git add src/tools/native_evidence.rs src/tools/mod.rs src/contracts.rs src/parameters.rs spacemandmm-capabilities.json tests/native_evidence_summary.rs tests/tool_contracts.rs tests/mcp_conformance.rs
git commit -m "feat: expose native evidence summaries"
```

### Task 6: Add identity-checked native evidence comparison

**Files:**
- Modify: `src/native_evidence/model.rs`
- Modify: `src/native_evidence/mod.rs`
- Modify: `src/native_evidence/statistics.rs`
- Modify: `src/tools/native_evidence.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/contracts.rs`
- Modify: `spacemandmm-capabilities.json`
- Create: `tests/native_evidence_comparison.rs`

**Interfaces:**
- Consumes: 2-20 complete evidence requests; it recomputes every artifact identity and summary.
- Produces: `dm_native_evidence_compare` with identity compatibility, matched metrics, deltas, and repeated-run distributions.

- [ ] **Step 1: Write failing identity-first comparison tests**

```rust
let result = compare_runs(&context, vec![baseline, changed_build]).unwrap_err();
assert_eq!(result.code(), "evidence_identity_mismatch");
assert!(result.differences().contains(&"dmb_sha256".to_owned()));
assert_eq!(statistics_call_count(), 0);
```

Add compatible two-run deltas, three-run mean/standard deviation/CV, phase mismatch, semantics
mismatch, missing metric, unverified identity, and cumulative-without-interval-pair tests.

- [ ] **Step 2: Run comparison tests and confirm the API is absent**

```powershell
cargo +1.95.0 test --test native_evidence_comparison
```

Expected: compilation fails because comparison models are absent.

- [ ] **Step 3: Implement technical identity compatibility**

Compare DMB, RSC, every declared native module, service executable, fixture manifest, BYOND version,
configuration profile, map, seed, and scenario. Report all differing dimensions in sorted order.
Require `identity_verification: verified` for every input.

- [ ] **Step 4: Match metrics and calculate deltas**

Metric keys include artifact kind, phase, metric, unit, evidence semantics, and canonical technical
group key. For two runs return absolute delta and percentage delta only when the baseline is nonzero.
For 3-20 runs add minimum, maximum, mean, sample standard deviation, and coefficient of variation.

- [ ] **Step 5: Register `dm_native_evidence_compare`**

Use analysis `READ`, `Experimental`, a 600-second timeout, and 1 MiB output. Its schema accepts
`runs` with 2-20 complete `NativeEvidenceRequest` objects and no edited summary payloads.

- [ ] **Step 6: Run comparison, contract, and conformance tests**

```powershell
cargo +1.95.0 test --test native_evidence_comparison --test native_evidence_summary --test tool_contracts --test mcp_conformance --test capability_registry
```

Expected: mismatches stop before statistics; compatible comparisons are deterministic.

- [ ] **Step 7: Record the checkpoint if commits are authorized**

```powershell
git add src/native_evidence/model.rs src/native_evidence/mod.rs src/native_evidence/statistics.rs src/tools/native_evidence.rs src/tools/mod.rs src/contracts.rs spacemandmm-capabilities.json tests/native_evidence_comparison.rs
git commit -m "feat: compare native evidence runs"
```

### Task 7: Verify Plan 4 and prepare its review checkpoint

**Files:**
- Modify: `README.md`
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Create: `docs/native-evidence.md`
- Modify: `tests/documentation.rs`
- Regenerate: `docs/tool-contracts.md`

**Interfaces:**
- Consumes: all Plan 4 components.
- Produces: cross-platform fixture-verified MMCP-PROF-020 implementation.

- [ ] **Step 1: Add failing documentation assertions**

Require individual documentation for both tools, all five artifact kinds, cumulative versus interval
semantics, `pre_game_cumulative`, half-open phases, type-7 percentiles, default redaction, raw-artifact
privacy, identity mismatch, and the no-production-conclusion boundary.

- [ ] **Step 2: Write the native evidence guide and regenerate contracts**

```powershell
cargo +1.95.0 run --locked --bin render_tool_docs -- docs/tool-contracts.md
```

Include one synthetic request/response example per tool using logical relative paths and synthetic
identifiers only.

- [ ] **Step 3: Run the exact Plan 4 gate**

```powershell
rustc +1.95.0 --version
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.95.0 test --locked --test native_evidence_readers --test native_evidence_timeline --test native_evidence_summary --test native_evidence_comparison --test build_provenance --test capability_registry --test tool_contracts --test mcp_conformance --test documentation
cargo +1.95.0 deny check --all-features
git diff --check
```

Expected: every command exits 0 on Windows and Ubuntu fixture environments.

- [ ] **Step 4: Scan returned fixtures and docs for protected identifiers and host paths**

```powershell
git grep -n -I -E "[A-Za-z]:\\\\Users\\\\|/home/[^/]+|/Users/[^/]+" -- README.md docs tests/fixtures/evidence tests/native_evidence_*.rs
git status --short
```

Expected: no machine profile path appears in published artifacts. Synthetic protected values may
exist only inside redaction fixtures and assertions, never in returned golden responses.

- [ ] **Step 5: Record the Plan 4 checkpoint if commits are authorized**

```powershell
git add README.md docs/architecture.md docs/security.md docs/native-evidence.md docs/tool-contracts.md tests/documentation.rs
git commit -m "docs: explain native evidence analysis"
```
