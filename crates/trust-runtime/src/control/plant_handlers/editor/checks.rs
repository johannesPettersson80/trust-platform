//! The plant editor's checks of the plant identity, the signal conversions, the actuator and the
//! model dynamics (specification 47 §7, §8, §9, §10; specification 46 §5.1, §7, §8): each finds
//! every problem of its section, with the field it belongs to and the message the editor shows.

use serde_json::{Map, Value};

use super::issues::{Issue, Target};
use super::js::{self, Num};
use super::tables;

/// The identity checks (specification 47 §7; specification 33): the plant ID starts with a letter
/// and then uses letters, digits, `_` or `-`; it is not the ID of another saved plant; the display
/// name is not blank.
pub(super) fn identity(draft: &Map<String, Value>, other_plant_ids: &[String]) -> Vec<Issue> {
    let mut issues = Vec::new();
    let id = js::text(draft, "plantId");
    let display_name = js::text(draft, "displayName");
    let id_issue = if !valid_plant_id(id) {
        Some((
            "invalid_plant_id",
            "Start with a letter; then use letters, numbers, _ or -.",
        ))
    } else if other_plant_ids.iter().any(|other| other == id) {
        Some((
            "duplicate_plant_id",
            "Choose an ID that is not already used in this project.",
        ))
    } else {
        None
    };
    if let Some((code, message)) = id_issue {
        issues.push(
            Issue::new(code, "id", message, Target::Identity("id"))
                .summarized(format!("Plant ID \"{id}\": {message}")),
        );
    }
    if display_name.trim().is_empty() {
        let message = "Enter a display name.";
        issues.push(
            Issue::new(
                "empty_display_name",
                "name",
                message,
                Target::Identity("displayName"),
            )
            .summarized(format!("Display name \"{display_name}\": {message}")),
        );
    }
    issues
}

/// `^[A-Za-z][A-Za-z0-9_-]*$`.
pub(super) fn valid_plant_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|next| next.is_ascii_alphanumeric() || next == '_' || next == '-')
}

/// The signal-conversion fields, with the label the summary names them by.
const SIGNAL_FIELDS: [(&str, &str, &str, bool); 5] = [
    ("initial_input", "initialInput", "Initial input", true),
    ("input_scale", "inputScale", "Input scale", true),
    ("input_offset", "inputOffset", "Input offset", true),
    ("output_scale", "outputScale", "Output scale", false),
    ("output_offset", "outputOffset", "Output offset", false),
];

/// The signal-conversion checks (specification 47 §8; specification 46 §3.2): a plant step override
/// is a positive whole number of milliseconds, and each conversion vector holds one finite number
/// per command or feedback signal.
pub(super) fn signal_engineering(draft: &Map<String, Value>) -> Vec<Issue> {
    let mut issues = Vec::new();
    let step = js::field(draft, "stepMs");
    if step.present() && !step.safe_integer().is_some_and(|value| value > 0) {
        let message = "Plant step must be a positive whole number of milliseconds.";
        issues.push(
            Issue::new(
                "invalid_plant_step",
                "step_ms",
                message,
                Target::Signal("step_ms"),
            )
            .summarized(format!(
                "Plant step override (ms) ({}): {message}",
                js::shown(draft.get("stepMs"))
            )),
        );
    }
    let commands = js::strings(draft, "commandSources").len();
    let feedbacks = js::strings(draft, "feedbackTargets").len();
    for (field, key, label, command) in SIGNAL_FIELDS {
        let Some(values) = js::numbers(draft.get(key)) else {
            continue;
        };
        let (expected, signal) = if command {
            (commands, "command")
        } else {
            (feedbacks, "feedback")
        };
        let message = if values.iter().any(|value| value.finite().is_none()) {
            format!("{label} must contain only finite numbers.")
        } else if values.len() != expected {
            let plural = if expected == 1 { "" } else { "s" };
            format!(
                "{label} needs exactly {expected} value{plural} for the {expected} {signal} signal{plural}."
            )
        } else {
            continue;
        };
        let summary = format!("{label} ({}): {message}", js::shown(draft.get(key)));
        issues.push(
            Issue::new(
                "invalid_signal_conversion",
                field,
                message,
                Target::Signal(field),
            )
            .summarized(summary),
        );
    }
    issues
}

