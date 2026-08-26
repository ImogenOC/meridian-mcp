# Tracy CI Recovery and Profiler Trust Boundary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Use superpowers:test-driven-development for each behavior change, superpowers:systematic-debugging for any unexpected failure, and superpowers:verification-before-completion before reporting completion.

**Goal:** Make Meridian-MCP's Tracy integration reliable under Windows BYOND 516.1687 and portable Ubuntu native checks by repairing runtime prerequisites, retaining the collector for the profiled runtime's lifetime, rotating Tracy connections for bounded captures, validating every trace, and proving that profiling leaves the repository unchanged.

**Architecture:** `dm_tracy_launch` starts DreamDaemon and one persistent `meridian-tracy-helper --session` process. The helper immediately drains the byond-tracy stream, rotates to a fresh Tracy `Worker` for each bounded capture, validates clocks, frames, zones, files, hook progress, and queue health, writes a standard `.tracy` file plus a schema-2 Meridian sidecar, and resumes draining. Rust owns process lifetime, artifact promotion, integrity checkpoints, and MCP contracts. Windows CI installs pinned x86 runtime prerequisites before building and always retains failure evidence.

**Tech Stack:** Rust 1.95.0, Tokio, rmcp 3.1.3, serde/serde_json, SHA-256, CMake/C++20, Tracy v0.14.0, byond-tracy build-d1ec404 plus an owned Meridian health patch, PowerShell 7, BYOND 516.1687, Windows Server 2025, Ubuntu.

**Spec:** `docs/superpowers/specs/2026-08-26-tracy-profiler-reliability-design.md`

---

## Global constraints

- Preserve the public behavior and names of `dm_tracy_prepare`, `dm_tracy_launch`, `dm_tracy_capture`, `dm_tracy_status`, `dm_tracy_stop`, `dm_tracy_hotspots`, `dm_tracy_zone`, `dm_tracy_frame_stats`, and `dm_tracy_compare` except for stricter validation and richer additive output.
- Use the repository-pinned Rust 1.95.0 toolchain for every Rust command. Confirm it with `rustc +1.95.0 --version` before trusting results.
- Use PowerShell for BYOND builds and live BYOND verification.
- Keep all generated traces, sidecars, logs, baselines, and recovery journals outside the indexed source tree or in ignored evidence directories.
- Never automatically restore, delete, or overwrite a user-authored file. Recovery output records exact ownership and proposed remediation only.
- Never publish absolute host paths, account names, environment variables, command lines containing secrets, or user traces as CI artifacts.
- Leave changes in the working tree. Do not commit or push without a separate explicit user request, even where a task below ends at a review checkpoint.

## Task 1: Repair and prove the Windows BYOND runtime prerequisite chain

**Requirements:** MMCP-PROF-005, Plan 1 acceptance items 3 and 4.

**Files:**

- Create: `scripts/install-byond-runtime.ps1`
- Modify: `scripts/install-auxtools-runtime.ps1`
- Modify: `scripts/run-large-prototype-integration.ps1`
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `tests/byond_compatibility_fixture.rs`
- Modify: `tests/workflow_contract.rs`
- Modify: `docs/provenance.md`
- Modify: `docs/dependency-policy.md`

### Step 1: Write failing contract tests

Add tests that require all of these literal properties:

```rust
assert!(installer.contains("Microsoft.DXSDK.D3DX"));
assert!(installer.contains("9.29.952.8"));
assert!(installer.contains("ead0906ae8a26c18a7525da7490127a2110f7c58f18293738283e30e97c6ea4b"));
assert!(installer.contains("D3DX9_43.dll"));
assert!(installer.contains("mfc140u.dll"));
assert!(workflow.contains("runs-on: windows-2025"));
assert!(workflow.contains("if: always()"));
assert!(workflow.contains("large-prototype-evidence"));
```

Add a fixture test that runs `install-byond-runtime.ps1 -CheckOnly` against a temporary empty `System32Directory` and `ApplicationDirectory`, and asserts the structured result identifies missing `MSVCP140.dll`, `VCRUNTIME140.dll`, `mfc140u.dll`, and `D3DX9_43.dll` without changing the fixture.

### Step 2: Run the tests and confirm the expected failure

Run:

```powershell
cargo +1.95.0 test --test byond_compatibility_fixture --test workflow_contract
```

Expected: failures naming the absent installer, the old `windows-latest` runner assertion, and missing evidence upload contract.

### Step 3: Implement the pinned installer

