# Security model

The trust boundary is the launch configuration, not a tool call. `MERIDIAN_MCP_MODE`, `MERIDIAN_MCP_ROOTS`, and `MERIDIAN_MCP_COMPILERS` are loaded before stdio starts and cannot be expanded by the client.

Existing inputs are canonicalized and must remain below an allowed root. New outputs require an existing contained parent. Existing outputs require `overwrite=true`. Caller-selected compilers must match the canonical allowlist. Automatically discovered DreamMaker installations remain a server policy decision.

Network operations target the loopback DreamDaemon owned by the server. The product has no internet-facing administration and no supported BYOND client-login protocol. Runtime lifecycle methods manage only the stored child handle.

Tool errors are caller-visible MCP tool results with stable policy codes such as `path_outside_workspace`, `output_exists`, `executable_not_allowed`, and `tool_not_available`. Protocol errors are reserved for requests the SDK cannot route.

These controls reduce accidental and model-driven misuse; they do not make an untrusted repository safe to compile or run. Development mode should be used only with trusted workspaces.
