# Meridian-MCP

> DreamMaker and SS13 MCP server for code search, diagnostics, maps, DMI analysis, controlled BYOND builds, debugging, and Tracy profiling.

Meridian-MCP gives MCP clients structured access to large DreamMaker repositories. It combines SpacemanDMM-backed source analysis with tightly contained development operations, so an agent can move from repository-scale discovery to exact symbol inspection, compiler evidence, runtime debugging, and performance analysis without receiving a general shell or arbitrary filesystem access.

Analysis mode is the default and is read-only. Development mode is an explicit startup choice that adds bounded compiler, generated-output, DreamDaemon, auxtools, and Tracy operations. Neither mode replaces the target repository's own build, test, or release process.

## What it is for

- Navigate large SS13 codebases through ranked context search, exact type/proc/var lookup, definitions, implementations, references, document symbols, and DreamChecker diagnostics.
- Inspect DMI metadata and pixels, extract states, find cross-file exact and common lazy-change duplicates, audit static icon references, and compare images without modifying source assets.
- Inspect, diff, search, and render DMM/TGM maps through the pinned SpacemanDMM implementation.
- Run a direct DreamMaker compile or Meridian-Rift's separate agent-owned full-build wrapper with bounded output and local provenance records.
- Start and control one owned loopback BYOND runtime, exercise project-provided `world.Topic()` test hooks, debug through auxtools, or capture and analyze Tracy profiles.
- Produce hash-bound native evidence summaries and comparisons while keeping raw profiling artifacts local.

Meridian-MCP is intended for repository analysis and engineering support. It is not a general command runner, a source-control client, an asset editor, or an autonomous substitute for human review.

## Authority and safety boundaries

The tools report different kinds of evidence; they do not make them interchangeable:

1. SpacemanDMM parsing and DreamChecker establish parser and static-analysis results.
2. DreamMaker establishes whether the `.dme` compiles.
3. A repository's documented build and test entry points establish project acceptance.
4. BYOND runtime behavior establishes what actually happens in a running world.

A green result at one layer is useful evidence, not proof that the later layers pass. `dm_compile` invokes DreamMaker directly and is not a tgstation-style full build. Meridian-Rift's `rift_compile` uses the contained `RIFT_BUILD.cmd`; it deliberately does not alter or replace the human-authored `BUILD.cmd`.

All paths are checked against immutable startup roots. Development executables are allowlisted at startup, runtime endpoints are loopback-only, and active processes must be owned by the current server. Managed build records detect stale inputs and outputs before launch. Runtime-integrity journals report workspace mutations but never revert, repair, stage, delete, or rewrite source files.

## Five-minute analysis setup

This is the smallest useful local setup for Codex on Windows. It provides source, DMI, and map analysis; it does not authorize compilation or runtime control.

1. Build the release binary with the repository-pinned Rust toolchain:

   ```powershell
   cargo +1.95.0 build --locked --release
   ```

2. Add a server entry to the existing Codex configuration. Replace the two example paths with absolute local paths:

   ```toml
   [mcp_servers.meridian-mcp]
   command = 'C:\path\to\meridian-mcp\target\release\meridian-mcp.exe'

   [mcp_servers.meridian-mcp.env]
   MERIDIAN_MCP_MODE = 'analysis'
   MERIDIAN_MCP_ROOTS = 'C:\path\to\Meridian-Rift'
   ```

3. Restart Codex, call `dm_server_status`, then call `dm_parse_environment` with the contained `.dme` path. Parse again after source changes; an unchanged environment reuses the active snapshot rather than reparsing.

Analysis mode needs no BYOND installation. `dm_generate_docs` is advertised only when an exact, hash-verified dmdoc helper and manifest are also packaged and configured.

## Development setup

Development mode should use the repository's installer and configuration updater because they validate the binary, helper manifest, auxtools DLL, authorized roots, and private state layout as one unit. Prerequisites depend on the tools you enable:

| Capability | Additional prerequisite |
| --- | --- |
| Direct compilation and normal runtime | An allowlisted BYOND installation containing `dm.exe` and its sibling runtime executables |
| Meridian-Rift full build | Windows, an authorized Meridian-Rift checkout, and `MERIDIAN_MCP_RIFT_BUILD=offline` or `network` |
| HTML documentation | Exact dmdoc helper recorded by the helper manifest |
| Auxtools debugging | Packaged auxtools v2.3.7 DLL and the x86 Microsoft Visual C++ runtime |
| Tracy profiling | Manifest-verified host helper and x86 byond-tracy hook for the supported BYOND baseline |

