use super::*;
use std::time::Duration;

fn duration_ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

fn assert_close(actual: f64, expected: f64) {
    let tolerance = f64::EPSILON * expected.abs().max(1.0) * 8.0;
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn cycle_stats_default_is_an_empty_finite_identity() {
    let stats = CycleStats::default();

    assert_eq!(stats.samples, 0);
    for value in [stats.min_ms, stats.max_ms, stats.avg_ms, stats.last_ms] {
        assert_eq!(value, 0.0);
        assert!(value.is_finite());
    }
}

#[test]
fn cycle_stats_first_sample_sets_every_projection() {
    let mut stats = CycleStats::default();

    stats.record(duration_ms(7));

    assert_eq!(stats.samples, 1);
    assert_eq!(stats.min_ms, 7.0);
    assert_eq!(stats.max_ms, 7.0);
    assert_eq!(stats.avg_ms, 7.0);
    assert_eq!(stats.last_ms, 7.0);
}

#[test]
fn cycle_stats_tracks_min_max_mean_and_last_across_zero_sample() {
    let mut stats = CycleStats::default();
    for value in [10, 0, 30, 20] {
        stats.record(duration_ms(value));
    }

    assert_eq!(stats.samples, 4);
    assert_eq!(stats.min_ms, 0.0);
    assert_eq!(stats.max_ms, 30.0);
    assert_close(stats.avg_ms, 15.0);
    assert_eq!(stats.last_ms, 20.0);
}

#[test]
fn task_stats_duration_and_overrun_accounting_are_independent() {
    let mut stats = TaskStats::default();
    stats.record_overrun(3);
    stats.record(duration_ms(4));
    stats.record(duration_ms(8));

    assert_eq!(stats.samples, 2);
    assert_eq!(stats.overruns, 3);
    assert_eq!(stats.min_ms, 4.0);
    assert_eq!(stats.max_ms, 8.0);
    assert_close(stats.avg_ms, 6.0);
    assert_eq!(stats.last_ms, 8.0);
}

#[test]
fn call_stats_tracks_total_mean_count_and_last() {
    let mut stats = CallStats::default();
    for value in [2, 4, 9] {
        stats.record(duration_ms(value));
    }

    assert_eq!(stats.calls, 3);
    assert_eq!(stats.total_ms, 15.0);
    assert_eq!(stats.min_ms, 2.0);
    assert_eq!(stats.max_ms, 9.0);
    assert_close(stats.avg_ms, 5.0);
    assert_eq!(stats.last_ms, 9.0);
}

#[test]
fn counters_saturate_instead_of_wrapping() {
    let mut metrics = RuntimeMetrics::new();
    metrics.faults = u64::MAX;
    metrics.overruns = u64::MAX;
    let task = SmolStr::new("Task");
    metrics.tasks.insert(
        task.clone(),
        TaskStats {
            overruns: u64::MAX,
            ..TaskStats::default()
        },
    );

    metrics.record_fault();
    metrics.record_overrun(&task, 1);

    assert_eq!(metrics.faults, u64::MAX);
    assert_eq!(metrics.overruns, u64::MAX);
    assert_eq!(metrics.tasks[&task].overruns, u64::MAX);
}

#[test]
fn zero_missed_activations_are_a_noop_without_task_materialization() {
    let mut metrics = RuntimeMetrics::new();
    let task = SmolStr::new("NeverMissed");

    metrics.record_overrun(&task, 0);

    assert_eq!(metrics.overruns, 0);
    assert!(!metrics.tasks.contains_key(&task));
}

#[test]
fn empty_cycle_window_has_zero_finite_percentiles() {
    let snapshot = CycleLatencyWindow::default().snapshot();

    assert_eq!(snapshot.window_samples, 0);
    for value in [
        snapshot.p50_ms,
        snapshot.p95_ms,
        snapshot.p99_ms,
        snapshot.max_ms,
    ] {
        assert_eq!(value, 0.0);
        assert!(value.is_finite());
    }
}

#[test]
fn percentile_uses_documented_rounded_index_for_small_populations() {
    let one = [7.0];
    assert_eq!(percentile(&one, 0.50), 7.0);
    assert_eq!(percentile(&one, 0.99), 7.0);

    let two = [1.0, 9.0];
    assert_eq!(percentile(&two, 0.50), 9.0);
    assert_eq!(percentile(&two, 0.95), 9.0);

    let four = [1.0, 2.0, 3.0, 4.0];
    assert_eq!(percentile(&four, 0.50), 3.0);
    assert_eq!(percentile(&four, 0.95), 4.0);
    assert_eq!(percentile(&four, 0.99), 4.0);
}

#[test]
fn percentile_clamps_quantile_bounds() {
    let values = [2.0, 4.0, 8.0];

    assert_eq!(percentile(&values, -10.0), 2.0);
    assert_eq!(percentile(&values, 10.0), 8.0);
}

#[test]
fn cycle_window_retains_exactly_the_most_recent_512_samples() {
    let mut window = CycleLatencyWindow::default();
    window.record_ms(10_000.0);
    for _ in 0..CYCLE_LATENCY_WINDOW_SIZE {
        window.record_ms(1.0);
    }

    let snapshot = window.snapshot();

    assert_eq!(snapshot.window_samples, CYCLE_LATENCY_WINDOW_SIZE as u64);
    assert_eq!(snapshot.p50_ms, 1.0);
    assert_eq!(snapshot.p95_ms, 1.0);
    assert_eq!(snapshot.p99_ms, 1.0);
    assert_eq!(snapshot.max_ms, 1.0);
}

#[test]
fn cycle_window_snapshot_does_not_change_future_rollover() {
    let mut observed = CycleLatencyWindow::default();
    let mut control = CycleLatencyWindow::default();
    for value in 1..=CYCLE_LATENCY_WINDOW_SIZE as u64 {
        observed.record_ms(value as f64);
        control.record_ms(value as f64);
    }

    let _ = observed.snapshot();
    observed.record_ms(999.0);
    control.record_ms(999.0);

    let observed_snapshot = observed.snapshot();
    let control_snapshot = control.snapshot();
    assert_eq!(
        observed_snapshot.window_samples,
        control_snapshot.window_samples
    );
    assert_eq!(observed_snapshot.p50_ms, control_snapshot.p50_ms);
    assert_eq!(observed_snapshot.p95_ms, control_snapshot.p95_ms);
    assert_eq!(observed_snapshot.p99_ms, control_snapshot.p99_ms);
    assert_eq!(observed_snapshot.max_ms, control_snapshot.max_ms);
}

#[test]
fn nonempty_percentile_projection_is_ordered_finite_and_exactly_counted() {
    let mut metrics = RuntimeMetrics::new();
    for value in [100, 1, 50, 5, 25, 75, 10] {
        metrics.record_cycle(duration_ms(value));
    }

    let snapshot = metrics.snapshot().cycle_percentiles;

    assert_eq!(snapshot.window_samples, 7);
    assert!(snapshot.p50_ms <= snapshot.p95_ms);
    assert!(snapshot.p95_ms <= snapshot.p99_ms);
    assert!(snapshot.p99_ms <= snapshot.max_ms);
    for value in [
        snapshot.p50_ms,
        snapshot.p95_ms,
        snapshot.p99_ms,
        snapshot.max_ms,
    ] {
        assert!(value.is_finite());
    }
}

#[test]
fn runtime_metrics_default_reports_vm_and_empty_state() {
    let metrics = RuntimeMetrics::default();
    let snapshot = metrics.snapshot();

    assert_eq!(snapshot.execution_backend, ExecutionBackend::BytecodeVm);
    assert_eq!(snapshot.faults, 0);
    assert_eq!(snapshot.overruns, 0);
    assert!(snapshot.tasks.is_empty());
    assert!(snapshot.profiling.enabled);
    assert!(snapshot.profiling.calls.is_empty());
    assert!(snapshot.profiling.top_contributors.is_empty());
}

#[test]
fn task_rows_are_sorted_by_exact_name_independent_of_insertion_order() {
    let mut metrics = RuntimeMetrics::new();
    for name in [
        "Zulu", "Echo", "Alpha", "Mike", "Bravo", "Hotel", "Charlie", "Foxtrot",
    ] {
        metrics.record_task(&SmolStr::new(name), duration_ms(1));
    }

    let names = metrics
        .snapshot()
        .tasks
        .into_iter()
        .map(|task| task.name.to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["Alpha", "Bravo", "Charlie", "Echo", "Foxtrot", "Hotel", "Mike", "Zulu"]
    );
}

#[test]
fn task_duration_and_positive_overrun_touch_only_named_task() {
    let mut metrics = RuntimeMetrics::new();
    let fast = SmolStr::new("Fast");
    let slow = SmolStr::new("Slow");
    metrics.record_task(&fast, duration_ms(2));
    metrics.record_task(&slow, duration_ms(8));
    metrics.record_overrun(&slow, 2);

    let snapshot = metrics.snapshot();
    let fast = snapshot
        .tasks
        .iter()
        .find(|task| task.name == "Fast")
        .unwrap();
    let slow = snapshot
        .tasks
        .iter()
        .find(|task| task.name == "Slow")
        .unwrap();

    assert_eq!(fast.overruns, 0);
    assert_eq!(fast.avg_ms, 2.0);
    assert_eq!(slow.overruns, 2);
    assert_eq!(slow.avg_ms, 8.0);
    assert_eq!(snapshot.overruns, 2);
}

#[test]
fn repeated_call_identity_aggregates_without_merging_other_kind() {
    let mut metrics = RuntimeMetrics::new();
    let name = SmolStr::new("MAIN");
    metrics.record_cycle(duration_ms(10));
    metrics.record_call("program", &name, duration_ms(2));
    metrics.record_call("program", &name, duration_ms(4));
    metrics.record_call("fb", &name, duration_ms(1));

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.profiling.calls.len(), 2);
    let program = snapshot
        .profiling
        .calls
        .iter()
        .find(|call| call.key == "program:MAIN")
        .unwrap();
    let fb = snapshot
        .profiling
        .calls
        .iter()
        .find(|call| call.key == "fb:MAIN")
        .unwrap();

    assert_eq!(program.calls, 2);
    assert_eq!(program.min_ms, 2.0);
    assert_eq!(program.max_ms, 4.0);
    assert_close(program.avg_ms, 3.0);
    assert_eq!(program.last_ms, 4.0);
    assert_close(program.avg_cycle_ms, 6.0);
    assert_eq!(fb.calls, 1);
    assert_close(fb.avg_cycle_ms, 1.0);
}

