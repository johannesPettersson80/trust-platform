// The overrides are compared as the request writes them, exactly.
#![allow(clippy::float_cmp)]

use std::collections::BTreeMap;

use serde_json::Value;

use super::*;
use crate::model::general::decode_general_model;
use crate::tuning::search::{SearchBounds, SearchCandidate, SearchTrial};
use crate::tuning::state_controller::design::{
    DesignStatus, SelectedModel, StateCandidate, StateControllerDesign,
};
use crate::tuning::state_controller::fixtures::two_tank_model;
use crate::tuning::state_controller::StateControllerFamily;

/// Lesson 68's control loop: the state-feedback law `K1, K2, N` on the two-tank cascade.
const LESSON_68_LOOP: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/control-loop.json"
);
/// Lesson 89's control loop: the finite search over the notch's two axes.
const LESSON_89_LOOP: &str = include_str!(
    "../../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/89-mechanical-resonance/control-loop.json"
);

/// A loop file decoded as the engine reads it.
fn loop_of(text: &str) -> LoopDefinition {
    LoopDefinition::decode(text).expect("the shipped loop file decodes")
}

/// The verification plan of lesson 68: one second per scan, the validation's two minutes, the process variable observed.
fn lesson_68_plan() -> VerificationPlan {
    VerificationPlan {
        step_ms: 1000,
        duration_ms: 120_000,
        max_total_cycles: 1000,
        record_every: 1,
        initial_values: BTreeMap::new(),
        observed_symbols: vec!["P1.ControlledTank2M".to_owned()],
        requirements: Vec::new(),
    }
}

/// The verification plan of lesson 89: a 20 ms scan, eight seconds of validation, the angle observed.
fn lesson_89_plan() -> VerificationPlan {
    VerificationPlan {
        step_ms: 20,
        duration_ms: 8000,
        max_total_cycles: 50_000,
        record_every: 1,
        initial_values: BTreeMap::new(),
        observed_symbols: vec!["P1.AngleRad".to_owned()],
        requirements: Vec::new(),
    }
}

/// The target-response case of a plan: the plan's own requirements, with no events, overrides or effects.
fn case_of(plan: &VerificationPlan) -> CampaignCase {
    CampaignCase {
        id: "target-response".to_owned(),
        plan: plan.clone(),
        initial_overrides: BTreeMap::new(),
        events: Vec::new(),
        effects: Vec::new(),
        requirements: plan.requirements.clone(),
    }
}

/// A design of a state-feedback law, as Tune stores it: its coefficient values by symbol.
fn state_design(parameters: BTreeMap<String, f64>) -> StateControllerDesign {
    StateControllerDesign {
        status: DesignStatus::Provisional,
        candidate: StateCandidate {
            applied: false,
            structure: StateControllerFamily::StateFeedback,
            parameters,
        },
        selected_model: SelectedModel {
            revision: "two-tank".to_owned(),
            provenance: "declared-provided-validated".to_owned(),
        },
        actual_st_verify: true,
        scope: String::new(),
        closed_loop_poles: Vec::new(),
        horizon: None,
    }
}

/// Lesson 68's design, as Tune writes it (the README's K1, K2 and N).
fn lesson_68_design() -> BTreeMap<String, f64> {
    BTreeMap::from([
        ("P1.K1".to_owned(), 0.0562),
        ("P1.K2".to_owned(), 0.0369),
        ("P1.N".to_owned(), 0.1131),
    ])
}

/// The request of one case under the settings, on the given plant (none: the configured plant).
fn request_of(
    loop_definition: &LoopDefinition,
    plan: &VerificationPlan,
    case: &CampaignCase,
    settings: RunSettings<'_>,
) -> Result<IsolatedRunRequest, Failure> {
    let model = two_tank_model();
    isolated_run_request(&RunRequestInput {
        loop_definition,
        model: &model,
        plan,
        case,
        settings,
        simulation: None,
        request_id: "run-law-1",
        expected_project_revision: "sha256:project",
    })
}

/// The parameter overrides of a request, as their symbols and values.
fn written(request: &IsolatedRunRequest) -> Vec<(String, f64)> {
    request
        .parameter_overrides
        .iter()
        .map(|parameter| (parameter.symbol.clone(), parameter.value))
        .collect()
}

#[test]
fn the_state_candidate_of_lesson_68_writes_k1_k2_and_n_through_the_parameter_record() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let plan = lesson_68_plan();
    let candidate = LawCandidate::State(Box::new(state_design(lesson_68_design())));
    let request = request_of(
        &loop_definition,
        &plan,
        &case_of(&plan),
        RunSettings::Candidate(&candidate),
    )
    .expect("the candidate's run request");
    assert_eq!(
        written(&request),
        vec![
            ("P1.K1".to_owned(), 0.0562),
            ("P1.K2".to_owned(), 0.0369),
            ("P1.N".to_owned(), 0.1131),
        ]
    );
}

