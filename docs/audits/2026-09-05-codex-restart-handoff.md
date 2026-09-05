# Codex restart handoff — 2026-09-05

The pushed remediation is installed and selected for the next Codex restart. The existing installation and configuration backup are retained. No source implementation changes were made during deployment.

## Release and configuration

- Source: `da29e984b7d7cf1c31cac9f64b1d1d93c13c0ff9`, built with pinned Rust 1.95.0 from a clean checkout.
- [GitHub Actions run 33981974113](https://github.com/ImogenOC/meridian-mcp/actions/runs/33981974113) completed successfully for that commit. All six jobs passed, including Windows/Linux Rust and native Tracy checks.
- Installed executable: `%LOCALAPPDATA%/meridian-mcp/releases/da29e984b7d7/meridian-mcp.exe`.
- SHA-256: `0732e62e4896a087c2b7991982c670112d571d91a1b5030e11f1ee79043c324e`.
- MCP build ID: `742e0f498d56c47751cfa08a8bd8a58d5011e9daf38d306b0c30e0eead07f9e3`.
- The `meridian-mcp` registration now points to that executable and its adjacent `helpers/manifest.json`.
- Existing roots, repository discovery, private state, compiler allowlist, network build ceiling, debugger, Tracy, timeouts and unrelated Codex settings were preserved. Only the executable and manifest values changed.

The package includes the verified dmdoc helper, pinned auxtools DLL, and the previously qualified native Tracy helper/hook. Helper hashes were checked during staging and installation; native patch identity metadata and license files were retained.

## Startup repair

Repository discovery initially prevented initialization because a linked worktree was missing but still registered with a three-day-old `initializing` lock. No active task used that path. Its complete Git administrative record was backed up and hash-checked, then the lock was removed and Git's dry-run confirmed exactly one stale record before pruning it. Existing checkout files and branches were not changed. All seven existing effective roots, including the other tasks' linked worktrees, remain authorized through the original configuration.

Backups are under `%LOCALAPPDATA%/meridian-mcp/restart-backups/20260905-da29e984/`: `config.toml.before` and `stale-worktree-Meridian-Rift4/`. The prior executable and helper package remain in their original installation directory. A rollback should restore only the former Meridian command and manifest settings from the backup if unrelated configuration has changed since deployment.

## Installed-server verification

A standalone session used the installed executable with the preserved startup environment and working directory:

| Check | Result |
| --- | --- |
| Build identity | Complete; pushed commit; `source_dirty: false`; installed hash matched |
| Tool inventory | 59 tools; documentation, debugger, Tracy and network Rift build enabled |
| Private state | Ready |
| Clean Meridian-Rift parse | Passed; 25,256 ms observed |
| Repeated parse | Reused generation 1; 1,226 ms observed |
| Cached diagnostics | `cached_snapshot`, no recomputation; 1,039 diagnostics |
| Canonical search | Correct `/datum/controller/subsystem/mapping` first result |
| Installed dmdoc | Generated eight files, including `index.html`, from the owned fixture |
| Session shutdown | EOF completed with exit code 0 |

Raw local evidence remains ignored under `target/restart-*`. Parsing is analysis evidence; this deployment check did not compile or boot the primary game. Prior runtime qualification is recorded in the [remediation verification report](2026-09-05-remediation-verification.md).

## Resume after restart

1. Fully quit and reopen Codex, as required by the repository's [installation verification procedure](../../README.md#verify-a-codex-installation). Current task connections do not establish post-restart acceptance.
2. Confirm `codex mcp get meridian-mcp` selects the release above.
3. In a resumed task, call `dm_server_status` and confirm the installed build ID and expected capabilities.
4. Parse that task's authorized DME before source tools. Repeat the parse to confirm reuse, then check cached diagnostics and search. Each new server session needs its own parse; a previous task's cached snapshot is not restart evidence.

Codex's shared MCP configuration is documented in the [official MCP guide](https://learn.chatgpt.com/docs/extend/mcp?surface=cli). The separate debugger stop-latency follow-up remains in the [workplan](../superpowers/plans/2026-09-05-mcp-audit-remediation.md).

## Post-restart acceptance

After the user restarted Codex on 2026-09-05, live app-tool calls confirmed the exact release build ID and executable hash above, all seven effective roots, ready private state, and enabled documentation, debugger, Tracy and network build capabilities. The server began at analysis generation 0 with no parsed environment.

Parsing the clean primary Meridian-Rift checkout at `7462a6942b2e71a3ea13c00169f65f575cb281b7` installed generation 1 in 30,925 ms with 452,780 indexed symbols. Repeating the parse reused generation 1 in 1,306 ms. Diagnostics returned the cached snapshot without recomputation: 1,039 total, comprising 127 error-severity diagnostics and 912 hints. Exact search returned `/datum/controller/subsystem/mapping` first.

Post-restart MCP acceptance passed. These checks did not compile or boot the game or rerun runtime helper fixtures. Other resumed tasks must parse their own DME before analysis tools.
