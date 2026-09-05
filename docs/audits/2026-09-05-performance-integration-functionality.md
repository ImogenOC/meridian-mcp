# Meridian-MCP performance, integration, and functionality audit

Date: 2026-09-05. Scope: read-only implementation audit, local verification, and a remediation workplan. No production source or tests were changed. Findings describe the existing implementation; none is a claim that remediation has been applied.

## Assessment

The portable test suite and both maintained stdio smoke checks pass. Exact symbol retrieval and unchanged snapshot reuse work at Meridian-Rift scale. The highest-priority defects are incomplete enforcement of configured roots and compiler allowlists, an insufficient basis for marking compiled artifacts verified, and missing ownership cleanup for standard DreamDaemon sessions. Fix these before expanding runtime or profiler use.

Ten findings follow. Four include direct stdio reproductions; the remaining findings are traced through current source and require the regression gates in the workplan. This is a broad audit, not exhaustive certification of every tool or native helper.

## Baseline and evidence

- Audited checkout: `48d6d54160959eafcc4992e20e56cc586437709f`, branch `main`, initially clean.
- Existing release binary reports source revision `f4d5e49ca2baa605b7ad28bdc06ced7e02e466fc`, clean release profile, SHA-256 `7b8147ab7416964b12abb1130b0b89c757812ece1fe4420169a4d65c21035d55`.
- `git diff f4d5e49ca2baa605b7ad28bdc06ced7e02e466fc HEAD` contains only 18 added README lines. Executable source and dependency files match; this is still an existing-binary test, not a fresh HEAD release build.
- Rust: `1.95.0 (59807616e 2026-04-14)`, `x86_64-pc-windows-msvc`, matching `rust-toolchain.toml` and CI.
- Meridian-Rift corpus: clean `ic-spawning-stupidity` checkout. Its feature set differs from the historical Dogmos benchmark corpus.

| Gate | Fresh result | Boundary |
| --- | --- | --- |
| `cargo +1.95.0 fmt --all -- --check` | Passed | Formatting only |
| `cargo +1.95.0 test --locked --offline --all-features` | 310 passed, 0 failed, 3 ignored | Local Rust/fixture gates; ignored tests remain unrun |
| `cargo +1.95.0 clippy --locked --offline --all-targets --all-features -- -D warnings` | Passed | Exact pinned compiler, local Windows |
| Maintained analysis stdio smoke | Passed; 24 tools, process exit 0 | Owned language fixture, cached diagnostics, ranked search, protocol `2024-11-05` |
| Maintained development stdio smoke | Passed; 33 tools, process exit 0 | Inventory and no-process error paths; no DreamDaemon launch |
| Independent analysis stdio probe | Negotiated `2025-11-25` and reproduced F1/F5/F6 | Existing release binary; disposable fixtures |
| Real Meridian-Rift parse/reuse/search | Completed | Parser/DreamChecker evidence, not game compile or runtime acceptance |
| Owned DreamMaker fixture compile | Inconclusive: no output/artifacts, owned stalled process stopped | No successful DreamMaker or DreamDaemon gate claimed |
| Tracy/auxtools native rebuild and live capture | Unrun | Separate native and live gates required |
| Dependency advisory gate / fresh hosted CI | Unrun | No current vulnerability or hosted-CI claim |
| In-app Meridian-MCP tools | Not exposed in this task | Standalone stdio works; app integration remains unverified |

The first Rust invocation selected a Git-distributed `link.exe` and failed before tests. Loading the installed Visual Studio Developer PowerShell environment resolved that environment issue; the complete test and Clippy runs above then passed. A development probe with an incomplete environment timed out and was not used as defect evidence.

Raw local logs are ignored audit outputs under `target/`: `audit-tests-20260905.log`, `audit-clippy-20260905.log`, `audit-smoke-analysis-20260905.log`, and `audit-smoke-development-20260905.log`. The disposable fixture directory uses the `target/audit-20260905-*` prefix. These logs are local evidence and may contain host paths; this report and the workplan omit those paths. An automatically rejected recursive cache cleanup was replaced with a reversible move of the generated cache into that ignored audit directory.

## Findings

### F1 — P1: Transitive parser reads bypass configured workspace roots

