//! The stored preferences and the draft rules, asserted on the decoder and draft cases of
//! `control-studio-state-design-preferences.test.ts` (the webview cases are not ported: they test the form, not the rules).

use serde_json::json;
use trust_control_math::lqr::FiniteHorizon;

use super::*;

fn dimensions() -> ModelDimensions {
    ModelDimensions {
        states: 2,
        inputs: 1,
        outputs: 1,
    }
}

fn draft_for(solver: &str, steps: &str, terminal: &str) -> StateDesignDraft {
    StateDesignDraft {
        state_weights: Some("[[1]]".to_string()),
        input_weights: Some("[[1]]".to_string()),
        solver_mode: Some(solver.to_string()),
        horizon_steps: Some(steps.to_string()),
        terminal_weights: Some(terminal.to_string()),
        ..Default::default()
    }
}

#[test]
fn declared_inputs_are_accepted_in_the_model_dimensions() {
    let value = decode_state_design_preferences(
        &json!({ "desiredPoles": [0.7, 0.8], "stateWeights": [[2, 0], [0, 1]],
            "inputWeights": [[3]], "processNoise": [[0.1, 0], [0, 0.2]], "measurementNoise": [[0.3]] }),
        Some(dimensions()),
    )
    .expect("every input has the model's size");
    assert_eq!(value.desired_poles, Some(vec![0.7, 0.8]));
    assert_eq!(
        value.state_weights,
        Some(vec![vec![2.0, 0.0], vec![0.0, 1.0]])
    );
    assert_eq!(value.measurement_noise, Some(vec![vec![0.3]]));
}

#[test]
fn wrong_dimensions_nonsymmetric_weights_nonpositive_inputs_and_wrong_pole_counts_are_refused() {
    assert!(
        decode_state_design_preferences(&json!({ "stateWeights": [[1]] }), Some(dimensions()))
            .is_none()
    );
    assert!(decode_state_design_preferences(
        &json!({ "stateWeights": [[1, 1], [0, 1]] }),
        Some(dimensions())
    )
    .is_none());
    assert!(
        decode_state_design_preferences(&json!({ "inputWeights": [[0]] }), Some(dimensions()))
            .is_none()
    );
    assert!(
        decode_state_design_preferences(&json!({ "desiredPoles": [0.7] }), Some(dimensions()))
            .is_none()
    );
}

#[test]
fn omitted_fields_stay_absent_and_unknown_keys_are_refused() {
    let value =
        decode_state_design_preferences(&json!({ "inputWeights": [[2]] }), Some(dimensions()))
            .expect("one declared field");
    assert_eq!(
        value,
        StateDesignPreferences {
            input_weights: Some(vec![vec![2.0]]),
            ..Default::default()
        }
    );
    assert!(decode_state_design_preferences(
        &json!({ "inputWeights": [[2]], "runtimeWrite": true }),
        Some(dimensions())
    )
    .is_none());
}

#[test]
fn malformed_finite_horizons_are_refused_and_a_valid_one_is_kept() {
    let refused = |horizon: serde_json::Value| {
        decode_state_design_preferences(&json!({ "finiteHorizon": horizon }), Some(dimensions()))
            .is_none()
    };
    assert!(refused(
        json!({ "steps": 0, "terminalWeights": [[1, 0], [0, 1]] })
    ));
    assert!(refused(
        json!({ "steps": 513, "terminalWeights": [[1, 0], [0, 1]] })
    ));
    assert!(
        refused(json!({ "steps": 2, "terminalWeights": [[1, 0]] })),
        "not square"
    );
    assert!(
        refused(json!({ "steps": 2, "terminalWeights": [[1, 2], [0, 1]] })),
        "not symmetric"
    );
    assert!(
        refused(json!({ "steps": 2, "terminalWeights": [[-1, 0], [0, 1]] })),
        "negative diagonal"
    );
    assert!(refused(
        json!({ "steps": 2, "terminalWeights": [[1, 0], [0, 1]], "extra": 1 })
    ));
    let kept = decode_state_design_preferences(
        &json!({ "finiteHorizon": { "steps": 2, "terminalWeights": [[1, 0], [0, 1]] } }),
        Some(dimensions()),
    )
    .expect("a two-step horizon with a valid terminal weight");
    assert_eq!(
        kept.finite_horizon,
        Some(FiniteHorizon {
            steps: 2,
            terminal_weights: vec![vec![1.0, 0.0], vec![0.0, 1.0]],
        })
    );
}

#[test]
fn a_draft_names_its_weights_and_its_finite_horizon_for_an_lqr() {
    let draft = draft_for("finite", "2", "[[1]]");
    let parsed =
        parse_state_design_draft(&draft, StateControllerFamily::Lqr).expect("the draft reads");
    assert_eq!(parsed.state_weights, Some(vec![vec![1.0]]));
    assert_eq!(parsed.input_weights, Some(vec![vec![1.0]]));
    assert_eq!(
        parsed.finite_horizon,
        Some(FiniteHorizon {
            steps: 2,
            terminal_weights: vec![vec![1.0]],
        })
    );
}

