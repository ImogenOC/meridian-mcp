# Tracy Profiler Integration Implementation Plan

> **Execution rule:** Follow this plan with test-driven development, exact Rust 1.95.0 commands, and no commits or pushes without explicit authorization.

**Goal:** Add a safe, opt-in, compatibility-evidenced Tracy profiling surface for BYOND to Meridian-MCP.

**Architecture:** Generalize the verified-helper supply chain, build a one-shot C++ helper over pinned Tracy server internals, coordinate an MCP-owned profiled DreamDaemon, and expose separate bounded capture and offline-analysis tools.

**Tech stack:** Rust 1.95.0, Tokio, rmcp 3.1.3, CMake/C++20, Tracy v0.14.0, byond-tracy build-d1ec404, PowerShell, BYOND 516.1685.

---

## Task 1: Establish provenance and helper-manifest v2

**Files:**
- Create: `tracy-capabilities.json`
- Create: `src/helper_manifest.rs`
- Modify: `src/lib.rs`
- Modify: `src/spaceman/docs.rs`
- Modify: `scripts/build-spacemandmm-helpers.ps1`
- Modify: `scripts/install-meridian-mcp.ps1`
- Test: `tests/helper_manifest.rs`
- Test: `tests/capability_registry.rs`

1. Write failing tests for schema-v2 parsing, unique helper IDs, platform and architecture matching, exact SHA/revision validation, BYOND bounds, traversal rejection, and schema-v1 dmdoc compatibility.
2. Run `cargo +1.95.0 test --test helper_manifest --test capability_registry` and confirm the intended failures.
3. Implement the generic parser and validator, then adapt dmdoc to request helper ID `dmdoc`.
4. Update the helper build and install scripts to emit schema v2 while preserving schema-v1 input support.
5. Add the Tracy/byond-tracy capability registry with exact revisions, protocol 82, licenses, verification gates, and explicit exclusions.
6. Run focused tests, then the complete Rust suite.

## Task 2: Add bounded helper stdin and atomic external output promotion

**Files:**
- Modify: `src/process.rs`
- Modify: `src/atomic_output.rs`
- Test: `tests/process_runner.rs`
- Test: `tests/atomic_output.rs`

1. Add failing tests proving stdin is bounded, closed after writing, never included in audit output, and rejected above the central limit.
2. Add failing tests proving an externally produced temporary file is validated and promoted only on success.
3. Extend `ProcessSpec` with optional bounded stdin bytes and add a promotion API that retains the existing overwrite and containment guarantees.
4. Run focused tests and the full suite.

## Task 3: Implement the fixed Tracy helper protocol

**Files:**
- Create: `helpers/tracy/CMakeLists.txt`
- Create: `helpers/tracy/src/main.cpp`
- Create: `helpers/tracy/src/protocol.hpp`
- Create: `helpers/tracy/src/protocol.cpp`
- Create: `helpers/tracy/src/session.hpp`
- Create: `helpers/tracy/src/session.cpp`
- Create: `helpers/tracy/src/queries.hpp`
- Create: `helpers/tracy/src/queries.cpp`
- Create: `helpers/tracy/tests/protocol_tests.cpp`
- Create: `helpers/tracy/tests/query_tests.cpp`

1. Add C++ tests first for one-request framing, schema and command rejection, bounded strings/results, deterministic sorting, malformed trace errors, and stable response envelopes.
2. Confirm the tests fail because the helper implementation is absent.
3. Implement only the fixed commands `capture`, `hotspots`, `zone`, `frame_stats`, and `compare` over Tracy `Worker` APIs.
4. Keep stdout protocol-only and send logs to stderr.
5. Run CTest on x64 Windows and Ubuntu configurations.

## Task 4: Build and verify native artifacts

**Files:**
- Create: `scripts/build-tracy-helpers.ps1`
- Create: `tests/tracy_build_contract.rs`
- Modify: `.github/workflows/ci.yml` or the repository's current portable workflow files
- Modify: `docs/provenance.md`
- Modify: `docs/dependency-policy.md`

1. Add failing contract tests for exact commits, x64 helper, x86 hook, no runtime source download, manifest production, and license inclusion.
2. Implement the PowerShell builder with explicit `-TracyPath`, `-ByondTracyPath`, `-OutputDirectory`, and `-ManifestPath` parameters.
3. Verify Git HEADs before running CMake/MSBuild or CMake/GCC `-m32`.
4. Add portable Windows and Ubuntu build/test jobs without live BYOND claims.
5. Run script contract tests and both local build paths available on the current host.

## Task 5: Add Tracy configuration and tool contracts

**Files:**
- Modify: `src/config.rs`
- Modify: `src/contracts.rs`
- Modify: `src/server.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/lib.rs`
- Test: `tests/config_and_paths.rs`
- Test: `tests/mcp_conformance.rs`
- Test: `tests/tool_contracts.rs`

1. Add failing tests for `MERIDIAN_MCP_TRACY=disabled|byond`, disabled default, development-only availability, valid-helper requirements, and the exact nine-tool inventory.
2. Add `TracyAccess` and startup validation for `tracy-server-helper` and `byond-tracy` manifest entries.
3. Register individual schemas and descriptions with bounded parameters and accurate effects.
4. Regenerate `docs/tool-contracts.md` and confirm the checked-in reference test passes.

