#![allow(clippy::float_cmp)] // The commissioning error of an exact target is exactly 0.0, so the tests compare it exactly.
use super::*;

fn criteria() -> CommissioningCriteria {
    CommissioningCriteria {
        target: 5.0,
        duration_seconds: 20.0,
        settling_time_seconds: 5.0,
        maximum_error: 0.1,
        task_period_ms: 500.0,
        process_range: Range {
            minimum: 0.0,
            maximum: 10.0,
        },
        output_limits: Range {
            minimum: -20.0,
            maximum: 20.0,
        },
        switching: None,
    }
}

/// The criteria of the switching tests: a 10 s response, an output between 0 and 100, and a switching check.
fn switching_criteria() -> CommissioningCriteria {
    let mut reviewed = criteria();
    reviewed.duration_seconds = 10.0;
    reviewed.output_limits = Range {
        minimum: 0.0,
        maximum: 100.0,
    };
    reviewed.switching = Some(SwitchingCriteria {
        low: 0.0,
        high: 100.0,
        minimum_on_seconds: 3.0,
        minimum_off_seconds: 3.0,
    });
    reviewed
}

fn scan_number(index: usize) -> u64 {
    u64::try_from(index + 1).expect("a short test")
}

fn sample(
    elapsed_seconds: f64,
    scan: u64,
    process_value: f64,
    applied_output: f64,
) -> CommissioningSample {
    CommissioningSample {
        elapsed_seconds,
        scan,
        in_auto: true,
        setpoint: Some(5.0),
        process_value: Some(process_value),
        applied_output: Some(applied_output),
        quality: ChannelQuality::Good,
    }
}

/// Auto scans at `times`, each on the target with the given measurement and output.
fn steady(times: &[f64], process_value: f64, applied_output: f64) -> Vec<CommissioningSample> {
    times
        .iter()
        .enumerate()
        .map(|(index, &elapsed_seconds)| {
            sample(
                elapsed_seconds,
                scan_number(index),
                process_value,
                applied_output,
            )
        })
        .collect()
}

fn recording(samples: Vec<CommissioningSample>) -> CommissioningRecording {
    CommissioningRecording {
        completeness: Completeness::Complete,
        samples,
        output_unit: "V".to_owned(),
        evidence: RunEvidence::PhysicalRecorded,
    }
}

/// A switching recording: one scan per second, on the target, with the given applied outputs.
fn switching_recording(applied: &[f64]) -> CommissioningRecording {
    let samples = applied
        .iter()
        .enumerate()
        .map(|(index, &output)| {
            sample(
                f64::from(u32::try_from(index).expect("a short test")),
                scan_number(index),
                5.0,
                output,
            )
        })
        .collect();
    recording(samples)
}

/// Eleven scans, one second apart, that move the output to the upper level and back twice: each high or low dwell is 3 s.
const SWITCHING_PATTERN: [f64; 11] = [
    0.0, 100.0, 100.0, 100.0, 0.0, 0.0, 0.0, 100.0, 100.0, 100.0, 0.0,
];

#[test]
fn a_long_response_recorded_at_a_record_period_passes_with_no_error() {
    // control-studio-apply-process-time.test.ts: a 0.5 s task recorded two executions per sample. The samples are 1 s
    // apart, and the last one, at 19.75 s, averages fewer executions (scan 41).
    let mut times: Vec<f64> = (0..20).map(f64::from).collect();
    times.push(19.75);
    let samples: Vec<CommissioningSample> = times
        .iter()
        .enumerate()
        .map(|(index, &elapsed_seconds)| {
            let scan = if index == 20 {
                41
            } else {
                2 * scan_number(index)
            };
            sample(elapsed_seconds, scan, 5.0, 2.0)
        })
        .collect();
    let error = assess_commissioning_recording(&recording(samples), &criteria())
        .expect("the response passes");
    assert!(error.abs() <= 1e-12, "the error is {error}");
}

#[test]
fn a_response_entered_at_30_s_passes_once_it_covers_the_duration_from_the_entry() {
    // B6.3, A4.2: the response starts at 30 s and its samples are recorded every 47.5 s, the last at 9,530 s. That is the reviewed
    // 9,500 s from the entry in full, so the assessment does not rely on its one-spacing tolerance.
    let mut reviewed = criteria();
    reviewed.duration_seconds = 9_500.0;
    reviewed.task_period_ms = 100.0;
    let times: Vec<f64> = (0..=200)
        .map(|index| 30.0 + 47.5 * f64::from(index))
        .collect();
    assert!(times[200] - times[0] >= reviewed.duration_seconds);
    let error = assess_commissioning_recording(&recording(steady(&times, 5.0, 2.0)), &reviewed)
        .expect("a response that covers the duration from its entry passes");
    assert!(error.abs() <= 1e-12, "the error is {error}");
}

