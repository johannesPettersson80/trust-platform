//! The state-feedback Tune of the course's lessons 68, 69 and 72 (specification 35, C4). Each lesson's shipped control loop and model
//! file, with the inputs its README gives, designs the gains, poles and stages that the README shows. The expected values are the
//! exact zero-order-hold sampling of the lesson's model with the design rules, worked out with the standard library: lesson 68's
//! gains by Ackermann's formula, lesson 69's stages by the backward Riccati recursion, and lesson 72's estimator gains by the
//! covariance iteration with the stopping rule of the TypeScript (`steadyStateKalman`). The README prints them to three or four
//! digits, and they agree with it.

use serde_json::{json, Value};
use trust_control_math::margins::MarginValue;

use crate::failure::Failure;
use crate::identification::identify_result::kept_candidate;
use crate::loop_definition::LoopDefinition;
use crate::messages::{TuneMethod, TuneRequest};
use crate::model::general::decode_general_model;
use crate::model::GeneralModel;
use crate::setup::discovery::{adapter_compatibility, ControllerCompatibility};
use crate::study::current_model::{CurrentModel, CurrentModelSource};
use crate::study::tune::state::{run, state_fields, StateDesignInput};
use crate::study::tune::{TuneInputs, TuneOutcome};
use crate::study::StudySection;
use crate::tuning::candidate::LawCandidate;
use crate::tuning::requirements::TuningRequirements;
use crate::tuning::state_controller::offer::offer;
use crate::tuning::state_controller::preferences::{form_fields, FormField, StateDesignDraft};
use crate::tuning::state_controller::{state_controller_poles, StateControllerFamily};

/// Lesson 68's control loop and its supplied model: the two-tank cascade with both levels measured.
const LESSON_68_LOOP: &str = include_str!("../../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/control-loop.json");
const LESSON_68_MODEL: &str = include_str!("../../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/provided-model.json");
/// Lesson 69's control loop: the same cascade under the LQR law. Its supplied model is lesson 68's.
const LESSON_69_LOOP: &str = include_str!("../../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/69-lqr/control-loop.json");
/// Lesson 72's control loop and its one-second model, with only tank 2 measured.
const LESSON_72_LOOP: &str = include_str!("../../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/72-lqg/control-loop.json");
const LESSON_72_MODEL: &str = include_str!("../../../../../editors/vscode/media/examples/control/11-state-space-estimation-and-predictive-control/72-lqg/lqg-model.json");

/// The loop of a lesson file, decoded as the engine reads it.
fn loop_of(text: &str) -> LoopDefinition {
    LoopDefinition::decode(text).expect("the shipped loop file decodes")
}

/// The model of a lesson file, decoded as the engine reads a supplied model.
fn model_of(text: &str) -> GeneralModel {
    let value: Value = serde_json::from_str(text).expect("the shipped model file is JSON");
    decode_general_model(&value).expect("the shipped model file decodes")
}

/// The study's current model when it is supplied (B3.7): a model with no Identify section.
fn supplied(model: GeneralModel) -> CurrentModel {
    CurrentModel {
        source: CurrentModelSource::Supplied,
        model,
        identify: None,
    }
}

/// The request of a state-space Tune: no structure, no pins and no settings, on the lesson's runtime.
fn request() -> TuneRequest {
    TuneRequest {
        project_root: String::new(),
        runtime_id: "simulator".to_owned(),
        loop_id: "control-state-feedback".to_owned(),
        storage_folder: String::new(),
        method: Some(TuneMethod::StateSpace),
        choice: None,
        structure: None,
        pins: None,
        settings: None,
        connection: None,
    }
}

/// The requirements that the lessons' READMEs enter: a 10 mm setpoint change, the settling time and the validation duration.
fn requirements(settling_time_seconds: f64, duration_seconds: f64) -> TuningRequirements {
    serde_json::from_value(serde_json::json!({
        "referenceChange": 0.01,
        "maximumError": 0.002,
        "settlingBand": 0.001,
        "settlingTimeSeconds": settling_time_seconds,
        "durationSeconds": duration_seconds,
    }))
    .expect("the README's requirements form a record")
}

