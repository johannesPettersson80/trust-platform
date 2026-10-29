//! The editor's edits and what it shows (specification 47 §6 to §12, §20.1), through `plant.edit`.

use serde_json::{json, Value};

use super::draft;
use crate::control::plant_handlers::editor::{edit, Answer, Project};

fn project(saved: &[(&str, &[&str])]) -> Project {
    Project {
        saved_ids: saved.iter().map(|(id, _)| (*id).to_string()).collect(),
        saved_feedback: saved
            .iter()
            .map(|(id, targets)| {
                (
                    (*id).to_string(),
                    targets.iter().map(|target| (*target).to_string()).collect(),
                )
            })
            .collect(),
        step_ms: Some(10.0),
    }
}

fn signals() -> Value {
    json!({
        "status": "ready",
        "commands": [{"symbol": "Main.Heater", "address": "%QL0", "dataType": "LREAL"}],
        "feedbacks": [
            {"symbol": "Main.Temperature", "address": "%IL0", "dataType": "LREAL"},
            {"symbol": "Main.Speed", "address": "%IL8", "dataType": "LREAL"},
        ],
    })
}

fn run(value: &Value, action: Option<Value>) -> Answer {
    edit(value, action.as_ref(), Some(&signals()), &project(&[])).expect("a well-formed request")
}

fn outcome(answer: &Answer) -> Value {
    answer.outcome.clone().unwrap_or(Value::Null)
}

fn saved_fopdt() -> Value {
    json!({
        "id": "plant:rt:heater", "persisted": true, "originalPlantId": "heater",
        "plantId": "heater", "displayName": "Heater", "description": "", "enabled": true,
        "runtimeId": "rt", "runtimeName": "Simulator", "startingPoint": "temperature",
        "model": "first_order_dead_time",
        "parameters": {"gain": 1, "time_constant_s": 10, "dead_time_s": 1},
        "initialState": [0], "commandSources": ["%QL0"], "feedbackTargets": ["%IL0"],
        "uncertainties": [{"enabled": true, "id": "noise", "target": "measurement", "mode": "add", "generator": "constant", "channel": 0, "value": 0.5}],
        "modelValuesTouched": false, "dirty": false,
    })
}

#[test]
fn a_new_plant_opens_at_the_temperature_starting_point_with_a_free_id() {
    // Specification 47 §7: the add flow proposes the temperature starting point; the suggested ID
    // must not be one the project already uses. Catches a suggestion that clashes with a saved
    // plant, or a new draft without its proposal.
    let seed = json!({"id": "draft:plant:rt", "runtimeId": "rt", "runtimeName": "Simulator", "persisted": false, "focusSection": "plant"});
    let answer = edit(
        &seed,
        Some(&json!({"kind": "open"})),
        None,
        &project(&[("tank_level", &[]), ("new_plant", &[])]),
    )
    .unwrap();
    let expected = draft(json!({
        "id": "draft:plant:rt", "runtimeId": "rt", "runtimeName": "Simulator", "persisted": false,
        "focusSection": "plant", "plantId": "new_plant_2", "displayName": "New simulated plant",
        "description": "", "enabled": true, "startingPoint": "temperature",
        "model": "first_order_dead_time",
        "parameters": {"gain": 1, "time_constant_s": 10, "dead_time_s": 1, "input_bias": 0, "output_bias": 0, "initial_delayed_input": 0},
        "initialState": [0], "modelValuesTouched": false, "commandSources": [], "feedbackTargets": [],
        "initialInput": [], "inputScale": [], "inputOffset": [], "outputScale": [], "outputOffset": [],
        "scenarioEvents": [], "dirty": true,
    }));
    assert_eq!(answer.draft, expected);
    assert_eq!(outcome(&answer), json!({"kind": "applied"}));
}