#[test]
fn an_incomplete_recording_or_a_sample_that_is_not_good_is_refused() {
    let expected = Failure::new(
        &texts::commissioning_recording_incomplete(),
        Cause::Runtime,
        NextAction::None,
    );
    let mut forced = steady(&[0.0, 1.0, 2.0], 5.0, 2.0);
    forced[1].quality = ChannelQuality::Forced;
    assert_eq!(
        assess_commissioning_recording(&recording(forced), &criteria()),
        Err(expected.clone())
    );
    let mut incomplete = recording(steady(&[0.0, 1.0], 5.0, 2.0));
    incomplete.completeness = Completeness::Incomplete;
    assert_eq!(
        assess_commissioning_recording(&incomplete, &criteria()),
        Err(expected)
    );
}

#[test]
fn a_response_that_never_enters_auto_at_the_target_names_the_missing_target() {
    let expected = Failure::new(
        &texts::commissioning_no_auto_target(),
        Cause::Implementation,
        NextAction::OpenLoopSetup,
    );
    let mut other_target = steady(&[0.0, 1.0, 2.0], 5.0, 2.0);
    for scan in &mut other_target {
        scan.setpoint = Some(4.0);
    }
    assert_eq!(
        assess_commissioning_recording(&recording(other_target), &criteria()),
        Err(expected.clone())
    );
    let mut manual = steady(&[0.0, 1.0, 2.0], 5.0, 2.0);
    for scan in &mut manual {
        scan.in_auto = false;
    }
    assert_eq!(
        assess_commissioning_recording(&recording(manual), &criteria()),
        Err(expected)
    );
}

#[test]
fn the_response_must_reach_the_reviewed_duration_less_one_spacing_of_its_samples() {
    let mut reviewed = criteria();
    reviewed.duration_seconds = 10.0;
    // Scans at 0 to 9 s: the last one lies one spacing (1 s) short of the 10 s duration, which is enough.
    let whole: Vec<f64> = (0..10).map(f64::from).collect();
    assert!(
        assess_commissioning_recording(&recording(steady(&whole, 5.0, 2.0)), &reviewed).is_ok(),
        "9 s reaches 10 s less one 1 s spacing"
    );
    // Scans at 0 to 8 s and at 8.99 s: the last one is 0.01 s short of the 9 s it must reach.
    let mut short: Vec<f64> = (0..9).map(f64::from).collect();
    short.push(8.99);
    assert_eq!(
        assess_commissioning_recording(&recording(steady(&short, 5.0, 2.0)), &reviewed),
        Err(Failure::new(
            &texts::commissioning_duration_not_covered(),
            Cause::Runtime,
            NextAction::None
        ))
    );
}

#[test]
fn a_sample_outside_the_mode_the_reference_or_the_ranges_violates_the_bounds() {
    let expected = Failure::new(
        &texts::commissioning_bounds_violated(),
        Cause::Settings,
        NextAction::Restore,
    );
    // Twenty seconds of scans, so the response covers the 20 s duration and the bounds are what is checked.
    let times: Vec<f64> = (0..20).map(f64::from).collect();
    let check = |samples: Vec<CommissioningSample>| {
        assess_commissioning_recording(&recording(samples), &criteria())
    };
    let mut high_output = steady(&times, 5.0, 2.0);
    high_output[3].applied_output = Some(25.0);
    assert_eq!(
        check(high_output),
        Err(expected.clone()),
        "the output leaves its range"
    );
    let mut high_measurement = steady(&times, 5.0, 2.0);
    high_measurement[3].process_value = Some(11.0);
    assert_eq!(
        check(high_measurement),
        Err(expected.clone()),
        "the measurement leaves its range"
    );
    let mut manual = steady(&times, 5.0, 2.0);
    manual[3].in_auto = false;
    assert_eq!(check(manual), Err(expected.clone()), "the loop leaves Auto");
    let mut moved_reference = steady(&times, 5.0, 2.0);
    moved_reference[3].setpoint = Some(5.5);
    assert_eq!(
        check(moved_reference),
        Err(expected.clone()),
        "the reference moves"
    );
    let mut missing = steady(&times, 5.0, 2.0);
    missing[3].process_value = None;
    assert_eq!(
        check(missing),
        Err(expected.clone()),
        "a sample without a measurement fails"
    );
    // Missing evidence never passes (B5.4), so a value that is not a number fails the range check too.
    let mut not_a_number = steady(&times, 5.0, 2.0);
    not_a_number[3].process_value = Some(f64::NAN);
    assert_eq!(
        check(not_a_number),
        Err(expected),
        "a value that is not a number fails"
    );
}

