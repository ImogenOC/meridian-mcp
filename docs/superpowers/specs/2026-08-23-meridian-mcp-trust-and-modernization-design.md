# Meridian-MCP Trust and Modernization Design

## Status

This specification records the maintainer-approved design decisions from the 2026-08-22 and 2026-08-23 Meridian-MCP research review. It defines the target architecture and documentation baseline. It does not claim that the current implementation already satisfies these requirements.

Related repository specifications:

- Meridian-Rift: `docs/superpowers/specs/2026-08-23-agent-standards-and-tooling-governance-design.md`.
- Aphelion Content Tools: `docs/superpowers/specs/2026-08-23-agent-standards-design.md`.

## Objective

Turn Meridian-MCP into an evidence-backed DreamMaker development server that is safe to expose to agentic clients, useful at Meridian-Rift scale, and honest about the distinction between BYOND compiler behavior, SpacemanDMM analysis, and experimental reverse-engineered protocols.

The product remains a general DreamMaker tool with an explicit Meridian-Rift project profile. Repository-specific conventions must not be hard-coded into the generic parser and protocol layers.

## Scope

The work covers:

- Source provenance and third-party dependency records.
- MCP transport modernization and client compatibility.
- DreamMaker parsing, indexing, search, definitions, diagnostics, compilation, map rendering, runtime control, and Topic queries.
- Filesystem, executable, process, and network trust boundaries.
- Tool contracts, support levels, testing, performance baselines, and public documentation.
- Integration expectations for Meridian-Rift and Aphelion Content Tools.

The work does not include:

- Replacing DreamMaker as the authority for language acceptance.
- Reimplementing all of SpacemanDMM.
- General remote machine administration.
- Internet-facing DreamDaemon management.
- Automatically updating Meridian-Rift code or upstream documentation.
- Preserving an inherited feature solely because it exists in the original dm-mcp source.

## Authority and evidence

Authority depends on the question being answered:

1. Official BYOND documentation and reproducible DreamMaker behavior govern language and runtime semantics.
2. A target repository's checked-in configuration governs project selection, includes, pinned BYOND version, and local policy.
3. Current tgstation guidance and implementation govern inherited tg systems unless Meridian-Rift records a deliberate downstream delta.
4. Nova documentation governs inherited Nova modularization and merge-preservation behavior.
5. SpacemanDMM is the selected analysis implementation. Its results are evidence, not compiler truth.
6. The original dm-mcp repository is provenance, not authority.
7. Meridian-MCP documentation describes only behavior demonstrated by tests or explicitly labeled experimental.

Every compatibility claim must identify the tested version, platform, client, and evidence level. “Expected to work” is not a supported status.

Primary research inputs are the official BYOND Dream Maker guide and reference, tgstation's checked-in guides and build tooling, the locally inherited Nova modularization handbook and public maintainer guidance, SpacemanDMM, the official MCP specification and Rust SDK, and the original `imcynic/dm-mcp` source history.

## Provenance baseline

Meridian-MCP must record:

- The original dm-mcp repository and source commit `6a739a4278b53e86b430abaf011467f22c9dd2ec`.
- That the working tree was imported without preserving git ancestry.
- Which subsystems were inherited, substantially changed, newly implemented, quarantined, or removed.
- The exact SpacemanDMM revision in `Cargo.lock` and the revision-selection policy in `Cargo.toml`.
- Relevant licenses and distribution obligations for direct and bundled dependencies.
- The origin and validation status of reverse-engineered constants, packet layouts, and algorithms.

The public README must link to this provenance record. Licensing documentation is a risk-control measure, not a substitute for legal advice.

## Target architecture

### MCP transport

The official Rust MCP SDK becomes the transport and protocol compatibility layer. Hand-written JSON-RPC framing is retired after equivalent Codex and protocol tests pass. Existing `dm_*` names remain stable unless a name misrepresents what the tool does.

Transport code owns framing, initialization compatibility, cancellation, progress, protocol errors, and schema exposure. It must not contain DreamMaker domain logic.

### Tool contract registry

Each tool has one structured contract containing:

- Name, summary, input schema, and output schema or documented result shape.
- Required server state and project profile.
- Whether it reads files, writes files, overwrites files, spawns a process, or opens a loopback connection.
- Capability mode and allowed roots.
- Default and maximum result sizes, timeouts, and line limits.
- Error categories and recovery guidance.
- Support level and verification evidence.