#[test]
fn an_opened_saved_plant_shows_its_defaults_without_changing_what_it_saves() {
    // Specification 47 §9: an omitted initial state shows as the model-sized zero vector and stays
    // omitted; conversions it does not set show their defaults. Catches an omission materialized,
    // or a zero vector of the wrong size.
    let seed = json!({
        "persisted": true, "originalPlantId": "arm", "plantId": "arm", "displayName": "Arm",
        "description": "", "enabled": true, "model": "second_order",
        "parameters": {"natural_frequency_rad_s": 1, "damping_ratio": 0.7}, "initialState": [],
        "initialStateDefaulted": true, "commandSources": ["%QL0"], "feedbackTargets": ["%IL0"], "dirty": false,
    });
    let answer = run(&seed, Some(json!({"kind": "open"})));
    assert_eq!(answer.draft["initialState"], json!([0, 0]));
    assert_eq!(answer.draft["startingPoint"], json!("mechanical_position"));
    assert_eq!(answer.draft["initialInput"], json!([0]));
    assert_eq!(answer.draft["inputScale"], json!([1]));
    assert_eq!(answer.draft["outputOffset"], json!([0]));
    assert_eq!(
        answer.draft["dirty"],
        json!(false),
        "opening changes nothing the user saved"
    );
    assert_eq!(
        answer.candidate.get("initial_state"),
        None,
        "the omission is kept"
    );
    // Every model's omitted state opens as its own number of zeros (specification 46 §7): none
    // for the static models, one or two by the model, A's rows for state space. A label edit
    // still saves the omission.
    for (model, parameters, zeros) in [
        ("static_gain", json!({"gain": 2}), 0),
        ("first_order", json!({"time_constant_s": 2}), 1),
        (
            "first_order_dead_time",
            json!({"time_constant_s": 2, "dead_time_s": 0.5}),
            1,
        ),
        ("integrator", json!({"gain": 2}), 1),
        (
            "gravity_tank",
            json!({"area_m2": 2, "outflow_coefficient_m2_sqrt_m_per_s": 0.5}),
            1,
        ),
        (
            "piecewise_linear",
            json!({"input_points": [0, 1], "output_points": [0, 1]}),
            0,
        ),
        (
            "robot_joint",
            json!({"inertia_kg_m2": 2, "friction_smoothing_rad_s": 0.01}),
            2,
        ),
        (
            "second_order",
            json!({"natural_frequency_rad_s": 2, "damping_ratio": 0.7}),
            2,
        ),
        (
            "state_space",
            json!({"a": [[0, 1], [-1, -1]], "b": [[0], [1]], "c": [[1, 0]], "d": [[0]]}),
            2,
        ),
    ] {
        let mut omitted = seed.clone();
        omitted["model"] = json!(model);
        omitted["parameters"] = parameters.clone();
        let opened = run(&omitted, Some(json!({"kind": "open"})));
        assert_eq!(
            opened.draft["initialState"],
            json!(vec![0; zeros]),
            "{model}"
        );
        let mut renamed = Value::Object(opened.draft.clone());
        renamed["displayName"] = json!("Renamed");
        let saved = run(&renamed, None).candidate;
        assert_eq!(saved["name"], json!("Renamed"), "{model}");
        assert_eq!(
            saved.get("initial_state"),
            None,
            "{model} keeps the omission"
        );
        assert_eq!(saved["parameters"], parameters, "{model}");
    }
}

#[test]
fn choosing_a_starting_point_asks_before_replacing_edited_or_saved_values() {
    // Specification 47 §7: untouched proposal values are replaced at once; edited ones, a saved
    // plant's, and the temperature example's listed replacements need confirmation. Catches edited
    // dynamics replaced without asking.
    let mut fresh = saved_fopdt();
    fresh["persisted"] = json!(false);
    let applied = run(
        &fresh,
        Some(json!({"kind": "choose_starting_point", "startingPoint": "motor_speed"})),
    );
    assert_eq!(outcome(&applied), json!({"kind": "applied"}));
    assert_eq!(applied.draft["model"], json!("first_order"));
    assert_eq!(
        applied.draft["parameters"],
        json!({"gain": 1, "time_constant_s": 1, "input_bias": 0, "output_bias": 0})
    );
    assert_eq!(
        applied.draft["commandSources"],
        json!(["%QL0"]),
        "bindings stay"
    );
    assert_eq!(
        applied.draft["uncertainties"], fresh["uncertainties"],
        "effects stay"
    );

    let mut touched = fresh.clone();
    touched["modelValuesTouched"] = json!(true);
    let asked = run(
        &touched,
        Some(json!({"kind": "choose_starting_point", "startingPoint": "motor_speed"})),
    );
    assert_eq!(
        outcome(&asked),
        json!({"kind": "confirm", "message": "Replace your edited Temperature or slow process dynamics with the Motor speed or first-order process starting values?"})
    );
    assert_eq!(
        asked.draft,
        draft(touched),
        "nothing changes before confirmation"
    );
    let saved = run(
        &saved_fopdt(),
        Some(json!({"kind": "choose_starting_point", "startingPoint": "tank"})),
    );
    assert_eq!(
        outcome(&saved)["kind"],
        json!("confirm"),
        "a saved plant asks too"
    );
    let example = run(
        &fresh,
        Some(json!({"kind": "choose_starting_point", "startingPoint": "temperature_example"})),
    );
    assert!(
        outcome(&example)["message"]
            .as_str()
            .is_some_and(|message| message.starts_with("Replace model values, starting state, actuator settings and signal conversions with the temperature teaching example?")),
        "{:?}",
        example.outcome
    );
}