/// The text of one field of the design form.
fn text(value: &str) -> String {
    value.to_owned()
}

/// Lesson 69's form: the LQR weights, the fixed-policy finite horizon of 20 steps and its terminal weight of 400 I.
fn lqr_state(input_weight: &str) -> StateDesignInput {
    StateDesignInput {
        draft: Some(StateDesignDraft {
            state_weights: Some(text("[[400, 0], [0, 400]]")),
            input_weights: Some(text(input_weight)),
            solver_mode: Some(text("finite")),
            horizon_steps: Some(text("20")),
            terminal_weights: Some(text("[[400, 0], [0, 400]]")),
            ..StateDesignDraft::default()
        }),
        ..StateDesignInput::default()
    }
}

/// Lesson 72's form: the LQR weights of lesson 69, the Kalman covariances with tank 2 measured, and the horizon.
fn lqg_state(process_noise: &str) -> StateDesignInput {
    StateDesignInput {
        draft: Some(StateDesignDraft {
            state_weights: Some(text("[[400, 0], [0, 400]]")),
            input_weights: Some(text("[[40000]]")),
            process_noise: Some(text(process_noise)),
            measurement_noise: Some(text("[[2.5e-5]]")),
            solver_mode: Some(text("finite")),
            horizon_steps: Some(text("20")),
            terminal_weights: Some(text("[[400, 0], [0, 400]]")),
            ..StateDesignDraft::default()
        }),
        ..StateDesignInput::default()
    }
}

/// The Tune of a loop on a current model with the requirements and the state-design inputs the README gives.
fn designed(
    loop_definition: &LoopDefinition,
    current: &CurrentModel,
    held: &TuningRequirements,
    state: &StateDesignInput,
) -> TuneOutcome {
    let request = request();
    let inputs = TuneInputs {
        loop_definition,
        model: Some(current),
        previous: None,
        request: &request,
        relay: None,
    };
    run(&inputs, Some(held), Some(state)).expect("the lesson's law designs")
}

/// The value of a coefficient of the candidate, by its symbol.
fn value(outcome: &TuneOutcome, symbol: &str) -> f64 {
    outcome.result.settings[symbol]
}

/// Asserts that `actual` is `expected` to within `tolerance`.
fn assert_near(actual: f64, expected: f64, tolerance: f64) {
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} is not {expected} within {tolerance}"
    );
}

