//! The plant editor's rule-bearing edits (specification 47 §7 to §12): each takes the draft and the
//! user's request and returns the new draft, or asks for confirmation, or refuses with the reason
//! the editor shows. Plain value entry needs no action: the editor writes the value into the draft.

use serde_json::{json, Map, Value};

use super::js;
use super::signals::Catalog;
use super::{candidate, effects, events, piecewise, ports, state_space, temperature, vocabulary};

/// The result of an action.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Outcome {
    /// The draft changed; `index` is the row the edit created or moved to, when it did.
    Applied { index: Option<usize> },
    /// The edit needs the user's confirmation, with this message; the draft is unchanged.
    Confirm(String),
    /// The edit is refused, with this message; the draft is unchanged.
    Rejected(String),
}

impl Outcome {
    pub(super) fn to_json(&self) -> Value {
        match self {
            Self::Applied { index: None } => json!({"kind": "applied"}),
            Self::Applied { index: Some(index) } => json!({"kind": "applied", "index": index}),
            Self::Confirm(message) => json!({"kind": "confirm", "message": message}),
            Self::Rejected(message) => json!({"kind": "rejected", "message": message}),
        }
    }
}

/// What an action needs to know of the project: the saved plants' IDs and the feedback targets the
/// other saved plants write.
pub(super) struct ActionContext<'a> {
    pub(super) saved_ids: &'a [String],
    pub(super) other_feedback: &'a [String],
    pub(super) catalog: &'a Catalog,
}