#[test]
fn the_temperature_example_maps_heater_percent_and_celsius_exactly() {
    // Specification 47 §7.1: gain = rise / 100, input bias 0, output bias = resting temperature,
    // initial state = starting temperature, initial delayed input 0, actuator 0–100 from 0, identity
    // conversions per port; identity, bindings and effects are kept. The physical values invert
    // exactly. Catches a lossy or asymmetric mapping.
    let answer = run(
        &saved_fopdt(),
        Some(json!({"kind": "apply_starting_point", "startingPoint": "temperature_example"})),
    );
    let applied = &answer.draft;
    assert_eq!(
        applied["parameters"],
        json!({"gain": 1, "time_constant_s": 30, "dead_time_s": 2, "input_bias": 0, "output_bias": 20, "initial_delayed_input": 0})
    );
    assert_eq!(applied["initialState"], json!([20]));
    assert_eq!(
        applied["actuator"],
        json!({"minimum": 0, "maximum": 100, "initial_applied": 0})
    );
    assert_eq!(
        (
            applied["initialInput"].clone(),
            applied["inputScale"].clone(),
            applied["inputOffset"].clone()
        ),
        (json!([0]), json!([1]), json!([0]))
    );
    assert_eq!(
        (
            applied["outputScale"].clone(),
            applied["outputOffset"].clone()
        ),
        (json!([1]), json!([0]))
    );
    assert_eq!(applied["plantId"], json!("heater"));
    assert_eq!(applied["uncertainties"], saved_fopdt()["uncertainties"]);
    let temperature = &answer.view["temperature"];
    assert_eq!(
        temperature["values"],
        json!({"resting_temperature": 20, "full_power_rise": 100, "time_constant_s": 30, "dead_time_s": 2})
    );
    assert_eq!(temperature["nonEquilibrium"], json!(false));
    assert_eq!(
        temperature["fields"][1],
        json!({
            "key": "full_power_rise",
            "label": "Temperature rise at full power (°C)",
            "help": "The change above the resting temperature at 100% power. A negative value describes cooling."
        })
    );
    assert_eq!(
        temperature["commandAssumption"],
        json!("PLC 0–100 = heater 0–100%")
    );
    assert_eq!(
        temperature["feedbackAssumption"],
        json!("PLC feedback = temperature in °C")
    );
    assert_eq!(
        temperature["heaterRange"],
        json!("Heater range: 0–100%. Adjust limits in Actuator & command.")
    );

    // A 50 °C rise is gain 0.5 with the resting temperature kept; with an input bias of 10 a new
    // resting temperature of 25 °C is output bias 25 + 0.5 × 10 = 30.
    let mut example = Value::Object(answer.draft.clone());
    example = Value::Object(
        run(
            &example,
            Some(
                json!({"kind": "set_temperature_value", "field": "full_power_rise", "text": "50"}),
            ),
        )
        .draft,
    );
    assert_eq!(example["parameters"]["gain"], json!(0.5));
    assert_eq!(example["parameters"]["output_bias"], json!(20));
    example["parameters"]["input_bias"] = json!(10);
    let rested = run(
        &example,
        Some(
            json!({"kind": "set_temperature_value", "field": "resting_temperature", "text": " 25 "}),
        ),
    );
    assert_eq!(rested.draft["parameters"]["output_bias"], json!(30));
    assert_eq!(
        rested.view["temperature"]["values"]["resting_temperature"],
        json!(25)
    );
    assert_eq!(
        rested.view["temperature"]["equation"][0],
        json!("Settled temperature (°C) = 30 + 0.5 × (power − 10).")
    );
    let cooling = run(
        &example,
        Some(json!({"kind": "set_temperature_value", "field": "full_power_rise", "text": "-50"})),
    );
    assert_eq!(
        cooling.draft["parameters"]["gain"],
        json!(-0.5),
        "a negative rise describes cooling"
    );
    // Each typed value is read as a number first: empty or unreadable text is refused with the
    // field's label, before the response time and delay rules.
    for (field, text, message) in [
        (
            "time_constant_s",
            "0",
            "Response time must be greater than 0 s.",
        ),
        ("dead_time_s", "-1", "Delay must be 0 s or greater."),
        (
            "time_constant_s",
            "abc",
            "Response time (s) must be a finite number.",
        ),
        (
            "resting_temperature",
            " ",
            "Resting temperature (°C) must be a finite number.",
        ),
    ] {
        let refused = run(
            &example,
            Some(json!({"kind": "set_temperature_value", "field": field, "text": text})),
        );
        assert_eq!(
            outcome(&refused),
            json!({"kind": "rejected", "message": message})
        );
        assert_eq!(
            Value::Object(refused.draft),
            example,
            "a refused value changes nothing"
        );
    }
}

