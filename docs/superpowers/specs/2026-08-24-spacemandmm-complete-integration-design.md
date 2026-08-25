# Complete SpacemanDMM Integration and DMI Analysis Design

## Status

This specification records the maintainer-approved design for upgrading Meridian-MCP to the current
SpacemanDMM revision, mapping every relevant upstream capability to an MCP-native contract, and adding
repository-scale DMI profiling and duplicate detection. It defines target behavior; it does not claim
that the current implementation already meets these requirements.

The selected architecture is a hybrid direct-adapter design. Meridian-MCP links the pinned
SpacemanDMM libraries for parsing, checking, DMI, DMM, and rendering work; implements MCP-native
indexes around the parsed DreamMaker model where language-server logic is not exposed as a library;
and uses only fixed, versioned helpers for capabilities that are inherently binary-only. It does not
run an internal LSP bridge or expose an arbitrary SpacemanDMM command runner.

## Approved decisions

- Upgrade the exact SpacemanDMM pin from `7fdd00d8e9b7f7583df4960b5ed38269685ec432`
  to `351ddc0ffb2439876d4565ce5130bb6b027ee605`.
- Upgrade Meridian-MCP's exact Rust toolchain and CI baseline from 1.88 to 1.95, matching the pinned
  upstream workspace. The Meridian-MCP crate may remain on edition 2021 unless implementation
  evidence requires an edition change.
- Map every relevant current SpacemanDMM capability to an MCP tool or internal adapter and record
  every exclusion with a testable reason.
- Prefer direct library adapters and immutable parsed snapshots. Do not use an internal LSP
  subprocess for language queries.
- Add read-only DMI profiling, extraction, cross-file duplicate discovery, and source-reference
  auditing, including common copied or lazily modified variants.
- Integrate the current auxtools debugger only as an opt-in, Windows-only, server-owned capability.
- Preserve existing Meridian-MCP tool names and response compatibility wherever the current behavior
  is correct. Add fields and tools rather than silently repurposing contracts.

## Objectives

1. Make SpacemanDMM's relevant parser, checker, language, DMI, DMM, renderer, documentation, and
   auxtools debugger capabilities available through typed Meridian-MCP tools.
2. Give agents repository-scale icon evidence without authorizing them to alter human-authored art.
3. Keep all reads, outputs, processes, and loopback communication inside immutable startup policy.
4. Support analysis on Windows and Ubuntu, with BYOND compilation and auxtools verification on
   Windows.
5. Make upstream capability drift visible in CI instead of relying on README claims or memory.
6. Promote individual tools to `Verified` only after their real transport and platform gates pass.

## Non-goals

This work does not:

- Implement or expose SpacemanDMM's early prototype graphical map editor.
- Add map mutation, automatic map repair, or automatic icon editing.
- Delete, merge, rename, move, recolor, redraw, or otherwise manipulate DMI content.
- Select a canonical icon from a duplicate cluster.
- Reproduce editor-only incremental text buffers, folding ranges, color swatches, or document-link UI.
- Expose arbitrary SpacemanDMM binaries, command-line arguments, DLLs, PIDs, ports, hosts, or
  environment variables.
- Integrate legacy extools or expose disassembly unavailable through the selected current auxtools
  backend.
- Add external `pngcrush` or `optipng` execution. PNG output uses contained Rust encoders.
- Replace DreamMaker compilation with DreamChecker. `dm_compile` and `rift_compile` retain their
  separate acceptance roles.
- Claim that static icon-reference analysis can prove an icon state unused when runtime expressions
  can select files or states dynamically.

## Upstream authority and dependency baseline

The implementation authority is the exact pinned source revision, not a floating branch or a
historical README. Revision `351ddc0ffb2439876d4565ce5130bb6b027ee605`, dated 2026-08-23, declares
Rust 1.95 and edition 2024 for its workspace and contains these active workspace crates:

