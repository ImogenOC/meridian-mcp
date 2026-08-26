# Tracy Profiler Reliability and Experiment Evidence Design

**Date:** 2026-08-26

## Status and relationship to the original design

This design revises the runtime, capture, validation, artifact, and promotion sections of
`2026-08-25-tracy-profiler-integration-design.md`. The original supply-chain decisions remain in
force: Tracy stays pinned to `099df3de3dc37eca4712c06b8320fb9c53596edd`, byond-tracy stays pinned
to `d1ec404737b04b1ea73d6df4a1b477deacdb1900`, the Tracy wire protocol remains 82, and profiling
remains opt-in and development-only.

The revision is required by findings MMCP-PROF-001 through MMCP-PROF-015 in
`meridian-mcp-profiling-findings.md`. A trace is diagnostic evidence until it passes every validity,
identity, integrity, and experiment-quality gate defined here.

BYOND 516.1687 replaces 516.1685 as the named current compatibility baseline. Evidence previously
recorded on 516.1685 remains historical and does not establish current compatibility. Supporting a
later BYOND build requires the dependency-policy review and fresh platform evidence; it is not
inferred from a matching Tracy protocol number.

The user authorized changes to the protected workflow, native hook patch, build, packaging, runtime,
and documentation files required by this design. Changes to Meridian-Rift's human-authored
`BUILD.cmd` remain outside scope.

## Goals

1. Make the BYOND 516.1687 Windows compatibility workflow start DreamDaemon reliably on a clean
   hosted runner and emit useful loader diagnostics when it cannot.
2. Prevent byond-tracy producer saturation by keeping a verified collector attached for the owned
   runtime's lifetime.
3. Reject corrupt, empty, non-monotonic, clock-inconsistent, or statistically unusable traces before
   they are published as successful captures.
4. Produce versioned, persistent evidence that binds each measurement to exact workload, process,
   binary, window, and tool identities.
5. Detect source-tree erosion, identify the lifecycle interval in which it occurred, and never
   restore or delete human-authored files automatically.
6. Support repeated, identity-matched controls without silently converting invalid runs into a
   performance baseline.

## Non-goals

- No arbitrary Tracy expressions, Python evaluation, HTTP service, GUI control, or UDP discovery.
- No automatic repair of a dirty or damaged repository.
- No packet-capture completeness claim.
- No cross-platform comparison that treats Windows private bytes and Linux RSS as the same metric.
- No modification of Meridian-Rift's human-authored build entry points.
- No automatic upload of user traces, which may contain source paths or other local identifiers.
- No universal compatibility claim for untested BYOND, Tracy, platform, or hook revisions.

## Delivery boundaries

The work is split into two independently reviewable implementations.

### Plan 1: CI Recovery and Profiler Trust Boundary

Plan 1 delivers the Windows runtime prerequisite repair, retained collector process, rotating Tracy
connections, internal protocol schema 2, launch readiness, hook and queue health, strict trace
validation, boundary-frame semantics, persistent diagnostic artifacts, crash-safe lifecycle,
workspace integrity journal, and all P0 acceptance gates.

### Plan 2: Experiment-Grade Profiling Evidence

Plan 2 builds on Plan 1's stable lifecycle. It delivers workload identity manifests, named windows,
range-aware analysis, separate process-memory series, loopback evidence, versioned statistics,
repeated controls and noise gates, artifact retention rules, user and agent documentation, and
promotion criteria.

Plan 2 must not weaken or bypass a Plan 1 invariant. Both plans use the contracts below.

## Finding coverage

| Finding | Design coverage |
| --- | --- |
| MMCP-PROF-001 | Clock and capture validation; diagnostic artifact retention. |
| MMCP-PROF-002 | Retained collector, rotating connections, queue-health telemetry. |
| MMCP-PROF-003 | Complete and boundary-frame classification and plausibility checks. |
| MMCP-PROF-004 | Persistent session-mode helper and consecutive capture windows. |
| MMCP-PROF-005 | Launch readiness, hook installation, producer progress, and status states. |
| MMCP-PROF-006 | Schema-2 sidecar and immutable experiment identity. |
| MMCP-PROF-007 | Authoritative windows, named phases, and range-aware analysis. |
| MMCP-PROF-008 | Exact process identity and separate role-specific memory series. |
| MMCP-PROF-009 | Statistics schema 2 and explicit raw, complete, partial, and analyzed counts. |
| MMCP-PROF-010 | `dm_tracy_control_stats` validation and noise envelope. |
| MMCP-PROF-011 | Owned loopback evidence and best-effort endpoint disclaimer. |
| MMCP-PROF-012 | Raw clock identity, conversion diagnostics, and wall-time validation. |
| MMCP-PROF-013 | Collector handshake and producer progress before launch success. |
| MMCP-PROF-014 | Durable parent creation, atomic artifact-set promotion, and reopen checks. |
| MMCP-PROF-015 | Integrity baseline, per-action checkpoints, exact ownership, and recovery journal. |