**Evidence:** reproduced through stdio. References: [argument containment](../../src/tools/mod.rs), lines 1143–1145; [parser construction](../../src/tools/parse.rs), lines 181–190; [source extraction](../../src/tools/parse.rs), lines 507–518.

Only the requested DME is checked with the startup `PathPolicy`. The parser receives no such policy and follows `#include` paths itself. A DME in the sole configured `allowed/` root containing `#include "../external.dm"` successfully parsed a sibling fixture. `dm_get_proc` then returned its `AUDIT_OUTSIDE_ROOT` marker from outside the configured root. The test used only audit-owned files, not private data.

**Impact:** analysis mode does not enforce its advertised transitive read boundary. Checking only the top-level tool argument is insufficient. Icon and map resource resolution should be reviewed under the same rule.

**Acceptance:** reject an out-of-root include before exposing its contents, preserve the previous snapshot, and support an include in a separately authorized root. Enforcement must cover actual parser reads, including conditional includes and canonicalized links; checking a completed index alone cannot prevent the read.

### F2 — P1: Omitting `compiler_path` bypasses compiler allowlisting

**Evidence:** reproduced through the maintained stdio session module. References: [optional compiler validation](../../src/tools/mod.rs), lines 1192–1198; [default compiler selection](../../src/tools/compile.rs), lines 301–307; [spawn](../../src/tools/compile.rs), lines 369–380.

An explicitly supplied compiler passes through `policy.executable`. When omitted, `find_dm_compiler()` chooses a conventional installation or PATH executable, canonicalizes it, and spawns it without an allowlist check. `run_contained_process` provides process containment, not executable policy validation.

With an empty compiler allowlist, explicitly naming the installed DreamMaker returned `executable_not_allowed`. Omitting that field selected the same executable and returned process evidence with `termination: wall_timeout`, `duration_ms: 1`, and that compiler's path. The one-millisecond budget bounded the probe; no compiled artifact was produced. Local evidence: `target/audit-compiler-policy-20260905.json`.

**Impact:** a development session with an empty allowlist, or one authorizing a different compiler, can attempt to run the discovered compiler. This also risks compiling with a different BYOND version than the selected installation.

**Acceptance:** validate the final selected executable for both branches before spawn; select the sole configured compiler by default or return an actionable missing/ambiguous configuration error. Negative tests must assert that no process was created.

### F3 — P1: Build provenance can certify an incomplete or mismatched source closure

**Evidence:** source-level finding. References: [snapshot acquisition](../../src/tools/compile.rs), line 285; [post-compile provenance](../../src/tools/compile.rs), lines 488–545 and 581–602; [launch validation](../../src/build_provenance.rs), lines 215–231.

`record_compile_provenance` requires only that the snapshot DME path matches. It hashes the snapshot's old input list after compilation, without proving that the list matches the inputs the compiler actually read. For example: parse a DME, add an include, then compile without reparsing. The recorded DME hash is current, but the newly included file is absent from the old snapshot list. Later edits to that file are not covered by the recorded input checks. Compiler `defines` can independently change the active include set; those arguments are not represented in `BuildRecord`.

Hashing only after compilation also cannot establish which bytes a compiler read if a source file changes during its run. This is especially relevant to an agent workspace where editing and building can overlap.

**Impact:** `provenance.status = verified` can overstate the evidence, and a later launch can miss a stale input. The shared snapshot input list also inherits F5's omissions.

**Acceptance:** capture the effective build configuration and complete input identities before spawn, verify stability afterward, and withhold verified status when the actual conditional closure cannot be established. Cover added includes, define-selected includes, and changes during compilation. Preserve previous artifacts and failed-attempt history.

### F4 — P1: Standard DreamDaemon ownership does not survive MCP shutdown

**Evidence:** source-level finding; live reproduction unrun. References: [standard spawn](../../src/tools/runtime.rs), lines 258–277; [runtime stop](../../src/state.rs), lines 303–314; [stdio shutdown](../../src/mcp.rs), lines 17–20. Compare [contained process runner](../../src/process.rs), line 156, and [debugger launch](../../src/tools/debugger.rs), line 91.

The standard DreamDaemon `Command` has neither `kill_on_drop(true)` nor a retained `ProcessContainment`. `RuntimeState` has no drop cleanup, and `run_server` simply returns after the SDK service finishes. Explicit stop kills only the stored child. Debugger and compiler launches already have stronger ownership handling.