Create a parameterized script with the following public surface:

```powershell
[CmdletBinding()]
param(
	[Parameter(Mandatory)] [string] $ApplicationDirectory,
	[string] $System32Directory = "$env:WINDIR\System32",
	[string] $DownloadDirectory = (Join-Path $env:RUNNER_TEMP 'meridian-byond-runtime'),
	[switch] $CheckOnly
)

$dxsdkPackage = 'Microsoft.DXSDK.D3DX'
$dxsdkVersion = '9.29.952.8'
$dxsdkSha256 = 'ead0906ae8a26c18a7525da7490127a2110f7c58f18293738283e30e97c6ea4b'
$requiredVcRuntimeFiles = @('MSVCP140.dll', 'VCRUNTIME140.dll', 'mfc140u.dll')
$requiredApplicationFiles = @('D3DX9_43.dll')
```

The script must:

1. Check x86 DLL architecture, not only filename presence.
2. Invoke the repository's explicit Microsoft x86 VC redistributable installation path when MSVC/MFC/UCRT components are missing, verify the installed files are x86, and record the redistributable installer hash and Microsoft signature result.
3. Download the exact NuGet package over HTTPS only when not in `-CheckOnly` mode.
4. verify SHA-256 before extraction.
5. extract `build/native/release/bin/x86/D3DX9_43.dll` app-locally beside every BYOND executable directory used by the job.
6. retain the package license and notice in the evidence directory.
7. emit one JSON result with `schema`, `status`, runner-image identity, `checked`, `installed`, per-DLL architecture/hashes, redistributable provenance, NuGet package identity, and `licenses` fields.
8. fail before launching BYOND when a required runtime remains unavailable.

Keep `install-auxtools-runtime.ps1` as the compatibility entry point, but delegate common runtime checks to the new installer rather than duplicating the DLL list.

### Step 4: Retain 65,537-type gate evidence on every exit path

In `run-large-prototype-integration.ps1`, create the evidence directory before any BYOND action and use one finalizer to write:

```powershell
$result = [ordered]@{
	schema = 1
	status = $status
	byond_version = $ByondVersion
	type_count = 65537
	launcher_exit_code_signed = [int32] $commandExitCode
	launcher_exit_code_hex = ('0x{0:X8}' -f ([uint32] $commandExitCode))
	prerequisites = $prerequisiteEvidence
	dreammaker = $dreamMakerEvidence
	dreamdaemon = $dreamDaemonEvidence
	marker_state = $markerState
	owned_processes = @($ownedProcessObservations)
	retained_fixture_id = $retainedFixtureId
	started_at_utc = $startedAtUtc
	finished_at_utc = [DateTime]::UtcNow.ToString('O')
	log_files = @($ownedLogFiles)
}
$result | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $resultPath -Encoding utf8NoBOM
```

Ensure the finalizer executes when BUILD.cmd, DreamMaker, or a post-build assertion fails. Redact the repository root from copied logs before upload.
`$dreamMakerEvidence` and `$dreamDaemonEvidence` each include the bounded command result, signed/hex exit code, redacted stdout/stderr tails, timeout state, and owned-process termination result; prerequisite evidence includes presence, x86 architecture, and hashes.

### Step 5: Update the workflow and provenance

Pin the live Windows job to `windows-2025`, invoke `install-byond-runtime.ps1`, keep BYOND at `516.1687`, and add an `actions/upload-artifact` step guarded by `if: always()` for the owned evidence directory. Document the package identity, exact hash, x86 selection, application-local placement, and license retention.

### Step 6: Verify Task 1

Run:

```powershell
cargo +1.95.0 test --test byond_compatibility_fixture --test workflow_contract
pwsh -NoProfile -File scripts/install-byond-runtime.ps1 -ApplicationDirectory "$env:TEMP\meridian-runtime-check" -CheckOnly
git diff --check
```

Expected: Rust tests pass; the local preflight returns structured missing/present state appropriate to the host; `git diff --check` is silent.

## Task 2: Add native clock, frame, artifact, and queue validation

**Requirements:** MMCP-PROF-001, MMCP-PROF-002, MMCP-PROF-003, MMCP-PROF-012, MMCP-PROF-014.

**Files:**

- Create: `helpers/tracy/src/validation.hpp`
- Create: `helpers/tracy/src/validation.cpp`
- Create: `helpers/tracy/tests/validation_tests.cpp`
- Modify: `helpers/tracy/src/session.hpp`
- Modify: `helpers/tracy/src/session.cpp`
- Modify: `helpers/tracy/src/queries.hpp`
- Modify: `helpers/tracy/src/queries.cpp`
- Modify: `helpers/tracy/CMakeLists.txt`

