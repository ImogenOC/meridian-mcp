# Over-64K BYOND CI Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Use superpowers:test-driven-development for every behavior change, superpowers:systematic-debugging for any unexpected result, and superpowers:verification-before-completion before reporting success. Execute inline unless the user explicitly authorizes delegation.

**Goal:** Replace the ambiguous Windows over-64K timeout with independent, evidence-backed checks for SpacemanDMM parsing, BYOND engine startup, and the real Meridian-Rift Windows path, while preserving a strict compatibility signal and ensuring one specialized fixture cannot suppress all later evidence.

**Architecture:** Treat the 65,537-leaf source as an engine regression fixture, not as a proxy for every Meridian-MCP capability. A cross-platform parser gate proves the freshly built MCP/SpacemanDMM stack can index and retrieve boundary types. A dedicated BYOND runtime matrix compares a compact 50,000-leaf control with a compact 65,537-leaf case and records process progress. The real Windows Meridian-Rift, auxtools, and Tracy gates run independently. Ubuntu is the required synthetic DreamDaemon engine lane because `/tg/station` runs DreamDaemon integration on Ubuntu; Windows synthetic startup remains a visible diagnostic until it meets a promotion criterion, while the real Windows Meridian-Rift Tracy launch remains the required Windows runtime proof.

**Tech Stack:** Rust 1.95.0, PowerShell 7, GitHub Actions, BYOND 516.1687, SpacemanDMM commit `351ddc0ffb2439876d4565ce5130bb6b027ee605`, Windows Server 2025, Ubuntu 24.04.

**Spec:** `docs/superpowers/specs/2026-08-26-tracy-profiler-reliability-design.md`, supplemented by `docs/compatibility.md` and the official BYOND 516 release notes.

---

## Confirmed diagnosis and non-goals

The failed run at Meridian-MCP commit `732094223bd2f60f099ad99d37ea389b9d97b4aa` proves:

- DreamMaker 516.1687 compiled the generated source and produced a DMB with exit code 0.
- The corrected readiness monitor did not stop when a launcher exited; the owned DreamDaemon process remained alive for the full 60-second window.
- DreamDaemon produced no stdout, stderr, world log, or readiness marker.
- The prior launcher-monitoring fix repaired one harness defect but did not resolve the underlying startup failure.

The current evidence does **not** distinguish slow DMB loading, a Windows hosted-runner launch/bind problem, or a surviving BYOND engine hang. Do not choose among those explanations without the control/matrix evidence in this plan.

This plan does not:

- downgrade a failed required product gate to success;
- claim Windows synthetic over-64K compatibility when that check is red;
- change Meridian-Rift's human-authored `BUILD.cmd` or `RIFT_BUILD.cmd`;
- update BYOND, SpacemanDMM, Tracy, byond-tracy, or Rust pins;
- commit or push changes without a separate explicit request.

## Acceptance contract

Implementation is complete only when one manually dispatched workflow shows all of the following:

1. Freshly built Meridian-MCP parses the compact 65,537-leaf fixture and retrieves its first, boundary, and last declared type on Windows and Ubuntu.
2. Ubuntu BYOND 516.1687 compiles and starts the compact 50,000-leaf control and 65,537-leaf boundary fixture, observes the exact marker, and leaves no owned process alive.
3. The Windows real-repository job reaches and passes Meridian-Rift parse/lookup/search/build, auxtools, and Tracy gates regardless of the Windows synthetic sentinel result.
4. Windows synthetic evidence reports one of `passed`, `environment_failure`, `boundary_regression`, or `inconclusive_timeout`; it is never represented as a generic missing-marker timeout.
5. Evidence includes fixture topology, declared counts, source/DMB size and hashes, elapsed phase timings, process CPU and working-set samples, exit state, logs, and cleanup state without absolute host paths or user-authored source.
6. Ubuntu portable CI, Rust formatting, Clippy under the exact 1.95.0 pin, tests, and dependency policy remain green.

The Windows synthetic sentinel may become required only after three consecutive scheduled or manual runs pass on `windows-2025`. Until then, documentation must label it diagnostic and use the real Meridian-Rift Tracy launch as the required Windows DreamDaemon proof.

## Decision table

