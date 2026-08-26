# Tracy Experiment-Grade Profiling Evidence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Use superpowers:test-driven-development for each behavior change, superpowers:systematic-debugging for any unexpected failure, and superpowers:verification-before-completion before reporting completion.

**Goal:** Turn the reliable Plan 1 Tracy lifecycle into reproducible profiling experiments with immutable workload identity, authoritative named ranges, separate process-memory evidence, identity-safe comparisons, repeated-control statistics, and redacted evidence bundles suitable for performance decisions.

**Architecture:** Every profiled launch creates an immutable experiment manifest and every capture extends its paired schema-2 sidecar with a named phase, authoritative raw-clock range, bounded annotations, process identities, role-specific memory series, and owned loopback observations. Native range-aware queries classify complete and partial frames explicitly. Rust rejects comparisons with incompatible identities and adds `dm_tracy_control_stats` for 3-20 repeated controls with fixed percentile/noise calculations. Evidence export includes summaries and provenance but never automatically uploads raw traces.

**Tech Stack:** Rust 1.95.0, Tokio, rmcp 3.1.3, serde/serde_json, SHA-256, CMake/C++20, Tracy v0.14.0, PowerShell 7, BYOND 516.1687, Windows Server 2025, Ubuntu.

**Spec:** `docs/superpowers/specs/2026-08-26-tracy-profiler-reliability-design.md`

**Prerequisite:** Complete and verify `docs/superpowers/plans/2026-08-26-tracy-ci-recovery-and-trust-boundary.md`. Do not weaken its readiness, validation, artifact, queue-health, integrity, or diagnostic-retention invariants.

---

## Global constraints

- Treat experiment identity fields as immutable after `dm_tracy_launch` succeeds.
- Accept only bounded JSON strings and bounded annotation maps. Reject embedded newlines, control characters, absolute paths, environment expansions, and duplicate canonical keys.
- Keep `network_mode` best effort and `network_isolation_confirmed` false. Record only owned loopback observations; do not claim process-wide or host-wide network absence.
- Preserve the original standard `.tracy` bytes and use the paired Meridian sidecar for metadata.
- Never automatically upload raw `.tracy` files, DreamDaemon logs, source snapshots, or user-authored content.
- Leave changes in the working tree. Do not commit or push without a separate explicit user request.

## Task 1: Add immutable experiment identity and bounded workload metadata

**Requirements:** MMCP-PROF-006, MMCP-PROF-008.

**Files:**

- Create: `src/tracy_experiment.rs`
- Modify: `src/lib.rs`
- Modify: `src/contracts.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/state.rs`
- Modify: `src/tracy_artifact.rs`
- Create: `tests/tracy_experiment.rs`
- Modify: `tests/tracy_tools.rs`
- Modify: `tests/mcp_conformance.rs`

### Step 1: Write failing identity and input-bound tests

Test:

- canonical identity is stable across annotation map ordering;
- changing DMB hash, RSC hash, BYOND version/executable hash, loaded native module identity, hook/helper identity, repository revision, dirty-worktree digest, startup mode, or launch parameters changes the executable identity digest;
- changing map, seed, configuration profile, feature set, scenario, external run ID, or annotation changes the workload identity digest without changing the executable identity digest;
- launch annotations accept at most 32 entries, keys of 1-64 ASCII snake-case bytes, and values of 0-512 UTF-8 bytes;
- newlines, control characters, absolute paths, `%VAR%`, `$env:VAR`, and duplicate canonical keys are rejected;
- generated JSON contains repository-relative or redacted paths only;
- a first capture can bind workload fields omitted at launch, the binding is written before the active window starts, and later captures can only repeat the exact bound values;
- a capture can never mutate executable identity.

The model must be:

