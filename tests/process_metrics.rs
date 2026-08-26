use meridian_mcp::process_metrics::{
    process_identity, sample_process, summarize_memory, ProcessRole, RoleMemorySeries,
};

#[test]
fn current_process_samples_are_bound_to_stable_role_identity() {
    let identity = process_identity(std::process::id(), ProcessRole::Collector).unwrap();
    let samples = sample_process(&identity, 25).unwrap();
    assert_eq!(identity.pid, std::process::id());
    assert!(identity.started_at_identity > 0);
    assert!(!samples.is_empty());
    assert!(samples
        .iter()
        .all(|sample| sample.monotonic_offset_ms == 25));
    assert!(samples.iter().all(|sample| sample.observed_value > 0));
}

#[test]
fn dreamdaemon_and_collector_roles_are_never_implicitly_combined() {
    let pid = std::process::id();
    let dreamdaemon = process_identity(pid, ProcessRole::DreamDaemon).unwrap();
    let collector = process_identity(pid, ProcessRole::Collector).unwrap();
    assert_ne!(dreamdaemon.role, collector.role);
    assert_eq!(
        dreamdaemon.started_at_identity,
        collector.started_at_identity
    );
}

#[test]
fn memory_summaries_remain_separate_by_role_and_metric() {
    let identity = process_identity(std::process::id(), ProcessRole::Collector).unwrap();
    let samples = sample_process(&identity, 0).unwrap();
    let summaries = summarize_memory(&[RoleMemorySeries {
        identity,
        operating_system: std::env::consts::OS.into(),
        sampling_interval_ms: 500,
        samples,
        missed_samples: 0,
    }]);
    assert_eq!(summaries.len(), 1);
    assert_eq!(summaries[0].identity.role, ProcessRole::Collector);
    assert!(summaries[0]
        .metrics
        .values()
        .all(|metric| metric.sample_count == 1 && metric.maximum_bytes > 0));
}
