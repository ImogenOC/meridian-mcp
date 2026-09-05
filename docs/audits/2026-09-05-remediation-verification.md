# MCP audit remediation verification — 2026-09-05

Tasks 1–9, including the added Unix ownership Task 5b, are implemented and independently reviewed. Authorized local qualification passed. The debugger stop latency observed below remains a follow-up; hosted, installation and real-game gates remain separate.

## Scope and identity

This executes the [audit remediation plan](../superpowers/plans/2026-09-05-mcp-audit-remediation.md) against baseline `48d6d54160959eafcc4992e20e56cc586437709f`. Changes remain uncommitted in the existing checkout. No installed MCP was replaced, no application configuration changed, and no primary game source was edited.

Rust is pinned to `1.95.0 (59807616e 2026-04-14)`. Local Windows checks use the x64 MSVC development environment. Local Linux checks use Ubuntu 24.04 under WSL1, a separate Cargo target directory, and the same pinned Rust version. These are local platform results, not hosted CI results.

SpacemanDMM remains at `351ddc0ffb2439876d4565ce5130bb6b027ee605`. Only `dreammaker` and `dmm-tools` carry the [documented local read-policy delta](../../vendor/spacemandmm/README.md), `meridian-read-policy-v2`, SHA-256 `9c634bdc169e3305c6e2845e5a38f2e1f59224a462e0f56969ff422a8657c20e`. Replaying that complete delta against the exact upstream baseline reproduced all 50 shipped source inventory hashes. Neighbor dependencies retain the exact upstream revision and matching explicit versions.

## Remediation acceptance

| Finding | Implemented behavior | Focused evidence |
| --- | --- | --- |
| F1, transitive reads | Actual parser/configuration, source, DMI traversal and map icon opens retain immutable startup policy; denied parses do not publish a partial snapshot | Authorized external roots, denied includes/configuration/resources, canonical links and preserved snapshots; vendor inventory detects changed, extra, hidden and missing source |
| F2, compiler selection | Explicit and implicit final executables pass the same policy; omitted path requires exactly one configured compiler | Empty/ambiguous/denied/stale compiler cases and no-spawn assertions |
| F3, provenance | Schema-2 pre-spawn arguments/compiler/input byte identities; conservative proved closure; unproved builds stay unverified | Added includes, compiler-only conditionals, mid-compile mutation, failed/unverified attempts and legacy record behavior |
| F4, runtime ownership | Windows owner/runtime jobs and Linux lifetime-pipe guardian; retained containment through confirmed cleanup | Native owner termination, EOF, transport error, cancellation, descendants, unrelated sentinel and cleanup failure/retry fixtures on both platforms |
| F5, snapshot identity | All authorized registered inputs and absent configuration candidates participate in reuse; proc excerpts stay with their snapshot | External edits, config appearance/change/removal, alias identity, missing inputs and excerpt generation |
| F6, parse admission | One deadline covers admission, blocking validation/build and publication; worker retains admission after caller drop; timeout metadata does not await analysis locks | Windows parser 25 / Linux parser 24; snapshot integrations 13 each; deterministic queue, cancellation, warm validation and held-publication-lock cases |
| F7, runtime controls | Session-specific output notifications release the lifecycle mutex during waits/readiness | Five-minute missing-marker wait does not block status/stop; old wait cannot consume or stop replacement session |
| F8/F9, DMI allocation/cache | Bounded reads and metadata/IDAT inflation; hash-before-decode; worker-owned admission; aggregate scan pixel/metadata/state/frame limits | 20 DMI plus 1 map test on each platform; bounded-reader and cancellation units; legacy PNG/metadata parity; independent review approved |
| F10, Tracy transport | Total write/response deadline, bounded framing and pending IDs, independent actual-child supervisor, retryable confirmed cleanup | Blocked duplex write, cancellation/late replies, oversized unterminated frames, healthy EOF, failed force then Drop, confirmation failure and journal retention |

## Interpretation limits

