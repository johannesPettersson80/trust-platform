//! The readable text of the command line (specification 35, §0.1b). Each function reads the fields of
//! its answer and holds no rule: which stage is available, and what a refusal means, come from the
//! answer itself.

use serde_json::Value;
use trust_control::failure::{Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{LoopEntry, OpenResult, OperationState, OperationUpdate};
use trust_control::stages::StageAvailability;
use trust_control::tuning::texts::{engineering, unit_suffix};

/// The text of a reset (A8.4): a new working identification study started, and the previous one is history.
pub const RESET_TEXT: &str =
    "The working identification study was started again; the previous study is in history.";

/// The text of a verb that the second Ctrl-C ended (§0.1b, A8.6).
pub const LEFT_TEXT: &str = "The experiment, its recording and the journal are left to the runtime; identify --resume follows it to its end.";

/// The last line of a validate result (B5.1): Validate never writes to the controller.
pub const NOT_WRITTEN_TEXT: &str = "Nothing was written to the controller.";

/// What the text of a tune result prints for a field that the result lacks.
pub(super) const MISSING: &str = "—";

/// Every next action (A11.2), to read the action of a failure that a method answered as JSON.
const NEXT_ACTIONS: [NextAction; 15] = [
    NextAction::ConnectRuntime,
    NextAction::StartSimulator,
    NextAction::OpenLoopSetup,
    NextAction::OpenLimits,
    NextAction::OpenRequirements,
    NextAction::OpenChannels,
    NextAction::ChooseMethod,
    NextAction::Identify,
    NextAction::Tune,
    NextAction::Validate,
    NextAction::Approve,
    NextAction::Restore,
    NextAction::Resume,
    NextAction::Wait,
    NextAction::None,
];

/// The text of an open answer: the project's loops, or one loop's stages and journaled operations.
#[must_use]
pub fn open_text(result: &OpenResult) -> String {
    match result {
        OpenResult::Loops { loops } => loops_text(loops),
        OpenResult::Opened {
            loop_definition,
            stages,
            reattached,
            ..
        } => opened_text(loop_definition, stages, reattached),
    }
}

fn loops_text(loops: &[LoopEntry]) -> String {
    if loops.is_empty() {
        return "This project declares no control loops.".to_string();
    }
    loops.iter().map(loop_line).collect::<Vec<_>>().join("\n")
}

fn loop_line(entry: &LoopEntry) -> String {
    match (&entry.loop_id, &entry.name) {
        (Some(id), Some(name)) => format!("{id}  {name}  ({})", entry.file),
        (Some(id), None) => format!("{id}  ({})", entry.file),
        (None, _) => format!(
            "{}: {}",
            entry.file,
            entry
                .problem
                .as_deref()
                .unwrap_or("This file is not a usable loop.")
        ),
    }
}

fn opened_text(
    loop_definition: &LoopDefinition,
    stages: &[StageAvailability],
    reattached: &[String],
) -> String {
    let mut lines = vec![
        format!("Loop {} ({})", loop_definition.name, loop_definition.id),
        "Stages:".to_string(),
    ];
    for stage in stages {
        lines.push(format!("  {:?}: {}", stage.stage, stage.label));
        // A stage that does not apply to the loop says why (C12), on the line under it.
        if let Some(reason) = stage.reason {
            lines.push(format!("    {reason}"));
        }
    }
    lines.push(if reattached.is_empty() {
        "Journaled operations: none".to_string()
    } else {
        format!("Journaled operations: {}", reattached.join(", "))
    });
    lines.join("\n")
}

/// The text of a refusal or failure on standard error: what failed, then the next action's words
/// when the action has words.
#[must_use]
pub fn failure_text(failure: &Failure) -> String {
    failure_lines(
        &failure.message,
        failure.detail.as_deref(),
        failure.next_action.label(),
    )
}

/// The text of a failure that a method answered as JSON (`{"failure": …}`): the lines of
/// [`failure_text`], read from the object's fields.
#[must_use]
pub fn answered_failure_text(failure: &Value) -> String {
    failure_lines(
        text_field(failure, "message").unwrap_or_default(),
        text_field(failure, "detail"),
        text_field(failure, "nextAction").map_or("", next_action_label),
    )
}

/// What failed, then the next action's words when the action has words.
fn failure_lines(message: &str, detail: Option<&str>, label: &str) -> String {
    let mut lines = vec![match detail {
        Some(detail) if detail.starts_with(message) => detail.to_string(),
        Some(detail) => format!("{message}\n{detail}"),
        None => message.to_string(),
    }];
    if !label.is_empty() {
        lines.push(format!("Next action: {label}"));
    }
    lines.join("\n")
}

/// The text of field `name` of a JSON object, when it is text.
pub(super) fn text_field<'a>(object: &'a Value, name: &str) -> Option<&'a str> {
    object.get(name).and_then(Value::as_str)
}

