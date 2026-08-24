# Rift Compile and Meridian-Rift Compatibility Design

## Status

This specification records the maintainer-approved design for adding a dedicated Meridian-Rift full-build tool and promoting DreamMaker compilation, parsing, lookup, definitions, and search only after a real repository-scale integration gate passes. It defines target behavior; it does not claim that the current implementation already meets these requirements.

The selected architecture is a dedicated `rift_compile` tool. It reuses contained process-execution infrastructure where appropriate but remains contractually distinct from the direct DreamMaker `dm_compile` gate.

## Objective

Provide Codex and other MCP clients with a safe, structured way to run Meridian-Rift's authoritative Windows `BUILD.cmd`, while producing enough reproducible evidence to mark the following capabilities `Verified`:

- `dm_parse_environment`
- `dm_get_type`
- `dm_get_proc`
- `dm_get_var`
- `dm_list_types`
- `dm_search_symbols`
- `dm_search_context`
- `dm_get_definition`
- `dm_compile`
- `rift_compile`

The work must preserve the distinction between parser evidence, direct DreamMaker evidence, and the repository's complete build evidence.

## Non-goals

This work does not:

- Replace `dm_compile` with `rift_compile`.
- Make `BUILD.cmd` portable to Linux.
- Permit callers to execute arbitrary scripts, build targets, or command-line arguments.
- Modify Meridian-Rift's human-authored `BUILD.cmd` or its inherited bootstrap and build implementation.
- Claim complete network observation or provide an operating-system network sandbox.
- Promote DreamChecker, map, rendering, DreamDaemon, or `Topic()` tools in this phase.
- Make the scheduled BYOND workflow a required pull-request gate.
- Change normal human `BUILD.cmd` behavior when the MCP-specific offline flag is absent.

## Authority and evidence hierarchy

For Meridian-Rift:

1. `BUILD.cmd` is the authoritative full-project build entry point on Windows.
2. Direct `dm.exe tgstation.dme` is a faster compiler gate, but it does not build TGUI, tgfont, behavior trees, icon-cutter output, or other repository build targets.
3. SpacemanDMM parsing and indexing are navigation and static-analysis evidence, not DreamMaker acceptance.
4. A zero process exit code is insufficient without the expected artifact and diagnostic evidence.

Compatibility promotion requires a successful named-platform integration run that records the Meridian-MCP commit, resolved Meridian-Rift commit, BYOND version, tool configuration, and produced artifacts.

## Public tool contract

### Startup ceiling

`MERIDIAN_MCP_RIFT_BUILD` is immutable startup configuration with three accepted values:

- `disabled` - default; `rift_compile` is not advertised.
- `offline` - `rift_compile` is advertised in development mode and accepts only offline invocations.
- `network` - `rift_compile` is advertised in development mode and accepts either offline or network-enabled invocations.

`rift_compile` is never advertised in analysis mode. An MCP request cannot raise the startup ceiling. Invalid startup values stop server initialization with an actionable error.

### Invocation parameters

`rift_compile` accepts only structured parameters:

- `network_mode`: `offline` by default, or `allow` when the startup ceiling is `network`.
- `timeout_ms`: optional wall-clock timeout within the contract maximum.
- `idle_timeout_ms`: optional no-progress timeout within the contract maximum.
- `capture_network`: optional best-effort endpoint auditing, default `false`.
- `force_rebuild`: optional vetted request to bypass build-cache decisions, default `false`.

The tool accepts no DME path, working directory, executable path, script path, target name, define list, or arbitrary argument list. It operates only on the active successfully parsed project profile.

### Project qualification

The active profile qualifies only when all of these facts hold:

- The parsed DME is named `tgstation.dme`.
- Its canonical parent is inside a configured workspace root.
- Canonical contained root `BUILD.cmd` and `RIFT_BUILD.cmd` entry points exist.
- A contained `dependencies.sh` yields a literal numeric BYOND version.
- The profile still refers to the active state generation when execution begins.

`ProjectProfile` records `BUILD.cmd` as the human-authoritative entry point and `RIFT_BUILD.cmd` as the MCP-controlled derivative. Failure returns a structured precondition error directing the caller to `dm_parse_environment`. The tool never searches parent directories or falls back to another script.