| Upstream crate | Meridian-MCP disposition |
| --- | --- |
| `dreammaker` | Direct parser, preprocessor, configuration, object-tree, constants, docs, locations, and DMM model dependency. |
| `dreamchecker` | Direct whole-program static analysis adapter. |
| `dmi` | Direct DMI metadata parser and state/frame model. |
| `dmm-tools` | Direct icon decoding, DMM inspection, render-pass, minimap, PNG, and GIF adapter. |
| `dmm-tools-cli` | Do not expose the CLI. Map its relevant `list-passes`, `minimap`, `diff-maps`, `map-info`, and `RenderMany` behavior to typed MCP tools backed by the libraries. |
| `dmdoc` | Fixed helper because the upstream crate is binary-oriented. Inputs and outputs remain typed and contained. |
| `dm-langserver` | Do not run as an internal LSP server. Use its advertised and implemented behavior as the equivalence inventory for MCP-native indexes and tools. |
| `dap-types` | Internal protocol model for the restricted debugger adapter; no standalone tool. |
| `interval-tree` | Internal implementation support where relevant; no standalone tool. |
| `builtins-proc-macro` | Upstream build implementation detail; no standalone tool. |

The commented-out `spaceman-dmm` editor crate is outside the active upstream workspace and describes
itself as an early prototype. It is excluded. The exact Git revision must agree across `Cargo.toml`,
`Cargo.lock`, provenance, compatibility documentation, and the capability registry.

## Capability equivalence inventory

The checked-in capability registry is authoritative for release claims. Its initial inventory maps
the current upstream surface as follows.

### DreamMaker and language-server behavior

| Upstream behavior | Meridian-MCP mapping |
| --- | --- |
| Environment detection, preprocessing, parsing, object tree, configuration, diagnostics, docs, and locations | `dm_parse_environment` and the immutable analysis snapshot. |
| QueryObjectTree | `dm_get_type`, `dm_list_types`, and exact member inspection. |
| Definition and type definition | `dm_get_definition`, including the resolved declaration kind and type target. |
| Workspace symbols | `dm_search_symbols`; extend kinds to macros and other indexed declarations. |
| Document symbols | New `dm_document_symbols`. |
| References | New `dm_find_references`. |
| Implementations and override chains | New `dm_find_implementations`; existing `dm_get_proc` keeps exact inherited-proc inspection. |
| Hover | Exact `dm_get_type`, `dm_get_proc`, and `dm_get_var` results, including docs, types, values, signatures, and source locations. No redundant cursor-position wrapper. |
| Completion | Deterministic symbol/context search plus document symbols and exact type/member inspection. No editor completion-session state. |
| Signature help | `dm_get_proc` parameters, defaults, return information where known, docs, and override provenance. |
| Diagnostics | Upgraded `dm_check_errors`. |
| Reparse | `dm_parse_environment`; failed reparses preserve the last complete generation. |
| StartDebugger | Restricted debugger tools described below. |
| Incremental text sync | Excluded as editor-buffer state; the MCP indexes canonical files on disk. |
| Folding ranges | Excluded as presentation-only editor behavior. |
| Document colors and color presentation | Excluded as presentation-only editor behavior. |
| Document links | Excluded as editor presentation; exact definitions and dmdoc crosslinks cover agent navigation. |
| SetTraceVsc and editor telemetry | Excluded as editor-specific tracing. |

### DMI, DMM, documentation, and debugger behavior