/// The words of the next action that `word` names in a failure's JSON; none for a word that names
/// no action.
fn next_action_label(word: &str) -> &'static str {
    NEXT_ACTIONS
        .into_iter()
        .find(|action| serde_json::to_value(action).is_ok_and(|value| value == word))
        .map_or("", NextAction::label)
}

/// The text of an identification's result: the readable text that `trust-control` composed from
/// the model (its type, "Provisional" when it is, its equation and its key numbers). A result
/// without that text is shown as its JSON.
#[must_use]
pub fn identify_text(result: &Value) -> String {
    result
        .get("text")
        .and_then(Value::as_str)
        .map_or_else(|| result.to_string(), str::to_string)
}

/// A number with its unit, as the chart prints numbers; "—" when the result lacks it.
pub(super) fn quantity(value: Option<f64>, unit: &str) -> String {
    value.map_or_else(
        || MISSING.to_string(),
        |value| format!("{}{}", engineering(value, None), unit_suffix(unit)),
    )
}

/// Field `name` of a JSON object, or null when the object lacks it, so that a missing field reads like a field without a value.
pub(super) fn member<'a>(object: &'a Value, name: &str) -> &'a Value {
    static NULL: Value = Value::Null;
    object.get(name).unwrap_or(&NULL)
}

/// The number in field `name` of a JSON object, when it is a number.
pub(super) fn number_field(object: &Value, name: &str) -> Option<f64> {
    object.get(name).and_then(Value::as_f64)
}

/// The text of a validate result (§0.1b, B5): the model check, then one line for each scenario with the plant it ran against
/// and the status of its current and its candidate run, followed by one line for each of the candidate's requirements with
/// its value, limit, unit and source (B5.4), then the overall result, whether Apply is available, and last that nothing was
/// written to the controller (B5.1). It reads the fields of the result and decides nothing; a field the result lacks reads
/// as "—".
#[must_use]
pub fn validate_text(result: &Value) -> String {
    let mut lines = vec![model_check_line(result)];
    for scenario in items(result, "scenarios") {
        lines.push(scenario_line(scenario));
        lines.extend(
            items(member(scenario, "candidate"), "requirements").map(|requirement| {
                verdict_line(
                    &format!("  {}", text_field(requirement, "id").unwrap_or(MISSING)),
                    requirement,
                )
            }),
        );
    }
    lines.extend([
        format!(
            "Overall: {}.",
            text_field(result, "status").unwrap_or(MISSING)
        ),
        apply_line(result),
        NOT_WRITTEN_TEXT.to_string(),
    ]);
    lines.join("\n")
}

/// Whether a validate result makes Apply available (B5.4). Only the result's own `applyAvailable` says so; a result without
/// it does not.
#[must_use]
pub fn apply_available(result: &Value) -> bool {
    result
        .get("applyAvailable")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// The model check's line (B5.2): its status, its normalized error and the limit, and its reason; "not run" when the result
/// carries no check (the model was already qualified).
fn model_check_line(result: &Value) -> String {
    let check = member(result, "modelCheck");
    if check.is_null() {
        return "Model check: not run.".to_string();
    }
    let reason =
        text_field(check, "reason").map_or_else(String::new, |reason| format!(" {reason}"));
    format!(
        "Model check: {}. Normalized error {}, limit {}.{reason}",
        text_field(check, "status").unwrap_or(MISSING),
        quantity(number_field(check, "normalizedError"), ""),
        quantity(number_field(check, "limit"), ""),
    )
}

/// A scenario's line (B5.3): the plant it ran against and the status of its current and its candidate run, as `Scenario
/// setpoint-step on heater: current Pass, candidate Fail.`
fn scenario_line(scenario: &Value) -> String {
    format!(
        "Scenario {} on {}: current {}, candidate {}.",
        text_field(scenario, "id").unwrap_or(MISSING),
        text_field(scenario, "plant").unwrap_or(MISSING),
        text_field(member(scenario, "current"), "status").unwrap_or(MISSING),
        text_field(member(scenario, "candidate"), "status").unwrap_or(MISSING),
    )
}

/// A verdict as `label: Pass. Value 120.4 s, limit 300 s. Source: <source>. <reason>`: its status, its value and limit with
/// their unit (a dash for one it lacks), then its source and the sentence that says what was found, when it has them.
fn verdict_line(label: &str, verdict: &Value) -> String {
    let unit = text_field(verdict, "unit").unwrap_or_default();
    let source = text_field(verdict, "source")
        .map_or_else(String::new, |source| format!(" Source: {source}."));
    let reason =
        text_field(verdict, "reason").map_or_else(String::new, |reason| format!(" {reason}"));
    format!(
        "{label}: {}. Value {}, limit {}.{source}{reason}",
        text_field(verdict, "status").unwrap_or(MISSING),
        quantity(number_field(verdict, "value"), unit),
        quantity(number_field(verdict, "limit"), unit),
    )
}

/// Apply's line (B5.4): whether Apply is available.
fn apply_line(result: &Value) -> String {
    if apply_available(result) {
        "Apply is available.".to_string()
    } else {
        "Apply is not available.".to_string()
    }
}

/// The items of the array in field `name` of a JSON object; none when the object lacks the array.
pub(super) fn items<'a>(object: &'a Value, name: &str) -> impl Iterator<Item = &'a Value> {
    object
        .get(name)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
}