**Impact:** dropping the server after EOF/disconnect can leave its game process running, holding a port and executing against the workspace. Descendants are not covered by standard stop. An integrity journal can describe an interrupted owner without stopping the child.

**Acceptance:** prove cleanup of the owned process tree after normal stop, transport EOF, cancellation during launch, and forced owner termination; retain process identity so unrelated processes cannot be targeted. Verify journal finalization/recovery separately from termination.

### F5 — P2: Snapshot reuse omits external inputs and newly appearing configuration

**Evidence:** reproduced through stdio. References: [input collection](../../src/analysis_snapshot.rs), lines 285–306; [profile discovery](../../src/project.rs), lines 15–38; [reuse](../../src/tools/parse.rs), lines 338–350.

`build_source_inputs` drops every canonical file outside the DME parent, even when another configured root legitimately authorizes that file. After the F1 fixture's external variable changed from `1` to `2`, reparse returned `reused: true`, generation 1, and `dm_get_var` still returned `Float(1.0)`. `dm_get_proc` read the changed file directly, so one response set mixed old semantic data with current source text.

Configuration is added to the fingerprint only if it existed during profile discovery. Creating `SpacemanDMM.toml` after a successful parse also returned `reused: true` with no generation change. Configuration discovery itself was therefore skipped.

**Impact:** normal reparse can retain outdated declarations or analysis settings. Fixing F1 by supporting multiple authorized roots must not retain the DME-parent fingerprint filter.

**Acceptance:** fingerprint every authorized registered input, track configuration existence/discovery changes, and test add/change/delete transitions. Document the metadata-only fingerprint's remaining limitation when content is deliberately changed without changing length or modification time. Source responses should identify or avoid mixed snapshot/disk generations.

### F6 — P2: Parse budgets exclude queueing, and cancellation can release serialization early

**Evidence:** queue delay reproduced; cancellation branch traced in source. References: [parse permit](../../src/tools/parse.rs), lines 134–142; [blocking worker and timeout](../../src/tools/parse.rs), lines 181–267.

The timeout starts after waiting for `parse_permit` and after reuse validation. In the disposable 18,000-type fixture, a parse with `timeout_ms: 1` queued behind a previously timed-out worker and returned after 454 ms. A caller behind a much longer worker has no bounded queue wait.

The explicit timeout branch moves the permit into a detached waiter, which is good. Dropping/cancelling the request future before that branch instead drops the permit while the non-abortable worker continues. Another parse can then overlap it. `reusable_snapshot` also performs synchronous filesystem metadata reads directly in the async request.

**Impact:** the advertised timeout does not bound request latency, and cancellation can defeat the protection against overlapping parse memory peaks.

**Acceptance:** enforce one request deadline across queueing and validation, and make worker ownership of the permit independent of request-future lifetime. Use a deterministic blocked worker to test cancellation, not a timing-only race. Move synchronous validation off the async executor.

### F7 — P2: Output waits block runtime status and stop

**Evidence:** source-level finding. References: [output polling](../../src/tools/runtime.rs), lines 510–550 and 563–607; [stop/status lock acquisition](../../src/tools/runtime.rs), lines 855–893.

`dm_wait_for_output` holds the runtime mutex for the entire polling loop, including sleeps, up to 300 seconds. `dm_stop`, `dm_status`, and the runtime part of `dm_server_status` require the same lock. Launch readiness also holds runtime state while waiting.

**Impact:** a client cannot promptly inspect or stop a running process when another request is waiting for a missing output marker. This undermines recovery precisely when startup or runtime behavior stalls.

**Acceptance:** wait on cloned bounded output/notification state, acquire runtime ownership only for short state transitions, and prove that a five-minute output wait does not delay status or stop beyond a short control deadline.

### F8 — P2: DMI limits are applied after allocation and do not bound scan residency

**Evidence:** source-level finding. References: [decode](../../src/spaceman/dmi.rs), lines 114–149; [scan retention](../../src/tools/dmi.rs), lines 191–205; [limits](../../src/limits.rs), lines 50–63.

`prepare_dmi` reads the entire file before checking its 64 MiB limit and decodes the image before checking the pixel limit. During duplicate scanning, every decoded asset is retained in the local `assets` vector. Evicting it from the 512 MiB cache does not free an image still held by that vector. `max_blocking_jobs` is defined as four but is not used anywhere else in `src`.

