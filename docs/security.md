# Security model

The trust boundary is the launch configuration, not a tool call. `MERIDIAN_MCP_MODE`, `MERIDIAN_MCP_ROOTS`, `MERIDIAN_MCP_COMPILERS`, and `MERIDIAN_MCP_RIFT_BUILD` are loaded before stdio starts and cannot be expanded by the client. The full-build values are exactly `disabled`, `offline`, and `network`; callers can select only `network_mode=offline` or `network_mode=allow` within that ceiling.

Existing inputs are canonicalized and must remain below an allowed root. New outputs require an existing contained parent. Existing outputs require `overwrite=true`. Caller-selected compilers must match the canonical allowlist. Automatically discovered DreamMaker installations remain a server policy decision.

Network operations target the loopback DreamDaemon owned by the server. The product has no internet-facing administration and no supported BYOND client-login protocol. Runtime lifecycle methods manage only the stored child handle.

Tracy is disabled by default and cannot be enabled by a tool call. Startup validation requires exact source revisions, protocol 82, platform/architecture identities, BYOND bounds, contained helper paths, and SHA-256 hashes. The profiled runtime binds byond-tracy to `127.0.0.1` on an MCP-selected port, uses fixed `-params tracy`, and excludes caller-defined environment values. The native helper accepts one bounded JSON request from stdin and has no arbitrary code, expression, Python, or HTTP execution surface. Only one capture may run; cancellation terminates the contained helper tree before the owned DreamDaemon is stopped.

On Windows, `dm_compile` and `rift_compile` use a shared bounded process runner. An owned Job Object terminates descendants on timeout or cancellation. Standard output and error are size-limited, and generated artifacts are measured with timestamps, sizes, and SHA-256 hashes. `rift_compile` constructs the command from a qualified parsed profile, a trusted system `cmd.exe`, and the canonical root `RIFT_BUILD.cmd`; its schema exposes no arbitrary executable, path, argument vector, target, URL, credential, or environment field.

The full-build child receives a sanitized allowlist of required operating-system variables plus the selected DreamMaker path and fixed build flags. A caller cannot inject credentials. `TG_BOOTSTRAP_CACHE` is retained only when its canonical directory is inside an allowed root. Offline mode checks a warm pinned cache and supplies Bun and pip no-network controls before delegating; these controls are cooperative and are not proof that the operating system blocked all network activity.

When `capture_network=true`, Windows periodically samples TCP and UDP owner tables for the owned process tree. Short-lived endpoints can be missed, so samples are observational, bounded, and always include `capture_complete: false`. Audit unavailability is a warning rather than false proof of no traffic.

Tool errors are caller-visible MCP tool results with stable policy codes such as `path_outside_workspace`, `output_exists`, `executable_not_allowed`, and `tool_not_available`. Protocol errors are reserved for requests the SDK cannot route.

These controls reduce accidental and model-driven misuse; they do not make an untrusted repository safe to compile or run. Development mode should be used only with trusted workspaces.
