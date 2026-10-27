//! Tests of the plant editor's rules. Every expected value is worked out from specifications 46 and
//! 47 by hand; each test names the wrong behaviour it catches.

use serde_json::{json, Map, Value};

use super::*;

mod actions_view;
mod effects_events;
mod ports_signals;
mod state_space_piecewise;

/// A draft object from JSON.
pub(super) fn draft(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("a draft is an object")
}

/// The (path, message) pairs of a list of issues.
pub(super) fn found(issues: &[issues::Issue]) -> Vec<(String, String)> {
    issues
        .iter()
        .map(|issue| (issue.path.clone(), issue.message.clone()))
        .collect()
}

/// A first-order draft bound to one command and one feedback signal.
pub(super) fn first_order() -> Value {
    json!({
        "plantId": "tank",
        "displayName": "Tank",
        "description": "",
        "enabled": true,
        "model": "first_order",
        "commandSources": ["%QW0"],
        "feedbackTargets": ["%IW0"],
        "parameters": {"gain": 1, "time_constant_s": 5},
        "initialState": [0],
    })
}

#[test]
fn the_model_selector_lists_every_model_in_product_order() {
    // Specification 47 §7 (the selector table) and the cascade section after §21.1. Catches a
    // model missing from the selector, a renamed model or a changed order.
    let names: Vec<(&str, &str)> = tables::MODELS
        .iter()
        .map(|model| (model.kind, model.product_name))
        .collect();
    assert_eq!(
        names,
        [
            ("static_gain", "Static gain"),
            ("first_order", "First order"),
            ("first_order_dead_time", "First order with dead time"),
            ("integrator", "Integrator"),
            ("second_order", "Second order"),
            ("piecewise_linear", "Piecewise linear"),
            ("state_space", "State space"),
            ("gravity_tank", "Gravity-drained tank"),
            ("gravity_tank_cascade", "Cascaded gravity tanks"),
            ("robot_joint", "Gravity-loaded robot joint"),
        ]
    );
    // Specification 46 §7.8 and §7.9: one level state for the tank; angle and speed for the joint.
    assert_eq!(
        tables::model_descriptor("gravity_tank"),
        (
            "Gravity-drained tank".to_string(),
            "Inflow raises the level; gravity drains it faster as the level rises.".to_string(),
            1,
            1,
            1
        )
    );
    assert_eq!(tables::model_descriptor("robot_joint").2, 2);
    assert_eq!(tables::model_descriptor("robot_joint").4, 2);
}