| Upstream behavior | Meridian-MCP mapping |
| --- | --- |
| DMI metadata and decoded pixels | `dm_dmi_info` and internal DMI cache. |
| DMI PNG/GIF rendering | Development-only `dm_extract_dmi`. |
| Same-name duplicate example | Superseded by `dm_find_dmi_duplicates`, which covers same-file and cross-file states. |
| Map metadata | Enhanced `dm_map_info`. |
| Map coordinate/type search | Existing `dm_find_on_map`. |
| Map differences | New `dm_diff_maps`. |
| Render-pass inventory | New `dm_list_render_passes`. |
| Minimap bounds and pass selection | Enhanced `dm_render_map`. |
| RenderMany batch operation | New `dm_render_maps`. |
| dmdoc HTML generation and crosslinks | Development-only `dm_generate_docs`. |
| Debugger initialize and configuration-done requests | Internal `dm_debug_launch` lifecycle; no client-controlled protocol passthrough. |
| Auxtools launch | `dm_debug_launch` launches and owns the debuggee. |
| Auxtools attach | Excluded because it permits targeting a process or listener not owned by Meridian-MCP. |
| Source, conditional, and function breakpoints plus runtime exception breaking | `dm_debug_set_breakpoints`, `dm_debug_set_function_breakpoints`, and `dm_debug_set_exception_breakpoints`. |
| Threads, stacks, scopes, variables, evaluation, and exception information | Dedicated debugger query tools. |
| Debugger-provided `stddef.dm` source | `dm_debug_source`, restricted to source references emitted by the active session. |
| Breakpoint, pause, step, runtime, output, and termination events | `dm_debug_wait_for_event` over the bounded active-session event queue. |
| Continue, pause, step in, step over, and step out | Typed `dm_debug_control` action enum. |
| Disconnect and terminate-debuggee behavior | `dm_debug_stop`, with mandatory owned-process cleanup. |
| Extools disassembly | Excluded: legacy backend only; current auxtools reports disassembly unsupported. |
| Restart | Excluded from the initial contract because the pinned debugger does not implement a restart request. |

An upstream implementation detail does not need a public MCP tool when a typed MCP contract already
provides the same agent-relevant evidence. Such rows remain in the registry as `mapped`, not silently
omitted.

## Public tool surface

### Existing language and compilation tools

The existing contracts remain and are enhanced only additively:

- `dm_parse_environment`
- `dm_get_type`
- `dm_get_proc`
- `dm_get_var`
- `dm_list_types`
- `dm_search_symbols`
- `dm_search_context`
- `dm_check_errors`
- `dm_get_definition`
- `dm_compile`
- `rift_compile`

`dm_search_symbols` adds macro and declaration-kind coverage. `dm_check_errors` returns rule ID,
severity, explanation, source span, and configuration or suppression provenance when upstream makes
them available. DreamChecker results remain static-analysis evidence. `dm_compile` remains the direct
DreamMaker gate and `rift_compile` remains Meridian-Rift's contained full-build gate.

### New language tools

- `dm_document_symbols` lists declarations in one contained source file with kind, canonical name,
  type ownership, source span, and nesting.
- `dm_find_references` accepts an exact canonical symbol identity and returns bounded, deterministic
  read/write/call/type references with source spans and reference classifications where knowable.
- `dm_find_implementations` returns proc declarations, overrides, inherited implementations, and
  relevant type implementations in deterministic inheritance order.

These tools require a successful parse and identify the snapshot generation used.

### New DMI tools

- `dm_dmi_info` profiles one contained DMI.
- `dm_compare_dmi_states` performs an explicit two-state comparison.
- `dm_find_dmi_duplicates` scans a contained scope and reports duplicate clusters.
- `dm_audit_icons` combines DMI integrity, duplicate, source-reference, and best-effort unused-state
  findings.
- `dm_extract_dmi` writes an explicitly selected state, frame, contact sheet, PNG, or animated GIF
  where the pinned upstream encoder supports it. It is development-only.

### DMM and documentation tools

- `dm_map_info` gains dictionary/model counts, bounds, z-levels, dimensions, and parse warnings.
- `dm_find_on_map` retains contained exact type-instance search.
- `dm_diff_maps` reports structured size, dictionary, model, and coordinate changes between two maps.
- `dm_list_render_passes` reports pass name, description, default state, and pinned upstream revision.
- `dm_render_map` gains inclusive coordinate bounds and explicit render-pass enable/disable lists.
- `dm_render_maps` accepts a bounded list of maps and chunks and returns per-output results equivalent
  to upstream `RenderMany` without accepting arbitrary JSON or output paths outside policy.
