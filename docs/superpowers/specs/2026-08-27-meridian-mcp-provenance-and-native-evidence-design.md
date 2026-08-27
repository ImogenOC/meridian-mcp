# Meridian-MCP Provenance and Native Evidence Design

**Date:** 2026-08-27

## Status and scope

This design addresses the remaining open findings MMCP-PROF-017 through MMCP-PROF-021 from
`meridian-mcp-profiling-findings.md`. That findings document remains an external evidence record;
this specification is the repository-owned implementation contract.

The work extends existing path policy, analysis snapshots, artifact hashing, runtime ownership,
Tracy integrity journaling, tool contracts, and build identity. It does not replace those systems or
weaken their trust boundaries.

The approved delivery order is:

1. Worktree authorization diagnostics and canonical proc ownership.
2. Compile and fixture provenance.
3. Standard-runtime integrity.
4. Native evidence ingestion and comparison.
5. Documentation, compatibility evidence, and release gates.

Each stage is independently testable. Later stages may depend on earlier identity types, but no stage
may bypass an earlier invariant.

## Goals

1. Explain the effective path-policy configuration and authorize explicitly trusted linked Git
   worktrees without broadly allowing unrelated directories.
2. Make exact proc lookup, search, definitions, and document symbols agree on declaration and
   implementation ownership.
3. Bind MCP-managed DMB and RSC artifacts to the exact source, generated, native, and service inputs
   that produced them.
4. Prevent a known stale managed DMB from launching after changed inputs or a failed compile.
5. Detect and report tracked workspace mutations caused during an ordinary `dm_run` session without
   reverting human or game-authored changes.
6. Read, validate, align, summarize, and compare bounded native BYOND and application evidence while
   preserving exact run and build identity.
7. Keep human-built artifacts usable when Meridian-MCP has no provenance record, while making their
   unverified status explicit.

## Non-goals

- No dynamic client expansion of the startup trust boundary.
- No authorization of every sibling directory merely because it is below a common parent.
- No modification of Git configuration, remotes, linked-worktree metadata, or repository contents to
  authorize a worktree.
- No automatic repair, checkout, reset, deletion, or restoration of a changed workspace.
- No claim that an unmanaged human-built DMB is fresh.
- No forced migration of human build scripts to Meridian-MCP manifests.
- No inference of native modules, service executables, generated bindings, or required procs when a
  fixture manifest does not declare them.
- No arbitrary expressions, JSONPath, scripting, SQL, regular-expression replacement language, or
  caller-provided parser plugins in native evidence tools.
- No upload of raw evidence, traces, logs, profiles, or manifests.
- No claim that default identifier redaction proves the source artifacts contain no personal data.

## Finding coverage

| Finding | Design coverage |
| --- | --- |
| MMCP-PROF-017 | Immutable repository worktree expansion, effective-policy status, structured path-policy diagnostics. |
| MMCP-PROF-018 | Shared proc-resolution model with separate declaration and implementation owners. |
| MMCP-PROF-019 | Standard-runtime integrity baseline, monitor, journal, checkpoints, and warnings. |
| MMCP-PROF-020 | Bounded native evidence readers, normalized phase model, redaction, summaries, and comparisons. |
| MMCP-PROF-021 | Durable build provenance, fixture synchronization, stale-artifact classification, and launch gate. |

## Delivery boundaries

The implementation is split into four focused plans followed by one release plan.

### Plan 1: Analysis correctness and policy visibility

Plan 1 implements MMCP-PROF-017 and MMCP-PROF-018. It adds repository-aware worktree expansion at
startup, a read-only server-status tool, richer policy failures, and one proc-resolution service used
by all exact and indexed analysis tools.

### Plan 2: Compile and fixture provenance

Plan 2 implements MMCP-PROF-021. It adds the private state directory, build and fixture manifest
schemas, durable artifact records, stale classification, fixture synchronization, and the managed
DMB launch gate.

### Plan 3: Standard-runtime integrity

