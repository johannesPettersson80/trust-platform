//! The overrides of a measured-disturbance candidate's isolated run (specification 35, C8): the candidate's `K_ff` with its lead and lag is
//! written to the variables that lesson 84's seed names, and its PI gains are written with them, each with the binding's type, unit and
//! bounds. The run is built on the loop and its model as the study holds them, with the loop's channels (C8 Validate).

#![allow(clippy::float_cmp)] // The values are the literals the candidate carries, compared exactly on purpose.

use std::collections::BTreeMap;

use super::*;
use crate::tuning::candidate::CandidateSource;
use crate::tuning::project_candidate::ProjectCandidate;
use crate::tuning::structure::{ParallelGains, Structure};
use crate::tuning::structures::fixtures::{heater_model, requirements};
use crate::validation::engineer_verification_campaign::engineer_verification_campaign;

/// Lesson 84 as the shipped file declares it: the heater with its draw-off fed forward through a lead and a lag.
fn lesson_84() -> LoopDefinition {
    LoopDefinition::decode(include_str!(
        "../../../../editors/vscode/media/examples/control/14-integrated-capstones/84-independent/control-loop.json"
    ))
    .expect("lesson 84 decodes")
}

/// The candidate of lesson 84's PI with its feedforward values: the gains 2, 0.5 and 0, `K_ff` 0.5, the lead 8 s and the lag 3 s.
fn candidate() -> LawCandidate {
    LawCandidate::Project(ProjectCandidate {
        applied: false,
        source: CandidateSource::ConstrainedComparison,
        structure: Structure::PI,
        gains: ParallelGains {
            proportional_gain: 2.0,
            integral_gain: 0.5,
            derivative_gain: 0.0,
        },
        values: BTreeMap::from([
            ("P1.Parameters.ProportionalGain".to_owned(), 2.0),
            ("P1.Parameters.IntegralGain".to_owned(), 0.5),
            ("P1.FeedforwardGain".to_owned(), 0.5),
            ("P1.LeadS".to_owned(), 8.0),
            ("P1.LagS".to_owned(), 3.0),
        ]),
    })
}

#[test]
fn a_feedforward_candidate_writes_its_gain_with_its_lead_and_lag_to_the_variables_its_seed_names() {
    let loop_definition = lesson_84();
    let overrides = feedforward_overrides(&loop_definition, &candidate())
        .expect("the feedforward values are written");
    let written: Vec<(&str, f64)> = overrides
        .iter()
        .map(|item| (item.symbol.as_str(), item.value))
        .collect();
    assert_eq!(
        written,
        vec![
            ("P1.FeedforwardGain", 0.5),
            ("P1.LeadS", 8.0),
            ("P1.LagS", 3.0)
        ]
    );
}

#[test]
fn a_feedforward_candidate_without_its_lag_is_refused_before_the_run() {
    let loop_definition = lesson_84();
    let mut without_lag = candidate();
    if let LawCandidate::Project(project) = &mut without_lag {
        project.values.remove("P1.LagS");
    }
    let refusal = feedforward_overrides(&loop_definition, &without_lag)
        .expect_err("the lag is missing from the candidate");
    assert_eq!(refusal.message, FEEDFORWARD_VALUE_MISSING);
}

#[test]
fn the_candidate_run_of_lesson_84_writes_the_pi_gains_and_the_feedforward_values_on_the_full_loop()
{
    let loop_definition = lesson_84();
    let model = heater_model();
    let observed = BTreeMap::from([
        ("P1.Parameters.ProportionalGain".to_owned(), 1.0),
        ("P1.Parameters.IntegralGain".to_owned(), 0.1),
        ("P1.Parameters.DerivativeGain".to_owned(), 0.0),
        ("P1.FeedforwardGain".to_owned(), 0.25),
        ("P1.LeadS".to_owned(), 4.0),
        ("P1.LagS".to_owned(), 2.0),
    ]);
    let task = requirements(1.0, 0.3, 0.1, 40.0, 80.0);
    // The campaign plans the target-response case on the loop's two-input model, through its channels (C8, Validate).
    let cases = engineer_verification_campaign(
        &loop_definition,
        &model,
        &task,
        &observed,
        &BTreeMap::new(),
    )
    .expect("the campaign is built");
    let case = cases.first().expect("the target-response case");
    let law = candidate();
    let request = isolated_run_request(&RunRequestInput {
        loop_definition: &loop_definition,
        model: &model,
        plan: &case.plan,
        case,
        settings: RunSettings::Candidate(&law),
        simulation: None,
        request_id: "job-candidate",
        expected_project_revision: "rev-1",
    })
    .expect("the candidate's run is requested");
    let written: BTreeMap<&str, f64> = request
        .parameter_overrides
        .iter()
        .map(|item| (item.symbol.as_str(), item.value))
        .collect();
    assert_eq!(
        written.len(),
        6,
        "the PI's three and the feedforward's three: {written:?}"
    );
    assert_eq!(written["P1.Parameters.ProportionalGain"], 2.0);
    assert_eq!(written["P1.Parameters.IntegralGain"], 0.5);
    assert_eq!(written["P1.Parameters.DerivativeGain"], 0.0);
    assert_eq!(written["P1.FeedforwardGain"], 0.5);
    assert_eq!(written["P1.LeadS"], 8.0);
    assert_eq!(written["P1.LagS"], 3.0);
}
