//! Tests of the cascade's parameter values (specification 35, C0 and C6): both PIs written to their own records with each derivative zero,
//! and the refusals of a candidate that lacks the outer PI, holds a derivative gain, names a value outside its binding, or has no binding.

#![allow(clippy::float_cmp)] // The written values are compared with the exact gains the candidate holds.

use std::collections::BTreeMap;

use super::*;
use crate::commissioning::adapters::ControllerGains;
use crate::loop_definition::{ModeBinding, Parameter, ParameterBinding, RealType};
use crate::tuning::project_seed::CascadeGains;
use crate::tuning::structure::ParallelGains;

const INNER_P: &str = "P1.InnerParameters.ProportionalGain";
const INNER_I: &str = "P1.InnerParameters.IntegralGain";
const INNER_D: &str = "P1.InnerParameters.DerivativeGain";
const OUTER_P: &str = "P1.OuterParameters.ProportionalGain";
const OUTER_I: &str = "P1.OuterParameters.IntegralGain";
const OUTER_D: &str = "P1.OuterParameters.DerivativeGain";
const ALL: [&str; 6] = [INNER_P, INNER_I, INNER_D, OUTER_P, OUTER_I, OUTER_D];

fn seed() -> CascadePidSeed {
    CascadePidSeed {
        inner: CascadeGains {
            instance: "P1.Inner".to_owned(),
            proportional_symbol: INNER_P.to_owned(),
            integral_symbol: INNER_I.to_owned(),
            derivative_symbol: INNER_D.to_owned(),
        },
        outer: CascadeGains {
            instance: "P1.Outer".to_owned(),
            proportional_symbol: OUTER_P.to_owned(),
            integral_symbol: OUTER_I.to_owned(),
            derivative_symbol: OUTER_D.to_owned(),
        },
    }
}

/// The binding of the six gains, each `LREAL` in `±10⁶`, with the inner controller's mode.
fn binding() -> ParameterBinding {
    ParameterBinding {
        loop_id: "loop:vessel-temperature".to_owned(),
        task_name: "MainTask".to_owned(),
        parameters: ALL
            .iter()
            .map(|symbol| Parameter {
                symbol: (*symbol).to_owned(),
                iec_type: RealType::Lreal,
                unit: "1".to_owned(),
                convention: "Control.PID gain".to_owned(),
                minimum: -1e6,
                maximum: 1e6,
            })
            .collect(),
        mode: ModeBinding {
            symbol: "P1.InnerAuto".to_owned(),
            manual_value: false,
        },
    }
}

/// The record's values before Apply: zero for each of the six gains.
fn observed() -> BTreeMap<String, f64> {
    ALL.iter()
        .map(|symbol| ((*symbol).to_owned(), 0.0))
        .collect()
}

/// The gains of a cascade candidate: the inner PI `Kp = 2`, `Ki = 0.5`, and the outer PI when it is given.
fn gains(outer: Option<ParallelGains>) -> ControllerGains {
    ControllerGains {
        proportional_gain: Some(2.0),
        integral_gain: Some(0.5),
        derivative_gain: Some(0.0),
        outer_pid: outer,
        ..ControllerGains::default()
    }
}

fn outer_pi() -> ParallelGains {
    ParallelGains {
        proportional_gain: 0.25,
        integral_gain: 0.02,
        derivative_gain: 0.0,
    }
}

#[test]
fn both_pis_are_written_to_their_records_with_each_derivative_zero() {
    let values = cascade_values(
        Some(&binding()),
        &seed(),
        &gains(Some(outer_pi())),
        &observed(),
    )
    .expect("both PIs are written");
    assert_eq!(values.get(INNER_P), Some(&2.0));
    assert_eq!(values.get(INNER_I), Some(&0.5));
    assert_eq!(values.get(INNER_D), Some(&0.0));
    assert_eq!(values.get(OUTER_P), Some(&0.25));
    assert_eq!(values.get(OUTER_I), Some(&0.02));
    assert_eq!(values.get(OUTER_D), Some(&0.0));
    assert_eq!(
        values.len(),
        6,
        "the record holds the six gains and no other value"
    );
}

#[test]
fn a_candidate_without_the_outer_pi_is_refused_with_its_next_action() {
    let refusal = cascade_values(Some(&binding()), &seed(), &gains(None), &observed())
        .expect_err("the outer PI is missing");
    assert_eq!(refusal.message, OUTER_PI_REQUIRED);
    assert_eq!(refusal.next_action, NextAction::Tune);
}

#[test]
fn a_nonzero_derivative_gain_is_refused_because_both_controllers_are_pi() {
    let mut with_derivative = gains(Some(outer_pi()));
    with_derivative.derivative_gain = Some(0.1);
    let refusal = cascade_values(Some(&binding()), &seed(), &with_derivative, &observed())
        .expect_err("a PI has no derivative term");
    assert_eq!(refusal.message, DERIVATIVE_NOT_HELD);
}

#[test]
fn a_gain_outside_its_binding_is_refused() {
    let too_large = ParallelGains {
        proportional_gain: 2e6,
        ..outer_pi()
    };
    assert!(cascade_values(
        Some(&binding()),
        &seed(),
        &gains(Some(too_large)),
        &observed()
    )
    .is_err());
}

#[test]
fn a_loop_without_a_parameter_binding_writes_nothing() {
    assert!(cascade_values(None, &seed(), &gains(Some(outer_pi())), &observed()).is_err());
}
