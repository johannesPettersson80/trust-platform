//! The imperfection and scenario editors' rules (specification 47 §6.2, §11, §12; specification 46
//! §3.5, §3.6, §10).

use serde_json::{json, Map, Value};

use super::{draft, found};
use crate::control::plant_handlers::editor::vocabulary;
use crate::control::plant_handlers::editor::{effects, events};

fn with_effects(rows: Value, parameters: Value) -> Map<String, Value> {
    draft(json!({"model": "first_order", "parameters": parameters, "uncertainties": rows}))
}

fn row(value: Value) -> Map<String, Value> {
    draft(value)
}

#[test]
fn backlash_is_a_process_output_play_with_a_positive_width() {
    // Specification 46 §10.1: backlash acts on a process output in Replace mode with a width above
    // 0. Catches backlash accepted on a measurement, or a zero width accepted.
    let mut valid = effects::fresh(&[]);
    for (key, value) in [
        ("id", json!("gear-clearance")),
        ("target", json!("process_output")),
        ("mode", json!("replace")),
        ("generator", json!("backlash")),
        ("width", json!(0.05)),
    ] {
        valid.insert(key.into(), value);
    }
    assert_eq!(vocabulary::generator_fields("backlash"), ["width"]);
    assert_eq!(
        vocabulary::uncertainty_field_label("width"),
        "Backlash width"
    );
    let rows = json!([valid.clone()]);
    assert!(effects::checks(&with_effects(rows.clone(), json!({}))).is_empty());
    assert_eq!(
        effects::persisted(&with_effects(rows, json!({}))),
        [json!({
            "id": "gear-clearance", "target": "process_output", "mode": "replace",
            "generator": "backlash", "channel": 0, "start_s": 0, "width": 0.05,
        })],
        "only the generator's own values are saved"
    );
    let mut on_measurement = valid.clone();
    on_measurement.insert("target".into(), json!("measurement"));
    let mut zero_width = valid;
    zero_width.insert("id".into(), json!("zero-clearance"));
    zero_width.insert("width".into(), json!(0));
    assert_eq!(
        found(&effects::checks(&with_effects(
            json!([on_measurement, zero_width]),
            json!({})
        ))),
        [
            (
                "uncertainties[0].generator".to_string(),
                "Backlash requires Process output and Replace mode.".to_string()
            ),
            (
                "uncertainties[1].width".to_string(),
                "Backlash width must be greater than 0.".to_string()
            ),
        ]
    );
}

#[test]
fn every_included_effect_is_checked_and_saved_in_order() {
    // Specification 46 §3.5 and §10.1; specification 47 §11 (order is behaviour). Catches an
    // invalid ID accepted, a zero dropout duration accepted, or an excluded effect saved.
    assert_eq!(
        vocabulary::generator_fields("normal_approx_white"),
        ["mean", "standard_deviation"]
    );
    assert_eq!(
        vocabulary::generator_fields("dropout"),
        [
            "rate_per_s",
            "duration_s",
            "dropout_policy",
            "replacement_value"
        ]
    );
    let mut bad_id = effects::fresh(&[]);
    bad_id.insert("id".into(), json!("bad id"));
    let mut dropout = effects::fresh(&[bad_id.clone()]);
    for (key, value) in [
        ("id", json!("effect_2")),
        ("generator", json!("dropout")),
        ("target", json!("measurement")),
        ("mode", json!("replace")),
        ("rate_per_s", json!(1)),
        ("duration_s", json!(0)),
        ("dropout_policy", json!("hold_last")),
    ] {
        dropout.insert(key.into(), value);
    }
    assert_eq!(
        found(&effects::checks(&with_effects(
            json!([bad_id, dropout]),
            json!({"gain": 1})
        ))),
        [
            (
                "uncertainties[0].id".to_string(),
                "Start with a letter; then use letters, numbers, _ or -.".to_string()
            ),
            (
                "uncertainties[1].duration_s".to_string(),
                "Duration must be greater than 0 s.".to_string()
            ),
        ]
    );
    let mut bias = effects::fresh(&[]);
    bias.insert("id".into(), json!("bias"));
    bias.insert("value".into(), json!(2));
    let mut excluded = effects::fresh(&[bias.clone()]);
    excluded.insert("id".into(), json!("disabled"));
    excluded.insert("enabled".into(), json!(false));
    assert_eq!(
        effects::persisted(&with_effects(json!([bias, excluded]), json!({}))),
        [
            json!({"id": "bias", "target": "measurement", "mode": "add", "generator": "constant", "channel": 0, "start_s": 0, "value": 2})
        ]
    );
}

