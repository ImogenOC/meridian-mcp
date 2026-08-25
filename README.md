# Meridian-MCP

Meridian-MCP is a DreamMaker analysis and controlled-development server for Model Context Protocol clients. It uses SpacemanDMM for parsing, indexing, DreamChecker diagnostics, and DMM inspection; DreamMaker remains authoritative for compilation and BYOND remains authoritative for runtime behavior.

The default capability mode is read-only analysis. Development mode must be enabled when the server launches and adds bounded compiler, map-output, DreamDaemon, and loopback `Topic()` operations. Neither mode replaces a target repository's build or test entry point.

## Trust documents

- [Provenance and inherited code](docs/provenance.md)
- [Source authority](docs/source-authority.md)
- [Compatibility and evidence](docs/compatibility.md)
- [Dependency policy](docs/dependency-policy.md)
- [Security policy](SECURITY.md)
- [Detailed security model](docs/security.md)
- [Architecture](docs/architecture.md)
- [Tool contracts](docs/tool-contracts.md)
- [Testing](TESTING.md)

## Capability status

| Area | Status | Notes |
| --- | --- | --- |
| Parse, lookup, definitions, and search | Provisional | Unit-tested with purpose-written fixtures; full-corpus evidence is recorded separately. |
| DreamChecker diagnostics | Provisional | SpacemanDMM analysis, not DreamMaker acceptance. |
| DMI profiling, extraction, duplicate detection, and icon audit | Experimental | Pixel/metadata analysis is bounded and non-mutating; hotspot parsing remains incomplete upstream. |
| DMM/TGM information, differences, search, and rendering | Provisional | Parser-backed dimensions, models, coordinates, render passes, bounds, and typed batches. |
| HTML documentation | Experimental | Exact-revision, hash-verified dmdoc helper; unavailable unless packaged at startup. |
| Auxtools debugger | Experimental | Windows development-mode opt-in; one MCP-owned DreamSeeker session, no attach/restart/disassembly. |
| DreamMaker compilation | Provisional | A direct compiler gate only; not `BUILD.cmd` or a tgstation full build. |
| Meridian-Rift full build | Provisional | Windows-only `rift_compile` through the contained `RIFT_BUILD.cmd`; promotion awaits a recorded green named integration run. |
| PNG rendering, DreamDaemon, and `Topic()` | Provisional | Development mode with containment, loopback, and process-ownership controls. |
| Inherited BYOND client login protocol | Unsupported | Removed because provenance and compatibility evidence were insufficient. |

Support labels are defined in [compatibility](docs/compatibility.md). A passing SpacemanDMM check is useful evidence, not proof that DreamMaker or a repository's full validation suite passes.

## Available tools

Analysis mode exposes the eleven read-only tools below. Development mode adds seven general active tools and can add the separately gated `rift_compile`; active tools are not advertised or callable unless development mode was enabled when the server launched. See [tool contracts](docs/tool-contracts.md) for the generated capability mode, support level, side effects, timeout, and output limit of every tool.

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
| `dm_document_symbols` | List nested type, variable, and procedure declarations in one parsed source file. |
| `dm_find_references` | Find semantic read/write references to an exact canonical declaration, excluding shadowed locals and unresolved dynamic accesses. |
| `dm_find_implementations` | List parent/child implementations for an exact type, variable, or procedure identity in stable order. |
| `dm_dmi_info` | Profile DMI metadata, states, directions, frames, dimensions, hashes, and parser warnings without changing the asset. |
| `dm_compare_dmi_states` | Compare two complete DMI states for exact, cropped, palette-only, mirrored, rotated, and scaled lazy changes. |
| `dm_find_dmi_duplicates` | Scan bounded contained DMI scopes, bucket candidates, and cluster cross-file exact or lazy-change duplicates. |
| `dm_audit_icons` | Resolve static inherited `icon`/`icon_state` references, report missing evidence and dynamic expressions, and optionally estimate unused states. |
| `dm_map_info` | Parse a contained DMM/TGM map and report its format, grid dimensions, unique-tile count, file size, and most frequent base types and areas. |
| `dm_diff_maps` | Compare coordinate models across two DMM/TGM maps independently of dictionary keys, with bounded structured differences. |
| `dm_list_render_passes` | List every render pass from the pinned SpacemanDMM revision, including its default state and description. |
| `dm_find_on_map` | Find occurrences of a requested type path and its descendants in a contained DMM/TGM map. Returns BYOND coordinates, exact matched type paths, and tile keys. |

### Development mode

