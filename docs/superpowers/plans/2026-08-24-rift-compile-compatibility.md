# Rift Compile and Meridian-Rift Compatibility Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking. Do not dispatch subagents unless the user separately authorizes them.

**Goal:** Add a separately gated `rift_compile` MCP tool that executes Meridian-Rift's agent-owned `RIFT_BUILD.cmd`, verifies direct and full DreamMaker builds at repository scale, and promotes only capabilities backed by a successful Windows integration run.

**Architecture:** Meridian-Rift gains an isolated PowerShell wrapper around the unchanged human build pipeline. Meridian-MCP gains an immutable build-access ceiling, a shared bounded process runner, Windows Job Object containment, optional best-effort endpoint auditing, artifact evidence, and a real stdio compatibility harness. Promotion is a second phase after the first green BYOND 516.1685 workflow run.

**Tech Stack:** Rust 1.88, Tokio, rmcp 3.1.3, serde/schemars, windows-sys, SHA-256, PowerShell 5.1+, Windows batch, Bun 1.3.5, BYOND 516.1685, GitHub Actions.

**Spec:** `docs/superpowers/specs/2026-08-23-rift-compile-compatibility-design.md`

## Global Constraints

- Leave both repositories uncommitted unless the user explicitly authorizes commits.
- Do not modify Meridian-Rift `BUILD.cmd`, `tools/bootstrap/*`, `tools/build/build.ts`, `tools/build/lib/download.ts`, release/deployment scripts, or Meridian-Rift CI. If the wrapper proves insufficient, stop and request explicit infrastructure-change approval.
- Preserve the already-approved guidance edits in Meridian-Rift `AGENTS.md`, `docs/agent/README.md`, and `docs/agent/source-authority.md`.
- `BUILD.cmd` remains the human-authoritative Windows full build. `RIFT_BUILD.cmd` is an agent-owned non-interactive derivative.
- `dm_compile` remains a direct DreamMaker gate and must never be described as equivalent to `BUILD.cmd`.
- `rift_compile` is development-only, absent by default, and cannot broaden its immutable startup ceiling.
- Startup values are exactly `disabled`, `offline`, and `network`; invocation values are exactly `offline` and `allow`.
- Offline enforcement is cooperative preflight plus strict process-local package-manager configuration, not an operating-system firewall.
- Network auditing is optional, observational, bounded, and always reports `capture_complete: false`.
- The initial named compatibility baseline is Windows with BYOND `516.1685`.
- All affected tools remain `Provisional` until the first successful recorded Windows workflow run.
- DreamChecker, map, rendering, DreamDaemon, and `Topic()` verification remain deferred and visible.
- Use PowerShell for Meridian-Rift build and test commands. Inspect `$LASTEXITCODE` after native commands.
- Test the shipped `.cmd` and installed MCP stdio entry points, not only internal helpers.

## Existing Approved Working-Tree Changes

The implementation session begins with these intentional uncommitted changes:

- Meridian-Rift `AGENTS.md`: protected human-infrastructure rule.
- Meridian-Rift `docs/agent/README.md`: routing entry for critical infrastructure.
- Meridian-Rift `docs/agent/source-authority.md`: explicit approval and wrapper preference.
- Meridian-MCP `docs/superpowers/specs/2026-08-23-rift-compile-compatibility-design.md`: approved design.

Do not discard or overwrite them.

---

### Task 1: Add the separate Meridian-Rift agent build wrapper

**Files:**
- Create in Meridian-Rift: `RIFT_BUILD.cmd`
- Create in Meridian-Rift: `tools/build/rift/lib.ps1`
- Create in Meridian-Rift: `tools/build/rift/invoke.ps1`
- Create in Meridian-Rift: `tools/build/rift/test.ps1`
- Create in Meridian-Rift: `tools/build/rift/README.md`
- Test against, but do not modify: `BUILD.cmd`
- Test against, but do not modify: `dependencies.sh`

**Interfaces:**
- Consumes: `MERIDIAN_RIFT_BUILD_NETWORK=offline|allow`, `MERIDIAN_RIFT_FORCE_REBUILD=0|1`, the checked-in dependency pins, and the existing `tools/build/build.bat build` operation.
- Produces: a stable root `RIFT_BUILD.cmd`, `Invoke-RiftBuild`, `Test-RiftBuildContract`, `Get-RiftDependencyPins`, `Test-RiftOfflinePrerequisites`, and exact child exit propagation.

- [ ] **Step 1: Write the PowerShell contract tests before the wrapper exists**

Create `tools/build/rift/test.ps1` with isolated tests that dot-source `lib.ps1`. The tests must assert:

```powershell
$contract = Test-RiftBuildContract -RepositoryRoot $RepositoryRoot
Assert-Equal $contract.delegate 'tools/build/build.bat'
Assert-Equal $contract.target 'build'
Assert-Equal $contract.human_wait_on_error $true
Assert-Equal $contract.wrapper_wait_on_error $false

Assert-ThrowsCode {
	Assert-RiftBuildMode -Mode 'internet'
} 'invalid_build_mode'

Assert-ThrowsCode {
	Test-RiftOfflinePrerequisites -RepositoryRoot $FixtureRoot
} 'offline_preflight_failed'

$config = New-RiftOfflineEnvironment -TemporaryRoot $TestDrive
Assert-Equal $config.variables.PIP_NO_INDEX '1'
Assert-Contains (Get-Content -Raw $config.bunfig) 'offline = true'
Assert-Contains (Get-Content -Raw $config.bunfig) 'telemetry = false'
```

Add a fixture `BUILD.cmd` string matching the current delegate and a drifted string targeting `lint`; the second must fail without executing either string. Add a cleanup test proving the temporary Bun configuration directory is deleted after a simulated delegate failure.

- [ ] **Step 2: Run the wrapper tests and verify the expected initial failure**

Run from Meridian-Rift:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\build\rift\test.ps1
if ($LASTEXITCODE -eq 0) { throw 'Expected missing wrapper library tests to fail' }
```

Expected: non-zero because `tools/build/rift/lib.ps1` and its functions do not exist.

- [ ] **Step 3: Implement the testable wrapper library**

Create `tools/build/rift/lib.ps1` with these exact public functions:

```powershell
function Assert-RiftBuildMode {
	param([Parameter(Mandatory)][string]$Mode)
}