```rust
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutableIdentity {
	pub schema: u32,
	pub executable_id: String,
	pub repository_revision: Option<String>,
	pub repository_dirty_digest: String,
	pub dmb_sha256: String,
	pub rsc_sha256: Option<String>,
	pub byond_version: String,
	pub byond_executable_sha256: String,
	pub native_modules: Vec<NativeModuleIdentity>,
	pub helper_identity: HelperIdentity,
	pub hook_identity: HelperIdentity,
	pub startup_mode: String,
	pub launch_parameters_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadIdentity {
	pub workload_id: String,
	pub map: Option<String>,
	pub seed: Option<String>,
	pub configuration_profile: Option<String>,
	pub feature_set: Vec<String>,
	pub scenario: Option<String>,
	pub external_run_id: Option<String>,
	pub annotations: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExperimentIdentity {
	pub experiment_id: String,
	pub executable: ExecutableIdentity,
	pub workload: WorkloadIdentity,
}
```

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test tracy_experiment --test tracy_tools --test mcp_conformance
```

Expected: absent module, arguments, and sidecar fields fail.

### Step 3: Implement canonical identity calculation

Serialize a dedicated canonical digest input with fixed field order and lowercase hexadecimal hashes. Hash launch parameters after redacting secrets and normalizing only values whose semantics are order-insensitive. Do not include wall-clock timestamps or host-specific absolute paths in `experiment_id`.

Add optional bounded `experiment_name`, `map`, `seed`, `configuration_profile`, `feature_set`, `scenario`, `external_run_id`, and `annotations` arguments to `dm_tracy_launch`. Write an immutable `experiment-launch.meridian.json` containing executable identity and the launch-supplied workload draft before launch success. The first `dm_tracy_capture` may fill workload fields omitted at launch; immediately before its active window, canonicalize and lock the workload in a no-overwrite `experiment-identity.meridian.json`. Later captures must repeat the exact locked workload identity or omit those arguments. Unknown annotations contribute to the workload hash but never alter executable identity.

### Step 4: Extend schema-2 trace sidecars

Add `experiment_identity`, `launch_manifest_sha256`, and `experiment_manifest_sha256` to each sidecar. Validate that the sidecar identity equals the locked runtime identity before promotion.

### Step 5: Verify Task 1

Run the focused tests, full Rust suite, and `git diff --check`. Expected: all pass and no host path appears in fixture JSON.

## Task 2: Add named phases and authoritative raw-clock capture ranges

**Requirements:** MMCP-PROF-007, MMCP-PROF-009, MMCP-PROF-012.

**Files:**

- Modify: `helpers/tracy/src/protocol.hpp`
- Modify: `helpers/tracy/src/protocol.cpp`
- Modify: `helpers/tracy/src/session.hpp`
- Modify: `helpers/tracy/src/session.cpp`
- Modify: `helpers/tracy/src/queries.hpp`
- Modify: `helpers/tracy/src/queries.cpp`
- Modify: `helpers/tracy/tests/protocol_tests.cpp`
- Modify: `helpers/tracy/tests/query_tests.cpp`
- Modify: `helpers/tracy/tests/validation_tests.cpp`
- Modify: `src/tracy_protocol.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `src/contracts.rs`
- Modify: `tests/tracy_protocol.rs`
- Modify: `tests/tracy_tools.rs`

### Step 1: Write failing range tests

Test exact inclusion and classification for zones and frames that are:

- wholly before the requested range;
- exactly touching the left edge;
- complete inside the range;
- crossing only the left edge;
- crossing only the right edge;
- spanning both edges;
- exactly touching the right edge;
- wholly after the range.

Use half-open ranges `[begin, end)`. A zero-length or reversed range is invalid. Test that phase names are 1-64 bytes of lowercase ASCII letters, digits, underscore, or hyphen, and cannot repeat within one experiment unless an explicit monotonically increasing `phase_iteration` distinguishes them.

### Step 2: Run and confirm failure

Run native CTest plus:

```powershell
cargo +1.95.0 test --test tracy_protocol --test tracy_tools
```

Expected: range/phase fields and classification assertions fail.

### Step 3: Define one authoritative range model

Use:

```cpp
struct QueryRange {
	uint64_t raw_begin;
	uint64_t raw_end;
	std::string phase;
	uint32_t phase_iteration;
};

struct RangeCounts {
	uint64_t raw_total;
	uint64_t intersecting;
	uint64_t complete;
	uint64_t partial_left;
	uint64_t partial_right;
	uint64_t spanning;
	uint64_t analyzed;
};
```

The collector records raw clock begin/end at window boundaries. Offline commands use the sidecar's range by default and accept an explicit narrower range only when it is contained by the sidecar range. Never derive an authoritative range from file creation time or wall-clock timestamps.

### Step 4: Extend capture inputs and outputs

Add `phase`, `phase_iteration`, the bounded workload fields from Task 1, and optional bounded capture annotations to `dm_tracy_capture`. Workload fields can bind omitted launch values only on the first capture; capture annotations describe only that window and do not alter workload/executable identity. Return raw range, converted wall span, complete/partial counts, and phase identity. Store the same values in the paired sidecar.

### Step 5: Verify Task 2

Run CTest, focused Rust tests, and full Rust tests. Expected: boundary fixtures pass and the same range counts are reported by native and Rust deserialization tests.

## Task 3: Upgrade offline analysis to statistics schema 2

**Requirements:** MMCP-PROF-007, MMCP-PROF-009.

**Files:**

- Modify: `helpers/tracy/src/queries.hpp`
- Modify: `helpers/tracy/src/queries.cpp`
- Modify: `helpers/tracy/tests/query_tests.cpp`
- Modify: `src/tracy_protocol.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `tests/tracy_protocol.rs`
- Modify: `tests/tracy_tools.rs`

### Step 1: Write failing schema-2 analysis tests

For hotspots, zone, and frame statistics require:

```rust
pub struct StatisticsEnvelopeV2<T> {
	pub schema: u32,
	pub experiment_id: String,
	pub capture_id: String,
	pub phase: String,
	pub range: RawRange,
	pub counts: AnalysisCounts,
	pub statistics: T,
	pub warnings: Vec<AnalysisWarning>,
}

pub struct AnalysisCounts {
	pub raw: u64,
	pub intersecting: u64,
	pub complete: u64,
	pub partial_first: u64,
	pub partial_last: u64,
	pub spanning: u64,
	pub invalid: u64,
	pub excluded: u64,
	pub analyzed: u64,
}
```

Assert `analyzed <= complete <= intersecting <= raw`, partial equals all boundary classes, exclusions reconcile exactly, and every percentile reports its sample count. Empty complete samples return a structured `insufficient_complete_samples` error rather than zeros.

### Step 2: Run and confirm failure

Run CTest and focused Rust tests. Expected: current schema-1 response assertions fail.

### Step 3: Implement deterministic statistics

Sort integer nanosecond durations before selection. Use nearest-rank percentiles with index `ceil(p * n) - 1`, clamped to `[0, n - 1]`, for p50, p95, and p99. Calculate mean with a checked wide accumulator and report min/max. Exclude partial frames and partial zones from latency percentiles while retaining their explicit counts.

Keep stdout JSON-only and cap result rows centrally. Sorting ties use stable source name, line, and zone name keys.

### Step 4: Preserve compatibility explicitly

Offline request schema 1 remains readable where the existing public commands require it, but new responses use schema 2. A legacy trace without a Meridian sidecar is analyzed over its full range with `window_source: full_trace_legacy` and `identity_verification: unavailable`; self-comparison remains allowed, while cross-trace output is provisional and cannot feed `dm_tracy_control_stats`. When a sidecar exists, reject a mismatched sidecar/trace hash before invoking analysis.

### Step 5: Verify Task 3

Run native and Rust tests. Add golden JSON fixtures for one valid range, one boundary-heavy range, and one insufficient-sample result. Expected: byte-stable output across repeated runs.

## Task 4: Capture separate DreamDaemon and collector memory series

**Requirements:** MMCP-PROF-008.

**Files:**

- Create: `src/process_metrics.rs`
- Modify: `src/lib.rs`
- Modify: `src/state.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `src/tracy_artifact.rs`
- Modify: `Cargo.toml`
- Create: `tests/process_metrics.rs`
- Modify: `tests/tracy_tools.rs`

