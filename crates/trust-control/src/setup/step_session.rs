//! The session of the step-by-step loop setup (specification 35, B1.3 and C12): the answers the engineer gave, in the order their
//! questions were asked, and the navigation between them. Each `controlStudio/setupStep` request carries the session the last answer
//! returned. This module answers it with the question that comes next, the words of a refused answer, or the finished draft and the
//! sentence the engineer confirms before the save. The page keeps the session as it is and asks no question of its own; this is the port
//! of `setupNavigation.ts`.
//!
//! The rules: an answer is stored once its question's rule accepts it, and a changed answer discards the answers after it. Back returns to
//! the nearest earlier answer the engineer may still change; the answers before that one are replayed without asking, and the questions
//! from it on are asked again with their earlier answers shown. A stored answer whose key is not the question at its place is discarded
//! with the answers after it.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::catalog::CatalogAnswer;
use super::configure_channels::accept_rate;
use super::messages::{QuestionKind, SetupQuestion, SetupStepResult};
use super::prompts::{
    accept_number, accept_text, back_target, changed_answer_discards_later, next_step, sole_task,
    ChoiceItem, SetupStep, SetupStepAnswers, SetupWorkflow, StepId, StepKind,
};
use super::step_answers::{decode_answers, STRUCTURE};
use super::texts::prompts as text;

/// One answer the session holds: the key of its question, the answer as the engineer gave it, whether the engineer may still change it, and
/// whether its question's rule accepted it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionAnswer {
    /// The draft key of the question.
    pub key: String,
    /// The answer as the engineer gave it: text, a number as text, a choice's value, or a file's text.
    pub raw: Value,
    /// Whether the engineer may still change the answer; Back returns to it.
    pub editable: bool,
    /// Whether the question's rule accepted the answer.
    pub accepted: bool,
}

/// The session the page holds: the answers in the order their questions were asked, and how many of them, from the first, a Back replays
/// without asking.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StepSession {
    /// The answers, in the order of their questions.
    pub answers: Vec<SessionAnswer>,
    /// The number of answers, from the first, that a Back replays without asking.
    pub replay_until: usize,
}

/// What the engineer did at the question the session shows.
#[derive(Debug, Clone, PartialEq)]
pub enum StepAction {
    /// The first request of the setup: nothing is answered yet.
    Start,
    /// The engineer answered the question with this answer.
    Answer(Value),
    /// The engineer pressed Back; the text the question shows, if any, is kept for when the question is asked again.
    Back(Option<Value>),
}

/// The session a request carries: none at the start of the setup, else the session the engine wrote.
///
/// # Errors
/// The session is not one the engine wrote.
pub fn session_of(value: Option<Value>) -> Result<StepSession, String> {
    match value {
        None => Ok(StepSession::default()),
        Some(value) => {
            serde_json::from_value(value).map_err(|_| text::session_unreadable().to_owned())
        }
    }
}

/// Answers one request of the step-by-step setup: the session it carries and what the engineer did at the question the session shows. The
/// reply is the session to keep, the question to ask or the finished draft, and the words of a refused answer.
#[must_use]
pub fn respond(
    mut session: StepSession,
    catalog: &CatalogAnswer,
    action: StepAction,
    runtime_id: &str,
) -> SetupStepResult {
    if catalog.signals.is_empty() {
        return SetupStepResult {
            session: session_value(&session),
            question: None,
            refusal: Some(text::no_signals().to_owned()),
            can_go_back: false,
            previous: None,
            draft: None,
            summary: None,
        };
    }
    let mut refusal = None;
    match action {
        StepAction::Start => {}
        StepAction::Answer(raw) => {
            if let Some(asked) = walk(&mut session, catalog).asked {
                match convert(&asked.step, &raw) {
                    Ok(_) => store(&mut session, &asked, raw, true),
                    Err(reason) => refusal = Some(reason),
                }
            }
        }
        StepAction::Back(raw) => {
            if let Some(asked) = walk(&mut session, catalog).asked {
                let editable: Vec<bool> = session
                    .answers
                    .iter()
                    .map(|answer| answer.editable)
                    .collect();
                if let Some(target) = back_target(&editable, asked.index) {
                    let shown = raw
                        .or_else(|| asked.previous.clone())
                        .unwrap_or(Value::Null);
                    store(&mut session, &asked, shown, false);
                    session.replay_until = target;
                }
            }
        }
    }
    finish(session, catalog, runtime_id, refusal)
}

/// The question the walk stopped at: its place among the answers, the question, and the answer the engineer gave it before.
struct Asked {
    index: usize,
    step: SetupStep,
    previous: Option<Value>,
}

/// The draft the stored answers make, and the question the walk stops at; `asked` is `None` once every question is answered.
struct Walk {
    draft: Map<String, Value>,
    asked: Option<Asked>,
}