Plan 3 implements MMCP-PROF-019. It reuses and generalizes the Tracy integrity model for standard
DreamDaemon sessions, adds bounded mutation monitoring, and returns structured warnings from runtime
status and stop.

### Plan 4: Native evidence ingestion

Plan 4 implements MMCP-PROF-020. It adds bounded parsers, normalized evidence models, named phase
alignment, identifier redaction, summaries, and identity-checked comparison.

### Plan 5: Compatibility and release

Plan 5 updates generated and human documentation, runs cross-platform fixtures, exercises managed
compile-to-run behavior with BYOND, and records platform-specific compatibility status. Fixture-only
success does not establish live BYOND compatibility.

## Configuration and private state

### Immutable startup configuration

The existing `MERIDIAN_MCP_ROOTS` remains the exact-root allowlist. A new optional
`MERIDIAN_MCP_REPOSITORIES` path list authorizes the linked worktrees belonging to explicitly named
Git repositories.

For each configured repository path, startup will:

1. Canonicalize and contain the configured path under an explicit workspace root or accept it as an
   exact repository authorization entry.
2. Resolve the Git common directory with a fixed `git rev-parse --path-format=absolute
   --git-common-dir` invocation.
3. Enumerate linked worktrees with fixed `git worktree list --porcelain -z` output.
4. Canonicalize every returned worktree root.
5. Re-resolve each candidate's common directory and require exact equality with the configured
   repository's common directory.
6. Freeze the resulting effective roots before the MCP stdio transport starts.

The server never accepts a repository, worktree, or root path from a tool call. A worktree created
after startup requires an MCP restart before it becomes effective. An unrelated Git checkout, a
directory that merely shares a parent, and a worktree whose common-directory identity differs remain
outside policy.

Development mode adds `MERIDIAN_MCP_STATE_DIR`. It names an existing private directory used for
durable build records and runtime journals. The directory is canonicalized at startup, is never
accepted from a tool call, and must not be inside a game workspace root. Development startup fails
when the directory is absent, aliases a workspace through a symlink or reparse point, or is not
writable. CI and installation scripts create it before launching the server.

The private state directory may contain machine-local canonical paths. Those records remain local,
are excluded from published compatibility evidence, and are never copied into repository
documentation.

### Effective root records

Each effective root has:

- Canonical root path.
- Source: `explicit_root` or `linked_git_worktree`.
- Local repository identity kind and digest when available.
- Worktree HEAD revision and dirty state when available.
- Startup validation status.

The local repository identity is a SHA-256 digest of the canonical Git common-directory identity. It
is suitable for proving that two local worktrees belong to one repository. It is not a portable
project identity and is never used as a replacement for source revision or remote provenance.

## Server status and path-policy failures

A new analysis-safe `dm_server_status` tool reports:

- Meridian-MCP build identity.
- Capability mode and active optional capability states.
- Containment mode `immutable_startup_roots`.
- Effective roots and their authorization sources.
- Compiler allowlist entries.
- Private state readiness without listing state-record contents.
- Active analysis generation, parsed DME, and project root when present.
- Runtime summary without changing process state.

Every path-policy failure includes the same bounded policy context:

- Stable policy code.
- Requested path and canonical path when resolution succeeded.
- Policy source `server_startup_configuration`.
- Containment mode.
- Effective allowed roots and root sources.
- Recovery guidance explaining whether the caller must select an existing root or restart with an
  explicitly authorized repository/worktree.

This context is returned only to the local MCP caller. Tool descriptions and checked-in fixtures use
logical placeholder paths and never preserve machine-specific profile paths.

## Canonical proc ownership

### Resolution model

The current object tree can represent a child override whose local proc entry has an implementation
but no new declaration. Treating `declaration.is_some()` as the implementation-owner test therefore
selects the inherited parent incorrectly.

One shared `ProcResolution` service will resolve:

- Requested type path and proc name.
- Nearest local implementation owner.
- Original declaration owner.
- Ordered implementation chain from requested type toward ancestors.
- Resolution kind: `local_implementation`, `inherited_implementation`, or `not_found`.
- Candidate source locations and the reason each candidate was selected or skipped.

Declaration ownership and implementation ownership are separate fields. A child override remains the
implementation owner even when its signature declaration is inherited. The declaration owner is the
nearest ancestor entry carrying the declaration metadata.

### Consumers

`dm_get_proc`, `dm_get_definition`, `dm_search_symbols`, `dm_search_context`,
`dm_document_symbols`, and `dm_find_implementations` use the same resolver or indexes built from its
canonical records.

`dm_get_proc` returns:

- `requested_type_path`.
- `proc_name`.
- `implementation_owner`.
- `declaration_owner`.
- `resolution_kind`.
- The selected implementation body and location.
- The bounded override chain.
- `resolution_diagnostics` only when inheritance or normalization affected the result.

Search and document-symbol parent fields use `implementation_owner`, not the declaration ancestor.
Definition lookup defaults to the nearest implementation and exposes the declaration as a separate
related location. Existing callers that read `type_path`, `declared`, and `overrides` retain those
fields during one compatibility cycle, but their values are derived from the canonical resolution.

### Proc-resolution failures

A missing proc returns a structured `symbol_not_found` result containing the normalized requested
type, proc name, searched type chain, and bounded same-name candidates. It does not silently return an
ancestor with a different normalized path. An internal disagreement between indexes returns
`symbol_index_inconsistent` and the analysis generation; it does not choose whichever index answered
first.

## Build and fixture provenance

### Managed and unmanaged artifacts

A DMB is `managed` after a successful `dm_compile` or `rift_compile` creates a verified build record,
or after an explicit fixture manifest registers it. A DMB with no durable Meridian-MCP record is
`unmanaged`.

Managed artifacts are strict:

- `dm_run`, `dm_debug_launch`, and `dm_tracy_launch` revalidate the current inputs against the last
  successful build record.
- A known stale managed artifact is refused without a caller override.
- Running the DMB outside Meridian-MCP remains a human-controlled action.

Unmanaged human-built artifacts remain launchable by default. Launch results and status report
`provenance_status: unverified` and the reason `no_managed_build_record`. Callers may set
`require_verified_provenance: true` to reject unmanaged artifacts. This option cannot permit a stale
managed artifact.

### Fixture manifest schema

A versioned fixture manifest is a checked-in or contained JSON document with:

- Schema version and logical fixture ID.
- DME path and expected DMB and optional RSC paths.
- Declared input files grouped by role: `source`, `generated_binding`, `native_module`,
  `service_executable`, or `configuration`.
- Required generated proc paths and optional expected argument names.
- Optional expected protocol/build constants represented as exact text tokens.

Paths are relative to the manifest directory, use forward-slash normalization in the canonical
document, and must resolve below one effective workspace root. Globs, parent traversal, URLs,
commands, environment variables, and optional executable arguments are forbidden. Every declared
file must exist and be a regular file before a fixture can become verified.

The manifest declares inputs; it does not authorize execution. Native modules and service
executables are hashed but are not launched by the sync checker.

### Build record schema

A successful managed compilation writes an atomic schema-1 build record under the private state
directory. The record contains:

- Stable record ID and canonical artifact lookup key.
- MCP build identity and record schema.
- Compiler path, version when observable, and SHA-256.
- Compilation mode and bounded normalized arguments.
- Project identity, DME-relative path, analysis generation, and source revision/dirty state.
- Every verified input's relative path, role, byte size, and SHA-256.
- DMB and optional RSC relative paths, sizes, timestamps, and SHA-256 values.
- Fixture manifest path relative to its workspace root and manifest SHA-256 when used.
- Compilation result and creation time.

The canonical artifact lookup key is derived from the local repository identity and normalized
workspace-relative DMB path. Canonical absolute paths may remain in the private local record for
lookup, but published tool results prefer root-relative paths plus the effective-root identity.

