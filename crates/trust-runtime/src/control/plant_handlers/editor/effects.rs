//! The imperfection editor's rules (specification 47 §11; specification 46 §3.5, §10): the checks
//! of every included effect, the effect entries a plant saves, the presets of the Add effect
//! chooser, the preset a row matches, and the defaults a new target or generator brings.

use serde_json::{json, Map, Value};

use super::issues::{Issue, Target};
use super::js::{self, Num};
use super::vocabulary;

/// The checks of the included effects (specification 46 §3.5, §10.1): a valid ID, unique among the
/// included effects; a parameter of the model for a parameter target, else a channel that is a
/// nonnegative whole number; a start not before 0 s and an end not before the start; hold only on a
/// process input, process output or measurement; quantisation and clipping only in Replace mode;
/// dropout only on a measurement in Replace mode; backlash only on a process output in Replace mode;
/// and the generator's own values (none for Hold, which keeps the last value).
pub(super) fn checks(draft: &Map<String, Value>) -> Vec<Issue> {
    let parameters: Vec<&str> = js::object(draft, "parameters")
        .keys()
        .map(String::as_str)
        .collect();
    let rows = js::rows(draft, "uncertainties");
    let mut issues = Vec::new();
    let mut ids: Vec<String> = Vec::new();
    for (row, effect) in rows.iter().enumerate() {
        if !js::flag(effect, "enabled", false) {
            continue;
        }
        let found = effect_problems(effect, &parameters, &ids);
        ids.push(js::text(effect, "id").to_string());
        for (field, message) in found {
            let summary = format!(
                "Effect {} ({}) · {} ({}): {message}",
                row + 1,
                effect
                    .get("id")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown"),
                vocabulary::uncertainty_field_label(field),
                js::shown(effect.get(field)),
            );
            issues.push(
                Issue::new(
                    "invalid_uncertainty",
                    format!("uncertainties[{row}].{field}"),
                    message,
                    Target::Effect(row, field.to_string()),
                )
                .summarized(summary),
            );
        }
    }
    issues
}