The MCP schema and human-readable tool reference must be generated from, or checked against, the same contract source. Documentation drift is a CI failure.

### Project profiles

The generic engine accepts an explicit workspace root and `.dme`. A project profile may additionally discover:

- `SpacemanDMM.toml`.
- The repository's pinned BYOND version from `dependencies.sh` or a future equivalent.
- The full-build entry point.
- Repository-specific verification commands.

The Meridian profile describes these facts but does not silently run a full build during parsing or diagnosis.

### Analysis engine

Analysis tools include environment parsing, type/proc/var lookup, exact definition lookup, symbol search, contextual search, DreamChecker diagnostics, and read-only map information.

Parsing a new environment atomically replaces the previous environment state only after success. A failed parse leaves the last valid state unchanged and returns `state_preserved: true` with the active environment and state generation. Source changes are not assumed visible until a successful reparse.

Results use canonical DreamMaker paths, repository-relative source paths when possible, one-based lines and columns, deterministic ordering, and bounded source excerpts.

### Active operations

Compilation, rendered map output, DreamDaemon launch, wait, stop, and Topic calls are active operations. They remain separate from analysis and must never be implied by a read-only tool.

`dm_compile` invokes DreamMaker for a compiler gate. It does not claim to perform tgstation's full project build. A future project-build tool, if justified, receives a distinct name and contract.

Only server-owned DreamDaemon processes may be waited on or stopped. Runtime readiness requires a configured positive marker or other explicit evidence; process liveness alone is not readiness.

Topic and protocol connections are loopback-only. Internet-facing administration is outside the product scope.

## Capability modes

The server exposes two explicit modes:

- `analysis`: read-only parsing, indexing, lookup, search, diagnostics, and map inspection.
- `development`: all analysis tools plus controlled compilation, map output, runtime process management, and Topic calls.

The default is `analysis`. Enabling `development` is an installation or launch decision, not something a model can activate through an MCP tool call.

Both modes require configured workspace roots. Reads must resolve inside an allowed root. Writes are limited to allowed roots or a server-managed temporary directory. Path validation must account for canonical paths, traversal, reparse points, and symlinks.

DreamMaker executables are discovered from supported installations or selected from an allowlist. An unrestricted caller-provided executable path is disabled by default. Map rendering refuses to replace an existing file unless the request explicitly sets `overwrite=true`. Runtime DMBs must be inside an allowed root.

## Support levels

Every user-facing capability has one of these labels:

- `verified`: passes committed automated tests and the documented integration gate on a named platform/version.
- `provisional`: has useful automated coverage but lacks part of the supported integration matrix.
- `experimental`: may change or fail; disabled by default when it crosses a security or protocol boundary.
- `unsupported`: retained only for migration or removed from normal builds.

Windows is the primary verified platform. Linux may be best-effort when covered by CI. macOS is unsupported until tested. Platform claims must follow evidence rather than intent.

## BYOND client protocol decision

The inherited client login, packet framing, and RUNSUB implementation is experimental and disabled in normal builds. It may remain behind a Cargo feature during the evidence-gathering phase.

It cannot become provisional or verified without all of the following:

- Traceable provenance for its algorithm, constants, and packet layouts.
- Sanitized golden packets or a reproducible capture procedure.
- Successful live handshakes against every claimed BYOND version.
- Negative, truncation, size-bound, and malformed-input tests.
- A demonstrated workflow that cannot be served adequately by `world.Topic()`.

If no active consumer demonstrates that need during the review, the subsystem is removed. Self-encryption/decryption round trips do not count as compatibility evidence.

## Dependency policy

SpacemanDMM dependencies are pinned to exact git revisions. Updating the revision requires a dedicated compatibility change containing:

- Parser and diagnostic fixture results.
- Full Meridian-Rift parse/index results.
- Map fixture results when DMM tooling changes.
- Performance comparison against the recorded baseline.
- License and advisory refresh.

The Rust minimum version is declared through `rust-version` and exercised in CI. Dependency audits and license inventory run in CI, with documented handling for advisories that cannot be fixed immediately.

## Test architecture

