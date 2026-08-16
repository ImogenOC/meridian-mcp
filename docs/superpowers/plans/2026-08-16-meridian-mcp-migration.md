# Meridian MCP Migration and Rebrand Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Migrate the hardened dm-mcp working tree into `meridian-mcp`, rebrand the server/package identity, and preserve the `dm_*` MCP tool API.

**Architecture:** The new repository becomes the canonical Rust MCP checkout. The Rust package, binary, server metadata, and documentation use `meridian-mcp`; tool names remain `dm_*`. Meridian's native stdio launcher resolves the release binary from client-local `MERIDIAN_MCP_REPO` configuration and never embeds that path in Git-tracked files.

**Tech Stack:** Rust 2021, Tokio, serde/serde_json, SpacemanDMM crates, PowerShell, Windows `cmd.exe`, MCP JSON-RPC over stdio.

**Spec:** `docs/superpowers/specs/2026-08-16-meridian-mcp-migration-design.md`

## Global Constraints

- Preserve every current dm-mcp hardening change, including the output-draining runtime supervision and source extraction.
- Preserve every MCP tool name beginning with `dm_`.
- Do not copy `.git`, `target`, compiled binaries, user-local state, or machine-specific paths into the new repository.
- Prefer relative paths in repository scripts and documentation; keep the local checkout path only in client configuration.
- Do not commit or push.
- The original dm-mcp checkout remains untouched until the migrated repository validates.

---

### Task 1: Establish the migrated source tree

**Files:**
- Create: all source, documentation, test, and manifest files currently present in the dm-mcp working tree, excluding `.git`, `target`, and build artifacts.
- Preserve: `Cargo.lock`, `CHANGELOG.md`, `CONTRIBUTING.md`, `README.md`, `TESTING.md`, `src/`, and the three smoke-test scripts.

**Interfaces:**
- Consumes: the complete current dm-mcp working tree.
- Produces: a byte-for-byte source/documentation baseline in `meridian-mcp` before identity edits.

- [ ] **Step 1: Record the source manifest**

Run from the source checkout:

```powershell
Get-ChildItem -Recurse -File | Where-Object { $_.FullName -notmatch '\\(?:\.git|target)\\' } | Resolve-Path | Sort-Object Path
```

- [ ] **Step 2: Copy the source tree without repository metadata or build output**

Copy the source working tree into `meridian-mcp`, preserving uncommitted tracked edits and untracked
documentation. Do not copy `.git`, `target`, `*.exe`, `*.pdb`, or generated runtime logs.

- [ ] **Step 3: Confirm the destination baseline**

Run:

```powershell
git status --short
Get-ChildItem -Force
```

Expected: the destination contains the migrated source and documentation, no nested `.git`, and no
`target` directory copied from the source.

### Task 2: Rebrand the Rust package and server identity

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `src/main.rs`
- Modify: `src/mcp.rs`
- Modify: any source comments or logging filters found by the stale-identity search.

**Interfaces:**
- Consumes: the migrated source baseline.
- Produces: package name `meridian-mcp`, crate logging target `meridian_mcp`, release binary `meridian-mcp(.exe)`, and MCP server name `meridian-mcp`.

- [ ] **Step 1: Add an identity regression check before changing identity**

Search all source and documentation:

```powershell
rg -n 'dm-mcp|dm_mcp|DM_MCP' .
```

Classify each match as project identity, compatibility alias, or stable `dm_*` tool API before editing.

- [ ] **Step 2: Rename package and binary identity**

Change the Cargo package name and any explicit binary metadata to `meridian-mcp`. Update the lockfile
through Cargo rather than hand-editing dependency resolution data.

- [ ] **Step 3: Rename server metadata and logging identity**

Change the MCP `serverInfo.name` and default tracing filter to Meridian identity. Leave every tool
definition and dispatch arm named `dm_*`.

- [ ] **Step 4: Verify identity and API separation**

Run:

```powershell
rg -n 'name: "dm_|"dm_[a-z_]+"|serverInfo|CARGO_PKG_NAME|RUST_LOG' src
```

Expected: all tool names remain `dm_*`, while package/server/logging identity is `meridian`.

### Task 3: Rebrand documentation and test workflow without renaming the tool API