#[test]
fn each_model_shows_only_its_own_dynamics_fields() {
    // Specification 47 §9 table. Catches a field of another model shown, or one missing.
    let keys = |model: &str| -> Vec<&str> {
        tables::dynamics_fields(model)
            .iter()
            .map(|field| field.key)
            .collect()
    };
    assert_eq!(keys("static_gain"), ["gain", "input_bias", "output_bias"]);
    assert_eq!(
        keys("first_order"),
        [
            "gain",
            "time_constant_s",
            "input_bias",
            "output_bias",
            "initial_state"
        ]
    );
    assert_eq!(
        keys("first_order_dead_time"),
        [
            "gain",
            "time_constant_s",
            "dead_time_s",
            "input_bias",
            "output_bias",
            "initial_delayed_input",
            "initial_state"
        ]
    );
    assert_eq!(
        keys("integrator"),
        [
            "gain",
            "input_bias",
            "output_bias",
            "state_minimum",
            "state_maximum",
            "initial_state"
        ]
    );
    assert_eq!(
        keys("second_order"),
        [
            "gain",
            "natural_frequency_rad_s",
            "damping_ratio",
            "input_bias",
            "output_bias",
            "initial_state"
        ]
    );
    assert_eq!(keys("piecewise_linear"), ["input_points", "output_points"]);
    assert_eq!(
        keys("state_space"),
        ["a", "b", "c", "d", "input_delays_s", "initial_state"]
    );
    assert_eq!(
        keys("gravity_tank"),
        [
            "area_m2",
            "outflow_coefficient_m2_sqrt_m_per_s",
            "initial_state"
        ]
    );
    assert_eq!(
        keys("robot_joint"),
        [
            "inertia_kg_m2",
            "gravity_torque_nm",
            "viscous_friction_nm_s_per_rad",
            "coulomb_friction_nm",
            "friction_smoothing_rad_s",
            "initial_state"
        ]
    );
    // Specification 47 §9: omitted gain resolves to 1, omitted biases to 0, the initial delayed
    // input to the input bias; §21.1: the robot joint's gravity and friction terms to 0.
    let defaults = |model: &str| -> Vec<(&str, Option<f64>, Option<&str>)> {
        tables::dynamics_fields(model)
            .iter()
            .filter(|field| field.optional)
            .map(|field| (field.key, field.default_value, field.default_from))
            .collect()
    };
    assert_eq!(
        defaults("first_order_dead_time"),
        [
            ("gain", Some(1.0), None),
            ("dead_time_s", Some(0.0), None),
            ("input_bias", Some(0.0), None),
            ("output_bias", Some(0.0), None),
            ("initial_delayed_input", None, Some("input_bias")),
        ]
    );
    assert_eq!(
        defaults("robot_joint"),
        [
            ("gravity_torque_nm", Some(0.0), None),
            ("viscous_friction_nm_s_per_rad", Some(0.0), None),
            ("coulomb_friction_nm", Some(0.0), None),
        ]
    );
    assert_eq!(tables::stored_key("gain"), "parameters.gain");
    assert_eq!(tables::stored_key("initial_state"), "initial_state");
}

#[test]
fn the_actuator_fields_say_what_an_empty_field_means() {
    // Specification 47 §10 and specification 46 §8: an omitted rate is unbounded and 0 blocks
    // movement; omitted deadband, stiction, slip and lag are 0, the exponent 1 (linear), and the
    // initial applied value is the initial input. Catches an empty field shown as zero.
    let fields: Vec<(&str, &str, Option<f64>, Option<&str>)> = tables::ACTUATOR_FIELDS
        .iter()
        .map(|field| {
            (
                field.key,
                field.blank_meaning,
                field.default_value,
                field.default_from,
            )
        })
        .collect();
    assert_eq!(
        fields,
        [
            ("minimum", "Unbounded", None, None),
            ("maximum", "Unbounded", None, None),
            (
                "rate_up_per_s",
                "Unbounded; 0 blocks upward movement",
                None,
                None
            ),
            (
                "rate_down_per_s",
                "Unbounded; 0 blocks downward movement",
                None,
                None
            ),
            ("deadband", "0 (no deadband)", Some(0.0), None),
            ("stiction_band", "0 (no static friction)", Some(0.0), None),
            ("slip_jump", "0 (no breakaway jump)", Some(0.0), None),
            (
                "characteristic_exponent",
                "1 (linear characteristic)",
                Some(1.0),
                None
            ),
            ("lag_s", "0 (no lag)", Some(0.0), None),
            (
                "initial_applied",
                "Initial input (resolved at runtime)",
                None,
                Some("initial_input")
            ),
        ]
    );
}

fn actuator_issues(actuator: Value) -> Vec<(String, String)> {
    let mut value = first_order();
    value["actuator"] = actuator;
    found(&checks::actuator(&draft(value)))
}