## Task 6: Implement explicit hook preparation

**Files:**
- Create: `src/tools/tracy.rs`
- Modify: `src/tools/mod.rs`
- Test: `tests/tracy_tools.rs`

1. Add failing tests for contained DMB selection, platform filename selection, matching-file idempotence, mismatch refusal, explicit overwrite, hash verification, and atomic replacement.
2. Implement `dm_tracy_prepare` using the generic helper manifest and atomic output APIs.
3. Return exact artifact identity and preparation state.
4. Run focused and complete tests.

## Task 7: Add profiled runtime lifecycle

**Files:**
- Modify: `src/state.rs`
- Modify: `src/tools/runtime.rs`
- Modify: `src/tools/debugger.rs`
- Modify: `src/tools/tracy.rs`
- Create: `src/process_environment.rs`
- Test: `tests/tracy_runtime.rs`

1. Add failing tests for runtime kinds, lifecycle serialization, fixed `-params tracy`, loopback-only environment, readiness markers, and debugger/runtime mutual exclusion.
2. Extract reusable minimal Windows runtime-environment construction without changing normal `dm_run` behavior.
3. Implement `dm_tracy_launch`, `dm_tracy_status`, and `dm_tracy_stop`.
4. Ensure capture termination precedes DreamDaemon termination and all error paths clear ownership state.
5. Run runtime, debugger, and full-suite regressions.

## Task 8: Add bounded live capture

**Files:**
- Create: `src/tracy_protocol.rs`
- Modify: `src/tools/tracy.rs`
- Modify: `src/limits.rs`
- Test: `tests/tracy_capture.rs`

1. Add failing tests for strict helper envelopes, one active capture, private loopback port, duration/memory bounds, contained output, optional network audit, interrupted capture cleanup, and atomic promotion.
2. Implement the Rust protocol client through `run_contained_process`.
3. Implement `dm_tracy_capture` and persist only bounded status metadata.
4. Run focused and complete tests.

## Task 9: Add offline analysis and source correlation

**Files:**
- Modify: `src/tracy_protocol.rs`
- Modify: `src/tools/tracy.rs`
- Test: `tests/tracy_analysis.rs`

1. Add failing fixture-backed tests for hotspots, zone details, ServerTick percentiles, comparison identity, deterministic ordering, truncation metadata, and operation without a parsed environment.
2. Implement `dm_tracy_hotspots`, `dm_tracy_zone`, `dm_tracy_frame_stats`, and `dm_tracy_compare`.
3. When a snapshot exists, attach exact definition/source correlation without changing profiler values.
4. Run focused and complete tests.

## Task 10: Package, configure, and document

**Files:**
- Modify: `scripts/install-meridian-mcp.ps1`
- Modify: `scripts/configure-codex-meridian-mcp.ps1`
- Modify: `README.md`
- Modify: `TESTING.md`
- Modify: `docs/architecture.md`
- Modify: `docs/security.md`
- Modify: `docs/provenance.md`
- Modify: `docs/dependency-policy.md`
- Modify: `docs/compatibility.md`
- Test: `tests/documentation.rs`

1. Add failing documentation and script-contract tests for explicit `-EnableTracy`, disabled default, individual tool descriptions, process/network effects, pinned versions, and experimental status.
2. Install only verified manifest artifacts and emit the opt-in Codex environment setting only when requested.
3. Document operator workflow, cleanup, overhead, licensing, compatibility evidence, and limitations.
4. Run documentation tests and validate all local Markdown links.

## Task 11: Add live Windows and Ubuntu compatibility gates

**Files:**
- Create: `tests/fixtures/tracy/`
- Create: `scripts/run-tracy-integration.ps1`
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `tests/compatibility/meridian-rift.json`
- Modify: `docs/compatibility.md`

1. Create a technical DM fixture that invokes the prof extension and exercises stable known procs and ServerTick frames.
2. Drive the installed MCP stdio entry point through prepare, launch, capture, analyze, compare, status, and stop.
3. Assert trace identity, known proc file/line, frame statistics, clean termination, and bounded artifacts.
4. Record exact Windows evidence, then exact Ubuntu evidence independently.
5. Run a separate real Meridian-Rift smoke capture without changing `BUILD.cmd` or inherited Tracy DM source.

## Task 12: Final verification and installed configuration

1. Run `cargo +1.95.0 fmt --all -- --check`.
2. Run `cargo +1.95.0 clippy --all-targets --all-features -- -D warnings`.
3. Run `cargo +1.95.0 test --all-features`.
4. Run `cargo +1.95.0 build --locked --release --all-features`.
5. Run the repository's current cargo-deny gate using its exact pinned invocation.
6. Run CTest for the native helper and the available live BYOND gates.
7. Build/package/install the release, configure with `-EnableTracy`, and verify the installed stdio tool inventory and a bounded smoke call.
8. Review `git diff --check`, `git status --short`, and all generated references.
9. Report evidence and request a Codex restart only after the installed configuration is verified.