### Step 1: Add failing native tests

Construct deterministic in-memory fixtures for:

- monotonic raw clock samples with an explicit conversion ratio;
- zero, negative, overflowing, and implausibly large wall-time spans;
- complete frames, left-boundary frames, right-boundary frames, and frames crossing both boundaries;
- a trace containing no zones, no source files, or no frame set;
- the exact 217-byte malformed fixture;
- a nonpositive frame, a frame longer than the active window, and an invalid clock-frequency fixture;
- queue depth below capacity, exactly at capacity, and above the recorded capacity;
- a `.tracy` file that cannot be reopened after serialization.

The public validation model must be exactly:

```cpp
enum class FrameClass { Complete, LeftBoundary, RightBoundary, Spanning };

struct QueueHealth {
	uint64_t capacity;
	uint64_t depth;
	uint64_t high_water;
	uint64_t saturation_count;
	uint64_t dropped_events;
	uint64_t produced_events;
	uint64_t consumed_events;
	uint64_t last_producer_progress_raw;
	bool hook_installed;
	bool prologue_validated;
	std::string byond_build;
	std::string offset_table_identity;
};

struct CaptureValidation {
	bool valid;
	uint64_t raw_begin;
	uint64_t raw_end;
	double nanoseconds_per_tick;
	double wall_span_seconds;
	uint64_t complete_frames;
	uint64_t partial_frames;
	uint64_t zones;
	uint64_t source_files;
	QueueHealth queue;
	std::vector<std::string> error_codes;
};
```

### Step 2: Run CTest and confirm failure

Run from an x64 developer shell or the configured CMake environment:

```powershell
cmake -S helpers/tracy -B target/tracy-plan1-tests -DBUILD_TESTING=ON
cmake --build target/tracy-plan1-tests --config Release
ctest --test-dir target/tracy-plan1-tests -C Release --output-on-failure
```

Expected: configuration or compilation fails because validation files and APIs do not exist.

### Step 3: Implement strict validation

Implement pure functions with stable error codes:

```cpp
CaptureValidation ValidateCapture(
	uint64_t raw_begin,
	uint64_t raw_end,
	double nanoseconds_per_tick,
	double requested_seconds,
	const std::vector<FrameInterval>& frames,
	uint64_t zones,
	uint64_t source_files,
	const QueueHealth& queue);

FrameClass ClassifyFrame(uint64_t frame_begin, uint64_t frame_end, uint64_t range_begin, uint64_t range_end);
```

Reject captures with non-positive conversion, non-increasing raw time, wall span outside `max(2 seconds, requested_seconds * 0.25)` of the requested window, zero complete positive frames, zero complete zones, zero source files, a frame longer than the active window, increased saturation/drop counters, queue depth greater than capacity, any failed hook/prologue assertion, structural size below the valid-owned-fixture minimum, or failed trace reopen. Report every failed invariant in one result. Keep partial frames and zones in counts but exclude them from complete-sample statistics.

### Step 4: Make serialization and reopen part of success

Write to an owned temporary path, flush and close it, reopen it through Tracy's standard reader, run the validation pass, and only then return success. A failed reopen must leave the temporary file for the Rust diagnostic-retention layer.

### Step 5: Verify Task 2

Re-run the CMake/CTest commands. Expected: protocol, query, and validation tests all pass on the host configuration.

## Task 3: Extend byond-tracy with owned hook and queue-health telemetry

**Requirements:** MMCP-PROF-002, MMCP-PROF-005, MMCP-PROF-013.

**Files:**

- Create: `helpers/tracy/byond-tracy-health.patch`
- Modify: `scripts/build-tracy-helpers.ps1`
- Modify: `tracy-capabilities.json`
- Modify: `tests/tracy_build_contract.rs`
- Modify: `docs/provenance.md`

### Step 1: Add failing provenance and build-contract tests

Require the manifest and build script to name both owned patches in a fixed order, record the upstream byond-tracy revision, hash each patch, and expose telemetry fields `queue_capacity`, `queue_depth`, `queue_high_water`, `queue_saturation_count`, `queue_dropped_events`, `produced_events`, `consumed_events`, `last_producer_progress`, per-hook installation/prologue status/module-relative offset, BYOND build, and offset-table identity.