### Unit and contract tests

- Deterministic tests for parsing adapters, canonical paths, search ordering, bounds, and error categories.
- Tool-schema snapshots or equivalent schema assertions.
- Path-containment, overwrite, executable-allowlist, timeout, and process-ownership tests.
- Protocol transport tests using the official SDK's supported harnesses.

### Fresh DreamMaker fixtures

Meridian-MCP owns small, purpose-written fixtures for object trees, absolute and relative paths, proc overrides, vars, defines, conditionals, documentation, compiler errors, DreamChecker diagnostics, DMM/TGM maps, multiple z-levels, missing resources, runtime readiness, logs, crashes, Topic return types, and shutdown.

Fixtures are not copied from tgstation. A local tgstation or Meridian-Rift checkout is used only for integration testing.

### Differential and integration tests

- DreamMaker is the acceptance authority for language fixtures.
- SpacemanDMM differences are recorded explicitly rather than hidden.
- Meridian-Rift `tgstation.dme` is parsed and indexed as a full-corpus integration gate.
- Runtime tests use a dedicated minimal DMB and verify readiness, logs, Topic behavior, timeout behavior, and clean stop.
- Map tests assert semantic coordinates and visible output; file creation alone is insufficient.
- The installed release binary is tested through stdio with Codex-compatible initialization and tool calls.

Rust, MCP contract, and license checks run on every change. BYOND-backed integration runs on a scheduled or manually dispatched Windows job until its reliability and cost justify making it a required per-change gate.

### Performance

Initial gates record parse duration, peak memory where measurable, symbol count, document count, and representative query latency. CI fails only on severe regression, initially greater than two times the accepted baseline, until enough data exists for tighter thresholds.

## Documentation set

The implementation creates or rewrites:

- `README.md`: accurate purpose, quick start, capability table, support labels, security summary, and links.
- `docs/architecture.md`: component boundaries and state lifecycle.
- `docs/tool-contracts.md`: generated or contract-checked tool reference.
- `docs/source-authority.md`: authority and conflict-resolution rules.
- `docs/provenance.md`: lineage and subsystem disposition.
- `docs/security.md`: threat model and capability modes.
- `docs/compatibility.md`: platform, BYOND, SpacemanDMM, MCP, and client matrix.
- `docs/dependency-policy.md`: revision and license policy.
- `TESTING.md`: evidence levels, fixtures, local gates, CI gates, and real-client validation.
- `SECURITY.md`: reporting and supported-version policy.
- `CONTRIBUTING.md`: repository-specific workflow.
- `CHANGELOG.md`: corrected current capability history.

Machine-specific paths and private workflow assumptions do not belong in general documentation. Codex-specific setup may live in a separate client guide.

## Cross-repository contract

Meridian-Rift owns game-code standards, upstream lineage, its authoritative full-build command, and placement/marker policy. Aphelion Content Tools owns structured content, schemas, writer workflows, and staged export. Meridian-MCP consumes their checked-in profiles and reports evidence; it does not become the policy owner for either repository.

The MCP is responsible for source navigation and diagnostics. PowerShell remains the owner of Windows build and test orchestration. A focused check is iteration evidence; completion claims follow the target repository's full verification matrix.

## Rollout boundaries

Implementation is split into independently reviewable plans:

1. Documentation, provenance, support labels, and trust inventory.
2. Security boundaries and tool-contract registry.
3. Independent fixtures and full-corpus integration gates.
4. Exact dependency pinning and compatibility workflow.
5. Official Rust MCP SDK migration.
6. Client-protocol quarantine or removal.
7. Installed-binary and Codex validation.

No phase may broaden a support claim before its evidence gate passes.

## Acceptance criteria

The design is complete when:

- Every exposed tool has a contract, capability mode, bounds, and evidence label.
- Normal builds do not expose the unverified client protocol.
- Default operation cannot execute an arbitrary caller-selected program or write outside configured roots.
- DreamMaker, SpacemanDMM, and repository build results are described as distinct evidence.
- Dependencies and original-source provenance are pinned and documented.
- Windows, BYOND, MCP, and Codex compatibility claims are backed by reproducible tests.
- Meridian-Rift parses and indexes at repository scale within the accepted baseline.
- Public documentation contains no unverified “full support” claims.