- `dm_generate_docs` generates dmdoc HTML into an explicit contained directory. It is
  development-only.

### Restricted debugger tools

The debugger surface consists of:

- `dm_debug_launch`
- `dm_debug_set_breakpoints`
- `dm_debug_set_function_breakpoints`
- `dm_debug_set_exception_breakpoints`
- `dm_debug_threads`
- `dm_debug_stack_trace`
- `dm_debug_scopes`
- `dm_debug_variables`
- `dm_debug_evaluate`
- `dm_debug_exception_info`
- `dm_debug_source`
- `dm_debug_wait_for_event`
- `dm_debug_control`
- `dm_debug_stop`

`dm_debug_control` accepts only `pause`, `continue`, `step_in`, `step_over`, or `step_out`, with a
thread identifier where required. It is not a generic DAP request tunnel.

## Adapter architecture

### Spaceman facade

All upstream integration lives behind an internal `spaceman` facade with five adapters:

- `spaceman::language`
- `spaceman::dmi`
- `spaceman::dmm`
- `spaceman::docs`
- `spaceman::debugger`

Tool modules own request validation and result contracts. The facade owns translation to and from
the pinned upstream APIs. This contains revision-specific changes and prevents SpacemanDMM types from
becoming accidental public MCP schemas.

### Immutable analysis snapshot

A successful parse constructs one complete `Arc<AnalysisSnapshot>` before it replaces active state.
The snapshot contains:

- Canonical environment path and project root.
- SpacemanDMM object tree plus immutable context data extracted during parsing: cloned configuration,
  resolved file paths, owned macro-definition records, and diagnostics. The upstream `Context` and
  `DefineHistory` are not
  retained because its `RefCell` internals are not `Sync`.
- Preprocessor macro index and file table.
- Type, proc, variable, and document-symbol indexes.
- Reference and implementation indexes.
- Documentation, signatures, values, and source locations.
- DreamChecker configuration and diagnostic metadata.
- Project profile, upstream revision, and monotonic state generation.

A failed parse leaves the prior snapshot intact. Tool dispatch locks shared state only long enough to
clone the snapshot handle. Parsing, checking, scanning, rendering, and documentation execute outside
that lock on bounded blocking workers. Runtime and debugger lifecycle state are separate from the
analysis snapshot.

### Asset cache

DMI decoding uses a bounded cache keyed by canonical path, file size, modification time, and SHA-256
content identity. Every request revalidates the asset before returning cached metadata or pixels.
Icon edits do not require a DME reparse and advance an asset-generation identifier independently of
the analysis generation.

The cache retains decoded assets only within configured entry and byte limits. Eviction is
least-recently-used with deterministic reporting. Cache behavior never changes result ordering or
match classification.

## DMI profile contract

`dm_dmi_info` returns:

- Canonical contained path, SHA-256, file size, sheet dimensions, and cell dimensions.
- State count, total sprite/frame count, and ordered state table.
- State name, duplicate index, movement flag, direction count, animation frame count, delays, loop,
  and rewind metadata.
- Per-direction and per-frame sheet rectangle.
- Alpha bounds, opaque/translucent/transparent pixel counts, and normalized frame hash.
- Metadata parse warnings and unsupported fields.

The pinned `dmi` parser contains a hotspot placeholder rather than complete hotspot support. The MCP
must report hotspot information as unsupported; it must not silently fabricate or discard a claim
that hotspot semantics were validated.

## Duplicate and lazy-change analysis

### Comparison levels

The analyzer supports frame-level and whole-state comparison. A state identity is independent of its
DMI path and state name, but preserves direction labels, frame order, and animation structure.
Animation metadata is compared separately from image identity so callers can distinguish identical
pixels from identical runtime behavior.