function Test-RiftBuildContract {
	param([Parameter(Mandatory)][string]$RepositoryRoot)
}

function Get-RiftDependencyPins {
	param([Parameter(Mandatory)][string]$RepositoryRoot)
}

function Test-RiftOfflinePrerequisites {
	param(
		[Parameter(Mandatory)][string]$RepositoryRoot,
		[string]$BootstrapCache
	)
}

function New-RiftOfflineEnvironment {
	param([Parameter(Mandatory)][string]$TemporaryRoot)
}

function Invoke-RiftBuild {
	param(
		[Parameter(Mandatory)][string]$RepositoryRoot,
		[ValidateSet('offline', 'allow')][string]$NetworkMode,
		[bool]$ForceRebuild
	)
}
```

Implementation requirements:

- Parse only literal `export NAME=value` lines from `dependencies.sh`; reject substitutions and non-literal version data.
- Treat the expected base delegate as the constant contained path `tools/build/build.bat` with target `build`.
- Read `BUILD.cmd` only to check that its normalized delegate still contains the same contained batch path and target. Never execute parsed batch content.
- In offline mode, verify the pinned vendored Bun executable, pinned Python executable, pip marker, requirements hash marker, icon-cutter executable, root lockfile, and TGUI lockfile.
- Run the vendored Bun twice with `install --offline --frozen-lockfile --dry-run`, once from the repository root and once from `tgui`; any non-zero result is `offline_preflight_failed`.
- Create a temporary `XDG_CONFIG_HOME` containing `bunfig.toml`:

```toml
telemetry = false
env = false

[install]
offline = true
frozenLockfile = true
```

- Set `PIP_NO_INDEX=1`, `PIP_DISABLE_PIP_VERSION_CHECK=1`, and `PIP_REQUIRE_VIRTUALENV=0` only for the build child.
- For `ForceRebuild`, remove only canonical root `tgstation.dmb` and `tgstation.rsc` with `Remove-Item -LiteralPath`; reject any resolved path outside the repository root.
- Invoke the fixed contained `tools/build/build.bat` with the sole target `build`, capture `$LASTEXITCODE`, clean the temporary directory in `finally`, and return the exact code.

- [ ] **Step 4: Add the stable `.cmd` entry point**

Create `RIFT_BUILD.cmd` with no caller-controlled arguments:

```batch
@echo off
setlocal
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%~dp0tools\build\rift\invoke.ps1"
exit /b %ERRORLEVEL%
```

`invoke.ps1` reads only the two approved environment variables, defaults network mode to `offline`, converts `MERIDIAN_RIFT_FORCE_REBUILD` from `0|1`, calls `Invoke-RiftBuild`, and exits with its code. It must reject extra command-line arguments.

- [ ] **Step 5: Document ownership and the human-build boundary**

Create `tools/build/rift/README.md` stating:

- Humans continue to use root `BUILD.cmd`.
- MCP uses root `RIFT_BUILD.cmd`.
- The wrapper mirrors `tools/build/build.bat build` without `--wait-on-error`.
- Offline mode requires a warm, pinned cache and fails rather than fetching.
- Network mode preserves the inherited bootstrap behavior.
- The endpoint audit is performed by Meridian-MCP, not this script.
- Changes to inherited critical infrastructure require separate explicit approval.

- [ ] **Step 6: Run focused wrapper verification**

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\build\rift\test.ps1
if ($LASTEXITCODE -ne 0) { throw "rift wrapper tests failed: $LASTEXITCODE" }

cmd.exe /d /s /c "set MERIDIAN_RIFT_BUILD_NETWORK=invalid&& call RIFT_BUILD.cmd"
if ($LASTEXITCODE -eq 0) { throw 'RIFT_BUILD.cmd accepted an invalid mode' }
```

Expected: focused tests pass; the real entry point rejects the invalid mode before executing the base build.

- [ ] **Step 7: Review checkpoint without committing**

Inspect `git diff --check` and confirm `BUILD.cmd`, `tools/bootstrap`, `tools/build/build.ts`, and `tools/build/lib/download.ts` have no diffs.

---

### Task 2: Add the immutable Meridian-MCP build-access ceiling and tool contract

**Files:**
- Modify: `src/config.rs`
- Modify: `src/contracts.rs`
- Modify: `src/lib.rs`
- Modify: `src/server.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/parameters.rs`
- Modify: `tests/config_and_paths.rs`
- Modify: `tests/tool_contracts.rs`
- Modify: `tests/mcp_conformance.rs`
- Modify: `tests/active_tool_policy.rs`

**Interfaces:**
- Consumes: `MERIDIAN_MCP_RIFT_BUILD`.
- Produces: `RiftBuildAccess`, `RiftNetworkMode`, configuration-aware tool visibility, conditional external-network effects, and the `rift_compile` schema.

- [ ] **Step 1: Write failing startup and visibility tests**

Add tests equivalent to:

```rust
assert_eq!(
	ServerConfig::from_values_with_rift_build(
		Some("development"),
		vec![root.clone()],
		vec![],
		Some("offline"),
	)?.rift_build_access(),
	RiftBuildAccess::Offline,
);
assert!(ServerConfig::from_values_with_rift_build(
	Some("development"), vec![root], vec![], Some("internet")
).is_err());
```

Verify the tool matrix. The Windows assertions are:

```rust
assert!(!tool_names(CapabilityMode::Analysis, RiftBuildAccess::Network).contains("rift_compile"));
assert!(!tool_names(CapabilityMode::Development, RiftBuildAccess::Disabled).contains("rift_compile"));
assert!(tool_names(CapabilityMode::Development, RiftBuildAccess::Offline).contains("rift_compile"));
assert!(tool_names(CapabilityMode::Development, RiftBuildAccess::Network).contains("rift_compile"));
```

On non-Windows platforms, assert `rift_compile` is never advertised. A direct stale-schema call with development mode and a non-disabled startup ceiling must still return `unsupported_platform`, not panic or execute a fallback.

Also assert the `rift_compile` schema exposes exactly `network_mode`, `timeout_ms`, `idle_timeout_ms`, `capture_network`, and `force_rebuild`, with no path, executable, target, define, or argument-list property.

- [ ] **Step 2: Run focused tests and verify failure**