#[test]
fn the_temperature_units_are_confirmed_for_the_signals_they_were_confirmed_for() {
    // Specification 47 §7.1: the example's unit assumptions need confirmation before saving, and the
    // confirmation covers the bindings and conversions it was given for. Catches a Save allowed
    // without it, or a confirmation that survives a changed conversion or binding.
    let unconfirmed = |answer: &Answer| {
        answer
            .issues
            .iter()
            .any(|issue| issue.code == "temperature_units_unconfirmed")
    };
    let example = run(
        &saved_fopdt(),
        Some(json!({"kind": "apply_starting_point", "startingPoint": "temperature_example"})),
    );
    assert!(unconfirmed(&example));
    assert_eq!(example.view["temperature"]["unitsConfirmed"], json!(false));
    let confirmed = run(
        &Value::Object(example.draft.clone()),
        Some(json!({"kind": "confirm_temperature_units", "confirmed": true})),
    );
    assert!(!unconfirmed(&confirmed), "{:?}", confirmed.issues);
    assert_eq!(confirmed.view["temperature"]["unitsConfirmed"], json!(true));
    let mut scaled = Value::Object(confirmed.draft.clone());
    scaled["outputScale"] = json!([2]);
    assert!(unconfirmed(&run(&scaled, None)), "a changed conversion");
    let rebound = run(
        &Value::Object(confirmed.draft.clone()),
        Some(json!({"kind": "bind_port", "direction": "feedback", "index": 0, "address": "%IL8"})),
    );
    assert!(unconfirmed(&rebound), "a changed binding");
    let withdrawn = run(
        &Value::Object(confirmed.draft.clone()),
        Some(json!({"kind": "confirm_temperature_units", "confirmed": false})),
    );
    assert!(unconfirmed(&withdrawn));
    let again = run(
        &Value::Object(confirmed.draft.clone()),
        Some(json!({"kind": "apply_starting_point", "startingPoint": "temperature_example"})),
    );
    assert!(unconfirmed(&again), "applying the example again asks again");
}

#[test]
fn binding_a_port_checks_the_address_and_names_the_port() {
    // Specification 47 §8: picker selection and manual entry report failures at the affected row.
    // Catches an address bound without its check, or a failure without the port's name.
    let fresh = saved_fopdt();
    let refused = edit(
        &fresh,
        Some(&json!({"kind": "bind_port", "direction": "command", "index": 0, "address": "%ql0"})),
        Some(&json!({"status": "compile_error", "commands": [], "feedbacks": [], "error": "x"})),
        &project(&[]),
    )
    .unwrap();
    assert_eq!(
        outcome(&refused),
        json!({"kind": "rejected", "message": "Command / PLC output %QL0: Compile the project successfully before entering a PLC signal."})
    );
    let bound = run(
        &fresh,
        Some(
            json!({"kind": "bind_port", "direction": "feedback", "index": 0, "address": " %il8 "}),
        ),
    );
    assert_eq!(
        bound.draft["feedbackTargets"],
        json!(["%IL8"]),
        "the compiled spelling is bound"
    );
    let other = edit(
        &fresh,
        Some(&json!({"kind": "bind_port", "direction": "feedback", "index": 0, "address": "%IL8"})),
        Some(&signals()),
        &project(&[("heater", &["%IL0"]), ("pump", &["%IL8"])]),
    )
    .unwrap();
    assert_eq!(
        outcome(&other),
        json!({"kind": "rejected", "message": "Feedback / PLC input %IL8: Choose a different %I input; %IL8 is already used by another plant."})
    );
}

#[test]
fn a_saved_plant_may_keep_its_own_id_and_feedback() {
    // Specification 47 §7: the ID must be unique among the other plants; a plant's own saved ID and
    // targets are not another plant's. Catches an edited plant refusing its own ID.
    let answer = edit(
        &saved_fopdt(),
        None,
        Some(&signals()),
        &project(&[("heater", &["%IL0"])]),
    )
    .unwrap();
    assert!(answer.issues.is_empty(), "{:?}", answer.issues);
    let mut copy = saved_fopdt();
    copy["originalPlantId"] = json!(null);
    copy["persisted"] = json!(false);
    let clash = edit(
        &copy,
        None,
        Some(&signals()),
        &project(&[("heater", &["%IL0"])]),
    )
    .unwrap();
    let codes: Vec<&str> = clash.issues.iter().map(|issue| issue.code).collect();
    assert_eq!(codes, ["duplicate_plant_id", "invalid_port_binding"]);
}

