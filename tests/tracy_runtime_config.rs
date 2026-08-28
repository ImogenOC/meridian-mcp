use meridian_mcp::tracy_runtime_config::{
    inspect_runtime_configuration, wake_client_url, RuntimeConfigurationError,
};
use std::path::PathBuf;

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "meridian-mcp-tracy-runtime-config-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        Self { root }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn profiling_configuration_requires_the_resume_flag_for_a_bounded_wake() {
    let fixture = Fixture::new("sleeping");
    std::fs::write(
        fixture.root.join("config.txt"),
        "# RESUME_AFTER_INITIALIZATIONS\n",
    )
    .unwrap();

    let error = inspect_runtime_configuration(&fixture.root).unwrap_err();
    assert!(matches!(
        error,
        RuntimeConfigurationError::ResumeAfterInitializationsNotEnabled
    ));
}

#[test]
fn post_wake_progress_must_continue_after_the_wake_tick() {
    assert!(meridian_mcp::tracy_runtime_config::sustained_scheduler_progress(10, 11, 12));
    assert!(!meridian_mcp::tracy_runtime_config::sustained_scheduler_progress(10, 11, 11));
    assert!(!meridian_mcp::tracy_runtime_config::sustained_scheduler_progress(10, 10, 11));
}

#[test]
fn wake_client_url_is_fixed_to_the_owned_loopback_server() {
    assert_eq!(wake_client_url(1337), "byond://127.0.0.1:1337##guest");
}

#[test]
fn profiling_configuration_is_hash_bound_and_encoded_for_world_params() {
    let fixture = Fixture::new("awake profile");
    std::fs::write(fixture.root.join("config.txt"), "# base configuration\n").unwrap();
    std::fs::write(
        fixture.root.join("dev_overrides.txt"),
        "RESUME_AFTER_INITIALIZATIONS\n",
    )
    .unwrap();

    let inspected = inspect_runtime_configuration(&fixture.root).unwrap();
    assert!(inspected.identity.resume_after_initializations);
    assert_eq!(inspected.identity.file_count, 2);
    assert_eq!(inspected.identity.directory_sha256.len(), 64);
    assert!(inspected.world_parameter().starts_with("config-directory="));
    assert!(!inspected.world_parameter().contains('&'));
    assert!(!inspected.world_parameter().contains(' '));

    std::fs::write(
        fixture.root.join("dev_overrides.txt"),
        "RESUME_AFTER_INITIALIZATIONS\nPROFILER_INTERVAL 3000\n",
    )
    .unwrap();
    let changed = inspect_runtime_configuration(&fixture.root).unwrap();
    assert_ne!(
        inspected.identity.directory_sha256,
        changed.identity.directory_sha256
    );
}
