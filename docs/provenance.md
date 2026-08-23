# Provenance

Meridian-MCP began as a non-ancestral import of [`imcynic/dm-mcp`](https://github.com/imcynic/dm-mcp) at source commit `6a739a4278b53e86b430abaf011467f22c9dd2ec`. The import did not preserve that repository's Git ancestry. The original repository is a provenance reference, not an implementation authority.

The local history begins at `d6ad23a` and currently includes substantial integration revision `a2344cc`. Use local file history together with the source commit above when tracing inherited behavior.

| Subsystem | Disposition |
| --- | --- |
| Hand-written stdio JSON-RPC | Inherited and removed; official `rmcp` 3.1.3 owns transport and negotiation. |
| SpacemanDMM parsing and DreamChecker | Inherited, substantially revised, retained with independent fixtures. |
| Ranked contextual search | Locally implemented. |
| Compiler, DreamDaemon, and `Topic()` adapters | Inherited and substantially revised; retained behind development controls. |
| DMM inspection and rendering | Substantially revised around SpacemanDMM's map pipeline. |
| BYOND client login, packets, and RUNSUB | Inherited, unverified, and removed from the supported product. |

SpacemanDMM crates currently resolve to revision `7fdd00d8e9b7f7583df4960b5ed38269685ec432`. `Cargo.toml` and `Cargo.lock` must name the same exact revision before release.

This engineering inventory is not legal advice. Review dependency licenses under [dependency policy](dependency-policy.md).