### Step 2: Run the focused test

```powershell
cargo +1.95.0 test --test tracy_build_contract
```

Expected: failure because the health patch and its provenance fields are absent.

### Step 3: Implement the patch without a second transport

Patch the existing byond-tracy Tracy channel to publish a bounded diagnostic message on connection and on health-state change. Use fixed-width integer fields and a protocol version. Do not open another socket, write into the source repository, or emit per-event logs.

The patch must track:

```cpp
struct MeridianQueueHealth {
	uint32_t schema;
	uint64_t capacity;
	uint64_t depth;
	uint64_t high_water;
	uint64_t saturation_count;
	uint64_t dropped_events;
	uint64_t produced_events;
	uint64_t consumed_events;
	uint64_t last_producer_progress_raw;
	MeridianHookHealth proc_execution;
	MeridianHookHealth server_tick;
	MeridianHookHealth map_send;
	char byond_build[32];
	char offset_table_identity[65];
};
```

Each `MeridianHookHealth` contains `installed`, `prologue_validated`, and `module_relative_offset`; it must never expose a raw ASLR address. Use atomic counters where the producer and Tracy sender threads can race. Increment `produced_events` only after the hook has successfully accepted an event, and `consumed_events` after the sender consumes it. Increment `dropped_events` for every rejected producer event. Sample health no more than once per server tick and retain the producer's lossless queue policy.

### Step 4: Verify exact patch application and artifact identity

Update the builder to use `git apply --check` and `git apply` for the empty-queue patch followed by the health patch. Fail if the checkout is already modified. Hash the unpatched revision, both patches, the final x86 DLL, and x64 helper into the helper manifest.

### Step 5: Verify Task 3

```powershell
cargo +1.95.0 test --test tracy_build_contract
pwsh -NoProfile -File scripts/build-tracy-helpers.ps1 -Help
git diff --check
```

Expected: contract test passes; help parses without executing a build; diff check is silent.

## Task 4: Implement the persistent native collector session protocol

**Requirements:** MMCP-PROF-002, MMCP-PROF-004, MMCP-PROF-013.

**Files:**

- Create: `helpers/tracy/src/collector.hpp`
- Create: `helpers/tracy/src/collector.cpp`
- Create: `helpers/tracy/tests/session_tests.cpp`
- Modify: `helpers/tracy/src/main.cpp`
- Modify: `helpers/tracy/src/protocol.hpp`
- Modify: `helpers/tracy/src/protocol.cpp`
- Modify: `helpers/tracy/src/session.hpp`
- Modify: `helpers/tracy/src/session.cpp`
- Modify: `helpers/tracy/CMakeLists.txt`

### Step 1: Add failing protocol and state-machine tests

Test newline-delimited schema-2 requests for `session_start`, `capture_window`, `session_status`, `session_stop`, and `cancel`. Test these exact transitions:

```text
Starting -> Draining -> Capturing -> Validating -> Draining -> Stopping -> Stopped
```

Also assert that:

- a second `session_start` is rejected;
- `capture_window` is rejected before readiness;
- cancel during capture returns to `Draining` after attaching a replacement worker;
- three sequential captures each use a distinct worker generation;
- producer progress must increase before `session_start` reports ready;
- malformed JSON and unknown commands return one error response without terminating the session;
- EOF performs bounded cleanup and exits nonzero if DreamDaemon is still expected but no drain worker can attach.
- only one capture can be active and one lifecycle request can be pending; every additional capture returns `capture_busy` immediately;
- unknown top-level fields and unknown command parameters are rejected;
- requests and responses beyond central byte limits are rejected without desynchronizing the stream;
- reaching session lifetime, resident-memory, trace-byte, capture-duration, or capture-count limits returns `session_limit_reached` and requires stop/relaunch.

### Step 2: Run CTest and confirm failure

Use the Task 2 CMake/CTest commands. Expected: missing collector types and schema-2 commands fail compilation/tests.

### Step 3: Implement the session envelope

Use one request and one response per line:

```json
{"schema":2,"id":"req-1","command":"session_start","host":"127.0.0.1","port":8086,"connect_timeout_ms":15000,"progress_timeout_ms":15000}
{"schema":2,"id":"req-1","ok":true,"result":{"state":"draining","worker_generation":1,"producer_progress":42}}
```

Every response repeats `schema`, `id`, and `ok`. Errors use `error.code`, `error.message`, and bounded `error.details`; logs go to stderr. Keep schema-1 one-shot offline commands functional.