### Input closure

A verified general build requires an active successful analysis snapshot for the same canonical DME.
The source input closure comes from the snapshot's parsed source files and configuration files. A
fixture manifest adds explicit generated, native, service, and configuration inputs that cannot be
reliably inferred from DreamMaker includes.

If `dm_compile` is called without a matching active snapshot, compilation may proceed but cannot
create a verified managed record. Its result reports `provenance_status: unverified` and recommends
calling `dm_parse_environment` first. A failed or partial parse never replaces a prior matching
snapshot.

`rift_compile` already requires a qualified active project profile and therefore always attempts a
verified managed record when the build succeeds.

### Stale classification

Before launching a managed DMB, Meridian-MCP re-hashes all recorded inputs and outputs. The artifact
is stale when:

- Any recorded input is missing, added under a required fixture role, or hash-mismatched.
- The fixture manifest hash changed.
- A required generated proc or exact token is absent.
- The DMB or RSC is missing or differs from the successful output hash.
- A later compile attempt for the same artifact failed.
- The latest successful compile record was produced from a different canonical project identity.

A failed compile never deletes or replaces the prior DMB. It atomically appends a failed-attempt
record that marks the prior managed success stale. The failure response reports the retained output
hash and `dmb_updated: false`, but later managed launch returns `stale_build_artifact` with the exact
changed roles and paths.

A subsequent successful verified compile supersedes the stale marker atomically. An MCP restart
reloads the durable state and preserves the same decision.

### Fixture synchronization tool

`dm_check_fixture_sync` is a read-only analysis tool accepting a contained fixture manifest path. It
returns:

- Manifest and input hashes.
- Missing or changed files by role.
- Required generated procs that are missing or have incompatible arguments.
- Missing exact protocol/build tokens.
- Expected artifact paths and any matching durable build-record status.
- `verified`, `stale`, or `invalid` classification with stable reason codes.

The tool uses the active analysis snapshot when it matches the fixture DME. Otherwise it performs a
bounded fixture-only parse without replacing server analysis state. It never runs generators,
compilers, native modules, or services.

## Standard-runtime integrity

### Session state

Every standard `dm_run` captures an integrity baseline before spawning DreamDaemon. The protected
root is the matching active project root. If no matching project exists, it is the canonical DMB
parent. The launch result reports the selected root and integrity mode.

Standard `RuntimeState` gains:

- Session ID and managed/unmanaged build identity.
- Integrity baseline and durable journal handle.
- Exact MCP-owned paths.
- Timestamped output observations with monotonically increasing sequence numbers.
- First-observed mutation records.
- Monitor stop signal and task.

The standard journal is stored under the private state directory, not the game workspace. It is
created before DreamDaemon starts and finalized only after the owned process tree is stopped and the
final checkpoint completes.

### Baseline and mutation identity

Existing dirty state is recorded and allowed. A file that was dirty before launch remains protected:
changing it again during the session is a new mutation relative to its baseline content.

For Git workspaces the baseline records tracked path state, Git object identity where available,
working-tree content SHA-256, size, and preexisting porcelain status. Non-Git roots use the existing
bounded file-manifest fallback. Scope-limit failure prevents launch rather than running without an
integrity baseline.

The monitor uses bounded periodic Git status checks and hashes only paths whose tracked state changed
since the prior observation. The fixed default interval is five seconds. It records the first
observation time, prior and current identities, process/session identity, and the nearest preceding
runtime output sequence and line. The interval is centrally bounded and is not caller-configurable in
the first implementation.

### Checkpoints and results

Checkpoints occur:

1. Before process spawn.
2. After launch readiness succeeds or fails.
3. When a natural process exit is observed.
4. Before and after explicit stop.
5. During final journal recovery on a later server start.

`dm_status`, `dm_wait_for_output`, and `dm_stop` surface all observed integrity events. A tracked
mutation produces a structured `source_integrity_warning`; it does not make successful process
termination fail. A tracked deletion or an attempted cleanup of an unowned path remains the stronger
`workspace_integrity_violation` classification.