/// The text of a finished `cancel` (§0.1b, E11.6): the operations it cancelled. Their recordings are kept in the study, and the
/// journal is cleared for them. A result that names none says so.
#[must_use]
pub fn cancelled_text(result: &Value) -> String {
    let cancelled: Vec<&str> = result
        .get("cancelled")
        .and_then(Value::as_array)
        .map_or_else(Vec::new, |items| {
            items.iter().filter_map(Value::as_str).collect()
        });
    if cancelled.is_empty() {
        "No operation was cancelled.".to_string()
    } else {
        format!(
            "Cancelled {}. Its recording is kept in the study, and the journal is cleared for it.",
            cancelled.join(", ")
        )
    }
}

/// The one-line text of a progress update on standard error (§0.1b).
#[must_use]
pub fn progress_line(update: &OperationUpdate) -> String {
    let words = match update.state {
        OperationState::Running => update
            .progress
            .clone()
            .unwrap_or_else(|| "running".to_string()),
        OperationState::Finished => "finished".to_string(),
        OperationState::Failed => update
            .failure
            .as_ref()
            .map_or_else(|| "failed".to_string(), |failure| failure.message.clone()),
        OperationState::Cancelled => "cancelled".to_string(),
    };
    format!(
        "{} ({}): {}",
        update.command,
        update.operation_id,
        words.replace(['\r', '\n'], " ")
    )
}