| Control (50,000) | Boundary (65,537) | Classification | Action |
|---|---|---|---|
| pass | pass | `passed` | Keep the compact fixture and consider Windows promotion after three consecutive passes. |
| pass | fail/hang | `boundary_regression` | Preserve evidence, keep Ubuntu required, keep Windows diagnostic, and prepare an upstream BYOND reproduction. |
| fail/hang | not run | `environment_failure` | Diagnose the runner/runtime/launch configuration; do not attribute the result to 64K. |
| pass | active with measurable progress at 300 s | `inconclusive_timeout` | Retain samples and increase only the diagnostic ceiling in a follow-up based on observed progress. |
| compiler failure | not run | `compile_failure` | Treat as fixture or BYOND compiler failure and fail the required lane. |

## Global implementation constraints

- Use `cargo +1.95.0` for every Rust command and record `rustc +1.95.0 --version` in verification output.
- Use PowerShell for all BYOND build and live-runtime work.
- Start with tests that fail for the intended reason.
- Use only fixture-owned marker/log/evidence files; do not upload the generated DM source or DMB.
- Use BYOND's documented positional port `0`, `-close`, `-trusted`, `-verbose`, and explicit `-log <relative-file>` options for the synthetic fixture.
- Do not infer readiness from process liveness, a bound port, or launcher exit alone.
- Never kill a process that was present before the test. Record creation identity as well as PID where the platform exposes it.
- Keep the real Windows product gates required. A synthetic diagnostic cannot replace them.
- Leave all implementation changes uncommitted unless the user explicitly requests commits.

## Task 1: Lock the three independent compatibility claims in tests

**Files:**

- Modify: `tests/workflow_contract.rs`
- Modify: `tests/byond_compatibility_fixture.rs`
- Modify: `tests/process_readiness.rs`
- Test: `tests/workflow_contract.rs`
- Test: `tests/byond_compatibility_fixture.rs`
- Test: `tests/process_readiness.rs`

- [x] **Step 1: Add failing workflow contract tests**

Require separately named jobs/steps for:

```rust
assert!(workflow.contains("windows-meridian-compatibility:"));
assert!(workflow.contains("prototype-parser-compatibility:"));
assert!(workflow.contains("prototype-runtime-compatibility:"));
assert!(workflow.contains("matrix.required"));
assert!(workflow.contains("continue-on-error: ${{ !matrix.required }}"));
```

Assert that the Windows real-repository job contains the Meridian-Rift, auxtools, and Tracy scripts and does not `need` the synthetic runtime job. Assert that Ubuntu is the required runtime matrix entry and Windows is the diagnostic entry.

- [x] **Step 2: Add failing fixture contract tests**

Require the fixture generator to emit:

- one compact parent path;
- exactly the requested number of leaf paths;
- `declared_leaf_count` and `declared_type_count` with distinct names;
- the exact first, boundary, and last paths;
- no bucket parent hierarchy.

Use a small test count to assert the generated path set is unique and flat. Keep a separate contract that the production boundary invocation requests 65,537 leaves.

- [x] **Step 3: Add failing readiness telemetry tests**

Extend the existing helper-process tests to require:

```powershell
status
elapsed_milliseconds
samples
last_progress_milliseconds
process_exit_code
```

Test three deterministic children:

1. delayed marker while the launcher remains alive;
2. launcher exits but a marker is emitted by a separately tracked owned process;
3. no marker with increasing CPU or working set, which must classify as a wall timeout rather than an idle failure.

- [x] **Step 4: Confirm the expected failures**

Run:

```powershell
rustc +1.95.0 --version
cargo +1.95.0 test --test workflow_contract --test byond_compatibility_fixture --test process_readiness
```

Expected: failures identify the missing job separation, compact fixture contract, and telemetry fields.

## Task 2: Minimize the fixture and make its evidence diagnostic

**Files:**

- Modify: `scripts/new-large-prototype-fixture.ps1`
- Modify: `scripts/process-readiness.psm1`
- Modify: `scripts/run-large-prototype-integration.ps1`
- Modify: `tests/byond_compatibility_fixture.rs`
- Modify: `tests/process_readiness.rs`

- [x] **Step 1: Generate separate flat parser and bucketed runtime fixtures**

Generate a single parent and short leaf paths, for example:

```dm
/datum/mlp
/datum/mlp/p00000
/datum/mlp/p00001
```

Retain `world/New()` with the exact marker write and immediate `shutdown()`. The MCP parser lane uses this flat layout. The BYOND runtime lane uses 256-child buckets because local BYOND 516.1687 verification proved that 65,537 direct children trigger DreamMaker's distinct `Maximum number of child nodes exceeded!` error before a DMB exists. Return structured generator metadata:

```json
{
  "layout": "flat",
  "declared_leaf_count": 65537,
  "declared_parent_count": 1,
  "declared_type_count": 65538,
  "first_path": "/datum/mlp/p00000",
  "boundary_path": "/datum/mlp/p65535",
  "last_path": "/datum/mlp/p65536"
}
```

The runtime form records `layout: bucketed`, 258 declared parents, 65,795 declared types, and bucket-qualified lookup paths. Do not call the leaf count the total BYOND prototype count. This implementation adjustment is required to test the documented total-prototype startup boundary instead of the compiler's direct-child limit.

- [x] **Step 2: Replace the fixed game port**

Remove the `GamePort = 14570` default from the synthetic harness. Invoke DreamDaemon with positional port `0`, which BYOND documents as any available port. The marker-only fixture does not require a stable client endpoint.

Use an explicit fixture-relative log argument:

```powershell
ArgumentList = @($dmb, '0', '-trusted', '-log', 'dreamdaemon.world.log', '-close', '-verbose')
```

`-logself` is documented and not itself a defect; explicit `-log` is chosen so evidence does not depend on world-name inference.

- [x] **Step 3: Add process-progress sampling**

Change `Wait-ProcessReadiness` to sample every 250 ms and retain a bounded series at one-second resolution. Each sample contains:

```powershell
[ordered]@{
	elapsed_milliseconds = $elapsed
	pid = $Process.Id
	has_exited = $Process.HasExited
	total_processor_milliseconds = $Process.TotalProcessorTime.TotalMilliseconds
	working_set_bytes = $Process.WorkingSet64
	private_memory_bytes = $Process.PrivateMemorySize64
	marker_exists = Test-Path -LiteralPath $MarkerPath
}
```

On Windows, also record a boolean `main_window_present`; do not retain window titles. Record process start time only as a relative identity token or hash so evidence contains no host-local details.

- [x] **Step 4: Use a five-minute wall ceiling**

Set the synthetic runtime default to 300 seconds. This is a wall ceiling, not proof that 300 seconds is an expected startup time. Do not add an idle timeout until the first matrix evidence establishes a safe progress signal for DreamDaemon loading.

- [x] **Step 5: Expand evidence schema and classification**

Bump the evidence schema and record:

- operating system and runner image;
- fixture generator metadata;
- source byte count and SHA-256;
- DMB byte count and SHA-256;
- compile and runtime elapsed milliseconds;
- exact redacted arguments;
- bounded process samples;
- marker state and marker observation time;
- explicit classification from the decision table;
- owned-process creation identity and verified cleanup result.

Capture bounded Windows Application event records for DreamDaemon or BYOND faults that occur inside the test interval. Store event provider, event ID, level, and a redacted message tail only. Event-log access failure is diagnostic metadata, not a new reason to fail.

- [x] **Step 6: Verify Task 2**

Run:

```powershell
cargo +1.95.0 test --test byond_compatibility_fixture --test process_readiness
pwsh -NoProfile -File scripts/new-large-prototype-fixture.ps1 -OutputDirectory "$env:TEMP\meridian-prototype-plan-check" -PrototypeCount 16
git diff --check
```

Inspect the 16-leaf fixture and delete only that explicit temporary directory after confirming its resolved path is under `$env:TEMP`.

## Task 3: Prove the freshly built MCP/SpacemanDMM parser above 64K

**Files:**

- Create: `scripts/run-large-prototype-parser-integration.ps1`
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `tests/workflow_contract.rs`
- Modify: `tests/byond_compatibility_fixture.rs`
- Modify: `docs/compatibility.md`

- [x] **Step 1: Add a failing parser integration contract**

The new script must start the just-built Meridian-MCP binary in analysis mode and send, in order:

1. `initialize`;
2. `notifications/initialized`;
3. `dm_parse_environment` for the generated DME;
4. `dm_get_type` for first, boundary, and last path;
5. `dm_list_types` for `/datum/mlp` with a bounded page that includes the selected paths, or separate exact lookups when page limits prevent that assertion.

Require `total_types >= declared_type_count`, exact lookup paths, a positive indexed-symbol count, and a bounded completion time. Persist timings and MCP/Spaceman helper provenance, but not generated source.

- [x] **Step 2: Use the release artifact built from the checked-out source**

The parser job must:

- check out the exact pinned SpacemanDMM revision;
- freshly build the dmdoc helper and release Meridian-MCP under Rust 1.95.0;
- set `MERIDIAN_MCP_HELPER_MANIFEST` to the generated manifest;
- run the parser script on Windows and Ubuntu;
- record the MCP SHA, SpacemanDMM SHA, helper hash, platform, and Rust version.