#[test]
fn the_current_settings_of_lesson_68_write_the_gains_the_controller_holds() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let plan = lesson_68_plan();
    // After an Apply the controller holds other gains than the program was compiled with: the current run writes those.
    let observed = BTreeMap::from([
        ("P1.K1".to_owned(), 0.1),
        ("P1.K2".to_owned(), 0.2),
        ("P1.N".to_owned(), 0.3),
    ]);
    let request = request_of(
        &loop_definition,
        &plan,
        &case_of(&plan),
        RunSettings::Current(&observed),
    )
    .expect("the current run request");
    assert_eq!(
        written(&request),
        vec![
            ("P1.K1".to_owned(), 0.1),
            ("P1.K2".to_owned(), 0.2),
            ("P1.N".to_owned(), 0.3),
        ]
    );
}

#[test]
fn a_state_design_without_one_of_the_law_coefficients_is_refused_before_a_run() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let plan = lesson_68_plan();
    let mut partial = lesson_68_design();
    partial.remove("P1.N");
    let candidate = LawCandidate::State(Box::new(state_design(partial)));
    // A coefficient left out would keep the controller's own value for it, so the run is never built without it.
    let failure = request_of(
        &loop_definition,
        &plan,
        &case_of(&plan),
        RunSettings::Candidate(&candidate),
    )
    .expect_err("every coefficient of the law must be in the design");
    assert!(failure.message.contains("P1.N"), "{}", failure.message);
}

#[test]
fn the_search_candidate_of_lesson_89_writes_its_two_axes() {
    let loop_definition = loop_of(LESSON_89_LOOP);
    let plan = lesson_89_plan();
    let chosen = BTreeMap::from([
        ("P1.NotchFrequencyHz".to_owned(), 2.0),
        ("P1.NotchQuality".to_owned(), 3.0),
    ]);
    let search = SearchCandidate {
        parameters: chosen.clone(),
        selected: 0,
        trials: vec![SearchTrial {
            parameters: chosen,
            attempted: true,
            bounds: Some(SearchBounds::Met),
            score: Some(0.5),
            reason: None,
        }],
        score_unit: "rad²·s".to_owned(),
    };
    let candidate = LawCandidate::Search(Box::new(search));
    let request = request_of(
        &loop_definition,
        &plan,
        &case_of(&plan),
        RunSettings::Candidate(&candidate),
    )
    .expect("the chosen trial's run request");
    assert_eq!(
        written(&request),
        vec![
            ("P1.NotchFrequencyHz".to_owned(), 2.0),
            ("P1.NotchQuality".to_owned(), 3.0),
        ]
    );
}

#[test]
fn a_search_trial_that_leaves_a_bound_parameter_out_is_refused_before_a_run() {
    let loop_definition = loop_of(LESSON_89_LOOP);
    let plan = lesson_89_plan();
    let partial = BTreeMap::from([("P1.NotchFrequencyHz".to_owned(), 2.0)]);
    let search = SearchCandidate {
        parameters: partial.clone(),
        selected: 0,
        trials: vec![SearchTrial {
            parameters: partial,
            attempted: true,
            bounds: Some(SearchBounds::Met),
            score: Some(0.5),
            reason: None,
        }],
        score_unit: "rad²·s".to_owned(),
    };
    let candidate = LawCandidate::Search(Box::new(search));
    let failure = request_of(
        &loop_definition,
        &plan,
        &case_of(&plan),
        RunSettings::Candidate(&candidate),
    )
    .expect_err("the trial must write every bound parameter");
    assert!(
        failure.message.contains("P1.NotchQuality"),
        "{}",
        failure.message
    );
}

#[test]
fn the_current_settings_of_lesson_89_write_both_bound_parameters() {
    let loop_definition = loop_of(LESSON_89_LOOP);
    let plan = lesson_89_plan();
    let observed = BTreeMap::from([
        ("P1.NotchFrequencyHz".to_owned(), 0.0),
        ("P1.NotchQuality".to_owned(), 1.0),
    ]);
    let request = request_of(
        &loop_definition,
        &plan,
        &case_of(&plan),
        RunSettings::Current(&observed),
    )
    .expect("the current run request");
    assert_eq!(
        written(&request),
        vec![
            ("P1.NotchFrequencyHz".to_owned(), 0.0),
            ("P1.NotchQuality".to_owned(), 1.0),
        ]
    );
}

/// The decoded lesson 68 model file, which measures both tanks (the README's two-output model).
#[test]
fn lesson_68_supplied_model_decodes_with_two_measured_outputs() {
    let value: Value = serde_json::from_str(include_str!(
        "../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/provided-model.json"
    ))
    .expect("the model file is JSON");
    let model = decode_general_model(&value).expect("the model decodes");
    assert_eq!(model.identity().outputs.len(), 2);
}