### Step 1: Write failing process-identity and sampler tests

Test sampling of the current test process and an owned child. Assert a process identity contains PID plus creation/start identity so PID reuse is detected. Assert DreamDaemon and collector samples are stored in separate arrays and cannot be summed implicitly.

Use:

```rust
pub struct ProcessIdentity {
	pub pid: u32,
	pub started_at_identity: u64,
	pub role: ProcessRole,
}

pub struct MemorySample {
	pub monotonic_offset_ms: u64,
	pub aligned_tracy_offset: Option<u64>,
	pub metric_kind: MemoryMetricKind,
	pub unit: MemoryUnit,
	pub observed_value: u64,
}

pub struct RoleMemorySeries {
	pub identity: ProcessIdentity,
	pub operating_system: String,
	pub sampling_interval_ms: u64,
	pub samples: Vec<MemorySample>,
	pub missed_samples: u64,
}
```

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test process_metrics --test tracy_tools
```

Expected: missing module and sidecar memory fields.

### Step 3: Implement cross-platform owned-process sampling

On Windows use `OpenProcess`, `GetProcessTimes`, and `GetProcessMemoryInfo` through `windows-sys`, adding only the required feature flags, and emit separate `working_set_bytes`, `private_bytes`, and `virtual_bytes` observations. On Linux read `/proc/<pid>/stat` and `/proc/<pid>/status` or `/proc/<pid>/statm`, bind the sample to process start ticks, and emit separate `rss_bytes` and `virtual_bytes` observations. Never compare unlike metric kinds across operating systems; unsupported metrics are absent rather than zero-filled.

Sample both owned processes every 500 ms from launch through stop. Bound samples to the session maximum and record missed samples. Stop a role's series immediately when identity changes or the process exits.

### Step 4: Associate exact samples with captures

Sidecars contain the subset whose monotonic offsets intersect the authoritative capture range plus one nearest sample on each side. The experiment manifest retains the complete bounded series on stop. Report DreamDaemon and collector maxima, medians, and sample counts separately.

### Step 5: Verify Task 4

Run focused tests on Windows and Ubuntu, full Rust tests, and Clippy. Expected: identities are stable, role series remain separate, and platform-unavailable fields serialize as null.

## Task 5: Record honest best-effort loopback evidence

**Requirements:** MMCP-PROF-011.

**Files:**

- Modify: `src/network_audit.rs`
- Modify: `src/state.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `src/tracy_artifact.rs`
- Modify: `tests/tracy_tools.rs`
- Create: `tests/runtime_tools.rs`
- Modify: `docs/tracy-profiling.md`

### Step 1: Write failing honesty and redaction tests

Assert output always contains:

```rust
pub struct NetworkEvidence {
	pub mode: String,
	pub network_isolation_confirmed: bool,
	pub capture_complete: bool,
	pub listener_verified: bool,
	pub collector_connection_verified: bool,
	pub owned_loopback_endpoints: Vec<OwnedEndpointObservation>,
	pub observation_failures: Vec<String>,
}
```

Assert `network_isolation_confirmed` and `capture_complete` are always false, endpoints must be loopback and owned by the known DreamDaemon or collector process identity, failures are explicit, and no unrelated endpoint or host interface is serialized.

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test tracy_tools --test runtime_tools
```

Expected: missing structured network evidence.

### Step 3: Implement bounded observations

Use the Plan 1 listener bind and accepted collector handshake as direct evidence for `listener_verified` and `collector_connection_verified`. At launch, capture, and stop, optionally observe only the configured Tracy loopback endpoint and bind it to an owned PID where the platform supports that lookup. Record observation timestamp offsets and state. Failure to observe is not proof of absence and becomes a warning, not fabricated success.

### Step 4: Verify Task 5

Run focused tests and inspect fixture JSON. Expected: all pass, the disclaimer is present, and `network_isolation_confirmed` remains false.

## Task 6: Enforce identity-safe trace comparisons

**Requirements:** MMCP-PROF-006, MMCP-PROF-007, MMCP-PROF-008, MMCP-PROF-009.

**Files:**

- Modify: `src/tracy_artifact.rs`
- Modify: `src/tracy_protocol.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `src/contracts.rs`
- Modify: `tests/tracy_artifacts.rs`
- Modify: `tests/tracy_tools.rs`