## Windows BYOND runtime prerequisites

BYOND 516.1687's Windows DreamDaemon imports the legacy `D3DX9_43.dll` runtime in addition to x86
MSVC, MFC, and UCRT components. The existing auxtools runtime check is not a sufficient DreamDaemon
loader check.

A separate PowerShell BYOND runtime prerequisite installer will:

- Verify the x86 `MSVCP140.dll`, `VCRUNTIME140.dll`, and `mfc140u.dll` runtime components.
- Install the Microsoft x86 VC redistributable only when those components are missing.
- Download `Microsoft.DXSDK.D3DX` version `9.29.952.8` from the official NuGet endpoint.
- Require package SHA-256
  `ead0906ae8a26c18a7525da7490127a2110f7c58f18293738283e30e97c6ea4b`.
- Extract only the unmodified x86 release `D3DX9_43.dll` application-locally beside the CI BYOND
  executables.
- Retain the package `LICENSE.txt` and `NOTICE.md` beside the CI runtime evidence.
- Record the DLL hashes and GitHub runner image identity without publishing machine-specific home or
  temporary-directory paths.

The compatibility workflow uses an explicit `windows-2025` runner label. Runtime installation is a
visible workflow step and never occurs inside Meridian-MCP startup or a tool call.

The over-64K prototype gate always writes a schema-versioned evidence document, including on failure.
Failure evidence contains the signed and hexadecimal launcher exit code, prerequisite presence and
hashes, DreamMaker result, bounded DreamDaemon stdout and stderr, marker state, owned-process
observations, and a logical retained-fixture identifier. The hosted artifact upload includes this
JSON and the bounded logs even when later workflow steps are skipped.

## Retained collector architecture

### Process ownership

`dm_tracy_launch` owns two children: DreamDaemon and one `meridian-tracy-helper` collector process.
Both children belong to the same MCP lifecycle and process-containment boundary. A failure to attach
the collector terminates both children before launch returns.

The collector is a persistent fixed-command process. It communicates only through inherited stdin,
stdout, and stderr. Rust writes bounded one-line JSON requests and reads one bounded response for each
matching request ID. Stderr remains diagnostic-only and size-limited. There is no listening control
socket, caller-provided environment, arbitrary command, or expression surface.

### Rotating Tracy connections

The collector process persists, but its internal Tracy `Worker` connections rotate:

1. Launch starts DreamDaemon and the collector immediately.
2. The collector attaches a drain worker and consumes producer events while no capture is requested.
3. A capture request closes the drain worker, immediately attaches a fresh capture worker, and waits
   for a verified handshake and advancing event stream.
4. The collector records the authoritative active window, captures for the requested duration,
   disconnects, writes and validates the trace, and attaches a new drain worker before replying.
5. Stop cancels an active window, closes the current worker, stops the collector, then stops
   DreamDaemon.

This keeps the producer drained without retaining an indefinitely growing in-memory trace. The
reconnect interval is measured and included in capture diagnostics. A reconnect failure is a
structured lifecycle error and does not fall back to an unobserved disconnected runtime.

Only one window may be active. The request queue is bounded to one active and one pending lifecycle
request. Additional capture requests return `capture_busy`; they are not queued indefinitely.

### Session limits

The collector enforces centrally defined limits for session lifetime, helper resident memory, trace
bytes, request bytes, response bytes, capture duration, and capture count. Reaching a limit produces
`session_limit_reached` and requires an orderly `dm_tracy_stop` followed by relaunch. It never silently
drops historical state or expands a caller-provided limit.

## Launch readiness and hook health

`dm_tracy_launch` returns success only after all of these states are true:

- DreamDaemon process ownership is established.
- The verified hook file hash matches the helper manifest.
- byond-tracy reports the expected BYOND build/offset-table identity.
- The proc-execution, server-tick, and map-send hooks report installed status.
- Each hook reports prologue validation and its module-relative offset; raw ASLR addresses are not
  exposed.
- The loopback listener is bound and the verified collector completes protocol 82 handshake.
- Producer and frame counters advance within the bounded readiness interval.
- The drain worker is active.

Status distinguishes `process_starting`, `hook_failed`, `listener_waiting`, `collector_connecting`,
`healthy_idle`, `capture_active`, `producer_stalled`, `saturated`, `stopping`, and `stopped`. It reports
the last structured error rather than reducing all failures to `running: false`.

The Meridian-owned byond-tracy patch emits bounded diagnostic data over the existing Tracy channel:

- Queue capacity, current depth, high-water depth, saturation count, and drop count.
- Total events produced and consumed.
- Last producer progress time.
- Hook installation and prologue-validation results.
- BYOND build and offset-table identity.

Queue health is sampled no more than once per server tick. The producer remains lossless: it does not
overwrite queued events or change to a silent drop policy. A nonzero drop count is therefore always
an invalid-capture condition.

## Internal helper protocol schema 2

The helper protocol advances from schema 1 to schema 2. Manifest hash verification prevents mixing a
schema-2 server with an older helper. Commands remain a closed enum.

Persistent session commands are `session_start`, `capture_window`, `session_status`, `session_stop`,
and `cancel`. Offline commands remain `hotspots`, `zone`, `frame_stats`, `compare`, and the Plan 2
`control_stats`. Every response carries the matching request ID, protocol schema, success flag, and a
structured result or error object.

The helper accepts multiple requests only when started in fixed session mode. Offline invocation
remains a one-request process. Requests reject unknown top-level fields and command parameters;
Plan 2's bounded workload `annotations` object is the only extensible field. An embedded newline,
oversized message, unknown command, mismatched ID, extra response, or response after stop is a
protocol failure.

## Capture artifact contract

### Files

A successful capture produces two sibling files:

- `<name>.tracy`: a standard Tracy trace readable by the pinned helper and Tracy GUI.
- `<name>.tracy.meridian.json`: a Meridian sidecar using schema version 2.

Rust creates a missing contained parent directory durably before capture. The trace and sidecar are
written to private reserved paths, validated, and promoted as one logical artifact set. After
promotion, Rust reopens both files, confirms their hashes and sizes, and only then assembles the MCP
response. Later captures and `dm_tracy_stop` do not remove them.

If validation fails, the requested success paths are not published. The raw trace, helper response,
and a diagnostic sidecar are moved into
`.meridian-tracy-diagnostics/<session-id>/<capture-id>/` beneath the requested contained parent.
The response identifies these diagnostic artifacts. They remain local and are never automatically
uploaded except for owned technical CI fixtures.

### Sidecar identity

The schema-2 sidecar records:

- Meridian-MCP version and Git revision.
- Helper hash, source revision, internal protocol schema, Tracy revision, and Tracy protocol.
- Hook hash, source revision, patch hash, BYOND compatibility range, and reported offset-table
  identity.
- BYOND executable version and SHA-256.
- DMB and RSC SHA-256 values.
- Loaded native module file names and hashes where the operating system permits exact enumeration.
- Game and collector process roles, PIDs, and process-creation times.
- Loopback addresses and ports.
- Requested duration, measured wall duration, reconnect interval, and active trace window.
- Raw, complete, partial, excluded, and analyzed frame and zone counts.
- Queue-health start, end, and high-water observations.
- Clock diagnostics and every evaluated validity invariant.
- Optional immutable workload metadata and its canonical SHA-256.
- Artifact paths normalized relative to the qualified workspace root, plus bytes and hashes.
- Integrity journal identity and final status.

No environment dump, account name, home path, credential-like value, or machine-specific absolute
temporary path is included.

## Window and statistics semantics

The capture worker records raw Tracy timestamps at handshake completion, active-window start,
active-window end, and disconnect. The authoritative measurement window is the interval from active
start through active end; handshake, reconnect, and disconnect events are boundary data.