#[test]
fn lesson_68_state_feedback_designs_the_readme_gains_from_its_desired_poles() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let model = model_of(LESSON_68_MODEL);
    let current = supplied(model.clone());
    let state = StateDesignInput {
        draft: Some(StateDesignDraft {
            desired_poles: Some(text("[0.9231163463866358, 0.8869204367171575]")),
            ..StateDesignDraft::default()
        }),
        ..StateDesignInput::default()
    };
    let outcome = designed(
        &loop_definition,
        &current,
        &requirements(60.0, 120.0),
        &state,
    );

    // The README prints K1 0.0562, K2 0.0369 and N 0.1131; the exact values are the Ackermann gains on the exact ZOH model.
    assert_near(value(&outcome, "P1.K1"), 0.056_160_138_938_318_616, 1e-9);
    assert_near(value(&outcome, "P1.K2"), 0.036_934_501_217_585_32, 1e-9);
    assert_near(value(&outcome, "P1.N"), 0.113_094_640_155_904_02, 1e-9);

    // The closed-loop poles are the desired sampled poles, read back from the gains.
    let answer = outcome
        .result
        .state
        .as_ref()
        .expect("a state law answers its state");
    assert_eq!(answer.family, StateControllerFamily::StateFeedback);
    let mut poles: Vec<f64> = answer
        .closed_loop_poles
        .iter()
        .map(|pole| pole.real)
        .collect();
    poles.sort_by(f64::total_cmp);
    assert_eq!(poles.len(), 2);
    assert_near(poles[0], 0.886_920_436_717_157_5, 1e-9);
    assert_near(poles[1], 0.923_116_346_386_635_8, 1e-9);
    assert!(answer.horizon.is_none(), "pole placement keeps no stages");
    assert_eq!(
        answer
            .fields
            .iter()
            .map(|field| field.label.as_str())
            .collect::<Vec<_>>(),
        ["Desired poles"]
    );

    // The README's phase margin is 95 degrees on the sampled loop (N7.9).
    let margins = outcome
        .result
        .margins
        .as_ref()
        .expect("a continuous single-input law has margins");
    let MarginValue::Finite(degrees) = margins.phase_margin_degrees else {
        panic!(
            "the phase margin is not a value: {:?}",
            margins.phase_margin_degrees
        );
    };
    assert_near(degrees, 95.0, 0.5);

    // The candidate is stored unapplied, as a state law, and its section holds its rules (B4.1, C4).
    let section = &outcome.section;
    assert!(section.candidate.is_none() && section.project_candidate.is_none());
    assert!(matches!(
        section.law_candidate(),
        Some(LawCandidate::State(_))
    ));
    section
        .check()
        .expect("the stored state section holds its rules");
    assert_eq!(outcome.result.method_title, "State-space design");
}

#[test]
fn lesson_69_lqr_designs_the_readme_gains_and_the_riccati_stages() {
    let loop_definition = loop_of(LESSON_69_LOOP);
    let current = supplied(model_of(LESSON_68_MODEL));
    let outcome = designed(
        &loop_definition,
        &current,
        &requirements(90.0, 120.0),
        &lqr_state("[[40000]]"),
    );

    // The README: K1 0.0784, K2 0.0245 and N 0.1229 for R = 40000; the first gain is the one the runtime repeats.
    assert_near(value(&outcome, "P1.K1"), 0.078_412_587_174_395_77, 1e-9);
    assert_near(value(&outcome, "P1.K2"), 0.024_514_449_575_293_897, 1e-9);
    assert_near(value(&outcome, "P1.N"), 0.122_927_036_749_689_76, 1e-9);

    // The stages: twenty gains, and stage 19, the one-step design from the terminal weight: the README's [0.0182, 0.0004].
    let answer = outcome
        .result
        .state
        .as_ref()
        .expect("a state law answers its state");
    assert_eq!(answer.family, StateControllerFamily::Lqr);
    let horizon = answer
        .horizon
        .as_ref()
        .expect("a finite horizon keeps its stages");
    assert_eq!(horizon.steps, 20);
    assert_eq!(horizon.gain_sequence.len(), 20);
    assert_eq!(horizon.riccati_sequence.len(), 21);
    assert_near(
        horizon.gain_sequence[19][0][0],
        0.018_153_479_869_078_695,
        1e-9,
    );
    assert_near(
        horizon.gain_sequence[19][0][1],
        0.000_360_362_829_562_468_9,
        1e-9,
    );
    assert_eq!(
        answer
            .fields
            .iter()
            .map(|field| field.label.as_str())
            .collect::<Vec<_>>(),
        [
            "Solver policy",
            "State weights (Q)",
            "Input weights (R)",
            "Finite horizon steps",
            "Terminal weights"
        ]
    );

    // The second run of the README, with R = 160000: K1 0.0344, K2 0.0098 and N 0.0642.
    let heavier = designed(
        &loop_definition,
        &current,
        &requirements(90.0, 120.0),
        &lqr_state("[[160000]]"),
    );
    assert_near(value(&heavier, "P1.K1"), 0.034_421_319_420_877_45, 1e-9);
    assert_near(value(&heavier, "P1.K2"), 0.009_793_356_653_013_965, 1e-9);
    assert_near(value(&heavier, "P1.N"), 0.064_214_676_073_891_47, 1e-9);
}