Execution also requires exactly one allowlisted DreamMaker executable. `rift_compile` injects that canonical path as `DM_EXE`; zero configured compilers is `compiler_not_configured`, and more than one is `compiler_ambiguous`. This avoids caller selection while keeping the BYOND installation an immutable launch decision.

### Effects and documentation

The contract registry gains an external-network effect distinct from loopback networking. `rift_compile` declares the maximum effect permitted by its startup ceiling and invocation. Generated documentation explains that offline calls do not intentionally use external networking, while `network_mode=allow` may use external hosts through Meridian-Rift's checked-in bootstrap.

`dm_compile` keeps its existing direct-compiler semantics. It gains only the optional `capture_network` observation flag; it does not gain network-bootstrap permission.

## Execution architecture

### Shared contained-process runner

`dm_compile` and `rift_compile` use one internal process runner responsible for:

- Bounded stdout and stderr collection.
- Wall-clock and idle timeout enforcement.
- Progress detection based on output and process CPU-time changes where supported.
- Exit status and termination-reason reporting.
- Owned process-tree containment and termination.
- Optional network-audit lifecycle.

Tool-specific modules remain responsible for command construction, preflight, diagnostic interpretation, and artifact acceptance. The shared runner does not know DreamMaker or Meridian-Rift semantics.

On Windows, `rift_compile` launches the canonical `RIFT_BUILD.cmd` through the system command processor with fixed quoting and a canonical working directory equal to the parsed project root. Descendants are assigned to an owned Job Object so timeout or cancellation terminates the build tree rather than only the immediate `cmd.exe` process. The wrapper delegates non-interactively to the same `tools/build/build.bat build` base operation used by `BUILD.cmd`; it does not add `--wait-on-error`.

On non-Windows systems, `rift_compile` is unavailable and returns a stable unsupported-platform result if reached through an older cached schema. The rest of Meridian-MCP remains portable.

### Artifact evidence

Before execution, the tool records evidence for `tgstation.dmb` and `tgstation.rsc`:

- Existence.
- Size.
- Last-write timestamp.
- SHA-256 hash when present.

It repeats the snapshot after the process exits and classifies the outcome as:

- `fresh_artifacts` - the required outputs exist and at least one freshness property changed as expected.
- `valid_cache_hit` - the build explicitly reports targets current, exits successfully, and both required artifacts remain valid.
- `build_failed` - non-zero exit, parsed error diagnostics, timeout, cancellation, or missing required output.
- `insufficient_evidence` - the process reports success but artifact or cache evidence cannot prove a valid result.

Only `fresh_artifacts` and `valid_cache_hit` set `success: true`. A forced build must produce `fresh_artifacts`; a cache hit during `force_rebuild=true` is an evidence failure.

### Structured result

The result includes:

- Canonical project root, DME path, and build entry point.
- Active state generation.
- Declared BYOND version.
- Startup ceiling and invocation network mode.
- Force-rebuild and audit settings.
- Start time and duration.
- Exit code and termination reason.
- Bounded stdout and stderr with truncation metadata.
- Parsed DreamMaker-style diagnostics.
- Before-and-after artifact snapshots and outcome classification.
- Observed network endpoints when auditing is enabled.
- `capture_complete: false` for every best-effort network audit.
- Actionable error category and recovery guidance on failure.

## Offline and network build modes

### Offline mode

The MCP sets `MERIDIAN_RIFT_BUILD_NETWORK=offline` only for the child wrapper environment. `RIFT_BUILD.cmd` delegates to a Meridian-owned PowerShell script under `tools/build/rift/`, which completes preflight before it starts the inherited build implementation.

Preflight runs before the wrapper delegates to `tools/build/build.bat` and verifies that the pinned local prerequisites needed by the default build are usable, including:

- The pinned Bun executable or an already provisioned compatible executable.
- Bun package data required by both root and TGUI installs, checked with `bun install --offline --frozen-lockfile` behavior.
- The platform-appropriate icon-cutter binary when icon cutting is required.
- Python bootstrap prerequisites needed by behavior-tree compilation.
- The configured or discoverable DreamMaker version matching the repository declaration.