/// The actuator checks (specification 47 §10; specification 46 §8): every value is finite; rates,
/// deadband, stiction, slip jump and lag are not negative; the maximum is not below the minimum;
/// the initial applied value lies between them; the slip jump does not exceed the stiction band;
/// the installed characteristic's exponent is positive, and one other than 1 needs a range.
pub(super) fn actuator(draft: &Map<String, Value>) -> Vec<Issue> {
    let actuator = js::object(draft, "actuator");
    let value = |key: &str| js::field(actuator, key);
    let mut found: Vec<(String, String)> = Vec::new();
    // Each value the editor holds must be finite; the fields are checked in the editor's order, then
    // any other the draft holds.
    let known = tables::ACTUATOR_FIELDS.iter().map(|field| field.key);
    let others = actuator.keys().map(String::as_str).filter(|key| {
        !tables::ACTUATOR_FIELDS
            .iter()
            .any(|field| field.key == *key)
    });
    for key in known.chain(others) {
        if value(key) == Num::NotFinite {
            found.push((
                key.to_string(),
                "Enter a finite number or leave this field empty.".into(),
            ));
        }
    }
    for (key, label) in [
        ("rate_up_per_s", "Rising rate"),
        ("rate_down_per_s", "Falling rate"),
        ("deadband", "Deadband"),
        ("stiction_band", "Stiction band"),
        ("slip_jump", "Slip jump"),
        ("lag_s", "Actuator lag"),
    ] {
        if value(key).below(0.0) {
            found.push((key.to_string(), format!("{label} must be 0 or greater.")));
        }
    }
    let minimum = value("minimum");
    let maximum = value("maximum");
    let initial = value("initial_applied");
    if greater(minimum, maximum) {
        found.push((
            "maximum".into(),
            "Maximum must be greater than or equal to minimum.".into(),
        ));
    } else if initial.present() && (greater(minimum, initial) || greater(initial, maximum)) {
        found.push((
            "initial_applied".into(),
            "Initial applied value must be within minimum and maximum.".into(),
        ));
    }
    if greater(value("slip_jump"), value("stiction_band")) {
        found.push((
            "slip_jump".into(),
            "Slip jump must not exceed the stiction band.".into(),
        ));
    }
    let exponent = value("characteristic_exponent");
    if exponent.finite().is_some_and(|exponent| exponent <= 0.0) {
        found.push((
            "characteristic_exponent".into(),
            "Installed characteristic exponent must be greater than 0.".into(),
        ));
    } else if exponent.present()
        && exponent != Num::Finite(1.0)
        && (!minimum.present() || !maximum.present() || at_least(minimum, maximum))
    {
        found.push((
            "characteristic_exponent".into(),
            "A nonlinear installed characteristic requires finite minimum and maximum values with minimum below maximum.".into(),
        ));
    }
    found
        .into_iter()
        .map(|(key, message)| {
            let label = tables::ACTUATOR_FIELDS
                .iter()
                .find(|field| field.key == key)
                .map_or(key.as_str(), |field| field.label);
            let summary = format!("{label} ({}): {message}", js::shown(actuator.get(&key)));
            Issue::new(
                "invalid_actuator",
                format!("actuator.{key}"),
                message,
                Target::Actuator(key.clone()),
            )
            .summarized(summary)
        })
        .collect()
}

/// `a > b` in JavaScript, where an absent or non-finite operand makes the comparison false.
fn greater(a: Num, b: Num) -> bool {
    matches!((a.finite(), b.finite()), (Some(a), Some(b)) if a > b)
}

/// `a >= b` in JavaScript.
fn at_least(a: Num, b: Num) -> bool {
    matches!((a.finite(), b.finite()), (Some(a), Some(b)) if a >= b)
}