#[test]
fn a_state_space_resize_asks_before_removing_and_a_paste_fills_the_matrix() {
    // Specification 47 §9. Catches a removal without confirmation, or a paste of the wrong size
    // accepted.
    let model = json!({
        "plantId": "ss", "displayName": "SS", "model": "state_space",
        "parameters": {"a": [[0]], "b": [[1, 2]], "c": [[1]], "d": [[0, 0]], "input_delays_s": [0, 3]},
        "initialState": [0], "commandSources": ["%QL0", "%QL0"], "feedbackTargets": ["%IL0"],
    });
    let shrink = json!({"kind": "resize_state_space", "states": 1, "inputs": 1, "outputs": 1});
    let asked = run(&model, Some(shrink.clone()));
    assert_eq!(
        outcome(&asked),
        json!({"kind": "confirm", "message": "Reducing inputs removes matrix columns and 1 nonzero input delay."})
    );
    let mut confirmed = shrink;
    confirmed["confirmed"] = json!(true);
    let resized = run(&model, Some(confirmed));
    assert_eq!(resized.draft["parameters"]["b"], json!([[1]]));
    assert_eq!(resized.draft["parameters"]["input_delays_s"], json!([0]));
    assert_eq!(resized.draft["modelValuesTouched"], json!(true));
    let pasted = run(
        &model,
        Some(json!({"kind": "paste_matrix", "matrix": "b", "text": "4\t5"})),
    );
    assert_eq!(pasted.draft["parameters"]["b"], json!([[4, 5]]));
    let cell = run(
        &model,
        Some(json!({"kind": "paste_matrix", "matrix": "b", "text": "7", "row": 0, "column": 1})),
    );
    assert_eq!(
        cell.draft["parameters"]["b"],
        json!([[1, 7]]),
        "one number fills the cell it was pasted into"
    );
    let wrong = run(
        &model,
        Some(json!({"kind": "paste_matrix", "matrix": "b", "text": "1 2 3"})),
    );
    assert_eq!(
        outcome(&wrong),
        json!({"kind": "rejected", "message": "B paste must contain 1 rows and 2 columns."})
    );
}

#[test]
fn a_state_space_resize_keeps_the_dimensions_it_does_not_name() {
    // Specification 47 §9: a change names the dimension it changes and keeps the draft's others,
    // so two dimensions changed one after the other both apply; growing removes nothing, so it
    // asks nothing. Catches a resize that resets a dimension left out to an older value, or that
    // refuses an action naming one dimension.
    let model = json!({
        "plantId": "ss", "displayName": "SS", "model": "state_space",
        "parameters": {"a": [[0]], "b": [[1]], "c": [[1]], "d": [[0]], "input_delays_s": [0]},
        "initialState": [0], "commandSources": ["%QL0"], "feedbackTargets": ["%IL0"],
    });
    // Rows and columns of each matrix: B is states by inputs, C outputs by states, D outputs by
    // inputs.
    let shape = |answer: &Answer, matrix: &str| {
        let rows = answer.draft["parameters"][matrix]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let columns = rows.first().and_then(Value::as_array).map_or(0, Vec::len);
        (rows.len(), columns)
    };
    let inputs = run(
        &model,
        Some(json!({"kind": "resize_state_space", "inputs": 2})),
    );
    assert_eq!(
        outcome(&inputs)["kind"],
        json!("applied"),
        "{:?}",
        outcome(&inputs)
    );
    assert_eq!((shape(&inputs, "b"), shape(&inputs, "c")), ((1, 2), (1, 1)));
    let outputs = run(
        &Value::Object(inputs.draft.clone()),
        Some(json!({"kind": "resize_state_space", "outputs": 2})),
    );
    assert_eq!(
        outcome(&outputs)["kind"],
        json!("applied"),
        "{:?}",
        outcome(&outputs)
    );
    assert_eq!(
        (
            shape(&outputs, "b"),
            shape(&outputs, "c"),
            shape(&outputs, "d")
        ),
        ((1, 2), (2, 1), (2, 2)),
        "the inputs the first change made are kept"
    );
    assert_eq!(outputs.draft["parameters"]["b"][0][0], json!(1));
}