```powershell
cargo test --test config_and_paths --test tool_contracts --test mcp_conformance --test active_tool_policy
if ($LASTEXITCODE -eq 0) { throw 'Expected new build-access tests to fail before implementation' }
```

- [ ] **Step 3: Implement configuration types without breaking existing fixture constructors**

Add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiftBuildAccess {
	Disabled,
	Offline,
	Network,
}

#[derive(Clone, Copy, Debug, Deserialize, JsonSchema, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum RiftNetworkMode {
	Offline,
	Allow,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RiftCompileParams {
	pub network_mode: Option<RiftNetworkMode>,
	pub timeout_ms: Option<u64>,
	pub idle_timeout_ms: Option<u64>,
	#[serde(default)]
	pub capture_network: bool,
	#[serde(default)]
	pub force_rebuild: bool,
}
```

Use `Offline` when `network_mode` is absent, a 1,800,000 ms default and maximum wall timeout, and a 120,000 ms default/900,000 ms maximum idle timeout. Reject values below 1,000 ms rather than silently accepting a zero-duration build.

Keep `ServerConfig::from_values(mode, roots, compilers)` as a compatibility constructor that selects `Disabled`. Add `from_values_with_rift_build(mode, roots, compilers, rift_build)` for tests and `from_env()` production loading. Expose `rift_build_access()`.

- [ ] **Step 4: Make tool discovery configuration-aware**

Add:

```rust
pub fn contracts_for_configuration(
	mode: CapabilityMode,
	rift_build: RiftBuildAccess,
) -> Vec<&'static ToolContract>;

pub fn get_tool_definitions_for(
	mode: CapabilityMode,
	rift_build: RiftBuildAccess,
) -> Vec<ToolDefinition>;
```

Store `RiftBuildAccess` in `ToolExecutionContext`. Keep `ToolExecutionContext::new(mode, policy)` defaulting to `Disabled` for existing tests and add `with_rift_build(mode, policy, access)` for the server and new tests.

Add `network_external: bool` to `ToolEffects`. Set it only on the `rift_compile` maximum contract. Update SDK annotations so external-network or process tools are not marked read-only or open-world false incorrectly; `rift_compile` with network capability is `open_world(true)`.

Filter `rift_compile` from advertised contracts under `cfg(not(windows))`. In `call_tool`, handle a direct non-Windows `rift_compile` request before the normal active-contract rejection so clients holding an older schema receive the stable `unsupported_platform` result.

- [ ] **Step 5: Register the `rift_compile` schema and policy rejection**

Register the tool with summary `Run Meridian-Rift's contained RIFT_BUILD.cmd full-build gate.` and a maximum timeout of `1_800_000` ms. Add a dispatch guard that returns `network_mode_denied` when `Allow` is requested under `RiftBuildAccess::Offline`.

Do not add path containment branches for `rift_compile`; it accepts no paths.

- [ ] **Step 6: Run focused configuration and contract tests**

```powershell
cargo test --test config_and_paths --test tool_contracts --test mcp_conformance --test active_tool_policy
if ($LASTEXITCODE -ne 0) { throw "configuration/contract tests failed: $LASTEXITCODE" }
```

- [ ] **Step 7: Review checkpoint without committing**

Run `cargo fmt --all -- --check` and inspect only Task 2 files plus the preserved design/guidance changes.

---

### Task 3: Extend project profiles with separate human and agent entry points

**Files:**
- Modify: `src/project.rs`
- Modify: `src/state.rs`
- Modify: `tests/project_profile.rs`

**Interfaces:**
- Consumes: a canonical parsed `.dme` and `PathPolicy`.
- Produces: `ProjectProfile::{root, human_build_entrypoint, rift_build_entrypoint, byond_version}` and a qualification method used by `rift_compile`.

- [ ] **Step 1: Write failing profile tests**

Create fixtures containing both scripts and assert:

```rust
assert_eq!(profile.root(), root.canonicalize()?.as_path());
assert!(profile.human_build_entrypoint().unwrap().ends_with("BUILD.cmd"));
assert!(profile.rift_build_entrypoint().unwrap().ends_with("RIFT_BUILD.cmd"));
assert_eq!(profile.byond_version(), Some("516.1685"));
assert!(profile.is_rift_build_qualified());
```

Add negative tests for a non-`tgstation.dme`, missing `BUILD.cmd`, missing `RIFT_BUILD.cmd`, non-literal dependency version, and a script symlink/reparse target outside the allowed root.

- [ ] **Step 2: Run the focused test and verify failure**

```powershell
cargo test --test project_profile
if ($LASTEXITCODE -eq 0) { throw 'Expected new project-profile tests to fail before implementation' }
```

- [ ] **Step 3: Implement explicit entry-point fields**

Use:

```rust
pub struct ProjectProfile {
	root: PathBuf,
	dme_path: PathBuf,
	spaceman_config: Option<PathBuf>,
	human_build_entrypoint: Option<PathBuf>,
	rift_build_entrypoint: Option<PathBuf>,
	byond_version: Option<String>,
}
```

`is_rift_build_qualified()` returns true only for canonical contained `tgstation.dme`, both contained root scripts, and a literal BYOND version. Do not search parents or infer another script name.

- [ ] **Step 4: Run profile and parse-state tests**

```powershell
cargo test --test project_profile
cargo test parse_environment
if ($LASTEXITCODE -ne 0) { throw "project profile tests failed: $LASTEXITCODE" }
```

- [ ] **Step 5: Review checkpoint without committing**

Confirm failed reparses still preserve the prior `ProjectProfile` and state generation.

---

### Task 4: Build shared bounded process, artifact, and network-audit infrastructure

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/lib.rs`
- Create: `src/artifact.rs`
- Create: `src/process.rs`
- Create: `src/network_audit.rs`
- Create: `tests/process_runner.rs`
- Create: `tests/fixtures/process/parent.ps1`
- Create: `tests/fixtures/process/child.ps1`

**Interfaces:**
- Consumes: a server-constructed fixed process specification.
- Produces: bounded process outcomes, SHA-256 artifact snapshots, owned-tree termination, and optional endpoint observations.

- [ ] **Step 1: Write failing platform-independent process and artifact tests**

Test these exact public crate interfaces:

```rust
pub struct ArtifactSnapshot {
	pub path: PathBuf,
	pub exists: bool,
	pub size: Option<u64>,
	pub modified_unix_ms: Option<u128>,
	pub sha256: Option<String>,
}

