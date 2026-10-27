//! What the plant editor shows for a draft besides its values (specification 47 §6 to §12): the
//! model's shape, the dynamics fields and where they appear, the starting states, the named ports,
//! the effective conversions, the temperature example's values and sentences, the state-space and
//! piecewise editors' facts, each effect's preset and each event's choices, and the summaries of the
//! optional settings. The editor renders these as given.

use serde_json::{json, Map, Value};

use super::js::{self, Num};
use super::{effects, events, piecewise, ports, state_space, tables, temperature, vocabulary};

/// The draft's parameter names: the model's own fields in their order, then any other parameter in
/// name order. Parameter targets and parameter events choose from these, the first by default.
pub(super) fn parameter_names(draft: &Map<String, Value>) -> Vec<String> {
    let parameters = js::object(draft, "parameters");
    let model = js::text(draft, "model");
    let mut names: Vec<String> = tables::dynamics_fields(model)
        .iter()
        .map(|field| field.key)
        .filter(|key| parameters.contains_key(*key))
        .map(str::to_string)
        .collect();
    for key in parameters.keys() {
        if !names.contains(key) {
            names.push(key.clone());
        }
    }
    names
}

/// The facts the event checks and choices need.
pub(super) fn event_context(draft: &Map<String, Value>) -> events::EventContext {
    let commands = js::strings(draft, "commandSources").len();
    events::EventContext {
        enabled: js::flag(draft, "enabled", true),
        states: draft
            .get("initialState")
            .and_then(Value::as_array)
            .map_or(0, Vec::len),
        outputs: js::strings(draft, "feedbackTargets").len().max(1),
        scalar_actuator: js::text(draft, "model") != "state_space" || commands <= 1,
        parameters: parameter_names(draft),
    }
}

/// Whether the draft shows the temperature example.
pub(super) fn temperature_view(draft: &Map<String, Value>) -> bool {
    js::text(draft, "model") == "first_order_dead_time"
        && js::text(draft, "startingPoint") == tables::TEMPERATURE_EXAMPLE
}

/// The view of the draft.
pub(super) fn view(draft: &Map<String, Value>) -> Value {
    let model = js::text(draft, "model");
    let parameters = js::object(draft, "parameters");
    let (product_name, purpose, states, inputs, outputs) = tables::model_descriptor(model);
    let (states, inputs, outputs) = if model == "state_space" {
        let b = parameters.get("b").and_then(Value::as_array);
        let c = parameters.get("c").and_then(Value::as_array);
        (
            draft
                .get("initialState")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
            b.and_then(|rows| rows.first())
                .and_then(Value::as_array)
                .map_or(inputs, Vec::len),
            c.map_or(outputs, Vec::len),
        )
    } else {
        (states, inputs, outputs)
    };
    let (command_ports, feedback_ports) = ports::labels(draft);
    json!({
        "model": {
            "kind": model,
            "productName": product_name,
            "purpose": purpose,
            "states": states,
            "inputs": inputs,
            "outputs": outputs,
        },
        "editor": match model {
            "piecewise_linear" => "piecewise",
            "state_space" => "state_space",
            _ => "fields",
        },
        "temperatureExampleAvailable": model == "first_order_dead_time",
        "temperature": temperature_view(draft).then(|| temperature_facts(draft)),
        "dynamics": dynamics_fields(draft),
        "startingState": starting_state(draft),
        "ports": {"commands": command_ports, "feedbacks": feedback_ports.clone()},
        "signals": signal_rows(draft),
        "stepText": format!(
            "Plant step: {}. Saving does not start or restart simulation.",
            match js::field(draft, "stepMs") {
                Num::Absent => "uses the project simulation setting".to_string(),
                step => format!("{} ms", shown_number(step)),
            }
        ),
        "stateSpace": (model == "state_space").then(|| state_space_facts(parameters)),
        "piecewise": (model == "piecewise_linear").then(|| piecewise_facts(parameters)),
        "parameters": parameter_names(draft),
        "effects": effect_facts(draft),
        "events": event_facts(draft, &feedback_ports),
        "summaries": {
            "actuator": actuator_summary(draft),
            "signals": signal_summary(draft),
            "realism": realism_summary(draft),
            "scenarios": scenario_summary(draft),
        },
    })
}

