# RIFT Controller and MCP Contract Repair Plan

> **For Codex:** Execute this plan inline with the `superpowers:executing-plans` workflow. Preserve existing uncommitted Meridian-MCP changes and leave all work uncommitted.

**Goal:** Make RIFT process termination, numeric bounds, compile evidence, and MCP timeout ownership explicit and testable across the controller and `rift_compile` integration.

**Architecture:** RIFT remains the owner of the child DreamMaker process and emits one compact, versioned compile-result record. Meridian-MCP owns the outer wrapper process, supplies a shorter inner wall timeout plus the requested inner idle timeout through validated environment inputs, and reserves outer cleanup time. Legacy cache markers remain accepted for compatibility with older RIFT branches.

**Tech Stack:** Bun/TypeScript tests for RIFT; Rust/Serde/Tokio tests for Meridian-MCP; repository-pinned formatters and test runners.

---

### Task 1: Preserve process termination semantics in RIFT

- [x] Add failing tests covering compile wall timeout, compile cancellation, and runtime readiness timeout classification.
- [x] Add a typed termination-to-`RiftError` mapping and use it at compile, readiness, test, and bounded-run process result boundaries.
- [x] Verify focused RIFT tests pass.

### Task 2: Bound RIFT timeout inputs

- [x] Add failing tests for CLI and profile values above supported wall, idle, readiness, and lock-wait limits.
- [x] Centralize maximum values and apply them to command-line, profile, and environment parsing.
- [x] Document accepted ranges and verify focused tests pass.

### Task 3: Add a versioned RIFT compile-result contract

- [x] Add failing RIFT tests for a compact `RIFT_RESULT` record and launcher selection of that output format.
- [x] Implement the result renderer, output format, and launcher update with artifact hashes, freshness, evidence, status, exit code, and reuse state.
- [x] Add failing Meridian-MCP tests for valid reuse evidence and invalid/mismatched result records.
- [x] Parse and validate the result record against captured canonical artifacts while retaining legacy marker compatibility.

### Task 4: Establish nested timeout ownership

- [x] Add failing RIFT tests for validated environment timeout defaults and CLI precedence.
- [x] Add failing Meridian-MCP tests for controller timeout environment values and reserved outer cleanup time.
- [x] Implement the environment handshake, keep outer idle monitoring from pre-empting the silent wrapper, and expose the effective timeout policy in response details.
- [x] Update both repositories' contract documentation.

### Task 5: Verify and audit the completed changes

- [x] Run RIFT formatting/lint/type/unit gates using its maintained commands.
- [x] Run Meridian-MCP formatting, focused tests, and repository test gates with the pinned Rust toolchain.
- [x] Inspect both diffs for unrelated edits and unresolved findings.
- [x] Record the remaining exact-commit/live gate: both repair sets are intentionally uncommitted, so production `rift_compile` plus RIFT boot/test/soak evidence must be rerun after an authorized commit and installed MCP restart.
