//! The plant editor's vocabularies and the tables `plant.capabilities` carries (specification 47
//! §11, §12; specification 46 §3.5, §3.6, §10): the uncertainty targets, modes, generators and
//! field labels, the imperfection presets, the scenario actions, and the editor tables as one
//! document, built from these and from the model tables.

use serde_json::{json, Map, Value};

use super::tables::{
    dynamics_fields, starting_point_for_model, starting_state_fields, stored_key, DynamicsField,
    ValueKind, ACTUATOR_FIELDS, CUSTOM_STATE_SPACE, CUSTOM_STATE_SPACE_HELP, MODELS,
    STARTING_POINTS,
};

/// The uncertainty targets, modes and generators of specification 46 §3.5 and §10.
pub(super) const UNCERTAINTY_TARGETS: [&str; 6] = [
    "command",
    "process_input",
    "state",
    "process_output",
    "measurement",
    "parameter",
];
pub(super) const UNCERTAINTY_MODES: [&str; 4] = ["add", "multiply", "replace", "hold"];
pub(super) const UNCERTAINTY_GENERATORS: [&str; 11] = [
    "constant",
    "uniform_white",
    "pulse",
    "sine",
    "normal_approx_white",
    "colored_noise",
    "random_walk",
    "quantise",
    "clip",
    "backlash",
    "dropout",
];

/// What a dropout does while it lasts (specification 46 §10): hold the last value, replace it, or
/// fault the measurement.
pub(super) const DROPOUT_POLICIES: [&str; 3] = ["hold_last", "replace", "fault"];

/// The parameters a generator takes, in the editor's order; none for an unknown generator.
pub(super) fn generator_fields(generator: &str) -> &'static [&'static str] {
    match generator {
        "constant" => &["value"],
        "uniform_white" => &["minimum", "maximum"],
        "pulse" => &["value", "duration_s"],
        "sine" => &["amplitude", "frequency_hz", "phase_rad", "bias"],
        "normal_approx_white" => &["mean", "standard_deviation"],
        "colored_noise" => &["standard_deviation", "correlation_time_s"],
        "random_walk" => &["diffusion_per_sqrt_s", "minimum", "maximum"],
        "quantise" => &["quantum"],
        "clip" => &["minimum", "maximum"],
        "backlash" => &["width"],
        "dropout" => &[
            "rate_per_s",
            "duration_s",
            "dropout_policy",
            "replacement_value",
        ],
        _ => &[],
    }
}

/// Every field an uncertainty effect may hold, in the editor's order.
const UNCERTAINTY_FIELDS: [&str; 25] = [
    "id",
    "target",
    "mode",
    "generator",
    "parameter",
    "channel",
    "start_s",
    "end_s",
    "value",
    "minimum",
    "maximum",
    "duration_s",
    "amplitude",
    "frequency_hz",
    "phase_rad",
    "bias",
    "mean",
    "standard_deviation",
    "correlation_time_s",
    "diffusion_per_sqrt_s",
    "quantum",
    "rate_per_s",
    "dropout_policy",
    "replacement_value",
    "width",
];

/// The label of an uncertainty field; an unknown field shows its name with spaces.
pub(super) fn uncertainty_field_label(field: &str) -> String {
    let label = match field {
        "id" => "Effect ID",
        "target" => "Target",
        "mode" => "Mode",
        "generator" => "Generator",
        "parameter" => "Parameter",
        "channel" => "Channel",
        "start_s" => "Start (s)",
        "end_s" => "End (s)",
        "value" => "Value",
        "minimum" => "Minimum",
        "maximum" => "Maximum",
        "duration_s" => "Duration (s)",
        "amplitude" => "Amplitude",
        "frequency_hz" => "Frequency (Hz)",
        "phase_rad" => "Phase (rad)",
        "bias" => "Bias",
        "mean" => "Mean",
        "standard_deviation" => "Standard deviation",
        "correlation_time_s" => "Correlation time (s)",
        "diffusion_per_sqrt_s" => "Diffusion per √s",
        "quantum" => "Quantum",
        "rate_per_s" => "Dropout rate (/s)",
        "dropout_policy" => "Dropout action",
        "replacement_value" => "Replacement value",
        "width" => "Backlash width",
        other => return other.replace('_', " "),
    };
    label.to_string()
}

/// The imperfection presets of the Add effect chooser (specification 47 §6.2, §11): each preset's
/// identity, label and what the user should expect.
pub(super) const IMPERFECTION_TYPES: [(&str, &str, &str); 9] = [
    (
        "bias",
        "Bias",
        "The measured value stays offset from the ideal value.",
    ),
    (
        "drift",
        "Drift",
        "The measured value wanders slowly over time.",
    ),
    (
        "gaussian_noise",
        "Gaussian noise",
        "Fast random variation appears around the ideal value.",
    ),
    (
        "quantization",
        "Quantization",
        "The measured value moves in discrete steps.",
    ),
    (
        "sample_and_hold",
        "Sample and hold",
        "The measured value pauses for the configured interval.",
    ),
    (
        "dropout",
        "Dropout",
        "Measurements intermittently disappear or use the selected fallback.",
    ),
    (
        "stuck_sensor",
        "Stuck sensor",
        "The measured value freezes when the effect becomes active.",
    ),
    (
        "outlier",
        "Outlier",
        "A short measurement spike challenges filtering and diagnostics.",
    ),
    (
        "custom",
        "Custom runtime effect",
        "Use the exact runtime target, mode, and generator controls.",
    ),
];