If a prerequisite is absent, stale, or would require a download, preflight fails without starting `tools/build/build.bat` and names the missing item. The wrapper supplies a temporary global Bun configuration with `install.offline = true`, disables automatic environment-file loading and telemetry, and sets pip's no-index and version-check controls. Missing cached dependencies are errors. The temporary configuration is outside the repository and is removed in `finally` cleanup.

This is cooperative enforcement around the checked-in build tooling, not an operating-system firewall, and documentation must not claim otherwise. The human-authored `BUILD.cmd`, JavaScript and Python bootstrap scripts, `build.ts`, and downloader stay untouched.

### Network-enabled mode

`network_mode=allow` is accepted only when the server started with `MERIDIAN_MCP_RIFT_BUILD=network`. The Meridian-owned wrapper performs its structural checks, then preserves Meridian-Rift's normal bootstrap behavior, including pinned Bun, package, Python, and icon-cutter acquisition where required.

Network permission is scoped to the owned build process tree and the duration of the call. Meridian-MCP itself does not become a general HTTP client, and the caller cannot provide URLs or credentials through `rift_compile`.

The child receives a documented environment allowlist containing the Windows process variables required by the build plus MCP-selected values such as the approved DreamMaker path and build-mode flags. Credential-bearing variables and unrelated client environment values are not inherited. An optional `TG_BOOTSTRAP_CACHE` is retained only when its canonical path is inside a configured workspace root; otherwise the build uses Meridian-Rift's contained default cache.

### Best-effort network audit

When `capture_network=true`, the process runner periodically samples operating-system endpoint tables for the owned process tree. Windows collection covers observable TCP and UDP endpoints with process identifier, protocol, local endpoint, remote endpoint where applicable, first-seen time, and last-seen time. Results are deduplicated and bounded.

Short-lived connections, activity between samples, kernel-mediated resolution, and processes that exit before ownership discovery may be missed. Every result therefore reports `capture_complete: false` and labels the data observational. The audit does not enforce offline mode and cannot be used as proof that no connection occurred.

The same optional audit is exposed by `dm_compile` so direct DreamMaker execution can be observed under the same semantics.

## Meridian-Rift agent build wrapper

Meridian-Rift adds only separate Meridian-owned infrastructure:

- Root `RIFT_BUILD.cmd` is the stable MCP-facing entry point.
- `tools/build/rift/invoke.ps1` validates the wrapper environment, performs offline preflight, creates temporary process-local configuration, optionally removes only `tgstation.dmb` and `tgstation.rsc`, invokes `tools/build/build.bat build`, propagates `$LASTEXITCODE`, and cleans temporary state.
- `tools/build/rift/README.md` documents ownership, modes, prerequisites, and the relationship to `BUILD.cmd`.
- `tools/build/rift/test.ps1` exercises preflight failures, cleanup, exit propagation, and delegate drift detection without downloading dependencies.

The wrapper's base delegate and target intentionally mirror the current `BUILD.cmd`. A drift test reads the human entry point as data, normalizes its single delegate command, and fails if the human workflow no longer targets `tools/build/build.bat ... build`. The wrapper never executes text parsed from `BUILD.cmd`.

`MERIDIAN_RIFT_FORCE_REBUILD=1` authorizes the wrapper to remove only the generated compiler outputs before delegation. The MCP sets this variable only for `force_rebuild=true`. Other build caches remain under the human build system's control.

No inherited build, bootstrap, downloader, release, deployment, or CI implementation is modified for the MCP wrapper. If later evidence shows that a requirement cannot be enforced externally, work stops for a new explicit infrastructure-change approval.

## Real-repository compatibility corpus

### Transport boundary

The compatibility gate builds the release binary and drives it through stdio MCP initialization, tool discovery, and tool calls. Calling Rust functions directly is insufficient for promotion.

### Versioned compatibility manifest

Meridian-MCP owns a small data manifest of stable queries and assertions against Meridian-Rift. It covers:

- Exact type lookup for representative root, datum, atom, mob, and Meridian-owned types.
- Locally declared and inherited proc lookup.
- Locally declared and inherited variable lookup.
- Type listing under bounded prefixes and hierarchy relationships.
- Exact symbol-name search across type, proc, and variable kinds.
- Ranked context queries whose intended result must appear within a bounded top result window.
- Definition resolution for types and members.