- Canonicalization-before-open policy does not provide operating-system handle isolation against concurrent malicious path replacement.
- Snapshot reuse uses metadata and alias identity. Deliberate byte changes preserving length and modification time remain outside that fingerprint's guarantee.
- Verified compilation supports the conservative closure described in [provenance](../provenance.md#managed-build-provenance). General game builds and Rift generator closure remain unverified. Pre/post hashes do not prove a concurrent change-and-restore never occurred.
- [Runtime ownership](../runtime-ownership.md) covers cooperative owned runtimes. Windows completion checks the observed job members before termination; concurrent child creation during that collection is not a hostile-tree guarantee. Linux requires readable procfs and inherited process groups; deliberate group escape or independent termination of both guardian and owner is outside the guarantee.
- Parser cancellation abandons results while the non-abortable worker retains admission. It does not interrupt SpacemanDMM or a blocked filesystem operation.
- Collector stop can return a bounded error before the OS confirms termination. Ownership and the integrity journal remain retryable; only confirmed cleanup permits finalization. Generic Unix collector ownership covers the direct child, separately from DreamDaemon's guardian.
- Real-game profiling, destructive full-build compatibility in a separately authorized disposable checkout, hosted CI, installation and post-restart app-tool acceptance remain separate gates.

## Native and live qualification

The owned Windows DreamMaker fixture initially hung without output in the sandbox context. The same maintained script succeeded in the approved native user context and produced a fresh DMB. Subsequent real-engine qualification uses that context; no loader root cause or installation repair is inferred from this distinction.

A fresh pinned Tracy native build passed all four Meridian-owned CTests. The x64 helper SHA-256 is `e291902b6ef831a7ba5fd413287b112c11e832407537e8d95fdd014a6efab194`; the x86 hook SHA-256 is `c6bc531d2ffc33603ca28c6c42d4377f19ac0d977bf7171b7cc54723355006a8`. Native compilation and tests alone do not establish a live capture. Windows owned live capture passed on the final executable; Linux BYOND live execution is unavailable on this host.

The Linux native helper/hook build also passed all four owned CTests. Its helper SHA-256 is `d42fd36bfbf4b27a503635da494b7e55b580b74fda34ceade7a1566f306b9733`; hook SHA-256 is `e78f027dee2451b506582819725a57eeebdeb6260c941d243b70f723b5df9618`. Portable checksum-verified CMake 3.31.6 and Ninja 1.13.1 were supplied under the ignored build directory, using the existing 32-bit compiler. Clean LF copies of the exact third-party revisions avoided cross-platform checkout normalization differences. The empty-queue patch's working bytes were restored to its checked-in LF form before both final builds; no semantic patch or upstream pin changed.

A purpose-written x86 C++ probe compiles successfully but fails execution on this WSL1 host with `Exec format error`. Linux BYOND live qualification is therefore unavailable here; native x64 helper tests and x86 hook compilation do not establish live x86 execution. No system packages or WSL configuration were changed to bypass that boundary.

Raw logs and scratch fixtures remain ignored under `target/` and `.superpowers/sdd/`. They may contain local paths. This report records repository-relative evidence and portable identities only.


## Final executable and gate matrix

The release scale-test build relinked the Windows entry point. Stdio, runtime, provenance, debugger and live Tracy checks were repeated on the final bytes. Earlier `0d087a…` / build ID `60d8e4…` evidence is historical. No installation was performed.

| Executable | SHA-256 |
| --- | --- |
| `target/release/meridian-mcp.exe` | `c38c3ed8cd5115c7d812206de813b4baa357fbb90cb1cb97e9b20a331b9a29f8` |
| `target/remediation-linux/release/meridian-mcp` | `e4839e120677f523646672a46a0fb869a72ae332e62ecee8099696927ccd6671` |

Final Windows MCP build ID: `40c46323b8b29ee69c0b6aa31b83f77515e20b9744659b9f5080fb2017c81392`.

| Gate | Result and evidence under `target/` |
| --- | --- |
| Windows full locked all-feature Rust suite | 388 passed, 4 ignored, 52 result groups; `task-10-final-windows-tests-v3.log` |
| Linux full locked all-feature Rust suite | 381 passed, 5 ignored, 52 result groups; `task-10-final-linux-tests-v2.log` |
| Pinned rustfmt and strict all-target/all-feature Clippy | Both platforms passed; `task-10-final-fmt.log`, `task-10-final-windows-clippy.log`, `task-10-final-linux-{fmt,clippy}.log` |
| Release compilation | Both platforms passed; final Windows relink also built by the release scale gate |
| Dependency policy | Fresh advisories/bans/licenses/sources passed with existing checked-in exceptions and warnings; `task-10-final-deny.log` |
| Generated contracts and documentation | Regenerated output had no drift; contract/documentation tests passed in both full suites |
| Vendor source identity | Complete patch replay matched all 50 inventory files; capability audit passed 45 records |
| Owned configuration and evidence validators | Roundtrip configuration plus Meridian/provenance evidence validators passed; actual app configuration unchanged |
| Release stdio | Windows and Linux: 33 development tools, 24 analysis tools, language parse/cached diagnostics/search and EOF exit passed; final Windows `*-stdio-*-v2.log`, Linux `*-stdio-*.log` |
| Windows owned runtime | Readiness, Topic ping/pong, handshake classification, stop and map inspection passed; `task-10-final-windows-runtime-stdio-v2.log` |
| Managed provenance and integrity | Fresh/restored builds launch; changed inputs and restarted stale records reject; journal finalized and zero owned processes remain; `task-10-final-provenance-integrity-v2.json` |
| Pinned auxtools v2.3.7 | Six of six requests passed on the headless owned fixture; `task-10-final-auxtools-v2.json`. Stop took 30,027 ms; this is a remaining latency issue |
| Native Tracy | Windows and Linux each passed all four owned CTests on fresh pinned helper/hook builds |
| Final Windows live Tracy | Four 30-second captures plus 120-second drain interval passed; `task-10-final-tracy-v2.json` and preserved `task-10-final-tracy-artifacts-v2/` |
| Portable unsupported Rift behavior | Linux stale-schema rejection passed; `task-10-final-linux-unsupported-rift.log` |
| Labeled retrieval | Exact-identifier MRR 1.0 and natural-language recall@10 1.0; `task-10-final-search-relevance.log` |
| Real-corpus scale gate | Passed on the selected corpus: cold 30,266 ms, warm 1,275 ms; `task-10-final-scale.log` |

Ignored counts include child-fixture entry points exercised by parent tests and the separately run corpus scale gate. They are not omitted failing tests. Two initial Windows broad runs hit timing limits under concurrent build load: an unchanged private-state lock test's two-second completion bound and an owned runtime's three-second startup bound. The latter fixture's parent-to-descendant creation interval was 3.5184201 seconds, before cleanup assertions; both failed-fixture PIDs were subsequently absent. The unchanged private-state suite passed 4/4 in isolation, the full library passed 141/141 in isolation, and the final full Windows rerun passed. The initial failures remain in `task-10-final-windows-tests.log` and `task-10-final-windows-tests-v2.log`; no deadline was relaxed.

### Real-corpus measurements

Five sequential fresh analysis processes used the final Windows executable and clean Meridian-Rift commit `7462a6942b2e71a3ea13c00169f65f575cb281b7`. Commit and dirty state were checked before and after each run. No concurrent build, test or capture was running. A fresh process gives a cold application snapshot; the operating-system file cache was not flushed and warmed across runs. The corpus has no Dogmos. All runs reported 65,165 types, 452,780 symbols and 127 parser error diagnostics with zero warnings, matching the audit's error count. This does not establish a DreamMaker game compile.

| Measurement | Samples | Median | p95 | p99 |
| --- | --- | --- | --- | --- |
| Cold parse request | 5 | 24,822.19 ms | 25,151.15 ms | 25,151.15 ms |
| Warm reuse request | 5 | 1,245.83 ms | 1,272.20 ms | 1,272.20 ms |
| Search request | 50 | 3.85 ms | 47.53 ms | 50.15 ms |
| Per-process peak working set | 5 | 1,782.50 MiB | 1,783.95 MiB | 1,783.95 MiB |

Percentiles use nearest rank; with five samples p95 and p99 are both the maximum. Request timings include stdio and harness serialization/parsing. Memory uses Windows `PeakWorkingSet64`, sampled during requests; it is not private allocation or total machine memory. Median cold server stages were preprocess/parse 11,032 ms, DreamChecker 3,377 ms, search documents 3,109 ms, analysis indexes 5,984 ms and initial fingerprint 1,290 ms. Admission wait was zero. Stage medians need not sum to the median total. Warm reuse retained the same generation in every run.

| Query | Median ms | Maximum ms | Candidates / scored | Stable top symbol |
| --- | --- | --- | --- | --- |
| dogmos | 47.53 | 50.15 | 0 | No result |
| /datum/controller/subsystem/mapping | 0.68 | 0.99 | 1 | `/datum/controller/subsystem/mapping` |
| native dog library health detection | 3.14 | 3.36 | 4,879 | `/mob/living/basic/pet/dog/breaddog/var/health` |
| find references to icon state | 39.10 | 43.39 | 102,103 | `/datum/controller/subsystem/spatial_grid/proc/find_hanging_cell_refs_for_movable` |
| air temperature reset | 2.91 | 3.41 | 5,607 | `/obj/machinery/air_sensor/proc/reset` |
| bluespace personal cache | 2.46 | 3.46 | 4,142 | `/datum/job/personal_ai` |
| camera network visibility | 2.21 | 13.29 | 3,003 | `/mob/eye/camera/ai/proc/update_visibility` |
| liquid turf processing | 7.93 | 8.30 | 20,540 | `/obj/effect/abstract/liquid_turf/proc/ChangeToNewTurf` |
| admin technology | 3.97 | 4.28 | 10,022 | `/obj/item/scalpel/advanced/alien/admin/var/desc` |
| move manager path | 4.86 | 5.35 | 11,678 | `/datum/move_manager/proc/frustrations_move` |

These ten queries are latency probes; the separate labeled fixture supplies relevance acceptance. Raw runs and their summary are `task-10-final-analysis-measurements.json` and `task-10-final-analysis-summary.json`. The original audit's cold parse overlapped compilation, so it is not a matched baseline for claiming a parser speedup. The named scale assertion and these five runs are separate observations.

The final isolated DMI fixture measured cold 11.7467 ms and warm 2.5464 ms, one decoder call, shared cached identity and 131,072 live decoded bytes. Pixels, metadata and profile JSON remained equivalent to the legacy decoder. This is one debug-fixture observation, not a production throughput claim; enforced limits and decoder avoidance have separate regression coverage. See `task-10-final-dmi-measurement.log`.

## Final live Tracy observations

The maintained `scripts/run-tracy-integration.ps1` assertions and default durations were preserved. An ignored scratch wrapper only resolves the repository script root and copies owned artifacts/responses before the normal finalizer removes them. The final run captured one immediate and three steady-state traces, separated by the required 120-second drain interval.

All four traces have 299 complete frames, the expected fixture proc and valid hash-bound schema-2 sidecars. Queue high water was 27 of 262,144 events, with zero saturation and zero drops. Exact-zone/frame queries succeeded, self-comparison produced zero deltas, and the drain worker was attached and ready before stop. Both status requests reported 0 ms at integer-millisecond resolution; stop took 521 ms. The integrity journal finalized and both recorded owned PIDs were absent after completion. This establishes the owned fixture's behavior, not a real-game control baseline.

| Role | Samples | Peak working set | Peak private bytes | Missed samples |
| --- | --- | --- | --- | --- |
| DreamDaemon | 492 | 30.90 MiB | 16.47 MiB | 0 |
| Collector | 492 | 7.04 MiB | 68.35 MiB | 0 |

Role metrics remain separate. These are launch-through-stop Windows measurements; private bytes and working set are different quantities. Final evidence is `target/task-10-final-tracy-v2.json`, `target/task-10-final-tracy-cleanup-v2.json`, and `target/task-10-final-tracy-artifacts-v2/`.
## Remaining work and deployment handoff

- **Debugger stop latency:** the owned auxtools run consistently spends about 30 seconds in stop. In [src/spaceman/debugger.rs](../../src/spaceman/debugger.rs), `AuxConnection::disconnect` awaits the normal response timeout before `DebuggerSession::stop` terminates the child. Add a nonresponsive-disconnect regression, establish one stop budget independent of protocol I/O, and verify actual cleanup before success. This separate debugger transport was outside Tasks 1–9; no debugger source was changed here.
- **Linux live BYOND/Tracy:** rerun on native Linux or a host that executes x86 ELF. The current WSL1 x86 probe fails with `Exec format error`; x64 Rust/native results and x86 hook compilation remain valid separate evidence.
- **Hosted and full-game acceptance:** hosted Windows/Linux CI, destructive full-build compatibility in an explicitly authorized disposable checkout, and real Meridian-Rift profiling remain unrun.
- **Deployment:** no installed binary or app configuration was changed. If installation is requested, prepare the exact binary/helper/configuration diff against the registered setup, then fully quit and reopen Codex and verify server status, parse, repeated reuse, cached diagnostics and search through the newly exposed app tools. Standalone stdio does not establish that post-restart acceptance.

The repository work remains uncommitted. The updated [workplan](../superpowers/plans/2026-09-05-mcp-audit-remediation.md) separates completed local work from these follow-ups.
