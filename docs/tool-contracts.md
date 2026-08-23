# Tool contracts

Generated from `src/contracts.rs`; do not edit by hand.

| Tool | Mode | Support | Effects | Timeout ms | Max output bytes | Summary |
| --- | --- | --- | --- | ---: | ---: | --- |
| `dm_check_errors` | Analysis | Provisional | memory | - | 1048576 | Run SpacemanDMM DreamChecker diagnostics. |
| `dm_compile` | Development | Provisional | read, write, process | 1800000 | 1048576 | Run an allowlisted DreamMaker compiler gate. |
| `dm_find_on_map` | Analysis | Provisional | read | - | 1048576 | Find exact type instances in a DMM/TGM map. |
| `dm_get_definition` | Analysis | Provisional | memory | - | 262144 | Locate an exact parsed definition. |
| `dm_get_proc` | Analysis | Provisional | read | - | 1048576 | Inspect exact proc implementations and source excerpts. |
| `dm_get_type` | Analysis | Provisional | memory | - | 1048576 | Inspect an exact DreamMaker type. |
| `dm_get_var` | Analysis | Provisional | memory | - | 262144 | Inspect an exact DreamMaker variable. |
| `dm_list_types` | Analysis | Provisional | memory | - | 1048576 | List parsed types under an optional prefix. |
| `dm_map_info` | Analysis | Provisional | read | - | 1048576 | Read DMM/TGM dimensions and atom statistics. |
| `dm_parse_environment` | Analysis | Provisional | read | - | 1048576 | Parse and atomically index a DreamMaker environment. |
| `dm_render_map` | Development | Provisional | read, write | - | 262144 | Render a contained DMM/TGM map output. |
| `dm_run` | Development | Provisional | read, write, process, loopback | 300000 | 1048576 | Start a contained DreamDaemon program on loopback. |
| `dm_search_context` | Analysis | Provisional | read | - | 1048576 | Rank parsed symbols and source context deterministically. |
| `dm_search_symbols` | Analysis | Provisional | memory | - | 1048576 | Search parsed symbol names. |
| `dm_status` | Development | Provisional | memory | - | 1048576 | Inspect server-owned DreamDaemon state. |
| `dm_stop` | Development | Provisional | memory | - | 262144 | Stop the server-owned DreamDaemon process. |
| `dm_topic` | Development | Provisional | loopback | 60000 | 262144 | Call world.Topic on the loopback game server. |
| `dm_wait_for_output` | Development | Provisional | memory | 300000 | 1048576 | Wait for bounded server-owned DreamDaemon output. |
