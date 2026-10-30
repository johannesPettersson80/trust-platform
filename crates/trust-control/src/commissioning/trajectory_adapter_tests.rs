//! The values that Apply writes for a trajectory-following law (specification 35, C11): the three PID gains and the two feedforward gains,
//! each to its own variable, through the project adapter; and a candidate without both feedforward gains is refused.

#![allow(clippy::float_cmp)] // The values are the literals the gains carry, compared exactly on purpose.

use std::collections::BTreeMap;

use super::*;
use crate::loop_definition::ProjectSeedBlock;
use crate::tuning::project_seed::ParallelPidSeed;
use crate::tuning::structure::Structure;

/// Lesson 90 as the shipped file declares it: the drum axis, whose seed names five variables.
fn lesson_90() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control/09-industrial-control-structures/90-trajectory-feedforward/control-loop.json"
    ))
    .expect("lesson 90 decodes")
}

/// The gains of a PID candidate with its two feedforward gains, or without them.
fn gains(feedforward: Option<(f64, f64)>) -> ControllerGains {
    ControllerGains {
        proportional_gain: Some(2.0),
        integral_gain: Some(0.5),
        derivative_gain: Some(0.25),
        rate_gain: feedforward.map(|(rate, _)| rate),
        acceleration_gain: feedforward.map(|(_, acceleration)| acceleration),
        ..ControllerGains::default()
    }
}

#[test]
fn a_trajectory_law_writes_its_pid_gains_and_both_feedforward_gains_through_the_project_adapter() {
    let loop_definition = lesson_90();
    let adapter = controller_adapter(&loop_definition).expect("the engine serves lesson 90's law");
    assert!(
        matches!(adapter.kind(), AdapterKind::ProjectPid { .. }),
        "the project adapter applies a trajectory law"
    );
    let values = adapter
        .materialize(&gains(Some((1.0, 0.75))), &BTreeMap::new())
        .expect("the five values are within their bounds");
    assert_eq!(
        values,
        BTreeMap::from([
            ("P1.Parameters.ProportionalGain".to_owned(), 2.0),
            ("P1.Parameters.IntegralGain".to_owned(), 0.5),
            ("P1.Parameters.DerivativeGain".to_owned(), 0.25),
            ("P1.RateGain".to_owned(), 1.0),
            ("P1.AccelerationGain".to_owned(), 0.75),
        ])
    );
}

#[test]
fn a_trajectory_candidate_without_its_feedforward_gains_is_refused_by_the_adapter() {
    let loop_definition = lesson_90();
    let adapter = controller_adapter(&loop_definition).expect("the engine serves lesson 90's law");
    let refusal = adapter
        .materialize(&gains(None), &BTreeMap::new())
        .expect_err("the feedforward gains are required");
    assert_eq!(full_text(&refusal), texts::project_feedforward_required());
}

#[test]
fn a_parallel_pid_law_refuses_a_nonzero_feedforward_gain_it_has_no_variable_for() {
    // The same law with a parallel-pid seed: its PID variables only, so a rate gain has nowhere to be written.
    let mut loop_definition = lesson_90();
    if let Some(project) = loop_definition.controller.project.as_mut() {
        project.seed = Some(ProjectSeedBlock::ParallelPid(ParallelPidSeed {
            family: Structure::PID,
            proportional_symbol: "P1.Parameters.ProportionalGain".to_owned(),
            integral_symbol: Some("P1.Parameters.IntegralGain".to_owned()),
            derivative_symbol: Some("P1.Parameters.DerivativeGain".to_owned()),
        }));
    }
    let adapter = controller_adapter(&loop_definition).expect("the parallel law is served");
    assert!(adapter
        .materialize(&gains(Some((1.0, 0.0))), &BTreeMap::new())
        .is_err());
    assert!(adapter
        .materialize(&gains(Some((0.0, 0.0))), &BTreeMap::new())
        .is_ok());
}