/// The scenario event kinds of specification 46 §3.6 with the editor's action names.
pub(super) const SCENARIO_EVENT_KINDS: [(&str, &str); 7] = [
    ("set_state", "Set state"),
    ("add_state", "Add to state"),
    ("set_parameter", "Change model parameter"),
    ("set_sensor_bias", "Set sensor bias"),
    ("stick_actuator", "Stick actuator"),
    ("release_actuator", "Release actuator"),
    ("reset_plant", "Reset plant"),
];

/// The editor tables as `plant.capabilities` carries them: the editor shows every label, help text,
/// default and choice from here.
pub(in crate::control::plant_handlers) fn editor_tables() -> Value {
    let value_kind = |kind: ValueKind| match kind {
        ValueKind::Number => "number",
        ValueKind::Vector => "vector",
        ValueKind::Matrix => "matrix",
    };
    let dynamics_field = |field: &DynamicsField| {
        let mut entry = json!({
            "key": field.key,
            "label": field.label,
            "valueKind": value_kind(field.value_kind),
            "plainTerm": field.plain_term,
            "controlTerm": field.control_term,
            "unit": field.unit,
            "effect": field.effect,
            "storedKey": stored_key(field.key),
            "optional": field.optional,
        });
        if let Some(default) = field.default_value {
            entry["defaultValue"] = super::js::js_number(default);
        }
        if let Some(from) = field.default_from {
            entry["defaultFrom"] = json!(from);
        }
        entry
    };
    let models: Vec<Value> = MODELS
        .iter()
        .map(|model| {
            json!({
                "kind": model.kind,
                "productName": model.product_name,
                "purpose": model.purpose,
                "states": model.states,
                "inputs": model.inputs,
                "outputs": model.outputs,
                "dynamicsFields": dynamics_fields(model.kind).iter().map(dynamics_field).collect::<Vec<_>>(),
                "startingStateFields": starting_state_fields(model.kind)
                    .iter()
                    .map(|(label, description)| json!({"label": label, "description": description}))
                    .collect::<Vec<_>>(),
                "startingPoint": starting_point_for_model(model.kind),
            })
        })
        .collect();
    let actuator_fields: Vec<Value> = ACTUATOR_FIELDS
        .iter()
        .map(|field| {
            let mut entry = json!({
                "key": field.key,
                "label": field.label,
                "plainTerm": field.plain_term,
                "controlTerm": field.control_term,
                "unit": field.unit,
                "effect": field.effect,
                "blankMeaning": field.blank_meaning,
                "storedKey": format!("actuator.{}", field.key),
            });
            if let Some(default) = field.default_value {
                entry["defaultValue"] = super::js::js_number(default);
            }
            if let Some(from) = field.default_from {
                entry["defaultFrom"] = json!(from);
            }
            entry
        })
        .collect();
    let starting_points: Vec<Value> = STARTING_POINTS
        .iter()
        .map(|point| {
            json!({
                "id": point.id,
                "label": point.label,
                "model": point.model,
                "whyReasonable": point.why_reasonable,
                "limitation": point.limitation,
            })
        })
        .collect();
    let field_labels: Map<String, Value> = UNCERTAINTY_FIELDS
        .iter()
        .map(|field| {
            (
                (*field).to_string(),
                Value::String(uncertainty_field_label(field)),
            )
        })
        .collect();
    json!({
        "models": models,
        "actuatorFields": actuator_fields,
        "startingPoints": starting_points,
        "customStateSpace": {
            "id": CUSTOM_STATE_SPACE,
            "whyReasonable": CUSTOM_STATE_SPACE_HELP.0,
            "limitation": CUSTOM_STATE_SPACE_HELP.1,
        },
        "imperfectionTypes": IMPERFECTION_TYPES
            .iter()
            .map(|(id, label, expectation)| json!({"id": id, "label": label, "expectation": expectation}))
            .collect::<Vec<_>>(),
        "uncertaintyTargets": UNCERTAINTY_TARGETS,
        "uncertaintyModes": UNCERTAINTY_MODES,
        "uncertaintyGenerators": UNCERTAINTY_GENERATORS
            .iter()
            .map(|generator| json!({"id": generator, "fields": generator_fields(generator)}))
            .collect::<Vec<_>>(),
        "uncertaintyFieldLabels": field_labels,
        "uncertaintyDropoutPolicies": DROPOUT_POLICIES,
        "scenarioEventKinds": SCENARIO_EVENT_KINDS
            .iter()
            .map(|(kind, label)| json!({"kind": kind, "label": label}))
            .collect::<Vec<_>>(),
    })
}