Define the limits once in `protocol.hpp` and mirror them in Rust contract tests: request bytes, response bytes, session lifetime, helper resident memory, trace bytes, capture duration, capture count, one active lifecycle request, and one pending lifecycle request. The caller can request a stricter duration but cannot expand a central limit.
Inject the resident-memory reader in session tests. Production reads its own process working set on Windows and RSS on Linux before and after every lifecycle command; an unavailable reader is a structured health error, not an unlimited session. Check trace-byte limits during serialization and before artifact promotion.

### Step 4: Implement rotation semantics

On start, attach a drain worker immediately. For `capture_window`:

1. record the initial queue and producer counters;
2. close the drain worker;
3. attach a fresh capture worker and wait for the requested bounded interval;
4. close and serialize that worker;
5. validate and reopen the raw trace;
6. attach a new drain worker before sending the final response;
7. include worker generations, queue deltas, clock diagnostics, and validation in the response.

If steps 3-5 fail, preserve the raw temporary path in the response and still attempt step 6. If step 6 fails, set session state to `failed` and reject further captures.

### Step 5: Verify Task 4

Rebuild and run CTest. Expected: all native tests pass, including three consecutive capture rotations and cancellation.

## Task 5: Add the Rust persistent collector client and owned process lifecycle

**Requirements:** MMCP-PROF-004, MMCP-PROF-005, MMCP-PROF-013.

**Files:**

- Create: `src/tracy_collector.rs`
- Modify: `src/lib.rs`
- Modify: `src/process.rs`
- Modify: `src/state.rs`
- Modify: `src/tracy_protocol.rs`
- Modify: `tests/tracy_protocol.rs`
- Modify: `tests/process_runner.rs`

### Step 1: Write failing Rust tests

Use Tokio duplex streams to test the client without BYOND. Expose a testable constructor accepting owned async reader/writer handles. Test request IDs, out-of-order response multiplexing for known pending requests, unknown/duplicate response ID rejection, timeouts, stderr tail bounds, cancellation, child-exit detection, and graceful stop followed by forced kill only after the stop timeout.

Separate the testable transport from the owned child process:

```rust
pub struct CollectorTransport<R, W> {
	reader: tokio::io::BufReader<R>,
	writer: W,
	next_request_id: u64,
	pending: BTreeMap<u64, oneshot::Sender<TracyResponse>>,
	request_timeout: Duration,
}

pub struct TracyCollector {
	child: tokio::process::Child,
	transport: CollectorTransport<tokio::process::ChildStdout, tokio::process::ChildStdin>,
	stderr_tail: std::sync::Arc<tokio::sync::Mutex<VecDeque<String>>>,
}

pub enum TracySessionPhase {
	ProcessStarting,
	HookFailed,
	ListenerWaiting,
	CollectorConnecting,
	HealthyIdle,
	CaptureActive,
	ProducerStalled,
	Saturated,
	Stopping,
	Stopped,
}
```

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test tracy_protocol --test process_runner
```

Expected: tests fail because no persistent collector client exists.

### Step 3: Implement schema-2 transport

Add serializable request/response types and these methods:

```rust
impl<R, W> CollectorTransport<R, W>
where
	R: AsyncRead + Unpin,
	W: AsyncWrite + Unpin,
{
	pub async fn request<T>(&mut self, command: TracySessionCommand) -> Result<T, ToolError>;
}

impl TracyCollector {
	pub async fn spawn(spec: TracyCollectorSpec) -> Result<Self, ToolError>;
	pub async fn session_start(&mut self, request: SessionStartRequest) -> Result<SessionStatus, ToolError>;
	pub async fn capture_window(&mut self, request: CaptureWindowRequest) -> Result<CaptureResult, ToolError>;
	pub async fn status(&mut self) -> Result<SessionStatus, ToolError>;
	pub async fn cancel(&mut self) -> Result<SessionStatus, ToolError>;
	pub async fn stop(&mut self) -> Result<SessionStatus, ToolError>;
}
```

Use central input/output byte limits. Never include helper stdin or full stderr in MCP audit output. Return a bounded sanitized stderr tail only for failures.

### Step 4: Store explicit ownership in server state

Replace the capture-only Tracy state with one state object owning DreamDaemon, the collector, session phase, hook identity, evidence root, integrity journal, and exact generated paths. No task may infer ownership from filename patterns.

### Step 5: Verify Task 5

Run focused tests and `cargo +1.95.0 clippy --all-targets --all-features -- -D warnings`. Expected: focused tests and Clippy pass.

## Task 6: Rework launch, status, capture, and stop around readiness and rotation

**Requirements:** MMCP-PROF-001 through MMCP-PROF-005, MMCP-PROF-013.

**Files:**

- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/contracts.rs`
- Modify: `src/capabilities.rs`
- Modify: `src/state.rs`
- Modify: `tests/tracy_tools.rs`
- Modify: `tests/mcp_conformance.rs`