/// Walks the questions from the start. A question with a stored answer that Back replays (an accepted one, before the replay limit) is
/// answered without asking; the first other question is the one asked (`setupNavigation.ts`, `answer`).
fn walk(session: &mut StepSession, catalog: &CatalogAnswer) -> Walk {
    let mut draft = Map::new();
    let mut index = 0;
    loop {
        let Some(step) = next_question(catalog, &draft) else {
            return Walk { draft, asked: None };
        };
        let key = key_of(step.step);
        if session
            .answers
            .get(index)
            .is_some_and(|answer| answer.key != key)
        {
            session.answers.truncate(index);
        }
        let stored = session.answers.get(index).cloned();
        let replayed = match stored.as_ref() {
            Some(answer) if index < session.replay_until && answer.accepted => {
                convert(&step, &answer.raw).ok()
            }
            _ => None,
        };
        if let Some(value) = replayed {
            draft.insert(key.to_owned(), value);
            index += 1;
            continue;
        }
        session.replay_until = session.replay_until.min(index);
        let previous = stored.map(|answer| answer.raw);
        return Walk {
            draft,
            asked: Some(Asked {
                index,
                step,
                previous,
            }),
        };
    }
}

/// The question after the draft's answers, or `None` when every question is answered.
fn next_question(catalog: &CatalogAnswer, draft: &Map<String, Value>) -> Option<SetupStep> {
    let answers = decode_answers(&Value::Object(draft.clone()), catalog).ok()?;
    next_step(catalog, &answers)
}

/// Stores the answer at the question's place. An accepted answer that differs from the one it replaces discards the answers after it; a
/// Back-marked answer is the text the question shows, which is not accepted.
fn store(session: &mut StepSession, asked: &Asked, raw: Value, accepted: bool) {
    let key = key_of(asked.step.step).to_owned();
    let discards = accepted
        && changed_answer_discards_later(
            session.answers.get(asked.index).map(|answer| &answer.raw),
            &raw,
        );
    if discards {
        session.answers.truncate(asked.index + 1);
    }
    let answer = SessionAnswer {
        key,
        raw,
        editable: true,
        accepted,
    };
    match session.answers.get_mut(asked.index) {
        Some(slot) => *slot = answer,
        None => session.answers.push(answer),
    }
    if accepted {
        // The next request walks past this question without asking it again: the walk replays the answers up to the question after it.
        session.replay_until = session.replay_until.max(asked.index + 1);
    }
}

/// The value an answer stores under its question's key, or the words that refuse it: text is read by `accept_text`, a number by its rule,
/// a rate by `accept_rate` (kept as the text the draft reads back), a choice must be one of the items' values, and a file is its text.
fn convert(step: &SetupStep, raw: &Value) -> Result<Value, String> {
    match &step.kind {
        StepKind::Text => accept_text(&typed(raw)?).map(Value::from),
        StepKind::Number(rule) => accept_number(&typed(raw)?, *rule).map(number_value),
        StepKind::OptionalRate => {
            let answer = typed(raw)?;
            accept_rate(&answer).map(|_| Value::from(answer))
        }
        StepKind::Choice(items) => offered(items, raw),
        StepKind::File => typed(raw).map(Value::from),
    }
}

/// The text an answer carries; an answer that is not text is refused as a blank one.
fn typed(raw: &Value) -> Result<String, String> {
    raw.as_str()
        .map(str::to_owned)
        .ok_or_else(|| text::enter_value().to_owned())
}

/// A finite number as the draft holds it.
fn number_value(number: f64) -> Value {
    serde_json::Number::from_f64(number).map_or(Value::Null, Value::Number)
}

/// The answer of a choice, when one of the items the question offers stands for it.
fn offered(items: &[ChoiceItem], raw: &Value) -> Result<Value, String> {
    if items.iter().any(|item| item.value == *raw) {
        Ok(raw.clone())
    } else {
        Err(text::not_among_offered().to_owned())
    }
}

