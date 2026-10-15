#![allow(clippy::float_cmp)] // The expected levels and settings are exact literals of the fixture.

//! Tests of the switching controller's Validate requests (C3, B5.3): the requirements of its two levels and its dwell times, the
//! actuator's own checks when it declares a minimum time, the target case without its dwell checks, and the overrides that write the
//! three settings by symbol. The loop is the cold room of the design tests.

use std::collections::BTreeMap;

use super::{actuator_requirements, overrides, switching_requirements, without_dwell};
use crate::model::simulation::{ActuatorCapabilities, ActuatorKind};
use crate::study::tune::design::on_off::tests::{actuator, cold_room};
use crate::validation::run_validation::{DwellState, RunCheck, Window};

/// The window of the run, from its start to two minutes.
const WINDOW: Window = Window {
    start: 0.0,
    end: 120.0,
};

/// The settings of the cold room's run: the hysteresis, and the two minimum times the test sets.
fn settings(on: f64, off: f64) -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.HysteresisC".to_owned(), 0.5),
        ("P1.MinimumOnS".to_owned(), on),
        ("P1.MinimumOffS".to_owned(), off),
    ])
}

#[test]
fn the_switching_requirements_check_both_levels_and_each_dwell_at_the_midpoint() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    let requirements = switching_requirements(&loop_definition, &settings(20.0, 30.0), WINDOW)
        .expect("the requirements of the cold room");
    let ids: Vec<&str> = requirements.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "switching-output-levels",
            "switching-on-dwell",
            "switching-off-dwell"
        ]
    );
    assert_eq!(
        requirements[0].check,
        RunCheck::Levels {
            allowed: vec![0.0, 100.0],
            tolerance: 0.0
        }
    );
    assert_eq!(
        requirements[1].check,
        RunCheck::Dwell {
            threshold: 50.0,
            state: DwellState::High,
            minimum_seconds: 20.0
        }
    );
    assert_eq!(
        requirements[2].check,
        RunCheck::Dwell {
            threshold: 50.0,
            state: DwellState::Low,
            minimum_seconds: 30.0
        }
    );
}

#[test]
fn the_target_case_drops_the_dwell_checks_and_keeps_the_levels() {
    let loop_definition = cold_room(&actuator(Some((20.0, 20.0))), "half-band");
    let requirements = switching_requirements(&loop_definition, &settings(20.0, 20.0), WINDOW)
        .expect("the requirements");
    let kept = without_dwell(&requirements);
    let target: Vec<&str> = kept.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(target, ["switching-output-levels"]);
}

#[test]
fn an_actuator_checks_its_levels_and_only_the_minimum_times_it_declares() {
    let declared = ActuatorCapabilities {
        kind: ActuatorKind::OnOff,
        minimum: 0.0,
        maximum: 100.0,
        rate_up_per_s: None,
        rate_down_per_s: None,
        minimum_on_seconds: Some(20.0),
        minimum_off_seconds: Some(0.0),
    };
    let requirements = actuator_requirements(&declared, "P1.CompressorPct", "", WINDOW);
    let ids: Vec<&str> = requirements.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        ["actuator-levels", "actuator-high-dwell"],
        "a declared zero adds no dwell check"
    );
}

#[test]
fn the_settings_are_written_by_symbol_with_the_bound_parameters_types_and_bounds() {
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let written = overrides(&loop_definition, &settings(20.0, 30.0)).expect("the overrides");
    let pairs: Vec<(&str, f64)> = written
        .iter()
        .map(|item| (item.symbol.as_str(), item.value))
        .collect();
    assert_eq!(
        pairs,
        [
            ("P1.HysteresisC", 0.5),
            ("P1.MinimumOnS", 20.0),
            ("P1.MinimumOffS", 30.0)
        ]
    );
    assert_eq!(written[1].unit, "s", "a minimum time is written in seconds");
}

#[test]
fn a_setting_without_a_value_is_refused_before_a_run_is_built() {
    let loop_definition = cold_room(&actuator(Some((20.0, 30.0))), "half-band");
    let mut missing = settings(20.0, 30.0);
    missing.remove("P1.MinimumOffS");
    let failure = overrides(&loop_definition, &missing).expect_err("the OFF time has no value");
    assert!(
        failure.message.contains("P1.MinimumOffS"),
        "{}",
        failure.message
    );
}