Whole-state matches require a consistent mapping across all directions and frames. Geometric
transforms remap direction semantics: for example, a horizontal mirror maps east to west rather than
claiming that mirrored east-facing pixels are still an east-facing state.

### Deterministic candidate funnel

The scan avoids all-pairs comparison by using progressively broader candidate buckets:

1. **Exact normalized RGBA.** Fully transparent pixels have hidden RGB normalized to zero before
   hashing. This avoids false differences from invisible editor residue.
2. **Pixel identity with metadata differences.** Pixel sequences match, but directions, delays,
   movement, loop, rewind, or duplicate-index metadata differ.
3. **Exact geometric transforms.** Horizontal mirror, vertical mirror, 180-degree rotation, and
   90/270-degree rotation where cell dimensions permit.
4. **Translated or padded equivalence.** Frames are cropped to alpha bounds, normalized, and compared;
   the result records the source and target offsets and canvas difference.
5. **Palette equivalence.** The alpha mask and row-major canonical color-index topology match while
   concrete RGBA values differ. This detects palette swaps and recolors without calling unrelated
   silhouettes identical.
6. **Near duplicate.** Plausible candidates are aligned under their best valid transform and compared
   with a bounded premultiplied RGBA difference. Results include normalized similarity, changed-pixel
   count, maximum per-pixel difference, and the threshold that admitted the match.

The broad candidate stage may use a small luminance/alpha perceptual signature to avoid quadratic
comparisons, but the final reported score is calculated from aligned pixels, not from the signature
alone. Near-duplicate thresholds are explicit inputs within server-defined maxima and have
conservative defaults.

This funnel covers common copied or lazy modifications: same-name duplicates, renamed copies,
cross-DMI copies, mirrors, rotations, transparent padding, one-pixel shifts, palette swaps, and small
pixel edits.

### Match classification

- Exact pixel and exact transform matches are high confidence.
- Cropped padding or translation matches are high confidence when alpha-bounded content is exact.
- Palette-equivalent matches are medium confidence.
- Near matches are medium or low confidence according to their explicit score and changed-pixel
  ratio.

Each result includes match kind, confidence, both paths, state names, duplicate indices, direction and
frame coordinates, transform, direction remapping, offset, palette relationship, animation metadata
differences, and relevant static source references. Cluster and member ordering is stable across
identical scans.

### Repository scan and source correlation

The default scope is `**/*.dmi` beneath the active parsed project root. Callers may choose a narrower
contained directory or glob. The scan excludes version-control metadata, Rust build output, and
Meridian-MCP-owned generated caches by fixed policy; it does not follow paths outside configured
roots.

The analysis snapshot correlates statically resolvable `icon` and `icon_state` defaults and
assignments with type paths and source locations. `dm_audit_icons` reports:

- Missing contained DMI files.
- Statically named states missing from a resolved DMI.
- Duplicate state declarations inside one DMI.
- Cross-DMI duplicate and near-duplicate clusters.
- Best-effort states with no static references.
- Dynamic expressions that prevent a definitive unused-state result.

An unreferenced result is always labeled best-effort. Runtime icon construction, string composition,
appearance mutation, overlays, resource loading, and other dynamic behavior can make static evidence
incomplete.

### Human authorship boundary

All DMI analysis is report-only. The MCP never deletes, merges, renames, edits, recolors, redraws, or
selects a canonical asset. Visually identical states may have distinct intent, history, or licensing.
Any consolidation requires human approval plus source-reference, behavior, and provenance review.
Extraction is mechanical and never modifies the source DMI.

## DMM and renderer behavior

Map tools reuse the active object tree where rendering semantics require it and identify both the
analysis generation and upstream revision. Render-pass names are validated against the pinned
registry. Unknown passes fail rather than being ignored.

Bounds are one-indexed, inclusive map coordinates and are validated against map dimensions. Batch
rendering has explicit file, chunk, pixel, byte, and output-count limits. Each output uses an existing
contained parent, requires `overwrite=true` when already present, and is created through a temporary
file followed by atomic replacement.