Assertions use canonical DreamMaker paths, declaration kinds, parent relationships, repository-contained relative file paths, and expected result membership. They do not pin absolute line numbers or exact whole-corpus symbol counts. Ranked searches must be deterministic across repeated identical requests.

Definition results must resolve inside the checkout and the reported source span must contain the requested declaration. Search and listing responses must obey their requested and contract maximum limits.

### Parse and state cases

The full-corpus run records parse duration, parsed file count, symbol counts, warnings, state generation, and tool latency. These measurements are evidence and trend data; repository growth alone is not a failure. Severe regression thresholds remain governed by the compatibility documentation.

Owned fixtures continue to verify atomic reparsing, generation increments, and preservation of the last valid state after failure. The real checkout verifies that all selected operations remain usable at repository scale.

Negative cases cover:

- Tool calls before parsing.
- Unknown type, proc, variable, and definition paths.
- Malformed query paths.
- Reads outside configured roots.
- A failed reparse followed by a successful lookup from the preserved state.
- `rift_compile` against an unqualified project profile.
- An offline build with a deliberately unavailable prerequisite.
- A network-enabled request under an offline startup ceiling.

## Windows integration workflow

The scheduled and manually dispatched BYOND workflow runs on a GitHub-hosted Windows runner.

The workflow:

1. Checks out the Meridian-MCP revision under test.
2. Checks out Meridian-Rift's current default branch, or a manually supplied `meridian_ref`, into a sibling directory.
3. Resolves and records both commit SHAs.
4. Reads the literal BYOND version from Meridian-Rift's `dependencies.sh`.
5. Requires the initial compatibility baseline `516.1685`; a future pin change fails with instructions to review and update the baseline deliberately.
6. Installs that exact BYOND version and configures the compiler allowlist.
7. Builds the Meridian-MCP release binary.
8. Starts it in development mode with `MERIDIAN_MCP_RIFT_BUILD=network` and both checkout roots contained.
9. Runs full-corpus parse and every versioned analysis assertion through stdio MCP.
10. Runs a fresh direct `dm_compile` and validates its artifacts and diagnostics.
11. Runs a forced network-enabled `rift_compile` through `RIFT_BUILD.cmd` with endpoint auditing and requires fresh artifacts.
12. Invokes the human `BUILD.cmd` once against the warm checkout, requires success, and confirms that its default target accepts the same generated artifacts.
13. Runs a forced offline `rift_compile`; the wrapper removes only the generated compiler artifacts and must rebuild them without network bootstrap.
14. Emits a single structured evidence bundle and uploads it with the bounded logs.

The manual workflow accepts an optional Meridian-Rift ref. Scheduled runs omit it and follow the repository's default branch. Every run records the resolved SHA rather than treating a branch name as evidence.

The workflow remains scheduled/manual because BYOND and full TGUI builds are comparatively expensive. Per-change CI retains Windows and Ubuntu Rust, contract, release-binary, and owned-fixture gates.

## Ubuntu behavior

Ubuntu CI verifies:

- Startup parsing for all `MERIDIAN_MCP_RIFT_BUILD` values.
- Contract and schema generation.
- Analysis-mode and development-mode tool visibility.
- The stable unsupported-platform response for `rift_compile`.
- Shared process-runner behavior that is platform-independent.
- Existing parser, search, stdio MCP, release-binary, and owned-fixture coverage.

Ubuntu does not provide evidence that `BUILD.cmd`, DreamMaker, or `rift_compile` works there. Public compatibility tables state this boundary directly.

## Evidence bundle

The integration script writes machine-readable JSON containing:

- Workflow run metadata.
- Both repository SHAs and remote identities.
- Windows runner image and architecture.
- Rust, MCP, BYOND, Bun, and Python versions used.
- Startup ceiling and per-call build modes.
- Compatibility-manifest version and individual assertion results.
- Parse, lookup, definition, and search timings.
- Direct and full-build structured tool responses.
- Artifact hashes, sizes, timestamps, and freshness classifications.
- Observed endpoints and the mandatory incomplete-capture disclaimer.
- Overall pass/fail and the first failing stage.

Raw output remains bounded before upload, and environment variables, credentials, and authorization headers are not serialized.

## Promotion policy

