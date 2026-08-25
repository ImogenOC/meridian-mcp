# Restricted Auxtools Debugger Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expose the complete current auxtools debugger behavior through an opt-in, Windows-only, loopback-only MCP surface that launches and owns DreamSeeker and cannot attach to arbitrary processes.

**Architecture:** Adapt the pinned SpacemanDMM auxtools length-prefixed bincode protocol directly into a transport-independent Rust module. Meridian-MCP verifies a fixed `debug_server.dll`, listens on an ephemeral loopback port, launches a contained DMB through fixed DreamSeeker discovery, and stores one bounded owned debugger session separate from analysis state.

**Tech Stack:** Rust 1.95, Tokio TCP/process primitives, bincode 1.3.3, serde, Windows Job Objects, BYOND DreamMaker/DreamSeeker 516.1685, auxtools debug server v2.3.7.

**Spec:** `docs/superpowers/specs/2026-08-24-spacemandmm-complete-integration-design.md`

## Global Constraints

- Complete the foundation/language plan first.
- Use actual pinned SpacemanDMM behavior: launch and own DreamSeeker, not DreamDaemon.
- Never attach to a caller-selected PID, process, port, host, executable, or DLL.
- Bind only `127.0.0.1:0`; reject any non-loopback resolved endpoint.
- The client cannot supply process environment or arguments.
- Runtime download is forbidden. Only the explicit fixed fetch script may access the release URL.
- Do not implement extools, disassembly, restart, or arbitrary DAP passthrough.
- Evaluation is active code execution inside the owned trusted debuggee; development mode and explicit startup opt-in are mandatory.
- Leave changes uncommitted absent explicit authorization.

---

### Task 1: Add debugger startup policy and fixed artifact acquisition

**Files:**
- Create: `src/spaceman/debugger/mod.rs`
- Create: `src/spaceman/debugger/artifact.rs`
- Create: `scripts/fetch-auxtools.ps1`
- Create: `tests/debugger_policy.rs`
- Modify: `.gitignore`
- Modify: `src/spaceman/mod.rs`
- Modify: `src/config.rs`
- Modify: `src/path_policy.rs`
- Modify: `src/contracts.rs`
- Modify: `src/server.rs`
- Modify: `src/tools/mod.rs`
- Modify: `Cargo.toml`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: Development mode, compiler allowlist, current executable directory, fixed artifact constants.
- Produces:

```rust
pub const AUXTOOLS_VERSION: &str = "v2.3.7";
pub const AUXTOOLS_SHA256: &str = "b188999ac58a0e0171b015c39a403ab7da2f37ddb8ac3817a078f5bce02a8be7";
pub const AUXTOOLS_RELEASE_URL: &str = "https://github.com/willox/auxtools/releases/download/v2.3.7/debug_server.dll";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebuggerAccess { Disabled, Auxtools }

pub struct DebuggerInstallation {
    pub dreamseeker: std::path::PathBuf,
    pub debug_server_dll: std::path::PathBuf,
    pub dll_sha256: String,
}

pub fn validate_debugger_installation(config: &ServerConfig) -> Result<Option<DebuggerInstallation>, DebuggerPolicyError>;
```

- [ ] **Step 1: Write failing startup-policy tests**

Require:

- Unset `MERIDIAN_MCP_DEBUGGER` means disabled.
- `auxtools` in analysis mode fails startup.
- Unknown values fail startup.
- Non-Windows never advertises debugger tools.
- Missing or hash-mismatched DLL fails startup when explicitly enabled.
- DreamSeeker must be the canonical `dreamseeker.exe` sibling of exactly one allowlisted `dm.exe`.
- A stale call while disabled returns `tool_not_available`.

- [ ] **Step 2: Run tests and confirm debugger policy is absent**

Run: `cargo test --test debugger_policy startup_policy --all-features`

Expected: missing `DebuggerAccess` and artifact validation.

- [ ] **Step 3: Implement immutable configuration and tool feature gates**

Parse `MERIDIAN_MCP_DEBUGGER` only in `ServerConfig::from_env`. Existing test constructors default to disabled; add an explicit constructor argument for debugger tests. Replace one-off Rift filtering with:

```rust
pub enum FeatureGate { Always, RiftCompile, DmdocHelper, AuxtoolsDebugger }
pub struct ActiveFeatures { pub rift_compile: bool, pub dmdoc: bool, pub auxtools: bool }
```

Every tool contract declares one gate. `MeridianServer::new` computes immutable active features after validating configured artifacts.

- [ ] **Step 4: Implement fixed DreamSeeker and DLL discovery**

Require exactly one allowlisted compiler for debugger startup. Resolve `dreamseeker.exe` in its canonical parent; do not search PATH or accept a client path. Resolve the DLL only at:

```text
<meridian-mcp executable directory>/helpers/auxtools/v2.3.7/debug_server.dll
```

Canonicalize and verify SHA-256 before accepting startup.

- [ ] **Step 5: Implement the explicit fetch script**

`scripts/fetch-auxtools.ps1` accepts only `-DestinationRoot`. It downloads the constant URL to a temporary file, requires HTTP success, verifies the exact SHA-256, then atomically places the file at `helpers/auxtools/v2.3.7/debug_server.dll`. It deletes temporary bytes on every failure. Add `/helpers/auxtools/` to `.gitignore`.

- [ ] **Step 6: Run policy and script-contract tests**

Run:

```powershell
cargo test --test debugger_policy startup_policy --all-features
cargo test --test workflow_contract --all-features
```

Expected: all startup and fixed-script rules pass without downloading.

- [ ] **Step 7: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: gate exact auxtools installation`.

---

### Task 2: Implement the bounded auxtools wire protocol

**Files:**
- Create: `src/spaceman/debugger/protocol.rs`
- Modify: `src/spaceman/debugger/mod.rs`
- Modify: `Cargo.toml`
- Modify: `tests/debugger_policy.rs`

**Interfaces:**
- Consumes: `TcpListener` bound to loopback and the pinned v2.3.7 bincode protocol.
- Produces:

```rust
#[derive(serde::Serialize, serde::Deserialize, Debug)]
pub enum AuxRequest {
    Disconnect,
    Configured,
    StdDef,
    Eval { frame_id: Option<u32>, command: String, context: Option<String> },
    CurrentInstruction { frame_id: u32 },
    BreakpointSet { instruction: InstructionRef, condition: Option<String> },
    BreakpointUnset { instruction: InstructionRef },
    CatchRuntimes { should_catch: bool },
    LineNumber { proc: ProcRef, offset: u32 },
    Offset { proc: ProcRef, line: u32 },
    Stacks,
    StackFrames { stack_id: u32, start_frame: Option<u32>, count: Option<u32> },
    Scopes { frame_id: u32 },
    Variables { vars: VariablesRef },
    Continue { kind: ContinueKind },
    Pause,
}

pub struct AuxConnection {
    stream: tokio::net::TcpStream,
    max_message_bytes: usize,
    response_timeout: std::time::Duration,
}

impl AuxConnection {
    pub async fn request(&mut self, request: AuxRequest) -> Result<AuxResponse, AuxProtocolError>;
    pub async fn disconnect(&mut self) -> Result<(), AuxProtocolError>;
}
```

- [ ] **Step 1: Write protocol compatibility fixtures**

Serialize representative request/response variants with bincode 1.3.3 and lock their length-prefixed byte vectors. Add a loopback fake server that sends notification and breakpoint-hit events between request and response, oversized length, malformed bincode, timeout, and disconnect.

- [ ] **Step 2: Run the protocol tests and confirm the module is absent**

Run: `cargo test debugger::protocol --all-features`

Expected: compile failure for `AuxRequest`/`AuxConnection`.

- [ ] **Step 3: Add exact protocol models**

Add `bincode = "1.3.3"`. Reproduce only the current auxtools request/response/domain enums from the pinned source, with a provenance comment naming the exact file paths and revision. Do not include extools types.

- [ ] **Step 4: Implement bounded framing and event routing**

Write `u32` little-endian payload length followed by bincode bytes. Reject zero or lengths above `8 * 1024 * 1024` before allocation. Run one reader task that routes `Notification`, `BreakpointHit`, and `Disconnect` to a bounded event queue while sending ordinary responses to the single in-flight request. A mutex prevents concurrent protocol requests from reordering responses.

- [ ] **Step 5: Run protocol tests**

Run: `cargo test debugger::protocol --all-features`

Expected: byte fixtures, interleaved events, timeout, malformed data, oversize, and disconnect tests pass.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: add bounded auxtools protocol`.