/// Applies `action` (`{"kind": ..., ...}`) to the draft.
///
/// # Errors
///
/// The reason a request names no known action or lacks what the action needs.
pub(super) fn apply(
    draft: &Map<String, Value>,
    action: &Map<String, Value>,
    context: &ActionContext<'_>,
) -> Result<(Map<String, Value>, Outcome), String> {
    let kind = js::text(action, "kind");
    let applied = |next: Map<String, Value>| Ok((next, Outcome::Applied { index: None }));
    let unchanged = |outcome: Outcome| Ok((draft.clone(), outcome));
    let index = |key: &str| -> Result<usize, String> {
        js::field(action, key)
            .safe_integer()
            .and_then(|value| usize::try_from(value).ok())
            .ok_or_else(|| format!("plant.edit action {kind} needs a row {key}"))
    };
    let delta = || -> Result<isize, String> {
        match js::field(action, "delta").safe_integer() {
            Some(-1) => Ok(-1),
            Some(1) => Ok(1),
            _ => Err(format!("plant.edit action {kind} needs a delta of -1 or 1")),
        }
    };
    let name = |key: &str| -> Result<String, String> {
        action
            .get(key)
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("plant.edit action {kind} needs {key}"))
    };
    match kind {
        "open" => {
            let fresh = !js::flag(draft, "persisted", false) && js::text(draft, "model").is_empty();
            applied(if fresh {
                candidate::new_draft(draft, context.saved_ids)
            } else {
                candidate::opened(draft)
            })
        }
        "choose_starting_point" => {
            let requested = name("startingPoint")?;
            if let Some(message) = candidate::starting_point_confirmation(draft, &requested) {
                return unchanged(Outcome::Confirm(message));
            }
            match candidate::with_starting_point(draft, &requested) {
                Some(next) => applied(next),
                None => unchanged(Outcome::Rejected(format!(
                    "Unknown starting point {requested}."
                ))),
            }
        }
        "apply_starting_point" => {
            let requested = name("startingPoint")?;
            match candidate::with_starting_point(draft, &requested) {
                Some(next) => applied(next),
                None => unchanged(Outcome::Rejected(format!(
                    "Unknown starting point {requested}."
                ))),
            }
        }
        "set_temperature_value" => {
            let field = name("field")?;
            if !temperature::FIELDS.iter().any(|(key, _, _)| *key == field) {
                return Err(format!(
                    "plant.edit action set_temperature_value names no field {field}"
                ));
            }
            match temperature::entered(&field, &name("text")?) {
                Ok(value) => applied(temperature::with_value(draft, &field, value)),
                Err(message) => unchanged(Outcome::Rejected(message)),
            }
        }
        "confirm_temperature_units" => {
            let Some(confirmed) = action.get("confirmed").and_then(Value::as_bool) else {
                return Err("plant.edit action confirm_temperature_units needs confirmed".into());
            };
            applied(temperature::with_units_confirmed(draft, confirmed))
        }
        "bind_port" => {
            let direction = name("direction")?;
            if direction != "command" && direction != "feedback" {
                return Err(
                    "plant.edit action bind_port needs direction command or feedback".into(),
                );
            }
            let port = index("index")?;
            let address = name("address")?;
            if address.is_empty() {
                return applied(ports::bind(draft, &direction, port, ""));
            }
            let mut used = context.other_feedback.to_vec();
            if direction == "feedback" {
                used.extend(
                    js::strings(draft, "feedbackTargets")
                        .into_iter()
                        .enumerate()
                        .filter(|(other, _)| *other != port)
                        .map(|(_, target)| target),
                );
            }
            match ports::check_address(context.catalog, &direction, &address, &used) {
                Ok(address) => applied(ports::bind(draft, &direction, port, &address)),
                Err((_, message)) => {
                    let (commands, feedbacks) = ports::labels(draft);
                    let names = if direction == "command" {
                        commands
                    } else {
                        feedbacks
                    };
                    let label = names.get(port).cloned().unwrap_or_default();
                    unchanged(Outcome::Rejected(format!(
                        "{label} {}: {message}",
                        ports::canonical(&address)
                    )))
                }
            }
        }
        "resize_state_space" => {
            // A dimension the action leaves out keeps the draft's, so that each change is applied
            // to the draft as it is then (specification 47 §9).
            let current = state_space::dimensions(js::object(draft, "parameters"));
            let dimension = |key: &str, kept: usize| {
                if !action.contains_key(key) {
                    return Some(kept);
                }
                js::field(action, key)
                    .safe_integer()
                    .filter(|value| *value >= 1)
                    .and_then(|value| usize::try_from(value).ok())
            };
            let (Some(states), Some(inputs), Some(outputs)) = (
                dimension("states", current.states),
                dimension("inputs", current.inputs),
                dimension("outputs", current.outputs),
            ) else {
                return unchanged(Outcome::Rejected(
                    "States, inputs and outputs must be whole numbers of at least 1.".into(),
                ));
            };
            let requested = state_space::Dimensions {
                states,
                inputs,
                outputs,
            };
            let parameters = js::object(draft, "parameters");
            if !js::flag(action, "confirmed", false)
                && let Some(message) = state_space::resize_confirmation(parameters, requested)
            {
                return unchanged(Outcome::Confirm(message));
            }
            let initial = draft
                .get("initialState")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let (parameters, initial) = state_space::resize(parameters, &initial, requested);
            let mut next = draft.clone();
            next.insert("parameters".into(), Value::Object(parameters));
            next.insert("initialState".into(), Value::Array(initial));
            next.insert("modelValuesTouched".into(), json!(true));
            applied(next)
        }
        "paste_matrix" => {
            let matrix = name("matrix")?;
            if !matches!(matrix.as_str(), "a" | "b" | "c" | "d") {
                return Err("plant.edit action paste_matrix needs matrix a, b, c or d".into());
            }
            let text = name("text")?;
            let parameters = js::object(draft, "parameters");
            let dimensions = state_space::dimensions(parameters);
            let (rows, columns) = match matrix.as_str() {
                "a" => (dimensions.states, dimensions.states),
                "b" => (dimensions.states, dimensions.inputs),
                "c" => (dimensions.outputs, dimensions.states),
                _ => (dimensions.outputs, dimensions.inputs),
            };
            let parsed = state_space::parse_paste(&text);
            let mut cells = state_space::matrix(parameters.get(&matrix));
            match parsed {
                Some(block)
                    if block.len() == rows && block.iter().all(|row| row.len() == columns) =>
                {
                    cells = block;
                }
                // One number pasted into a cell replaces that cell.
                Some(block) if block.len() == 1 && block[0].len() == 1 => {
                    let (Ok(row), Ok(column)) = (index("row"), index("column")) else {
                        return unchanged(Outcome::Rejected(paste_size(&matrix, rows, columns)));
                    };
                    match cells.get_mut(row).and_then(|cells| cells.get_mut(column)) {
                        Some(cell) => *cell = block[0][0],
                        None => {
                            return unchanged(Outcome::Rejected(paste_size(&matrix, rows, columns)))
                        }
                    }
                }
                _ => return unchanged(Outcome::Rejected(paste_size(&matrix, rows, columns))),
            }
            let mut parameters = parameters.clone();
            parameters.insert(
                matrix,
                Value::Array(
                    cells
                        .iter()
                        .map(|row| {
                            Value::Array(row.iter().map(|cell| js::js_number(*cell)).collect())
                        })
                        .collect(),
                ),
            );
            let mut next = draft.clone();
            next.insert("parameters".into(), Value::Object(parameters));
            next.insert("modelValuesTouched".into(), json!(true));
            applied(next)
        }
        "append_point" | "move_point" => {
            let parameters = js::object(draft, "parameters");
            let points = piecewise::points(parameters);
            let next_points = if kind == "append_point" {
                piecewise::append(&points)
            } else {
                match piecewise::moved(&points, index("index")?, delta()?) {
                    Some(points) => points,
                    None => {
                        return unchanged(Outcome::Rejected(
                            "Moving this point would leave the input points out of order.".into(),
                        ))
                    }
                }
            };
            let mut next = draft.clone();
            next.insert(
                "parameters".into(),
                Value::Object(piecewise::with_points(parameters, &next_points)),
            );
            next.insert("modelValuesTouched".into(), json!(true));
            applied(next)
        }
        "add_effect"
        | "apply_effect_preset"
        | "duplicate_effect"
        | "set_effect_target"
        | "set_effect_generator" => effect_action(draft, kind, &index, &name),
        "add_event" => {
            let mut rows = draft
                .get("scenarioEvents")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            rows.push(Value::Object(events::fresh()));
            let mut next = draft.clone();
            next.insert("scenarioEvents".into(), Value::Array(rows.clone()));
            Ok((
                next,
                Outcome::Applied {
                    index: Some(rows.len() - 1),
                },
            ))
        }
        "set_event_kind" => {
            let row = index("index")?;
            let event_kind = name("eventKind")?;
            if !vocabulary::SCENARIO_EVENT_KINDS
                .iter()
                .any(|(known, _)| *known == event_kind)
            {
                return Err(format!(
                    "plant.edit action set_event_kind names no event kind {event_kind}"
                ));
            }
            let parameters = super::view::parameter_names(draft);
            replace_row(draft, "scenarioEvents", row, |event| {
                events::with_kind(event, &event_kind, &parameters)
            })
        }
        "remove_effect" | "move_effect" | "remove_event" | "move_event" | "duplicate_event" => {
            row_action(draft, kind, index("index")?, &delta)
        }
        other => Err(format!("unknown plant.edit action {other}")),
    }
}