#[test]
fn every_preset_is_a_complete_valid_effect() {
    // Specification 47 §11: the Add effect chooser uses the existing preset types, and each preset
    // is valid with its visible defaults; the displayed order is the execution order. Catches a
    // preset with a hidden required field, or presets saved out of order.
    assert_eq!(
        row(json!(effects::apply_preset(
            "gaussian_noise",
            &row(json!({"id": "noise", "enabled": true}))
        ))),
        row(json!({
            "id": "noise", "enabled": true, "target": "measurement", "channel": 0, "mode": "add",
            "generator": "normal_approx_white", "mean": 0, "standard_deviation": 0.01, "start_s": 0,
        }))
    );
    assert_eq!(
        effects::apply_preset(
            "quantization",
            &row(json!({"id": "quant", "enabled": true}))
        ),
        row(
            json!({"id": "quant", "enabled": true, "target": "measurement", "channel": 0, "mode": "replace", "generator": "quantise", "quantum": 0.01, "start_s": 0})
        )
    );
    assert_eq!(
        effects::apply_preset(
            "stuck_sensor",
            &row(json!({"id": "stuck", "enabled": true}))
        ),
        row(
            json!({"id": "stuck", "enabled": true, "target": "measurement", "channel": 0, "mode": "hold", "generator": "constant", "start_s": 0})
        )
    );
    let presets = [
        "bias",
        "drift",
        "gaussian_noise",
        "quantization",
        "sample_and_hold",
        "dropout",
        "stuck_sensor",
        "outlier",
    ];
    let rows: Vec<Map<String, Value>> = presets
        .iter()
        .map(|preset| effects::apply_preset(preset, &row(json!({"id": preset, "enabled": true}))))
        .collect();
    for (preset, effect) in presets.iter().zip(&rows) {
        assert!(
            effects::checks(&with_effects(json!([effect]), json!({"gain": 1}))).is_empty(),
            "{preset} is valid as proposed"
        );
        assert_eq!(
            effects::preset_of(effect),
            *preset,
            "{preset} is recognized again"
        );
    }
    let saved: Vec<Value> = effects::persisted(&with_effects(json!(rows), json!({})))
        .into_iter()
        .map(|effect| effect["id"].clone())
        .collect();
    assert_eq!(saved, presets.map(|preset| json!(preset)));
    assert_eq!(
        effects::preset_of(&row(
            json!({"target": "state", "mode": "add", "generator": "constant"})
        )),
        "custom"
    );
}