fn dynamics_fields(draft: &Map<String, Value>) -> Value {
    let model = js::text(draft, "model");
    let all = tables::dynamics_fields(model);
    let fopdt = model == "first_order_dead_time";
    let response = |key: &str| matches!(key, "gain" | "time_constant_s" | "dead_time_s");
    let starting_fields = !tables::starting_state_fields(model).is_empty();
    let (fields, details): (Vec<_>, Vec<_>) = if temperature_view(draft) {
        (Vec::new(), all.iter().collect())
    } else if fopdt {
        all.iter().partition(|field| response(field.key))
    } else if starting_fields {
        (
            all.iter()
                .filter(|field| field.key != "initial_state")
                .collect(),
            Vec::new(),
        )
    } else {
        (all.iter().collect(), Vec::new())
    };
    let describe = |field: &tables::DynamicsField, main: bool| {
        let label = match (fopdt && main, field.key) {
            (true, "gain") => "Response gain",
            (true, "time_constant_s") => "Response time (s)",
            (true, "dead_time_s") => "Delay before response (s)",
            _ => field.label,
        };
        let mut entry = json!({
            "key": field.key,
            "label": label,
            "valueKind": match field.value_kind {
                tables::ValueKind::Number => "number",
                tables::ValueKind::Vector => "vector",
                tables::ValueKind::Matrix => "matrix",
            },
            "plainTerm": field.plain_term,
            "controlTerm": field.control_term,
            "unit": field.unit,
            "effect": field.effect,
            "storedKey": tables::stored_key(field.key),
            "optional": field.optional,
        });
        if let Some(default) = field.default_value {
            entry["defaultValue"] = js::js_number(default);
        }
        if fopdt && main {
            entry["help"] = json!(if field.key == "time_constant_s" {
                "After the delay, about 63% of the remaining output change occurs in this time."
            } else {
                field.effect
            });
        }
        entry
    };
    json!({
        "fields": fields.into_iter().map(|field| describe(field, true)).collect::<Vec<_>>(),
        "details": details.into_iter().map(|field| describe(field, false)).collect::<Vec<_>>(),
    })
}

fn starting_state(draft: &Map<String, Value>) -> Value {
    let model = js::text(draft, "model");
    if model == "first_order_dead_time" {
        let (label, description) = if temperature_view(draft) {
            ("Starting temperature (°C)", "Absolute temperature when simulation starts or resets. This need not equal the resting temperature.")
        } else {
            (
                "Starting value",
                "The output value when simulation starts or resets, in the model's output units.",
            )
        };
        return json!({"kind": "scalar", "fields": [{"label": label, "description": description}]});
    }
    let fields = tables::starting_state_fields(model);
    if fields.is_empty() {
        return json!({
            "kind": "vector",
            "fields": [],
            "text": format!(
                "Initial model state: {}. Change it in Model & behavior.",
                js::shown(Some(draft.get("initialState").unwrap_or(&json!([]))))
            ),
        });
    }
    json!({
        "kind": "fields",
        "fields": fields
            .iter()
            .map(|(label, description)| json!({"label": label, "description": description}))
            .collect::<Vec<_>>(),
    })
}

/// Element `index` of the array `key` as `draft.key?.[index] ?? fallback` reads it: a missing array
/// or element is the fallback, a value that is not a number `NaN` (`null` on the wire).
fn element_or(draft: &Map<String, Value>, key: &str, index: usize, fallback: f64) -> Value {
    match draft
        .get(key)
        .and_then(Value::as_array)
        .and_then(|values| values.get(index))
    {
        None => js::js_number(fallback),
        Some(value) => js::num(Some(value))
            .finite()
            .map_or(Value::Null, js::js_number),
    }
}