### Step 1: Write failing compatibility-matrix tests

Create sidecar pairs differing one field at a time. Require rejection for mismatched BYOND version/executable, DMB/RSC hashes, loaded native modules, helper/hook identities, map, seed, configuration profile, feature set, scenario, startup mode, phase name, phase iteration policy, range semantics, or missing memory role identity. Permit differing experiment IDs only when all required comparison dimensions match and the caller explicitly requests cross-experiment comparison. Legacy identity returns `identity_verification: unavailable`, can be self-compared, and cannot feed automated controls.

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test tracy_artifacts --test tracy_tools
```

Expected: existing compare command accepts incompatible fixtures.

### Step 3: Implement a structured compatibility result

```rust
pub struct ComparisonCompatibility {
	pub compatible: bool,
	pub mode: ComparisonMode,
	pub checked_fields: Vec<String>,
	pub mismatches: Vec<IdentityMismatch>,
	pub warnings: Vec<String>,
}
```

Run this check before native trace parsing. Default mode is `same_experiment_same_phase`. `cross_experiment` must be explicit and still requires all technical identities to match. Return all bounded mismatches at once.

### Step 4: Verify Task 6

Run focused tests and full Rust tests. Expected: every matrix cell has deterministic compatible/rejected output.

## Task 7: Add `dm_tracy_control_stats` for repeated controls

**Requirements:** MMCP-PROF-010.

**Files:**

- Modify: `helpers/tracy/src/protocol.hpp`
- Modify: `helpers/tracy/src/protocol.cpp`
- Modify: `helpers/tracy/src/queries.hpp`
- Modify: `helpers/tracy/src/queries.cpp`
- Modify: `helpers/tracy/tests/query_tests.cpp`
- Modify: `src/tracy_protocol.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/contracts.rs`
- Modify: `src/capabilities.rs`
- Modify: `tests/tracy_protocol.rs`
- Modify: `tests/tracy_tools.rs`
- Modify: `tests/mcp_conformance.rs`

### Step 1: Write failing command, bound, and statistics tests

Require 3-20 trace/sidecar pairs. Reject duplicate paths and out-of-bound input counts. Report incompatible identities, mixed phase definitions, invalid captures, fewer than three complete samples per trace, saturated inputs, and missing requested zones as incomplete inputs; one incomplete input prevents a valid noise baseline. Test exact values for p50/p95/p99, mean, min, max, range, sample standard deviation, coefficient of variation, and exact-zone inclusive/self metrics.

The response model must be:

```rust
pub struct ControlStatistics {
	pub schema: u32,
	pub input_count: u32,
	pub valid_count: u32,
	pub incomplete_count: u32,
	pub establishes_control_baseline: bool,
	pub compatibility: ComparisonCompatibility,
	pub frame_time: DistributionSummary,
	pub zones: BTreeMap<String, DistributionSummary>,
	pub noise: NoiseEnvelope,
}