The complete packaging example is in [Operator and contributor reference](#operator-and-contributor-reference). At minimum, development configuration sets `MERIDIAN_MCP_MODE=development`, supplies one or more roots, allowlists the compiler, and points `MERIDIAN_MCP_STATE_DIR` to an existing writable directory outside every workspace root. Restart the MCP client after changing startup authorization.

## Common workflows

### Analyze code

1. `dm_parse_environment` loads one contained `.dme` into an atomic analysis snapshot.
2. `dm_search_context` performs repository-scale discovery. `dm_search_symbols` is useful for direct partial-name lookup.
3. Verify candidates with `dm_get_type`, `dm_get_proc`, `dm_get_var`, or `dm_get_definition`.
4. Use `dm_find_references`, `dm_find_implementations`, `dm_document_symbols`, and `dm_check_errors` for impact analysis.
5. Reparse after any source change. Parser success is not compiler success.

Reparsing is cheap when nothing changed. `dm_parse_environment` records a metadata fingerprint for every file registered by SpacemanDMM, plus the environment and configuration inputs. A repeated call whose paths, lengths, and settled modification times still match returns the active snapshot with `reused: true` and an unchanged `state_generation`; it does not hash file contents or promise byte-for-byte identity. On a station-sized environment of roughly 10,000 files and 65,000 types this is the difference between about 35 seconds and under half a second, so calling it defensively before an analysis sequence is inexpensive. A missing input, changed metadata, or file modified within two seconds of the check fails reuse closed and triggers a full parse. Pass `force: true` to reparse regardless.

The parse response reports cold build stages under `timings_ms`; a reused response reports only queue wait, reuse validation, and total time. Its `retrieval` object identifies the ready lexical BM25 document count, semantic chunk schema version, and `dense.status = not_configured`. The chunk schema provides stable identities for a future vector generation, but Meridian-MCP does not currently create embeddings, an ANN index, or a vector database. `dm_search_context` is candidate-bounded lexical retrieval. Use exact definition, reference, implementation, and symbol tools when the requested identity is already known.

Procedure results distinguish the implementation owner, which supplies the nearest executable body, from the declaration owner, which supplies declaration metadata. Exact lookup, definitions, searches, document symbols, and implementation queries use the same snapshot-owned resolver.

`dm_check_errors` reads diagnostics already produced by the latest successful `dm_parse_environment`; it does not rerun the parser or DreamChecker. The response identifies that cached analysis generation, summarizes every matching diagnostic by severity, component, rule, and configuration provenance, and returns at most 50 diagnostic records by default. Follow `pagination.next_cursor` with the same filters and state generation for later pages. `file_path`, `severity`, `component`, `rule`, and `configured` narrow both the summary and page. A response with `truncated: true` and `diagnostic_page_limit` is a normal paginated result, not lost server output.

### Compile and exercise a world

Use `dm_compile` for a direct compiler gate. Use `rift_compile` only for the contained Meridian-Rift full-build workflow. A successful managed compile records the compiler, repository identity, parsed source closure, fixture inputs, and DMB/RSC hashes. A later failed compile or changed input/output makes that managed artifact stale.

After a successful build, `dm_run` starts one owned loopback DreamDaemon. Use `dm_wait_for_output` for readiness, `dm_topic` for a project-provided test endpoint, `dm_status` for process and integrity evidence, and `dm_stop` for cleanup. Unmanaged human-built DMBs are reported as `unverified` unless verified provenance is required explicitly.

### Inspect icons and maps

Use `dm_dmi_info` before comparing or extracting DMI states. `dm_find_dmi_duplicates` looks across contained scopes for exact matches plus cropped, palette-only, mirrored, rotated, and scaled copies. `dm_audit_icons` checks statically resolvable inherited `icon` and `icon_state` references. Generated PNGs are written only by the explicit development tools and only to contained paths.

For maps, use `dm_map_info` for structure, `dm_find_on_map` for type-path occurrences, `dm_diff_maps` for coordinate-model differences, and `dm_list_render_passes` before `dm_render_map` or `dm_render_maps`.

### Debug with auxtools

Compile and parse matching source, launch the contained DMB with `dm_debug_launch`, configure breakpoints, continue or step, and inspect threads, frames, scopes, and variables after a stop event. The adapter owns one Windows BYOND host and does not attach to arbitrary processes. See the [detailed debugger workflow](#debugger-workflow) and the per-tool descriptions below.

### Profile with Tracy

Prepare the verified hook, launch an owned profiling runtime, capture one or more bounded windows, and inspect hotspots, zones, frames, comparisons, and repeated controls offline. Each accepted trace is paired with an identity and queue-health sidecar. See the [Tracy profiler](#tracy-profiler) reference and [native evidence analysis](docs/native-evidence.md).

## Capability and platform status

| Area | Status | Notes |
| --- | --- | --- |
| Parse, lookup, definitions, and search | Provisional | Unit-tested with purpose-written fixtures; the named matrix separately verifies freshly built MCP/SpacemanDMM parsing above 64K declared leaves on Windows and Ubuntu. |
| DreamChecker diagnostics | Provisional | SpacemanDMM analysis, not DreamMaker acceptance. |
| DMI profiling, extraction, duplicate detection, and icon audit | Experimental | Pixel/metadata analysis is bounded and non-mutating; hotspot parsing remains incomplete upstream. |
| DMM/TGM information, differences, search, and rendering | Provisional | Parser-backed dimensions, models, coordinates, render passes, bounds, and typed batches. |
| HTML documentation | Experimental | Exact-revision, hash-verified dmdoc helper; unavailable unless packaged at startup. |
| Auxtools debugger | Experimental | Windows development-mode opt-in; one MCP-owned DreamSeeker session, no attach/restart/disassembly. |
| Tracy profiling | Experimental | Explicit development-mode opt-in; pinned native hook and fixed-command helper, MCP-owned loopback runtime, bounded capture, and offline trace analysis. |
| DreamMaker compilation | Provisional | A direct compiler gate only; not `BUILD.cmd` or a tgstation full build. |
| Meridian-Rift full build | Provisional | Windows-only `rift_compile` through the contained `RIFT_BUILD.cmd`; promotion awaits a recorded green named integration run. |
| PNG rendering, DreamDaemon, and `Topic()` | Provisional | Development mode with containment, loopback, and process-ownership controls. |
| Inherited BYOND client login protocol | Unsupported | Removed because provenance and compatibility evidence were insufficient. |

Support labels are defined in [compatibility](docs/compatibility.md). A passing SpacemanDMM check is useful evidence, not proof that DreamMaker or a repository's full validation suite passes.

| Platform | Status |
| --- | --- |
| Windows | Verified only for evidence listed in [compatibility](docs/compatibility.md) |
| Linux | Provisional Ubuntu 24.04 gates for Rust, release stdio MCP, over-64K parser compatibility, required synthetic BYOND startup, and independent live Tracy evidence |
| macOS | Unsupported and untested |

## Complete tool reference

Analysis mode exposes the read-only tools below. Development mode adds the active compiler, rendering, documentation, and runtime tools. `rift_compile`, the auxtools debugger, and Tracy profiling have separate immutable startup gates; gated tools are not advertised or callable when their prerequisites are absent. See [tool contracts](docs/tool-contracts.md) for the generated capability mode, support level, side effects, timeout, and output limit of every tool.

### Analysis mode

| Tool | Description |
| --- | --- |
| `dm_server_status` | Report the MCP build identity, capability mode, optional startup gates, `immutable_startup_roots` containment policy, effective roots and authorization sources, compiler allowlist, active analysis generation, and owned runtime summary. It is read-only and does not require a parsed environment. |
| `dm_parse_environment` | Parse a contained `.dme` file and atomically replace the cached object tree and lexical search index. Returns counts, diagnostics, cold/reuse stage timings, the pinned SpacemanDMM revision, and honest lexical/dense readiness. Reuse requires matching path/length/settled-mtime metadata for every parser-registered input; it is not a content hash. Optional `force` reparses anyway, and `timeout_ms` is bounded to 1-1,800,000 ms. Parses are serialized, so overlapping calls queue rather than building two object trees at once. |
| `dm_check_fixture_sync` | Validate a contained declarative fixture manifest against exact parsed proc signatures, required tokens in declared text inputs, and any available managed build record. Returns `verified`, `stale`, or `invalid` without compiling or modifying the fixture. |
| `dm_get_type` | Inspect an exact DreamMaker type path. Returns its documentation, source location, parent and child types, and variable and procedure metadata, including which members are declared on that type. |
| `dm_get_proc` | Inspect an exact procedure on a type. Returns its requested type, implementation owner, declaration owner, local-or-inherited resolution kind, and every parsed implementation with parameters, documentation, source location, and a bounded source excerpt. |
| `dm_get_var` | Inspect an exact variable on a type. Returns its declared type, constant value, whether it has an initial expression, documentation, and source location. |
| `dm_list_types` | Enumerate parsed type paths, optionally restricted by a type-path prefix and maximum traversal depth. |
| `dm_search_symbols` | Find type, procedure, and variable names by case-insensitive partial match, with an optional symbol-kind filter and result limit. |
| `dm_search_context` | Run deterministic candidate-bounded lexical BM25 retrieval across parsed symbols, documentation, parameters, source paths, and source text. Supports type, type-prefix, and file filters, reports candidate/scoring counts, and can return bounded source excerpts. Verify candidates with the exact inspection tools; no dense or vector retrieval is configured. |
| `dm_check_errors` | Read a bounded page of parser and DreamChecker diagnostics cached by the latest successful parse. Returns full matching summary counts, normalized file/severity/component/rule/configuration filters, an opaque continuation cursor, and the analysis generation. It does not rerun analysis and is not a DreamMaker compile result. |
| `dm_get_definition` | Resolve an exact type, variable, or procedure to its parsed definition and source location. |
| `dm_document_symbols` | List nested type, variable, and procedure declarations in one parsed source file. |
| `dm_find_references` | Find semantic read/write references to an exact canonical declaration, excluding shadowed locals and unresolved dynamic accesses. |
| `dm_find_implementations` | List parent/child implementations for an exact type, variable, or procedure identity in stable order. |
| `dm_dmi_info` | Profile DMI metadata, states, directions, frames, dimensions, hashes, and parser warnings without changing the asset. |
| `dm_compare_dmi_states` | Compare two complete DMI states for exact, cropped, palette-only, mirrored, rotated, and scaled lazy changes. |
| `dm_find_dmi_duplicates` | Scan bounded contained DMI scopes, bucket candidates, and cluster cross-file exact or lazy-change duplicates. |
| `dm_audit_icons` | Resolve static inherited `icon`/`icon_state` references, report missing evidence and dynamic expressions, and optionally estimate unused states. |
| `dm_map_info` | Parse a contained DMM/TGM map and report its format, grid dimensions, unique-tile count, file size, and most frequent base types and areas. |
| `dm_diff_maps` | Compare coordinate models across two DMM/TGM maps independently of dictionary keys, with bounded structured differences. |
| `dm_list_render_passes` | List every render pass from the pinned SpacemanDMM revision, including its default state and description. |
| `dm_find_on_map` | Find occurrences of a requested type path and its descendants in a contained DMM/TGM map. Returns BYOND coordinates, exact matched type paths, and tile keys. |
| `dm_native_evidence_summary` | Read explicit bounded BYOND/native artifact kinds, preserve separate clock domains, apply mandatory identifier redaction, assign only unambiguous half-open phases, and return hash-bound descriptive statistics. Raw artifacts remain unchanged and local. |
| `dm_native_evidence_compare` | Re-read 2-20 complete evidence requests, require the same verified managed build and workload identity before statistics, and return matched metric deltas plus repeated-run distributions. Edited summaries are not accepted. |

### Development mode

| Tool | Description |
| --- | --- |
| `dm_compile` | Run an allowlisted DreamMaker compiler directly against a contained `.dme` file. Supports an approved compiler path, working directory, preprocessor defines, total and idle timeouts, an optional fixture manifest, and optional best-effort process-tree endpoint observation. Returns bounded output, structured diagnostics, artifact evidence, and build provenance. It is not a repository full build. |
| `rift_compile` | On Windows, run the active qualified Meridian-Rift checkout's fixed `RIFT_BUILD.cmd` full-build wrapper. It accepts no paths, commands, URLs, targets, credentials, or arbitrary environment values. Availability requires development mode plus an explicit startup ceiling; results distinguish fresh artifacts, valid cache hits, failures, and insufficient evidence. |
| `dm_render_map` | Render one z-level of a contained DMM/TGM map to a PNG using the parsed environment. The output must remain inside an allowed root, and an existing file is replaced only when `overwrite` is explicitly enabled. |
| `dm_render_maps` | Preflight and render a bounded typed batch of contained map chunks in request order; it accepts no raw RenderMany command. |
| `dm_extract_dmi` | Mechanically extract one frame/direction or a contact sheet from a selected DMI state to an atomic contained PNG. |
| `dm_generate_docs` | Generate contained HTML from the active environment using only the packaged, exact-revision, hash-verified dmdoc helper. |
| `dm_run` | Start one server-owned DreamDaemon process for a contained `.dmb` on loopback. Supports a port, working directory, additional daemon arguments, an optional literal or regular-expression readiness marker, and `require_verified_provenance`. Known stale managed artifacts are always refused. In configured development sessions it persists a workspace-integrity baseline before spawn. |
| `dm_wait_for_output` | Wait for a literal or regular-expression marker in the bounded output retained from the server-owned DreamDaemon process. Reports matches, timeouts, process exit, recent timestamped output, and the current or finalized integrity summary. |
| `dm_status` | Report whether the server-owned DreamDaemon process is running, its launch provenance, and current integrity evidence. A live process includes its PID and port; stopped state includes the last exit code and finalizes a natural process exit. |
| `dm_stop` | Stop and clean up the DreamDaemon process owned by this Meridian-MCP server before final integrity evaluation. It does not target unrelated system processes and never reverts a workspace mutation. |
| `dm_topic` | Send a bounded `world.Topic()` request to the running loopback DreamDaemon process and return the decoded response. This is intended for project-provided debug and test handlers. |

### Auxtools debugger

When `MERIDIAN_MCP_DEBUGGER=auxtools` is enabled under development mode and the fixed DLL installation validates, Meridian-MCP exposes a restricted debugger adapter over the pinned auxtools protocol. The Windows debug server is 32-bit and requires the x86 Microsoft Visual C++ runtime even when Meridian-MCP runs as a 64-bit process; CI installs and verifies that prerequisite before launching the selected BYOND host.

| Tool | Description |
| --- | --- |
| `dm_debug_launch` | Launch one contained DMB through a BYOND executable beside the single allowlisted `dm.exe`. `host_mode: "interactive"` (the default) uses `dreamseeker.exe` for developer sessions; `host_mode: "headless"` uses `dreamdaemon.exe` for non-desktop environments such as CI. Meridian-MCP injects only the fixed hash-verified debugger DLL, opens an ephemeral loopback listener, owns the process tree, retains the debugger-provided `stddef.dm`, and refuses to start while a normal or Tracy DreamDaemon runtime is active. |
| `dm_debug_set_breakpoints` | Replace the complete source-breakpoint set with lines from one contained file in the active parsed generation. Each line must resolve inside a parsed procedure; optional bounded conditions are passed to auxtools. Reparse invalidates this source mapping, so stop and relaunch the debugger after source changes. |
| `dm_debug_set_function_breakpoints` | Replace the complete breakpoint set using canonical DreamMaker procedure paths, with optional override identifiers, instruction offsets, and bounded conditions. Use this when the exact procedure identity is known or source-line mapping is inappropriate. |
| `dm_debug_set_exception_breakpoints` | Enable or disable breaking when DreamMaker reports a runtime exception. This controls only the active owned debugger session. |
| `dm_debug_control` | Pause, continue, step into, step over, or step out of the active debuggee. Step actions use a debugger-issued thread identifier; actions are a fixed enum rather than arbitrary protocol requests. |
| `dm_debug_threads` | List the active debuggee's bounded thread inventory and debugger-issued identifiers. Use the returned identifiers for stack and control operations. |
| `dm_debug_stack_trace` | Read a bounded page of frames for one debugger-issued thread identifier. Frames include procedure identity and source information when the parsed snapshot can resolve it. |
| `dm_debug_scopes` | Return argument, local, and global variable references for one debugger-issued frame identifier. The references are valid only for the active session. |
| `dm_debug_variables` | Read one bounded page of values from a debugger-issued variables reference. Nested values may return further references for subsequent calls. |
| `dm_debug_evaluate` | Evaluate a bounded DreamMaker expression in an optional frame using the `watch`, `repl`, or `hover` context. Evaluation executes inside the active debuggee and can have game-state side effects; do not use it as a read-only query for untrusted expressions. |
| `dm_debug_exception_info` | Return the most recently retained runtime-exception message and current event sequence from the active session. It is not a historical exception log. |
| `dm_debug_source` | Read the retained debugger-provided `stddef.dm` only through source reference `1`, which is issued by the active adapter. It accepts no caller-selected file path or URL. |
| `dm_debug_wait_for_event` | Wait for the first bounded breakpoint, step, pause, runtime, output, or termination event after an optional sequence number. Calls can filter event kinds and wait for at most 300 seconds; results report queue eviction when older events were dropped. |
| `dm_debug_stop` | Disconnect the debugger and terminate only the selected BYOND host process tree owned by this Meridian-MCP session. It never detaches and never accepts a PID. |

#### Debugger workflow

1. Compile the target DMB and call `dm_parse_environment` for its matching DME before using source-oriented breakpoints.
2. Call `dm_debug_launch` with the contained DMB. Keep the default interactive host for normal debugging, or select `host_mode: "headless"` where no desktop session is available. Only one debugger session may exist, and it is mutually exclusive with standard and Tracy runtimes.
3. Configure source, function, and/or runtime-exception breakpoints. Each breakpoint-setting call replaces the relevant active set; send the complete desired list.
4. Use `dm_debug_control` to continue or step, then `dm_debug_wait_for_event` with the last observed sequence to avoid replaying an older event.
5. On a stop event, inspect `dm_debug_threads`, `dm_debug_stack_trace`, `dm_debug_scopes`, and `dm_debug_variables`. Use `dm_debug_exception_info` for the latest runtime and `dm_debug_source` only for an issued standard-definition reference.
6. Use `dm_debug_evaluate` only when executing that expression in the game is intentional.
7. Call `dm_debug_stop` before recompiling, reparsing changed source, starting DreamDaemon, or ending the client session.

The adapter does not support attaching to an existing process, selecting another DLL, choosing a non-loopback endpoint, restarting a debuggee in place, arbitrary DAP/auxtools passthrough, or legacy extools disassembly. Timeout, disconnect, MCP shutdown, and explicit stop clean up the owned process rather than leaving it detached. Auxtools remains Windows-only and experimental even when its live integration gate passes.

### Tracy profiler

When `MERIDIAN_MCP_TRACY=byond` is enabled under development mode and both native artifacts validate against the helper manifest, the server adds these tools. The current live baseline is Tracy protocol 82 and BYOND 516.1687; the pinned hook declares support for 516.1685-1687, but it remains experimental until the named live gates record green evidence.

| Tool | Description |
| --- | --- |
| `dm_tracy_prepare` | Copy the exact hash-verified x86 byond-tracy hook beside a contained DMB. A matching hook is idempotent; replacing a different file requires `overwrite=true`. |
| `dm_tracy_launch` | Require an existing contained experiment directory, create its durable integrity journal, start one MCP-owned DreamDaemon and persistent collector, validate producer readiness, and retain a drain worker on the private loopback profiler endpoint. `startup_timeout_ms` controls collector connection and producer-health readiness and defaults to 60 seconds. For a Meridian-Rift headless run, supply a contained, hash-bound `config_directory` with `RESUME_AFTER_INITIALIZATIONS`, leave `wake_sleeping_world=true`, and set `initialization_timeout_ms` high enough for the fixed initialization marker (default 180 seconds). The MCP then sends one post-initialization loopback Topic wake and records the result. Launch evidence includes the exact MCP build identity. |
| `dm_tracy_capture` | Use a bounded transient reconnect retry to rotate to a fresh capture worker, begin timing only after queue-health readiness, reopen and validate the trace, resume draining, and atomically publish `.tracy` plus `.tracy.meridian.json`. Invalid traces are retained only as non-authoritative diagnostics and never enter statistics. Network evidence is best effort and scoped to owned loopback observations. |
| `dm_tracy_status` | Report DreamDaemon and collector state, profiler endpoint, transition/worker purpose, retry count, queue and hook health, capture activity, integrity-journal state, and the last structured error. |
| `dm_tracy_stop` | Record shutdown integrity, cancel an active window, stop the persistent collector, then terminate only the MCP-owned Tracy DreamDaemon and finalize the journal after the final clean checkpoint. It does not target a standard runtime, unrelated process, or repair source changes. |
| `dm_tracy_hotspots` | Load a contained trace and return a bounded, deterministic proc/file/line ranking by inclusive time, self time, call count, or maximum duration. |
| `dm_tracy_zone` | Return bounded aggregate statistics for an exact profiled proc name across its recorded file/line identities. |
| `dm_tracy_frame_stats` | Summarize the trace's base `ServerTick` frame series with count, span, mean, extrema, and p50/p95/p99 durations. |
| `dm_tracy_compare` | Compare two contained traces by exact proc/file/line identity and return bounded inclusive, self-time, and count deltas. |
| `dm_tracy_control_stats` | Validate 3-20 immutable, identity-compatible capture pairs; aggregate a selected complete-frame or exact-zone percentile; and report deterministic sample deviation, coefficient of variation, range, fixed noise thresholds, and baseline eligibility. |

Offline analysis works without a parsed environment. When one is active, matching trace file/line records receive additive source-correlation metadata; profiler measurements are not rewritten.

Meridian-Rift sets `world.sleep_offline = TRUE` immediately before yielding for one tick, then reads `RESUME_AFTER_INITIALIZATIONS` after that yield. On a headless world with no clients, the flag alone cannot execute its own recovery line. Meridian-MCP therefore treats the flag as a prerequisite, not proof of progress: the default wake path waits for the initialization-complete log marker, waits beyond the sleep boundary, tries bounded Topic requests on the owned loopback game port, and requires continued profiler producer progress. If Topic can service requests but cannot resume the suspended scheduler, the MCP starts one fixed DreamSeeker sibling as a process-contained loopback guest and holds it until `dm_tracy_stop`. Its executable hash and process identity are evidence; it cannot target a caller-selected host. `runtime_wake` records both attempts and the accepted strategy in launch, status, capture, and final evidence. Set `wake_sleeping_world=false` only for a deliberate sleeping control.

## Operator and contributor reference

### Rust build and verification

The repository pins Rust 1.95.0 with rustfmt and Clippy to match CI. BYOND integration gates additionally require the project-pinned BYOND version.

```powershell
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.95.0 test --locked --all-features
cargo +1.95.0 build --locked --release
cargo +1.95.0 deny check
```

The release binary is `target\release\meridian-mcp.exe` on Windows and `target/release/meridian-mcp` on Linux.

### Repository command inventory

Run these PowerShell entry points from the repository root. Scripts that accept paths resolve and validate them before executing; integration scripts are evidence gates, not substitutes for a downstream repository's own build and test workflow.

| Command | Purpose |
| --- | --- |
| `test_mcp.ps1` | Build or exercise an installed Meridian-MCP binary over real stdio JSON-RPC. It validates initialization, exact tool inventories and schemas, bounded errors, optional DME parse/search, compile, runtime, Topic, and map behavior. Use `-SkipBuild` with `-BinaryPath`/`-ServerPath` to test an exact release artifact. |
| `test_parse.ps1` | Run the parse-focused subset of `test_mcp.ps1` for one required DME, with optional exact type, proc, and search probes. It preserves the selected debug/release configuration, binary override, timeout, and `-SkipBuild` behavior. |
| `scripts/audit-spacemandmm-capabilities.ps1` | Compare the checked-in SpacemanDMM capability registry with its declared coverage; `-Check` is the non-mutating CI audit. An exact upstream checkout can be supplied for source-aware auditing. |
| `scripts/build-spacemandmm-helpers.ps1` | Build dmdoc from the pinned local SpacemanDMM checkout, copy the platform helper, hash it, and write or merge the helper manifest. It does not select an arbitrary revision. |
| `scripts/build-tracy-helpers.ps1` | Copy exact clean Tracy/byond-tracy revisions into private build sources, verify and apply ordered clock/queue-health patches, build x64 helper and x86 hook, run all CTests, copy licenses, hash patches/artifacts, and merge schema-v2 manifest entries. It performs no source download or vendor-checkout edit. |
| `scripts/run-tracy-native-tests.ps1` | Run the complete pinned native build, protocol, query, validation, rotation, cancellation, and packaging gate on Windows or Ubuntu, reported independently. |
| `scripts/run-tracy-integration.ps1` | Compile the owned BYOND fixture, retain the drain worker for two minutes, run three 30-second MCP captures, verify trace/sidecar pairs and queue health, exercise every analysis command, and stop owned processes. |
| `scripts/run-tracy-experiment.ps1` | Run a named 3-20-control experiment through the release MCP, produce range-aware summaries and a complete experiment manifest, validate the pairs independently, and retain raw traces locally. `-OverwritePreparedHook` explicitly authorizes replacement of an existing adjacent `prof.dll` with the newly built, hash-verified hook. |
| `scripts/validate-tracy-evidence.ps1` | Rehash an existing experiment evidence directory and validate schema-2 identity, ranges, queue health, unique phase iterations, separate process-memory roles, network disclaimers, and control eligibility without launching BYOND. |
| `scripts/fetch-auxtools.ps1` | Download auxtools `debug_server.dll` v2.3.7 from the fixed release URL, verify its fixed SHA-256, and atomically install it below a supplied destination root. |
| `scripts/install-auxtools-runtime.ps1` | Verify the x86 MSVC runtime required by the pinned auxtools DLL and, on Windows CI, install it from the runner's bundled `vc_redist.x86.exe` when missing. It performs no network download. |
| `scripts/install-byond.ps1` | Install the pinned Windows BYOND archive for CI/integration use through verified download and archive checks. This is test infrastructure, not a project build command. |
| `scripts/install-byond-linux.ps1` | Install the pinned Linux BYOND archive for the Ubuntu live-integration job after verifying the exact archive hash and compiler artifact. |
| `scripts/install-byond-runtime.ps1` | Verify or provision the pinned x86 Windows runtime prerequisites used by BYOND integration. It checks the required VC runtime files and installs the fixed-hash application-local DirectX DLL; `-CheckOnly` performs no installation. |
| `scripts/install-meridian-mcp.ps1` | Atomically install a release binary, manifest-selected dmdoc/Tracy helpers, and the verified auxtools DLL into a destination root. Optional workspace/repository roots are validated and returned as configuration values; development mode creates only the exact private state directory after proving it is outside workspace roots. `-EnableTracy` requires both native Tracy manifest identities. The script does not edit Codex configuration. |
| `scripts/configure-codex-meridian-mcp.ps1` | Update one named Meridian-MCP server entry in an existing Codex TOML configuration. It preserves unrelated servers and environment keys while setting explicitly supplied workspace roots, repository roots, development state, helper manifest, and Tracy opt-in. All supplied directories must exist, and private state must be outside workspace roots. |
| `scripts/test-configure-codex-meridian-mcp.ps1` | Exercise configuration on private temporary TOML and directory fixtures, proving an unrelated server and unrelated selected-server key survive while root and state values round-trip. |
| `scripts/run-byond-integration.ps1` | Compile the owned BYOND fixtures used by the runtime integration gate. |
| `scripts/new-large-prototype-fixture.ps1` | Generate a temporary technical DreamMaker environment containing a requested number of unique prototype paths. Flat layout stresses the MCP parser; bucketed layout keeps each parent below DreamMaker's direct-child ceiling for BYOND runtime testing. It creates no game content. |
| `scripts/run-large-prototype-parser-integration.ps1` | Generate a bounded technical type corpus, parse it through the selected Meridian-MCP binary, resolve its first, boundary, and last declared type paths, and write parser/provenance evidence. It does not start BYOND. |
| `scripts/run-large-prototype-integration.ps1` | Compile and start a generated control or over-64K world with pinned BYOND, require its readiness marker, sample bounded process progress, classify failures, verify cleanup, and write machine-readable runtime evidence. |
| `scripts/run-auxtools-integration.ps1` | Compile and drive the small owned debugger fixture through auxtools launch, inventory/query, exception-breakpoint configuration, and clean stop, writing machine-readable evidence. `-DmbPath` remains available for an explicit full-game diagnostic, but CI uses the owned fixture so native protocol compatibility is not coupled to Meridian-Rift boot time. Meridian-Rift itself remains covered by the separate compile and analysis compatibility gates. |
| `scripts/run-meridian-analysis-compatibility.ps1` | Run the versioned read-only parse, lookup, definition, search, diagnostics, DMI, map, render, and documentation compatibility manifest against a real Meridian-Rift checkout. |
| `scripts/run-meridian-compatibility.ps1` | Run the named Windows Meridian-Rift compatibility sequence, including direct compile, network and offline `rift_compile`, the warm authoritative human build, negative-policy sessions, and evidence output. Use only with a disposable integration checkout as described in `TESTING.md`. |
| `scripts/run-provenance-integrity-integration.ps1` | Run the owned BYOND 516.1687 managed-build fixture: sync, compile, verified launch, tracked mutation reporting, stale rejection after source change and failed compile, persistence across restart, exact-byte restoration, fresh recompile, and clean stop. It writes bounded schema-1 evidence and never invokes a downstream human build script. |
| `scripts/test-provenance-evidence-validation.ps1` | Validate managed provenance evidence size, schema, and privacy boundaries and run built-in malicious-document rejection cases without launching BYOND. |
| `scripts/test-meridian-evidence-validation.ps1` | Parse and exercise the compatibility harness's evidence validator, proving legitimate null fields are accepted, forbidden sensitive keys are rejected, and failed-reparse assertions use structured error details. |
| `scripts/test_unsupported_rift_compile.ps1` | Verify the stable non-Windows `unsupported_platform` response without installing BYOND or invoking a Windows build wrapper. |

`scripts/MeridianMcpSession.psm1` is the shared stdio JSON-RPC session module used by integration scripts. `scripts/process-readiness.psm1` provides bounded process-readiness sampling, BYOND identity checks, and stable runtime classifications for the large-prototype gates. They are imported modules, not standalone commands. Files under `tests/fixtures/` are owned test inputs rather than operator entry points. Exact parameters, fixtures, destructive-gate warnings, and CI-equivalent invocations are documented in [TESTING.md](TESTING.md).

### Packaging and configuration example

```powershell
./scripts/build-spacemandmm-helpers.ps1 `
    -UpstreamPath C:\path\to\SpacemanDMM `
    -OutputDirectory ./target/package `
    -ManifestPath ./target/package/helpers/manifest.json

./scripts/fetch-auxtools.ps1 -DestinationRoot ./target/package

./scripts/install-meridian-mcp.ps1 `
    -BinaryPath ./target/release/meridian-mcp.exe `
    -HelperManifestPath ./target/package/helpers/manifest.json `
    -AuxtoolsRoot ./target/package `
    -DestinationRoot C:\path\to\installed-meridian-mcp `
    -InstalledName meridian-mcp.exe `
    -WorkspaceRoots C:\path\to\Meridian-Rift `
    -RepositoryRoots C:\path\to\Meridian-Rift `
    -StateDirectory C:\path\to\private-meridian-state `
    -Development

./scripts/configure-codex-meridian-mcp.ps1 `
    -ConfigPath C:\path\to\.codex\config.toml `
    -BinaryPath C:\path\to\installed-meridian-mcp\meridian-mcp.exe `
    -HelperManifestPath C:\path\to\installed-meridian-mcp\helpers\manifest.json `
    -WorkspaceRoots C:\path\to\Meridian-Rift `
    -RepositoryRoots C:\path\to\Meridian-Rift `
    -StateDirectory C:\path\to\private-meridian-state `
    -Development
```

Add `-EnableTracy` to both installation and configuration only when the combined manifest contains the verified Tracy helper and hook. Restart Codex after changing its MCP configuration.

### Startup configuration

The server reads immutable startup configuration:

- `MERIDIAN_MCP_MODE`: `analysis` (default) or `development`.
- `MERIDIAN_MCP_ROOTS`: semicolon-separated workspace roots on Windows; platform path-list syntax elsewhere.
- `MERIDIAN_MCP_REPOSITORIES`: optional path list of explicitly authorized local Git working trees. At startup, Meridian-MCP discovers and verifies their linked worktrees using fixed local Git commands, then adds those exact canonical paths to the effective roots.
- `MERIDIAN_MCP_COMPILERS`: allowlisted DreamMaker executables.
- `MERIDIAN_MCP_STATE_DIR`: required in development mode. This existing writable private state directory must be outside every workspace root and stores local atomic build records, failed-attempt history, and runtime-integrity journals; it is never published as evidence. Multiple MCP processes may share it through operation-scoped operating-system locks.
- `MERIDIAN_MCP_RIFT_BUILD`: `disabled` (default), `offline`, or `network`. The ceiling is immutable and `rift_compile` remains absent unless enabled.
- `MERIDIAN_MCP_HELPER_MANIFEST`: build-produced manifest for the exact dmdoc helper; absent or mismatched helpers keep `dm_generate_docs` unavailable.
- `MERIDIAN_MCP_DEBUGGER`: `disabled` (default) or `auxtools`. Auxtools requires development mode, one allowlisted `dm.exe`, its sibling `dreamseeker.exe`, and the fixed hash-verified DLL beside Meridian-MCP.
- `MERIDIAN_MCP_TRACY`: `disabled` (default) or `byond`. Tracy requires development mode and exact `tracy-server-helper` (host x86_64) and `byond-tracy` (x86) manifest entries for the current platform and BYOND baseline.

Every configured root and repository must already exist. An explicit root is recorded as `explicit_root`; a discovered linked worktree is recorded as `linked_git_worktree`. Repository membership is verified against a local SHA-256 identity derived from Git's canonical common directory. That identity is useful only for local authorization and is not portable source provenance. Exact duplicate roots are collapsed, with explicit authorization taking precedence. Git remotes are not queried.

Client tool calls cannot change the mode, effective roots, executable allowlist, or full-build ceiling. Restart Meridian-MCP after changing any startup authorization. Use `dm_server_status` to inspect the effective policy; path-policy failures return the same containment mode, policy source, effective roots, and recovery context. `rift_compile` defaults to `network_mode=offline`; `network_mode=allow` is accepted only under the startup value `network`. Offline mode is cooperative preflight and strict process-local package-manager configuration, not an operating-system firewall.

The configuration updater expects the named server and environment tables to exist already. It updates the installed binary, manifest, roots, mode, state, debugger, and Tracy settings while preserving unrelated servers and existing keys such as the compiler allowlist and Rift build ceiling. A complete Windows development table can therefore look like this before or after the updater runs:

```toml
[mcp_servers.meridian-mcp]
command = 'C:\path\to\installed-meridian-mcp\meridian-mcp.exe'

[mcp_servers.meridian-mcp.env]
MERIDIAN_MCP_MODE = 'development'
MERIDIAN_MCP_ROOTS = 'C:\path\to\Meridian-Rift'
MERIDIAN_MCP_REPOSITORIES = 'C:\path\to\Meridian-Rift'
MERIDIAN_MCP_COMPILERS = 'C:\Program Files (x86)\BYOND\bin\dm.exe'
MERIDIAN_MCP_STATE_DIR = 'C:\path\to\private-meridian-state'
MERIDIAN_MCP_RIFT_BUILD = 'offline'
MERIDIAN_MCP_HELPER_MANIFEST = 'C:\path\to\installed-meridian-mcp\helpers\manifest.json'
MERIDIAN_MCP_DEBUGGER = 'auxtools'
MERIDIAN_MCP_TRACY = 'disabled'
```

## Operational details

AphelionDMM uses the versioned compatibility declaration at `tests/compatibility/aphelion-dmm.json`. Its adapter is limited to `dm_parse_environment`, `dm_map_info`, and `dm_check_errors`, resolves repository identities through trusted startup configuration, and records the negotiated Meridian-MCP version plus the parsed state generation. It is not a generic MCP proxy and accepts no client-selected executable, method name, repository root, or filesystem path.

Active operations are available only in development mode. `dm_compile` invokes DreamMaker directly. For Meridian-Rift, `rift_compile` invokes the separate agent-owned `RIFT_BUILD.cmd`; humans continue to use the authoritative `BUILD.cmd`. Full-build output can optionally include bounded, observational endpoint samples, but `capture_complete` is always `false`.

Successful managed compilation binds the compiler, repository identity, parsed source closure, optional fixture inputs, and DMB/RSC hashes to a local build record. A later failed compile or any changed recorded input/output makes that managed artifact stale and all launch adapters refuse it. An unmanaged human-built DMB remains launchable with an explicit `unverified` warning by default; set `require_verified_provenance=true` to reject it. Fixture manifests are declarative relative-path records and cannot carry commands, arguments, URLs, globs, environment variables, or paths outside their fixture directory.

Standard runtime integrity journals live under `runtime-integrity/` in private state. Each active journal owns a per-session operating-system liveness lock, so another MCP process skips its startup recovery while the owner is live and recovers it only after the owner exits. The baseline is written before process spawn, a fixed five-second monitor records the first observed change with the nearest preceding owned output line, and wait/status/stop refresh or finalize the same record, including after natural process exit. Only exact owned files such as the current runtime log may be exempted; directories and globs are rejected. Modified or added paths are warnings, while deletion is a violation reported after process termination. Meridian-MCP never reverts, repairs, deletes, stages, or rewrites a changed workspace file.

Each non-owned runtime mutation is returned with the stable `source_integrity_warning` code. Native cumulative snapshots captured before the declared game-start boundary are classified `pre_game_cumulative` and are not presented as in-game interval measurements.

See [CONTRIBUTING.md](CONTRIBUTING.md) and [TESTING.md](TESTING.md) for development and verification guidance.

## Trust, design, and policy documents

- [Provenance and inherited code](docs/provenance.md)
- [Source authority](docs/source-authority.md)
- [Compatibility and evidence](docs/compatibility.md)
- [Dependency policy](docs/dependency-policy.md)
- [Security policy](SECURITY.md)
- [Detailed security model](docs/security.md)
- [Architecture](docs/architecture.md)
- [Native evidence analysis](docs/native-evidence.md)
- [Tracy profiling](docs/tracy-profiling.md)
- [Tool contracts](docs/tool-contracts.md)
- [Testing](TESTING.md)

## License

Meridian-MCP is distributed under MIT. Dependencies retain their own licenses; see [dependency policy](docs/dependency-policy.md).