Each event reports:

- Relative path and tracked status.
- Change kind: added, modified, deleted, or renamed.
- Baseline and observed Git object identity when available.
- Baseline and observed SHA-256 and size.
- First-observed monotonic session offset.
- Nearest preceding output marker and sequence.
- Runtime kind, session ID, DMB identity, and owned process identity.

Meridian-MCP never rewrites, checks out, restores, deletes, or stages the changed file. Owned output
exceptions remain exact registered paths and cannot be directories, globs, or paths supplied after a
mutation is observed.

### Natural exit and recovery

Runtime tools refresh the child state and finalize a naturally exited session before returning.
Transport loss or host termination may leave an active journal. On the next startup,
`dm_server_status` reports `recovery_required`; a bounded recovery pass compares the recorded
baseline with current state and finalizes the journal as `recovered_with_changes` or
`recovered_clean`. Recovery does not infer which process changed files after the prior MCP process
ended.

## Native evidence ingestion

### Tools and artifact descriptors

Two read-only tools are added:

- `dm_native_evidence_summary` validates and summarizes one run's evidence.
- `dm_native_evidence_compare` validates two or more summaries from identity-compatible runs and
  reports matched metric deltas and repeated-run distributions.

The summary tool accepts an explicit bounded list of artifact descriptors. Supported kinds are:

- `byond_proc_profile_json`
- `byond_sendmaps_json`
- `performance_csv`
- `runtime_jsonl`
- `event_jsonl`

There is no format autodetection. Every descriptor contains a path, kind, and only the fixed options
defined for that kind. `event_jsonl` may declare simple dotted field names for timestamp, event name,
phase, numeric metrics, grouping fields, and additional identifier fields. Array traversal,
wildcards, expressions, transformations, and executable parser hooks are not supported.

All paths pass the existing containment policy. Readers stream input where practical and enforce
central limits for artifact count, bytes per artifact, total bytes, rows, columns, JSON depth, line
length, string length, unique groups, selected metrics, and returned records. Limit failures return
`evidence_limit_exceeded` with partial counts but no partial statistical claim.

### Run identity

Each summary binds:

- Every artifact's relative path, byte size, and SHA-256.
- Managed build record and fixture manifest identities when a DMB path is supplied.
- DMB, RSC, native module, service executable, compiler, BYOND, and MCP identities available from
  that record.
- Caller-supplied bounded workload fields such as map, seed, configuration profile, scenario, and
  external run ID.
- Identity verification status and missing dimensions.

Caller-supplied identity fields supplement recorded technical identity and cannot replace or
override it. A summary without a verified managed build may be inspected but reports
`identity_verification: unavailable` and cannot enter a verified comparison.

### Time and phase model

The normalized timeline keeps wall time, BYOND world time, and artifact-local sample indexes as
separate domains. It never treats one domain as another without an explicit anchor.

The caller may provide bounded named phases with:

- Phase name.
- Wall-time half-open range.
- Optional BYOND world-time half-open range.
- Optional source marker identity.

Structured logs and event records may contribute explicit anchors when their descriptor maps both a
wall timestamp and world time. Conflicting anchors return `timeline_conflict`; the tool does not pick
one silently.

Each artifact is classified as `cumulative_snapshot`, `interval_series`, or `event_stream` according
to its fixed format semantics and observed fields. BYOND proc and sendmaps profile snapshots record
their latest represented timestamp when available. If that timestamp precedes the declared game-start
phase, the result labels the data `pre_game_cumulative` and excludes it from later live-phase claims.

Rows and events are assigned only to phases whose explicit ranges contain them. Unaligned data is
retained in counts under `unassigned`; it is not stretched or proportionally assigned.

### Redaction

Raw artifacts remain unchanged. Returned records and grouping keys redact identifier fields by
default. The fixed protected-name set includes common player, client, account, key, ckey, mob, and
external-chat identifier fields. Event descriptors may add protected field names but may not disable
the fixed set.