/// Removes, moves or duplicates one effect or event row, so that the rows and what the editor shows
/// of them change together. Effects apply in their order, so any two neighbours may change places;
/// events only when they are due at the same time (specification 47 §11, §12). A duplicated event
/// is an exact copy after the original.
fn row_action(
    draft: &Map<String, Value>,
    kind: &str,
    row: usize,
    delta: &dyn Fn() -> Result<isize, String>,
) -> Result<(Map<String, Value>, Outcome), String> {
    let key = if kind.ends_with("_effect") {
        "uncertainties"
    } else {
        "scenarioEvents"
    };
    let mut rows = draft
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if row >= rows.len() {
        return Err(format!("plant.edit action {kind} names no row {row}"));
    }
    let index = match kind {
        "remove_effect" | "remove_event" => {
            rows.remove(row);
            None
        }
        "duplicate_event" => {
            rows.insert(row + 1, rows[row].clone());
            Some(row + 1)
        }
        _ => {
            let delta = delta()?;
            let Some(target) = row
                .checked_add_signed(delta)
                .filter(|target| *target < rows.len())
            else {
                return Ok((
                    draft.clone(),
                    Outcome::Rejected("There is no neighbour to change places with.".into()),
                ));
            };
            if kind == "move_event" && !events::can_move(&js::rows(draft, key), row, delta) {
                return Ok((
                    draft.clone(),
                    Outcome::Rejected("Only events due at the same time can change places.".into()),
                ));
            }
            rows.swap(row, target);
            Some(target)
        }
    };
    let mut next = draft.clone();
    next.insert(key.into(), Value::Array(rows));
    Ok((next, Outcome::Applied { index }))
}

