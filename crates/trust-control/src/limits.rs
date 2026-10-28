//! Control Studio's limits (specification 35, A7) and the record period of A4.2. Each limit is
//! enforced in one place and quoted from here in messages. Durations are milliseconds of process
//! time unless the name says wall time.

/// The shortest experiment.
pub const MINIMUM_EXPERIMENT_DURATION_MS: u64 = 1_000;
/// The longest experiment: 24 h.
pub const MAXIMUM_EXPERIMENT_DURATION_MS: u64 = 86_400_000;
/// The longest recording: 24 h.
pub const RECORDING_DURATION_LIMIT_MS: u64 = 86_400_000;
/// The most samples one recording keeps.
pub const RECORDING_SAMPLE_LIMIT: u64 = 100_000;
/// The most channels one recording keeps.
pub const RECORDING_CHANNEL_LIMIT: u64 = 32;
/// The most samples times channels one recording keeps.
pub const RECORDING_CELL_LIMIT: u64 = 1_600_000;
/// The most recordings one runtime keeps at once.
pub const RECORDINGS_PER_RUNTIME: u64 = 2;
/// The most points one fit receives; a longer recording is block-averaged down to it (A4.5).
pub const FIT_POINT_LIMIT: u64 = 5_000;
/// The longest isolated simulation: 24 h.
pub const ISOLATED_SIMULATION_DURATION_LIMIT_MS: u64 = 86_400_000;
/// The most samples one isolated simulation keeps.
pub const ISOLATED_SIMULATION_SAMPLE_LIMIT: u64 = 100_000;
/// The most channels one isolated simulation keeps.
pub const ISOLATED_SIMULATION_CHANNEL_LIMIT: u64 = 64;
/// The most samples times channels one isolated simulation keeps.
pub const ISOLATED_SIMULATION_CELL_LIMIT: u64 = 1_600_000;
/// The most scans one isolated simulation runs.
pub const ISOLATED_SIMULATION_SCAN_LIMIT: u64 = 10_000_000;
/// The longest an isolated simulation may take, in wall time.
pub const ISOLATED_SIMULATION_WALL_TIME_LIMIT_MS: u64 = 600_000;
/// The most trials one search runs.
pub const SEARCH_TRIAL_LIMIT: u64 = 64;
/// The longest a search may take, in wall time.
pub const SEARCH_WALL_TIME_LIMIT_MS: u64 = 3_600_000;
/// The longest Apply observation: 24 h.
pub const APPLY_OBSERVATION_LIMIT_MS: u64 = 86_400_000;
/// Restoration is confirmed by the third execution of the loop's task after the restoring write.
pub const RESTORATION_CONFIRMATION_EXECUTIONS: u64 = 3;
/// The most one loop's study keeps in its project, recordings included: 256 MiB.
pub const STUDY_STORAGE_LIMIT_BYTES: u64 = 256 * 1024 * 1024;
/// Wall time without a newer scan after which a value is stale (A3.4).
pub const CONNECTION_LOSS_TIMEOUT_MS: u64 = 5_000;
/// Samples a recording keeps over the expected settling time of the response it records, and at
/// least in all (A4.2).
pub const SAMPLES_PER_SETTLING_TIME: u64 = 200;

/// The connection-loss timeout: 5 s, or three resource cycles when that is longer. An unknown
/// cycle keeps 5 s.
#[must_use]
pub fn connection_loss_timeout_ms(resource_cycle_ms: Option<u64>) -> u64 {
    match resource_cycle_ms {
        Some(cycle) if cycle > 0 => CONNECTION_LOSS_TIMEOUT_MS.max(3 * cycle),
        _ => CONNECTION_LOSS_TIMEOUT_MS,
    }
}

/// The record period of A4.2, in executions of the loop's task.
///
/// It follows the process: about 200 samples over the expected settling time, and at least 200 in
/// all, so a slow process is not recorded at every execution of a fast task. It is never below the
/// smallest period for which the planned duration fits `sample_limit`; without an expected
/// settling time that period is the record period.
#[must_use]
pub fn record_period_executions(
    planned_duration_ms: u64,
    task_period_ms: u64,
    sample_limit: u64,
    expected_settling_ms: Option<u64>,
) -> u64 {
    let task = task_period_ms.max(1);
    let fits = planned_duration_ms
        .div_ceil(task * sample_limit.max(1))
        .max(1);
    let Some(settling) = expected_settling_ms.filter(|settling| *settling > 0) else {
        return fits;
    };
    let per_settling = settling / (task * SAMPLES_PER_SETTLING_TIME);
    let per_recording = planned_duration_ms / (task * SAMPLES_PER_SETTLING_TIME);
    fits.max(per_settling.min(per_recording)).max(1)
}

/// The sample period of a recording in ms, which its record period counts executions of (A4.1, A4.2): the loop's task
/// period when the loop's task is scheduled, so that its executions are sampled; otherwise the resource cycle, or the task
/// period when the runtime reports no cycle (0).
#[must_use]
pub fn sample_period_ms(scheduled: bool, task_period_ms: u64, cycle_interval_ms: u64) -> u64 {
    if scheduled || cycle_interval_ms == 0 {
        task_period_ms
    } else {
        cycle_interval_ms
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sample_period_is_the_task_of_a_scheduled_loop_and_the_cycle_otherwise() {
        assert_eq!(
            sample_period_ms(true, 100, 10),
            100,
            "a scheduled loop samples its task"
        );
        assert_eq!(
            sample_period_ms(false, 100, 10),
            10,
            "a background program samples every resource cycle"
        );
        assert_eq!(
            sample_period_ms(false, 100, 0),
            100,
            "an unknown cycle keeps the task period"
        );
    }

    #[test]
    fn the_connection_loss_timeout_is_5_s_or_three_cycles() {
        assert_eq!(connection_loss_timeout_ms(None), 5_000);
        assert_eq!(connection_loss_timeout_ms(Some(0)), 5_000);
        assert_eq!(connection_loss_timeout_ms(Some(100)), 5_000);
        assert_eq!(connection_loss_timeout_ms(Some(2_000)), 6_000);
    }

    #[test]
    fn the_record_period_follows_the_process_not_the_task() {
        // A 75 min settling time on a 100 ms task: about 200 samples per settling time.
        assert_eq!(
            record_period_executions(28_800_000, 100, RECORDING_SAMPLE_LIMIT, Some(4_500_000)),
            225
        );
        // A fast process keeps every execution.
        assert_eq!(
            record_period_executions(10_000, 100, RECORDING_SAMPLE_LIMIT, Some(2_000)),
            1
        );
        // A 5 s settling time on a 10 ms task over 60 s: two executions per sample.
        assert_eq!(
            record_period_executions(60_000, 10, RECORDING_SAMPLE_LIMIT, Some(5_000)),
            2
        );
        // At least 200 samples in all: a short recording of a slow process keeps more.
        assert_eq!(
            record_period_executions(60_000, 100, RECORDING_SAMPLE_LIMIT, Some(3_600_000)),
            3
        );
    }

    #[test]
    fn without_a_settling_time_the_record_period_only_fits_the_sample_limit() {
        assert_eq!(
            record_period_executions(10_000, 100, RECORDING_SAMPLE_LIMIT, None),
            1
        );
        assert_eq!(
            record_period_executions(86_400_000, 10, RECORDING_SAMPLE_LIMIT, None),
            87
        );
    }

    #[test]
    fn the_record_period_is_never_below_the_period_that_fits_the_sample_limit() {
        assert_eq!(
            record_period_executions(86_400_000, 10, RECORDING_SAMPLE_LIMIT, Some(1_000)),
            87
        );
    }
}
