# Dependency policy

Direct dependencies use Cargo's lockfile and conservative version requirements. Git dependencies must be pinned to an exact revision in `Cargo.toml`; a floating branch is not releaseable.

Updating SpacemanDMM requires parser and diagnostic fixtures, DMM/TGM tests when relevant, a full Meridian-Rift parse and representative queries, performance comparison, refreshed license/advisory results, and the exact new revision in `Cargo.toml`, `Cargo.lock`, provenance, and compatibility documentation.

The approved integration baseline is SpacemanDMM revision `351ddc0ffb2439876d4565ce5130bb6b027ee605`, whose workspace MSRV is Rust 1.95. Meridian-MCP therefore pins Rust 1.95.0 in both local tooling and CI; use that exact compiler for dependency, Clippy, test, and release evidence.

Run `cargo update` only for an intentional dependency change. Review the lockfile diff, then run formatting, clippy, all-feature tests, release build, and `cargo deny check`. Advisory exceptions must be narrow, documented, time-bounded, and later removed.

## Temporary advisory exceptions

The following RustSec entries report unmaintained transitive crates, not known vulnerabilities. They
remain visible as `cargo deny` notes and expire for review on 2026-11-23.

| Advisory | Dependency path | Removal condition |
| --- | --- | --- |
| `RUSTSEC-2024-0370` | SpacemanDMM `dreammaker` -> `get-size-derive` -> `proc-macro-error` | Remove when the pinned parser graph no longer uses `proc-macro-error`. |
| `RUSTSEC-2024-0388` | SpacemanDMM `dreammaker` -> `derivative` | Remove when SpacemanDMM replaces `derivative`. |
| `RUSTSEC-2024-0425` | SpacemanDMM `dreammaker` -> `get-size` | Remove when SpacemanDMM replaces `get-size`. |
| `RUSTSEC-2024-0427` | SpacemanDMM `dreammaker` -> `get-size-derive` | Remove when SpacemanDMM replaces `get-size-derive`. |
| `RUSTSEC-2024-0436` | `image` -> `exr` -> `pulp` -> `paste` | Remove when a compatible image graph no longer uses `paste`. |
| `RUSTSEC-2025-0141` | Meridian-MCP debugger protocol -> `bincode 1.3.3` | Remove only when the pinned auxtools wire protocol adopts a maintained, byte-compatible codec. |

Do not renew an exception without checking the current SpacemanDMM revision, compatible direct
dependency releases, and the advisory database. A vulnerability or unsoundness advisory is not
covered by these maintenance-only exceptions.

SpacemanDMM's root license at the pinned revision is detected as GPL-3.0-or-later. The cargo-deny
clarifications, including the `dmi` crate which omits a manifest license field, are hash-locked to that file so a source-license change fails closed. Binary distributors
must evaluate obligations for the complete graph. This is not legal advice.
