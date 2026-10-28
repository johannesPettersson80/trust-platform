//! Tests of the Smith predictor's Apply (specification 35, C9 and D-18): the write set of a candidate on a loop that names the PI and the
//! model's three values, the refusal of a candidate without them, and the operating point that the library block's variables take.

#![allow(clippy::float_cmp)] // The written values are the terms as given, so they are compared exactly.

use std::collections::BTreeMap;

use super::super::adapters::{controller_adapter, ControllerGains};
use super::super::fixture_loops::smith_predictor_loop;
use super::super::project_law::project_gains;
use super::*;
use crate::failure::Cause;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::project_candidate::{served_smith, smith_candidate};
use crate::tuning::project_seed::{ParallelPidSeed, SmithLibraryBlock, SmithPidSeed};
use crate::tuning::structure::{ParallelGains, Structure};

/// The PI of the test's candidate: `Kp = 2`, `Ki = 0.2`, `Kd = 0`.
fn pi() -> ParallelGains {
    ParallelGains {
        proportional_gain: 2.0,
        integral_gain: 0.2,
        derivative_gain: 0.0,
    }
}

#[test]
fn apply_writes_the_pi_and_the_model_values_in_one_write_set() {
    let loop_definition = smith_predictor_loop();
    let seed = served_smith(&loop_definition)
        .expect("the loop declares a Smith predictor seed")
        .clone();
    let terms = SmithTerms {
        gain: 0.2,
        time_constant_seconds: 5.0,
        delay_seconds: 1.0,
        operating: (0.0, 0.0),
    };
    let candidate = smith_candidate(&seed, CandidateSource::ConstrainedComparison, pi(), terms);
    let gains = project_gains(&loop_definition, &candidate);
    let adapter = controller_adapter(&loop_definition).expect("the engine serves the predictor");
    let values = adapter
        .materialize(&gains, &BTreeMap::new())
        .expect("the write set holds the PI and the model values");
    assert_eq!(
        values,
        BTreeMap::from([
            ("Main.Program.Kp".to_owned(), 2.0),
            ("Main.Program.Ki".to_owned(), 0.2),
            ("Main.Program.Kd".to_owned(), 0.0),
            ("Main.Program.ModelGain".to_owned(), 0.2),
            ("Main.Program.ModelTau".to_owned(), 5.0),
            ("Main.Program.ModelDelay".to_owned(), 1.0),
        ])
    );
}

#[test]
fn apply_refuses_a_candidate_that_holds_no_model_values() {
    let loop_definition = smith_predictor_loop();
    let adapter = controller_adapter(&loop_definition).expect("the engine serves the predictor");
    let gains = ControllerGains {
        proportional_gain: Some(2.0),
        integral_gain: Some(0.2),
        derivative_gain: Some(0.0),
        ..ControllerGains::default()
    };
    let refusal = adapter
        .materialize(&gains, &BTreeMap::new())
        .expect_err("a Smith predictor writes its model values with its PI");
    assert_eq!(refusal.cause, Cause::Settings);
    assert_eq!(refusal.next_action, NextAction::Tune);
    assert_eq!(
        refusal.message,
        "A Smith predictor's candidate holds no model values, so Apply has nothing to write for the predictor."
    );
}

#[test]
fn the_library_block_takes_the_models_operating_point_after_the_model_values() {
    let seed = SmithPidSeed {
        pid: ParallelPidSeed {
            family: Structure::PI,
            proportional_symbol: "P1.Kp".to_owned(),
            integral_symbol: Some("P1.Ki".to_owned()),
            derivative_symbol: Some("P1.Kd".to_owned()),
        },
        gain_symbol: "P1.K".to_owned(),
        time_constant_symbol: "P1.T".to_owned(),
        delay_symbol: "P1.Theta".to_owned(),
        library_block: Some(SmithLibraryBlock {
            output_symbol: "P1.Out".to_owned(),
            maximum_correction_symbol: "P1.MaxC".to_owned(),
            operating_input_symbol: "P1.U0".to_owned(),
            operating_output_symbol: "P1.Y0".to_owned(),
        }),
    };
    let terms = SmithTerms {
        gain: 0.2,
        time_constant_seconds: 5.0,
        delay_seconds: 1.0,
        operating: (75.0, 35.0),
    };
    let written = smith_terms(&seed, Some(&terms)).expect("the model values are written");
    assert_eq!(
        written,
        vec![
            ("P1.K", 0.2),
            ("P1.T", 5.0),
            ("P1.Theta", 1.0),
            ("P1.U0", 75.0),
            ("P1.Y0", 35.0),
        ]
    );
}

#[test]
fn without_the_library_block_the_operating_point_is_not_written() {
    let seed = SmithPidSeed {
        pid: ParallelPidSeed {
            family: Structure::PI,
            proportional_symbol: "P1.Kp".to_owned(),
            integral_symbol: Some("P1.Ki".to_owned()),
            derivative_symbol: Some("P1.Kd".to_owned()),
        },
        gain_symbol: "P1.K".to_owned(),
        time_constant_symbol: "P1.T".to_owned(),
        delay_symbol: "P1.Theta".to_owned(),
        library_block: None,
    };
    let terms = SmithTerms {
        gain: 0.2,
        time_constant_seconds: 5.0,
        delay_seconds: 1.0,
        operating: (75.0, 35.0),
    };
    let written = smith_terms(&seed, Some(&terms)).expect("the model values are written");
    assert_eq!(written.len(), 3);
}