This is the actual proof for the earlier “SpacemanDMM over 64K” requirement. The DreamDaemon fixture alone cannot prove parser compatibility.

- [x] **Step 3: Verify Task 3 locally**

Run:

```powershell
cargo +1.95.0 build --locked --release
pwsh -NoProfile -File scripts/run-large-prototype-parser-integration.ps1 -BinaryPath ./target/release/meridian-mcp.exe -EvidencePath ./integration/evidence/large-prototype-parser-local.json
cargo +1.95.0 test --test workflow_contract --test byond_compatibility_fixture
```

Expected: the three exact lookups pass and the evidence reports at least 65,538 declared fixture types.

## Task 4: Separate product gates from the engine sentinel

**Files:**

- Modify: `.github/workflows/byond-integration.yml`
- Modify: `tests/workflow_contract.rs`
- Modify: `docs/compatibility.md`
- Modify: `README.md`

- [x] **Step 1: Create `windows-meridian-compatibility`**

Move the existing real Windows sequence into its own required job:

1. checkout and provenance validation;
2. exact helper builds;
3. BYOND/runtime/auxtools provisioning;
4. release Meridian-MCP build;
5. owned small runtime fixture compilation;
6. Meridian-Rift parse, lookup, definition, search, direct compile, network `rift_compile`, warm human `BUILD.cmd`, and offline `rift_compile`;
7. live auxtools gate;
8. live Tracy gate;
9. unconditional evidence upload.

Do not place the synthetic over-64K DreamDaemon gate before these steps. Do not give this job a `needs` dependency on a synthetic job.

Add a required assertion that the real Meridian-Rift parse reports more than 65,536 types if the checked-out ref currently does. If the real corpus is below that threshold, record the count without inventing a claim and rely on Task 3 for the parser boundary proof.

- [x] **Step 2: Create `prototype-parser-compatibility`**

Use a Windows/Ubuntu matrix. Both entries are required. They build and test the compact 65,537-leaf fixture through the release MCP/SpacemanDMM parser only; they do not install or start BYOND.

- [x] **Step 3: Create `prototype-runtime-compatibility`**

Use this explicit matrix:

```yaml
strategy:
  fail-fast: false
  matrix:
    include:
      - os: ubuntu-24.04
        required: true
      - os: windows-2025
        required: false
continue-on-error: ${{ !matrix.required }}
```

Each entry installs verified BYOND 516.1687 and runs the 50,000-leaf control followed by the 65,537-leaf boundary case. The boundary case runs only after the control passes. Upload separate evidence for each OS even on failure.

Ubuntu is required because the check concerns DreamDaemon's engine loader and `/tg/station`'s authoritative integration runtime is Ubuntu 24.04. Windows remains visible rather than silently skipped; it is diagnostic until it meets the promotion criterion. The required Windows product path remains covered by `windows-meridian-compatibility` and its real Tracy launch.

- [x] **Step 4: Preserve Linux Tracy as an independent job**

Keep `tracy-linux-live` independent. It must not depend on the prototype parser or runtime jobs. A failure in one specialized lane must not erase evidence from the other lanes.

- [x] **Step 5: Make artifact names unambiguous**

Use separate names:

- `windows-meridian-compatibility-evidence`;
- `prototype-parser-windows-evidence`;
- `prototype-parser-ubuntu-evidence`;
- `prototype-runtime-windows-evidence`;
- `prototype-runtime-ubuntu-evidence`;
- `tracy-linux-compatibility-evidence`.

Do not upload `.tracy`, generated `.dm`, `.dme`, `.dmb`, `.rsc`, or absolute paths.

- [x] **Step 6: Verify Task 4 contracts**

Run:

```powershell
cargo +1.95.0 test --test workflow_contract
git diff --check
```

Manually inspect the workflow graph and confirm no required real-product job has a synthetic `needs` dependency.

## Task 5: Align documentation and status claims

**Files:**

- Modify: `README.md`
- Modify: `docs/compatibility.md`
- Modify: `TESTING.md`
- Modify: `docs/provenance.md`
- Modify: `CHANGELOG.md`

- [x] **Step 1: Correct the compatibility matrix**

Document three separate rows:

| Claim | Required platform | Evidence |
|---|---|---|
| MCP/SpacemanDMM parse and exact lookup above 64K declared fixture types | Windows and Ubuntu | Parser matrix evidence |
| BYOND 516.1687 synthetic over-64K DreamDaemon startup | Ubuntu required; Windows diagnostic until promoted | Runtime matrix evidence |
| Real Meridian-Rift Windows runtime and profiler path | Windows | Meridian compatibility, auxtools, and Tracy evidence |