| Tool | Description |
| --- | --- |
| `dm_compile` | Run an allowlisted DreamMaker compiler directly against a contained `.dme` file. Supports an approved compiler path, working directory, preprocessor defines, total and idle timeouts, and optional best-effort process-tree endpoint observation. Returns bounded output, structured diagnostics, and before/after DMB evidence. It is not a repository full build. |
| `rift_compile` | On Windows, run the active qualified Meridian-Rift checkout's fixed `RIFT_BUILD.cmd` full-build wrapper. It accepts no paths, commands, URLs, targets, credentials, or arbitrary environment values. Availability requires development mode plus an explicit startup ceiling; results distinguish fresh artifacts, valid cache hits, failures, and insufficient evidence. |
| `dm_render_map` | Render one z-level of a contained DMM/TGM map to a PNG using the parsed environment. The output must remain inside an allowed root, and an existing file is replaced only when `overwrite` is explicitly enabled. |
| `dm_render_maps` | Preflight and render a bounded typed batch of contained map chunks in request order; it accepts no raw RenderMany command. |
| `dm_extract_dmi` | Mechanically extract one frame/direction or a contact sheet from a selected DMI state to an atomic contained PNG. |
| `dm_generate_docs` | Generate contained HTML from the active environment using only the packaged, exact-revision, hash-verified dmdoc helper. |
| `dm_run` | Start one server-owned DreamDaemon process for a contained `.dmb` on loopback. Supports a port, working directory, additional daemon arguments, and an optional literal or regular-expression readiness marker. |
| `dm_wait_for_output` | Wait for a literal or regular-expression marker in the bounded output retained from the server-owned DreamDaemon process. Reports matches, timeouts, process exit, and recent output. |
| `dm_status` | Report whether the server-owned DreamDaemon process is running. A live process includes its PID and port; stopped state includes the last exit code, and both include recent captured output. |
| `dm_stop` | Stop and clean up the DreamDaemon process owned by this Meridian-MCP server. It does not target unrelated system processes. |
| `dm_topic` | Send a bounded `world.Topic()` request to the running loopback DreamDaemon process and return the decoded response. This is intended for project-provided debug and test handlers. |

When `MERIDIAN_MCP_DEBUGGER=auxtools` is enabled under development mode and the fixed DLL installation validates, the server also advertises `dm_debug_launch`, `dm_debug_stop`, `dm_debug_set_breakpoints`, `dm_debug_set_function_breakpoints`, `dm_debug_set_exception_breakpoints`, `dm_debug_control`, `dm_debug_threads`, `dm_debug_stack_trace`, `dm_debug_scopes`, `dm_debug_variables`, `dm_debug_evaluate`, `dm_debug_exception_info`, `dm_debug_source`, and `dm_debug_wait_for_event`. These tools own one DreamSeeker process, use loopback only, and never attach to an arbitrary PID.

## Build

The repository pins Rust 1.95.0 with rustfmt and Clippy to match CI. BYOND integration gates additionally require the project-pinned BYOND version.

```powershell
cargo build --release
cargo test
```

The release binary is `target\release\meridian-mcp.exe` on Windows and `target/release/meridian-mcp` on Linux.

## Configuration

The server reads immutable startup configuration:

- `MERIDIAN_MCP_MODE`: `analysis` (default) or `development`.
- `MERIDIAN_MCP_ROOTS`: semicolon-separated workspace roots on Windows; platform path-list syntax elsewhere.
- `MERIDIAN_MCP_COMPILERS`: allowlisted DreamMaker executables.
- `MERIDIAN_MCP_RIFT_BUILD`: `disabled` (default), `offline`, or `network`. The ceiling is immutable and `rift_compile` remains absent unless enabled.
- `MERIDIAN_MCP_HELPER_MANIFEST`: build-produced manifest for the exact dmdoc helper; absent or mismatched helpers keep `dm_generate_docs` unavailable.
- `MERIDIAN_MCP_DEBUGGER`: `disabled` (default) or `auxtools`. Auxtools requires development mode, one allowlisted `dm.exe`, its sibling `dreamseeker.exe`, and the fixed hash-verified DLL beside Meridian-MCP.

Every configured root must already exist. Client tool calls cannot change the mode, roots, executable allowlist, or full-build ceiling. `rift_compile` defaults to `network_mode=offline`; `network_mode=allow` is accepted only under the startup value `network`. Offline mode is cooperative preflight and strict process-local package-manager configuration, not an operating-system firewall.

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

Active operations are available only in development mode. `dm_compile` invokes DreamMaker directly. For Meridian-Rift, `rift_compile` invokes the separate agent-owned `RIFT_BUILD.cmd`; humans continue to use the authoritative `BUILD.cmd`. Full-build output can optionally include bounded, observational endpoint samples, but `capture_complete` is always `false`.

## Platform support

| Platform | Status |
| --- | --- |
| Windows | Verified only for evidence listed in [compatibility](docs/compatibility.md) |
| Linux | Provisional Ubuntu 24.04 CI gate for Rust, the release binary, stdio MCP, and owned-fixture parse/search; no BYOND claim |
| macOS | Unsupported and untested |

See [CONTRIBUTING.md](CONTRIBUTING.md) and [TESTING.md](TESTING.md) for development and verification guidance.

## License

Meridian-MCP is distributed under MIT. Dependencies retain their own licenses; see [dependency policy](docs/dependency-policy.md).