fn signal_rows(draft: &Map<String, Value>) -> Value {
    let commands: Vec<Value> = js::strings(draft, "commandSources")
        .iter()
        .enumerate()
        .map(|(index, address)| {
            json!({
                "address": address,
                "initialInput": element_or(draft, "initialInput", index, 0.0),
                "inputScale": element_or(draft, "inputScale", index, 1.0),
                "inputOffset": element_or(draft, "inputOffset", index, 0.0),
            })
        })
        .collect();
    let feedbacks: Vec<Value> = js::strings(draft, "feedbackTargets")
        .iter()
        .enumerate()
        .map(|(index, address)| {
            json!({
                "address": address,
                "outputScale": element_or(draft, "outputScale", index, 1.0),
                "outputOffset": element_or(draft, "outputOffset", index, 0.0),
            })
        })
        .collect();
    json!({"commands": commands, "feedbacks": feedbacks})
}

/// A number as `String(value)` writes it in a sentence; `NaN` for a value that is not finite.
fn shown_number(value: Num) -> String {
    temperature::shown(value.finite().unwrap_or(f64::NAN))
}

/// `draft.key?.[0] ?? fallback` as a number: a missing value is the fallback, a non-number `NaN`.
fn first_or(draft: &Map<String, Value>, key: &str, fallback: f64) -> f64 {
    match draft
        .get(key)
        .and_then(Value::as_array)
        .and_then(|values| values.first())
    {
        None => fallback,
        Some(value) => js::num(Some(value)).finite().unwrap_or(f64::NAN),
    }
}

/// A parameter as `String(value ?? fallback)` writes it.
fn parameter_text(parameters: &Map<String, Value>, key: &str, fallback: f64) -> (f64, String) {
    let value = match js::field(parameters, key) {
        Num::Absent => fallback,
        Num::NotFinite => f64::NAN,
        Num::Finite(value) => value,
    };
    (value, temperature::shown(value))
}

fn temperature_facts(draft: &Map<String, Value>) -> Value {
    let values = temperature::values(draft);
    let parameters = js::object(draft, "parameters");
    let scale = first_or(draft, "inputScale", 1.0);
    let offset = first_or(draft, "inputOffset", 0.0);
    let output_scale = first_or(draft, "outputScale", 1.0);
    let output_offset = first_or(draft, "outputOffset", 0.0);
    let initial = draft
        .get("initialState")
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(|value| js::num(Some(value)).finite());
    let actuator = js::object(draft, "actuator");
    let limit = |key: &str| match js::field(actuator, key) {
        Num::Absent => "unbounded".to_string(),
        other => shown_number(other),
    };
    let (_, output_bias) = parameter_text(parameters, "output_bias", 0.0);
    let (gain, gain_text) = parameter_text(parameters, "gain", 1.0);
    let (_, input_bias) = parameter_text(parameters, "input_bias", 0.0);
    json!({
        "fields": temperature::FIELDS
            .iter()
            .map(|(key, label, help)| json!({"key": key, "label": label, "help": help}))
            .collect::<Vec<_>>(),
        "values": {
            "resting_temperature": js::js_number(values.resting_temperature),
            "full_power_rise": js::js_number(values.full_power_rise),
            "time_constant_s": js::js_number(values.time_constant_s),
            "dead_time_s": js::js_number(values.dead_time_s),
        },
        "nonEquilibrium": initial != Some(values.resting_temperature),
        "unitsConfirmed": temperature::units_confirmed(draft),
        "commandAssumption": if scale == 1.0 && offset == 0.0 {
            "PLC 0–100 = heater 0–100%".to_string()
        } else {
            format!(
                "Heater power (%) = {} × PLC command + {}",
                temperature::shown(scale),
                temperature::shown(offset)
            )
        },
        "feedbackAssumption": if output_scale == 1.0 && output_offset == 0.0 {
            "PLC feedback = temperature in °C".to_string()
        } else {
            format!(
                "PLC feedback = {} × temperature (°C) + {}",
                temperature::shown(output_scale),
                temperature::shown(output_offset)
            )
        },
        "heaterRange": format!(
            "Heater range: {}–{}%. Adjust limits in Actuator & command.",
            limit("minimum"),
            limit("maximum")
        ),
        "equation": [
            format!("Settled temperature (°C) = {output_bias} + {gain_text} × (power − {input_bias})."),
            format!(
                "Power is heater command in %. Gain: {gain_text} °C / %. Full-power rise: {} °C.",
                temperature::shown(gain * 100.0)
            ),
            "This is the equilibrium target, not an immediate temperature jump. Delay and response time determine how temperature approaches it.",
        ],
    })
}