fn paste_size(matrix: &str, rows: usize, columns: usize) -> String {
    format!(
        "{} paste must contain {rows} rows and {columns} columns.",
        matrix.to_ascii_uppercase()
    )
}

fn effect_action(
    draft: &Map<String, Value>,
    kind: &str,
    index: &dyn Fn(&str) -> Result<usize, String>,
    name: &dyn Fn(&str) -> Result<String, String>,
) -> Result<(Map<String, Value>, Outcome), String> {
    let rows = js::rows(draft, "uncertainties");
    let preset = || -> Result<String, String> {
        let preset = name("preset")?;
        if vocabulary::IMPERFECTION_TYPES
            .iter()
            .any(|(id, _, _)| *id == preset)
        {
            Ok(preset)
        } else {
            Err(format!("plant.edit action {kind} names no preset {preset}"))
        }
    };
    let with_rows = |rows: Vec<Map<String, Value>>, index: Option<usize>| {
        let mut next = draft.clone();
        next.insert(
            "uncertainties".into(),
            Value::Array(rows.into_iter().map(Value::Object).collect()),
        );
        Ok((next, Outcome::Applied { index }))
    };
    match kind {
        "add_effect" => {
            let row = effects::apply_preset(&preset()?, &effects::fresh(&rows));
            let mut rows = rows;
            rows.push(row);
            let index = rows.len() - 1;
            with_rows(rows, Some(index))
        }
        "duplicate_effect" => {
            let row = index("index")?;
            let Some(original) = rows.get(row) else {
                return Err(format!(
                    "plant.edit action duplicate_effect names no effect {row}"
                ));
            };
            let mut copy = original.clone();
            copy.insert(
                "id".into(),
                effects::fresh(&rows).remove("id").unwrap_or(Value::Null),
            );
            copy.insert("enabled".into(), json!(true));
            let mut rows = rows.clone();
            rows.insert(row + 1, copy);
            with_rows(rows, Some(row + 1))
        }
        _ => {
            let row = index("index")?;
            if row >= rows.len() {
                return Err(format!("plant.edit action {kind} names no effect {row}"));
            }
            let parameters = super::view::parameter_names(draft);
            let parameters: Vec<&str> = parameters.iter().map(String::as_str).collect();
            let mut rows = rows;
            rows[row] = match kind {
                "apply_effect_preset" => effects::apply_preset(&preset()?, &rows[row]),
                "set_effect_target" => {
                    let target = name("target")?;
                    if !vocabulary::UNCERTAINTY_TARGETS.contains(&target.as_str()) {
                        return Err(format!(
                            "plant.edit action set_effect_target names no target {target}"
                        ));
                    }
                    effects::with_target(&rows[row], &target, &parameters)
                }
                _ => {
                    let generator = name("generator")?;
                    if !vocabulary::UNCERTAINTY_GENERATORS.contains(&generator.as_str()) {
                        return Err(format!(
                            "plant.edit action set_effect_generator names no generator {generator}"
                        ));
                    }
                    effects::with_generator(&rows[row], &generator)
                }
            };
            with_rows(rows, Some(row))
        }
    }
}

fn replace_row(
    draft: &Map<String, Value>,
    key: &str,
    row: usize,
    change: impl FnOnce(&Map<String, Value>) -> Map<String, Value>,
) -> Result<(Map<String, Value>, Outcome), String> {
    let mut rows = draft
        .get(key)
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let Some(current) = rows.get(row).and_then(Value::as_object) else {
        return Err(format!("plant.edit names no row {row} of {key}"));
    };
    rows[row] = Value::Object(change(current));
    let mut next = draft.clone();
    next.insert(key.into(), Value::Array(rows));
    Ok((next, Outcome::Applied { index: Some(row) }))
}
