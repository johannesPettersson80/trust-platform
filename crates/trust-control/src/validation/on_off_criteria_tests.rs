#![allow(clippy::float_cmp)] // The recorded values are exact literals of the scans.

//! Tests of the switching criteria of Validate on recorded scans (C3): the transition rule, the retained band and the aborts, each passing
//! on a clean cycle and failing on the scan that breaks it, the missing evidence that is never a pass, the threshold policy, and the
//! direction of a direct action. The loop is the cold room of the design tests: a reverse action, a half-band of 0.5 degC around 10 degC.

use std::collections::BTreeMap;

use super::{switching_results, ABORT_ID, BAND_ID, TRANSITION_ID};
use crate::identification::connected_record::{
    RunConditions, ValidationChannel, ValidationRun, ValidationSample, ValidationValue,
};
use crate::identification::independent_validation::RequirementStatus;
use crate::loop_definition::{Action, LoopDefinition, ThresholdEquality};
use crate::recording::channels::ChannelQuality;
use crate::study::tune::design::on_off::tests::{actuator, cold_room};
use crate::validation::requirement_record::RunRequirementResult;

/// The cold room's settings of a run: the hysteresis of 0.5 degC, and the two minimum times.
fn settings() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.HysteresisC".to_owned(), 0.5),
        ("P1.MinimumOnS".to_owned(), 20.0),
        ("P1.MinimumOffS".to_owned(), 20.0),
    ])
}

/// A scan at `time`: the latch, the applied output (0 or 100 %), the measurement and the setpoint, each good. The latch is left out when
/// `latch` is `None`, which makes the scan's evidence missing.
fn scan(time: f64, latch: Option<f64>, applied: f64, process: f64) -> ValidationSample {
    let good = |value: f64| ValidationValue {
        value: Some(value),
        quality: ChannelQuality::Good,
    };
    let mut values = BTreeMap::from([
        ("P1.CompressorPct".to_owned(), good(applied)),
        ("P1.RoomC".to_owned(), good(process)),
        ("P1.SetpointC".to_owned(), good(10.0)),
    ]);
    if let Some(latch) = latch {
        values.insert("P1.CoolingOn".to_owned(), good(latch));
    }
    ValidationSample {
        time_seconds: time,
        sequence: 0,
        values,
    }
}

/// The recorded run of the scans, one second apart.
fn run_of(samples: Vec<ValidationSample>) -> ValidationRun {
    let times: Vec<f64> = samples.iter().map(|sample| sample.time_seconds).collect();
    ValidationRun {
        id: "switching-criteria".to_owned(),
        evidence: crate::identification::connected_record::RunEvidence::PhysicalRecorded,
        parameters: settings(),
        conditions: RunConditions {
            initial_state: BTreeMap::new(),
            events: Vec::new(),
            duration_seconds: 10.0,
            sample_times_seconds: times,
            seed: 1,
            model_revision: String::new(),
            program_revision: String::new(),
            parameter_schema_revision: String::new(),
        },
        channels: vec![ValidationChannel {
            id: "P1.CompressorPct".to_owned(),
            unit: "%".to_owned(),
        }],
        samples,
    }
}

/// The rows of the run's switching criteria, as (identity, status) pairs in their order.
fn verdicts(
    loop_definition: &LoopDefinition,
    samples: Vec<ValidationSample>,
) -> Vec<(String, RequirementStatus)> {
    let block = loop_definition
        .controller
        .on_off
        .as_ref()
        .expect("the cold room is a switching controller");
    switching_results(loop_definition, block, &settings(), &run_of(samples))
        .iter()
        .map(|row: &RunRequirementResult| (row.id.clone(), row.status))
        .collect()
}

/// The verdicts of a clean reverse cycle: the request turns ON at 10.6 degC and OFF at 9.4 degC, each after the output has been
/// requested; the output follows its request after the dwell, and the request holds inside the band in between.
fn clean_cycle() -> Vec<ValidationSample> {
    vec![
        scan(0.0, Some(0.0), 0.0, 10.0),
        scan(1.0, Some(0.0), 0.0, 10.2),
        scan(2.0, Some(1.0), 0.0, 10.6),
        scan(3.0, Some(1.0), 100.0, 10.8),
        scan(4.0, Some(1.0), 100.0, 10.4),
        scan(5.0, Some(0.0), 100.0, 9.4),
        scan(6.0, Some(0.0), 0.0, 9.2),
    ]
}

