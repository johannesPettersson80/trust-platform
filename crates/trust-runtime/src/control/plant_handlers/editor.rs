//! The plant setup editor's rules (specification 47 §6 to §13), which the VS Code editor shows and
//! never keeps a copy of: `plant.edit` takes the editor's draft, applies the edit it asks for, and
//! answers with the draft, every validation issue, what the editor shows besides the values, and
//! the plant candidate the draft saves as; `plant.signals` builds the PLC signals a plant can bind
//! to; `plant.capabilities` carries the editor's tables.

use serde_json::{json, Map, Value};

mod actions;
mod candidate;
mod checks;
mod effects;
mod events;
mod issues;
mod js;
mod piecewise;
mod ports;
mod signals;
mod state_space;
mod tables;
mod temperature;
mod view;
mod vocabulary;

#[cfg(test)]
mod tests;

pub(super) use signals::build as build_signal_catalog;
pub(super) use vocabulary::editor_tables;

/// What `plant.edit` needs to know of the project beyond the draft.
pub(super) struct Project {
    /// The IDs of the saved plants, in saved order.
    pub(super) saved_ids: Vec<String>,
    /// The feedback targets of each saved plant, by plant ID.
    pub(super) saved_feedback: Vec<(String, Vec<String>)>,
    /// The project's default plant step in milliseconds, when it is known (specification 46 §3.1).
    pub(super) step_ms: Option<f64>,
}

/// The answer to a `plant.edit` request.
pub(super) struct Answer {
    pub(super) draft: Map<String, Value>,
    pub(super) outcome: Option<Value>,
    issues: Vec<issues::Issue>,
    pub(super) view: Value,
    pub(super) candidate: Value,
}

impl Answer {
    /// Whether the draft may be saved: it has no issue.
    pub(super) fn can_save(&self) -> bool {
        self.issues.is_empty()
    }

    /// Adds the runtime's own judgement of the complete configuration (specification 47 §13.1): it
    /// stays authoritative, so a candidate the editor's checks pass and the runtime refuses still
    /// blocks Save, with the runtime's reason.
    pub(super) fn add_configuration_issue(&mut self, reason: String) {
        self.issues.push(issues::Issue::new(
            "validation_failed",
            "",
            reason,
            issues::Target::Configuration,
        ));
    }

    /// The issues as the answer carries them.
    pub(super) fn issues_json(&self) -> Value {
        let plant_id = js::text(&self.draft, "plantId");
        Value::Array(
            self.issues
                .iter()
                .map(|issue| issue.to_json(plant_id))
                .collect(),
        )
    }
}

/// Applies `action` to `draft` (no action: the draft as it is) and checks the result.
///
/// # Errors
///
/// The reason a request is malformed: a draft that is not an object, or an unknown action.
pub(super) fn edit(
    draft: &Value,
    action: Option<&Value>,
    signals: Option<&Value>,
    project: &Project,
) -> Result<Answer, String> {
    let draft = draft
        .as_object()
        .ok_or_else(|| "plant.edit draft must be an object".to_string())?;
    let catalog = signals::Catalog::from_json(signals);
    // The plant this draft edits is not "another" plant: its saved ID and feedback are its own.
    let original = draft
        .get("originalPlantId")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let other_ids: Vec<String> = project
        .saved_ids
        .iter()
        .filter(|id| id.as_str() != original)
        .cloned()
        .collect();
    let other_feedback: Vec<String> = project
        .saved_feedback
        .iter()
        .filter(|(id, _)| id.as_str() != original)
        .flat_map(|(_, targets)| targets.iter().cloned())
        .collect();
    let (draft, outcome) = match action {
        None => (draft.clone(), None),
        Some(action) => {
            let action = action
                .as_object()
                .ok_or_else(|| "plant.edit action must be an object".to_string())?;
            let context = actions::ActionContext {
                saved_ids: &project.saved_ids,
                other_feedback: &other_feedback,
                catalog: &catalog,
            };
            let (draft, outcome) = actions::apply(draft, action, &context)?;
            (draft, Some(outcome.to_json()))
        }
    };
    let issues = checks_of(
        &draft,
        &other_ids,
        &other_feedback,
        &catalog,
        project.step_ms,
    );
    Ok(Answer {
        view: view::view(&draft),
        candidate: candidate::candidate(&draft),
        draft,
        outcome,
        issues,
    })
}

/// Every issue of the draft, in the order the editor's summary lists them: identity, connections,
/// dynamics (fields, matrix cells, input delays, curve points), actuator, signal conversions,
/// effects and scenario events.
fn checks_of(
    draft: &Map<String, Value>,
    other_ids: &[String],
    other_feedback: &[String],
    catalog: &signals::Catalog,
    project_step_ms: Option<f64>,
) -> Vec<issues::Issue> {
    let model = js::text(draft, "model");
    let mut issues = checks::identity(draft, other_ids);
    issues.extend(ports::checks(draft, catalog, other_feedback));
    issues.extend(checks::dynamics(draft));
    if model == "state_space" {
        issues.extend(state_space::cells(draft));
        issues.extend(state_space::input_delays(draft, project_step_ms));
    }
    if model == "piecewise_linear" {
        issues.extend(piecewise::checks(js::object(draft, "parameters")));
    }
    issues.extend(checks::actuator(draft));
    issues.extend(checks::signal_engineering(draft));
    issues.extend(effects::checks(draft));
    issues.extend(events::checks(draft, &view::event_context(draft)));
    // Specification 47 §7.1: the example's unit assumptions are confirmed before saving.
    if view::temperature_view(draft) && !temperature::units_confirmed(draft) {
        issues.push(issues::Issue::new(
            "temperature_units_unconfirmed",
            "",
            "Confirm that the PLC signals use the displayed heater-percent and temperature units, or configure their conversion.",
            issues::Target::TemperatureUnits,
        ));
    }
    issues
}

/// The answer's JSON fields besides the contract envelope.
pub(super) fn answer_fields(answer: &Answer) -> Map<String, Value> {
    let mut fields = Map::new();
    fields.insert("draft".into(), Value::Object(answer.draft.clone()));
    if let Some(outcome) = &answer.outcome {
        fields.insert("outcome".into(), outcome.clone());
    }
    fields.insert("issues".into(), answer.issues_json());
    fields.insert("can_save".into(), json!(answer.can_save()));
    fields.insert("view".into(), answer.view.clone());
    fields.insert("candidate".into(), answer.candidate.clone());
    fields
}