/// The problems of one included effect, by field, in the order the checks find them.
fn effect_problems(
    effect: &Map<String, Value>,
    parameters: &[&str],
    earlier_ids: &[String],
) -> Vec<(&'static str, String)> {
    let mut found: Vec<(&'static str, String)> = Vec::new();
    let id = js::text(effect, "id");
    if !super::checks::valid_plant_id(id) {
        found.push((
            "id",
            "Start with a letter; then use letters, numbers, _ or -.".into(),
        ));
    } else if earlier_ids.iter().any(|other| other == id) {
        found.push(("id", "Choose an effect ID that is not already used.".into()));
    }
    let target = js::text(effect, "target");
    let mode = js::text(effect, "mode");
    let generator = js::text(effect, "generator");
    let value = |field: &str| js::field(effect, field);
    if target == "parameter" {
        let parameter = js::text(effect, "parameter");
        if parameter.is_empty() || !parameters.contains(&parameter) {
            found.push((
                "parameter",
                "Choose a parameter owned by the selected plant model.".into(),
            ));
        }
    } else if !value("channel")
        .safe_integer()
        .is_some_and(|channel| channel >= 0)
    {
        found.push(("channel", "Channel must be a nonnegative integer.".into()));
    }
    let start = value("start_s");
    if start.below(0.0) {
        found.push(("start_s", "Start time must be 0 s or greater.".into()));
    }
    // `end_s < (start_s ?? 0)`: a missing start is 0 s, a start that is not a number compares false.
    let start_or_zero = if start.present() {
        start
    } else {
        Num::Finite(0.0)
    };
    if let (Some(end), Some(start)) = (value("end_s").finite(), start_or_zero.finite())
        && end < start
    {
        found.push(("end_s", "End time must not precede start time.".into()));
    }
    if mode == "hold" && !matches!(target, "process_input" | "process_output" | "measurement") {
        found.push((
            "mode",
            "Hold is available only for process input, process output, or measurement.".into(),
        ));
    }
    if matches!(generator, "quantise" | "clip") && mode != "replace" {
        found.push((
            "mode",
            "Quantisation and clipping require Replace mode.".into(),
        ));
    }
    if generator == "dropout" && (target != "measurement" || mode != "replace") {
        found.push((
            "generator",
            "Dropout requires Measurement and Replace mode.".into(),
        ));
    }
    if generator == "backlash" && (target != "process_output" || mode != "replace") {
        found.push((
            "generator",
            "Backlash requires Process output and Replace mode.".into(),
        ));
    }
    if mode == "hold" {
        // Hold keeps the last accepted value; its generator asks for nothing.
        return found;
    }
    // A generator value the effect needs: a finite number, above 0 s for a duration or time.
    let required = |field: &'static str, label: &str, positive: bool| match value(field) {
        Num::Finite(number) if positive && number <= 0.0 => {
            let mut sentence = label.to_string();
            if let Some(first) = sentence.get_mut(0..1) {
                first.make_ascii_uppercase();
            }
            Some((field, format!("{sentence} must be greater than 0 s.")))
        }
        Num::Finite(_) => None,
        Num::Absent | Num::NotFinite => Some((field, format!("Enter {label}."))),
    };
    let need = |field: &'static str,
                label: &str,
                positive: bool,
                found: &mut Vec<(&'static str, String)>| {
        found.extend(required(field, label, positive));
    };
    match generator {
        "constant" => need("value", "a value", false, &mut found),
        "uniform_white" | "clip" => {
            need("minimum", "a minimum", false, &mut found);
            need("maximum", "a maximum", false, &mut found);
            if let (Some(minimum), Some(maximum)) =
                (value("minimum").finite(), value("maximum").finite())
            {
                if generator == "uniform_white" && minimum >= maximum {
                    found.push(("maximum", "Maximum must be greater than minimum.".into()));
                } else if generator == "clip" && minimum > maximum {
                    found.push((
                        "maximum",
                        "Maximum must be greater than or equal to minimum.".into(),
                    ));
                }
            }
        }
        "pulse" => {
            need("value", "a value", false, &mut found);
            need("duration_s", "duration", true, &mut found);
        }
        "sine" => {
            need("amplitude", "an amplitude", false, &mut found);
            need("frequency_hz", "a frequency", false, &mut found);
            need("phase_rad", "a phase", false, &mut found);
            if value("frequency_hz").below(0.0) {
                found.push(("frequency_hz", "Frequency must be 0 Hz or greater.".into()));
            }
        }
        "normal_approx_white" => {
            need(
                "standard_deviation",
                "a standard deviation",
                false,
                &mut found,
            );
            if value("standard_deviation").below(0.0) {
                found.push((
                    "standard_deviation",
                    "Standard deviation must be 0 or greater.".into(),
                ));
            }
        }
        "colored_noise" => {
            need(
                "standard_deviation",
                "a standard deviation",
                false,
                &mut found,
            );
            need("correlation_time_s", "correlation time", true, &mut found);
        }
        "random_walk" => need(
            "diffusion_per_sqrt_s",
            "a diffusion rate",
            false,
            &mut found,
        ),
        "quantise" => {
            need("quantum", "a quantum", false, &mut found);
            if value("quantum")
                .finite()
                .is_some_and(|quantum| quantum <= 0.0)
            {
                found.push(("quantum", "Quantum must be greater than 0.".into()));
            }
        }
        "backlash" => {
            need("width", "a backlash width", false, &mut found);
            if value("width").finite().is_some_and(|width| width <= 0.0) {
                found.push(("width", "Backlash width must be greater than 0.".into()));
            }
        }
        "dropout" => {
            need("rate_per_s", "a dropout rate", false, &mut found);
            need("duration_s", "duration", true, &mut found);
            let policy = js::text(effect, "dropout_policy");
            if policy.is_empty() {
                found.push((
                    "dropout_policy",
                    "Choose what happens during dropout.".into(),
                ));
            }
            if policy == "replace" {
                need(
                    "replacement_value",
                    "a replacement value",
                    false,
                    &mut found,
                );
            }
        }
        _ => {}
    }
    found
}

/// The effect entries a plant saves: the included effects in order, each with its ID, target, mode
/// and generator, its parameter or channel, its start and end when set, and the values its
/// generator takes (a replacement value only for the Replace dropout action).
pub(super) fn persisted(draft: &Map<String, Value>) -> Vec<Value> {
    js::rows(draft, "uncertainties")
        .into_iter()
        .filter(|effect| js::flag(effect, "enabled", false))
        .map(|effect| {
            let mut entry = Map::new();
            for key in ["id", "target", "mode", "generator"] {
                if let Some(value) = effect.get(key) {
                    entry.insert(key.to_string(), value.clone());
                }
            }
            let place = if js::text(&effect, "target") == "parameter" {
                "parameter"
            } else {
                "channel"
            };
            if let Some(value) = effect.get(place) {
                entry.insert(place.to_string(), value.clone());
            }
            for key in ["start_s", "end_s"] {
                if let Some(value) = effect.get(key) {
                    entry.insert(key.to_string(), value.clone());
                }
            }
            let replace = js::text(&effect, "dropout_policy") == "replace";
            for field in vocabulary::generator_fields(js::text(&effect, "generator")) {
                if *field == "replacement_value" && !replace {
                    continue;
                }
                if let Some(value) = effect.get(*field) {
                    entry.insert((*field).to_string(), value.clone());
                }
            }
            Value::Object(entry)
        })
        .collect()
}