**Files:**
- Modify: `README.md`
- Modify: `CHANGELOG.md`
- Modify: `CONTRIBUTING.md`
- Modify: `TESTING.md`
- Modify: `test_mcp.ps1`
- Modify: `test_parse.ps1`
- Modify: `test-mcp.sh`

**Interfaces:**
- Consumes: the renamed package/server identity and stable `dm_*` tool names.
- Produces: user-facing Meridian MCP documentation, release commands for `meridian-mcp`, and smoke-test output using the new identity.

- [ ] **Step 1: Update public identity text**

Replace project/server branding, repository descriptions, release binary names, and examples. Keep
tool names and compatibility notes explicit.

- [ ] **Step 2: Update smoke-test binary discovery and assertions**

Make the scripts resolve `meridian-mcp(.exe)` from their local repository-relative `target/release`
directory and assert `serverInfo.name == "meridian-mcp"` while continuing to require the existing
`dm_*` tools.

- [ ] **Step 3: Preserve source-backed test paths**

Keep DME paths caller-supplied or relative to the invoking checkout. Do not insert a Meridian or
developer-specific absolute path into the scripts.

### Task 4: Update Meridian's launcher and client registration

**Files:**
- Rename: `tools/dogmos/dm-mcp-launch.cmd` to `tools/dogmos/meridian-mcp-launch.cmd`.
- Modify: `tools/dogmos/README.md`.
- Modify: client-local Codex MCP configuration outside Git repositories.

**Interfaces:**
- Consumes: `MERIDIAN_MCP_REPO` with legacy `DM_MCP_REPO` fallback.
- Produces: a native Windows stdio launcher for `target/release/meridian-mcp.exe` and an updated Codex registration.

- [ ] **Step 1: Rename the launcher and update its environment contract**

Prefer `MERIDIAN_MCP_REPO`; if only `DM_MCP_REPO` exists, accept it as a migration alias and emit no
stdout diagnostics. Resolve the release binary relative to the supplied checkout.

- [ ] **Step 2: Update workflow documentation**

Document that dm-mcp is now Meridian MCP, that `dm_*` tool names are intentionally stable, and that
the deterministic PowerShell/Rust harness remains authoritative for verification.

- [ ] **Step 3: Update Codex registration outside Git**

Point the client-local command at the renamed launcher and set `MERIDIAN_MCP_REPO` to the local
checkout. Keep all machine-specific values outside repository artifacts.

### Task 5: Run the migration validation matrix

**Files:**
- Test: migrated `test_mcp.ps1`, `test_parse.ps1`, and `test-mcp.sh`.
- Test: `meridian-mcp` Rust test/build commands.
- Test: Meridian launcher and repository audits.

**Interfaces:**
- Consumes: the fully migrated and rebranded repository plus updated local client registration.
- Produces: evidence that the renamed server is protocol-compatible, source-aware, and callable through Codex.

- [ ] **Step 1: Run Rust tests and release build**

```powershell
cargo test
cargo build --release
```

Expected: zero test failures and `target/release/meridian-mcp(.exe)` exists.

- [ ] **Step 2: Run protocol and source-backed smoke tests**

```powershell
powershell.exe -NoProfile -File .\test_mcp.ps1 -SkipBuild
powershell.exe -NoProfile -File .\test_mcp.ps1 -SkipBuild -DmePath ..\Meridian-Rift\tgstation.dme -TypePath /turf/open -ProcName AfterChange
powershell.exe -NoProfile -File .\test_parse.ps1 -DmePath ..\Meridian-Rift\tgstation.dme -TypePath /turf/open -ProcName AfterChange
```

Expected: protocol 2024-11-05, all required `dm_*` tools present, Meridian source excerpt returned.

- [ ] **Step 3: Run the configured launcher handshake**

Send `initialize`, `tools/list`, and `tools/call` for `dm_status` through the exact launcher configured
for the client. Verify the response identifies `meridian-mcp` and exposes the stable tools.

- [ ] **Step 4: Run repository hygiene checks**

```powershell
git diff --check
rg --hidden -n -g '!.git/**' -g '!target/**' 'C:\\Users\\|C:/Users/|/Users/' .
```

Expected: no whitespace errors and no developer-specific absolute paths in tracked artifacts.

- [ ] **Step 5: Record validation and remaining compatibility aliases**

Document the exact results in `TESTING.md` or the migration review note. Do not claim completion until
the Codex MCP tool registry exposes the renamed server and direct `dm_*` calls succeed.
