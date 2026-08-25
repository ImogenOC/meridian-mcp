# Tracy Profiler Integration Design

**Date:** 2026-08-25

## Goal

Add an opt-in, bounded Tracy profiling subsystem to Meridian-MCP for BYOND/DreamMaker workloads. The subsystem must launch and own a profiled DreamDaemon, capture a trace, inspect it through typed MCP tools, and record exact supply-chain and compatibility evidence without changing Meridian-Rift's human-authored build entry points.

## Decisions

- Pin Tracy to `v0.14.0` (`099df3de3dc37eca4712c06b8320fb9c53596edd`) and protocol 82, matching byond-tracy's declared server compatibility rather than assuming protocol identity is sufficient.
- Pin byond-tracy to `build-d1ec404` (`d1ec404737b04b1ea73d6df4a1b477deacdb1900`).
- Treat BYOND 516.1685 as the initial compatibility target, not a universal compatibility claim.
- Reuse Tracy's C++ `TracyServer` and `Worker` implementation through a Meridian-owned fixed-command helper.
- Do not embed the upstream Python MCP wrapper. Its arbitrary Python evaluation, shared service state, and unrestricted paths do not meet Meridian-MCP's contract.
- Reuse the checked-in `python/bindings/ServerModule.cpp` query implementations as an API reference, but do not depend on the Python module at runtime. The binding is buildable from the pinned source; its general-purpose Python exposure is broader than Meridian-MCP requires.
- Build a 64-bit `meridian-tracy-helper` and a 32-bit `prof.dll` or `libprof.so`.
- Keep Tracy disabled unless `MERIDIAN_MCP_TRACY=byond` is explicitly configured in development mode with a valid helper manifest.
- Perform no runtime downloads, builds, or hidden worktree changes.

## Public tools

| Tool | Purpose |
|---|---|
| `dm_tracy_prepare` | Atomically install the verified BYOND hook beside a selected DMB. |
| `dm_tracy_launch` | Launch an MCP-owned DreamDaemon with fixed Tracy parameters and loopback endpoint. |
| `dm_tracy_capture` | Run one bounded live capture and atomically promote the completed trace. |
| `dm_tracy_status` | Report runtime, capture, endpoint, helper, hook, and error state. |
| `dm_tracy_stop` | Stop capture first and then the owned DreamDaemon. |
| `dm_tracy_hotspots` | Return bounded inclusive, self, count, and duration statistics. |
| `dm_tracy_zone` | Inspect one proc or source location with bounded child statistics. |
| `dm_tracy_frame_stats` | Summarize ServerTick frame durations and percentiles. |
| `dm_tracy_compare` | Compare two traces and report bounded regressions and improvements. |

All tools are development-only and experimental initially. Offline trace analysis may run without a parsed DM environment; a current analysis snapshot adds definition and source correlation.

## Helper protocol

The helper is a one-shot executable. Rust writes one newline-delimited JSON request to stdin and accepts exactly one response line on stdout. Diagnostic logs go to stderr.

```json
{"schema_version":1,"id":1,"command":"frame_stats","params":{"trace_path":"C:/contained/run.tracy"}}
```

```json
{"schema_version":1,"id":1,"ok":true,"result":{"frame_count":100}}
```

Commands are a closed enum. Result counts, string sizes, trace sizes, memory limits, and durations are fixed or centrally bounded. Capture always targets an internally selected loopback address and port. Rust canonicalizes and contains every input and output path before invoking the helper.

## Runtime ownership

`RuntimeState` gains explicit `Standard` and `Tracy` kinds plus capture state. A lifecycle mutex serializes runtime, debugger, and Tracy transitions. Tracy and the auxtools debugger are mutually exclusive initially. Normal `dm_run` behavior remains unchanged.

`dm_tracy_launch` supplies only the minimum required child environment, `UTRACY_BIND_ADDRESS=127.0.0.1`, an internally allocated `UTRACY_BIND_PORT`, and fixed `-params tracy`. It accepts no caller-selected host or arbitrary DreamDaemon arguments. Readiness requires the byond-tracy initialization marker or a bounded failure.

Because byond-tracy does not implement a reliable runtime destroy operation, `dm_tracy_stop` terminates an active capture helper before terminating the owned DreamDaemon.

## Artifact preparation

Preparation is explicit and auditable. The hook artifact is selected from a schema-version-2 helper manifest, hash-verified, then copied beside the DMB using a private temporary file and atomic replacement. Existing mismatched files are rejected unless `overwrite=true`. A matching existing file is an idempotent success.

The generalized helper manifest records helper ID, platform, architecture, path, SHA-256, source revision, protocol version, and optional BYOND bounds. Existing schema-version-1 dmdoc manifests remain readable during migration.

## Capture and analysis

Live capture is single-client and bounded by duration and memory. The helper writes to a private contained temporary path; Rust validates the response and resulting trace before atomic promotion. Interrupted or failed captures do not replace the requested output.

Offline query ordering is deterministic. Every response reports truncation and the limits applied. Procedure identity uses path, source file, and line where available. Comparisons match by that identity rather than display name alone.

## Build and packaging

`scripts/build-tracy-helpers.ps1` receives explicit local Tracy and byond-tracy source paths, verifies exact Git revisions, builds the x64 helper and x86 hook, and writes the manifest. It does not clone or fetch source. Windows and Ubuntu CI may prepare pinned source in an earlier, visible step.

The installer copies only manifest-listed, verified artifacts. `configure-codex-meridian-mcp.ps1` gains `-EnableTracy`; the default configuration remains disabled.

## Verification and promotion

Portable Windows and Ubuntu jobs compile and unit-test the helper, verify manifests, and run the Rust contract tests. They do not claim live BYOND compatibility.

Live gates use a technical DM fixture and the installed MCP stdio entry point to prepare, launch, capture, analyze, compare, and stop. A result is compatibility-verified only when it records the exact OS, architecture, BYOND version, Meridian-MCP SHA, fixture or Meridian-Rift SHA, Tracy commit, byond-tracy commit, protocol, and artifact hashes.

Windows and Ubuntu receive independent compatibility states. The tools remain experimental until live evidence and profiling-overhead measurements are recorded.