#[test]
fn the_error_is_taken_over_the_samples_after_the_settling_time_and_one_must_exist() {
    let times: Vec<f64> = (0..20).map(f64::from).collect();
    // The first five samples (before the 5 s settling time) are 4 units off; the later ones are on target.
    let mut early_error = steady(&times, 5.0, 2.0);
    for scan in &mut early_error[..5] {
        scan.process_value = Some(9.0);
    }
    assert_eq!(
        assess_commissioning_recording(&recording(early_error), &criteria()),
        Ok(0.0),
        "only the settled samples count"
    );
    let mut late = criteria();
    late.settling_time_seconds = 100.0;
    assert_eq!(
        assess_commissioning_recording(&recording(steady(&times, 5.0, 2.0)), &late),
        Err(Failure::new(
            &texts::commissioning_deadline_unrecorded(),
            Cause::Runtime,
            NextAction::None
        ))
    );
}

#[test]
fn an_error_above_its_limit_names_the_error_and_the_limit_and_one_within_it_passes() {
    let times: Vec<f64> = (0..20).map(f64::from).collect();
    let failure = assess_commissioning_recording(&recording(steady(&times, 5.5, 2.0)), &criteria())
        .expect_err("an error of 0.5 exceeds 0.1");
    assert_eq!(
        failure.message,
        texts::commissioning_error_exceeds(0.5, 0.1)
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::TaskInfeasible, NextAction::Restore)
    );
    let within = assess_commissioning_recording(&recording(steady(&times, 5.05, 2.0)), &criteria())
        .expect("an error of 0.05 is within 0.1");
    assert!((within - 0.05).abs() <= 1e-9, "the error is {within}");
}

#[test]
fn a_switching_recording_passes_when_both_levels_and_both_dwells_hold() {
    let error = assess_commissioning_recording(
        &switching_recording(&SWITCHING_PATTERN),
        &switching_criteria(),
    )
    .expect("the levels hold and every dwell is 3 s");
    assert!(error.abs() <= 1e-12, "the error is {error}");
}

#[test]
fn a_dwell_shorter_than_its_minimum_is_refused_with_its_requirement() {
    let mut reviewed = switching_criteria();
    reviewed.switching = Some(SwitchingCriteria {
        low: 0.0,
        high: 100.0,
        minimum_on_seconds: 4.0,
        minimum_off_seconds: 3.0,
    });
    let failure =
        assess_commissioning_recording(&switching_recording(&SWITCHING_PATTERN), &reviewed)
            .expect_err("the high dwells are 3 s, and 4 s is the minimum");
    assert!(
        failure
            .message
            .starts_with(&texts::commissioning_check_failed("switching-on-dwell", "")),
        "{}",
        failure.message
    );
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::TaskInfeasible, NextAction::Restore)
    );
}

#[test]
fn an_output_away_from_its_switching_levels_fails_the_level_check_first() {
    let applied = [
        0.0, 100.0, 50.0, 100.0, 0.0, 0.0, 0.0, 100.0, 100.0, 100.0, 0.0,
    ];
    let failure =
        assess_commissioning_recording(&switching_recording(&applied), &switching_criteria())
            .expect_err("the output sits at 50, which is not a level");
    assert!(
        failure
            .message
            .starts_with(&texts::commissioning_check_failed(
                "switching-output-levels",
                ""
            )),
        "{}",
        failure.message
    );
}

#[test]
fn a_sample_before_the_auto_target_counts_for_quality_but_not_for_the_bounds() {
    let times: Vec<f64> = (0..20).map(f64::from).collect();
    let mut before = steady(&times, 5.0, 2.0);
    before.insert(
        0,
        CommissioningSample {
            elapsed_seconds: -1.0,
            scan: 0,
            in_auto: false,
            setpoint: Some(4.0),
            process_value: Some(12.0),
            applied_output: Some(99.0),
            quality: ChannelQuality::Good,
        },
    );
    assert_eq!(
        assess_commissioning_recording(&recording(before.clone()), &criteria()),
        Ok(0.0),
        "the bounds apply from the target on"
    );
    before[0].quality = ChannelQuality::Bad;
    assert_eq!(
        assess_commissioning_recording(&recording(before), &criteria()),
        Err(Failure::new(
            &texts::commissioning_recording_incomplete(),
            Cause::Runtime,
            NextAction::None
        )),
        "the quality applies to every sample"
    );
}