#[test]
fn effects_and_events_are_added_with_their_defaults() {
    // Specification 47 §11 and §12: an added effect takes the preset's values and a free ID; a
    // duplicate is included under a new ID; an added event sets state 0 to 0 at 0 s. Catches a
    // duplicated effect ID or an added event without its fields.
    let base = saved_fopdt();
    let added = run(
        &base,
        Some(json!({"kind": "add_effect", "preset": "drift"})),
    );
    assert_eq!(outcome(&added), json!({"kind": "applied", "index": 1}));
    assert_eq!(added.draft["uncertainties"][1]["id"], json!("effect_1"));
    assert_eq!(
        added.draft["uncertainties"][1]["generator"],
        json!("random_walk")
    );
    let duplicated = run(
        &Value::Object(added.draft.clone()),
        Some(json!({"kind": "duplicate_effect", "index": 0})),
    );
    assert_eq!(
        duplicated.draft["uncertainties"][1]["id"],
        json!("effect_2")
    );
    assert_eq!(duplicated.draft["uncertainties"][1]["value"], json!(0.5));
    assert_eq!(
        duplicated.draft["uncertainties"][2]["id"],
        json!("effect_1"),
        "the rows after it move down"
    );
    let event = run(&base, Some(json!({"kind": "add_event"})));
    assert_eq!(
        event.draft["scenarioEvents"],
        json!([{"enabled": true, "atMs": 0, "kind": "set_state", "channel": 0, "value": 0}])
    );
    let parameter = run(
        &Value::Object(event.draft.clone()),
        Some(json!({"kind": "set_event_kind", "index": 0, "eventKind": "set_parameter"})),
    );
    assert_eq!(
        parameter.draft["scenarioEvents"][0]["parameter"],
        json!("gain"),
        "the model's first parameter"
    );
    assert!(edit(&base, Some(&json!({"kind": "launch"})), None, &project(&[])).is_err());
}

#[test]
fn rows_are_removed_moved_and_copied_with_what_the_editor_shows_of_them() {
    // Specification 47 §11 and §12: effects apply in their order, so neighbours may change places;
    // events change places only when due at the same time; a duplicated event is an exact copy
    // after the original. The answer's view follows the rows. Catches a move that changes the time
    // order of events, a view left describing the old rows, or the wrong row removed.
    let mut base = saved_fopdt();
    base["uncertainties"] = json!([
        {"enabled": true, "id": "noise", "target": "measurement", "mode": "add", "generator": "constant", "channel": 0, "value": 0.5},
        {"enabled": true, "id": "limit", "target": "measurement", "mode": "replace", "generator": "clip", "channel": 0, "minimum": 0, "maximum": 1},
    ]);
    base["scenarioEvents"] = json!([
        {"enabled": true, "atMs": 0, "kind": "reset_plant"},
        {"enabled": true, "atMs": 0, "kind": "stick_actuator"},
        {"enabled": true, "atMs": 5, "kind": "release_actuator"},
    ]);
    let moved = run(
        &base,
        Some(json!({"kind": "move_effect", "index": 0, "delta": 1})),
    );
    assert_eq!(outcome(&moved), json!({"kind": "applied", "index": 1}));
    assert_eq!(moved.draft["uncertainties"][0]["id"], json!("limit"));
    assert_eq!(moved.view["effects"][0]["preset"], json!("custom"));
    assert_eq!(moved.view["effects"][1]["label"], json!("Bias"));
    let removed = run(&base, Some(json!({"kind": "remove_effect", "index": 0})));
    assert_eq!(
        removed.draft["uncertainties"],
        json!([base["uncertainties"][1]])
    );
    assert_eq!(removed.view["effects"].as_array().map(Vec::len), Some(1));

    let swapped = run(
        &base,
        Some(json!({"kind": "move_event", "index": 1, "delta": -1})),
    );
    assert_eq!(outcome(&swapped), json!({"kind": "applied", "index": 0}));
    assert_eq!(
        swapped.draft["scenarioEvents"][0]["kind"],
        json!("stick_actuator")
    );
    let refused = run(
        &base,
        Some(json!({"kind": "move_event", "index": 1, "delta": 1})),
    );
    assert_eq!(
        outcome(&refused),
        json!({"kind": "rejected", "message": "Only events due at the same time can change places."})
    );
    assert_eq!(refused.draft["scenarioEvents"], base["scenarioEvents"]);
    let copied = run(&base, Some(json!({"kind": "duplicate_event", "index": 2})));
    assert_eq!(outcome(&copied), json!({"kind": "applied", "index": 3}));
    assert_eq!(copied.draft["scenarioEvents"][3], base["scenarioEvents"][2]);
    assert_eq!(copied.view["events"].as_array().map(Vec::len), Some(4));
    let dropped = run(&base, Some(json!({"kind": "remove_event", "index": 0})));
    assert_eq!(
        dropped.draft["scenarioEvents"][0]["kind"],
        json!("stick_actuator")
    );
    assert!(edit(
        &base,
        Some(&json!({"kind": "remove_event", "index": 7})),
        None,
        &project(&[])
    )
    .is_err());
}

