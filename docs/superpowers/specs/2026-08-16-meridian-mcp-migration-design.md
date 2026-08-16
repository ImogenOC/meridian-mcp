# Meridian MCP Migration and Rebrand Design

Status: approved by the maintainer.

## Goal

Move the maintained DreamMaker MCP implementation into the `meridian-mcp` repository, rebrand its
project and server identity, and preserve the existing `dm_*` tool API so current workflows do not
break.

## Identity and compatibility

- Repository identity: `meridian-mcp`.
- Rust package and release binary: `meridian-mcp` / `meridian-mcp.exe`.
- MCP `serverInfo.name`: `meridian-mcp`.
- Rust logging target and user-facing documentation: Meridian MCP.
- MCP tool names remain unchanged (`dm_parse_environment`, `dm_get_proc`, `dm_compile`, and so on).
- The local launcher prefers `MERIDIAN_MCP_REPO` and accepts `DM_MCP_REPO` as a migration alias.

The tool names are an API boundary. Rebranding them would create unnecessary configuration churn and
would not improve DreamMaker usability.

## Migration boundary

The current dm-mcp working tree is the source of truth for the migration. Copy its tracked and
untracked source/documentation changes into `meridian-mcp`, excluding `.git`, `target`, compiled
artifacts, and machine-local state. The original checkout remains untouched until the new repository
passes validation.

## Repository layout

The migrated repository keeps the existing focused layout:

- `src/`: Rust MCP implementation.
- `test_mcp.ps1`, `test_parse.ps1`, `test-mcp.sh`: protocol and source-backed smoke tests.
- `TESTING.md`: operational test contract.
- `docs/superpowers/specs/`: approved design records.
- `docs/superpowers/plans/`: implementation plans.

Functional smoke-test filenames remain stable because they are workflow entry points, not branding
surfaces.

## Client integration

The Meridian repository's native Windows stdio launcher resolves the release binary from
`MERIDIAN_MCP_REPO`, forwards all standard handles, and emits no diagnostics on stdout. The Codex
registration points to that launcher and keeps the checkout location in client-local configuration,
never in a Git-tracked file.

## Validation

The migration is complete only when all of the following pass:

1. Rust unit tests.
2. Release build producing `meridian-mcp(.exe)`.
3. Protocol smoke test with the renamed binary.
4. Meridian DME parse, type, and source-backed proc query.
5. Exact Codex launcher handshake.
6. Search proving no stale public `dm-mcp` identity remains except intentional compatibility aliases
   and `dm_*` tool names.
7. Repository diff checks and PowerShell syntax checks.
8. No developer-specific absolute paths in tracked repository artifacts.

No commits or pushes are part of this migration.
