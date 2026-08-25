# Tool contracts

Generated from `src/contracts.rs`; do not edit by hand.

| Tool | Mode | Support | Effects | Timeout ms | Max output bytes | Summary |
| --- | --- | --- | --- | ---: | ---: | --- |
| `dm_audit_icons` | Analysis | Experimental | read | - | 1048576 | Audit parsed icon evidence and duplicate DMI states. |
| `dm_check_errors` | Analysis | Provisional | memory | - | 1048576 | Run SpacemanDMM DreamChecker diagnostics. |
| `dm_compare_dmi_states` | Analysis | Provisional | read | - | 1048576 | Compare complete DMI states including common lazy changes. |
| `dm_compile` | Development | Provisional | read, write, process | 1800000 | 1048576 | Run an allowlisted DreamMaker compiler gate. |
| `dm_debug_control` | Development | Experimental | read, process, loopback | 30000 | 262144 | Pause, continue, or step the active debuggee. |
| `dm_debug_evaluate` | Development | Experimental | read, process, loopback | 30000 | 1048576 | Evaluate an expression in the active debuggee. |
| `dm_debug_exception_info` | Development | Experimental | read, process, loopback | - | 262144 | Read the last retained runtime exception. |
| `dm_debug_launch` | Development | Experimental | read, process, loopback | 60000 | 262144 | Launch one owned DreamSeeker auxtools session. |
| `dm_debug_scopes` | Development | Experimental | read, process, loopback | 30000 | 1048576 | Read variable scopes for a debug frame. |
| `dm_debug_set_breakpoints` | Development | Experimental | read, process, loopback | 30000 | 1048576 | Replace source-oriented auxtools breakpoints. |
| `dm_debug_set_exception_breakpoints` | Development | Experimental | read, process, loopback | 30000 | 262144 | Toggle breaks on DreamMaker runtimes. |
| `dm_debug_set_function_breakpoints` | Development | Experimental | read, process, loopback | 30000 | 1048576 | Set canonical proc breakpoints. |
| `dm_debug_source` | Development | Experimental | read, process, loopback | - | 1048576 | Read the retained auxtools standard-definition source. |
| `dm_debug_stack_trace` | Development | Experimental | read, process, loopback | 30000 | 1048576 | Read bounded debuggee stack frames. |
| `dm_debug_stop` | Development | Experimental | read, process, loopback | 30000 | 262144 | Disconnect and terminate the owned debugger session. |
| `dm_debug_threads` | Development | Experimental | read, process, loopback | 30000 | 1048576 | List auxtools debuggee stacks. |
| `dm_debug_variables` | Development | Experimental | read, process, loopback | 30000 | 1048576 | Read a bounded variable-reference page. |
| `dm_debug_wait_for_event` | Development | Experimental | read, process, loopback | 300000 | 1048576 | Wait for a bounded debugger event. |
| `dm_diff_maps` | Analysis | Provisional | read | - | 1048576 | Compare coordinate models across two DMM/TGM maps. |
| `dm_dmi_info` | Analysis | Provisional | read | - | 1048576 | Profile DMI metadata and frame pixels without altering art. |
| `dm_document_symbols` | Analysis | Provisional | read | - | 1048576 | List declarations in one parsed source file. |
| `dm_extract_dmi` | Development | Experimental | read, write | - | 262144 | Mechanically extract a selected DMI state without altering source art. |
| `dm_find_dmi_duplicates` | Analysis | Experimental | read | - | 1048576 | Find cross-file exact and lazy-change DMI duplicates. |
| `dm_find_implementations` | Analysis | Provisional | memory | - | 1048576 | Find type or member implementations. |
| `dm_find_on_map` | Analysis | Provisional | read | - | 1048576 | Find exact type instances in a DMM/TGM map. |
| `dm_find_references` | Analysis | Experimental | read | - | 1048576 | Find bounded exact member references. |
| `dm_generate_docs` | Development | Experimental | read, write, process | 300000 | 262144 | Generate contained HTML through the verified exact dmdoc helper. |
| `dm_get_definition` | Analysis | Provisional | memory | - | 262144 | Locate an exact parsed definition. |
| `dm_get_proc` | Analysis | Provisional | read | - | 1048576 | Inspect exact proc implementations and source excerpts. |
| `dm_get_type` | Analysis | Provisional | memory | - | 1048576 | Inspect an exact DreamMaker type. |
| `dm_get_var` | Analysis | Provisional | memory | - | 262144 | Inspect an exact DreamMaker variable. |
| `dm_list_render_passes` | Analysis | Provisional | memory | - | 262144 | List pinned SpacemanDMM render-pass behavior. |
| `dm_list_types` | Analysis | Provisional | memory | - | 1048576 | List parsed types under an optional prefix. |
| `dm_map_info` | Analysis | Provisional | read | - | 1048576 | Read DMM/TGM dimensions and atom statistics. |
| `dm_parse_environment` | Analysis | Provisional | read | - | 1048576 | Parse and atomically index a DreamMaker environment. |
| `dm_render_map` | Development | Provisional | read, write | - | 262144 | Render a contained DMM/TGM map output. |
| `dm_render_maps` | Development | Experimental | read, write | - | 1048576 | Render a bounded typed batch of map chunks. |
| `dm_run` | Development | Provisional | read, write, process, loopback | 300000 | 1048576 | Start a contained DreamDaemon program on loopback. |
| `dm_search_context` | Analysis | Provisional | read | - | 1048576 | Rank parsed symbols and source context deterministically. |
| `dm_search_symbols` | Analysis | Provisional | memory | - | 1048576 | Search parsed symbol names. |
| `dm_status` | Development | Provisional | memory | - | 1048576 | Inspect server-owned DreamDaemon state. |
| `dm_stop` | Development | Provisional | memory | - | 262144 | Stop the server-owned DreamDaemon process. |
| `dm_topic` | Development | Provisional | loopback | 60000 | 262144 | Call world.Topic on the loopback game server. |
| `dm_wait_for_output` | Development | Provisional | memory | 300000 | 1048576 | Wait for bounded server-owned DreamDaemon output. |
| `rift_compile` | Development | Provisional | read, write, process, network | 1800000 | 1048576 | Run Meridian-Rift's contained RIFT_BUILD.cmd full-build gate. |
