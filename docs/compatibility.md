# Compatibility and evidence

- **Verified:** committed automated tests and a documented integration gate pass on a named platform and version.
- **Provisional:** useful automated coverage exists, but part of the integration matrix remains untested.
- **Experimental:** behavior may change or fail and is disabled by default when it crosses a security or protocol boundary.
- **Unsupported:** not exposed in normal operation or removed.

| Component | Target | Status | Evidence |
| --- | --- | --- | --- |
| Windows | Current host, 2026-08-23 | Verified | All-feature Rust suite, installed stdio smoke, and full Meridian-Rift parse/search passed. |
| Linux | Current CI runner | Best effort | Rust-only checks when CI runs; no BYOND claim. |
| macOS | Any | Unsupported | No test evidence. |
| Rust | 1.88 minimum | Verified | Declared in Cargo and configured in CI. |
| BYOND | 516.1685 Meridian-Rift pin | Provisional | Full-project source parsing/search passed; native fixture compile/runtime remains blocked by the restricted host process context. |
| SpacemanDMM | `7fdd00d8e9b7f7583df4960b5ed38269685ec432` | Provisional | Parser, search, map, and diagnostic tests. |
| MCP transport | `rmcp` 3.1.3 | Verified | Official SDK tests and installed stdio negotiation/tool smoke passed. |

Update this table only from fresh, reproducible evidence. Never infer platform support from another operating system.