pub enum TerminationReason {
	Exited,
	WallTimeout,
	IdleTimeout,
	Cancelled,
	SpawnFailed,
}

pub struct ProcessSpec {
	pub program: PathBuf,
	pub arguments: Vec<OsString>,
	pub working_directory: PathBuf,
	pub environment: Vec<(OsString, OsString)>,
	pub timeout: Duration,
	pub idle_timeout: Duration,
	pub capture_network: bool,
}

pub async fn run_contained_process(spec: ProcessSpec) -> Result<ProcessOutcome>;
```

Assert SHA-256 changes with file content, stdout/stderr remain at or below contract bounds with truncation counts, wall and idle timeouts are distinct, and a successful exit preserves the exact code.

- [ ] **Step 2: Write Windows-only containment and audit-shape tests**

`parent.ps1` starts `child.ps1`; the child waits and writes a marker. Terminate the parent through a 1-second runner timeout, wait longer than the child delay, and assert the marker was never written. When audit is requested, assert:

```rust
assert!(!outcome.network_audit.capture_complete);
assert!(outcome.network_audit.requested);
assert!(outcome.network_audit.observations.len() <= MAX_NETWORK_OBSERVATIONS);
```

Do not require a live endpoint in unit tests because observation timing is inherently nondeterministic.

- [ ] **Step 3: Run focused tests and verify failure**

```powershell
cargo test --test process_runner
if ($LASTEXITCODE -eq 0) { throw 'Expected process runner tests to fail before implementation' }
```

- [ ] **Step 4: Add dependencies and artifact snapshots**

Add `sha2 = "0.10"`. Extend `windows-sys = "0.61.2"` with:

```toml
features = [
	"Win32_Foundation",
	"Win32_NetworkManagement_IpHelper",
	"Win32_Networking_WinSock",
	"Win32_System_Diagnostics_ToolHelp",
	"Win32_System_JobObjects",
	"Win32_System_Threading",
]
```

`ArtifactSnapshot::capture` canonicalizes existing files, hashes by streaming fixed-size buffers, and never follows a result outside the already-qualified project root.

- [ ] **Step 5: Implement bounded process execution**

`ProcessOutcome` contains:

```rust
pub struct ProcessOutcome {
	pub exit_code: Option<i32>,
	pub termination: TerminationReason,
	pub duration_ms: u128,
	pub stdout: BoundedOutput,
	pub stderr: BoundedOutput,
	pub network_audit: NetworkAuditReport,
}
```

Use tail-preserving bounded buffers so final compiler diagnostics survive truncation. Progress is any output or increase in owned process/job CPU time. On Windows, assign the child to a Job Object configured with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`; use `TerminateJobObject` for timeout/cancellation and query job accounting for idle detection. If assignment fails, terminate the just-spawned child and return containment failure rather than running uncontained.

The runner calls `env_clear()` and applies only `ProcessSpec.environment`; tool modules must explicitly construct every child variable they require.

- [ ] **Step 6: Implement best-effort endpoint sampling**

Define:

```rust
pub struct EndpointObservation {
	pub protocol: EndpointProtocol,
	pub process_id: u32,
	pub local_endpoint: String,
	pub remote_endpoint: Option<String>,
	pub first_seen_ms: u128,
	pub last_seen_ms: u128,
}

pub struct NetworkAuditReport {
	pub requested: bool,
	pub available: bool,
	pub capture_complete: bool,
	pub truncated: bool,
	pub observations: Vec<EndpointObservation>,
	pub warning: Option<String>,
}
```

On Windows, sample TCP and UDP owner tables for PIDs currently assigned to the Job Object, deduplicate by protocol/PID/local/remote tuple, and cap observations. On other platforms, requested audits return `available: false`, `capture_complete: false`, and `network_audit_unavailable` without failing the process.

- [ ] **Step 7: Run the focused process tests**

```powershell
cargo test --test process_runner --all-features
if ($LASTEXITCODE -ne 0) { throw "process runner tests failed: $LASTEXITCODE" }
```

- [ ] **Step 8: Review checkpoint without committing**

Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and inspect the Windows `unsafe` blocks for minimal scope and checked return values.

---

### Task 5: Refactor `dm_compile` onto the shared runner and add optional auditing

**Files:**
- Modify: `src/tools/compile.rs`
- Modify: `src/parameters.rs`
- Modify: `src/tools/mod.rs`
- Modify: `test_mcp.ps1`
- Create: `tests/compiler_runner.rs`

**Interfaces:**
- Consumes: existing contained compiler arguments plus `capture_network: bool`.
- Produces: the existing direct-compiler result fields plus bounded-output metadata, artifact snapshots, termination reason, and a best-effort audit report.

- [ ] **Step 1: Write regression tests for existing and new compile behavior**

Preserve tests for relative DME arguments, defines, diagnostic parsing, total timeout, idle timeout, allowlisted executables, and `dmb_exists`/`dmb_updated`. Add assertions:

```rust
assert_eq!(payload["network_audit"]["requested"], true);
assert_eq!(payload["network_audit"]["capture_complete"], false);
assert!(payload["stdout_truncated_bytes"].as_u64().is_some());
assert!(payload["artifact_after"]["sha256"].is_string());
```

- [ ] **Step 2: Run focused compiler tests and verify failure**

```powershell
cargo test compile --all-features
cargo test --test compiler_runner --all-features
if ($LASTEXITCODE -eq 0) { throw 'Expected new compiler-runner assertions to fail before refactor' }
```

- [ ] **Step 3: Replace the private compiler loop with `run_contained_process`**

Keep DreamMaker-specific code in `src/tools/compile.rs`: compiler resolution, define normalization, diagnostic regex, DME argument construction, and success interpretation. Remove duplicated stream capture, timeout, and CPU polling after equivalent shared-runner tests pass.

Construct a minimal DreamMaker child environment containing required Windows process variables and the existing process `PATH`; do not forward unrelated credentials merely because the shared runner now clears its environment.

Capture the `.dmb` before and after execution. Preserve current result keys for compatibility and add:

```json
{
  "termination": "exited",
  "stdout_truncated_bytes": 0,
  "stderr_truncated_bytes": 0,
  "artifact_before": {},
  "artifact_after": {},
  "network_audit": {
    "requested": true,
    "capture_complete": false
  }
}
```

- [ ] **Step 4: Update the real stdio schema smoke**

Add `capture_network` to the `dm_compile` schema assertion in `test_mcp.ps1`. Do not require observed endpoints.

- [ ] **Step 5: Run focused direct-compiler verification**

```powershell
cargo build
if ($LASTEXITCODE -ne 0) { throw "debug build failed: $LASTEXITCODE" }
cargo test compile --all-features
cargo test --test compiler_runner --all-features
.\test_mcp.ps1 -SkipBuild -BinaryPath .\target\debug\meridian-mcp.exe -Mode development
if ($LASTEXITCODE -ne 0) { throw "dm_compile refactor verification failed: $LASTEXITCODE" }
```

- [ ] **Step 6: Review checkpoint without committing**

Confirm `dm_compile` still advertises and reports a direct compiler gate only.

---

### Task 6: Implement `rift_compile` against `RIFT_BUILD.cmd`

**Files:**
- Create: `src/tools/rift.rs`
- Modify: `src/tools/mod.rs`
- Modify: `src/server.rs`
- Modify: `src/path_policy.rs`
- Modify: `src/state.rs`
- Modify: `src/result.rs`
- Create: `tests/rift_compile.rs`

**Interfaces:**
- Consumes: active qualified `ProjectProfile`, immutable `RiftBuildAccess`, structured `RiftCompileParams`, and contained compiler allowlist.
- Produces: a structured full-build result with artifact classification and stable error categories.

- [ ] **Step 1: Write policy and fake-wrapper tests first**

Cover:

- Call before parse: `project_not_parsed`.
- Parsed non-Meridian fixture: `project_not_qualified`.
- Zero allowlisted compilers: `compiler_not_configured`; more than one: `compiler_ambiguous`.
- `allow` under offline ceiling: `network_mode_denied`.
- Missing offline prerequisite: `offline_preflight_failed` and no wrapper marker.
- Linux: `unsupported_platform`.
- Windows fake qualified project: fixed `RIFT_BUILD.cmd` writes non-empty `.dmb`/`.rsc`; result is `fresh_artifacts`.
- Forced fake build that leaves artifacts unchanged: `insufficient_evidence`.
- No input property can select another script or append arguments.

The Windows fake wrapper belongs only in a temporary contained test root and receives no caller-controlled script content.

- [ ] **Step 2: Run focused tests and verify failure**

```powershell
cargo test --test rift_compile --all-features
if ($LASTEXITCODE -eq 0) { throw 'Expected rift_compile tests to fail before implementation' }
```

- [ ] **Step 3: Implement child environment sanitization**

Construct a fresh environment allowlist containing only required Windows/process variables:

```text
SystemRoot, WINDIR, ComSpec, PATH, PATHEXT, TEMP, TMP,
ProgramFiles, ProgramFiles(x86), ProgramW6432,
LOCALAPPDATA, APPDATA, USERPROFILE,
NUMBER_OF_PROCESSORS, PROCESSOR_ARCHITECTURE
```

Add explicit MCP-selected `DM_EXE`, `MERIDIAN_RIFT_BUILD_NETWORK`, and `MERIDIAN_RIFT_FORCE_REBUILD`. Do not inherit tokens, credentials, proxy credentials, package-registry credentials, or arbitrary client variables. Retain `TG_BOOTSTRAP_CACHE` only if `PathPolicy::read_path` proves the canonical directory is inside a configured root.

Require exactly one canonical compiler in the startup allowlist for `rift_compile`; inject it as `DM_EXE`. Do not add a compiler selector to the tool request. The Windows workflow configures only the downloaded BYOND 516.1685 `dm.exe`.

- [ ] **Step 4: Implement fixed command construction and generation checks**

On Windows, resolve the system command processor from the trusted system directory and construct only:

```text
cmd.exe /D /S /C "<canonical contained project root>\RIFT_BUILD.cmd"
```

No request value contributes to program, script, working directory, or arguments. Snapshot the state generation and profile before preflight; reject a mismatch as `state_generation_changed` if execution is ever moved outside the serialized state lock.

Immediately before spawning, revalidate the canonical `RIFT_BUILD.cmd`, project root, and both artifact paths through `PathPolicy` so deletion/replacement after parsing cannot redirect execution.

- [ ] **Step 5: Implement artifact classification**

Capture `tgstation.dmb` and `tgstation.rsc` before and after. Return:

```rust
pub enum BuildEvidence {
	FreshArtifacts,
	ValidCacheHit,
	BuildFailed,
	InsufficientEvidence,
}
```

`FreshArtifacts` requires both outputs and a changed hash, timestamp, or size. `ValidCacheHit` requires zero exit, no parsed errors, both valid artifacts, and an explicit up-to-date build marker. `force_rebuild=true` rejects a cache hit. Any non-zero exit, timeout, idle timeout, parsed error, or missing artifact is an MCP tool error with the approved stable category.

Recognize the Juke message for the DM target with a narrowly tested, case-insensitive expression equivalent to `Skipping 'dm' (up to date)`. Store the matched line in the result as cache evidence; do not treat an unrelated skipped target as proof.

- [ ] **Step 6: Dispatch the tool and return the complete result**

The result includes project root, both entry points, DME, state generation, BYOND version, startup ceiling, invocation mode, force/audit settings, timing, exit, bounded output, diagnostics, before/after artifacts, evidence classification, endpoint observations, warning fields, and recovery guidance.

- [ ] **Step 7: Run focused tool tests on both platforms available in CI**

```powershell
cargo test --test rift_compile --all-features
cargo test --test active_tool_policy --test mcp_conformance --all-features
if ($LASTEXITCODE -ne 0) { throw "rift_compile tests failed: $LASTEXITCODE" }
```

- [ ] **Step 8: Test the real Meridian-Rift wrapper entry point without launching a full build**

```powershell
$env:MERIDIAN_RIFT_BUILD_NETWORK = 'invalid'
& '..\Meridian-Rift\RIFT_BUILD.cmd'
if ($LASTEXITCODE -eq 0) { throw 'Real RIFT_BUILD.cmd did not reject invalid mode' }
Remove-Item Env:MERIDIAN_RIFT_BUILD_NETWORK
```