Protected values are replaced with `<redacted>` and are excluded from group keys. Fixed bounded
sanitizers also remove explicit `key=value` forms for protected names from returned free-text samples.
The response reports which fields and how many values were redacted. It states that redaction is
best-effort and that raw source artifacts may still contain identifiers.

### Summaries

For selected numeric performance columns, the tool reports count, missing count, minimum, maximum,
mean, sample standard deviation, and deterministic type-7 p50, p95, and p99 values for each named
phase and the full represented interval.

Runtime JSONL is grouped by a normalized technical signature composed from available category,
exception/proc, source file, source line, and redacted message template fields. Event JSONL is grouped
only by explicitly selected non-protected fields. Results report raw, accepted, rejected, redacted,
assigned, and unassigned record counts.

Proc and sendmaps profiles report cumulative call/sample counts, total and average duration fields
that are actually present, represented time range, cumulative classification, and the highest bounded
contributors. Missing fields remain absent and are listed under `unavailable_metrics`; they are never
derived from unrelated counters.

### Comparison

`dm_native_evidence_compare` accepts between two and twenty evidence requests using the same bounded
descriptor and phase schema. It recomputes artifact hashes and summaries rather than trusting a
caller-edited prior response.

Verified comparison requires equality of all available technical build dimensions: DMB, RSC,
declared native modules, service executable, fixture manifest, BYOND version, and relevant workload
fields. Different run IDs and artifact hashes are expected. A mismatch returns
`evidence_identity_mismatch` before calculating deltas.

Comparisons match format kind, phase name, metric name, unit, cumulative/interval classification, and
technical group key. They report per-run values, absolute and percentage deltas where defined,
sample counts, minimum, maximum, mean, sample standard deviation, and coefficient of variation for
three or more runs. Cumulative snapshots are never compared as interval rates unless the input format
contains explicit start and end snapshots for the same run.

## Error model

New stable error or warning codes include:

- `repository_worktree_invalid`
- `path_outside_workspace`
- `symbol_not_found`
- `symbol_index_inconsistent`
- `fixture_manifest_invalid`
- `fixture_out_of_sync`
- `build_provenance_unavailable`
- `stale_build_artifact`
- `integrity_scope_too_large`
- `source_integrity_warning`
- `workspace_integrity_violation`
- `integrity_recovery_required`
- `evidence_format_invalid`
- `evidence_limit_exceeded`
- `timeline_conflict`
- `evidence_identity_mismatch`

Warnings appear in successful domain results and remain distinguishable from tool errors. A
successful `dm_stop` with a game-authored tracked mutation therefore reports process termination
success plus `source_integrity_warning`. A stale managed DMB is a launch error because executing the
wrong program would invalidate all later evidence.

## Security and privacy

- Startup configuration remains the only authorization boundary.
- Linked-worktree discovery uses fixed Git subcommands, no shell, no caller arguments, and no remote
  operations.
- Private state records are atomic, size-bounded, schema-versioned, and never executed.
- Manifest paths are contained and declarative. They cannot name commands, URLs, arguments, or
  environment variables.
- Artifact revalidation happens immediately before process spawn to reduce time-of-check/time-of-use
  drift.
- Native evidence parsers stream bounded local files and never fetch URLs or load parser plugins.
- Raw evidence is never copied, rewritten, deleted, or uploaded.
- Default redaction cannot be disabled through a tool call. Results disclose its best-effort scope.
- Machine-local state paths and repository-identity digests stay out of public fixtures and generated
  documentation examples.

These controls do not make untrusted source, DMBs, native modules, logs, or profiles safe. Development
mode remains appropriate only for trusted workspaces and artifacts.

## Documentation

The implementation updates:

- README capability and individual-tool descriptions.
- `docs/architecture.md` for effective roots, private state, proc resolution, provenance, standard
  integrity, and native evidence flow.
