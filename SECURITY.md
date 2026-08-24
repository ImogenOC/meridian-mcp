# Security policy

Report suspected vulnerabilities privately to repository maintainers. Do not put credentials, private source, unsanitized captures, or exploitable public proofs in an issue.

Only the current development branch is supported before 1.0. Security fixes may require upgrading rather than backporting.

The default mode is analysis. Development mode adds file output, local process creation, and loopback networking and should be enabled only for trusted workspaces. Roots, compiler allowlists, and the `MERIDIAN_MCP_RIFT_BUILD=disabled|offline|network` ceiling are immutable startup configuration. Tool calls must not escape roots, overwrite implicitly, choose arbitrary executables, connect to non-loopback hosts, or control unowned processes.

`rift_compile` is absent by default and Windows-only. It can execute only the parsed Meridian-Rift checkout's fixed contained `RIFT_BUILD.cmd`, with a sanitized child environment and no caller-provided command, URL, credential, target, or path. Its offline controls are cooperative rather than a firewall. Optional endpoint auditing is bounded and observational and always reports `capture_complete: false`.

The inherited BYOND client-login protocol is unsupported. `world.Topic()` is a separate loopback-only capability.

See [source authority](docs/source-authority.md), [compatibility](docs/compatibility.md), and [dependency policy](docs/dependency-policy.md).
The detailed threat model is in [security model](docs/security.md).