Remove the current statement that the over-64K world “must” pass on Windows before the repository gate. Preserve the Windows diagnostic status explicitly.

- [x] **Step 2: Explain evidence limitations**

State plainly:

- source declarations are not an authoritative count of every internal BYOND prototype;
- the fixture deliberately guarantees more than 65,536 declared leaf paths;
- parser success does not prove DreamDaemon startup;
- DreamDaemon startup does not prove SpacemanDMM parsing;
- a diagnostic Windows sentinel failure does not become a compatibility pass.

- [x] **Step 3: Record source authority**

Link the official BYOND 516 release note for the 516.1686 defect and the official DreamDaemon/startup option reference. Link `/tg/station`'s Ubuntu integration workflow and `tools/ci/run_server.sh` as the reason Ubuntu is the required synthetic runtime lane.

- [x] **Step 4: Add operator instructions**

Document the manual dispatch, artifact names, decision-table classifications, and the three-consecutive-pass promotion rule. Include the exact `gh workflow run` command only if the repository's existing authentication workflow supports it; otherwise give GitHub UI instructions.

## Task 6: Full verification and one decisive hosted run

**Files:**

- Verify only; modify a failing implementation only after returning to its relevant task and adding or correcting a test.

- [x] **Step 1: Run the exact Rust gates**

```powershell
rustc +1.95.0 --version
cargo +1.95.0 fmt --all -- --check
cargo +1.95.0 clippy --workspace --all-targets --all-features -- -D warnings
cargo +1.95.0 test --workspace --all-features
cargo +1.95.0 deny --all-features check
```

Record the exact compiler version. A green unpinned local `cargo` run is not acceptance evidence.

- [x] **Step 2: Run PowerShell and fixture gates**

```powershell
pwsh -NoProfile -File scripts/test-meridian-evidence-validation.ps1
pwsh -NoProfile -File scripts/audit-spacemandmm-capabilities.ps1 -Check
cargo +1.95.0 test --test workflow_contract --test byond_compatibility_fixture --test process_readiness
git diff --check
```

- [x] **Step 3: Run the local Windows live path**

With BYOND 516.1687 and the exact installed release binary:

1. run the 50,000-leaf control once;
2. run the 65,537-leaf boundary fixture three times;
3. run the parser boundary fixture once;
4. run the owned small runtime fixture with Topic and stop;
5. verify no owned DreamDaemon process remains after each run.

Record elapsed startup times. Do not generalize from local Windows success to hosted Windows success.

- [ ] **Step 4: Dispatch the hosted workflow**

Run `BYOND integration` manually against the intended Meridian-Rift ref. Do not open a PR merely to trigger it.

Acceptance requires:

- both parser matrix entries green;
- Ubuntu control and boundary runtime cases green;
- Windows real Meridian-Rift, auxtools, and Tracy job green;
- Linux Tracy job green;
- Windows synthetic result visible with a specific classification and complete evidence;
- no later job skipped because an earlier specialized job failed.

- [ ] **Step 5: Handle the Windows sentinel result**

- If Windows control and boundary pass, begin the three-run promotion count.
- If control passes and boundary fails, retain the exact fixture/evidence and prepare a minimal upstream BYOND report; do not keep mutating Meridian-MCP launch logic.
- If the control fails, investigate Windows runner prerequisites/launch behavior using the captured event and process evidence.
- If the process shows sustained progress at 300 seconds, propose a measured timeout adjustment in a separate review; do not silently increase it during this implementation.

- [x] **Step 6: Final local review**

Confirm:

```powershell
git status --short
git diff --stat
git diff --check
```

Report the exact hosted run URL, job outcomes, Windows sentinel classification, and any remaining provisional claim. Leave changes in the working tree for user review.

## Research basis

- BYOND 516.1686 release note: worlds over 64K prototypes could sometimes crash or hang on DreamDaemon startup.
- BYOND startup reference: port `0` selects any available port; `-close`, `-log`, `-logself`, `-trusted`, and `-verbose` are documented options.
- `/tg/station` integration: Ubuntu 24.04 with a 55-minute job ceiling and a direct blocking `DreamDaemon tgstation.dmb -close -trusted -verbose ...` launch.

These sources justify the boundary test and the Ubuntu engine lane. They do not prove that the current Windows timeout is a BYOND regression; the control/boundary matrix is required for that conclusion.
