//! The points an experiment writes over time (`experimentTrajectory` and `automaticExperimentTrajectory` in
//! `experiment.ts`; specification 35, B3.4; annex E5.2 and E5.4).
//!
//! A trajectory is the complete plan of the experiment's writes. The runtime writes each point for the next
//! execution once its time has been reached (annex E5.4), so the times are process times from the first execution.

use std::f64::consts::PI;

use super::plan::{ControlExperimentKind, ControlExperimentPoint};

/// The trajectory of an automatic step: the step at the start, held to the end, since the runtime restores the value
/// from before it at the end of the duration (B3.4). A duration shorter than three executions of the task cannot
/// show the step, so it has no trajectory.
pub(super) fn automatic_trajectory(
    from: f64,
    amplitude: f64,
    duration_ms: f64,
    sample_period_ms: f64,
) -> Option<Vec<ControlExperimentPoint>> {
    if duration_ms < sample_period_ms * 3.0 {
        None
    } else {
        Some(vec![point(0.0, from + amplitude)])
    }
}

/// The trajectory a requested experiment writes (`experimentTrajectory`). A relay has none: its output follows the
/// measurement (annex E5.4).
pub(super) fn experiment_trajectory(
    kind: ControlExperimentKind,
    from: f64,
    amplitude: f64,
    duration_ms: f64,
    sample_period_ms: f64,
    seed: u32,
) -> Vec<ControlExperimentPoint> {
    match kind {
        ControlExperimentKind::Observation | ControlExperimentKind::Relay => Vec::new(),
        ControlExperimentKind::SetpointStep
        | ControlExperimentKind::LoadStep
        | ControlExperimentKind::MvStep
        | ControlExperimentKind::OutputStep => vec![point(0.0, from + amplitude)],
        ControlExperimentKind::MvPulse => vec![
            point(0.0, from + amplitude),
            point((duration_ms / 2.0).floor(), from),
        ],
        _ => {
            let interval_ms = if kind == ControlExperimentKind::Multisine {
                sample_period_ms.max(50.0)
            } else {
                (sample_period_ms * 10.0).max(500.0)
            };
            if kind == ControlExperimentKind::Prbs {
                prbs_points(from, amplitude, duration_ms, interval_ms, seed)
            } else {
                multisine_points(from, amplitude, duration_ms, interval_ms)
            }
        }
    }
}

/// A pseudo-random binary sequence from a 32-bit xorshift generator, one bit per interval: a set bit writes the
/// high level and a clear bit the low level (`experimentTrajectory`).
pub(super) fn prbs_points(
    from: f64,
    amplitude: f64,
    duration_ms: f64,
    interval_ms: f64,
    seed: u32,
) -> Vec<ControlExperimentPoint> {
    let mut state = seed.max(1);
    (0..point_count(duration_ms, interval_ms))
        .map(|index| {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            let value = from
                + if (state & 1) == 1 {
                    amplitude
                } else {
                    -amplitude
                };
            point(point_time(index, interval_ms), value)
        })
        .collect()
}

/// A multisine of the first three harmonics of the duration, averaged (`experimentTrajectory`).
fn multisine_points(
    from: f64,
    amplitude: f64,
    duration_ms: f64,
    interval_ms: f64,
) -> Vec<ControlExperimentPoint> {
    let duration_seconds = duration_ms / 1000.0;
    (0..point_count(duration_ms, interval_ms))
        .map(|index| {
            let at_ms = point_time(index, interval_ms);
            let time = at_ms / 1000.0;
            let wave = [1.0, 2.0, 3.0].into_iter().fold(0.0, |sum, harmonic| {
                sum + (2.0 * PI * harmonic * time / duration_seconds).sin()
            }) / 3.0;
            point(at_ms, from + amplitude * wave)
        })
        .collect()
}

/// One point of a trajectory.
pub(super) fn point(at_ms: f64, value: f64) -> ControlExperimentPoint {
    ControlExperimentPoint { at_ms, value }
}

/// The number of samples of a trajectory of `duration_ms` at one sample every `interval_ms`.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // A count of samples of a bounded duration.
fn point_count(duration_ms: f64, interval_ms: f64) -> usize {
    (duration_ms / interval_ms).ceil() as usize
}

/// The time of the sample at `index`, in ms.
#[allow(clippy::cast_precision_loss)] // A sample index is far below 2⁵³.
fn point_time(index: usize, interval_ms: f64) -> f64 {
    index as f64 * interval_ms
}