Expected: rejection occurs in the shipped wrapper before the inherited build starts.

- [ ] **Step 9: Review checkpoint without committing**

Inspect the schema and command construction to confirm no arbitrary path, executable, URL, target, argument, or environment injection is possible.

---

### Task 7: Add the versioned Meridian-Rift analysis compatibility manifest and stdio harness

**Files:**
- Create: `tests/compatibility/meridian-rift.json`
- Create: `scripts/MeridianMcpSession.psm1`
- Create: `scripts/run-meridian-compatibility.ps1`
- Modify: `test_mcp.ps1`
- Create: `tests/compatibility_manifest.rs`

**Interfaces:**
- Consumes: installed Meridian-MCP binary, Meridian-Rift checkout, DreamMaker path, and a versioned assertion manifest.
- Produces: one JSON evidence document with per-tool results, timings, SHAs, versions, artifacts, and first failure.

- [ ] **Step 1: Write the manifest schema test and checked-in assertions**

Create a schema-version `1` manifest with stable tg-derived cases that exist on Meridian-Rift's default lineage:

```json
{
  "schema_version": 1,
  "types": [
    {"path": "/datum/controller/subsystem", "file_suffix": "code/controllers/subsystem.dm"},
    {"path": "/obj/item", "file_suffix": "code/game/objects/items.dm"},
    {"path": "/mob/living/carbon/human", "parent": "/mob/living/carbon"}
  ],
  "procs": [
    {"type_path": "/datum/controller/subsystem", "name": "fire", "file_suffix": "code/controllers/subsystem.dm"},
    {"type_path": "/obj/item/bedsheet", "name": "attempt_pickup", "inherited_from": "/obj/item"}
  ],
  "vars": [
    {"type_path": "/datum/controller/subsystem", "name": "next_fire", "file_suffix": "code/controllers/subsystem.dm"},
    {"type_path": "/obj/item/bedsheet", "name": "w_class", "inherited_from": "/obj/item"}
  ],
  "type_lists": [
    {"prefix": "/datum/controller/subsystem", "contains": ["/datum/controller/subsystem"]}
  ],
  "symbol_searches": [
    {"query": "update_nextfire", "kind": "proc", "contains_name": "update_nextfire"},
    {"query": "w_class", "kind": "var", "contains_name": "w_class"}
  ],
  "context_searches": [
    {"query": "scheduled world time for next subsystem fire", "top": 10, "contains_symbol": "/datum/controller/subsystem/proc/update_nextfire"}
  ],
  "definitions": [
    {"type_path": "/datum/controller/subsystem", "kind": "type", "file_suffix": "code/controllers/subsystem.dm"},
    {"type_path": "/obj/item/bedsheet", "member": "w_class", "kind": "var", "defined_in": "/obj/item"}
  ]
}
```

`tests/compatibility_manifest.rs` validates unique cases, bounded `top` values, repository-relative suffixes, and coverage of every approved analysis tool.

- [ ] **Step 2: Run the manifest test and verify failure**

```powershell
cargo test --test compatibility_manifest
if ($LASTEXITCODE -eq 0) { throw 'Expected missing compatibility manifest test to fail' }
```

- [ ] **Step 3: Extract reusable stdio session helpers**

Move the existing protocol-line conversion, process-session, and response assertion logic from `test_mcp.ps1` into `scripts/MeridianMcpSession.psm1` without changing behavior. Export:

```powershell
Export-ModuleMember -Function ConvertTo-McpJsonLine, Invoke-McpSession, Get-McpResponse
```

Update `test_mcp.ps1` to import the module and rerun its existing smoke matrix before adding new harness behavior.

- [ ] **Step 4: Implement the full compatibility harness**

`scripts/run-meridian-compatibility.ps1` accepts only:

```powershell
param(
	[Parameter(Mandatory)][string]$BinaryPath,
	[Parameter(Mandatory)][string]$MeridianRiftRoot,
	[Parameter(Mandatory)][string]$DreamMakerPath,
	[Parameter(Mandatory)][string]$EvidencePath,
	[string]$MeridianMcpSha,
	[string]$MeridianRiftSha
)
```

It starts the release binary with development mode, both roots, the exact compiler allowlist, and `MERIDIAN_MCP_RIFT_BUILD=network`. Through one stdio session it:

1. Verifies `rift_compile` discovery and exact schemas.
2. Calls `dm_parse_environment` on `tgstation.dme`.
3. Executes every manifest lookup/search/definition assertion.
4. Repeats each ranked context query and requires identical ordering.
5. Runs `dm_compile` with `capture_network=true` and validates fresh direct artifacts.
6. Runs forced `rift_compile` with `network_mode=allow` and auditing.
7. Runs the human `BUILD.cmd` against the warm checkout and records `$LASTEXITCODE`.
8. Runs forced `rift_compile` with `network_mode=offline`.
9. Writes evidence in a `finally` block, even on failure.

Delete only `tgstation.dmb` and `tgstation.rsc` between the direct and full compiler gates. Validate resolved deletion paths remain direct children of the disposable CI checkout.

Run the fixed human `BUILD.cmd` with a 30-minute wall timeout. On timeout, terminate its owned process tree in the disposable runner and record failure; never wait indefinitely on its interactive error behavior.

- [ ] **Step 5: Add negative stdio sessions**

Run separate short sessions proving:

- `rift_compile` is absent under `disabled`.
- `rift_compile` is absent in analysis mode even with startup `network`.
- `allow` is rejected under startup `offline`.
- A deliberately empty offline cache fails before a wrapper marker can be written.
- A failed reparse preserves the prior generation and exact lookup remains usable.

- [ ] **Step 6: Validate evidence redaction and bounds**

Before serialization, allow only named evidence fields. Reject keys matching `token`, `secret`, `password`, `authorization`, or `cookie` case-insensitively. Record output truncation counts and never serialize the full child environment.

- [ ] **Step 7: Run owned-fixture protocol regression locally**

```powershell
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "release build failed: $LASTEXITCODE" }
.\test_mcp.ps1 -SkipBuild -BinaryPath .\target\release\meridian-mcp.exe -Mode analysis -DmePath .\tests\fixtures\language\fixture.dme -SearchQuery 'return supplied value'
if ($LASTEXITCODE -ne 0) { throw "installed MCP smoke failed: $LASTEXITCODE" }
cargo test --test compatibility_manifest
if ($LASTEXITCODE -ne 0) { throw "manifest test failed: $LASTEXITCODE" }
```