/// The text of a cancel answer: what the operation was when the cancellation reached it.
#[must_use]
pub fn cancel_text(update: &OperationUpdate) -> String {
    match update.state {
        OperationState::Running => format!("Cancellation requested for {}.", update.operation_id),
        OperationState::Finished => format!("{} had already finished.", update.operation_id),
        OperationState::Failed => format!("{} had already failed.", update.operation_id),
        OperationState::Cancelled => format!("{} was already cancelled.", update.operation_id),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use trust_control::failure::Cause;

    use super::*;

    fn update(state: OperationState) -> OperationUpdate {
        OperationUpdate {
            operation_id: "operation-3".to_string(),
            command: "identify".to_string(),
            state,
            stage: None,
            progress: None,
            result: None,
            failure: None,
            message: None,
            stages: None,
        }
    }

    #[test]
    fn a_stage_that_does_not_apply_says_why_under_its_label() {
        // Specification 35, C12: a loop without a controller law has Tune Not applicable, and the text gives the engine's reason.
        use trust_control::stages::{stage_availability, StageEvidence};
        let lesson = LoopDefinition::decode(include_str!(
            "../../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/control-loop.json"
        ))
        .expect("lesson");
        let stages = stage_availability(&StageEvidence {
            controller_stages_apply: false,
            ..StageEvidence::default()
        });
        let text = opened_text(&lesson, &stages, &[]);
        assert!(text.contains("  Identify: Needs data\n"), "{text}");
        assert!(
            text.contains(
                "  Tune: Not applicable\n    This loop has no controller law, so there is nothing to tune.\n"
            ),
            "{text}"
        );
    }

    #[test]
    fn the_loops_are_listed_one_per_line_with_the_problem_of_each_broken_file() {
        let loops = vec![
            LoopEntry {
                file: "control-loop.json".to_string(),
                loop_id: Some("tank".to_string()),
                name: Some("Tank".to_string()),
                problem: None,
            },
            LoopEntry {
                file: "control-loops/b.json".to_string(),
                loop_id: None,
                name: None,
                problem: Some("b.json could not be read.".to_string()),
            },
        ];
        assert_eq!(
            loops_text(&loops),
            "tank  Tank  (control-loop.json)\ncontrol-loops/b.json: b.json could not be read."
        );
        assert_eq!(loops_text(&[]), "This project declares no control loops.");
    }

    #[test]
    fn a_failure_is_its_whole_text_then_its_next_action_words() {
        let set_up = Failure::new(
            "This loop is not set up. Configure it.",
            Cause::Implementation,
            NextAction::OpenLoopSetup,
        );
        assert_eq!(
            failure_text(&set_up),
            "This loop is not set up. Configure it.\nNext action: Loop setup…"
        );
        assert_eq!(
            failure_text(&Failure::unreachable("connection refused")),
            "Control Studio cannot reach the runtime.\nconnection refused\nNext action: Connect runtime…"
        );
        let waiting =
            Failure::from_runtime("stale_scan", "The program has not finished its first scan.");
        assert_eq!(
            failure_text(&waiting),
            "Wait for the running operation to finish.\nThe program has not finished its first scan."
        );
    }

    #[test]
    fn a_progress_update_is_one_line_naming_its_command_and_operation() {
        let mut running = update(OperationState::Running);
        running.progress = Some("Recording\nthe plant".to_string());
        assert_eq!(
            progress_line(&running),
            "identify (operation-3): Recording the plant"
        );
        assert_eq!(
            progress_line(&update(OperationState::Finished)),
            "identify (operation-3): finished"
        );
    }

    #[test]
    fn a_finished_cancel_names_the_operations_it_cancelled_or_says_that_none_was() {
        assert_eq!(
            cancelled_text(&json!({ "cancelled": ["rec-1"] })),
            "Cancelled rec-1. Its recording is kept in the study, and the journal is cleared for it."
        );
        assert_eq!(
            cancelled_text(&json!({ "cancelled": [] })),
            "No operation was cancelled."
        );
    }

    #[test]
    fn a_cancel_says_what_the_operation_was_when_the_cancellation_reached_it() {
        assert_eq!(
            cancel_text(&update(OperationState::Running)),
            "Cancellation requested for operation-3."
        );
        assert_eq!(
            cancel_text(&update(OperationState::Finished)),
            "operation-3 had already finished."
        );
    }

    #[test]
    fn a_failure_answered_as_json_reads_as_its_failure_text() {
        for failure in [
            Failure::new(
                "This loop is not set up. Configure it.",
                Cause::Implementation,
                NextAction::OpenLoopSetup,
            ),
            Failure::unreachable("connection refused"),
            Failure::from_runtime("stale_scan", "The program has not finished its first scan."),
            Failure::new("No model.", Cause::ModelOrData, NextAction::Identify),
            Failure::new("Nothing to do.", Cause::Unclassified, NextAction::None),
        ] {
            let answered = serde_json::to_value(&failure).expect("the failure's JSON");
            assert_eq!(answered_failure_text(&answered), failure_text(&failure));
        }
    }

    #[test]
    fn every_next_action_reads_back_from_its_json_word() {
        for action in NEXT_ACTIONS {
            // A next action added to trust-control stops this match from compiling until NEXT_ACTIONS names it.
            match action {
                NextAction::ConnectRuntime
                | NextAction::StartSimulator
                | NextAction::OpenLoopSetup
                | NextAction::OpenLimits
                | NextAction::OpenRequirements
                | NextAction::OpenChannels
                | NextAction::ChooseMethod
                | NextAction::Identify
                | NextAction::Tune
                | NextAction::Validate
                | NextAction::Approve
                | NextAction::Restore
                | NextAction::Resume
                | NextAction::Wait
                | NextAction::None => {}
            }
            let word = serde_json::to_value(action).expect("the action's word");
            assert_eq!(
                next_action_label(word.as_str().expect("a word")),
                action.label(),
                "{word}"
            );
        }
        assert_eq!(next_action_label("no-such-action"), "");
    }

    #[test]
    fn an_identification_reads_as_the_text_that_trust_control_composed() {
        let result = json!({
            "modelType": "First-order model with delay",
            "text": "First-order model with delay · Provisional\ny(s) = 0.21 e^(-12s) / (75s + 1) u(s)",
        });
        assert_eq!(
            identify_text(&result),
            "First-order model with delay · Provisional\ny(s) = 0.21 e^(-12s) / (75s + 1) u(s)"
        );
        assert_eq!(
            identify_text(&json!({ "stages": [] })),
            r#"{"stages":[]}"#,
            "a result without its text is shown as its JSON"
        );
    }

    /// A validate result as the engine answers it: a provisional model whose check passes, one scenario whose runs pass, and two
    /// requirements, one that passes and one that fails, so Apply is not available.
    fn validated() -> Value {
        json!({
            "modelCheck": {
                "status": "Pass",
                "modelRevision": "rev-1",
                "recordingId": "rec-4",
                "limit": 0.15,
                "normalizedError": 0.04,
                "reason": "The error is within the limit over the whole response.",
            },
            "scenarios": [{
                "id": "setpoint-step",
                "plant": "heater",
                "current": {
                    "runId": "run-1",
                    "projectRevision": "rev-p",
                    "evidence": "ST-executed",
                    "status": "Pass",
                    "requirements": [],
                },
                "candidate": {
                    "runId": "run-2",
                    "projectRevision": "rev-p",
                    "evidence": "ST-executed",
                    "status": "Fail",
                    "requirements": [
                        {
                            "id": "settling-time",
                            "status": "Pass",
                            "value": 120.37,
                            "limit": 300,
                            "unit": "s",
                            "source": "candidate run of setpoint-step",
                            "reason": "The response settles within 300 s.",
                        },
                        {
                            "id": "overshoot",
                            "status": "Fail",
                            "value": 14.2,
                            "limit": 10,
                            "unit": "%",
                            "source": "candidate run of setpoint-step",
                            "reason": "The overshoot is 14.2 %, above the 10 % limit.",
                        },
                    ],
                },
            }],
            "status": "Fail",
            "applyAvailable": false,
            "stages": [],
        })
    }

    #[test]
    fn a_validation_reads_as_its_model_check_scenarios_requirements_overall_result_and_apply() {
        assert_eq!(
            validate_text(&validated()),
            [
                "Model check: Pass. Normalized error 0.04, limit 0.15. The error is within the limit over the whole response.",
                "Scenario setpoint-step on heater: current Pass, candidate Fail.",
                "  settling-time: Pass. Value 120.4 s, limit 300 s. Source: candidate run of setpoint-step. The response settles within 300 s.",
                "  overshoot: Fail. Value 14.2 %, limit 10 %. Source: candidate run of setpoint-step. The overshoot is 14.2 %, above the 10 % limit.",
                "Overall: Fail.",
                "Apply is not available.",
                NOT_WRITTEN_TEXT,
            ]
            .join("\n")
        );
    }

    #[test]
    fn apply_is_available_only_when_the_result_says_so() {
        let mut passed = validated();
        passed["status"] = json!("Pass");
        passed["applyAvailable"] = json!(true);
        assert!(apply_available(&passed));
        assert!(validate_text(&passed).ends_with(&format!(
            "Overall: Pass.\nApply is available.\n{NOT_WRITTEN_TEXT}"
        )));
        assert!(
            !apply_available(&json!({ "status": "Pass" })),
            "a result without the flag does not make Apply available"
        );
    }

    #[test]
    fn a_check_the_result_lacks_is_not_run_and_a_missing_field_reads_as_a_dash() {
        let mut unchecked = validated();
        unchecked["modelCheck"] = Value::Null;
        assert_eq!(
            validate_text(&unchecked).lines().next(),
            Some("Model check: not run.")
        );
        let inconclusive = json!({
            "scenarios": [{
                "id": "switching",
                "candidate": {
                    "requirements": [{
                        "id": "dwell",
                        "status": "Inconclusive",
                        "unit": "s",
                        "reason": "No complete high dwell interval was observed.",
                    }],
                },
            }],
        });
        assert_eq!(
            validate_text(&inconclusive).lines().nth(1),
            Some("Scenario switching on —: current —, candidate —.")
        );
        assert_eq!(
            validate_text(&inconclusive).lines().nth(2),
            Some("  dwell: Inconclusive. Value —, limit —. No complete high dwell interval was observed.")
        );
        let nothing = [
            "Model check: not run.",
            "Overall: —.",
            "Apply is not available.",
            NOT_WRITTEN_TEXT,
        ]
        .join("\n");
        assert_eq!(validate_text(&json!({})), nothing);
        assert_eq!(validate_text(&Value::Null), nothing);
    }
}