A complete continuous frame has both boundaries inside the active window. A partial-first frame
begins before the window and ends inside it. A partial-last frame begins inside the window and ends
after it or at the collector disconnect boundary. Non-continuous frames use their explicit begin and
end values under the same rule.

Default percentiles use only complete frames with positive duration. Responses report:

- `statistics_schema_version: 2`
- `raw_frame_count`
- `complete_frame_count`
- `partial_first_frame_count`
- `partial_last_frame_count`
- `invalid_frame_count`
- `analyzed_frame_count`
- exclusion reasons and bounds

Zones use the same complete/partial principle. Inclusive and self-time statistics use fully contained
zones only; boundary zones are counted and excluded rather than clipped or prorated. This avoids
inventing duration for nested zones whose children cross a measurement boundary.

Legacy traces without a Meridian sidecar remain readable. They are analyzed over their full trace
range, report `window_source: full_trace_legacy`, and cannot become compatibility-verified experiment
evidence. A self-comparison remains allowed; a cross-trace comparison reports identity verification
as unavailable unless both sidecars are present.

## Clock validation

Tracy timestamps and operating-system monotonic wall time remain separate clock domains. The helper
records Tracy's reported timer source, frequency or multiplier, first and last raw timestamp,
converted timestamps, and all monotonicity failures. It does not repair an invalid trace by replacing
Tracy time with wall time.

Before publishing a trace, validation requires:

- Raw and converted timestamps are monotonic.
- Converted span and every analyzed duration are positive.
- At least one complete positive frame and one complete zone exist.
- Queue saturation and drop counts did not increase during the active window.
- The trace file reopens successfully and contains the captured frame and zone evidence.
- Trace bytes exceed the fixed minimum structural size established by a valid owned fixture.
- Converted active span is within the documented absolute and proportional tolerance of measured
  monotonic wall duration.
- No analyzed frame duration exceeds the active window duration.

Every invariant and its observed value appears in the sidecar. A failure returns `invalid_capture`
with all failed invariant codes, not only the first failure.

## Workspace integrity and operation journal

The runtime records an integrity baseline before launch and checkpoints after launch, every capture,
and stop. Existing dirty state is part of the baseline and is not itself an error.

For Git workspaces, the baseline records the tracked index identity and bounded porcelain state. For
non-Git contained roots, the baseline records a bounded relative file manifest sufficient to detect
new deletion or replacement. If the root exceeds the supported manifest bound, launch fails with
`integrity_scope_too_large` rather than running unprotected.

Each MCP-owned path is registered exactly when created. Cleanup may remove only individually
registered temporary files and directories created by the same session. Cleanup never accepts a
workspace root, unresolved parent, glob, or caller-computed recursive target. Generated DMB, RSC,
logs, hook, capture, sidecar, and diagnostic paths each have explicit ownership and retention rules.

A caller-supplied DMB or RSC is a retained input and never becomes cleanup-owned merely because it was
launched or profiled. A prepared verified hook, successful trace, successful sidecar, and diagnostic
artifact are retained outputs. Only private atomic-write files, bounded session logs designated as
temporary, and empty directories created solely for those temporary files may be removed by session
cleanup.

At every checkpoint the integrity guard reports newly introduced tracked modifications, deletions,
or renames and identifies the lifecycle interval in which they first appeared. Any undeclared tracked
deletion produces `workspace_integrity_violation`. Stop still terminates owned processes and writes
evidence, but never restores, checks out, or deletes the affected source.

A durable session journal is written before the first process starts and finalized after the last
integrity check. On MCP startup and `dm_tracy_status`, an unfinished journal is reported as
`recovery_required` with its last completed lifecycle action and integrity observations.

## Experiment identity and comparison

Plan 2 adds optional bounded immutable workload metadata to launch and capture. Supported canonical
fields include map, seed, configuration profile, feature set, scenario, phase, and an external run ID.
Unknown fields may be retained under a bounded annotations object but do not alter executable
identity. Metadata is canonicalized and hashed; it cannot be mutated after the first window.

Cross-trace comparisons verify BYOND, DMB, RSC, native module, hook, helper, map, seed,
configuration, feature-set, and scenario identities before statistics. A mismatch returns
`experiment_identity_mismatch` with the differing fields. Missing legacy identity yields a
provisional comparison marked `identity_verification: unavailable`; it cannot feed an automated
control budget.