---

### Task 3: Add the owned DreamSeeker debugger session lifecycle

**Files:**
- Create: `src/spaceman/debugger/session.rs`
- Create: `src/tools/debugger.rs`
- Modify: `src/spaceman/debugger/mod.rs`
- Modify: `src/state.rs`
- Modify: `src/process.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `src/limits.rs`
- Modify: `tests/debugger_policy.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: Verified `DebuggerInstallation`, contained DMB, owned process containment, `AuxConnection`.
- Produces:

```rust
pub enum DebuggerLifecycle { Idle, Launching, Running, Stopped, Terminated, Failed }
pub struct DebuggerSession {
    pub lifecycle: DebuggerLifecycle,
    pub process: OwnedProcess,
    pub connection: AuxConnection,
    pub port: u16,
    pub dmb_path: PathBuf,
    pub stddef_source: Option<String>,
    pub events: VecDeque<DebuggerEvent>,
    pub last_exception: Option<String>,
}
pub struct DebugLaunchParams { pub dmb_path: PathBuf, pub startup_timeout_ms: Option<u64> }
```

- [ ] **Step 1: Write failing state-machine and command-construction tests**

Require valid transitions, reject a second session, reject non-DMB/outside-root paths, bind `127.0.0.1:0`, and lock the exact child command:

```text
<fixed dreamseeker.exe> <contained.dmb> -trusted
```

Require exactly these added child variables and no inherited credentials:

```text
AUXTOOLS_DEBUG_MODE=LAUNCHED
AUXTOOLS_DEBUG_PORT=<selected loopback port>
AUXTOOLS_DEBUG_DLL=<verified fixed DLL>
```

- [ ] **Step 2: Run tests and confirm lifecycle APIs are absent**

Run: `cargo test --test debugger_policy lifecycle --all-features`

Expected: missing session state and `dm_debug_launch`.

- [ ] **Step 3: Extend limits and owned-process support**

Add `max_debug_message_bytes = 8 MiB`, `max_debug_events = 1_000`, `max_debug_output_bytes = 1 MiB`, `max_debug_startup_ms = 60_000`, `max_debug_request_ms = 30_000`, `max_debug_frames = 1_000`, and `max_debug_variables = 10_000`. Reuse the Windows Job Object/process-tree cleanup used by compile runners; expose a long-lived `OwnedProcess` handle without allowing PID-based lookup.

- [ ] **Step 4: Implement launch handshake**

Create the loopback listener first, launch DreamSeeker, accept exactly one connection before timeout, reject non-loopback peer addresses, request `StdDef`, send `Configured`, then transition to `Running`. On any failure, disconnect, terminate the owned process tree, join reader tasks, record `Failed`, and leave no live session.

- [ ] **Step 5: Add `dm_debug_launch` and `dm_debug_stop`**

Both are development-only and auxtools-gated. `dm_debug_stop` always sends `Disconnect` best-effort, closes TCP, stops readers, and terminates the owned process tree. It never detaches and never accepts a process identifier.

- [ ] **Step 6: Run lifecycle, active-policy, and runtime regression tests**

Run:

```powershell
cargo test --test debugger_policy lifecycle --all-features
cargo test --test active_tool_policy --all-features
cargo test --all-features runtime
```

Expected: lifecycle tests pass and existing DreamDaemon runtime behavior is unchanged.

