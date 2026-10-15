#![allow(clippy::float_cmp)] // The settings and limits are exact literals of the cold room fixture.

//! Tests of the switching controller's commissioning write set (C3, B6.1): the three settings written through their bound parameters over
//! the observed record, the refusals of a setting outside its binding or missing, the candidate's gains by symbol, and the switching
//! limits that a run is judged by. The loop is the cold room of the design tests.

use std::collections::BTreeMap;

use super::{candidate_gains, materialize, switching_criteria, with_switching};
use crate::commissioning::adapters::ControllerGains;
use crate::loop_definition::Range;
use crate::study::tune::design::on_off::tests::{actuator, cold_room};
use crate::tuning::on_off_candidate::OnOffCandidate;
use crate::validation::commissioning_assessment::CommissioningCriteria;

/// The three settings of the cold room as gains: the hysteresis, and the two minimum times.
fn settings(hysteresis: f64, on: f64, off: f64) -> ControllerGains {
    ControllerGains {
        on_off_hysteresis: Some(hysteresis),
        on_off_minimum_on_seconds: Some(on),
        on_off_minimum_off_seconds: Some(off),
        ..ControllerGains::default()
    }
}

#[test]
fn the_write_set_holds_the_three_settings_over_the_observed_record() {
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let observed = BTreeMap::from([("P1.Other".to_owned(), 7.0)]);
    let written = materialize(&loop_definition, &settings(0.5, 20.0, 30.0), &observed)
        .expect("the settings are inside their bindings");
    assert_eq!(written.get("P1.HysteresisC"), Some(&0.5));
    assert_eq!(written.get("P1.MinimumOnS"), Some(&20.0));
    assert_eq!(written.get("P1.MinimumOffS"), Some(&30.0));
    assert_eq!(
        written.get("P1.Other"),
        Some(&7.0),
        "the rest of the record is kept"
    );
}

#[test]
fn a_setting_outside_its_binding_is_refused_and_a_missing_one_is_named() {
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    // The hysteresis is bound to 0 to 30 degC, so 31 degC is refused.
    let outside = materialize(
        &loop_definition,
        &settings(31.0, 20.0, 30.0),
        &BTreeMap::new(),
    )
    .expect_err("31 degC is above the binding");
    assert!(
        outside.message.contains("P1.HysteresisC"),
        "{}",
        outside.message
    );
    let missing = ControllerGains {
        on_off_hysteresis: Some(0.5),
        on_off_minimum_on_seconds: Some(20.0),
        ..ControllerGains::default()
    };
    let failure = materialize(&loop_definition, &missing, &BTreeMap::new())
        .expect_err("the OFF time has no value");
    assert!(
        failure.message.contains("P1.MinimumOffS"),
        "{}",
        failure.message
    );
}

#[test]
fn the_candidate_gains_carry_its_settings_by_their_symbols() {
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let candidate = OnOffCandidate {
        applied: false,
        values: BTreeMap::from([
            ("P1.HysteresisC".to_owned(), 0.5),
            ("P1.MinimumOnS".to_owned(), 20.0),
            ("P1.MinimumOffS".to_owned(), 30.0),
        ]),
    };
    assert_eq!(
        candidate_gains(&loop_definition, &candidate),
        settings(0.5, 20.0, 30.0)
    );
}

#[test]
fn a_binary_output_has_no_band_so_its_auto_entry_allowance_is_zero() {
    // C3: the first output is the applied level exactly, whatever the noise and the task's own tolerance say.
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let task = crate::commissioning::fixture_loops::requirements();
    let allowance = crate::commissioning::entry_allowance::auto_entry_tolerance(
        &loop_definition,
        &task,
        &BTreeMap::new(),
        Some(0.3),
        None,
    )
    .expect("the switching allowance");
    assert_eq!(allowance, 0.0);
}

#[test]
fn the_switching_limits_are_the_levels_and_the_reviewed_minimum_times() {
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let reviewed = BTreeMap::from([
        ("P1.MinimumOnS".to_owned(), 20.0),
        ("P1.MinimumOffS".to_owned(), 30.0),
    ]);
    let limits = switching_criteria(&loop_definition, &reviewed)
        .expect("the limits")
        .expect("a switching loop has limits");
    assert_eq!((limits.low, limits.high), (0.0, 100.0));
    assert_eq!(
        (limits.minimum_on_seconds, limits.minimum_off_seconds),
        (20.0, 30.0)
    );
    let criteria = CommissioningCriteria {
        target: 12.0,
        duration_seconds: 300.0,
        settling_time_seconds: 90.0,
        maximum_error: 2.0,
        task_period_ms: 100.0,
        process_range: Range {
            minimum: -10.0,
            maximum: 20.0,
        },
        output_limits: Range {
            minimum: 0.0,
            maximum: 100.0,
        },
        switching: None,
    };
    let with = with_switching(&criteria, &loop_definition, &reviewed).expect("the criteria");
    assert_eq!(
        with.switching.map(|limits| limits.minimum_off_seconds),
        Some(30.0),
        "the run's criteria carry the switching limits"
    );
}