Analysis tools accept optional timestamp or frame ranges contained within the sidecar's active
window. A capture may name one phase. Queries return the selected phase and exact normalized range.

## Process memory and network evidence

Optional process sampling records DreamDaemon and collector series separately. Each sample contains
process role, PID, creation time, monotonic wall offset, aligned Tracy offset when valid, metric kind,
unit, and observed value. Windows reports working set, private bytes, and virtual bytes. Linux reports
RSS and virtual bytes from the supported process interfaces. No combined service-plus-game field
exists, and unlike metrics are not compared across operating systems.

Owned network state directly proves the selected loopback listener and accepted collector connection.
Optional process endpoint sampling remains observational and reports `capture_complete: false`.
Packet capture is neither required nor implied.

## Repeated controls and noise gates

Plan 2 adds `dm_tracy_control_stats`, an offline fixed command that accepts between three and twenty
schema-2 capture artifact sets. It supports complete-frame p50, p95, and p99 metrics and an exact zone
identity's inclusive or self time.

All inputs must pass capture validity, identity equality, phase equality, metric availability, and
minimum complete-sample requirements. The result reports input count, valid count, incomplete count,
minimum, maximum, mean, sample standard deviation, coefficient of variation, absolute range, and the
fixed noise-envelope rule used. One malformed, missing, saturated, or mixed-identity input makes the
batch incomplete; it does not silently widen the envelope or establish a budget.

## Failure and shutdown ordering

Shutdown is idempotent and follows this order:

1. Reject new lifecycle work.
2. Cancel an active capture and wait for bounded acknowledgement.
3. Stop the collector process and its output tasks.
4. Stop DreamDaemon and its process tree.
5. Finalize local diagnostic artifacts.
6. Run the final integrity checkpoint.
7. Finalize the durable session journal.

Cancellation, helper crash, game crash, MCP transport loss, and timeout all use the same ordering.
Repeated stop reports the prior terminal state without treating an already stopped session as a new
failure.

## Documentation and compatibility state

The README, tool descriptions, architecture, security, dependency policy, provenance, tool contract,
and compatibility matrix must describe the retained collector, window semantics, schema versions,
artifact sidecars, runtime prerequisites, integrity behavior, and evidence limitations.

Tracy capabilities remain Experimental until all Plan 1 P0 gates pass on Windows and Ubuntu. Plan 2
may promote individual analysis tools to Provisional only after repeated-control gates pass. Windows
and Ubuntu retain independent compatibility states.

## Verification and acceptance

### Plan 1 acceptance

- Exact Rust 1.95.0 formatting, Clippy with `-D warnings`, all-feature tests, and dependency-policy
  gates pass.
- Native protocol, query, clock, frame-classification, rotating-connection, cancellation, and
  malformed-capture tests pass on Windows and Ubuntu.
- The Windows loader prerequisite fixture proves a missing D3DX runtime fails before launch and the
  pinned application-local runtime enables DreamDaemon startup.
- The 65,537-type prototype compiles, starts, emits its marker, and stops on BYOND 516.1687.
- DreamDaemon can boot for two minutes before the first requested window and still produce a valid
  capture without relaunch.
- Three consecutive 30-second windows have nonzero complete zones and frames, monotonic clocks, no
  saturation, and no invalid boundary durations.
- Invalid 217-byte, zero-zone, negative-span, nonpositive-frame, oversized-frame, and clock-frequency
  fixtures all return structured `invalid_capture` errors and retained diagnostics.
- Repeated launch, capture, and stop leave tracked workspace state unchanged from baseline.
- Failed CI runs upload bounded runtime, process, and capture diagnostics.

### Plan 2 acceptance

- A boot-plus-idle experiment returns an idle-only range and phase-specific hotspot and frame data.
- Sidecars bind captures to exact executable, module, workload, process, clock, and artifact
  identities without publishing host-specific paths.
- DreamDaemon and collector memory samples remain separate and align to the capture window.
- Cross-trace mismatches are rejected before statistics.
- Three valid controls produce a versioned noise envelope; any invalid or mixed input makes the batch
  incomplete.
- Loopback listener and accepted connection evidence are reported without claiming complete packet
  capture.
- Artifacts remain readable after subsequent captures and after stop.
- Documentation and compatibility tables state exactly which operating-system and BYOND combinations
  have fresh hosted evidence.
