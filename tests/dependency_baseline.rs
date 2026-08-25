const RUST_VERSION: &str = "1.95";
const RUST_TOOLCHAIN: &str = "1.95.0";
const SPACEMANDMM_REVISION: &str = "351ddc0ffb2439876d4565ce5130bb6b027ee605";

#[test]
fn rust_toolchain_and_manifest_require_the_approved_version() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let toolchain = std::fs::read_to_string(root.join("rust-toolchain.toml")).unwrap();
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();

    assert!(
        toolchain.contains(&format!("channel = \"{RUST_TOOLCHAIN}\"")),
        "rust-toolchain.toml must pin Rust {RUST_TOOLCHAIN}"
    );
    assert!(
        manifest.contains(&format!("rust-version = \"{RUST_VERSION}\"")),
        "Cargo.toml must require Rust {RUST_VERSION}"
    );
}

#[test]
fn every_spacemandmm_dependency_uses_the_approved_revision() {
    let manifest = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"),
    )
    .unwrap();

    for package in ["dreammaker", "dreamchecker", "dmi", "dmm-tools"] {
        let prefix = format!("{package} =");
        let line = manifest
            .lines()
            .find(|line| line.starts_with(&prefix))
            .unwrap_or_else(|| panic!("missing {package} dependency"));
        assert!(
            line.contains(SPACEMANDMM_REVISION),
            "{package} is not pinned to {SPACEMANDMM_REVISION}"
        );
    }
}