/// The whole result of the three rows for a status set: the transition rule, the retained band and the aborts, in that order.
fn all_pass() -> Vec<(String, RequirementStatus)> {
    vec![
        (TRANSITION_ID.to_owned(), RequirementStatus::Pass),
        (BAND_ID.to_owned(), RequirementStatus::Pass),
        (ABORT_ID.to_owned(), RequirementStatus::Pass),
    ]
}

#[test]
fn a_clean_cycle_passes_the_transition_rule_the_retained_band_and_the_abort_row() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    assert_eq!(verdicts(&loop_definition, clean_cycle()), all_pass());
}

#[test]
fn a_request_turned_on_inside_the_band_fails_the_transition_rule_at_that_scan() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    let mut samples = clean_cycle();
    // The request turns ON at 10.3 degC, inside the band (the ON threshold is 10.5 degC).
    samples[2] = scan(2.0, Some(1.0), 0.0, 10.3);
    let verdicts = verdicts(&loop_definition, samples);
    assert_eq!(
        verdicts[0],
        (TRANSITION_ID.to_owned(), RequirementStatus::Fail)
    );
    assert_eq!(
        verdicts[1..],
        all_pass()[1..],
        "the band and the aborts hold"
    );
}

#[test]
fn a_request_held_past_its_other_threshold_fails_the_retained_band() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    let mut samples = clean_cycle();
    // The request stays ON at 9.4 degC, past the OFF threshold of 9.5 degC: it should have turned OFF at that scan.
    samples[5] = scan(5.0, Some(1.0), 100.0, 9.4);
    samples[6] = scan(6.0, Some(1.0), 100.0, 9.2);
    let verdicts = verdicts(&loop_definition, samples);
    assert_eq!(
        verdicts[0],
        (TRANSITION_ID.to_owned(), RequirementStatus::Pass),
        "no request changed without its threshold"
    );
    assert_eq!(verdicts[1], (BAND_ID.to_owned(), RequirementStatus::Fail));
    assert_eq!(verdicts[2], (ABORT_ID.to_owned(), RequirementStatus::Pass));
}

#[test]
fn an_applied_change_that_its_request_does_not_hold_is_an_abort_and_fails_the_abort_row() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    let mut samples = clean_cycle();
    // The output switches ON at 1 s while the request is OFF: an abort of the output, never a normal completion.
    samples[1] = scan(1.0, Some(0.0), 100.0, 10.2);
    let verdicts = verdicts(&loop_definition, samples);
    assert_eq!(verdicts[2], (ABORT_ID.to_owned(), RequirementStatus::Fail));
}

#[test]
fn a_scan_without_the_requested_state_is_inconclusive_and_never_a_pass() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    let mut samples = clean_cycle();
    samples[3] = scan(3.0, None, 100.0, 10.8);
    let verdicts = verdicts(&loop_definition, samples);
    assert!(
        verdicts
            .iter()
            .all(|(_, status)| *status == RequirementStatus::Inconclusive),
        "every row is inconclusive without the evidence: {verdicts:?}"
    );
}

#[test]
fn the_strict_policy_needs_the_measurement_beyond_the_threshold() {
    let mut loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    // The request turns ON at exactly 10.5 degC, the ON threshold: the default policy accepts it.
    let mut samples = clean_cycle();
    samples[2] = scan(2.0, Some(1.0), 0.0, 10.5);
    assert_eq!(
        verdicts(&loop_definition, samples.clone())[0].1,
        RequirementStatus::Pass
    );
    if let Some(block) = loop_definition.controller.on_off.as_mut() {
        block.threshold_equality = ThresholdEquality::Beyond;
    }
    assert_eq!(
        verdicts(&loop_definition, samples)[0].1,
        RequirementStatus::Fail,
        "strictly beyond is needed"
    );
}

#[test]
fn a_direct_action_requests_on_below_the_setpoint_and_off_above_it() {
    let mut loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    loop_definition.controller.action = Action::Direct;
    let samples = vec![
        scan(0.0, Some(0.0), 0.0, 10.0),
        scan(1.0, Some(1.0), 0.0, 9.5),
        scan(2.0, Some(1.0), 100.0, 9.6),
        scan(3.0, Some(0.0), 100.0, 10.5),
        scan(4.0, Some(0.0), 0.0, 10.4),
    ];
    assert_eq!(verdicts(&loop_definition, samples), all_pass());
}
