# Changelog

All notable changes to meridian-mcp will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added deterministic `dm_search_context` ranking over parsed DreamMaker symbols, documentation, source excerpts, and file paths.
- Added provenance, source-authority, compatibility, dependency, and security records.

### Changed

- Defined analysis and development capability modes and evidence-based support labels.
- Clarified that `dm_compile` is a direct DreamMaker gate rather than a repository full build.

### Removed

- Removed the unsupported inherited BYOND client-login protocol and `dm_connect_test` from the target supported product; `world.Topic()` remains separate.

### Fixed

- Read DreamDaemon `-logself` files so readiness markers and runtime output are available on Windows.
- Correct BYOND Topic request framing and response decoding for string and float responses.
- Parse DMM/TGM maps with SpacemanDMM for exact dimensions, instance counts, and coordinates.
- Render actual map PNGs through the SpacemanDMM minimap pipeline.
- Return type children and documentation, proc documentation and accurate body metadata, and variable documentation and declared types.
- Return experimental full-client handshake failures as structured tool errors instead of JSON-RPC internal errors.

## [0.1.0] - 2026-01-28

### Added

- Initial release as `meridian-mcp`
- **18 MCP Tools** for DreamMaker development:
  - `dm_parse_environment` - Parse .dme files and cache object tree
  - `dm_get_type` - Get type definition with vars and procs
  - `dm_get_proc` - Get proc source code and location
  - `dm_get_var` - Get variable definition and value
  - `dm_list_types` - List types matching a pattern
  - `dm_search_symbols` - Search for symbols by name
  - `dm_check_errors` - Run dreamchecker type analysis
  - `dm_get_definition` - Go to definition for any symbol
  - `dm_compile` - Compile project with DreamMaker
  - `dm_render_map` - Render .dmm map to PNG
  - `dm_map_info` - Get map dimensions and statistics
  - `dm_find_on_map` - Find object instances on maps
  - `dm_run` - Start DreamDaemon server
  - `dm_wait_for_output` - Wait for literal or regex markers in DreamDaemon output
  - `dm_stop` - Stop DreamDaemon server
  - `dm_status` - Check DreamDaemon status
  - `dm_topic` - Send Topic() calls to running server
  - `dm_connect_test` - Test BYOND client connection

- **BYOND Client Protocol** implementation
  - Full packet framing and types
  - RUNSUB encryption support
  - Connection testing capabilities

- Cross-platform support (Windows primary, Linux compatible)
- Integration with SpacemanDMM for AST parsing and type checking
- `dm_*` MCP tool names retained for compatibility with existing clients and workflows

### Technical Notes

- Built with Rust + Tokio for async I/O
- Uses tree-sitter style parsing via SpacemanDMM
- Stateful server design (caches parsed environments)

[0.1.0]: Release notes are documented in this file.