#[test]
fn a_new_target_or_generator_brings_the_values_it_needs() {
    // Specification 46 §10.1: dropout acts on a measurement in Replace mode; quantisation and
    // clipping replace. Catches a dropout left on a parameter target, or a clip without a range.
    let parameter_row = row(
        json!({"id": "e", "enabled": true, "target": "parameter", "parameter": "gain", "mode": "add", "generator": "constant", "value": 0}),
    );
    let dropout = effects::with_generator(&parameter_row, "dropout");
    assert_eq!(dropout["target"], json!("measurement"));
    assert_eq!(dropout["mode"], json!("replace"));
    assert_eq!(dropout["channel"], json!(0));
    assert_eq!(dropout["dropout_policy"], json!("hold_last"));
    assert_eq!(
        (dropout["rate_per_s"].clone(), dropout["duration_s"].clone()),
        (json!(1), json!(1))
    );
    assert!(!dropout.contains_key("parameter"));
    let quantise = effects::with_generator(
        &row(json!({"generator": "constant", "mode": "add"})),
        "quantise",
    );
    assert_eq!(
        (quantise["mode"].clone(), quantise["quantum"].clone()),
        (json!("replace"), json!(0.1))
    );
    let clip = effects::with_generator(
        &row(json!({"generator": "constant", "mode": "add", "maximum": 5})),
        "clip",
    );
    assert_eq!(
        (clip["minimum"].clone(), clip["maximum"].clone()),
        (json!(0), json!(5)),
        "a set maximum is kept"
    );
    let to_parameter = effects::with_target(
        &row(json!({"target": "measurement", "channel": 2})),
        "parameter",
        &["gain", "time_constant_s"],
    );
    assert_eq!(to_parameter["parameter"], json!("gain"));
    assert!(!to_parameter.contains_key("channel"));
    let to_state = effects::with_target(&parameter_row, "state", &["gain"]);
    assert_eq!(to_state["channel"], json!(0));
    assert!(!to_state.contains_key("parameter"));
    assert_eq!(
        effects::fresh(&[row(json!({"id": "effect_1"}))])["id"],
        json!("effect_2")
    );
}

#[test]
fn event_times_in_seconds_are_exact_milliseconds() {
    // Specification 47 §6.2: 1.001 s saves as exactly 1001 ms; finer, negative or unparsable times
    // are refused, never rounded. Catches binary floating-point rounding of the time.
    assert_eq!(events::milliseconds_from_seconds("1.001"), Some(1001));
    assert_eq!(events::milliseconds_from_seconds("1.0010"), Some(1001));
    assert_eq!(events::milliseconds_from_seconds(" 2 "), Some(2000));
    assert_eq!(events::milliseconds_from_seconds("1."), Some(1000));
    assert_eq!(events::milliseconds_from_seconds("0"), Some(0));
    assert_eq!(events::milliseconds_from_seconds("1.0001"), None);
    assert_eq!(events::milliseconds_from_seconds(".5"), None);
    assert_eq!(events::milliseconds_from_seconds("-1"), None);
    assert_eq!(events::milliseconds_from_seconds("abc"), None);
    // The runtime keeps elapsed nanoseconds in an i64: 9,223,372,036,854 ms is the last time.
    assert_eq!(
        events::milliseconds_from_seconds("9223372036.854"),
        Some(9_223_372_036_854)
    );
    assert_eq!(events::milliseconds_from_seconds("9223372036.855"), None);
    assert_eq!(
        events::milliseconds_from_seconds("12345678901234567"),
        None,
        "at most 16 whole digits"
    );
}

fn context(states: usize, scalar_actuator: bool, enabled: bool) -> events::EventContext {
    events::EventContext {
        enabled,
        states,
        outputs: 1,
        scalar_actuator,
        parameters: vec!["gain".to_string()],
    }
}

fn event_issues(rows: Value, context: &events::EventContext) -> Vec<(String, String)> {
    found(&events::checks(
        &draft(json!({"scenarioEvents": rows})),
        context,
    ))
}