**Impact:** valid large asset sets, oversized files, or several concurrent requests can exceed the apparent memory/concurrency ceilings before the tool rejects them. This is a resource-boundary defect; no destructive oversized-input stress test was run.

**Acceptance:** limit reads before full allocation, reject oversized dimensions before pixel allocation, apply an enforced blocking-work budget, and bound total live decoded assets across a scan. Test boundaries with small injected limits and allocation/peak-memory evidence.

### F9 — P2: DMI cache hits still pay for decoding

**Evidence:** source-level finding. References: [tool load](../../src/tools/dmi.rs), lines 21–28; [cache installation](../../src/spaceman/dmi.rs), lines 60–76 and 114–121; [icon-reference loop](../../src/tools/dmi.rs), lines 320–356.

Every load calls `prepare_dmi`, which reads, hashes, and decodes the image. Only afterward does `DmiCache::install` check for an identical hash and return the cached `Arc`. `audit_icons` repeats `load` for each static icon-state reference, including many references to the same file.

**Impact:** the cache preserves asset identity and generation but does not avoid the expensive decode work it appears intended to cache. A large icon audit can decode the same image repeatedly. This was not quantified with a matched icon benchmark.

**Acceptance:** check content identity against the cache before decode, coalesce same-path loads, and reuse one decoded asset per unique file within an audit. Preserve content-hash correctness for metadata-preserving edits. Prove decode counts as well as returned generations.

### F10 — P2: Tracy control timeouts do not cover blocked writes

**Evidence:** source-level finding; live collector fault injection unrun. References: [transport request](../../src/tracy_collector.rs), lines 184–225; [collector stop](../../src/tracy_collector.rs), lines 410–431.

`request_with_timeout` waits for the writer mutex and completes `write_all` before starting its timeout around the response receiver. A collector that stops reading stdin can therefore block a request indefinitely. `stop` awaits this same request before reaching child termination. The response reader also uses unrestricted `read_until` before checking its byte limit (lines 94–125).

**Impact:** status/cancel/stop are not fully bounded under collector backpressure, despite using short response timeouts. Oversized unterminated responses can allocate beyond the declared response ceiling.

**Acceptance:** apply one deadline to writer acquisition, write, and response; clean pending entries on every terminal path; bound framing while reading; and enforce collector termination independently of successful protocol I/O.

## Performance observations

One release stdio run on the current Meridian-Rift checkout produced:

| Measurement | Observation |
| --- | --- |
| Types / lexical documents | 65,165 / 452,780 |
| Cold parse, server timing | 77,248 ms |
| Preprocess/parse | 28,621 ms |
| DreamChecker | 11,674 ms |
| Search document construction | 20,460 ms |
| Analysis indexes | 15,119 ms |
| Initial fingerprint | 1,062 ms |
| Warm reuse | 1,012 ms; unchanged generation 1 |
| Ten stdio query observations | 1–85 ms; sorted middle pair 6 and 9 ms |
| Exact mapping query | 1 ms; one candidate, one scored document; correct symbol |
| Broad icon-state query | 80 ms; 102,103 candidates/scored documents |
| One in-flight memory sample | About 1.86 GB working set and 2.00 GB private bytes |
| Cached diagnostics | 1,039 total: 127 errors and 912 hints; pagination worked |

Cold parsing overlapped Rust compilation and the memory sample was not a peak measurement. These observations do not establish a regression against older runs. The historical `dogmos` query had no results in this checkout; that is not evidence of a ranking defect. The checked-in scale test hard-codes a Dogmos assertion, so use the labeled fixture for relevance acceptance and explicitly identify the real corpus for performance comparisons.

Warm source reuse and exact lookup are worth preserving. Measure repeated clean-host cold/warm runs before choosing a general parser optimization. DMI decode avoidance and bounded lifecycle controls have direct code-level reasons for improvement.

## Remediation handoff

The [workplan](../superpowers/plans/2026-09-05-mcp-audit-remediation.md) orders independent fixes, names affected files and regression cases, and separates portable, native, installed, full-corpus, and live acceptance gates. Execution requires a separate implementation request. Leave subsequent implementation uncommitted unless explicitly authorized.