fn state_space_facts(parameters: &Map<String, Value>) -> Value {
    let dimensions = state_space::dimensions(parameters);
    let delays = parameters.get("input_delays_s");
    let values: Vec<Value> = (0..dimensions.inputs)
        .map(|index| match delays {
            None => json!(0),
            Some(delays) => delays
                .as_array()
                .and_then(|delays| delays.get(index))
                .and_then(|value| js::num(Some(value)).finite())
                .map_or(Value::Null, js::js_number),
        })
        .collect();
    json!({
        "dimensions": {
            "states": dimensions.states,
            "inputs": dimensions.inputs,
            "outputs": dimensions.outputs,
        },
        "delays": values,
        "delaysExplicit": delays.is_some(),
    })
}

fn piecewise_facts(parameters: &Map<String, Value>) -> Value {
    let points = piecewise::points(parameters);
    let rows: Vec<Value> = points
        .iter()
        .enumerate()
        .map(|(index, point)| {
            json!({
                "input": js::js_number(point.input),
                "output": js::js_number(point.output),
                "canMoveUp": piecewise::moved(&points, index, -1).is_some(),
                "canMoveDown": piecewise::moved(&points, index, 1).is_some(),
            })
        })
        .collect();
    json!({"points": rows, "canRemove": points.len() > 2})
}

fn effect_facts(draft: &Map<String, Value>) -> Value {
    let rows = js::rows(draft, "uncertainties");
    Value::Array(
        rows.iter()
            .map(|row| {
                let preset = effects::preset_of(row);
                let (_, label, expectation) = vocabulary::IMPERFECTION_TYPES
                    .iter()
                    .find(|(id, _, _)| *id == preset)
                    .copied()
                    .unwrap_or((preset, preset, ""));
                let fields: Vec<&str> = if js::text(row, "mode") == "hold" {
                    Vec::new()
                } else {
                    let replace = js::text(row, "dropout_policy") == "replace";
                    vocabulary::generator_fields(js::text(row, "generator"))
                        .iter()
                        .copied()
                        .filter(|field| *field != "replacement_value" || replace)
                        .collect()
                };
                json!({
                    "preset": preset,
                    "label": label,
                    "boundary": effects::boundary(row),
                    "expectation": expectation,
                    "fields": fields,
                })
            })
            .collect(),
    )
}

fn event_facts(draft: &Map<String, Value>, feedback_ports: &[String]) -> Value {
    let context = event_context(draft);
    let state_labels: Vec<String> = tables::starting_state_fields(js::text(draft, "model"))
        .iter()
        .map(|(label, _)| {
            let quantity = label.strip_prefix("Starting ").unwrap_or(label);
            let mut chars = quantity.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        })
        .collect();
    let rows = js::rows(draft, "scenarioEvents");
    Value::Array(
        rows.iter()
            .enumerate()
            .map(|(index, event)| {
                let kind = js::text(event, "kind");
                let sensor = kind == "set_sensor_bias";
                let channels = if sensor { context.outputs } else { context.states };
                let options: Vec<Value> = (0..channels)
                    .map(|channel| {
                        let label = if sensor {
                            feedback_ports
                                .get(channel)
                                .cloned()
                                .unwrap_or_else(|| format!("Output {}", channel + 1))
                        } else {
                            state_labels
                                .get(channel)
                                .cloned()
                                .unwrap_or_else(|| format!("State {}", channel + 1))
                        };
                        json!({"value": channel, "label": label})
                    })
                    .collect();
                let time_text = match event.get("timeSecondsText").and_then(Value::as_str) {
                    Some(text) => text.to_string(),
                    None => temperature::shown(
                        js::field(event, "atMs").finite().unwrap_or(f64::NAN) / 1000.0,
                    ),
                };
                json!({
                    "timeText": time_text,
                    "canMoveUp": events::can_move(&rows, index, -1),
                    "canMoveDown": events::can_move(&rows, index, 1),
                    "showsChannel": matches!(kind, "set_state" | "add_state" | "set_sensor_bias"),
                    "channelLabel": if sensor { "Measured quantity" } else { "State quantity" },
                    "channelOptions": options,
                    "showsParameter": kind == "set_parameter",
                    "showsValue": matches!(kind, "set_state" | "add_state" | "set_parameter" | "set_sensor_bias"),
                })
            })
            .collect(),
    )
}