#[test]
fn lesson_72_lqg_designs_the_readme_gains_and_the_estimator_gains() {
    let loop_definition = loop_of(LESSON_72_LOOP);
    let model = model_of(LESSON_72_MODEL);
    let current = supplied(model.clone());

    // Model error only (process noise 1e-8 I): the README's L1 0.002255 and L2 0.006778, from the TypeScript's stopping rule.
    let outcome = designed(
        &loop_definition,
        &current,
        &requirements(90.0, 120.0),
        &lqg_state("[[1e-8, 0], [0, 1e-8]]"),
    );
    assert_near(value(&outcome, "P1.K1"), 0.078_740_256_421_075_74, 1e-9);
    assert_near(value(&outcome, "P1.K2"), 0.023_874_303_938_916_23, 1e-9);
    assert_near(value(&outcome, "P1.N"), 0.122_614_560_359_992_06, 1e-9);
    assert_near(value(&outcome, "P1.L1"), 0.002_254_747_458_678_985_5, 1e-9);
    assert_near(value(&outcome, "P1.L2"), 0.006_777_664_794_470_649, 1e-9);
    let answer = outcome
        .result
        .state
        .as_ref()
        .expect("a state law answers its state");
    assert_eq!(answer.family, StateControllerFamily::Lqg);
    // Two poles of the controller and two of the estimator, all strictly inside the unit circle (C4).
    assert_eq!(answer.closed_loop_poles.len(), 4);
    assert!(answer
        .closed_loop_poles
        .iter()
        .all(|pole| pole.real.hypot(pole.imaginary) < 1.0));
    // The estimator's poles are those of (I − L·C)·A with the gains above, and the poles follow the coefficient values.
    let recomputed = state_controller_poles(&loop_definition, &model, &outcome.result.settings)
        .expect("the designed values are stable");
    assert_eq!(answer.closed_loop_poles, recomputed);

    // Upsets allowed (process noise 4e-6 in tank 1): the README's L1 0.2393 and L2 0.1056.
    let upsets = designed(
        &loop_definition,
        &current,
        &requirements(90.0, 120.0),
        &lqg_state("[[4e-6, 0], [0, 1e-8]]"),
    );
    assert_near(value(&upsets, "P1.L1"), 0.239_306_766_880_170_77, 1e-9);
    assert_near(value(&upsets, "P1.L2"), 0.105_558_858_926_505_9, 1e-9);
}

#[test]
fn a_state_law_without_requirements_is_refused_before_its_design() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let current = supplied(model_of(LESSON_68_MODEL));
    let request = request();
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&current),
        previous: None,
        request: &request,
        relay: None,
    };
    let failure = run(&inputs, None, None).expect_err("no requirements are held");
    assert_eq!(
        failure.message,
        crate::tuning::state_controller::texts::NEEDS_REQUIREMENTS
    );
}

#[test]
fn a_draft_field_that_is_not_json_names_its_field_in_the_refusal() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let current = supplied(model_of(LESSON_68_MODEL));
    let state = StateDesignInput {
        draft: Some(StateDesignDraft {
            desired_poles: Some(text("[0.5, ")),
            ..StateDesignDraft::default()
        }),
        ..StateDesignInput::default()
    };
    let request = request();
    let inputs = TuneInputs {
        loop_definition: &loop_definition,
        model: Some(&current),
        previous: None,
        request: &request,
        relay: None,
    };
    let failure = run(&inputs, Some(&requirements(60.0, 120.0)), Some(&state))
        .expect_err("the desired poles are not JSON");
    assert_eq!(failure.message, "Desired poles must be valid JSON.");
    assert_eq!(failure.field.as_deref(), Some("desiredPoles"));
}

