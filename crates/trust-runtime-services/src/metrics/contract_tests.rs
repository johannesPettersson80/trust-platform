//! Metrics contracts, moved from trust-runtime with the metrics (`host/metrics.rs` and
//! `host/metrics_contract_tests.rs`); the uptime clock is a port now.

#![allow(
    clippy::float_cmp,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation
)]

use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use trust_runtime_core::ports::MonotonicInstant;

use super::*;

/// A clock that never advances.
#[derive(Debug, Clone, Copy, Default)]
struct StoppedClock;

impl MonotonicClock for StoppedClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(Duration::ZERO)
    }
}

/// A clock that has measured a fixed time.
#[derive(Debug, Clone, Copy)]
struct FixedClock(Duration);

impl MonotonicClock for FixedClock {
    fn now(&self) -> MonotonicInstant {
        MonotonicInstant::from_origin(self.0)
    }
}

fn metrics() -> RuntimeMetrics<StoppedClock> {
    RuntimeMetrics::new()
}

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
fn uptime_is_the_clock_time_in_whole_milliseconds() {
    let fixed = RuntimeMetrics::with_clock(FixedClock(Duration::from_micros(2_999_999)));
    assert_eq!(fixed.uptime_ms(), 2_999);
    assert_eq!(fixed.snapshot().uptime_ms, 2_999);
    assert_eq!(metrics().uptime_ms(), 0);
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
    let mut durations = Durations::default();

    durations.record(duration_ms(7));
    let stats = CycleStats::of(&durations);

    assert_eq!(stats.samples, 1);
    assert_eq!(stats.min_ms, 7.0);
    assert_eq!(stats.max_ms, 7.0);
    assert_eq!(stats.avg_ms, 7.0);
    assert_eq!(stats.last_ms, 7.0);
}

#[test]
fn cycle_stats_tracks_min_max_mean_and_last_across_zero_sample() {
    let mut durations = Durations::default();
    for value in [10, 0, 30, 20] {
        durations.record(duration_ms(value));
    }
    let stats = CycleStats::of(&durations);

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

    assert_eq!(stats.durations.count, 2);
    assert_eq!(stats.overruns, 3);
    assert_eq!(stats.min_ms(), 4.0);
    assert_eq!(stats.max_ms(), 8.0);
    assert_close(stats.avg_ms(), 6.0);
    assert_eq!(stats.last_ms(), 8.0);
}

#[test]
fn call_stats_tracks_total_mean_count_and_last() {
    let mut stats = CallStats::default();
    for value in [2, 4, 9] {
        stats.record(duration_ms(value));
    }

    assert_eq!(stats.calls(), 3);
    assert_eq!(stats.total_ms(), 15.0);
    assert_eq!(stats.min_ms(), 2.0);
    assert_eq!(stats.max_ms(), 9.0);
    assert_close(stats.avg_ms(), 5.0);
    assert_eq!(stats.last_ms(), 9.0);
}

#[test]
fn counters_saturate_instead_of_wrapping() {
    let mut metrics = metrics();
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
    let mut metrics = metrics();
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
    // Specification 11: with the n values in ascending order, a percentile is the value at index
    // round((n - 1) * q), here found by selection in values that are not in order. One value is
    // every percentile; of two, 0.5 rounds away from zero to index 1; of four, p50 is index
    // round(1.5) = 2, p95 round(2.85) = 3 and p99 round(2.97) = 3.
    assert_eq!(
        order_statistics(&mut [7], [rank(1, 0.50), rank(1, 0.99)]),
        [7, 7]
    );
    assert_eq!(
        order_statistics(&mut [9, 1], [rank(2, 0.50), rank(2, 0.95)]),
        [9, 9]
    );
    let ranks = [0.50, 0.95, 0.99].map(|quantile| rank(4, quantile));
    assert_eq!(ranks, [2, 3, 3]);
    assert_eq!(order_statistics(&mut [4, 1, 3, 2], ranks), [3, 4, 4]);
}

#[test]
fn percentile_clamps_quantile_bounds() {
    assert_eq!(rank(3, -10.0), 0);
    assert_eq!(rank(3, 10.0), 2);
    assert_eq!(
        order_statistics(&mut [8, 2, 4], [rank(3, -10.0), rank(3, 10.0)]),
        [2, 8]
    );
}