#[test]
fn the_view_groups_fields_and_summarizes_the_optional_settings() {
    // Specification 47 §6.2 and §9: FOPDT shows its response fields with the starting value apart
    // and the rest under Model details; the optional settings stay summarized. Catches a field shown
    // twice or a summary that hides configured values.
    let mut value = saved_fopdt();
    value["actuator"] = json!({"minimum": 0, "maximum": 100, "initial_applied": 0});
    value["inputScale"] = json!([2]);
    value["initialInput"] = json!([0.5]);
    value["uncertainties"] = json!([
        {"enabled": true, "target": "command"},
        {"enabled": false, "target": "measurement"},
        {"enabled": true, "target": "state"},
    ]);
    value["scenarioEvents"] = json!([
        {"enabled": true, "atMs": 1500, "kind": "reset_plant"},
        {"enabled": false, "atMs": 0, "kind": "reset_plant"},
        {"enabled": true, "atMs": 2500, "kind": "reset_plant"},
    ]);
    value["stepMs"] = json!(5);
    let view = run(&value, None).view;
    let keys = |list: &Value| -> Vec<String> {
        list.as_array()
            .unwrap()
            .iter()
            .map(|field| field["key"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(
        keys(&view["dynamics"]["fields"]),
        ["gain", "time_constant_s", "dead_time_s"]
    );
    assert_eq!(
        keys(&view["dynamics"]["details"]),
        [
            "input_bias",
            "output_bias",
            "initial_delayed_input",
            "initial_state"
        ]
    );
    assert_eq!(
        view["dynamics"]["fields"][1]["label"],
        json!("Response time (s)")
    );
    assert_eq!(view["startingState"]["kind"], json!("scalar"));
    assert_eq!(
        view["stepText"],
        json!("Plant step: 5 ms. Saving does not start or restart simulation.")
    );
    assert_eq!(
        view["summaries"]["actuator"],
        json!("Range 0–100 · 3 configured settings")
    );
    assert_eq!(
        view["summaries"]["signals"],
        json!("1 scaling or offset value changed · 1 nonzero initial command · 5 ms plant step")
    );
    assert_eq!(
        view["summaries"]["realism"],
        json!("3 configured effects · 2 enabled effects · Actuator 1 · Process 1 · Sensor 1")
    );
    assert_eq!(
        view["summaries"]["scenarios"],
        json!("3 configured events · 2 included events · first at 1.5 s")
    );
    assert_eq!(
        view["parameters"],
        json!(["gain", "time_constant_s", "dead_time_s"]),
        "parameters in the model's field order"
    );
    value["scenarioEvents"][0]["atMs"] = json!(-1);
    assert_eq!(
        run(&value, None).view["summaries"]["scenarios"],
        json!("3 configured events · 2 included events · Correct event time in Scenarios")
    );
}

#[test]
fn state_and_sensor_events_name_the_models_quantities() {
    // Specification 47 §6.2: event targets are named quantities from the model's state and output
    // descriptors, without the starting-condition prefix. Catches bare channel numbers.
    let joint = json!({
        "model": "robot_joint", "parameters": {}, "initialState": [0.5, 0],
        "commandSources": ["%QL0"], "feedbackTargets": ["%IL0", "%IL8"],
        "scenarioEvents": [
            {"enabled": true, "atMs": 0, "kind": "set_state", "channel": 0, "value": 0},
            {"enabled": true, "atMs": 0, "kind": "set_sensor_bias", "channel": 1, "value": 0},
        ],
    });
    let view = run(&joint, None).view;
    let labels = |event: usize| -> Vec<String> {
        view["events"][event]["channelOptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|option| option["label"].as_str().unwrap().to_string())
            .collect()
    };
    assert_eq!(labels(0), ["Angle (rad)", "Angular speed (rad/s)"]);
    assert_eq!(labels(1), ["Joint angle (rad)", "Angular speed (rad/s)"]);
    assert_eq!(
        view["events"][1]["channelLabel"],
        json!("Measured quantity")
    );
    assert_eq!(
        view["events"][0]["canMoveDown"],
        json!(true),
        "equal times may change places"
    );
}

#[test]
fn the_draft_to_be_saved_is_judged_by_the_runtime_too() {
    // Specification 47 §13.1: the runtime's complete validation stays authoritative. A plant step of
    // 9,223,372,036,855 ms is a positive whole number (the editor's rule) but longer than the
    // runtime's time range (specification 46 §3.2). Catches a candidate the runtime refuses shown
    // as ready to save.
    let root = std::env::temp_dir().join(format!(
        "trust-plant-edit-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let service = crate::control::PlantControlService::default();
    let context = crate::control::plant_handlers::PlantHandlerContext::offline(&root, &service);
    let project_id = crate::control::plant_handlers::project_identity(&root);
    let mut value = saved_fopdt();
    value["persisted"] = json!(false);
    value["originalPlantId"] = json!(null);
    value["stepMs"] = json!(9_223_372_036_855_u64);
    let request = |draft: &Value| {
        json!({
            "contract": "trust.plant", "schema_version": 1, "project_id": project_id,
            "draft": draft, "signals": signals(),
        })
    };
    let response =
        crate::control::plant_handlers::edit::handle_plant_edit(7, Some(request(&value)), &context);
    let result = response.result.expect("plant.edit answers");
    assert_eq!(result["operation"], json!("plant.edit"));
    assert_eq!(result["can_save"], json!(false));
    let issues = result["issues"].as_array().unwrap();
    assert_eq!(issues.len(), 1, "{issues:?}");
    assert_eq!(issues[0]["code"], json!("validation_failed"));
    assert_eq!(issues[0]["target"], json!({"kind": "configuration"}));
    assert!(
        issues[0]["message"]
            .as_str()
            .unwrap()
            .contains("plants.step_ms must fit a positive runtime duration"),
        "{issues:?}"
    );
    value["stepMs"] = json!(100);
    let ready =
        crate::control::plant_handlers::edit::handle_plant_edit(8, Some(request(&value)), &context);
    let result = ready.result.expect("plant.edit answers");
    assert_eq!(result["can_save"], json!(true), "{result}");
    assert_eq!(result["candidate"]["step_ms"], json!(100));
    std::fs::remove_dir_all(root).unwrap();
}

fn typed(draft: &Value, key: &str, text: &str) -> Answer {
    run(
        draft,
        Some(json!({"kind": "set_dynamics_text", "key": key, "text": text})),
    )
}

// N1 (29 October 2026): the text typed into a Dynamics field is read by `plant.edit`
// (specification 47 §20.1), not by the extension. Each value is worked out by hand from
// JavaScript's `Number`, the rule the extension applied before: `0x10` is 16, `1e3` is 1000,
// surrounding spaces are ignored, and unreadable text holds `null`, which the checks name. Catches
// a reader that refuses hexadecimal or exponent text, keeps a number for unreadable text, or keeps
// a value in a field that was emptied.
#[test]
fn text_typed_into_a_number_field_is_read_by_the_runtime() {
    let heater = saved_fopdt();
    for (text, gain) in [
        ("0x10", json!(16)),
        ("1e3", json!(1000)),
        (" 2.5 ", json!(2.5)),
    ] {
        let answer = typed(&heater, "gain", text);
        assert_eq!(outcome(&answer)["kind"], json!("applied"), "{text}");
        assert_eq!(answer.draft["parameters"]["gain"], gain, "{text}");
        assert_eq!(answer.draft["modelValuesTouched"], json!(true), "{text}");
    }
    let unreadable = typed(&heater, "gain", "abc");
    assert_eq!(unreadable.draft["parameters"]["gain"], Value::Null);
    assert!(
        super::found(&unreadable.issues).contains(&(
            "parameters.gain".to_string(),
            "Gain must contain finite numbers.".to_string()
        )),
        "{:?}",
        unreadable.issues
    );
    assert!(!unreadable.can_save());
    let emptied = typed(&heater, "gain", "  ");
    assert!(
        !emptied.draft["parameters"]
            .as_object()
            .expect("parameters")
            .contains_key("gain"),
        "{:?}",
        emptied.draft
    );
    assert_eq!(emptied.draft["parameters"]["time_constant_s"], json!(10));
}

// A list or matrix field holds the JSON value of its text, else the text itself, so that the
// checks can name it; an emptied starting state is an empty list, and a field the model does not
// have is a malformed request. Catches a reader that drops unreadable text or writes a field of
// another model.
#[test]
fn text_typed_into_a_list_field_is_its_json_value_else_the_text() {
    let mut model = saved_fopdt();
    model["model"] = json!("state_space");
    model["parameters"] =
        json!({"a": [[0, 1], [-1, 0]], "b": [[0], [1]], "c": [[1, 0]], "d": [[0]]});
    model["initialState"] = json!([0, 0]);

    assert_eq!(
        typed(&model, "a", "[[0, 1], [-2, 0]]").draft["parameters"]["a"],
        json!([[0, 1], [-2, 0]])
    );
    assert_eq!(
        typed(&model, "input_delays_s", " [0.5] ").draft["parameters"]["input_delays_s"],
        json!([0.5])
    );
    assert_eq!(
        typed(&model, "a", "0 1; -2 0").draft["parameters"]["a"],
        json!("0 1; -2 0")
    );
    assert_eq!(
        typed(&model, "initial_state", "").draft["initialState"],
        json!([])
    );
    let Err(error) = edit(
        &model,
        Some(&json!({"kind": "set_dynamics_text", "key": "gain", "text": "1"})),
        Some(&signals()),
        &project(&[]),
    ) else {
        panic!("state space has no gain field");
    };
    assert!(error.contains("names no field gain"), "{error}");
}