#[test]
fn the_actuator_checks_find_every_out_of_domain_setting() {
    // Specification 46 §8 and specification 47 §10. Each pair catches the rule missing or the
    // issue reported at another field.
    let one = |path: &str, message: &str| vec![(path.to_string(), message.to_string())];
    assert_eq!(
        actuator_issues(json!({"minimum": 5, "maximum": 4})),
        one(
            "actuator.maximum",
            "Maximum must be greater than or equal to minimum."
        )
    );
    assert_eq!(
        actuator_issues(json!({"rate_up_per_s": -1})),
        one(
            "actuator.rate_up_per_s",
            "Rising rate must be 0 or greater."
        )
    );
    assert_eq!(
        actuator_issues(json!({"stiction_band": 3, "slip_jump": 4})),
        one(
            "actuator.slip_jump",
            "Slip jump must not exceed the stiction band."
        )
    );
    assert_eq!(
        actuator_issues(json!({"characteristic_exponent": 0})),
        one(
            "actuator.characteristic_exponent",
            "Installed characteristic exponent must be greater than 0."
        )
    );
    assert_eq!(
        actuator_issues(json!({"characteristic_exponent": 2})),
        one(
            "actuator.characteristic_exponent",
            "A nonlinear installed characteristic requires finite minimum and maximum values with minimum below maximum."
        )
    );
    assert_eq!(
        actuator_issues(json!({"minimum": 0, "maximum": 10, "initial_applied": 11})),
        one(
            "actuator.initial_applied",
            "Initial applied value must be within minimum and maximum."
        )
    );
    // A value the editor could not read arrives as null (JSON has no NaN): it is not finite.
    assert_eq!(
        actuator_issues(json!({"lag_s": null})),
        one(
            "actuator.lag_s",
            "Enter a finite number or leave this field empty."
        )
    );
    // A linear exponent needs no range, and a consistent range with an exponent is valid.
    assert!(actuator_issues(json!({"characteristic_exponent": 1})).is_empty());
    assert!(
        actuator_issues(json!({"minimum": 0, "maximum": 100, "characteristic_exponent": 2}))
            .is_empty()
    );
}

#[test]
fn the_candidate_carries_the_actuator_only_when_one_is_set() {
    // Specification 47 §13.2: clearing every optional actuator value must not leave an empty
    // actuator table behind. Catches an empty `actuator` saved.
    let mut value = first_order();
    value["actuator"] = json!({});
    assert_eq!(
        candidate::candidate(&draft(value.clone())).get("actuator"),
        None
    );
    value["actuator"] = json!({"minimum": 0, "maximum": 100, "characteristic_exponent": 2});
    assert_eq!(
        candidate::candidate(&draft(value)).get("actuator"),
        Some(&json!({"minimum": 0, "maximum": 100, "characteristic_exponent": 2}))
    );
}

#[test]
fn runtime_defaultable_values_may_be_left_empty() {
    // Specification 47 §9: omitted gain resolves to 1 and omitted FOPDT dead time to 0, so leaving
    // them empty is valid. Catches a defaultable field made mandatory.
    for (model, parameters, initial_state) in [
        ("static_gain", json!({}), json!([])),
        (
            "first_order_dead_time",
            json!({"time_constant_s": 5}),
            json!([0]),
        ),
        ("first_order", json!({"time_constant_s": 5}), json!([0])),
        ("integrator", json!({}), json!([0])),
        (
            "second_order",
            json!({"natural_frequency_rad_s": 1, "damping_ratio": 0.7}),
            json!([0, 0]),
        ),
    ] {
        let value =
            json!({"model": model, "parameters": parameters, "initialState": initial_state});
        assert!(checks::dynamics(&draft(value)).is_empty(), "{model}");
    }
}

