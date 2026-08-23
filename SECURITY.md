# Security policy

Report suspected vulnerabilities privately to repository maintainers. Do not put credentials, private source, unsanitized captures, or exploitable public proofs in an issue.

Only the current development branch is supported before 1.0. Security fixes may require upgrading rather than backporting.

The default mode is analysis. Development mode adds file output, local process creation, and loopback networking and should be enabled only for trusted workspaces. Roots and compiler allowlists are immutable startup configuration. Tool calls must not escape roots, overwrite implicitly, choose arbitrary executables, connect to non-loopback hosts, or control unowned processes.

The inherited BYOND client-login protocol is unsupported. `world.Topic()` is a separate loopback-only capability.

See [source authority](docs/source-authority.md), [compatibility](docs/compatibility.md), and [dependency policy](docs/dependency-policy.md).
The detailed threat model is in [security model](docs/security.md).