`dm_diff_maps` is read-only and reports dimension changes and differing coordinates with bounded
before/after model data. It does not rewrite either map. External PNG optimizers remain excluded.

## Documentation adapter

`dm_generate_docs` uses a fixed dmdoc helper built from the exact pinned revision. It supports the
upstream documented source, macro, type, variable, proc, Markdown/text-module, crosslink, and static
HTML behavior.

The MCP accepts typed source and output fields only. It supplies no arbitrary helper arguments,
environment entries, templates, Git repositories, or URLs. Output must be a contained directory,
overwrite must be explicit, and generation occurs in a temporary sibling before atomic replacement.
The result records generated file counts, bounded warnings, revision, duration, and truncation.

## Restricted auxtools debugger

### Startup and artifact policy

Debugger contracts exist only when all of these are true:

- The server runs on Windows in development mode.
- `MERIDIAN_MCP_DEBUGGER=auxtools` was set before startup.
- The fixed `debug_server.dll` artifact is present in the documented installation location.
- Its version is `v2.3.7` and its SHA-256 is
  `b188999ac58a0e0171b015c39a403ab7da2f37ddb8ac3817a078f5bce02a8be7`.

Ordinary Cargo builds and server startup never download the DLL. An explicit packaging/fetch script
may acquire the single fixed upstream release URL, verifies the hash before installation, and fails
closed. Runtime cannot substitute another DLL path or checksum.

### Ownership and transport

`dm_debug_launch` starts only the fixed DreamSeeker executable discovered beside an allowlisted BYOND
installation and a contained DMB, matching the pinned SpacemanDMM launcher. Meridian-MCP
owns the process handle and debugger session. The client cannot attach to an arbitrary process,
choose a DLL, choose a non-loopback host, or inject arbitrary process environment. Debugger
communication uses loopback and an ephemeral port selected by the server. The child receives only the
fixed `-trusted` argument plus `AUXTOOLS_DEBUG_MODE=LAUNCHED`, the selected loopback
`AUXTOOLS_DEBUG_PORT`, and the verified `AUXTOOLS_DEBUG_DLL` path.

The debugger lifecycle is explicit: `idle`, `launching`, `running`, `stopped`, `terminated`, or
`failed`. Query and control tools validate the active state. Session events and process output are
bounded. Timeout, cancellation, MCP shutdown, or `dm_debug_stop` closes the debugger connection,
stops owned reader tasks, and terminates the owned DreamSeeker tree when required.

The initial adapter exposes only behavior implemented by the current auxtools path: source and
conditional breakpoints, function breakpoints, runtime exception breaking and information, threads,
stack traces, scopes, variables, evaluation, debugger-provided `stddef.dm` source, pause, continue,
step in, step over, step out, and disconnect. `dm_debug_source` accepts only a source reference issued
by the active session; it is not a general file reader. Extools-only disassembly is explicitly
unsupported. Restart is not advertised until a future pinned upstream implementation supports and
verifies it.

## Security and resource limits

Analysis tools are read-only. Every input path is canonicalized and must remain below an immutable
configured root. Scans default to the active parsed root. Outputs are development-only, require an
existing contained parent and explicit overwrite, and use temporary creation plus atomic replacement.

Server configuration defines maxima for:

- DMI file count and aggregate input bytes.
- Image dimensions, decoded pixels, states, directions, and frames.
- Candidate comparisons and near-match work.
- Scan, render, documentation, and debugger duration.
- Match, reference, diagnostic, and cluster result counts.
- Generated output files and aggregate output bytes.
- Asset-cache entries and decoded bytes.
- Debugger events, variables, frames, and retained process output.

Results state when any limit truncated or skipped work. A truncated scan cannot claim that no
duplicates or references exist. Expensive CPU and blocking I/O work runs behind bounded semaphores so
concurrent MCP calls cannot create unbounded worker or memory growth.