- [ ] **Step 8: Review checkpoint without committing**

Confirm manifest assertions avoid absolute line numbers and exact whole-corpus counts.

---

### Task 8: Expand the scheduled Windows BYOND workflow and preserve Ubuntu boundaries

**Files:**
- Modify: `.github/workflows/byond-integration.yml`
- Modify: `scripts/run-byond-integration.ps1`
- Modify: `.github/workflows/ci.yml`
- Modify: `TESTING.md`

**Interfaces:**
- Consumes: optional manual `meridian_ref`; scheduled runs use Meridian-Rift's default branch.
- Produces: a Windows evidence artifact tied to exact repository SHAs and an Ubuntu unsupported-platform gate.

- [ ] **Step 1: Add workflow-structure tests before editing YAML**

Extend `tests/documentation.rs` or add `tests/workflow_contract.rs` to require:

- `workflow_dispatch.inputs.meridian_ref`.
- Scheduled trigger retained.
- Windows runner retained.
- A second checkout for `AphelionDevelopment/Meridian-Rift`.
- Release binary build.
- `scripts/run-meridian-compatibility.ps1` invocation.
- Evidence upload under `if: always()`.
- No BYOND step in Ubuntu CI.

- [ ] **Step 2: Run the workflow contract test and verify failure**

```powershell
cargo test --test workflow_contract
if ($LASTEXITCODE -eq 0) { throw 'Expected workflow contract test to fail before YAML changes' }
```

- [ ] **Step 3: Expand the Windows workflow**

Configure `workflow_dispatch` with optional string `meridian_ref`. Checkout Meridian-MCP first and Meridian-Rift into `integration/Meridian-Rift`; an empty ref lets checkout resolve the remote default branch. Record both SHAs with `git rev-parse HEAD`.

Read `BYOND_MAJOR` and `BYOND_MINOR` from literal `dependencies.sh` lines and fail unless the initial combined value is `516.1685`. Download that exact official BYOND archive, build Meridian-MCP release, run the owned fixture gate, then run the full compatibility harness. Upload `integration/evidence/*.json` and bounded logs even when a prior step fails.

- [ ] **Step 4: Harden the existing BYOND integration script**

Keep the owned runtime fixture compile. Add exact SHA/version reporting and delegate full-corpus work to `run-meridian-compatibility.ps1`; do not duplicate its MCP assertions.

- [ ] **Step 5: Add the Ubuntu unsupported-platform assertion**

In `.github/workflows/ci.yml`, start the release binary with development mode and `MERIDIAN_MCP_RIFT_BUILD=network`, call a cached-schema `rift_compile` request through the smoke harness, and require `unsupported_platform`. Do not install BYOND or attempt `RIFT_BUILD.cmd` on Ubuntu.

- [ ] **Step 6: Validate workflow syntax and contracts locally**

```powershell
cargo test --test workflow_contract --test documentation
if ($LASTEXITCODE -ne 0) { throw "workflow documentation tests failed: $LASTEXITCODE" }
```

If an installed YAML linter is already available, run it; do not add a new runtime dependency solely for this check.

- [ ] **Step 7: Review checkpoint without committing**

Confirm the Meridian-Rift workflow files remain unchanged and every external action version matches the repository's current action-version policy.

---

### Task 9: Document the new tool, security boundary, and deferred verification work

**Files:**
- Modify in Meridian-MCP: `README.md`
- Modify in Meridian-MCP: `SECURITY.md`
- Modify in Meridian-MCP: `docs/security.md`
- Modify in Meridian-MCP: `docs/architecture.md`
- Modify in Meridian-MCP: `docs/source-authority.md`
- Modify in Meridian-MCP: `docs/compatibility.md`
- Regenerate in Meridian-MCP: `docs/tool-contracts.md`
- Modify in Meridian-MCP: `TESTING.md`
- Modify in Meridian-Rift: `docs/agent/verification.md`
- Modify in Meridian-Rift: `docs/agent/meridian-mcp.md`
- Test: `tests/documentation.rs`

**Interfaces:**
- Consumes: the implemented registry and approved evidence language.
- Produces: drift-checked individual tool descriptions, startup instructions, protected-infrastructure guidance, and a deferred verification matrix.

- [ ] **Step 1: Write documentation assertions first**

Require the README to contain individual rows for `dm_compile` and `rift_compile`, all three startup values, both invocation modes, and links to security/compatibility/testing. Require compatibility documentation to contain a deferred matrix with rows for DreamChecker, map inspection, PNG rendering, DreamDaemon lifecycle, and `Topic()`.

- [ ] **Step 2: Run documentation tests and verify failure**

```powershell
cargo test --test documentation --test tool_contracts
if ($LASTEXITCODE -eq 0) { throw 'Expected new documentation assertions to fail before updates' }
```

- [ ] **Step 3: Update public documentation while retaining provisional labels**

Document:

- `MERIDIAN_MCP_RIFT_BUILD=disabled|offline|network`, default `disabled`.
- Caller `network_mode=offline|allow`, default `offline`.
- `RIFT_BUILD.cmd` is the MCP wrapper; `BUILD.cmd` remains human-authoritative.
- `dm_compile` direct versus `rift_compile` full-build semantics.
- Best-effort capture limitations and `capture_complete: false`.
- Sanitized child environment and no caller-provided URLs or credentials.
- Windows named support and Ubuntu's explicit no-BYOND boundary.
- No promotion before a recorded green workflow.

- [ ] **Step 4: Add the deferred verification matrix**

Use columns `Capability`, `Owned fixture`, `Named-platform/real-repository gate`, `Required semantic evidence`, `Current blocker`, and `Status`. Every row remains `Provisional` and names concrete missing work.

- [ ] **Step 5: Update Meridian-Rift agent workflow documentation**

State that agents use `RIFT_BUILD.cmd` only when Meridian-MCP full-build mode is enabled, humans continue using `BUILD.cmd`, and any proposed modification to protected human infrastructure requires a new explicit confirmation. Preserve PowerShell as the orchestration shell.

- [ ] **Step 6: Regenerate contract documentation**