Implementation lands with all affected tools still `Provisional`. Promotion occurs only after the Windows integration workflow succeeds against:

- A recorded Meridian-MCP commit.
- A recorded Meridian-Rift commit.
- Windows GitHub-hosted runner evidence.
- BYOND 516.1685 for the initial baseline.
- Both network-enabled and offline `rift_compile` modes.

Promotion is per tool. A failure in ranked search, for example, does not prevent separately evidenced exact lookup tools from being promoted, but the public capability row must not summarize a mixed set as wholly verified.

After a successful run:

- Update the applicable `SupportLevel` entries in `src/contracts.rs`.
- Regenerate `docs/tool-contracts.md`.
- Update the README capability table and individual tool descriptions.
- Update `docs/compatibility.md` with the run link, exact SHAs, BYOND version, and platform.
- Retain `dm_compile` wording as a direct compiler gate.
- Describe `rift_compile` as the Meridian-Rift `BUILD.cmd` full-build gate.

No documentation may claim verification based only on local observation or the existence of the workflow.

## Deferred compatibility work

`docs/compatibility.md` gains a visible deferred-verification matrix for:

- DreamChecker diagnostics.
- DMM/TGM information and search.
- PNG map rendering.
- DreamDaemon launch, readiness, output waiting, status, and stop.
- Loopback `Topic()` calls.

For each deferred capability, the matrix names the required owned fixture, real-repository or named-platform integration gate, artifact or semantic assertion, and current blocker. These entries remain `Provisional` and are not silently covered by this design.

## Error handling

Stable error categories include:

- `unsupported_platform`
- `tool_disabled`
- `network_mode_denied`
- `project_not_parsed`
- `project_not_qualified`
- `compiler_not_configured`
- `compiler_ambiguous`
- `state_generation_changed`
- `offline_preflight_failed`
- `build_spawn_failed`
- `build_timed_out`
- `build_idle_timed_out`
- `build_failed`
- `artifact_missing`
- `artifact_stale`
- `insufficient_evidence`
- `network_audit_unavailable`

Audit unavailability is a warning when auditing is optional; it does not turn a successful build into a failure. Offline preflight or enforcement failure is fatal. All failures include recovery guidance without exposing unrestricted paths or suggesting that the caller weaken the startup ceiling.

## Documentation updates

Meridian-MCP updates:

- `README.md` for startup configuration, the new individual tool description, and compatibility wording.
- `SECURITY.md` and `docs/security.md` for conditional external networking and observational auditing.
- `docs/architecture.md` for the shared runner and full-build boundary.
- `docs/source-authority.md` for the direct-compiler/full-build distinction.
- `docs/compatibility.md` for evidence and deferred capability work.
- `docs/tool-contracts.md` through the registry generator.
- `TESTING.md` for local, Ubuntu, and Windows integration gates.

Meridian-Rift updates its agent verification documentation to describe `RIFT_BUILD.cmd`, `MERIDIAN_RIFT_BUILD_NETWORK=offline`, the wrapper prerequisites, the protected-infrastructure approval rule, and the fact that `BUILD.cmd` remains the human-authoritative entry point.

## Acceptance criteria

The design is implemented when:

- `rift_compile` is absent by default and available only under the approved startup ceiling in development mode.
- The tool can run only the active Meridian-Rift profile's canonical root `RIFT_BUILD.cmd`.
- `BUILD.cmd` and inherited human-authored build/bootstrap implementation remain unchanged.
- Offline mode fails before a known download, supplies strict process-local package-manager controls, and does not claim operating-system enforcement.
- Network-enabled mode preserves existing bootstrap behavior.
- Optional process-tree endpoint auditing works on Windows and always reports incomplete capture.
- Timeouts and cancellation terminate the owned build process tree.
- Results distinguish fresh artifacts, valid cache hits, failures, and insufficient evidence.
- The real stdio MCP integration gate covers the approved analysis tools, `dm_compile`, and both `rift_compile` build modes.
- Ubuntu CI verifies portable behavior without making a BYOND or `BUILD.cmd` claim.
- No affected tool is promoted until the first successful Windows integration run is recorded.
- DreamChecker, map, rendering, runtime, and Topic verification remain explicitly tracked for follow-up.