/// The dynamics checks (specification 47 §9; specification 46 §7): a list or matrix field holds a
/// list or a matrix; every required field and every field the draft holds contains finite numbers; input delays exist only for continuous state
/// space; the time constant and natural frequency are positive; the dead time and damping ratio are
/// not negative; the integrator's maximum state is not below its minimum. State-space matrices and
/// piecewise points are checked cell by cell and row by row (`state_space`, `piecewise`).
pub(super) fn dynamics(draft: &Map<String, Value>) -> Vec<Issue> {
    let model = js::text(draft, "model");
    let parameters = js::object(draft, "parameters");
    let mut issues = Vec::new();
    let value = |key: &str| {
        if key == "initial_state" {
            draft.get("initialState")
        } else {
            parameters.get(key)
        }
    };
    let issue = |key: &str, message: String| {
        let label = tables::dynamics_fields(model)
            .iter()
            .find(|field| field.key == key)
            .map_or(key, |field| field.label);
        let shown = js::shown(value(key));
        Issue::new(
            "invalid_dynamics",
            tables::stored_key(key),
            message.clone(),
            Target::Dynamics(key.to_string()),
        )
        .summarized(format!("{label} ({shown}): {message}"))
    };
    // State space and piecewise curves are checked by their cell and row editors.
    let cell_checked = |key: &str| match model {
        "state_space" => matches!(key, "a" | "b" | "c" | "d" | "initial_state"),
        "piecewise_linear" => matches!(key, "input_points" | "output_points"),
        _ => false,
    };
    for field in tables::dynamics_fields(model) {
        if field.key == "input_delays_s" || cell_checked(field.key) {
            continue;
        }
        let current = value(field.key);
        if field.optional && current.is_none() {
            continue;
        }
        // A list or matrix field holds the text the user typed when it is not one.
        let shaped = current.is_none_or(|current| match field.value_kind {
            tables::ValueKind::Number => true,
            tables::ValueKind::Vector => current.is_array(),
            tables::ValueKind::Matrix => current
                .as_array()
                .is_some_and(|rows| rows.iter().all(Value::is_array)),
        });
        let finite = current.is_some_and(|current| {
            let numbers = js::flattened(current);
            !numbers.is_empty() && numbers.iter().all(|number| number.finite().is_some())
        });
        if !shaped {
            issues.push(issue(
                field.key,
                if field.value_kind == tables::ValueKind::Matrix {
                    "Enter a matrix such as [[0, 1], [-1, 0]]."
                } else {
                    "Enter a list such as [0, 1, 2]."
                }
                .into(),
            ));
        } else if !finite {
            let message = format!("{} must contain finite numbers.", field.label);
            // A starting state names each value that is no number, so that its own field shows it.
            let starting = (field.key == "initial_state")
                .then(|| current.and_then(Value::as_array))
                .flatten()
                .filter(|values| !values.is_empty());
            match starting {
                Some(values) => issues.extend(
                    values
                        .iter()
                        .enumerate()
                        .filter(|(_, value)| js::num(Some(value)).finite().is_none())
                        .map(|(index, _)| Issue {
                            path: format!("initial_state[{index}]"),
                            target: Target::StartingState(index),
                            ..issue(field.key, message.clone())
                        }),
                ),
                None => issues.push(issue(field.key, message)),
            }
        }
    }
    if model != "state_space" && parameters.contains_key("input_delays_s") {
        issues.push(issue(
            "input_delays_s",
            "Input delays are available only for continuous state-space models.".into(),
        ));
    }
    let scalar = |key: &str| js::field(parameters, key).finite();
    if scalar("time_constant_s").is_some_and(|value| value <= 0.0) {
        issues.push(issue(
            "time_constant_s",
            "Time constant must be greater than 0 s.".into(),
        ));
    }
    if scalar("dead_time_s").is_some_and(|value| value < 0.0) {
        issues.push(issue(
            "dead_time_s",
            "Dead time must be 0 s or greater.".into(),
        ));
    }
    if scalar("natural_frequency_rad_s").is_some_and(|value| value <= 0.0) {
        issues.push(issue(
            "natural_frequency_rad_s",
            "Natural frequency must be greater than 0 rad/s.".into(),
        ));
    }
    if scalar("damping_ratio").is_some_and(|value| value < 0.0) {
        issues.push(issue(
            "damping_ratio",
            "Damping ratio must be 0 or greater.".into(),
        ));
    }
    if let (Some(minimum), Some(maximum)) = (scalar("state_minimum"), scalar("state_maximum"))
        && minimum > maximum
    {
        issues.push(issue(
            "state_maximum",
            "Maximum state must be greater than or equal to minimum state.".into(),
        ));
    }
    issues
}
