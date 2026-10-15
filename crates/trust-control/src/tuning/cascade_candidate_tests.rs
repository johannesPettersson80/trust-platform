//! Tests of the cascade's candidate and its parameter map (specification 35, C6 and C0): the two PIs held by their variables, the outer PI
//! read back from them, and the six gain parameters of both records in the order the seed names them.

#![allow(clippy::float_cmp)] // The gains are compared with the exact values the design wrote.

use super::*;
use crate::loop_definition::{ModeBinding, Parameter, ParameterBinding, RealType};
use crate::tuning::project_seed::CascadeGains;
use crate::tuning::settings_pins::GainKey;

const INNER_P: &str = "P1.InnerParameters.ProportionalGain";
const INNER_I: &str = "P1.InnerParameters.IntegralGain";
const INNER_D: &str = "P1.InnerParameters.DerivativeGain";
const OUTER_P: &str = "P1.OuterParameters.ProportionalGain";
const OUTER_I: &str = "P1.OuterParameters.IntegralGain";
const OUTER_D: &str = "P1.OuterParameters.DerivativeGain";

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

/// The two designed PIs of the test: the inner `Kp = 2`, `Ki = 0.5`, the outer `Kp = 0.25`, `Ki = 0.02`, and zero derivatives.
fn design() -> CascadeDesign {
    CascadeDesign {
        inner: ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.0,
        },
        outer: ParallelGains {
            proportional_gain: 0.25,
            integral_gain: 0.02,
            derivative_gain: 0.0,
        },
    }
}

fn parameter(symbol: &str) -> Parameter {
    Parameter {
        symbol: symbol.to_owned(),
        iec_type: RealType::Lreal,
        unit: "1".to_owned(),
        convention: "Control.PID gain".to_owned(),
        minimum: -1e6,
        maximum: 1e6,
    }
}

/// A parameter binding of the given symbols, with the inner controller's mode.
fn binding(symbols: &[&str]) -> ParameterBinding {
    ParameterBinding {
        loop_id: "loop:vessel-temperature".to_owned(),
        task_name: "MainTask".to_owned(),
        parameters: symbols.iter().map(|symbol| parameter(symbol)).collect(),
        mode: ModeBinding {
            symbol: "P1.InnerAuto".to_owned(),
            manual_value: false,
        },
    }
}

#[test]
fn the_candidate_holds_both_pis_by_their_variables_and_is_unapplied() {
    let candidate = cascade_candidate(&seed(), CandidateSource::ConstrainedComparison, design());
    assert!(!candidate.applied);
    assert_eq!(candidate.structure, Structure::PI);
    assert_eq!(
        candidate.gains,
        design().inner,
        "the candidate's gains are the inner PI"
    );
    assert_eq!(
        candidate.values.into_iter().collect::<Vec<_>>(),
        vec![
            (INNER_I.to_owned(), 0.5),
            (INNER_P.to_owned(), 2.0),
            (OUTER_I.to_owned(), 0.02),
            (OUTER_P.to_owned(), 0.25),
        ],
        "the derivative variables are not held: Apply writes them as zero"
    );
}

#[test]
fn the_outer_pi_reads_back_from_its_variables_with_a_zero_derivative() {
    let candidate = cascade_candidate(&seed(), CandidateSource::ConstrainedComparison, design());
    assert_eq!(
        cascade_outer_gains(&seed(), &candidate),
        Some(ParallelGains {
            proportional_gain: 0.25,
            integral_gain: 0.02,
            derivative_gain: 0.0,
        })
    );
    let mut missing = candidate;
    missing.values.remove(OUTER_I);
    assert_eq!(cascade_outer_gains(&seed(), &missing), None);
}

#[test]
fn the_loop_maps_the_six_gains_of_both_records_in_the_seed_order() {
    let symbols = [INNER_P, INNER_I, INNER_D, OUTER_P, OUTER_I, OUTER_D];
    let mapped = cascade_mapped_parameters(&seed(), &binding(&symbols));
    assert_eq!(
        mapped
            .iter()
            .map(|item| item.symbol.as_str())
            .collect::<Vec<_>>(),
        symbols.to_vec()
    );
    assert!(matches!(mapped[0].key, GainKey::ProportionalGain));
    assert!(matches!(mapped[1].key, GainKey::IntegralGain));
    assert!(matches!(mapped[2].key, GainKey::DerivativeGain));
    assert!(matches!(mapped[5].key, GainKey::DerivativeGain));
    assert!(mapped
        .iter()
        .all(|item| !item.real && item.bounds.maximum == 1e6));
}

#[test]
fn a_gain_the_binding_does_not_hold_maps_nothing() {
    let without_outer_derivative = [INNER_P, INNER_I, INNER_D, OUTER_P, OUTER_I];
    let mapped = cascade_mapped_parameters(&seed(), &binding(&without_outer_derivative));
    assert_eq!(mapped.len(), 5);
}