#[test]
fn a_malformed_weight_is_named_under_its_own_field() {
    let draft = StateDesignDraft {
        state_weights: Some("[[".to_string()),
        ..Default::default()
    };
    let error = parse_state_design_draft(&draft, StateControllerFamily::Lqr)
        .expect_err("the weight is not JSON");
    assert_eq!(error.field, Some(DraftField::StateWeights));
    assert_eq!(error.message, "State weights (Q) must be valid JSON.");
}

#[test]
fn fields_that_another_family_does_not_use_are_not_read() {
    let draft = StateDesignDraft {
        state_weights: Some("[[".to_string()),
        desired_poles: Some("[0.5, 0.6]".to_string()),
        ..Default::default()
    };
    let parsed = parse_state_design_draft(&draft, StateControllerFamily::StateFeedback)
        .expect("state feedback reads only its poles");
    assert_eq!(parsed.desired_poles, Some(vec![0.5, 0.6]));
    assert!(parsed.state_weights.is_none());
}

#[test]
fn a_blank_horizon_a_blank_terminal_weight_and_an_unknown_solver_are_named() {
    let blank_steps = parse_state_design_draft(
        &draft_for("finite", "  ", "[[1]]"),
        StateControllerFamily::Lqr,
    )
    .expect_err("the steps are blank");
    assert_eq!(blank_steps.field, Some(DraftField::HorizonSteps));
    assert_eq!(
        blank_steps.message,
        "Horizon steps must be an integer from 1 to 512."
    );
    let blank_terminal =
        parse_state_design_draft(&draft_for("finite", "2", " "), StateControllerFamily::Lqr)
            .expect_err("the terminal weights are blank");
    assert_eq!(blank_terminal.field, Some(DraftField::TerminalWeights));
    assert_eq!(blank_terminal.message, "Enter the terminal weights matrix.");
    let solver = parse_state_design_draft(
        &draft_for("sometimes", "2", "[[1]]"),
        StateControllerFamily::Lqr,
    )
    .expect_err("the solver is neither finite nor infinite");
    assert_eq!(solver.field, Some(DraftField::SolverMode));
    assert_eq!(solver.message, "Choose finite or infinite horizon.");
}

#[test]
fn a_structurally_invalid_draft_is_refused_without_a_field() {
    let draft = StateDesignDraft {
        state_weights: Some("[[1, 1], [0, 1]]".to_string()),
        ..Default::default()
    };
    let error = parse_state_design_draft(&draft, StateControllerFamily::Lqr)
        .expect_err("the weight is not symmetric");
    assert_eq!(error.field, None);
    assert_eq!(
        error.message,
        "Use finite symmetric weight matrices and valid desired poles."
    );
}

#[test]
fn a_draft_is_decoded_only_when_its_fields_are_known_text_within_the_limit() {
    assert!(decode_state_design_draft(&json!({ "stateWeights": "[[1]]" })).is_some());
    assert!(
        decode_state_design_draft(&json!({ "stateWeights": "[[1]]", "runtimeWrite": "x" }))
            .is_none()
    );
    assert!(decode_state_design_draft(&json!({ "stateWeights": null })).is_none());
    let too_long = "a".repeat(MAXIMUM_DRAFT_UNITS + 1);
    assert!(decode_state_design_draft(&json!({ "stateWeights": too_long })).is_none());
}

#[test]
fn every_offered_choice_carries_its_words() {
    // B4.2, C4: the solver policy is a choice between two solutions, and the form lists them from the engine's words, so each choice says
    // what it is called and which one stands. A choice without words would be listed blank, and a form that knows no standing choice
    // would show an invented default.
    let solver = |finite: bool| {
        serde_json::to_value(form_fields(StateControllerFamily::Lqr, finite).remove(0))
            .expect("the field encodes")
    };
    let choices = json!([
        { "value": "infinite", "label": "Infinite-horizon / steady-state" },
        { "value": "finite", "label": "Finite-horizon (fixed policy)" },
    ]);
    assert_eq!(
        solver(false),
        json!({ "field": "solverMode", "label": "Solver policy", "choices": choices, "selected": "infinite" })
    );
    assert_eq!(
        solver(true),
        json!({ "field": "solverMode", "label": "Solver policy", "choices": choices, "selected": "finite" }),
        "a stored finite horizon is the choice that stands"
    );
    // A field of text offers no choice and names none that stands.
    let weights = serde_json::to_value(form_fields(StateControllerFamily::Lqr, false).remove(1))
        .expect("the field encodes");
    assert_eq!(
        weights,
        json!({ "field": "stateWeights", "label": "State weights (Q)" })
    );
}