#[test]
fn the_dynamics_checks_report_each_domain_at_its_field() {
    // Specification 46 §7.3 and §7.4: a time constant above 0 s, a dead time not below 0 s, an
    // integrator maximum not below its minimum. Catches a rule missing or the wrong field named.
    let issues = |parameters: Value, model: &str, initial_state: Value| {
        found(&checks::dynamics(&draft(
            json!({"model": model, "parameters": parameters, "initialState": initial_state}),
        )))
    };
    assert_eq!(
        issues(
            json!({"gain": 1, "time_constant_s": 0, "dead_time_s": -1, "input_bias": 0, "output_bias": 0, "initial_delayed_input": 0}),
            "first_order_dead_time",
            json!([0])
        ),
        [
            (
                "parameters.time_constant_s".to_string(),
                "Time constant must be greater than 0 s.".to_string()
            ),
            (
                "parameters.dead_time_s".to_string(),
                "Dead time must be 0 s or greater.".to_string()
            ),
        ]
    );
    assert_eq!(
        issues(
            json!({"gain": 2, "input_bias": 1, "output_bias": 3, "state_minimum": 5, "state_maximum": 4}),
            "integrator",
            json!([4])
        ),
        [(
            "parameters.state_maximum".to_string(),
            "Maximum state must be greater than or equal to minimum state.".to_string()
        )]
    );
    assert!(issues(
        json!({"gain": 2, "input_bias": 1, "output_bias": 3, "state_minimum": 0, "state_maximum": 10}),
        "integrator",
        json!([4])
    )
    .is_empty());
    // A required field that is missing or holds an unreadable number, and input delays outside
    // continuous state space (specification 46 §7.6.1).
    assert_eq!(
        issues(
            json!({"gain": 1, "time_constant_s": null, "input_delays_s": [0]}),
            "first_order",
            json!([0])
        ),
        [
            (
                "parameters.time_constant_s".to_string(),
                "Time constant (s) must contain finite numbers.".to_string()
            ),
            (
                "parameters.input_delays_s".to_string(),
                "Input delays are available only for continuous state-space models.".to_string()
            ),
        ]
    );
    // A starting value that is no number is named by its index, so that its own field shows the
    // problem (specification 47 §9). Catches one issue for the whole state, which no field shows.
    assert_eq!(
        issues(
            json!({"gain": 1, "time_constant_s": 30}),
            "first_order",
            json!([null])
        ),
        [(
            "initial_state[0]".to_string(),
            "Initial state must contain finite numbers.".to_string()
        )]
    );
    // A list field holds the text the user typed while it is not a list (specification 47 §9): the
    // text is refused as a list, not as a number. Catches typed text taken for a value, or a
    // number accepted where a list belongs.
    for typed in [json!("[1,"), json!(5)] {
        assert_eq!(
            issues(
                json!({"gain": 1, "time_constant_s": 30, "dead_time_s": 2}),
                "first_order_dead_time",
                typed
            ),
            [(
                "initial_state".to_string(),
                "Enter a list such as [0, 1, 2].".to_string()
            )]
        );
    }
    let summary = checks::dynamics(&draft(json!({
        "model": "first_order", "parameters": {"gain": 1, "time_constant_s": 0}, "initialState": [0],
    })));
    assert_eq!(
        summary[0].summary, "Time constant (s) (0): Time constant must be greater than 0 s.",
        "the summary names the field and repeats its value"
    );
}

#[test]
fn the_candidate_is_the_exact_saved_shape_of_the_draft() {
    // Specification 47 §13.2 and §20.2 (scenario events always present). Catches a renamed field, a
    // dropped conversion or an editor-only field saved.
    let mut value = first_order();
    value["description"] = json!("Slow level process");
    value["displayName"] = json!("Training tank");
    value["parameters"] = json!({"gain": 2, "time_constant_s": 5});
    value["initialState"] = json!([1]);
    value["modelValuesTouched"] = json!(true);
    value["dirty"] = json!(true);
    assert_eq!(
        candidate::candidate(&draft(value.clone())),
        json!({
            "id": "tank", "name": "Training tank", "description": "Slow level process",
            "enabled": true, "model": "first_order",
            "command_sources": ["%QW0"], "feedback_targets": ["%IW0"],
            "parameters": {"gain": 2, "time_constant_s": 5}, "initial_state": [1],
            "scenario_events": [],
        })
    );
    value["stepMs"] = json!(5);
    value["initialInput"] = json!([2.5]);
    value["inputScale"] = json!([0.1]);
    value["inputOffset"] = json!([-1]);
    value["outputScale"] = json!([10]);
    value["outputOffset"] = json!([4]);
    let saved = candidate::candidate(&draft(value));
    assert_eq!(saved["step_ms"], json!(5));
    assert_eq!(saved["initial_input"], json!([2.5]));
    assert_eq!(saved["input_scale"], json!([0.1]));
    assert_eq!(saved["input_offset"], json!([-1]));
    assert_eq!(saved["output_scale"], json!([10]));
    assert_eq!(saved["output_offset"], json!([4]));
}

