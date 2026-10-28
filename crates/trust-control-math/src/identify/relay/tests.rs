#![allow(clippy::float_cmp, clippy::cast_precision_loss)] // The period of the cycles is compared exactly where the record makes it exact.

use super::*;

fn sample(time_seconds: f64, process_value: f64, output: f64) -> RelaySample {
    RelaySample {
        time_seconds,
        process_value,
        output,
    }
}

/// The 11-sample record of the `TypeScript` test for the repeatability of a relay: the start-up sample and the
/// restoration are not complete relay periods, so they are excluded.
fn startup_record() -> Vec<RelaySample> {
    [
        [0.0, 0.0, 0.0],
        [0.1, 0.0, 1.0],
        [0.3, 0.2, -1.0],
        [0.7, -0.2, 1.0],
        [1.1, 0.2, -1.0],
        [1.5, -0.2, 1.0],
        [1.9, 0.2, -1.0],
        [2.3, -0.2, 1.0],
        [2.7, 0.2, -1.0],
        [3.1, -0.2, 1.0],
        [3.2, 0.0, 0.0],
    ]
    .iter()
    .map(|[time, process, output]| sample(*time, *process, *output))
    .collect()
}

/// The nine-sample record of an 80 / 70 relay with three cycles of 2 s.
fn feedback_record() -> Vec<RelaySample> {
    [
        (0.0, 35.0, 80.0),
        (1.0, 35.2, 70.0),
        (2.0, 34.8, 80.0),
        (3.0, 35.21, 70.0),
        (4.0, 34.79, 80.0),
        (5.0, 35.19, 70.0),
        (6.0, 34.81, 80.0),
        (7.0, 35.22, 70.0),
        (8.0, 34.78, 80.0),
    ]
    .into_iter()
    .map(|(time, process, output)| sample(time, process, output))
    .collect()
}

fn cycle(peak: f64, trough: f64, period_seconds: f64) -> RelayCycle {
    RelayCycle {
        peak,
        trough,
        period_seconds,
    }
}

#[test]
fn the_repeatability_excludes_the_start_up_and_the_restoration() {
    let result =
        relay_pi_from_record(1.0, -1.0, &startup_record()).expect("the record has complete cycles");
    assert_eq!(
        result.cycles.len(),
        3,
        "the incomplete first and last legs are excluded"
    );
    for cycle in &result.cycles {
        assert!((cycle.period_seconds - 0.8).abs() <= 1e-12);
    }
    assert!(
        matches!(result.calculation, RelayPiCalculation::Qualified(_)),
        "recording startup and restoration are not complete relay periods"
    );
}

#[test]
fn intermediate_actuator_values_are_reported_instead_of_missing_transitions() {
    // The output ramps through zero between the two levels, so no cycle is an ideal two-level relay.
    let samples: Vec<RelaySample> = [-1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 1.0, 0.0, -1.0, 0.0, 1.0]
        .iter()
        .enumerate()
        .map(|(index, output)| sample(index as f64 * 0.1, (index as f64).sin(), *output))
        .collect();
    assert_eq!(
        extract_relay_cycles(1.0, -1.0, &samples),
        Err(RelayRecordError::IntermediateOutput)
    );
}

#[test]
fn the_candidate_numbers_follow_the_ziegler_nichols_relay_rule() {
    let cycles = [
        cycle(35.2, 34.8, 4.0),
        cycle(35.21, 34.81, 4.1),
        cycle(35.19, 34.79, 3.9),
    ];
    let RelayPiCalculation::Qualified(result) = calculate_relay_pi_candidate(80.0, 70.0, &cycles)
    else {
        panic!("the three cycles qualify");
    };
    assert!((result.ultimate_gain - 31.830_988_6).abs() <= 1e-5);
    assert!((result.period_seconds - 4.0).abs() <= 1e-9);
    assert!((result.candidate.proportional_gain - 14.323_944_9).abs() <= 1e-5);
    assert!((result.candidate.integral_time_seconds - 3.333_333_3).abs() <= 1e-5);
    assert!((result.candidate.integral_gain - 4.297_183_5).abs() <= 1e-5);
    assert_eq!(
        calculate_relay_pi_candidate(80.0, 70.0, &cycles[..1]),
        RelayPiCalculation::NotQualified(RelayRejection::TooFewCycles)
    );
}

#[test]
fn nonrepeatable_amplitudes_and_periods_block_the_candidate() {
    for (variant, quantity) in [
        ("amplitude", RelayQuantity::Amplitude),
        ("period", RelayQuantity::Period),
    ] {
        let cycles: Vec<RelayCycle> = [1.0, 1.0, 2.0]
            .iter()
            .map(|factor| {
                cycle(
                    if variant == "amplitude" { *factor } else { 1.0 },
                    0.0,
                    if variant == "period" { *factor } else { 1.0 },
                )
            })
            .collect();
        assert_eq!(
            calculate_relay_pi_candidate(80.0, 70.0, &cycles),
            RelayPiCalculation::NotQualified(RelayRejection::Spread(quantity)),
            "{variant} variation must block the candidate"
        );
    }
}

#[test]
fn cycles_are_extracted_only_from_measured_feedback_driven_transitions() {
    let cycles = extract_relay_cycles(80.0, 70.0, &feedback_record())
        .expect("the record has three complete cycles");
    let expected = [
        cycle(35.21, 34.79, 2.0),
        cycle(35.19, 34.79, 2.0),
        cycle(35.22, 34.78, 2.0),
    ];
    assert_eq!(cycles.len(), expected.len());
    for (found, want) in cycles.iter().zip(expected) {
        assert!((found.peak - want.peak).abs() <= 1e-12);
        assert!((found.trough - want.trough).abs() <= 1e-12);
        assert_eq!(found.period_seconds, want.period_seconds);
    }
    let one_level = [0.0, 1.0, 2.0].map(|time| sample(time, 35.0 + time / 10.0, 80.0));
    assert_eq!(
        extract_relay_cycles(80.0, 70.0, &one_level),
        Err(RelayRecordError::TooFewTransitions)
    );
}

#[test]
fn a_record_qualifies_a_candidate_only_after_its_cycles_qualify() {
    let result = relay_pi_from_record(80.0, 70.0, &feedback_record())
        .expect("the record has three complete cycles");
    assert!(matches!(
        result.calculation,
        RelayPiCalculation::Qualified(_)
    ));
    assert_eq!(result.cycles.len(), 3);
}

#[test]
fn malformed_levels_and_samples_are_refused_with_their_reason() {
    assert_eq!(
        extract_relay_cycles(1.0, 1.0, &startup_record()),
        Err(RelayRecordError::Levels),
        "the low level must be below the high level"
    );
    assert_eq!(
        extract_relay_cycles(1.0, -1.0, &startup_record()[..2]),
        Err(RelayRecordError::Samples),
        "fewer than three samples"
    );
}