- [ ] **Step 7: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: own auxtools DreamSeeker sessions`.

---

### Task 4: Add source, function, exception breakpoints, and execution control

**Files:**
- Modify: `src/spaceman/debugger/session.rs`
- Modify: `src/tools/debugger.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `tests/debugger_policy.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: Active snapshot source/proc index, protocol offset requests, active session.
- Produces:

```rust
pub struct SourceBreakpoint { pub line: u32, pub condition: Option<String> }
pub struct FunctionBreakpoint { pub proc_path: String, pub override_id: Option<u32>, pub condition: Option<String> }
pub enum DebugControlAction { Pause, Continue, StepIn, StepOver, StepOut }
pub struct DebugControlParams { pub action: DebugControlAction, pub thread_id: Option<u32> }
```

- [ ] **Step 1: Write failing breakpoint/control tests with a fake aux server**

Require contained source membership, rejection of modified/unparsed source, source line to canonical proc/override mapping, `Offset` lookup, condition length limit, removed-breakpoint cleanup, function path validation, runtime exception toggle, and exact `ContinueKind` mapping for every action.

- [ ] **Step 2: Run focused tests and confirm tools are absent**

Run: `cargo test --test debugger_policy breakpoints_and_control --all-features`

Expected: missing breakpoint/control contracts.

- [ ] **Step 3: Implement source and function breakpoints**

Use the snapshot's source/proc index to resolve line to `(proc_path, override_id)`, then protocol `Offset` to obtain the instruction. Store the verified instruction set per source/function request and unset entries omitted from the next complete replacement request. Function breakpoints accept canonical `/type/proc/name#override` identity through typed fields, not a raw parser string.

- [ ] **Step 4: Implement exception and execution controls**

`dm_debug_set_exception_breakpoints` accepts only `break_on_runtimes: bool` and sends `CatchRuntimes`. `dm_debug_control` requires thread ID for stepping, rejects it for pause/continue only if ambiguous, and maps to `Pause` or the exact `ContinueKind` variants.

- [ ] **Step 5: Register and test the four tools**

Add `dm_debug_set_breakpoints`, `dm_debug_set_function_breakpoints`, `dm_debug_set_exception_breakpoints`, and `dm_debug_control`. Cap condition strings at 4,096 UTF-8 bytes and breakpoint lists at 10,000.

Run:

```powershell
cargo test --test debugger_policy breakpoints_and_control --all-features
cargo test --test tool_contracts --all-features
```

