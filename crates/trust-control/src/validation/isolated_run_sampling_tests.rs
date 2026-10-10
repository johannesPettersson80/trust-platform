use super::*;

/// The recorded times are exact multiples of the step, so a tight tolerance holds.
fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-12,
        "{actual} is not {expected}"
    );
}

#[test]
fn a_run_has_one_scan_at_time_zero_and_one_per_step_to_its_end() {
    // verification-client.test.ts: 500 s at 20 ms is 25,001 scans.
    assert_eq!(scan_count(20, 500_000), 25_001);
    assert_eq!(scan_count(100, 1_000), 11);
    assert_eq!(scan_count(100, 1_050), 11, "a partial step adds no scan");
    assert_eq!(scan_count(1_000, 0), 1);
}

#[test]
fn sixty_four_channels_of_a_500_s_run_at_20_ms_are_recorded_every_second_scan() {
    // verification-client.test.ts: 25,001 scans of 64 channels at a record period of 1 give 1,600,064 channel samples,
    // above the cell limit of 1,600,000, so the automatic record period is 2.
    assert!(scan_count(20, 500_000) * 64 > ISOLATED_SIMULATION_CELL_LIMIT);
    assert_eq!(isolated_record_every(20, 500_000, 64), 2);
}

#[test]
fn the_record_period_is_the_larger_of_the_sample_bound_and_the_cell_bound() {
    // 86,400,001 scans at 1 ms: 865 scans per sample keep the run within 100,000 samples. One channel fits 1,600,000
    // samples, so the sample limit decides.
    assert_eq!(isolated_record_every(1, 86_400_000, 1), 865);
    // 64 channels fit 25,000 samples: ⌈86,400,001 / 25,000⌉ = 3,457 scans per sample.
    assert_eq!(isolated_record_every(1, 86_400_000, 64), 3_457);
    // A fast run keeps every scan.
    assert_eq!(isolated_record_every(10, 10_000, 3), 1);
}

#[test]
fn a_four_hour_slow_process_stays_within_the_sample_limits_at_every_channel_count() {
    // control-studio-scenario-duration.test.ts: 14,400 s at 100 ms is 144,001 scans, and the record period keeps the run
    // within 360 executions per sample. Checked for every channel count the runtime accepts (64).
    for channels in 1..=64 {
        assert!(
            isolated_record_every(100, 14_400_000, channels) <= 360,
            "{channels} channels"
        );
    }
}

#[test]
fn the_recorded_times_are_the_means_of_each_block_of_scans_and_the_last_block_may_be_short() {
    let times = recorded_sample_times(100, 1_000, 3);
    assert_eq!(times.len(), 4);
    for (actual, expected) in times.iter().zip([0.1, 0.4, 0.7, 0.95]) {
        assert_close(*actual, expected);
    }
    // One scan per sample: the scan times themselves, from zero to the end of the run.
    let every_scan = recorded_sample_times(100, 1_000, 1);
    assert_eq!(every_scan.len(), 11);
    assert_close(every_scan[10], 1.0);
}