#[test]
fn calls_without_cycle_samples_remain_visible_with_zero_percentages() {
    let mut metrics = RuntimeMetrics::new();
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(3));

    let snapshot = metrics.snapshot();
    let call = &snapshot.profiling.calls[0];
    let top = &snapshot.profiling.top_contributors[0];

    assert_eq!(call.avg_cycle_ms, 3.0);
    assert_eq!(top.avg_cycle_ms, 3.0);
    assert_eq!(top.cycle_pct, 0.0);
    assert_eq!(top.last_cycle_pct, 0.0);
    assert!(top.cycle_pct.is_finite());
    assert!(top.last_cycle_pct.is_finite());
}

#[test]
fn contributor_percentages_use_average_and_last_cycle_denominators() {
    let mut metrics = RuntimeMetrics::new();
    metrics.record_cycle(duration_ms(10));
    metrics.record_cycle(duration_ms(20));
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(3));
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(5));

    let top = metrics.snapshot().profiling.top_contributors.remove(0);

    assert_close(top.avg_cycle_ms, 4.0);
    assert_close(top.cycle_pct, 4.0 / 15.0 * 100.0);
    assert_close(top.last_ms, 5.0);
    assert_close(top.last_cycle_pct, 25.0);
}

#[test]
fn contributor_percentages_are_not_clamped_to_one_hundred() {
    let mut metrics = RuntimeMetrics::new();
    metrics.record_cycle(duration_ms(1));
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(3));

    let top = metrics.snapshot().profiling.top_contributors.remove(0);

    assert_eq!(top.cycle_pct, 300.0);
    assert_eq!(top.last_cycle_pct, 300.0);
}