#[test]
fn omitted_enabled_and_initial_state_stay_omitted_until_changed() {
    // Specification 47 §9: a saved plant with omitted `initial_state` shows the model-sized zero
    // vector and keeps the omission on a no-op edit; the same holds for an omitted `enabled`.
    // Catches an omission materialized by opening and saving.
    let mut value = first_order();
    value["enabledDefaulted"] = json!(true);
    value["initialStateDefaulted"] = json!(true);
    let saved = candidate::candidate(&draft(value.clone()));
    assert_eq!(saved.get("enabled"), None);
    assert_eq!(saved.get("initial_state"), None);
    value["enabled"] = json!(false);
    value["initialState"] = json!([2]);
    let saved = candidate::candidate(&draft(value));
    assert_eq!(
        saved.get("enabled"),
        Some(&json!(false)),
        "a disabled plant says so"
    );
    assert_eq!(
        saved.get("initial_state"),
        Some(&json!([2])),
        "an edited state is saved"
    );
}

#[test]
fn fields_the_editor_does_not_manage_survive_saving() {
    // Specification 47 §13.2: fields outside the editor (model validity) survive projection and
    // serialization unchanged, and an editable field is never restored from the saved extras.
    let mut value = first_order();
    value["preservedConfiguration"] =
        json!({"validity": {"output_range": [0, 1]}, "model": "stale"});
    let saved = candidate::candidate(&draft(value));
    assert_eq!(saved["validity"], json!({"output_range": [0, 1]}));
    assert_eq!(
        saved["model"],
        json!("first_order"),
        "the editable model wins"
    );
    assert_eq!(
        candidate::preserved(&draft(
            json!({"id": "x", "validity": 1, "uncertainties": []})
        )),
        draft(json!({"validity": 1}))
    );
    // A saved plant's whole table may ride along; an editable field cleared in the editor (here
    // the actuator) is not resurrected from it, and the edited name wins.
    let mut cleared = first_order();
    cleared["displayName"] = json!("Renamed");
    cleared["preservedConfiguration"] =
        json!({"name": "Stale name", "actuator": {"lag_s": 99}, "validity": 1});
    let saved = candidate::candidate(&draft(cleared));
    assert_eq!(
        saved.get("actuator"),
        None,
        "a cleared actuator stays cleared"
    );
    assert_eq!(saved["name"], json!("Renamed"));
    assert_eq!(saved["validity"], json!(1));
}

#[test]
fn the_signal_conversions_hold_one_finite_value_per_signal() {
    // Specification 46 §3.2: a plant step of whole positive milliseconds, and one conversion value
    // per command or feedback signal. Catches a length or finiteness rule missing.
    let mut value = first_order();
    value["stepMs"] = json!(0);
    value["inputScale"] = json!([1, 2]);
    value["outputOffset"] = json!([null]);
    assert_eq!(
        found(&checks::signal_engineering(&draft(value))),
        [
            (
                "step_ms".to_string(),
                "Plant step must be a positive whole number of milliseconds.".to_string()
            ),
            (
                "input_scale".to_string(),
                "Input scale needs exactly 1 value for the 1 command signal.".to_string()
            ),
            (
                "output_offset".to_string(),
                "Output offset must contain only finite numbers.".to_string()
            ),
        ]
    );
    let mut value = first_order();
    value["stepMs"] = json!(5);
    value["initialInput"] = json!([2.5]);
    assert!(checks::signal_engineering(&draft(value)).is_empty());
}

