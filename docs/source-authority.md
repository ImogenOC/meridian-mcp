# Source authority

Use the authority that answers the question:

1. Official BYOND documentation and reproducible DreamMaker or DreamDaemon behavior govern language and runtime semantics.
2. A target repository's checked-in `.dme`, dependency pins, build entry point, and contributor guidance govern that project.
3. Current tgstation guidance and implementation govern inherited tg systems unless a downstream records a deliberate delta.
4. Nova documentation governs inherited Nova modularization and merge-preservation behavior.
5. SpacemanDMM is the selected analysis implementation. Its output is evidence, not compiler truth.
6. The original dm-mcp repository establishes lineage only.
7. Meridian-MCP documentation claims only tested behavior or explicitly labeled provisional, experimental, or unsupported behavior.

For Meridian-Rift, `BUILD.cmd` and repository guidance are the full-project acceptance boundary. `dm_compile` is a faster direct DreamMaker gate, not an equivalent full build.