```powershell
cargo run --bin render_tool_docs -- docs/tool-contracts.md
if ($LASTEXITCODE -ne 0) { throw "tool documentation generation failed: $LASTEXITCODE" }
cargo test --test documentation --test tool_contracts
if ($LASTEXITCODE -ne 0) { throw "documentation verification failed: $LASTEXITCODE" }
```

- [ ] **Step 7: Review checkpoint without committing**

Search for any wording that calls parser success or direct DreamMaker compilation a full build, and correct it.

---

### Task 10: Complete Phase A local verification and prepare the CI handoff

**Files:**
- Verify all modified files in both repositories.
- Write evidence outside tracked source or under a gitignored integration output directory.

**Interfaces:**
- Consumes: complete provisional implementation.
- Produces: local verification report and exact user handoff for branch commits/pushes and manual workflow dispatch.

- [ ] **Step 1: Run the full Meridian-MCP Rust and documentation suite**

```powershell
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { throw "rustfmt failed: $LASTEXITCODE" }
cargo clippy --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "clippy failed: $LASTEXITCODE" }
cargo test --all-features
if ($LASTEXITCODE -ne 0) { throw "tests failed: $LASTEXITCODE" }
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "release build failed: $LASTEXITCODE" }
```

- [ ] **Step 2: Run installed stdio MCP smoke in all startup ceilings**

Run analysis/disabled, development/disabled, development/offline, and development/network sessions. Verify exact tool visibility and policy errors. Run owned language fixture parse/search and the existing map/runtime fixture gates where the environment permits; report any unavailable BYOND process-context gate rather than treating it as passed.

- [ ] **Step 3: Run Meridian-Rift wrapper tests and real entry-point preflight**

```powershell
Push-Location '..\Meridian-Rift'
try {
	powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\build\rift\test.ps1
	if ($LASTEXITCODE -ne 0) { throw "wrapper tests failed: $LASTEXITCODE" }
	$env:MERIDIAN_RIFT_BUILD_NETWORK = 'offline'
	& .\RIFT_BUILD.cmd
	if ($LASTEXITCODE -ne 0) { throw "offline RIFT_BUILD.cmd failed: $LASTEXITCODE" }
} finally {
	Remove-Item Env:MERIDIAN_RIFT_BUILD_NETWORK -ErrorAction SilentlyContinue
	Pop-Location
}
```

This is the authoritative local full-build attempt. Record whether it produced fresh artifacts or a valid cache hit. Do not silently switch to network mode if offline prerequisites are missing.

- [ ] **Step 4: Audit working trees and protected surfaces**

In both repositories run `git diff --check` and `git status --short`. In Meridian-Rift additionally verify no diff under:

```text
BUILD.cmd
tools/bootstrap/
tools/build/build.ts
tools/build/lib/download.ts
.github/workflows/
```

- [ ] **Step 5: Stop at the external CI boundary**

Report the exact local commands, versions, results, artifact evidence, and gates not run. Ask the user to review, commit, and push both repositories. Provide the manual workflow inputs, including the pushed Meridian-Rift wrapper branch/ref. Do not promote support labels yet.

---

### Task 11: Run the named integration gate and promote only evidenced tools

**Files:**
- Modify after a green run: `src/contracts.rs`
- Regenerate after a green run: `docs/tool-contracts.md`
- Modify after a green run: `README.md`
- Modify after a green run: `docs/compatibility.md`
- Modify after a green run: `CHANGELOG.md`
- Test: `tests/tool_contracts.rs`
- Test: `tests/documentation.rs`

**Interfaces:**
- Consumes: a green workflow URL and evidence JSON tied to exact Meridian-MCP and Meridian-Rift SHAs and BYOND 516.1685.
- Produces: per-tool `Verified` statuses and durable evidence references.

- [ ] **Step 1: Inspect the first workflow evidence before changing labels**

Require all of:

- Exact MCP and Rift SHAs.
- Windows runner identity.
- BYOND `516.1685`.
- Successful full-corpus parse.
- Every manifest assertion passing through stdio MCP.
- Fresh direct `dm_compile` artifacts.
- Fresh network-enabled `rift_compile` artifacts.
- Successful warm human `BUILD.cmd` invocation.
- Fresh offline `rift_compile` artifacts.
- Bounded logs and `capture_complete: false` audit disclaimer.

If any item is absent, leave the corresponding tool `Provisional` and record the missing evidence.

- [ ] **Step 2: Write failing promotion assertions for only passing tools**

Update tests to expect `SupportLevel::Verified` only for the exact passing set among:

```text
dm_parse_environment
dm_get_type
dm_get_proc
dm_get_var
dm_list_types
dm_search_symbols
dm_search_context
dm_get_definition
dm_compile
rift_compile
```

All deferred tools remain `Provisional`.

- [ ] **Step 3: Change registry statuses and regenerate docs**

Change only passing `SupportLevel` entries, then run:

```powershell
cargo run --bin render_tool_docs -- docs/tool-contracts.md
if ($LASTEXITCODE -ne 0) { throw "tool docs regeneration failed: $LASTEXITCODE" }
```

- [ ] **Step 4: Record exact evidence near each public claim**

Add the workflow link, MCP SHA, Meridian-Rift SHA, BYOND version, Windows image, manifest version, and run date to `docs/compatibility.md`. If the README capability row contains mixed statuses, split it into tool-specific rows rather than calling the group wholly verified.

- [ ] **Step 5: Run final verification after promotion**

```powershell
cargo fmt --all -- --check
if ($LASTEXITCODE -ne 0) { throw "rustfmt failed: $LASTEXITCODE" }
cargo clippy --all-targets --all-features -- -D warnings
if ($LASTEXITCODE -ne 0) { throw "clippy failed: $LASTEXITCODE" }
cargo test --all-features
if ($LASTEXITCODE -ne 0) { throw "tests failed: $LASTEXITCODE" }
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "release build failed: $LASTEXITCODE" }
```

Rerun the installed stdio smoke and wrapper tests. Compare generated `docs/tool-contracts.md` against the registry and run `git diff --check` in both repositories.

- [ ] **Step 6: Final review checkpoint without committing**

Report the final verified/provisional tool list, exact evidence run, changed files, and remaining deferred DreamChecker/map/runtime work. Leave the promotion edits uncommitted for user review.
