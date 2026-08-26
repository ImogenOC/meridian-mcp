use meridian_mcp::tracy_statistics::summarize_controls;

#[test]
fn repeated_control_statistics_use_fixed_sample_noise_rules() {
    let (summary, noise) =
        summarize_controls(&[10_000_000, 10_000_000, 10_000_000, 10_000_000, 10_000_000]).unwrap();
    assert_eq!(summary.sample_count, 5);
    assert_eq!(summary.mean, 10_000_000.0);
    assert_eq!(summary.sample_stddev, 0.0);
    assert!(!noise.noisy);
    let (_, noisy) = summarize_controls(&[1_000_000, 1_000_000, 5_000_000]).unwrap();
    assert!(noisy.noisy);
    assert_eq!(noisy.cv_limit, 0.10);
    assert_eq!(noisy.range_ratio_limit, 0.20);
    assert_eq!(noisy.absolute_range_floor_ns, 1_000_000);
}

#[test]
fn fewer_than_three_controls_cannot_establish_a_baseline() {
    assert!(summarize_controls(&[1, 2]).is_none());
}
