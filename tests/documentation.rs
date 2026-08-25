use std::path::Path;

const REQUIRED_DOCUMENTS: &[&str] = &[
    "docs/provenance.md",
    "docs/source-authority.md",
    "docs/compatibility.md",
    "docs/dependency-policy.md",
    "SECURITY.md",
];

#[test]
fn required_trust_documents_exist_and_are_linked() {
    let readme = std::fs::read_to_string("README.md").expect("README should be readable");
    for document in REQUIRED_DOCUMENTS {
        assert!(Path::new(document).is_file(), "missing {document}");
        assert!(
            readme.contains(&format!("]({document})")),
            "README does not link {document}"
        );
    }
}

#[test]
fn local_markdown_links_resolve() {
    for source in ["README.md", "CONTRIBUTING.md", "SECURITY.md"] {
        let text = std::fs::read_to_string(source).expect("Markdown should be readable");
        for target in markdown_link_targets(&text) {
            if target.contains("://") || target.starts_with('#') || target.starts_with("mailto:") {
                continue;
            }
            let target = target.split('#').next().unwrap();
            let resolved = Path::new(source)
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join(target);
            assert!(resolved.exists(), "broken link in {source}: {target}");
        }
    }
}

#[test]
fn readme_documents_the_two_compilers_and_build_access_controls() {
    let readme = std::fs::read_to_string("README.md").expect("README should be readable");
    for required in [
        "| `dm_compile` |",
        "| `rift_compile` |",
        "MERIDIAN_MCP_RIFT_BUILD",
        "`disabled`",
        "`offline`",
        "`network`",
        "`network_mode=offline`",
        "`network_mode=allow`",
        "](docs/security.md)",
        "](docs/compatibility.md)",
        "](TESTING.md)",
    ] {
        assert!(readme.contains(required), "README is missing {required}");
    }
}

#[test]
fn readme_has_an_individual_description_for_every_public_tool() {
    let readme = std::fs::read_to_string("README.md").expect("README should be readable");
    for contract in meridian_mcp::all_contracts() {
        let table_row = format!("| `{}` |", contract.name);
        assert!(
            readme.contains(&table_row),
            "README has no individual table entry for {}",
            contract.name
        );
    }
}

#[test]
fn compatibility_document_tracks_deferred_named_gates() {
    let compatibility = std::fs::read_to_string("docs/compatibility.md")
        .expect("compatibility documentation should be readable");
    for heading in [
        "| Capability | Owned fixture | Named-platform/real-repository gate | Required semantic evidence | Current blocker | Status |",
        "DreamChecker",
        "Map inspection",
        "PNG rendering",
        "DreamDaemon lifecycle",
        "`Topic()`",
    ] {
        assert!(
            compatibility.contains(heading),
            "compatibility documentation is missing {heading}"
        );
    }
}

#[test]
fn tracy_is_explicitly_opt_in_and_each_tool_is_documented() {
    let readme = std::fs::read_to_string("README.md").expect("README should be readable");
    for required in [
        "MERIDIAN_MCP_TRACY",
        "`disabled` (default) or `byond`",
        "`dm_tracy_prepare`",
        "`dm_tracy_launch`",
        "`dm_tracy_capture`",
        "`dm_tracy_status`",
        "`dm_tracy_stop`",
        "`dm_tracy_hotspots`",
        "`dm_tracy_zone`",
        "`dm_tracy_frame_stats`",
        "`dm_tracy_compare`",
        "protocol 82",
    ] {
        assert!(readme.contains(required), "README is missing {required}");
    }
    let configure = std::fs::read_to_string("scripts/configure-codex-meridian-mcp.ps1")
        .expect("configure script should be readable");
    let install = std::fs::read_to_string("scripts/install-meridian-mcp.ps1")
        .expect("install script should be readable");
    assert!(configure.contains("[switch]$EnableTracy"));
    assert!(configure.contains("[string]$ServerName = 'meridian-mcp'"));
    assert!(!configure.contains("mcp_servers\\.dm-mcp"));
    assert!(install.contains("[switch]$EnableTracy"));
    assert!(install.contains("PSObject.Properties['protocol_version']"));
}

#[test]
fn auxtools_documentation_explains_setup_workflow_and_boundaries() {
    let readme = std::fs::read_to_string("README.md").expect("README should be readable");
    for required in [
        "### Auxtools debugger",
        "#### Debugger workflow",
        "`dm_debug_launch`",
        "`dm_debug_wait_for_event`",
        "`dm_debug_threads`",
        "`dm_debug_stack_trace`",
        "`dm_debug_scopes`",
        "`dm_debug_variables`",
        "`dm_debug_stop`",
        "does not support attaching to an existing process",
    ] {
        assert!(readme.contains(required), "README is missing {required}");
    }
}

fn markdown_link_targets(text: &str) -> Vec<String> {
    text.split("](")
        .skip(1)
        .filter_map(|suffix| suffix.split(')').next())
        .map(str::to_owned)
        .collect()
}