#[test]
fn percentiles_by_selection_are_the_values_of_the_sorted_window() {
    // Specification 11: 600 cycles leave the newest 512 in the window. The first 88 take 10 s each,
    // and the 512 after them are 1 to 512 ms in a scrambled order (k * 7 mod 512, 7 being prime to
    // 512). Sorted, the window is 1 to 512 ms: p50 is index round(511 * 0.5) = 256, 257 ms; p95 index
    // round(485.45) = 485, 486 ms; p99 index round(505.89) = 506, 507 ms; the maximum 512 ms.
    // Catches a selection that answers a value of the wrong rank, one that reads the evicted 10 s
    // cycles, and a snapshot that disturbs the window.
    let mut window = CycleLatencyWindow::default();
    for _ in 0..88 {
        window.record(duration_ms(10_000));
    }
    for k in 0..512_u64 {
        window.record(duration_ms(k * 7 % 512 + 1));
    }
    for _ in 0..2 {
        let snapshot = window.snapshot();
        assert_eq!(snapshot.window_samples, 512);
        assert_eq!(snapshot.p50_ms, 257.0);
        assert_eq!(snapshot.p95_ms, 486.0);
        assert_eq!(snapshot.p99_ms, 507.0);
        assert_eq!(snapshot.max_ms, 512.0);
    }
}

#[test]
fn cycle_window_retains_exactly_the_most_recent_512_samples() {
    let mut window = CycleLatencyWindow::default();
    window.record(duration_ms(10_000));
    for _ in 0..CYCLE_LATENCY_WINDOW_SIZE {
        window.record(duration_ms(1));
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
        observed.record(duration_ms(value));
        control.record(duration_ms(value));
    }

    let _ = observed.snapshot();
    observed.record(duration_ms(999));
    control.record(duration_ms(999));

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
    let mut metrics = metrics();
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
fn cycle_percentiles_track_recent_window() {
    let mut metrics = metrics();
    for duration in [5_u64, 10, 15, 20, 25] {
        metrics.record_cycle(duration_ms(duration));
    }

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.cycle_percentiles.window_samples, 5);
    assert!(snapshot.cycle_percentiles.p50_ms >= 10.0);
    assert!(snapshot.cycle_percentiles.p95_ms >= snapshot.cycle_percentiles.p50_ms);
    assert_eq!(snapshot.cycle_percentiles.max_ms, 25.0);
}

#[test]
fn runtime_metrics_default_reports_vm_and_empty_state() {
    let metrics = RuntimeMetrics::<StoppedClock>::default();
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
    let mut metrics = metrics();
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
    let mut metrics = metrics();
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
        .expect("fast task row");
    let slow = snapshot
        .tasks
        .iter()
        .find(|task| task.name == "Slow")
        .expect("slow task row");

    assert_eq!(fast.overruns, 0);
    assert_eq!(fast.avg_ms, 2.0);
    assert_eq!(slow.overruns, 2);
    assert_eq!(slow.avg_ms, 8.0);
    assert_eq!(snapshot.overruns, 2);
}

#[test]
fn profiling_records_call_entries_with_cycle_contribution() {
    let mut metrics = metrics();
    metrics.record_cycle(duration_ms(10));
    metrics.record_cycle(duration_ms(10));
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(4));
    metrics.record_call("fb", &SmolStr::new("TON_1"), duration_ms(2));

    let snapshot = metrics.snapshot();
    assert!(snapshot.profiling.enabled);
    assert_eq!(snapshot.profiling.calls.len(), 2);
    let main = snapshot
        .profiling
        .calls
        .iter()
        .find(|entry| entry.key.as_str() == "program:MAIN")
        .expect("missing program profile entry");
    assert_eq!(main.calls, 1);
    assert!(main.avg_cycle_ms > 0.0);
}

#[test]
fn repeated_call_identity_aggregates_without_merging_other_kind() {
    let mut metrics = metrics();
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
        .expect("program call row");
    let fb = snapshot
        .profiling
        .calls
        .iter()
        .find(|call| call.key == "fb:MAIN")
        .expect("function block call row");

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
    let mut metrics = metrics();
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
    let mut metrics = metrics();
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
    let mut metrics = metrics();
    metrics.record_cycle(duration_ms(1));
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(3));

    let top = metrics.snapshot().profiling.top_contributors.remove(0);

    assert_eq!(top.cycle_pct, 300.0);
    assert_eq!(top.last_cycle_pct, 300.0);
}

#[test]
fn top_contributors_are_limited_to_five_and_ties_use_key_order() {
    let mut metrics = metrics();
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
fn profiling_top_contributors_ranked_by_cycle_budget() {
    let mut metrics = metrics();
    for _ in 0..4 {
        metrics.record_cycle(duration_ms(10));
        metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(5));
        metrics.record_call("fb", &SmolStr::new("TON_1"), duration_ms(2));
    }

    let snapshot = metrics.snapshot();
    let top = snapshot
        .profiling
        .top_contributors
        .first()
        .expect("expected top contributor");
    assert_eq!(top.key.as_str(), "program:MAIN");
    assert!(top.cycle_pct > 0.0);
}

#[test]
fn disabling_profiling_clears_calls_and_ignores_disabled_records() {
    let mut metrics = metrics();
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
fn profiling_toggle_disables_and_reenables_collection() {
    let mut metrics = metrics();
    metrics.set_profiling_enabled(false);
    metrics.record_cycle(duration_ms(10));
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(3));
    let disabled = metrics.snapshot();
    assert!(!disabled.profiling.enabled);
    assert!(disabled.profiling.calls.is_empty());
    assert!(disabled.profiling.top_contributors.is_empty());

    metrics.set_profiling_enabled(true);
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(3));
    let enabled = metrics.snapshot();
    assert!(enabled.profiling.enabled);
    assert_eq!(enabled.profiling.calls.len(), 1);
}

