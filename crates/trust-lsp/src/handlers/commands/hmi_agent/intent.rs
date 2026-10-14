//! The operator task, rationale and assumptions a composition records (specification 43, AT-04).
//!
//! Port of `agent/intent.ts` (`compositionIntent`). The notes are persisted with the proposal and
//! shown in the builder's review. They are not PLC code or runtime authority.

use serde::Serialize;
use serde_json::Value;

use super::error::{AgentError, AgentResult};
use super::patterns::{pattern, PatternId};

/// Where the operator task came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum TaskSource {
    /// The request named the task.
    Request,
    /// The selected pattern states the task.
    Pattern,
    /// A refinement of an existing page, which keeps its title as the task.
    SourcePage,
}

/// The notes a composition records with its proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct DesignIntent {
    /// What the operator does with the page.
    pub(super) operator_task: String,
    /// Where the task came from.
    pub(super) task_source: TaskSource,
    /// Why the design has its shape.
    pub(super) rationale: String,
    /// What the design takes for granted, each listed once.
    pub(super) assumptions: Vec<String>,
}

/// The assumptions every composition records, first.
const STANDING_ASSUMPTIONS: [&str; 2] = [
    "Preview values are synthetic.",
    "Process connections are supplied, not inferred.",
];

/// Builds the notes of a composition request (`compositionIntent`). `title` is the page's title,
/// used when neither the request nor a pattern names a task.
///
/// # Errors
///
/// Returns `INVALID_ARGUMENT` when the request's `assumptions` is not a list of text.
pub(super) fn composition_intent(input: &Value, title: &str) -> AgentResult<DesignIntent> {
    let pattern_id = input
        .get("pattern")
        .and_then(Value::as_str)
        .and_then(PatternId::parse);
    let pattern_notes = pattern_id.map(pattern);
    let request_task = input.get("operatorTask").and_then(Value::as_str);
    let request_rationale = input.get("rationale").and_then(Value::as_str);
    let operator_task = match (request_task, pattern_notes) {
        (Some(task), _) => task.to_owned(),
        (None, Some(notes)) => notes.task.to_owned(),
        (None, None) => format!("Refine {title}"),
    };
    let task_source = match (request_task, pattern_notes) {
        (Some(_), _) => TaskSource::Request,
        (None, Some(_)) => TaskSource::Pattern,
        (None, None) => TaskSource::SourcePage,
    };
    let rationale = match (request_rationale, pattern_notes) {
        (Some(rationale), _) => rationale.to_owned(),
        (None, Some(notes)) => format!(
            "Uses {} with the supplied components and layout constraints.",
            notes.title.to_lowercase()
        ),
        (None, None) => {
            "Preserves the existing page as the starting point for refinement.".to_owned()
        }
    };
    let mut assumptions: Vec<String> = STANDING_ASSUMPTIONS
        .iter()
        .map(|text| (*text).to_owned())
        .collect();
    match input.get("assumptions").filter(|value| !value.is_null()) {
        None => {}
        Some(Value::Array(list)) => {
            for entry in list {
                let text = entry
                    .as_str()
                    .ok_or_else(|| AgentError::invalid("assumptions must be a list of text."))?;
                assumptions.push(text.to_owned());
            }
        }
        Some(_) => return Err(AgentError::invalid("assumptions must be a list of text.")),
    }
    let mut unique: Vec<String> = Vec::with_capacity(assumptions.len());
    for assumption in assumptions {
        if !unique.contains(&assumption) {
            unique.push(assumption);
        }
    }
    Ok(DesignIntent {
        operator_task,
        task_source,
        rationale,
        assumptions: unique,
    })
}
