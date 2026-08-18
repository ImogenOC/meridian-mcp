# meridian-mcp

**MCP Server for DreamMaker/BYOND Development**

An [MCP (Model Context Protocol)](https://modelcontextprotocol.io/) server that provides AI coding assistants with full access to DreamMaker language tooling. Works with Claude, Cursor, Windsurf, Continue, and any MCP-compatible client.

## Features

### Code Intelligence
- **Parse & Navigate** - Load `.dme` projects, explore type hierarchies, search symbols
- **Type Checking** - Real-time diagnostics via SpacemanDMM's dreamchecker
- **Go to Definition** - Find where any type, proc, or variable is defined

### Compilation & Runtime
- **Compile** - Build projects with the DM compiler, structured diagnostics, defines, working-directory control, and a timeout. Relative DME paths resolve from the requested working directory, matching `dm_run`; parsed compiler errors fail the tool even when DreamMaker exits with code 0.
- **Run/Stop** - Control DreamDaemon instances while continuously capturing stdout/stderr and exit codes
- **Runtime Readiness** - Wait for literal or regular-expression output markers instead of guessing with fixed sleeps
- **Topic Calls** - Send `Topic()` messages to running servers

### Map Tools
- **Map Info** - Get dimensions, z-levels, area statistics
- **Find on Map** - Search for object instances across maps
- **Render Maps** - Generate PNG previews of `.dmm` files

### Protocol Support
- Full BYOND client protocol implementation (for testing)
- RUNSUB encryption for secure packet handling

## Installation

### Prerequisites
- [Rust](https://rustup.rs/) 1.70+
- [BYOND](http://www.byond.com/download/) (for compilation and runtime features)

### Build from Source

```bash
# From a local checkout of this repository:
cd meridian-mcp
cargo build --release
```

The release binary will be at `target/release/meridian-mcp` on Unix-like systems and
`target/release/meridian-mcp.exe` on Windows.

### Configure Your MCP Client

#### Claude Code / Cursor / Windsurf

Add to your MCP settings (usually `~/.config/claude/mcp.json` or IDE settings):

```json
{
  "mcpServers": {
    "meridian-mcp": {
      "command": "/path/to/meridian-mcp"
    }
  }
}
```

#### Continue

Add to `~/.continue/config.json`:

```json
{
  "experimental": {
    "modelContextProtocolServers": [
      {
        "name": "meridian-mcp",
        "transport": {
          "type": "stdio",
          "command": "/path/to/meridian-mcp"
        }
      }
    ]
  }
}
```

## Available Tools

| Tool | Description |
|------|-------------|
| `dm_parse_environment` | Parse a `.dme` file and cache the object tree |
| `dm_get_type` | Get type info (vars, procs, parent, children) |
| `dm_get_proc` | Get proc details (params, location, docs) |
| `dm_get_var` | Get variable info (type, initial value) |
| `dm_list_types` | List types with optional path prefix filter |
| `dm_search_symbols` | Search for types, procs, or vars by pattern |
| `dm_check_errors` | Run type checker, get diagnostics |
| `dm_get_definition` | Find source location of any symbol |
| `dm_compile` | Compile the project with optional compiler path, working directory, defines, bounded timeout/watchdog, and structured diagnostics |
| `dm_render_map` | Render a map to PNG |
| `dm_map_info` | Get map dimensions and statistics |
| `dm_find_on_map` | Find instances of a type on a map |
| `dm_run` | Start DreamDaemon with a `.dmb` file, optional working directory, and extra daemon arguments |
| `dm_wait_for_output` | Wait for a literal or regex marker in DreamDaemon output |
| `dm_stop` | Stop the running game |
| `dm_status` | Get game server status |
| `dm_topic` | Send a Topic() call to the server |
| `dm_connect_test` | Test BYOND client protocol |

## Example Usage

Once configured, you can ask your AI assistant things like:

> "Parse /code/myss13/myss13.dme and show me the type hierarchy for /mob/living"

> "Find all instances of /obj/machinery/door on the station map"

> "Check for type errors in the codebase"

> "Compile the project and run it on port 1337"

## How It Works

meridian-mcp uses [SpacemanDMM](https://github.com/SpaceManiac/SpacemanDMM) for parsing and type checking. This is the same tooling used by the SS13 community for linting and IDE support.

The server communicates over stdio using JSON-RPC, following the MCP specification.

Runtime output is drained from stdout and stderr in fixed 8 KiB chunks so an unterminated line cannot
grow without limit. Retained diagnostics use a 500-line ring buffer, truncate any single line beyond
16 KiB with a `... [truncated]` suffix, and cap total retained line bytes at 1 MiB. `dm_status`,
immediate-start failures, and `dm_wait_for_output` expose the captured tail and the last exit code to
make crashes diagnosable.

`dm_get_proc` includes a bounded source excerpt for each override when its source file is available.
Relative source paths are resolved from the loaded `.dme` directory, which is important for projects
whose parser context reports paths such as `code/game/turfs/change_turf.dm`.

Tool names intentionally remain on the `dm_*` prefix for compatibility with existing MCP clients and
workflows.

## Platform Support

- **Windows** - Full support (BYOND native)
- **Linux** - Full support (requires BYOND Linux build)
- **macOS** - Untested but should work with BYOND Wine wrapper

## BYOND Paths

The server looks for BYOND in these locations:

**Windows:**
- `C:\Program Files (x86)\BYOND\bin\`
- `C:\Program Files\BYOND\bin\`

**Linux:**
- `/usr/local/byond/bin/`
- `/opt/byond/bin/`
- System PATH (`dm`, `DreamMaker`, `dreamdaemon`, `DreamDaemon`)

## Development

```bash
# Run with debug logging
RUST_LOG=meridian_mcp=debug cargo run

# Run tests
cargo test

# Read the complete testing and integration workflow
# See TESTING.md

# Verify JSON-RPC, dynamic tool schemas, and required tools (Windows PowerShell)
pwsh -File .\test_mcp.ps1

# Parse a real project and exercise source-backed symbol queries
pwsh -File .\test_parse.ps1 -DmePath C:\path\to\project.dme -TypePath /turf/open -ProcName AfterChange

# The shell wrapper delegates to PowerShell on Windows and uses jq/timeout on Linux
./test-mcp.sh --skip-build

# Build release
cargo build --release
```

## License

MIT

## Contributing

Issues and PRs welcome! This project aims to bring modern AI tooling to the BYOND/DreamMaker ecosystem.

---

*Built with ❤️ for the SS13 community*