#[test]
fn a_pinned_gain_replaces_its_designed_value_and_the_poles_follow_it() {
    let loop_definition = loop_of(LESSON_68_LOOP);
    let model = model_of(LESSON_68_MODEL);
    let current = supplied(model.clone());
    let state = StateDesignInput {
        draft: Some(StateDesignDraft {
            desired_poles: Some(text("[0.9231163463866358, 0.8869204367171575]")),
            ..StateDesignDraft::default()
        }),
        pins: Some(std::collections::BTreeMap::from([(
            "P1.K1".to_owned(),
            0.05,
        )])),
        ..StateDesignInput::default()
    };
    let outcome = designed(
        &loop_definition,
        &current,
        &requirements(60.0, 120.0),
        &state,
    );
    assert_near(value(&outcome, "P1.K1"), 0.05, 1e-12);
    assert_near(value(&outcome, "P1.K2"), 0.036_934_501_217_585_32, 1e-9);
    // The pin is the engineer's value, so the poles are those of the pinned gains.
    let answer = outcome
        .result
        .state
        .as_ref()
        .expect("a state law answers its state");
    let recomputed = state_controller_poles(&loop_definition, &model, &outcome.result.settings)
        .expect("the pinned gains are stable");
    assert_eq!(answer.closed_loop_poles, recomputed);
}

/// The refusal of a Tune of the loop with the given state-design inputs.
fn refused(
    loop_definition: &LoopDefinition,
    current: &CurrentModel,
    held: &TuningRequirements,
    state: &StateDesignInput,
) -> Failure {
    let request = request();
    let inputs = TuneInputs {
        loop_definition,
        model: Some(current),
        previous: None,
        request: &request,
        relay: None,
    };
    run(&inputs, Some(held), Some(state)).expect_err("the draft is refused")
}

#[test]
fn the_controller_offer_of_each_lesson_is_its_families_with_the_declared_one_recommended() {
    // Lesson 68 declares state feedback on one input: state feedback and LQR, with state feedback recommended.
    let offer_68 = offer(&loop_of(LESSON_68_LOOP)).expect("lesson 68's law is offered");
    assert_eq!(offer_68.families, ["state-feedback", "LQR"]);
    assert_eq!(offer_68.recommended, "state-feedback");
    assert_eq!(offer_68.family_settings["LQR"], ["P1.K1", "P1.K2", "P1.N"]);
    // Lesson 69 declares LQR: the same two families, with LQR recommended.
    let offer_69 = offer(&loop_of(LESSON_69_LOOP)).expect("lesson 69's law is offered");
    assert_eq!(offer_69.families, ["state-feedback", "LQR"]);
    assert_eq!(offer_69.recommended, "LQR");
    // Lesson 72 declares LQG: its estimator fixes the structure, so LQG alone, with the observer gains.
    let offer_72 = offer(&loop_of(LESSON_72_LOOP)).expect("lesson 72's law is offered");
    assert_eq!(offer_72.families, ["LQG"]);
    assert_eq!(offer_72.recommended, "LQG");
    assert_eq!(
        offer_72.family_settings["LQG"],
        ["P1.K1", "P1.K2", "P1.N", "P1.L1", "P1.L2"]
    );
}

#[test]
fn the_setup_answer_of_a_state_law_is_its_offer_as_the_page_receives_it() {
    let ControllerCompatibility::Accepted {
        families,
        recommended_family,
        methods,
        ..
    } = adapter_compatibility(&loop_of(LESSON_68_LOOP))
    else {
        panic!("a state law is accepted, not refused");
    };
    assert_eq!(families, ["state-feedback", "LQR"]);
    assert_eq!(recommended_family.as_deref(), Some("state-feedback"));
    assert_eq!(methods, Some(vec![TuneMethod::StateSpace]));
    // The JSON the page receives: the same families, the recommended family and the one method by its name.
    let page = serde_json::to_value(adapter_compatibility(&loop_of(LESSON_68_LOOP)))
        .expect("the answer encodes");
    assert_eq!(page["kind"], "accepted");
    assert_eq!(page["recommendedFamily"], "state-feedback");
    assert_eq!(page["methods"], json!(["state-space"]));
    assert_eq!(
        page["familySettings"]["LQR"],
        json!(["P1.K1", "P1.K2", "P1.N"])
    );
}