#[test]
fn top_contributors_are_limited_to_five_and_ties_use_key_order() {
    let mut metrics = RuntimeMetrics::new();
    metrics.record_cycle(duration_ms(10));
    for name in ["Zulu", "Echo", "Alpha", "Mike", "Bravo", "Charlie", "Hotel"] {
        metrics.record_call("program", &SmolStr::new(name), duration_ms(1));
    }

    let keys = metrics
        .snapshot()
        .profiling
        .top_contributors
        .into_iter()
        .map(|entry| entry.key.to_string())
        .collect::<Vec<_>>();

    assert_eq!(keys.len(), 5);
    assert_eq!(
        keys,
        vec![
            "program:Alpha",
            "program:Bravo",
            "program:Charlie",
            "program:Echo",
            "program:Hotel"
        ]
    );
}

#[test]
fn disabling_profiling_clears_calls_and_ignores_disabled_records() {
    let mut metrics = RuntimeMetrics::new();
    metrics.record_cycle(duration_ms(10));
    metrics.record_call("program", &SmolStr::new("Before"), duration_ms(2));

    metrics.set_profiling_enabled(false);
    metrics.record_call("program", &SmolStr::new("Ignored"), duration_ms(9));
    let disabled = metrics.snapshot();
    assert!(!disabled.profiling.enabled);
    assert!(disabled.profiling.calls.is_empty());
    assert!(disabled.profiling.top_contributors.is_empty());

    metrics.set_profiling_enabled(true);
    let clean = metrics.snapshot();
    assert!(clean.profiling.enabled);
    assert!(clean.profiling.calls.is_empty());
}