#[test]
fn the_plant_id_starts_with_a_letter_and_is_not_another_plants() {
    // Specification 47 §7 and specification 33's identity rule. Catches an invalid ID accepted, a
    // blank name accepted, or a plant refused its own saved ID.
    let issues = |id: &str, name: &str, others: &[&str]| {
        let others: Vec<String> = others.iter().map(|other| (*other).to_string()).collect();
        checks::identity(&draft(json!({"plantId": id, "displayName": name})), &others)
            .into_iter()
            .map(|issue| (issue.code, issue.message))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        issues("1 tank", " ", &["tank_level"]),
        [
            (
                "invalid_plant_id",
                "Start with a letter; then use letters, numbers, _ or -.".to_string()
            ),
            ("empty_display_name", "Enter a display name.".to_string()),
        ]
    );
    assert_eq!(
        issues("tank_level", "Tank", &["tank_level"]),
        [(
            "duplicate_plant_id",
            "Choose an ID that is not already used in this project.".to_string()
        )]
    );
    assert!(issues("tank-2_b", "Tank", &["tank_level"]).is_empty());
}

#[test]
fn optional_robot_joint_terms_and_labelled_starting_states() {
    // Specification 47 §21.1: the robot joint's gravity and friction terms may be omitted (they
    // default to 0); §6.2: the labelled starting states in canonical order, none for models that
    // keep a raw or scalar state. Catches an optional term made mandatory, or a starting-state label
    // that does not match its index.
    let joint = json!({
        "model": "robot_joint",
        "parameters": {"inertia_kg_m2": 2, "friction_smoothing_rad_s": 0.01},
        "initialState": [0, 0],
    });
    assert!(checks::dynamics(&draft(joint)).is_empty());
    let labels = |model: &str| -> Vec<&str> {
        tables::starting_state_fields(model)
            .iter()
            .map(|(label, _)| *label)
            .collect()
    };
    assert_eq!(labels("first_order"), ["Starting output"]);
    assert_eq!(labels("integrator"), ["Starting accumulated value"]);
    assert_eq!(labels("gravity_tank"), ["Starting level (m)"]);
    assert_eq!(
        labels("gravity_tank_cascade"),
        [
            "Starting upstream level (m)",
            "Starting downstream level (m)"
        ]
    );
    assert_eq!(
        labels("second_order"),
        ["Starting displacement", "Starting rate of change"]
    );
    assert_eq!(
        labels("robot_joint"),
        ["Starting angle (rad)", "Starting angular speed (rad/s)"]
    );
    for model in [
        "first_order_dead_time",
        "state_space",
        "static_gain",
        "piecewise_linear",
    ] {
        assert!(labels(model).is_empty(), "{model}");
    }
    assert_eq!(
        tables::starting_state_fields("robot_joint")[0].1,
        "Joint angle when simulation starts or resets. Enter radians, not degrees; 2π radians is one complete turn."
    );
}