#[test]
fn a_refusal_of_a_draft_field_names_the_field_and_a_refusal_of_the_whole_form_names_none() {
    let loop_definition = loop_of(LESSON_69_LOOP);
    let current = supplied(model_of(LESSON_68_MODEL));
    let held = requirements(90.0, 120.0);
    // Horizon steps of zero: the refusal names the horizon-steps field, and its key reaches the page.
    let zero_steps = StateDesignInput {
        draft: Some(StateDesignDraft {
            state_weights: Some(text("[[400, 0], [0, 400]]")),
            input_weights: Some(text("[[40000]]")),
            solver_mode: Some(text("finite")),
            horizon_steps: Some(text("0")),
            terminal_weights: Some(text("[[400, 0], [0, 400]]")),
            ..StateDesignDraft::default()
        }),
        ..StateDesignInput::default()
    };
    let failure = refused(&loop_definition, &current, &held, &zero_steps);
    assert_eq!(failure.field.as_deref(), Some("horizonSteps"));
    let page = serde_json::to_value(&failure).expect("the failure encodes");
    assert_eq!(page["field"], "horizonSteps");
    // A state weight that is not symmetric refuses the whole form: its sentence has no field.
    let asymmetric = StateDesignInput {
        draft: Some(StateDesignDraft {
            state_weights: Some(text("[[400, 1], [0, 400]]")),
            input_weights: Some(text("[[40000]]")),
            ..StateDesignDraft::default()
        }),
        ..StateDesignInput::default()
    };
    let failure = refused(&loop_definition, &current, &held, &asymmetric);
    assert!(failure.field.is_none());
    let page = serde_json::to_value(&failure).expect("the failure encodes");
    assert!(page.get("field").is_none(), "{page}");
}

#[test]
fn a_kept_state_section_reopens_to_the_live_answer() {
    // A pole-placement law, with its margins, and a finite-horizon law, with its stages: each reopens to the answer its run gave.
    let loop_68 = loop_of(LESSON_68_LOOP);
    let current = supplied(model_of(LESSON_68_MODEL));
    let poles = StateDesignInput {
        draft: Some(StateDesignDraft {
            desired_poles: Some(text("[0.9231163463866358, 0.8869204367171575]")),
            ..StateDesignDraft::default()
        }),
        ..StateDesignInput::default()
    };
    let live = designed(&loop_68, &current, &requirements(60.0, 120.0), &poles);
    let reopened = kept_candidate(
        &loop_68,
        Some(&current),
        Some(&live.section),
        live.result.stage_evidence,
    )
    .expect("the kept state section is answered");
    assert_eq!(reopened, live.result);

    let loop_69 = loop_of(LESSON_69_LOOP);
    let live = designed(
        &loop_69,
        &current,
        &requirements(90.0, 120.0),
        &lqr_state("[[40000]]"),
    );
    let reopened = kept_candidate(
        &loop_69,
        Some(&current),
        Some(&live.section),
        live.result.stage_evidence,
    )
    .expect("the kept state section is answered");
    assert_eq!(reopened, live.result);
    assert!(reopened
        .state
        .as_ref()
        .and_then(|answer| answer.horizon.as_ref())
        .is_some());
}