No new tool accepts an arbitrary executable, command, argument vector, shell fragment, DLL, PID,
host, URL, credential, environment map, or network destination.

## Result and error model

Successful results identify:

- Meridian-MCP version.
- Exact SpacemanDMM revision.
- Analysis generation when used.
- Asset generation and file hashes when used.
- Configured limits, work performed, truncation, and cache statistics relevant to the call.

Stable tool-error categories distinguish:

- Invalid or ambiguous input.
- Path outside policy.
- Missing parse state or stale generation.
- Missing file, state, symbol, map model, or debugger session.
- Unsupported pinned-upstream behavior.
- Configured resource limit or timeout.
- Partial best-effort evidence.
- Fixed-helper failure or checksum mismatch.
- External DreamMaker, DreamDaemon, or debugger-owned DreamSeeker failure.
- Internal adapter failure.

Best-effort incompleteness is returned as structured warning metadata unless it prevents the requested
operation. Protocol errors remain reserved for requests the MCP SDK cannot route.

## Capability registry and drift prevention

The repository adds a machine-readable capability registry. Every row contains:

- Stable upstream capability identifier and category.
- Upstream crate or binary and exact revision.
- Evidence location in the pinned source inventory.
- Disposition: `direct`, `mcp_native`, `fixed_helper`, `superseded`, or `excluded`.
- MCP tool or internal adapter.
- Required mode, feature flag, and operating system.
- Effects and limits.
- Verification test or explicit exclusion rationale.

CI validates that:

- The registry revision equals `Cargo.toml`, `Cargo.lock`, provenance, and compatibility records.
- Every public tool has a contract, detailed README entry, registry mapping, and test reference.
- Every registry mapping names a real tool, adapter, or accepted exclusion code.
- No relevant upstream inventory row is unresolved.
- Generated tool-contract documentation matches checked-in output.

The revision-update script audits the selected upstream workspace crates, language-server capability
declaration, DMM CLI subcommands, DMI features, and debugger request implementations. A SpacemanDMM
pin change is incomplete until the registry is refreshed and reviewed. Exact pinning prevents an
upstream feature from entering normal builds before that review.

## Compatibility and verification

### Exact Rust gates

`rust-toolchain.toml`, `Cargo.toml`, both GitHub workflows, README, testing documentation, and any CI
scripts move together to Rust 1.95. Verification invokes the explicit pinned toolchain rather than a
shell alias or whichever `cargo` appears first.

Required per-change gates are:

- `cargo fmt --check`.
- Clippy across all targets and supported features with warnings denied.
- Full Rust tests.
- Release build.
- `cargo deny check --all-features` on Ubuntu.
- Capability-registry and generated-document contract checks.
- MCP stdio initialization, `tools/list`, representative invocation, structured error, and shutdown.

### Unit and owned-fixture coverage

Tests cover:

- Snapshot construction, atomic replacement, failed-reparse preservation, and concurrent readers.
- Macros, document symbols, definitions, references, implementations, signatures, and DreamChecker
  diagnostic details.
- DMI metadata, duplicate indices, transparent-RGB normalization, transform and direction remapping,
  padding/translation, palette topology, near-score boundaries, animation metadata, clustering,
  determinism, cache invalidation, and limits.
- DMM metadata, coordinate differences, render passes, bounds, batch output, and overwrite policy.
- dmdoc containment, helper failure, and atomic output replacement.
- Debugger state transitions, checksum rejection, ownership rejection, loopback enforcement, bounded
  results, cancellation, and cleanup.
- Path traversal and rejection of arbitrary executable, command, DLL, process, network, and
  environment inputs.

DMI algorithm fixtures use non-creative technical pixel matrices generated inside test code. They
are validation infrastructure, not shipped game assets or human-facing artwork.

### Ubuntu gate

