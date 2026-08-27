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
| Tracy profiling | Meridian-owned fixed-command integration over pinned Tracy server internals and pinned byond-tracy; no code or arbitrary-expression surface is imported from the Python Tracy MCP. |

Experiment identity, half-open range envelopes, role-specific process sampling, owned-loopback evidence, compatibility checks, and repeated-control noise calculations are Meridian-owned layers. Tracy supplies trace collection/serialization and server data structures; byond-tracy supplies the BYOND hook. No third-party MCP output, evaluator, or control-baseline policy is treated as authoritative.
| BYOND client login, packets, and RUNSUB | Inherited, unverified, and removed from the supported product. |

SpacemanDMM crates currently resolve to revision `351ddc0ffb2439876d4565ce5130bb6b027ee605`. Tracy helpers are built from Tracy `099df3de3dc37eca4712c06b8320fb9c53596edd` (v0.14.0) and byond-tracy `d1ec404737b04b1ea73d6df4a1b477deacdb1900` (build-d1ec404), using Tracy protocol 82. The builder requires clean source checkouts, copies them into private build roots, applies `tracy-clock-access.patch`, then applies `byond-tracy-empty-queue.patch` and `byond-tracy-health.patch` in that fixed order. It runs all Meridian CTests and records source, patch, x64 helper, and x86 hook hashes in helper-manifest schema v2. Runtime code never downloads or edits these upstream sources.

Tracy is BSD-3-Clause and byond-tracy is BSD-2-Clause. Packaged copies of both license files accompany the native artifacts. The binding/API sources in Tracy are a reference for server internals; Meridian-MCP exposes its own fixed protocol and does not ship the Tracy GUI or Python evaluator/HTTP MCP surface.

The packaged hook applies one Meridian-owned source patch after verifying the upstream Git HEAD: `helpers/tracy/byond-tracy-empty-queue.patch` changes the freshly initialized ring-buffer head from 1 to 0 so an empty queue cannot expose an uninitialized event when the profiler connects immediately. A diagnostic live fixture reproduced the upstream race at `utracy_consume_queue`; repeated post-patch live capture is required before compatibility promotion. Artifact hashes cover the patched binary.

The Windows BYOND 516.1687 hosted gate also needs the legacy x86 DirectX helper imported by DreamDaemon. CI downloads `Microsoft.DXSDK.D3DX` version `9.29.952.8` only from the official NuGet endpoint and requires package SHA-256 `ead0906ae8a26c18a7525da7490127a2110f7c58f18293738283e30e97c6ea4b`. It extracts the unmodified x86 release `D3DX9_43.dll` application-local beside the CI BYOND executables. The package's `LICENSE.txt` and `NOTICE.md` are retained with runtime evidence. This installation is an explicit CI provisioning step; Meridian-MCP startup and MCP tool calls never download or install it.

The over-64K boundary follows BYOND's official [516.1686 release note](https://www.byond.com/docs/notes/516.html), which identifies a DreamDaemon startup crash or hang affecting worlds with more than 64K prototypes. The runtime corpus uses 256-child buckets because a flat 65,537-child parent instead triggers DreamMaker's distinct direct-child ceiling. Synthetic runtime arguments use BYOND's documented [startup options](https://www.byond.com/docs/ref/info.html#/proc/startup): port `0`, loopback `-ip 127.0.0.1`, `-close`, `-log`, `-trusted`, and `-verbose`. Linux ELF binaries do not expose Windows `FileVersionInfo`; the Linux lane therefore records the explicit workflow version backed by the verified archive version and SHA-256, while Windows additionally requires matching PE file metadata. `/tg/station`'s Ubuntu 24.04 [integration workflow](https://github.com/tgstation/tgstation/blob/master/.github/workflows/run_integration_tests.yml) and direct [DreamDaemon launcher](https://github.com/tgstation/tgstation/blob/master/tools/ci/run_server.sh) are the platform authority for the required synthetic engine lane. Meridian-MCP records source declaration counts separately from BYOND-internal prototypes and keeps the Windows synthetic result diagnostic until repeated hosted evidence supports promotion.

This engineering inventory is not legal advice. Review dependency licenses under [dependency policy](dependency-policy.md).