#[test]
fn every_starting_point_proposes_its_exact_editable_values() {
    // Specification 47 §7 (the proposal table) and the robot-joint and cascade proposals. Catches
    // a changed seed value, a missing bias or a wrong model.
    let proposal = |id: &str| {
        let (model, parameters, initial_state, actuator) =
            tables::proposal(id).expect("a proposal");
        (
            model,
            Value::Object(parameters),
            Value::Array(initial_state),
            actuator,
        )
    };
    assert_eq!(
        proposal("tank"),
        (
            "integrator",
            json!({"gain": 1, "input_bias": 0, "output_bias": 0}),
            json!([0]),
            None
        )
    );
    assert_eq!(
        proposal("gravity_drained_tank"),
        (
            "gravity_tank",
            json!({"area_m2": 2, "outflow_coefficient_m2_sqrt_m_per_s": 0.5}),
            json!([4]),
            None
        )
    );
    assert_eq!(
        proposal("robot_joint"),
        (
            "robot_joint",
            json!({"inertia_kg_m2": 2, "gravity_torque_nm": 4, "viscous_friction_nm_s_per_rad": 0.2, "coulomb_friction_nm": 0.1, "friction_smoothing_rad_s": 0.01}),
            json!([0.5, 0]),
            None
        )
    );
    assert_eq!(
        proposal("temperature"),
        (
            "first_order_dead_time",
            json!({"gain": 1, "time_constant_s": 10, "dead_time_s": 1, "input_bias": 0, "output_bias": 0, "initial_delayed_input": 0}),
            json!([0]),
            None
        )
    );
    assert_eq!(
        proposal("motor_speed"),
        (
            "first_order",
            json!({"gain": 1, "time_constant_s": 1, "input_bias": 0, "output_bias": 0}),
            json!([0]),
            None
        )
    );
    assert_eq!(
        proposal("mechanical_position"),
        (
            "second_order",
            json!({"gain": 1, "natural_frequency_rad_s": 1, "damping_ratio": 0.7, "input_bias": 0, "output_bias": 0}),
            json!([0, 0]),
            None
        )
    );
    assert_eq!(
        proposal("static_scaling"),
        (
            "static_gain",
            json!({"gain": 1, "input_bias": 0, "output_bias": 0}),
            json!([]),
            None
        )
    );
    assert_eq!(
        proposal("nonlinear_characteristic"),
        (
            "piecewise_linear",
            json!({"input_points": [-1, 0, 1], "output_points": [-1, 0, 1]}),
            json!([]),
            None
        )
    );
    assert_eq!(
        proposal("gravity_tank_cascade"),
        (
            "gravity_tank_cascade",
            json!({"area_m2": 0.5, "outflow_coefficient_m2_sqrt_m_per_s": 0.02}),
            json!([0.25, 0.25]),
            None
        )
    );
    // Specification 47 §7: State space proposes one state, input and output.
    assert_eq!(
        proposal("custom_state_space"),
        (
            "state_space",
            json!({"a": [[0]], "b": [[1]], "c": [[1]], "d": [[0]]}),
            json!([0]),
            None
        )
    );
    assert!(tables::proposal("unknown").is_none());
}

#[test]
fn every_starting_point_says_why_it_is_reasonable_and_what_it_cannot_do() {
    // Specification 47 §7: a starting point is described as one the user must adapt; it must not
    // claim to reproduce the physical plant. Catches a starting point without its limitation.
    let mut help: Vec<(&str, &str, &str)> = tables::STARTING_POINTS
        .iter()
        .map(|point| (point.id, point.why_reasonable, point.limitation))
        .collect();
    help.push((
        tables::CUSTOM_STATE_SPACE,
        tables::CUSTOM_STATE_SPACE_HELP.0,
        tables::CUSTOM_STATE_SPACE_HELP.1,
    ));
    let ids: Vec<&str> = help.iter().map(|(id, _, _)| *id).collect();
    assert_eq!(
        ids,
        [
            "tank",
            "gravity_drained_tank",
            "gravity_tank_cascade",
            "temperature",
            "motor_speed",
            "mechanical_position",
            "robot_joint",
            "static_scaling",
            "nonlinear_characteristic",
            "custom_state_space",
        ]
    );
    for (id, why, limitation) in help {
        assert!(!why.is_empty(), "{id} explains why it is reasonable");
        let limitation = limitation.to_ascii_lowercase();
        assert!(
            ["cannot", "without", "omits", "assumes no"]
                .iter()
                .any(|word| limitation.contains(word)),
            "{id} states what it cannot reproduce"
        );
    }
}
