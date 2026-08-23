# Meridian-MCP

Meridian-MCP is a DreamMaker analysis and controlled-development server for Model Context Protocol clients. It uses SpacemanDMM for parsing, indexing, DreamChecker diagnostics, and DMM inspection; DreamMaker remains authoritative for compilation and BYOND remains authoritative for runtime behavior.

The default capability mode is read-only analysis. Development mode must be enabled when the server launches and adds bounded compiler, map-output, DreamDaemon, and loopback `Topic()` operations. Neither mode replaces a target repository's build or test entry point.

## Trust documents

- [Provenance and inherited code](docs/provenance.md)
- [Source authority](docs/source-authority.md)
- [Compatibility and evidence](docs/compatibility.md)
- [Dependency policy](docs/dependency-policy.md)
- [Security policy](SECURITY.md)
- [Architecture](docs/architecture.md)
- [Tool contracts](docs/tool-contracts.md)

## Capability status

| Area | Status | Notes |
| --- | --- | --- |
| Parse, lookup, definitions, and search | Provisional | Unit-tested with purpose-written fixtures; full-corpus evidence is recorded separately. |
| DreamChecker diagnostics | Provisional | SpacemanDMM analysis, not DreamMaker acceptance. |
| DMM/TGM information and search | Provisional | Parser-backed dimensions, counts, and coordinates. |
| DreamMaker compilation | Provisional | A direct compiler gate only; not `BUILD.cmd` or a tgstation full build. |
| PNG rendering, DreamDaemon, and `Topic()` | Provisional | Development mode with containment, loopback, and process-ownership controls. |
| Inherited BYOND client login protocol | Unsupported | Removed because provenance and compatibility evidence were insufficient. |

Support labels are defined in [compatibility](docs/compatibility.md). A passing SpacemanDMM check is useful evidence, not proof that DreamMaker or a repository's full validation suite passes.

## Build

Prerequisites are Rust 1.88 or newer and, for BYOND integration gates, the project-pinned BYOND version.

```powershell
cargo build --release
cargo test
```

The Windows release binary is `target\release\meridian-mcp.exe`.

## Configuration

The server reads immutable startup configuration:

- `MERIDIAN_MCP_MODE`: `analysis` (default) or `development`.
- `MERIDIAN_MCP_ROOTS`: semicolon-separated workspace roots on Windows; platform path-list syntax elsewhere.
- `MERIDIAN_MCP_COMPILERS`: allowlisted DreamMaker executables.

Every configured root must already exist. Client tool calls cannot change the mode, roots, or executable allowlist.

```json
{
  "mcpServers": {
    "meridian-mcp": {
      "command": "C:\\path\\to\\meridian-mcp.exe",
      "env": {
        "MERIDIAN_MCP_MODE": "analysis",
        "MERIDIAN_MCP_ROOTS": "C:\\path\\to\\workspace"
      }
    }
  }
}
```

## Workflow

Call `dm_parse_environment` before source analysis. Use `dm_search_context` for repository-scale discovery, then verify candidates with `dm_get_type`, `dm_get_proc`, `dm_get_var`, or `dm_get_definition`. Reparse after source changes.

Active operations are available only in development mode. `dm_compile` invokes DreamMaker directly; for Meridian-Rift, run the repository's PowerShell/`BUILD.cmd` verification separately before claiming completion.

## Platform support

| Platform | Status |
| --- | --- |
| Windows | Verified only for evidence listed in [compatibility](docs/compatibility.md) |
| Linux | Best effort; Rust-only CI evidence when available |
| macOS | Unsupported and untested |

See [CONTRIBUTING.md](CONTRIBUTING.md) and `TESTING.md` for development and verification guidance.

## License

Meridian-MCP is distributed under MIT. Dependencies retain their own licenses; see [dependency policy](docs/dependency-policy.md).
