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

## Available tools

Analysis mode exposes the eleven read-only tools below. Development mode adds seven active tools; those tools are not advertised or callable unless development mode was enabled when the server launched. See [tool contracts](docs/tool-contracts.md) for the generated capability mode, support level, side effects, timeout, and output limit of every tool.

### Analysis mode

| Tool | Description |
| --- | --- |
| `dm_parse_environment` | Parse a contained `.dme` file and atomically replace the cached object tree and search index. Returns type and indexed-symbol counts; call it before the other source-analysis tools and again after source changes. |
| `dm_get_type` | Inspect an exact DreamMaker type path. Returns its documentation, source location, parent and child types, and variable and procedure metadata, including which members are declared on that type. |
| `dm_get_proc` | Inspect an exact procedure on a type. Returns every parsed implementation with parameters, documentation, source location, and a bounded source excerpt. |
| `dm_get_var` | Inspect an exact variable on a type. Returns its declared type, constant value, whether it has an initial expression, documentation, and source location. |
| `dm_list_types` | Enumerate parsed type paths, optionally restricted by a type-path prefix and maximum traversal depth. |
| `dm_search_symbols` | Find type, procedure, and variable names by case-insensitive partial match, with an optional symbol-kind filter and result limit. |
| `dm_search_context` | Run deterministic ranked retrieval across parsed symbols, documentation, parameters, source paths, and source text. Supports type, type-prefix, and file filters plus optional bounded source excerpts; verify candidates with the exact inspection tools. |
| `dm_check_errors` | Run SpacemanDMM DreamChecker against the cached environment and return structured errors and warnings, optionally filtered to one source file. This is static-analysis evidence, not a DreamMaker compile result. |
| `dm_get_definition` | Resolve an exact type, variable, or procedure to its parsed definition and source location. |
| `dm_map_info` | Parse a contained DMM/TGM map and report its format, grid dimensions, unique-tile count, file size, and most frequent base types and areas. |
| `dm_find_on_map` | Find occurrences of a requested type path and its descendants in a contained DMM/TGM map. Returns BYOND coordinates, exact matched type paths, and tile keys. |

### Development mode

| Tool | Description |
| --- | --- |
| `dm_compile` | Run an allowlisted DreamMaker compiler against a contained `.dme` file. Supports an approved compiler path, working directory, preprocessor defines, total timeout, and idle timeout, and returns bounded compiler output and structured diagnostics. |
| `dm_render_map` | Render one z-level of a contained DMM/TGM map to a PNG using the parsed environment. The output must remain inside an allowed root, and an existing file is replaced only when `overwrite` is explicitly enabled. |
| `dm_run` | Start one server-owned DreamDaemon process for a contained `.dmb` on loopback. Supports a port, working directory, additional daemon arguments, and an optional literal or regular-expression readiness marker. |
| `dm_wait_for_output` | Wait for a literal or regular-expression marker in the bounded output retained from the server-owned DreamDaemon process. Reports matches, timeouts, process exit, and recent output. |
| `dm_status` | Report whether the server-owned DreamDaemon process is running. A live process includes its PID and port; stopped state includes the last exit code, and both include recent captured output. |
| `dm_stop` | Stop and clean up the DreamDaemon process owned by this Meridian-MCP server. It does not target unrelated system processes. |
| `dm_topic` | Send a bounded `world.Topic()` request to the running loopback DreamDaemon process and return the decoded response. This is intended for project-provided debug and test handlers. |

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
