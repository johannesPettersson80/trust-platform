//! Sample checks shared by a stopped run and a status page (runtime protocol annex, E0.3, E4.5 and E4.6): every sample
//! holds one value, forced flag and quality per channel, a BOOL channel reads exactly 0 or 1 (E0.3), and the samples step
//! forward in scan and in runtime time (`decodeSamples` of `runtimeRecording.ts`).

use crate::failure::{Cause, Failure, NextAction};
use crate::signals::SignalBinding;

use super::RecordedSample;

/// The position of a sample: its scan and its runtime time. A follow of a running recording carries the last one it read
/// (`followCursor` of `runtimeRecording.ts`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplePosition {
    /// The sample's last sequence value.
    pub scan: u64,
    /// The sample's runtime time in nanoseconds.
    pub runtime_time_ns: i64,
}

impl SamplePosition {
    /// The position of `sample`, when it carries its runtime time.
    #[must_use]
    pub fn of(sample: &RecordedSample) -> Option<Self> {
        sample.runtime_time_ns.map(|runtime_time_ns| Self {
            scan: sample.scan,
            runtime_time_ns,
        })
    }
}

/// Whether every sample holds one value, one forced flag and one quality per channel, and every value is finite (E4.6).
#[must_use]
pub fn aligned(samples: &[RecordedSample], channels: usize) -> bool {
    samples.iter().all(|sample| {
        sample.values.len() == channels
            && sample.forced.len() == channels
            && sample.quality.len() == channels
            && sample
                .values
                .iter()
                .flatten()
                .all(|value| value.is_finite())
    })
}

/// Whether the samples step strictly forward from `previous`, or from the start of the recording when there is none: each
/// scan and each runtime time is greater than the one before, and each sample carries its runtime time (E4.5). Without a
/// previous sample the first runtime time must be 0 or later.
#[must_use]
pub fn samples_increase(samples: &[RecordedSample], previous: Option<SamplePosition>) -> bool {
    let mut last = previous;
    for sample in samples {
        let Some(runtime_time_ns) = sample.runtime_time_ns else {
            return false;
        };
        let increases = match last {
            Some(last) => sample.scan > last.scan && runtime_time_ns > last.runtime_time_ns,
            None => runtime_time_ns >= 0,
        };
        if !increases {
            return false;
        }
        last = Some(SamplePosition {
            scan: sample.scan,
            runtime_time_ns,
        });
    }
    true
}

/// Refuses a BOOL channel whose value is neither 0 nor 1 (E0.3). A missing value is not a value and is not refused.
///
/// # Errors
/// A BOOL channel of a sample reads a value other than 0 or 1.
pub fn check_bool_values(
    bindings: &[SignalBinding],
    samples: &[RecordedSample],
) -> Result<(), Failure> {
    let is_bool: Vec<bool> = bindings
        .iter()
        .map(|binding| binding.iec_type.eq_ignore_ascii_case("BOOL"))
        .collect();
    let refused = samples.iter().any(|sample| {
        sample.values.iter().zip(&is_bool).any(|(value, is_bool)| {
            *is_bool && matches!(value, Some(value) if !is_zero_or_one(*value))
        })
    });
    if refused {
        return Err(Failure::new(
            "The recording contains an invalid BOOL value.",
            Cause::Runtime,
            NextAction::None,
        ));
    }
    Ok(())
}

/// The refusal of samples that are malformed or out of order (`decodeSamples`).
pub(crate) fn invalid_samples() -> Failure {
    Failure::new(
        "The runtime recording contains invalid samples.",
        Cause::Runtime,
        NextAction::None,
    )
}

/// The seconds from `origin` to `time`, both in nanoseconds of process time.
#[allow(clippy::cast_precision_loss)] // Differences of process time stay far below 2^53 ns.
pub(crate) fn seconds_between(origin: i64, time: i64) -> f64 {
    time.saturating_sub(origin) as f64 / 1e9
}

/// A BOOL channel reads exactly 0 or 1 (E0.3).
#[allow(clippy::float_cmp)] // A BOOL value is exactly 0 or 1, or it is invalid.
fn is_zero_or_one(value: f64) -> bool {
    value == 0.0 || value == 1.0
}

#[cfg(test)]
mod tests {
    use crate::recording::fixtures::{binding, sample};
    use crate::recording::RecordedSample;
    use crate::signals::Quality;

    use super::{aligned, check_bool_values, samples_increase, SamplePosition};

    fn at(scan: u64, runtime_time_ns: Option<i64>) -> RecordedSample {
        sample(scan, runtime_time_ns, &[Some(1.0)], &[Quality::Good])
    }

    #[test]
    fn samples_must_step_forward_in_scan_and_runtime_time() {
        assert!(samples_increase(&[at(1, Some(0)), at(2, Some(10))], None));
        assert!(
            !samples_increase(&[at(2, Some(0)), at(2, Some(10))], None),
            "a repeated scan does not step forward"
        );
        assert!(
            !samples_increase(&[at(1, Some(10)), at(2, Some(10))], None),
            "a runtime time that does not increase is refused"
        );
        assert!(
            !samples_increase(&[at(1, Some(-1))], None),
            "the first runtime time is 0 or later"
        );
    }

    #[test]
    fn a_page_steps_forward_from_the_last_sample_of_the_page_before() {
        let last = Some(SamplePosition {
            scan: 5,
            runtime_time_ns: 50,
        });
        assert!(samples_increase(&[at(6, Some(60))], last));
        assert!(
            !samples_increase(&[at(5, Some(60))], last),
            "the next page does not repeat the last scan"
        );
        assert!(
            !samples_increase(&[at(6, Some(50))], last),
            "nor does it repeat its last time"
        );
    }

    #[test]
    fn a_sample_without_its_runtime_time_does_not_step_forward() {
        assert!(!samples_increase(&[at(1, None)], None));
    }

    #[test]
    fn a_bool_channel_reads_exactly_zero_or_one() {
        let bindings = [
            binding("processVariable", "%IL0", "LREAL"),
            binding("valid", "%MX16.0", "BOOL"),
        ];
        let with = |value: f64| {
            vec![sample(
                1,
                Some(0),
                &[Some(2.5), Some(value)],
                &[Quality::Good, Quality::Good],
            )]
        };
        assert!(check_bool_values(&bindings, &with(0.0)).is_ok());
        assert!(check_bool_values(&bindings, &with(1.0)).is_ok());
        assert!(check_bool_values(&bindings, &with(0.5)).is_err());
        assert!(check_bool_values(&bindings, &with(2.0)).is_err());
        assert!(
            check_bool_values(
                &bindings,
                &[sample(
                    1,
                    Some(0),
                    &[Some(2.5), None],
                    &[Quality::Good, Quality::Missing]
                )]
            )
            .is_ok(),
            "a missing BOOL is not a value"
        );
    }

    #[test]
    fn alignment_counts_every_channel_and_refuses_a_non_finite_value() {
        assert!(aligned(&[at(1, Some(0))], 1));
        assert!(!aligned(&[at(1, Some(0))], 2));
        let infinite = sample(1, Some(0), &[Some(f64::INFINITY)], &[Quality::Good]);
        assert!(!aligned(&[infinite], 1));
    }
}
