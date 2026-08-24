use std::fs;

#[test]
fn byond_workflow_runs_the_versioned_meridian_compatibility_gate() {
    let workflow = fs::read_to_string(".github/workflows/byond-integration.yml")
        .expect("BYOND integration workflow should be readable");

    for required in [
        "workflow_dispatch:",
        "meridian_ref:",
        "schedule:",
        "runs-on: windows-latest",
        "actions/checkout@v7",
        "AphelionDevelopment/Meridian-Rift",
        "path: integration/Meridian-Rift",
        "scripts/install-byond.ps1",
        "cargo build --release",
        "scripts/run-byond-integration.ps1",
        "if: always()",
        "actions/upload-artifact@v6",
    ] {
        assert!(
            workflow.contains(required),
            "workflow is missing {required}"
        );
    }

    let integration_script = fs::read_to_string("scripts/run-byond-integration.ps1")
        .expect("BYOND integration script should be readable");
    assert!(integration_script.contains("run-meridian-compatibility.ps1"));

    let installer = fs::read_to_string("scripts/install-byond.ps1")
        .expect("BYOND installer should be readable");
    for required in [
        "Invoke-WebRequest",
        "Expand-Archive",
        "MaxAttempts",
        "ExpectedSha256",
        "Get-FileHash",
        "byond-builds.dm-lang.org",
        "tgstation/1.0 CI Script",
        "dm.exe",
    ] {
        assert!(
            installer.contains(required),
            "BYOND installer is missing {required}"
        );
    }
}

#[test]
fn repository_pins_the_ci_rust_toolchain() {
    let toolchain = fs::read_to_string("rust-toolchain.toml")
        .expect("repository Rust toolchain should be pinned");
    assert!(toolchain.contains("channel = \"1.88.0\""));
    assert!(toolchain.contains("components = [\"rustfmt\", \"clippy\"]"));
}

#[test]
fn portable_ci_tests_the_unsupported_platform_boundary_without_byond() {
    let workflow = fs::read_to_string(".github/workflows/ci.yml")
        .expect("portable CI workflow should be readable");

    assert!(workflow.contains("MERIDIAN_MCP_RIFT_BUILD: network"));
    assert!(workflow.contains("test_unsupported_rift_compile.ps1"));
    for forbidden in ["Install BYOND", "byond.com/download", "RIFT_BUILD.cmd"] {
        assert!(
            !workflow.contains(forbidden),
            "portable CI must not attempt BYOND or the Windows build wrapper: {forbidden}"
        );
    }
}