### Step 1: Write failing lifecycle tests

Add a fake collector transport and fake runtime handle. Prove:

- launch is not successful until DreamDaemon is alive, the hook hash and expected BYOND/offset-table identity match, proc-execution/server-tick/map-send hooks and prologues validate, the loopback listener and protocol-82 handshake succeed, a drain worker is connected, and producer/frame progress increases;
- a two-minute delay between launch and first capture does not discard the session;
- three captures leave state in `Draining` with increasing worker generations;
- capture failure can return to `Draining` only when replacement attachment succeeds;
- status distinguishes `process_starting`, `hook_failed`, `listener_waiting`, `collector_connecting`, `healthy_idle`, `capture_active`, `producer_stalled`, `saturated`, `stopping`, and `stopped`, and retains the last structured error;
- stop requests collector shutdown before DreamDaemon shutdown and reports both outcomes;
- an already-running profiled session rejects a second launch.

### Step 2: Run focused tests and confirm failure

```powershell
cargo +1.95.0 test --test tracy_tools --test mcp_conformance
```

Expected: failures from the old one-shot capture lifecycle and absent readiness fields.

### Step 3: Implement additive MCP outputs

Return this bounded lifecycle information from launch/status/capture:

```rust
pub struct TracyRuntimeStatus {
	pub phase: TracySessionPhase,
	pub dreamdaemon_running: bool,
	pub collector_running: bool,
	pub byond_build: String,
	pub offset_table_identity: String,
	pub hooks: HookHealthSet,
	pub producer_progress: u64,
	pub frame_progress: u64,
	pub worker_generation: u64,
	pub queue: QueueHealth,
	pub last_capture: Option<CaptureSummary>,
	pub last_error: Option<StructuredToolError>,
}
```

Keep existing arguments accepted. Keep `network_mode` best effort. Report only owned loopback endpoint observations and never claim absence of other network traffic.

### Step 4: Make stop deterministic

Stop accepts one bounded collector grace interval and one bounded DreamDaemon grace interval. It sends `session_stop`, waits, terminates only the still-owned process, writes final status and integrity checkpoints, and leaves diagnostic files intact.

### Step 5: Verify Task 6

Run focused tests and the entire Rust suite:

```powershell
cargo +1.95.0 test --test tracy_tools --test mcp_conformance
cargo +1.95.0 test --all-features
```

Expected: all pass.

## Task 7: Add atomic trace-set promotion and diagnostic retention

**Requirements:** MMCP-PROF-001, MMCP-PROF-012, MMCP-PROF-014.

**Files:**

- Create: `src/tracy_artifact.rs`
- Modify: `src/lib.rs`
- Modify: `src/atomic_output.rs`
- Modify: `src/tools/tracy.rs`
- Create: `tests/tracy_artifacts.rs`
- Modify: `tests/atomic_output.rs`

### Step 1: Write failing artifact-set tests

Test durable parent creation, two-file promotion, rollback before publication when either validation fails, collision refusal, trace reopen failure, and diagnostic retention. Assert valid output is exactly:

```text
<requested-name>.tracy
<requested-name>.tracy.meridian.json
```

Assert invalid raw data is retained under:

```text
.meridian-tracy-diagnostics/<session-id>/<capture-id>/raw.tracy
.meridian-tracy-diagnostics/<session-id>/<capture-id>/validation.json
.meridian-tracy-diagnostics/<session-id>/<capture-id>/collector-stderr.log
```

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test tracy_artifacts --test atomic_output
```

Expected: missing module/test target or failed paired-promotion assertions.

### Step 3: Implement schema-2 sidecars

Plan 1 sidecars contain the complete fixed technical and lifecycle identity. Plan 2 adds immutable workload and experiment fields without removing these fields:

```rust
#[derive(Serialize, Deserialize)]
pub struct MeridianTraceSidecarV2 {
	pub schema: u32,
	pub session_id: String,
	pub capture_id: String,
	pub created_at_utc: String,
	pub requested_duration_ms: u64,
	pub measured_wall_duration_ms: u64,
	pub reconnect_interval_ms: u64,
	pub trace_sha256: String,
	pub trace_bytes: u64,
	pub meridian_mcp_identity: HelperIdentity,
	pub helper_identity: HelperIdentity,
	pub hook_identity: HelperIdentity,
	pub hook_patch_sha256: String,
	pub tracy_revision: String,
	pub tracy_protocol: u32,
	pub byond_version: String,
	pub byond_executable_sha256: String,
	pub offset_table_identity: String,
	pub dmb_sha256: String,
	pub rsc_sha256: Option<String>,
	pub loaded_native_modules: Vec<NativeModuleIdentity>,
	pub processes: Vec<ProcessIdentity>,
	pub loopback_endpoints: Vec<LoopbackEndpoint>,
	pub active_raw_range: RawRange,
	pub frame_counts: ValidatedCounts,
	pub zone_counts: ValidatedCounts,
	pub queue_start: QueueHealth,
	pub queue_end: QueueHealth,
	pub queue_high_water: QueueHealth,
	pub clock: ClockDiagnostics,
	pub validation: CaptureValidation,
	pub worker_generation: u64,
	pub integrity_journal_id: String,
	pub integrity_status: String,
	pub normalized_artifacts: Vec<ArtifactIdentity>,
}

pub struct NativeModuleIdentity {
	pub file_name: String,
	pub sha256: String,
}

pub struct ProcessIdentity {
	pub role: String,
	pub pid: u32,
	pub creation_identity: u64,
}

pub struct RawRange {
	pub begin: u64,
	pub end: u64,
}

pub struct ValidatedCounts {
	pub raw: u64,
	pub complete: u64,
	pub partial_first: u64,
	pub partial_last: u64,
	pub invalid: u64,
	pub analyzed: u64,
}

