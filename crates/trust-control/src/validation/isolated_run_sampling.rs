//! The sample grid of an isolated run (`isolatedRunSampling.ts`; specification 35, A4.2 and A4.6; limits of A7). An isolated
//! run executes one scan at time zero and one per step to its end. Each recorded sample averages `every` consecutive scans,
//! so the grid follows from the step, the duration and the record period. Times are numbers passed in: nothing here reads
//! a clock.

use crate::limits::{ISOLATED_SIMULATION_CELL_LIMIT, ISOLATED_SIMULATION_SAMPLE_LIMIT};

/// The scans a run executes: one at time zero and one per step to its end, `⌊duration / step⌋ + 1` (A4.6).
///
/// The runtime runs whole-millisecond steps of one millisecond or more, so a zero step counts as one millisecond rather
/// than dividing by zero.
#[must_use]
pub fn scan_count(step_ms: u64, duration_ms: u64) -> u64 {
    (duration_ms / step_ms.max(1)).saturating_add(1)
}

/// The smallest record period, in scans per recorded sample, that keeps a run within the sample limit and the cell limit
/// (A4.2, A7): at most `ISOLATED_SIMULATION_SAMPLE_LIMIT` samples, and their count times the observed channels at most
/// `ISOLATED_SIMULATION_CELL_LIMIT`. It is never below one.
///
/// The cell condition `⌈scans / k⌉ · channels ≤ cells` holds exactly when `⌈scans / k⌉ ≤ ⌊cells / channels⌋`, which holds
/// exactly when `k ≥ ⌈scans / ⌊cells / channels⌋⌉`. The period is the larger of the two bounds, found in one step rather
/// than by counting up. Channels count as at least one. The caller refuses more than `ISOLATED_SIMULATION_CHANNEL_LIMIT`
/// channels first. When the channels alone exceed the cell limit, one sample remains and the period is the whole run.
#[must_use]
pub fn isolated_record_every(step_ms: u64, duration_ms: u64, observed_channels: u64) -> u64 {
    let scans = scan_count(step_ms, duration_ms);
    let by_sample_limit = scans.div_ceil(ISOLATED_SIMULATION_SAMPLE_LIMIT).max(1);
    let samples_within_cells = (ISOLATED_SIMULATION_CELL_LIMIT / observed_channels.max(1)).max(1);
    by_sample_limit.max(scans.div_ceil(samples_within_cells))
}

/// The times of a run's recorded samples in seconds (`recordedSampleTimes`): each sample averages `every` consecutive
/// scans, which lie `step_ms` apart from time zero, and takes the mean of their times. The last sample may average fewer
/// scans (A4.2).
#[must_use]
pub fn recorded_sample_times(step_ms: u64, duration_ms: u64, every: u64) -> Vec<f64> {
    let scans = scan_count(step_ms, duration_ms);
    let every = every.max(1);
    let mut times = Vec::new();
    let mut first: u64 = 0;
    while first < scans {
        let last = first.saturating_add(every - 1).min(scans - 1);
        times.push(f64::midpoint(as_f64(first), as_f64(last)) * as_f64(step_ms) / 1000.0);
        first = first.saturating_add(every);
    }
    times
}

/// A count, or a time in whole milliseconds, as a float: exact below 2^53, far above the limits of A7.
#[allow(clippy::cast_precision_loss)]
fn as_f64(count: u64) -> f64 {
    count as f64
}

#[cfg(test)]
#[path = "isolated_run_sampling_tests.rs"]
mod tests;
