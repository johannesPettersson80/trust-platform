//! Tests of the relay evidence (numerical annex N2.9 and N5.R) on synthetic relay records of known amplitude and period.

use std::f64::consts::{PI, TAU};

use super::*;
use crate::recording::channels::{MANIPULATED_VARIABLE_BINDING, PROCESS_VARIABLE_BINDING};
use crate::recording::fixtures::{binding, run, sample};
use crate::signals::Quality;

/// The samples of one relay period, and the sampling step and length of a period in seconds: 400 samples of 0.01 s make a
/// period of 4 s, so every peak and trough of the sine falls on a sample.
const SAMPLES_PER_PERIOD: u32 = 400;
const SAMPLE_SECONDS: f64 = 0.01;
const PERIOD_SECONDS: f64 = 4.0;

/// A relay record of `periods` periods from the origin: the output is +1 in the first half of each period and −1 in the
/// second, so every period after the first starts at a low-to-high transition, and the measurement is the sine of the
/// period's phase times `amplitude(period)`.
fn relay_record(periods: u32, amplitude: impl Fn(u32) -> f64) -> Vec<RelaySample> {
    (0..=periods * SAMPLES_PER_PERIOD)
        .map(|index| {
            let period = index / SAMPLES_PER_PERIOD;
            let phase = index % SAMPLES_PER_PERIOD;
            let time_seconds = f64::from(index) * SAMPLE_SECONDS;
            RelaySample {
                time_seconds,
                process_value: amplitude(period) * (TAU * time_seconds / PERIOD_SECONDS).sin(),
                output: if phase < SAMPLES_PER_PERIOD / 2 {
                    1.0
                } else {
                    -1.0
                },
            }
        })
        .collect()
}

#[test]
fn a_synthetic_relay_record_qualifies_with_its_ultimate_gain_and_period() {
    // d = 1 (levels +1 and −1), a = 1 (the sine's peak), Pu = 4 s, so Ku = 4/π, Kp = 0.45·Ku and Ti = Pu/1.2 (N5.R).
    let evidence = evidence(1.0, -1.0, &relay_record(5, |_| 1.0));
    let RelayCycleEvidence::Qualified {
        cycles,
        output_amplitude,
        response_amplitude,
        period_seconds,
        ultimate_gain,
        proportional_gain,
        integral_time_seconds,
        integral_gain,
    } = evidence
    else {
        panic!("five periods of a clean relay oscillation qualify: {evidence:?}");
    };
    assert_eq!(cycles, 4, "five high starts give four complete cycles");
    assert!((output_amplitude - 1.0).abs() <= 1e-12);
    assert!((response_amplitude - 1.0).abs() <= 1e-9);
    assert!((period_seconds - PERIOD_SECONDS).abs() <= 1e-9);
    assert!((ultimate_gain - 4.0 / PI).abs() <= 1e-9);
    assert!((proportional_gain - 0.45 * ultimate_gain).abs() <= 1e-12);
    assert!((integral_time_seconds - PERIOD_SECONDS / 1.2).abs() <= 1e-9);
    assert!((integral_gain - proportional_gain / integral_time_seconds).abs() <= 1e-12);
}

#[test]
fn fewer_than_three_complete_cycles_do_not_qualify_with_the_n2_9_sentence() {
    // Three high starts make two complete cycles.
    let evidence = evidence(1.0, -1.0, &relay_record(3, |_| 1.0));
    assert_eq!(
        evidence,
        RelayCycleEvidence::NotQualified {
            reason: "At least three complete feedback relay cycles are required.".to_owned(),
        }
    );
}

#[test]
fn an_amplitude_spread_above_ten_percent_of_its_mean_names_the_amplitude() {
    // The cycles of periods 1 to 4 have amplitudes 1.0, 1.5, 1.0 and 1.0: their spread is 0.5 on a mean of 1.125.
    let evidence = evidence(
        1.0,
        -1.0,
        &relay_record(5, |period| if period == 2 { 1.5 } else { 1.0 }),
    );
    assert_eq!(
        evidence,
        RelayCycleEvidence::NotQualified {
            reason: "Relay amplitude spread exceeds 10% of its mean. Record more settled cycles or use another permitted identification method.".to_owned(),
        }
    );
}

#[test]
fn a_record_whose_levels_or_samples_cannot_be_read_names_the_defect() {
    assert_eq!(
        evidence(1.0, 1.0, &relay_record(5, |_| 1.0)),
        RelayCycleEvidence::NotQualified {
            reason: "Relay output levels must be finite and ordered.".to_owned(),
        }
    );
    assert_eq!(
        evidence(1.0, -1.0, &[]),
        RelayCycleEvidence::NotQualified {
            reason: "Relay samples need finite strictly increasing measured times, process values and outputs.".to_owned(),
        }
    );
}

#[test]
fn samples_of_keeps_only_the_samples_whose_measurement_and_output_are_numbers() {
    let record = run(
        vec![
            binding(PROCESS_VARIABLE_BINDING, "%IW0", "REAL"),
            binding(MANIPULATED_VARIABLE_BINDING, "%QW0", "REAL"),
        ],
        vec![
            sample(
                1,
                Some(10_000_000_000),
                &[Some(0.5), Some(1.0)],
                &[Quality::Good, Quality::Good],
            ),
            sample(
                2,
                Some(10_500_000_000),
                &[Some(0.7), None],
                &[Quality::Good, Quality::Good],
            ),
            sample(
                3,
                Some(11_000_000_000),
                &[Some(0.9), Some(-1.0)],
                &[Quality::Good, Quality::Good],
            ),
        ],
    );
    assert_eq!(
        samples_of(&record),
        vec![
            RelaySample {
                time_seconds: 0.0,
                process_value: 0.5,
                output: 1.0,
            },
            RelaySample {
                time_seconds: 1.0,
                process_value: 0.9,
                output: -1.0,
            },
        ]
    );
}