/// A new effect row with the next free `effect_<n>` ID: an included constant measurement offset of
/// 0 from 0 s.
pub(super) fn fresh(rows: &[Map<String, Value>]) -> Map<String, Value> {
    let mut sequence = 1;
    while rows
        .iter()
        .any(|row| js::text(row, "id") == format!("effect_{sequence}"))
    {
        sequence += 1;
    }
    object(json!({
        "enabled": true,
        "id": format!("effect_{sequence}"),
        "target": "measurement",
        "mode": "add",
        "generator": "constant",
        "channel": 0,
        "start_s": 0,
        "value": 0,
    }))
}

/// The row a preset gives (specification 47 §11), keeping the row's ID and Include state: a
/// measurement effect on channel 0 from 0 s with the preset's mode, generator and values.
pub(super) fn apply_preset(preset: &str, current: &Map<String, Value>) -> Map<String, Value> {
    let mut row = Map::new();
    for key in ["id", "enabled"] {
        if let Some(value) = current.get(key) {
            row.insert(key.to_string(), value.clone());
        }
    }
    row.extend(object(
        json!({"target": "measurement", "channel": 0, "start_s": 0}),
    ));
    let extra = match preset {
        "bias" => json!({"mode": "add", "generator": "constant", "value": 0}),
        "drift" => json!({"mode": "add", "generator": "random_walk", "diffusion_per_sqrt_s": 0.01}),
        "gaussian_noise" => {
            json!({"mode": "add", "generator": "normal_approx_white", "mean": 0, "standard_deviation": 0.01})
        }
        "quantization" => json!({"mode": "replace", "generator": "quantise", "quantum": 0.01}),
        "sample_and_hold" => json!({"mode": "hold", "generator": "constant", "end_s": 1}),
        "dropout" => {
            json!({"mode": "replace", "generator": "dropout", "rate_per_s": 1, "duration_s": 1, "dropout_policy": "hold_last"})
        }
        "stuck_sensor" => json!({"mode": "hold", "generator": "constant"}),
        "outlier" => json!({"mode": "add", "generator": "pulse", "value": 1, "duration_s": 0.1}),
        _ => json!({"mode": "add", "generator": "constant", "value": 0}),
    };
    row.extend(object(extra));
    row
}

/// The preset a row matches: a measurement effect of a preset's mode and generator, else custom.
pub(super) fn preset_of(row: &Map<String, Value>) -> &'static str {
    let mode = js::text(row, "mode");
    let generator = js::text(row, "generator");
    if js::text(row, "target") != "measurement" {
        return "custom";
    }
    if mode == "hold" {
        return if row.contains_key("end_s") {
            "sample_and_hold"
        } else {
            "stuck_sensor"
        };
    }
    match (generator, mode) {
        ("constant", "add") => "bias",
        ("random_walk", "add") => "drift",
        ("normal_approx_white", "add") => "gaussian_noise",
        ("quantise", "replace") => "quantization",
        ("dropout", "replace") => "dropout",
        ("pulse", "add") => "outlier",
        _ => "custom",
    }
}

/// The boundary a row's target lies at, as the effect cards name it.
pub(super) fn boundary(row: &Map<String, Value>) -> &'static str {
    match js::text(row, "target") {
        "measurement" => "Sensor",
        "command" => "Command",
        _ => "Process",
    }
}

/// The row with a new target: a parameter target takes the model's first parameter and no channel,
/// any other target channel 0 and no parameter.
pub(super) fn with_target(
    row: &Map<String, Value>,
    target: &str,
    parameters: &[&str],
) -> Map<String, Value> {
    let mut next = row.clone();
    next.insert("target".into(), json!(target));
    if target == "parameter" {
        next.insert(
            "parameter".into(),
            json!(parameters.first().copied().unwrap_or("")),
        );
        next.remove("channel");
    } else {
        next.insert("channel".into(), json!(0));
        next.remove("parameter");
    }
    next
}

/// The row with a new generator and the settings it needs: dropout makes the effect a Replace
/// measurement effect on channel 0 with rate 1/s, 1 s duration and the hold-last action;
/// quantisation and clipping switch to Replace, keeping a quantum (else 0.1) or a range (else 0–1).
pub(super) fn with_generator(row: &Map<String, Value>, generator: &str) -> Map<String, Value> {
    let mut next = row.clone();
    next.insert("generator".into(), json!(generator));
    let keep = |key: &str, default: Value| row.get(key).cloned().unwrap_or(default);
    match generator {
        "dropout" => {
            next.extend(object(json!({
                "target": "measurement", "mode": "replace", "channel": 0,
                "rate_per_s": 1, "duration_s": 1, "dropout_policy": "hold_last",
            })));
            next.remove("parameter");
        }
        "quantise" => {
            next.insert("mode".into(), json!("replace"));
            next.insert("quantum".into(), keep("quantum", json!(0.1)));
        }
        "clip" => {
            next.insert("mode".into(), json!("replace"));
            next.insert("minimum".into(), keep("minimum", json!(0)));
            next.insert("maximum".into(), keep("maximum", json!(1)));
        }
        _ => {}
    }
    next
}

fn object(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        _ => Map::new(),
    }
}