#[test]
fn idempotent_enabled_toggle_preserves_existing_profile_samples() {
    let mut metrics = RuntimeMetrics::new();
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(2));

    metrics.set_profiling_enabled(true);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.profiling.calls.len(), 1);
    assert_eq!(snapshot.profiling.calls[0].calls, 1);
}

#[test]
fn profiling_toggle_does_not_reset_cycle_task_fault_or_overrun_metrics() {
    let mut metrics = RuntimeMetrics::new();
    let task = SmolStr::new("Task");
    metrics.record_cycle(duration_ms(5));
    metrics.record_task(&task, duration_ms(2));
    metrics.record_fault();
    metrics.record_overrun(&task, 3);

    metrics.set_profiling_enabled(false);
    metrics.set_profiling_enabled(true);
    let snapshot = metrics.snapshot();

    assert_eq!(snapshot.cycle.last_ms, 5.0);
    assert_eq!(snapshot.cycle_percentiles.window_samples, 1);
    assert_eq!(snapshot.tasks.len(), 1);
    assert_eq!(snapshot.tasks[0].overruns, 3);
    assert_eq!(snapshot.faults, 1);
    assert_eq!(snapshot.overruns, 3);
}

#[test]
fn repeated_snapshots_are_observational_except_for_monotonic_uptime() {
    let mut metrics = RuntimeMetrics::new();
    metrics.record_cycle(duration_ms(5));
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(2));

    let first = metrics.snapshot();
    let second = metrics.snapshot();

    assert!(second.uptime_ms >= first.uptime_ms);
    assert_eq!(second.cycle.samples, first.cycle.samples);
    assert_eq!(
        second.cycle_percentiles.window_samples,
        first.cycle_percentiles.window_samples
    );
    assert_eq!(
        second.profiling.calls[0].calls,
        first.profiling.calls[0].calls
    );
    assert_eq!(
        second.profiling.calls[0].avg_cycle_ms,
        first.profiling.calls[0].avg_cycle_ms
    );
}
