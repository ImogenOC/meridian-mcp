# Architecture

`main.rs` configures stderr logging and loads immutable `ServerConfig`. `mcp.rs` starts the official `rmcp` stdio transport. `MeridianServer` owns the active mode, canonical `PathPolicy`, and a mutex-protected `ServerState`.

The server exposes only contracts active for its startup mode. Analysis mode contains parsing, indexing, exact lookup, search, diagnostics, and read-only map inspection. Development mode additionally exposes direct compilation, PNG output, DreamDaemon lifecycle, and loopback `Topic()` calls.

Tool calls cross these boundaries in order:

1. The SDK negotiates MCP, validates JSON-RPC framing, and presents the tool schema.
2. The contract registry decides whether the tool exists in the configured mode.
3. Path policy canonicalizes inputs, checks roots, enforces compiler allowlisting, and requires explicit output overwrite.
4. Domain adapters invoke SpacemanDMM or controlled BYOND operations and return transport-independent results.
5. The server converts domain results to SDK result models.

A successful parse builds the context, object tree, search index, and project profile before replacing state. It then advances `state_generation`. A failed parse retains the complete prior generation and reports `state_preserved: true`.

Runtime state contains only a DreamDaemon child created by this server, its loopback port, bounded output, reader tasks, and last exit code. Stop and wait operations cannot target arbitrary operating-system processes.
