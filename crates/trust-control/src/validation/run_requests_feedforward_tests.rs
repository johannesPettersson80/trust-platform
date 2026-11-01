//! The feedforward overrides of a trajectory-following candidate (specification 35, C11): its rate and acceleration gains are written to
//! the variables that lesson 90's seed names, and a candidate that lacks one of them is refused before the run.

#![allow(clippy::float_cmp)] // The values are the literals the candidates carry, compared exactly on purpose.

use std::collections::BTreeMap;

use super::*;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::structure::{ParallelGains, Structure};

/// Lesson 90 as the shipped file declares it: the drum axis, whose seed names `P1.RateGain` and `P1.AccelerationGain`.
fn lesson_90() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../examples/control/09-industrial-control-structures/90-trajectory-feedforward/control-loop.json"
    ))
    .expect("lesson 90 decodes")
}

/// A program candidate of the PID structure, holding the given variable values.
fn candidate(values: &[(&str, f64)]) -> LawCandidate {
    LawCandidate::Project(ProjectCandidate {
        applied: false,
        source: CandidateSource::Simc,
        structure: Structure::PID,
        gains: ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.25,
        },
        values: values
            .iter()
            .map(|(symbol, value)| ((*symbol).to_owned(), *value))
            .collect::<BTreeMap<_, _>>(),
    })
}

#[test]
fn a_trajectory_candidate_writes_its_feedforward_gains_to_the_variables_its_seed_names() {
    let loop_definition = lesson_90();
    let overrides = feedforward_overrides(
        &loop_definition,
        &candidate(&[
            ("P1.Parameters.ProportionalGain", 2.0),
            ("P1.RateGain", 1.0),
            ("P1.AccelerationGain", 0.75),
        ]),
    )
    .expect("both feedforward gains are written");
    let written: Vec<(&str, f64)> = overrides
        .iter()
        .map(|item| (item.symbol.as_str(), item.value))
        .collect();
    assert_eq!(
        written,
        vec![("P1.RateGain", 1.0), ("P1.AccelerationGain", 0.75)]
    );
}

#[test]
fn a_trajectory_candidate_without_a_feedforward_gain_is_refused_before_the_run() {
    let loop_definition = lesson_90();
    let refusal = feedforward_overrides(
        &loop_definition,
        &candidate(&[
            ("P1.Parameters.ProportionalGain", 2.0),
            ("P1.RateGain", 1.0),
        ]),
    )
    .expect_err("the acceleration gain is missing");
    assert_eq!(
        refusal.message,
        "A trajectory candidate holds no value for a feedforward gain."
    );
}
