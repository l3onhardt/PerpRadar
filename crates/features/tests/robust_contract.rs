use perp_radar_features::robust::RingWindow;

#[test]
fn robust_stats_returns_none_until_min_samples() {
    let mut window = RingWindow::new(5);
    window.push(1.0);
    window.push(2.0);

    assert_eq!(window.stats(3.0, 3, 5.0), None);
}

#[test]
fn robust_stats_uses_mad_and_clips_z_score() {
    let mut window = RingWindow::new(5);
    for value in [1.0, 2.0, 3.0, 4.0, 5.0] {
        window.push(value);
    }

    let stats = window.stats(100.0, 5, 5.0).unwrap();

    assert_eq!(stats.n, 5);
    assert_eq!(stats.median, 3.0);
    assert_eq!(stats.z, 5.0);
}

#[test]
fn robust_stats_falls_back_to_stddev_when_mad_is_zero() {
    let mut window = RingWindow::new(5);
    for value in [1.0, 1.0, 1.0, 2.0, 3.0] {
        window.push(value);
    }

    let stats = window.stats(2.0, 5, 5.0).unwrap();

    assert!(stats.scale > 0.0);
    assert!(stats.z.is_finite());
}

#[test]
fn robust_stats_returns_none_when_all_history_values_are_equal() {
    let mut window = RingWindow::new(5);
    for _ in 0..5 {
        window.push(1.0);
    }

    assert_eq!(window.stats(2.0, 5, 5.0), None);
}

#[test]
fn ring_window_keeps_only_finite_recent_values() {
    let mut window = RingWindow::new(3);
    window.push(1.0);
    window.push(f64::NAN);
    window.push(2.0);
    window.push(3.0);
    window.push(4.0);

    assert_eq!(window.values_recent(), vec![2.0, 3.0, 4.0]);
}

#[test]
fn percentile_rank_counts_values_less_than_or_equal_current() {
    let mut window = RingWindow::new(5);
    for value in [10.0, 20.0, 30.0, 40.0] {
        window.push(value);
    }

    assert_eq!(window.percentile_rank(25.0), Some(0.5));
    assert_eq!(window.percentile_rank(40.0), Some(1.0));
}