pub struct NoiseEnvelope {
	pub frame_cv: f64,
	pub frame_range_ns: u64,
	pub cv_limit: f64,
	pub range_ratio_limit: f64,
	pub absolute_range_floor_ns: u64,
	pub noisy: bool,
	pub reasons: Vec<String>,
}
```

### Step 2: Run and confirm failure

Run CTest and:

```powershell
cargo +1.95.0 test --test tracy_protocol --test tracy_tools --test mcp_conformance
```

Expected: unknown native command and absent MCP tool contract.

### Step 3: Implement fixed, documented calculations

Aggregate each requested p50, p95, or p99 per-trace statistic for complete frame time and each exact inclusive/self zone selector, then compute across trace controls. Use sample standard deviation for `n >= 2`. Coefficient of variation is `sample_stddev / mean`; return a structured undefined warning when mean is zero. The fixed default noise rule is noisy when CV exceeds `0.10` or absolute range exceeds `max(1_000_000 ns, 0.20 * median)` for the requested frame or zone metric. Return the constants, raw metrics, and reasons. Any incomplete input sets `establishes_control_baseline` false regardless of the calculated envelope.

Do not silently merge zones with the same display name but different source identity. The canonical zone key is source-relative file, line, and zone name.

### Step 4: Register and document the public tool

Add `dm_tracy_control_stats` to dispatch, contracts, capabilities, README tool inventory, and detailed Tracy documentation. Inputs include 3-20 contained trace paths, the frame percentile selector, optional exact zone identity plus inclusive/self and percentile selectors, and comparison mode. Outputs include schema, compatibility, valid/incomplete counts, distributions, the fixed noise rule, and the noise result.

### Step 5: Verify Task 7

Run native tests, focused Rust tests, MCP conformance, and full Rust tests. Expected: tool discovery and invocation contracts pass and golden statistics are exact.

## Task 8: Build reproducible experiment and evidence commands

**Requirements:** MMCP-PROF-006 through MMCP-PROF-011 and Plan 2 acceptance items 1-7.

**Files:**

- Create: `scripts/run-tracy-experiment.ps1`
- Create: `scripts/validate-tracy-evidence.ps1`
- Modify: `scripts/run-tracy-integration.ps1`
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `tests/workflow_contract.rs`
- Modify: `docs/tracy-profiling.md`
- Modify: `README.md`

### Step 1: Write failing workflow/script contract tests

Require the experiment script to expose:

```powershell
param(
	[Parameter(Mandatory)] [string] $ExperimentName,
	[Parameter(Mandatory)] [string] $Phase,
	[ValidateRange(3, 20)] [int] $ControlCount = 5,
	[ValidateRange(5, 300)] [int] $CaptureSeconds = 30,
	[string] $Map,
	[string] $Seed,
	[string] $ConfigurationProfile,
	[string[]] $FeatureSet = @(),
	[string] $Scenario,
	[string] $ExternalRunId,
	[hashtable] $Annotations = @{},
	[string[]] $ZoneKeys = @()
)
```

Require it to use MCP prepare, launch, status, capture, analysis, control stats, and stop entry points; emit an experiment manifest; validate all trace pairs; and write a redacted evidence index. Require the workflow upload path to include an explicit `!**/*.tracy` exclusion and upload summary evidence only.

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test workflow_contract
```

Expected: missing scripts and upload exclusions.

### Step 3: Implement the experiment runner

The runner must:

1. validate clean containment and create an owned evidence root;
2. launch one immutable experiment through MCP;
3. capture `ControlCount` repetitions with the same named phase and increasing iteration;
4. validate and analyze each pair;
5. invoke `dm_tracy_control_stats` with exact zone keys;
6. stop the runtime in `finally`;
7. run the Plan 1 integrity comparison;
8. produce `experiment.json`, `control-stats.json`, `evidence-index.json`, and redacted summaries;
9. leave raw traces local and list their hashes in the index.

Every failed run still emits an evidence index containing completed steps, owned artifacts, validation failures, and cleanup outcomes.

### Step 4: Implement independent evidence validation

`validate-tracy-evidence.ps1` rehashes every local trace/sidecar pair, verifies schema and immutable identity, confirms complete/partial counts, checks role-specific memory series and network disclaimers, and validates control-stat inputs. It performs no launch and writes no source file.

### Step 5: Verify Task 8

Run script contract tests and a fixture-only validation. On a Windows BYOND host, first run a boot-plus-idle phase and prove its authoritative range excludes boot while phase-specific hotspot/frame data remain inside the idle window. Then run one 3-control smoke experiment with 5-second windows and the acceptance 5-control experiment with 30-second windows. Expected: valid local trace pairs that remain readable after later captures and stop, reproducible summaries, no raw trace upload, and unchanged repository integrity.

## Task 9: Document evidence interpretation and compatibility promotion rules

**Requirements:** All Plan 2 requirements and Plan 2 acceptance items 1-8.

**Files:**

- Modify: `README.md`
- Modify: `docs/tracy-profiling.md`
- Modify: `docs/compatibility.md`
- Modify: `docs/dependency-policy.md`
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Modify: `docs/provenance.md`
- Modify: `docs/tool-contracts.md`
- Modify: `tests/documentation.rs`
- Modify: `tests/workflow_contract.rs`

### Step 1: Write failing documentation tests

Require the documentation to explain every Tracy tool individually, especially `dm_tracy_prepare`, `dm_tracy_launch`, `dm_tracy_capture`, `dm_tracy_status`, `dm_tracy_stop`, `dm_tracy_hotspots`, `dm_tracy_zone`, `dm_tracy_frame_stats`, `dm_tracy_compare`, and `dm_tracy_control_stats`. Require definitions of complete/partial frames, half-open ranges, immutable identity, process roles, memory fields, best-effort networking, noise thresholds, raw-trace retention, and Experimental compatibility status.

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test documentation --test workflow_contract
```

Expected: missing Plan 2 tool and interpretation text.

### Step 3: Write operational documentation

Include one exact PowerShell experiment example using non-personal sample values, one evidence-directory tree, and one interpretation table. State explicitly:

- a valid trace proves capture integrity, not representative workload;
- complete samples drive percentiles while partial samples remain visible in counts;
- DreamDaemon and collector memory are separate;
- identity mismatches invalidate default comparisons;
- high CV/noise prevents a performance conclusion but does not erase raw measurements;
- network observations are best effort and do not prove isolation;
- raw traces remain local unless a human explicitly chooses to share them.

Overall Tracy compatibility remains `Experimental` until both Plan 1 Windows live and Ubuntu native gates and the Plan 2 repeated-control acceptance run pass for the named BYOND baseline. Individual analysis tools can become `Provisional` only after their repeated-control gates pass. Windows and Ubuntu compatibility states remain independent, and the table must name the exact hosted run evidence rather than infer support across platforms.

### Step 4: Run complete verification

Run:

```powershell
rustc +1.95.0 --version
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --all-targets --all-features -- -D warnings
cargo +1.95.0 test --all-features
cargo +1.95.0 deny check --all-features
pwsh -NoProfile -File scripts/run-tracy-native-tests.ps1
pwsh -NoProfile -File scripts/validate-tracy-evidence.ps1 -EvidenceDirectory <owned-fixture-directory>
git status --short
git diff --check
```

Then, on the Windows BYOND 516.1687 host, run:

```powershell
pwsh -NoProfile -File scripts/run-tracy-experiment.ps1 -ExperimentName baseline-516-1687 -Phase steady-state -ControlCount 5 -CaptureSeconds 30
```

Expected:

- exact Rust pin, format, Clippy, tests, dependency policy, and native tests pass;
- fixture validation passes without launching BYOND;
- live experiment produces five identity-compatible valid trace pairs, separate role memory series, explicit range/count data, honest loopback evidence, and deterministic control statistics;
- repository integrity matches the pre-launch baseline;
- raw traces remain local and are absent from CI upload globs.

If the live environment is unavailable, report Plan 2's live acceptance as unverified and leave compatibility Experimental.

### Step 5: Review checkpoint

Cross-reference MMCP-PROF-006 through MMCP-PROF-011 and every Plan 2 acceptance item against a passing test, fixture validation, or explicitly named live gate. Inspect all generated JSON for personal identifiers and absolute paths. Leave all changes uncommitted unless the user separately authorizes a commit.