Ubuntu verifies every parser, language, DreamChecker, DMI, DMM, renderer, dmdoc, contract, and MCP
protocol behavior that does not require BYOND. It verifies that Windows-only debugger tools are not
advertised and return a stable unsupported-platform result if reached through a stale client schema.

Ubuntu does not claim DreamMaker, `rift_compile`, DreamDaemon, or auxtools compatibility. The public
compatibility table states that boundary.

### Windows and Meridian-Rift gate

Windows runs the same portable suite plus the existing BYOND integration. The repository-scale gate
drives the release binary over stdio and, against a recorded Meridian-Rift commit:

1. Parses `tgstation.dme` and records counts, duration, warnings, and generation.
2. Exercises exact type/member inspection, document symbols, definitions, references,
   implementations, symbol search, context search, and DreamChecker.
3. Profiles representative contained DMIs and runs a bounded repository icon audit.
4. Inspects and diffs representative maps, lists render passes, renders bounded output, and exercises
   batch rendering.
5. Generates contained dmdoc output and validates its manifest and crosslinks.
6. Runs direct `dm_compile` and Meridian-Rift `rift_compile` through their existing authoritative
   wrappers and evidence rules.
7. In the opt-in debugger job, verifies the exact auxtools artifact, launches an owned DreamSeeker,
   exercises supported debugger queries and control, and proves clean shutdown.

The evidence bundle records both repository SHAs, Rust and BYOND versions, SpacemanDMM revision,
active startup configuration, tool results, artifact hashes, limits, truncation, and the first failing
stage. Focused or best-effort checks are not reported as full compatibility.

## Documentation and provenance

The implementation updates:

- The GitHub README capability section with an actual description of every individual tool.
- `docs/tool-contracts.md` through its generator.
- Architecture, security, compatibility, dependency-policy, testing, installation, and configuration
  documentation.
- Provenance with the exact upstream revision and helper artifacts.
- License and `cargo deny` records for the complete dependency graph.

The project already treats the SpacemanDMM root license as GPL-3.0-or-later in dependency policy.
The upgrade and any bundled helper require a refreshed distribution-obligation review. Documentation
records the engineering evidence and does not present legal advice.

## Implementation sequence

1. Upgrade Rust, the exact SpacemanDMM pin, lockfile, CI, provenance, and dependency policy while
   preserving all existing tests.
2. Add the capability registry, audit script, and `spaceman` facade.
3. Refactor parsing into the immutable snapshot and add macro, document-symbol, reference, and
   implementation indexes.
4. Upgrade DreamChecker results and existing language contracts.
5. Add DMI profiling and test-matrix primitives.
6. Add exact, transformed, padded, palette, and near-duplicate scanning plus source correlation and
   `dm_audit_icons`.
7. Add mechanical DMI extraction.
8. Complete DMM differences, render-pass selection, bounded rendering, and batch rendering.
9. Add the fixed dmdoc helper adapter.
10. Add explicit auxtools acquisition, validation, restricted lifecycle, and debugger tools.
11. Update generated contracts and detailed public documentation.
12. Run the exact Windows, Ubuntu, stdio, and real Meridian-Rift compatibility gates before
    promoting support levels.

## Definition of done

The integration is complete only when:

- The repository pins SpacemanDMM revision `351ddc0ffb2439876d4565ce5130bb6b027ee605` and Rust 1.95
  consistently.
- Every relevant current upstream capability is mapped, tested, superseded with an equivalent, or
  explicitly excluded with an approved reason.
- Every public tool has a generated contract and detailed user documentation.
- Windows and Ubuntu CI pass their exact required gates.
- Real Meridian-Rift compatibility evidence passes over the shipped MCP stdio transport.
- DMI results find exact copies and the approved common lazy-change classes without modifying art.
- Generated outputs and debugger operations obey containment, overwrite, ownership, checksum, and
  resource limits.
- License, provenance, and advisory records are refreshed.
- The release binary intended for Codex is installed and smoke-tested before the user is told to
  restart Codex.