#[test]
fn every_included_event_is_checked_against_the_model() {
    // Specification 47 §12 and specification 46 §3.6. Each case catches one rule missing.
    let two_states = context(2, true, true);
    assert_eq!(
        event_issues(
            json!([{"enabled": true, "atMs": 0, "kind": "set_state", "channel": 2, "value": 1}]),
            &two_states
        ),
        [(
            "scenario_events[0].channel".to_string(),
            "State channel must be between 0 and 1.".to_string()
        )]
    );
    assert_eq!(
        event_issues(
            json!([{"enabled": true, "atMs": 0, "kind": "add_state", "channel": 0, "value": 1}]),
            &context(0, true, true)
        ),
        [(
            "scenario_events[0].channel".to_string(),
            "State channel is unavailable for this model.".to_string()
        )]
    );
    assert_eq!(
        event_issues(
            json!([{"enabled": true, "atMs": 0, "kind": "set_parameter", "parameter": "tau", "value": null}]),
            &two_states
        ),
        [
            (
                "scenario_events[0].parameter".to_string(),
                "Choose a parameter owned by the selected plant model.".to_string()
            ),
            (
                "scenario_events[0].value".to_string(),
                "Enter a finite event value.".to_string()
            ),
        ]
    );
    assert_eq!(
        event_issues(
            json!([{"enabled": true, "atMs": 0, "kind": "stick_actuator"}]),
            &context(1, false, true)
        ),
        [(
            "scenario_events[0].kind".to_string(),
            "Actuator fault events require a plant with one command signal.".to_string()
        )]
    );
    assert_eq!(
        event_issues(
            json!([{"enabled": true, "atMs": -5, "kind": "reset_plant"}]),
            &two_states
        ),
        [(
            "scenario_events[0].at_ms".to_string(),
            "Time must be a nonnegative whole number of milliseconds.".to_string()
        )]
    );
    assert_eq!(
        event_issues(json!([{"enabled": true, "atMs": 0, "timeSecondsText": "1.0001", "kind": "reset_plant"}]), &two_states),
        [(
            "scenario_events[0].at_ms".to_string(),
            "Enter nonnegative decimal seconds with at most 0.001 s precision; the value must fit the supported time range."
                .to_string()
        )]
    );
    assert_eq!(
        event_issues(
            json!([{"enabled": true, "atMs": 0, "kind": "reset_plant"}]),
            &context(1, true, false)
        ),
        [(
            "scenario_events[0].kind".to_string(),
            "Enable the plant or remove its scenario events.".to_string()
        )]
    );
    assert!(
        event_issues(
            json!([{"enabled": false, "atMs": -1, "kind": "unknown"}]),
            &two_states
        )
        .is_empty(),
        "an excluded event is not checked"
    );
}

#[test]
fn events_save_only_the_fields_their_action_takes() {
    // Specification 47 §20.2: each entry holds `at_ms`, `kind` and only the channel, parameter or
    // value its kind permits; excluded events are not saved. Catches an edited time saved in
    // seconds, or a stale channel saved with a parameter event.
    let rows = json!([
        {"enabled": true, "atMs": 1001, "kind": "set_parameter", "parameter": "gain", "value": 2, "channel": 5},
        {"enabled": false, "atMs": 0, "kind": "reset_plant"},
        {"enabled": true, "timeSecondsText": "1.5", "atMs": 0, "kind": "reset_plant"},
    ]);
    assert_eq!(
        events::persisted(&draft(json!({"scenarioEvents": rows}))),
        [
            json!({"at_ms": 1001, "kind": "set_parameter", "parameter": "gain", "value": 2}),
            json!({"at_ms": 1500, "kind": "reset_plant"}),
        ]
    );
    let changed = events::with_kind(
        &draft(
            json!({"enabled": true, "atMs": 7, "timeSecondsText": "0.007", "kind": "set_state", "channel": 1, "value": 3}),
        ),
        "set_parameter",
        &["gain".to_string(), "tau".to_string()],
    );
    assert_eq!(
        changed,
        draft(
            json!({"enabled": true, "atMs": 7, "timeSecondsText": "0.007", "kind": "set_parameter", "parameter": "gain", "value": 0})
        )
    );
}

#[test]
fn only_equal_time_events_change_places() {
    // Specification 47 §12: declaration order matters only between events due at the same time.
    // Catches a move that changes the time order.
    let rows: Vec<Map<String, Value>> = [
        json!({"atMs": 0}),
        json!({"atMs": 0}),
        json!({"atMs": 5}),
        json!({"timeSecondsText": "0.005", "atMs": 0}),
    ]
    .into_iter()
    .map(draft)
    .collect();
    assert!(events::can_move(&rows, 0, 1));
    assert!(events::can_move(&rows, 1, -1));
    assert!(!events::can_move(&rows, 1, 1));
    assert!(!events::can_move(&rows, 0, -1));
    assert!(
        events::can_move(&rows, 2, 1),
        "an edited time counts as typed"
    );
}