/// A number as the settings summaries write it: whole numbers as they are, others to eight
/// significant digits.
fn summary_number(value: f64) -> String {
    if value.is_nan() {
        return "NaN".to_string();
    }
    if value.fract() == 0.0 {
        return temperature::shown(value);
    }
    let rounded: f64 = format!("{value:.7e}").parse().unwrap_or(value);
    temperature::shown(rounded)
}

fn plural(count: usize, word: &str) -> String {
    format!("{count} {word}{}", if count == 1 { "" } else { "s" })
}

fn actuator_summary(draft: &Map<String, Value>) -> String {
    let actuator = js::object(draft, "actuator");
    if actuator.is_empty() {
        return "No actuator limits or response effects".to_string();
    }
    let mut details = Vec::new();
    let number = |key: &str| js::field(actuator, key);
    let shown = |value: Num| summary_number(value.finite().unwrap_or(f64::NAN));
    match (number("minimum"), number("maximum")) {
        (Num::Absent, Num::Absent) => {}
        (minimum, Num::Absent) => details.push(format!("Minimum {}", shown(minimum))),
        (Num::Absent, maximum) => details.push(format!("Maximum {}", shown(maximum))),
        (minimum, maximum) => details.push(format!("Range {}–{}", shown(minimum), shown(maximum))),
    }
    details.push(plural(actuator.len(), "configured setting"));
    details.join(" · ")
}

fn signal_summary(draft: &Map<String, Value>) -> String {
    let differs = |key: &str, identity: f64| {
        js::numbers(draft.get(key))
            .unwrap_or_default()
            .iter()
            .filter(|value| value.finite() != Some(identity))
            .count()
    };
    let changed = differs("inputScale", 1.0)
        + differs("inputOffset", 0.0)
        + differs("outputScale", 1.0)
        + differs("outputOffset", 0.0);
    let initial = differs("initialInput", 0.0);
    let conversions = if changed == 0 {
        "No signal scaling or offset".to_string()
    } else {
        format!("{} changed", plural(changed, "scaling or offset value"))
    };
    let initial = if initial == 0 {
        "initial command 0".to_string()
    } else {
        plural(initial, "nonzero initial command")
    };
    let step = match js::field(draft, "stepMs") {
        Num::Absent => "project simulation step".to_string(),
        step => format!("{} ms plant step", shown_number(step)),
    };
    format!("{conversions} · {initial} · {step}")
}

fn realism_summary(draft: &Map<String, Value>) -> String {
    let rows = js::rows(draft, "uncertainties");
    if rows.is_empty() {
        return "No process or sensor imperfections".to_string();
    }
    let enabled = rows
        .iter()
        .filter(|row| js::flag(row, "enabled", false))
        .count();
    let count = |target: &str| {
        rows.iter()
            .filter(|row| js::text(row, "target") == target)
            .count()
    };
    let actuator = count("command");
    let sensor = count("measurement");
    let process = rows.len() - actuator - sensor;
    [
        plural(rows.len(), "configured effect"),
        plural(enabled, "enabled effect"),
        format!("Actuator {actuator}"),
        format!("Process {process}"),
        format!("Sensor {sensor}"),
    ]
    .join(" · ")
}

fn scenario_summary(draft: &Map<String, Value>) -> String {
    let rows = js::rows(draft, "scenarioEvents");
    if rows.is_empty() {
        return "No scheduled scenarios".to_string();
    }
    let included: Vec<&Map<String, Value>> = rows
        .iter()
        .filter(|event| js::flag(event, "enabled", false))
        .collect();
    let summary = format!(
        "{} · {}",
        plural(rows.len(), "configured event"),
        plural(included.len(), "included event")
    );
    if included.is_empty() {
        return summary;
    }
    let due: Option<Vec<u64>> = included.iter().map(|event| events::due(event)).collect();
    match due {
        None => format!("{summary} · Correct event time in Scenarios"),
        Some(due) => {
            let first = due.iter().copied().min().unwrap_or(0) as f64 / 1_000.0;
            format!("{summary} · first at {} s", summary_number(first))
        }
    }
}