pub struct ClockDiagnostics {
	pub timer_source: String,
	pub raw_frequency: Option<u64>,
	pub nanoseconds_per_tick: f64,
	pub handshake_raw: u64,
	pub active_begin_raw: u64,
	pub active_end_raw: u64,
	pub disconnect_raw: u64,
	pub monotonicity_failures: Vec<String>,
}
```

Define `LoopbackEndpoint`, `QueueHealth`, `CaptureValidation`, and `ArtifactIdentity` as Rust mirrors of the native/provenance contracts from Tasks 2-4, with bounded strings and no raw socket handles or absolute paths. Do not serialize absolute source paths. Normalize source names to repository-relative paths where containment is proven; otherwise store a stable redacted basename token.

### Step 4: Promote only a reopened, validated pair

Create both temporary files under the durable destination parent, fsync/flush them, validate hashes and schema, then rename the trace and sidecar. If the second rename fails, remove only the first file when its exact newly-created identity matches the operation journal; otherwise leave it and return recovery instructions.

### Step 5: Verify Task 7

Run focused tests, the whole Rust suite, and `git diff --check`. Expected: all pass and no whitespace errors.

## Task 8: Add repository integrity baselines and recovery journals

**Requirements:** MMCP-PROF-015.

**Files:**

- Create: `src/workspace_integrity.rs`
- Modify: `src/lib.rs`
- Modify: `src/state.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/tracy.rs`
- Create: `tests/workspace_integrity.rs`

### Step 1: Write failing integrity tests

Use temporary repositories to test clean, dirty, untracked, renamed, deleted, and non-Git workspaces. Assert the baseline records pre-existing changes and later checkpoints report only deltas without modifying the workspace. Test that journals never contain absolute home-directory paths.

### Step 2: Run and confirm failure

```powershell
cargo +1.95.0 test --test workspace_integrity
```

Expected: test target/module does not exist.

### Step 3: Implement a Git-first, manifest-fallback baseline

Use `git status --porcelain=v2 -z --untracked-files=all` plus tracked index identity when available. For non-Git roots, hash repository-relative file paths, sizes, and contents while excluding configured evidence roots and existing ignored/generated directories. Enforce a fixed entry/byte bound and return `integrity_scope_too_large` before launch when exceeded. Store:

```rust
pub struct IntegrityCheckpoint {
	pub action: String,
	pub recorded_at_utc: String,
	pub baseline_digest: String,
	pub current_digest: String,
	pub added: Vec<String>,
	pub modified: Vec<String>,
	pub deleted: Vec<String>,
	pub owned_paths: Vec<OwnedPathRecord>,
}
```

Write the durable session journal before starting either process. Record checkpoints after launch, each capture, cancellation, and stop. A non-owned tracked deletion, rename, or modification is a hard profiling failure with `workspace_integrity_violation` and no automatic repair. On MCP startup and `dm_tracy_status`, report an unfinished journal as `recovery_required` with its last completed action and observations.

### Step 4: Verify Task 8

Run the focused test, full Rust suite, and inspect one fixture journal for path redaction. Expected: all tests pass and fixture source files remain byte-identical.

## Task 9: Strengthen live Windows and portable Ubuntu gates

**Requirements:** All Plan 1 requirements and Plan 1 acceptance items 1-10.

**Files:**

- Modify: `scripts/run-tracy-integration.ps1`
- Create: `scripts/run-tracy-native-tests.ps1`
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `.github/workflows/ci.yml`
- Modify: `tests/workflow_contract.rs`
- Modify: `README.md`
- Create: `docs/tracy-profiling.md`
- Modify: `docs/compatibility.md`
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Modify: `docs/tool-contracts.md`

### Step 1: Write failing workflow and documentation contract tests

Require:

- Windows native helper tests and live BYOND test on `windows-2025`;
- Ubuntu native helper configure/build/CTest;
- the exact delayed-first-capture marker;
- three 30-second capture requests;
- queue saturation and drop assertions;
- trace/sidecar reopen and schema validation;
- final integrity comparison;
- `if: always()` diagnostic upload;
- capability status remaining `Experimental` until the live gates pass.
- independent Windows and Ubuntu compatibility states rather than a combined portable claim.

### Step 2: Run contract tests and confirm failure

```powershell
cargo +1.95.0 test --test workflow_contract --test tracy_build_contract
```

Expected: failures naming missing lifecycle gates.

### Step 3: Implement the live gate

`run-tracy-integration.ps1` must:

1. create a clean owned evidence root;
2. record integrity baseline;
3. run `dm_tracy_launch` through the installed MCP binary;
4. wait 120 seconds before the first capture while polling status and producer progress;
5. run three 30-second captures through the MCP entry point;
6. reopen each `.tracy`, validate its schema-2 sidecar and SHA-256, and assert zero drops/saturations;
7. call hotspots, zone, frame stats, and compare against the new artifacts;
8. run the exact 217-byte, zero-zone, negative-span, nonpositive-frame, oversized-frame, and invalid-clock-frequency fixtures and require `invalid_capture` plus retained diagnostics for each;
9. stop the session through MCP;
10. assert repository integrity is unchanged from baseline;
11. write a redacted `summary.json` even on failure.

Before the successful live launch, the Windows workflow also runs the runtime installer in `-CheckOnly` mode against an empty application-local fixture and proves missing D3DX fails preflight. It then installs the pinned package and proves DreamDaemon startup uses the application-local DLL.

### Step 4: Implement the portable native gate

The Ubuntu gate builds the x64 helper, runs every CTest target including malformed-input, clock, frame, session, rotation, and cancellation fixtures, then runs Rust tests. It does not claim live BYOND compatibility.

### Step 5: Run exact local verification

Run in this order:

```powershell
rustc +1.95.0 --version
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --all-targets --all-features -- -D warnings
cargo +1.95.0 test --all-features
cargo +1.95.0 deny check --all-features
pwsh -NoProfile -File scripts/run-tracy-native-tests.ps1
pwsh -NoProfile -File scripts/run-tracy-integration.ps1
git status --short
git diff --check
```

Expected:

- rustc reports 1.95.0;
- format, Clippy, Rust tests, dependency policy, and native tests pass;
- on a Windows host with BYOND 516.1687 and the pinned runtime installed, the live gate records one delayed capture and three consecutive 30-second captures with valid trace pairs, producer progress, zero drops/saturations, and unchanged repository integrity;
- generated evidence is ignored/untracked as designed and no user-authored source file changed.

If the host cannot run live BYOND, stop after the portable gates and report the live gate as unverified; do not infer it passed.

### Step 6: Review checkpoint

Inspect `git diff --stat`, `git diff --check`, `git status --short`, and the redacted evidence summary. Confirm every Plan 1 acceptance item and MMCP-PROF-001 through MMCP-PROF-005 plus MMCP-PROF-012 through MMCP-PROF-015 is represented by a passing test or an explicitly named live gate. Leave the changes uncommitted unless the user separately authorizes a commit.
