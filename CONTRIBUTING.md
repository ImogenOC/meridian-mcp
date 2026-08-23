# Contributing to Meridian-MCP

Read [source authority](docs/source-authority.md), [provenance](docs/provenance.md), [compatibility](docs/compatibility.md), and [dependency policy](docs/dependency-policy.md) first.

## Workflow

1. Research the existing adapter and relevant SpacemanDMM or BYOND interface.
2. Add a failing behavioral test using a purpose-written fixture. Do not copy tgstation code into unit fixtures.
3. Make the smallest implementation change that passes the focused test.
4. Run `cargo fmt --all -- --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and `cargo test --all-features`.
5. Run installed-binary and BYOND gates when touching transport, compilation, maps, runtime, or `Topic()`.
6. Report focused checks as iteration evidence. Claim completion only after the full applicable matrix passes.

Tests should assert behavior, contracts, schemas, containment, and link integrity. Do not test exact human prose.

DreamMaker is the language acceptance authority. SpacemanDMM diagnostics must be identified as analysis results. Repository-specific build tooling remains authoritative for full-project validation.

Pin git dependencies by exact revision. SpacemanDMM updates must follow [dependency policy](docs/dependency-policy.md).

Bug reports should include operating system, Rust and BYOND versions, server mode, client/version, a minimal reproduction, expected and actual behavior, and sanitized stderr. Never attach credentials or private packet captures.