Expected: all pass with no disassemble/restart/attach schema.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: control auxtools breakpoints and stepping`.

---

### Task 5: Add debugger query, evaluation, source, and event tools

**Files:**
- Modify: `src/spaceman/debugger/session.rs`
- Modify: `src/tools/debugger.rs`
- Modify: `src/parameters.rs`
- Modify: `src/contracts.rs`
- Modify: `tests/debugger_policy.rs`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: `Stacks`, `StackFrames`, `Scopes`, `Variables`, `Eval`, `StdDef`, and retained event state.
- Produces the remaining approved debugger contracts: threads, stack trace, scopes, variables, evaluate, exception info, source, and bounded event waiting.

- [ ] **Step 1: Write failing query and bound tests**

Use a fake aux server to return two threads, paged frames, optional source lines, three scopes, nested variables, evaluation with variable reference, `stddef.dm`, runtime exception event, and oversized result lists. Require stable bounded output, truncation metadata, timeout behavior, and event filtering for breakpoint, step, pause, runtime, output, and termination events.

- [ ] **Step 2: Run tests and confirm query tools are absent**

Run: `cargo test --test debugger_policy queries --all-features`

Expected: missing query contracts.

- [ ] **Step 3: Implement threads, frames, scopes, and variables**

Map stack frame proc/override to snapshot source location when available; otherwise use the line supplied by auxtools and mark source resolution incomplete. Clamp paging and list counts to server limits. Preserve variable references only within the active session generation.

- [ ] **Step 4: Implement evaluation and exception information**

`dm_debug_evaluate` accepts `expression`, optional `frame_id`, and optional context enum `watch`, `repl`, or `hover`; cap expression at 16,384 UTF-8 bytes. Document and annotate it as active debuggee code execution. `dm_debug_exception_info` returns only the last runtime message from the active session and its event sequence.

- [ ] **Step 5: Implement restricted debugger source**

`dm_debug_source` accepts only `source_reference` equal to the active session's issued `STDDEF_SOURCE_REFERENCE`. Return bounded retained `stddef.dm` text. Reject paths, URLs, and unknown references.

- [ ] **Step 6: Implement bounded event waiting**

Add:

```rust
pub enum DebugEventKind { Breakpoint, Step, Pause, Runtime, Output, Terminated }
pub struct DebugWaitForEventParams {
    pub kinds: Option<Vec<DebugEventKind>>,
    pub after_sequence: Option<u64>,
    pub timeout_ms: Option<u64>,
}
```

`dm_debug_wait_for_event` waits on the active session's bounded event queue and Tokio notification, returns the first matching event after the requested sequence, caps timeout at 300,000 ms, and reports dropped-event count when queue eviction occurred.

- [ ] **Step 7: Register and run all query tests**

Register `dm_debug_threads`, `dm_debug_stack_trace`, `dm_debug_scopes`, `dm_debug_variables`, `dm_debug_evaluate`, `dm_debug_exception_info`, `dm_debug_source`, and `dm_debug_wait_for_event`.

Run:

```powershell
cargo test --test debugger_policy queries --all-features
cargo test --test active_tool_policy --all-features
cargo test --test mcp_conformance --all-features
```

Expected: all pass and every query reports session/state generation and truncation.

- [ ] **Step 8: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `feat: expose auxtools debugger queries`.

---

### Task 6: Add the real Windows auxtools integration gate

**Files:**
- Create: `scripts/run-auxtools-integration.ps1`
- Modify: `tests/fixtures/runtime/runtime.dm`
- Modify: `tests/fixtures/runtime/runtime.dme`
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `TESTING.md`
- Modify: `docs/compatibility.md`
- Modify: `spacemandmm-capabilities.json`

**Interfaces:**
- Consumes: Release MCP binary, exact BYOND installation, fixed DLL, runtime fixture.
- Produces: Machine-readable debugger evidence with launch, breakpoint, query, control, and cleanup results.

- [ ] **Step 1: Add a deterministic technical debugger fixture**

Add a proc that assigns a known numeric local, calls a second proc on a known line, and keeps the program alive long enough for a breakpoint. Do not add lore, names, descriptions, icons, or sounds.

- [ ] **Step 2: Write the PowerShell stdio integration script**

Use `MeridianMcpSession.psm1` to:

1. Compile the fixture through `dm_compile`.
2. Start a fresh MCP session with development mode, contained roots, exact compiler, and `MERIDIAN_MCP_DEBUGGER=auxtools`.
3. Confirm every debugger tool is advertised and attach/disassembly/restart tools are absent.
4. Launch the fixture DMB.
5. Set a source breakpoint and runtime exception breakpoint.
6. Continue and use `dm_debug_wait_for_event` to observe the breakpoint.
7. Query threads, frames, scopes, variables, and evaluate the known local.
8. Retrieve `stddef.dm` through the issued source reference.
9. Step over and continue.
10. Stop and verify DreamSeeker plus child processes exit.
11. Write bounded JSON containing versions, hashes, timings, responses, and `overall`.

- [ ] **Step 3: Add the exact artifact acquisition to the manual/scheduled workflow**

After BYOND installation, run `fetch-auxtools.ps1`, build the release MCP, and invoke `run-auxtools-integration.ps1`. Upload evidence even on failure. Keep this in the existing manual/scheduled BYOND workflow, not the ordinary pull-request Rust job.

- [ ] **Step 4: Run the portable suite before live BYOND**

Run:

```powershell
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features
cargo build --release
```

Expected: all pass.

- [ ] **Step 5: Run the live Windows gate**

Run:

```powershell
.\scripts\fetch-auxtools.ps1 -DestinationRoot (Split-Path -Parent (Resolve-Path .\target\release\meridian-mcp.exe))
.\scripts\run-auxtools-integration.ps1 `
  -BinaryPath .\target\release\meridian-mcp.exe `
  -FixtureDme .\tests\fixtures\runtime\runtime.dme
```

Expected: evidence reports `overall: passed`, the exact DLL hash, loopback transport, owned DreamSeeker PID, and clean shutdown.

- [ ] **Step 6: Record the checkpoint**

Run `git diff --check`. Proposed commit message if authorized: `test: verify auxtools through DreamSeeker`.