/// The reply to the session as the walk leaves it: the question to ask, or the finished draft and its confirmation.
fn finish(
    mut session: StepSession,
    catalog: &CatalogAnswer,
    runtime_id: &str,
    refusal: Option<String>,
) -> SetupStepResult {
    let walked = walk(&mut session, catalog);
    let session_json = session_value(&session);
    if let Some(asked) = walked.asked {
        let editable: Vec<bool> = session
            .answers
            .iter()
            .map(|answer| answer.editable)
            .collect();
        SetupStepResult {
            session: session_json,
            question: Some(question_of(&asked.step)),
            refusal,
            can_go_back: back_target(&editable, asked.index).is_some(),
            previous: asked.previous,
            draft: None,
            summary: None,
        }
    } else {
        let mut draft = walked.draft;
        draft.insert("structure".to_owned(), Value::from(STRUCTURE));
        let draft = Value::Object(draft);
        match decode_answers(&draft, catalog) {
            Ok(answers) => SetupStepResult {
                session: session_json,
                question: None,
                refusal,
                can_go_back: false,
                previous: None,
                draft: Some(draft),
                summary: Some(summary_of(&answers, runtime_id)),
            },
            Err(failure) => SetupStepResult {
                session: session_json,
                question: None,
                refusal: Some(failure.message),
                can_go_back: false,
                previous: None,
                draft: None,
                summary: None,
            },
        }
    }
}

/// The question as the page asks it: its key, its phase and title, the kind of answer it takes, its sentence and its choices.
fn question_of(step: &SetupStep) -> SetupQuestion {
    let (kind, prompt, items) = match &step.kind {
        StepKind::Text => (QuestionKind::Text, None, Vec::new()),
        StepKind::Number(_) => (QuestionKind::Number, None, Vec::new()),
        StepKind::OptionalRate => (
            QuestionKind::Rate,
            Some(text::leave_blank_for_no_limit().to_owned()),
            Vec::new(),
        ),
        StepKind::Choice(items) => (QuestionKind::Choice, None, items.clone()),
        StepKind::File => (QuestionKind::File, None, Vec::new()),
    };
    SetupQuestion {
        key: key_of(step.step).to_owned(),
        phase: step.phase.number(),
        phase_name: step.phase.name().to_owned(),
        title: step.title.clone(),
        kind,
        prompt,
        items,
    }
}

/// The sentence a finished setup confirms before the save: its name, its runtime, its process variable and output with their units, its
/// period, and what the loop does with the program (`configureLoop.ts`).
fn summary_of(answers: &SetupStepAnswers, runtime_id: &str) -> String {
    let period = sole_task(answers.controller.as_ref())
        .map(|task| task.interval_ms)
        .or(answers.task_period_ms)
        .unwrap_or_default();
    let process = answers
        .process_variable
        .as_ref()
        .map_or("", |signal| signal.symbol.as_str());
    let output = answers
        .controller_output
        .as_ref()
        .map_or("", |signal| signal.symbol.as_str());
    let process_unit = answers.process_unit.as_deref().unwrap_or_default();
    let output_unit = answers.output_unit.as_deref().unwrap_or_default();
    let signals =
        format!("PV: {process} ({process_unit}); output: {output} ({output_unit}); {period} ms.");
    let sentence = match answers.workflow {
        Some(SetupWorkflow::Proportional) => text::proportional_summary(),
        Some(SetupWorkflow::Adapter) => text::adapter_summary(),
        _ => text::observation_summary(),
    };
    let name = answers.name.as_deref().unwrap_or_default();
    text::save_confirmation(name, runtime_id, &signals, sentence)
}

fn session_value(session: &StepSession) -> Value {
    serde_json::to_value(session).unwrap_or(Value::Null)
}

/// The draft key of a question, as `step_answers` reads it.
#[must_use]
pub fn key_of(step: StepId) -> &'static str {
    match step {
        StepId::LoopName => "name",
        StepId::Controller => "controllerInstance",
        StepId::ControllerInstance => "customInstance",
        StepId::ControllerType => "customType",
        StepId::TaskPeriod => "taskPeriodMs",
        StepId::ProcessVariable => "processVariable",
        StepId::ControllerOutput => "controllerOutput",
        StepId::Setpoint => "setpoint",
        StepId::Workflow => "workflow",
        StepId::ProcessUnit => "processUnit",
        StepId::ProcessMinimum => "processMinimum",
        StepId::ProcessMaximum => "processMaximum",
        StepId::OutputUnit => "outputUnit",
        StepId::OutputMinimum => "outputMinimum",
        StepId::OutputMaximum => "outputMaximum",
        StepId::ProportionalGain => "gain",
        StepId::GainMinimum => "gainMinimum",
        StepId::GainMaximum => "gainMaximum",
        StepId::ManualCommand => "manualCommand",
        StepId::ManualMode => "manualMode",
        StepId::ManualModePolarity => "manualWhenTrue",
        StepId::ControllerTask => "controllerTask",
        StepId::ObservationDuration => "observationDurationSeconds",
        StepId::AdapterFile => "adapterMapping",
        StepId::ChannelMode => "channelMode",
        StepId::ActuatorCapability => "actuator",
        StepId::RisingRate => "risingRate",
        StepId::FallingRate => "fallingRate",
        StepId::AppliedFeedback => "feedback",
    }
}

#[cfg(test)]
#[path = "step_session_tests.rs"]
mod tests;
