//! The plant candidate a draft saves as (specification 47 §13.2, §20.2), and the drafts the editor
//! starts from: a new plant's proposal, an opened saved plant with its defaults made explicit, and a
//! change of starting point that replaces only the model's own values (specification 47 §7, §9).

use serde_json::{json, Map, Value};

use super::js;
use super::tables;
use super::{effects, events};

/// The candidate fields the editor owns; every other saved field is carried unchanged.
const EDITABLE_FIELDS: [&str; 18] = [
    "id",
    "name",
    "description",
    "enabled",
    "model",
    "command_sources",
    "feedback_targets",
    "parameters",
    "initial_state",
    "step_ms",
    "initial_input",
    "input_scale",
    "input_offset",
    "output_scale",
    "output_offset",
    "actuator",
    "uncertainties",
    "scenario_events",
];

/// The saved fields the editor does not manage (model validity, for one), kept as they were.
pub(super) fn preserved(saved: &Map<String, Value>) -> Map<String, Value> {
    saved
        .iter()
        .filter(|(key, _)| !EDITABLE_FIELDS.contains(&key.as_str()))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

/// The candidate of the draft (specification 47 §13.2; specification 46 §3.2): the preserved
/// saved fields, then the identity, model, ports, parameters and initial state; `enabled` left out
/// when it was omitted and is still true, `initial_state` left out when it was omitted and is still
/// all zeros; the plant step, the conversions and a non-empty actuator when set; the included
/// effects when the draft has an effect list; and the included scenario events.
pub(super) fn candidate(draft: &Map<String, Value>) -> Value {
    let mut candidate = preserved(js::object(draft, "preservedConfiguration"));
    let copy = |key: &str| draft.get(key).cloned().unwrap_or(Value::Null);
    candidate.insert("id".into(), json!(js::text(draft, "plantId")));
    candidate.insert("name".into(), json!(js::text(draft, "displayName")));
    candidate.insert("description".into(), json!(js::text(draft, "description")));
    let enabled = js::flag(draft, "enabled", true);
    if !(js::flag(draft, "enabledDefaulted", false) && enabled) {
        candidate.insert("enabled".into(), json!(enabled));
    }
    candidate.insert("model".into(), json!(js::text(draft, "model")));
    candidate.insert(
        "command_sources".into(),
        json!(js::strings(draft, "commandSources")),
    );
    candidate.insert(
        "feedback_targets".into(),
        json!(js::strings(draft, "feedbackTargets")),
    );
    candidate.insert(
        "parameters".into(),
        Value::Object(js::object(draft, "parameters").clone()),
    );
    let initial = draft
        .get("initialState")
        .cloned()
        .unwrap_or_else(|| json!([]));
    let all_zero = js::numbers(Some(&initial))
        .unwrap_or_default()
        .iter()
        .all(|value| value.finite() == Some(0.0));
    if !(js::flag(draft, "initialStateDefaulted", false) && all_zero) {
        candidate.insert("initial_state".into(), initial);
    }
    if draft.contains_key("stepMs") {
        candidate.insert("step_ms".into(), copy("stepMs"));
    }
    for (field, key) in [
        ("initial_input", "initialInput"),
        ("input_scale", "inputScale"),
        ("input_offset", "inputOffset"),
        ("output_scale", "outputScale"),
        ("output_offset", "outputOffset"),
    ] {
        if draft.contains_key(key) {
            candidate.insert(field.into(), copy(key));
        }
    }
    let actuator = js::object(draft, "actuator");
    if !actuator.is_empty() {
        candidate.insert("actuator".into(), Value::Object(actuator.clone()));
    }
    if draft.get("uncertainties").is_some_and(Value::is_array) {
        candidate.insert(
            "uncertainties".into(),
            Value::Array(effects::persisted(draft)),
        );
    }
    candidate.insert(
        "scenario_events".into(),
        Value::Array(events::persisted(draft)),
    );
    Value::Object(candidate)
}

/// The draft of a new plant on a runtime: the temperature starting point, the first free
/// `new_plant` ID, and no connections, conversions or events yet. Identity, runtime and draft
/// fields the editor set are kept.
pub(super) fn new_draft(draft: &Map<String, Value>, saved_ids: &[String]) -> Map<String, Value> {
    let (model, parameters, initial_state, _) =
        tables::proposal("temperature").expect("the temperature starting point exists");
    let mut id = "new_plant".to_string();
    let mut suffix = 2;
    while saved_ids.contains(&id) {
        id = format!("new_plant_{suffix}");
        suffix += 1;
    }
    let mut next = draft.clone();
    for (key, value) in [
        ("plantId", json!(id)),
        ("displayName", json!("New simulated plant")),
        ("description", json!("")),
        ("enabled", json!(true)),
        ("startingPoint", json!("temperature")),
        ("model", json!(model)),
        ("parameters", Value::Object(parameters)),
        ("initialState", Value::Array(initial_state)),
        ("modelValuesTouched", json!(false)),
        ("commandSources", json!([])),
        ("feedbackTargets", json!([])),
        ("initialInput", json!([])),
        ("inputScale", json!([])),
        ("inputOffset", json!([])),
        ("outputScale", json!([])),
        ("outputOffset", json!([])),
        ("scenarioEvents", json!([])),
        ("dirty", json!(true)),
    ] {
        next.insert(key.into(), value);
    }
    next
}

/// An opened saved plant with its defaults made explicit (specification 47 §9): its starting point
/// from its model, an omitted initial state as the model-sized zero vector (still marked omitted, so
/// that saving keeps the omission), and conversions it does not set as initial input 0, scale 1 and
/// offset 0 per port. Nothing else changes, and the draft stays unchanged-since-saved.
pub(super) fn opened(draft: &Map<String, Value>) -> Map<String, Value> {
    let mut next = draft.clone();
    let model = js::text(draft, "model").to_string();
    if js::text(draft, "startingPoint").is_empty() {
        next.insert(
            "startingPoint".into(),
            json!(tables::starting_point_for_model(&model)),
        );
    }
    let states = if model == "state_space" {
        js::object(draft, "parameters")
            .get("a")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    } else {
        tables::model_descriptor(&model).2
    };
    let initial_len = draft
        .get("initialState")
        .and_then(Value::as_array)
        .map(Vec::len);
    if js::flag(draft, "initialStateDefaulted", false) && initial_len != Some(states) {
        next.insert("initialState".into(), Value::Array(vec![json!(0); states]));
    }
    if !draft.contains_key("initialState") {
        next.insert("initialState".into(), Value::Array(vec![json!(0); states]));
    }
    let commands = js::strings(draft, "commandSources").len();
    let feedbacks = js::strings(draft, "feedbackTargets").len();
    for (key, count, value) in [
        ("initialInput", commands, 0),
        ("inputScale", commands, 1),
        ("inputOffset", commands, 0),
        ("outputScale", feedbacks, 1),
        ("outputOffset", feedbacks, 0),
    ] {
        if !draft.get(key).is_some_and(Value::is_array) {
            next.insert(key.into(), Value::Array(vec![json!(value); count]));
        }
    }
    if !draft.get("scenarioEvents").is_some_and(Value::is_array) {
        next.insert("scenarioEvents".into(), json!([]));
    }
    next
}

/// Whether choosing starting point `requested` needs the user's confirmation, and its message: the
/// temperature example always lists what it replaces; any other choice needs confirmation once the
/// user edited a model value or the draft is a saved plant (specification 47 §7, §7.1).
pub(super) fn starting_point_confirmation(
    draft: &Map<String, Value>,
    requested: &str,
) -> Option<String> {
    if requested == tables::TEMPERATURE_EXAMPLE {
        return Some("Replace model values, starting state, actuator settings and signal conversions with the temperature teaching example? This sets 20 °C at rest and start, 100 °C full-power rise, 30 s response, 2 s delay, heater limits 0–100%, initial heater power 0%, and identity PLC conversions. Keep the existing name, runtime, bindings, validity, imperfections and scenarios; retained effects can change the nominal response.".to_string());
    }
    let touched = js::flag(draft, "modelValuesTouched", false);
    let saved = js::flag(draft, "persisted", false);
    if !touched && !saved {
        return None;
    }
    Some(format!(
        "Replace your edited {} dynamics with the {} starting values?",
        tables::starting_point_name(js::text(draft, "startingPoint")),
        tables::starting_point_name(requested)
    ))
}

/// The draft at starting point `requested`: only the model, its parameters and initial state change
/// (the temperature example also sets its actuator and conversions, `temperature::apply`); plant
/// identity, placement, enabled state, signal mappings and every other section stay. `None` for an
/// unknown starting point.
pub(super) fn with_starting_point(
    draft: &Map<String, Value>,
    requested: &str,
) -> Option<Map<String, Value>> {
    if requested == tables::TEMPERATURE_EXAMPLE {
        return Some(super::temperature::apply(draft));
    }
    let (model, parameters, initial_state, _) = tables::proposal(requested)?;
    let mut next = draft.clone();
    next.insert("startingPoint".into(), json!(requested));
    next.insert("model".into(), json!(model));
    next.insert("parameters".into(), Value::Object(parameters));
    next.insert("initialState".into(), Value::Array(initial_state));
    next.insert("modelValuesTouched".into(), json!(false));
    Some(next)
}
