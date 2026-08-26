# Architecture

`main.rs` configures stderr logging and loads immutable `ServerConfig`. `mcp.rs` starts the official `rmcp` stdio transport. `MeridianServer` owns the active mode, canonical `PathPolicy`, and a mutex-protected `ServerState`.

The server exposes only contracts active for its startup configuration. Analysis mode contains parsing, indexing, exact lookup, search, diagnostics, and read-only map inspection. Development mode additionally exposes direct compilation, PNG output, DreamDaemon lifecycle, and loopback `Topic()` calls. Windows `rift_compile` is a further conditional contract controlled by the immutable full-build ceiling.

Tool calls cross these boundaries in order:

1. The SDK negotiates MCP, validates JSON-RPC framing, and presents the tool schema.
2. The contract registry decides whether the tool exists in the configured mode.
3. Path policy canonicalizes inputs, checks roots, enforces compiler allowlisting, and requires explicit output overwrite.
4. Domain adapters invoke SpacemanDMM or controlled BYOND operations and return transport-independent results.
5. The server converts domain results to SDK result models.

Direct DreamMaker compilation and Meridian-Rift full builds share bounded output, timeout, process-tree termination, artifact snapshots, and optional observational endpoint auditing. Tool modules retain command construction and acceptance semantics. `dm_compile` runs one allowlisted compiler. `rift_compile` requires a qualified active profile and runs only the canonical `RIFT_BUILD.cmd`, which delegates to the unchanged human build implementation without accepting client-controlled commands.

A successful parse builds the context, object tree, search index, and project profile before replacing state. It then advances `state_generation`. A failed parse retains the complete prior generation and reports `state_preserved: true`.

Runtime state contains only a DreamDaemon child created by this server, its loopback port, bounded output, reader tasks, and last exit code. Stop and wait operations cannot target arbitrary operating-system processes.

Tracy is a separately gated runtime kind. Preparation installs a verified x86 hook beside the selected DMB. Launch uses the repository-supported `-params tracy` switch and loopback-only `UTRACY_BIND_*` variables, then starts one schema-2 collector for the runtime lifetime. A drain worker consumes the lossless producer queue; each capture rotates through a fresh worker, validates raw-clock conversion, complete frames/zones/source files, hooks and queue telemetry, reopens the serialized trace, and attaches a replacement drain worker before replying. Rust owns both processes, immutable executable/workload identity, request multiplexing, paired trace/sidecar promotion, separate launch-through-stop process-memory series, and integrity checkpoints. Native offline decoding applies each sidecar's authoritative half-open trace range before frame/zone statistics. Comparison and repeated-control aggregation reject incompatible identity before invoking statistics. The Python Tracy MCP, arbitrary expression evaluation, GUI control, and UDP discovery remain outside the product.