- `docs/security.md` for repository worktree authorization, state storage, managed launch gates, and
  evidence privacy.
- Generated tool reference and checked-in contract registry.
- Configuration examples for Windows and Ubuntu.
- Compatibility matrices distinguishing fixture verification from live BYOND evidence.
- Agent guidance to parse before exact lookup, inspect server status on path failures, require fixture
  sync for generated/native fixtures, and never treat unverified or cumulative pre-game data as live
  performance evidence.

Examples use logical paths and synthetic identifiers. They contain no account names, profile
segments, private roots, or raw playtest records.

## Verification

### Plan 1 acceptance

- Primary checkout and explicitly configured linked worktree parse in one server session.
- An unrelated checkout returns `path_outside_workspace` with effective policy source and roots.
- Worktrees created after startup remain unavailable until restart.
- A parent declaration plus child override fixture makes all analysis tools agree on the child
  implementation owner and parent declaration owner.
- Inheritance fallback and missing symbols return bounded resolution diagnostics.

### Plan 2 acceptance

- A matching parsed environment plus successful compile creates a durable verified build record.
- Changing source, generated bindings, native modules, service executable, or fixture manifest makes
  the managed DMB stale.
- A failed compile leaves the old DMB untouched, records the failed attempt, and prevents managed
  launch after restart.
- `dm_check_fixture_sync` reports a missing generated proc before compilation or launch.
- Restoring inputs and completing a fresh verified compile permits launch.
- An unmanaged human-built DMB launches with an explicit unverified warning and is rejected when
  `require_verified_provenance` is true.

### Plan 3 acceptance

- A fixture intentionally modifies a tracked file during `dm_run`.
- `dm_stop` terminates the process and reports the path, before/after identities, first observation,
  closest output marker, session, and process identity.
- A preexisting dirty file changed again during runtime is detected relative to its launch baseline.
- No file is reverted, deleted, restored, or staged by the MCP.
- Clean standard runs finalize their journal with no workspace delta.
- An interrupted journal is reported and safely recovered after restart.

### Plan 4 acceptance

- Fixtures cover every supported format with LF and CRLF where the format permits both.
- Startup proc profiles whose represented timestamp ends before game start are classified
  `pre_game_cumulative`.
- Performance CSV returns deterministic phase-specific percentile tables.
- Runtime JSONL groups stable technical signatures and reports malformed records.
- Event JSONL aligns named phases without returning protected player identifiers.
- Missing metrics remain explicit rather than being synthesized.
- Different verified build identities are rejected before comparison.
- Readers reject oversized, deeply nested, overlong, or excessive-cardinality inputs on Windows and
  Ubuntu.

### Plan 5 acceptance

- The exact repository-pinned Rust compiler version is printed before verification.
- `cargo fmt --all -- --check` passes.
- `cargo clippy --locked --all-targets --all-features -- -D warnings` passes.
- `cargo test --locked --all-features` passes on Windows and Ubuntu.
- Dependency-policy, generated-documentation, workflow-contract, and `git diff --check` gates pass.
- PowerShell-only BYOND integration compiles, registers, stale-checks, launches, mutates a fixture,
  reports integrity, and stops on the supported Windows BYOND baseline.
- Ubuntu parser, policy, manifest, state, integrity, and evidence-reader gates pass independently of
  unavailable live BYOND features.
- Compatibility documentation distinguishes Rust/fixture verification, Windows live BYOND
  verification, and Ubuntu live BYOND verification.

## Completion criteria

MMCP-PROF-017 and MMCP-PROF-018 close when their Rust and cross-tool consistency fixtures pass.
MMCP-PROF-019 and MMCP-PROF-021 close only after the Windows live managed compile/run fixture passes
with a clean final process state and the expected integrity/provenance decisions. MMCP-PROF-020 closes
for artifact compatibility when every reader and comparison fixture passes on Windows and Ubuntu;
production performance conclusions remain separate and require representative repeated-run evidence.