#[test]
fn the_form_fields_follow_the_page_and_show_the_horizon_only_for_the_finite_policy() {
    let labels = |family: StateControllerFamily, finite: bool| -> Vec<String> {
        form_fields(family, finite)
            .into_iter()
            .map(|field| field.label)
            .collect()
    };
    // The page's own words (HEAD `threePageStateDesign.ts`): the solver policy first, the horizon only when it is finite.
    assert_eq!(
        labels(StateControllerFamily::Lqr, false),
        ["Solver policy", "State weights (Q)", "Input weights (R)"]
    );
    assert_eq!(
        labels(StateControllerFamily::Lqr, true),
        [
            "Solver policy",
            "State weights (Q)",
            "Input weights (R)",
            "Finite horizon steps",
            "Terminal weights"
        ]
    );
    assert_eq!(
        labels(StateControllerFamily::Lqg, true),
        [
            "Solver policy",
            "State weights (Q)",
            "Input weights (R)",
            "Process noise covariance",
            "Measurement noise covariance",
            "Finite horizon steps",
            "Terminal weights"
        ]
    );
    assert_eq!(
        labels(StateControllerFamily::StateFeedback, false),
        ["Desired poles"]
    );
    // The page builds each control from the key and shows the label beside it.
    let encoded = serde_json::to_value(form_fields(StateControllerFamily::Lqr, true))
        .expect("the fields encode");
    assert_eq!(
        encoded[3],
        json!({"field": "horizonSteps", "label": "Finite horizon steps"})
    );
}

#[test]
fn the_state_fields_of_a_loop_come_from_its_family_and_the_stored_policy() {
    let lesson_68 = loop_of(LESSON_68_LOOP);
    let lesson_69 = loop_of(LESSON_69_LOOP);
    let policy = |mode: &str| StateDesignInput {
        draft: Some(StateDesignDraft {
            solver_mode: Some(text(mode)),
            ..StateDesignDraft::default()
        }),
        ..StateDesignInput::default()
    };
    let labels = |fields: Option<Vec<FormField>>| -> Option<Vec<String>> {
        fields.map(|fields| fields.into_iter().map(|field| field.label).collect())
    };
    let lr_fields = |finite: bool| {
        let mut words = vec!["Solver policy", "State weights (Q)", "Input weights (R)"];
        if finite {
            words.extend(["Finite horizon steps", "Terminal weights"]);
        }
        Some(
            words
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<String>>(),
        )
    };
    // Lesson 68 declares state feedback: its desired poles, whatever the stored policy says.
    assert_eq!(
        labels(state_fields(&lesson_68, None)),
        Some(vec!["Desired poles".to_owned()])
    );
    assert_eq!(
        labels(state_fields(&lesson_68, Some(&policy("finite")))),
        Some(vec!["Desired poles".to_owned()])
    );
    // A family the engineer chose replaces the declared one: LQR on lesson 68, under the policy the draft stores.
    let lqr_choice = StateDesignInput {
        family: Some(StateControllerFamily::Lqr),
        ..policy("infinite")
    };
    assert_eq!(
        labels(state_fields(&lesson_68, Some(&lqr_choice))),
        lr_fields(false)
    );
    // Lesson 69 declares LQR: the stored policy decides the horizon fields, and no draft is the steady-state default.
    assert_eq!(labels(state_fields(&lesson_69, None)), lr_fields(false));
    assert_eq!(
        labels(state_fields(&lesson_69, Some(&policy("infinite")))),
        lr_fields(false)
    );
    assert_eq!(
        labels(state_fields(&lesson_69, Some(&policy("finite")))),
        lr_fields(true)
    );
    // The fields that the inputs answer gives are the fields the Tune answer gives under the same policy.
    let finite = lqr_state("[[40000]]");
    let current = supplied(model_of(LESSON_68_MODEL));
    let outcome = designed(&lesson_69, &current, &requirements(90.0, 120.0), &finite);
    assert_eq!(
        outcome.result.state.map(|answer| answer.fields),
        state_fields(&lesson_69, Some(&finite))
    );
    // A loop without a state law has no state fields.
    let pid = LoopDefinition::decode(include_str!(
        "../../../../../examples/control-pid-commissioning/control-loop.json"
    ))
    .expect("the example loop decodes");
    assert_eq!(state_fields(&pid, None), None);
}