#[test]
fn idempotent_enabled_toggle_preserves_existing_profile_samples() {
    let mut metrics = metrics();
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(2));

    metrics.set_profiling_enabled(true);

    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.profiling.calls.len(), 1);
    assert_eq!(snapshot.profiling.calls[0].calls, 1);
}

#[test]
fn profiling_toggle_does_not_reset_cycle_task_fault_or_overrun_metrics() {
    let mut metrics = metrics();
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
    let mut metrics = metrics();
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

#[test]
fn execution_backend_is_recorded_for_the_snapshot() {
    let mut metrics = metrics();
    metrics.set_execution_backend(ExecutionBackend::BytecodeVm);
    assert_eq!(
        metrics.snapshot().execution_backend.as_str(),
        "vm",
        "the snapshot reports the recorded backend"
    );
}

#[test]
fn calls_whose_text_kind_name_is_the_same_share_one_entry() {
    let mut metrics = metrics();
    metrics.record_cycle(duration_ms(10));
    metrics.record_call("a", &SmolStr::new("b:c"), duration_ms(1));
    metrics.record_call("a:b", &SmolStr::new("c"), duration_ms(3));
    metrics.record_call("a-", &SmolStr::new("b"), duration_ms(2));

    let calls = metrics.snapshot().profiling.calls;
    let keys: Vec<&str> = calls.iter().map(|call| call.key.as_str()).collect();
    assert_eq!(
        keys,
        ["a:b:c", "a-:b"],
        "the key is the text `kind:name`: two calls with the same text are one entry"
    );
    let shared = &calls[0];
    assert_eq!(shared.calls, 2);
    assert_eq!(
        (shared.kind.as_str(), shared.name.as_str()),
        ("a", "b:c"),
        "the entry keeps the kind and name of its first call"
    );
}

#[test]
fn restart_clears_every_record_and_measures_uptime_with_the_new_clock() {
    let mut metrics = RuntimeMetrics::with_clock(FixedClock(duration_ms(5_000)));
    let task = SmolStr::new("Task");
    metrics.record_cycle(duration_ms(5));
    metrics.record_task(&task, duration_ms(2));
    metrics.record_overrun(&task, 3);
    metrics.record_fault();
    metrics.record_call("program", &SmolStr::new("MAIN"), duration_ms(2));
    metrics.set_profiling_enabled(false);

    metrics.restart(FixedClock(duration_ms(7)));
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.uptime_ms, 7, "the new clock measures the uptime");
    assert_eq!(snapshot.cycle.samples(), 0);
    assert_eq!(snapshot.cycle.last_ms, 0.0);
    assert_eq!(snapshot.cycle_percentiles.window_samples, 0);
    assert_eq!(snapshot.faults, 0);
    assert_eq!(snapshot.overruns, 0);
    assert!(
        snapshot.tasks.is_empty(),
        "no task has run since the restart"
    );
    assert!(
        snapshot.profiling.enabled,
        "profiling is on, as in new metrics"
    );
    assert!(
        snapshot.profiling.calls.is_empty(),
        "no call is recorded since the restart"
    );
    assert_eq!(snapshot.execution_backend.as_str(), "vm");

    metrics.record_cycle(duration_ms(4));
    let snapshot = metrics.snapshot();
    assert_eq!(snapshot.cycle.samples(), 1);
    assert_eq!(snapshot.cycle_percentiles.window_samples, 1);
    assert_eq!(snapshot.cycle_percentiles.max_ms, 4.0);
}

#[test]
fn scan_allocations_add_up_from_none_and_a_restart_clears_them() {
    // Specification 34, "The allocation count": the metrics report no count until a scan is counted,
    // then the totals of every counted scan, and a restart starts them again. Worked by hand: scans
    // of 0 + 0 and 2 + 5 operations give an engine total of 2 and a services total of 5. The wrong
    // behaviour: a zero reported for scans that were never counted, the last scan reported instead
    // of the total, or the totals kept across a restart.
    let mut metrics = metrics();
    assert_eq!(metrics.snapshot().scan_allocations, None);
    metrics.record_scan_allocations(0, 0);
    assert_eq!(
        metrics.snapshot().scan_allocations,
        Some(ScanAllocations {
            engine: 0,
            services: 0
        })
    );
    metrics.record_scan_allocations(2, 5);
    assert_eq!(
        metrics.snapshot().scan_allocations,
        Some(ScanAllocations {
            engine: 2,
            services: 5
        })
    );
    metrics.restart(StoppedClock);
    assert_eq!(metrics.snapshot().scan_allocations, None);
}
