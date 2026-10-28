//! The session of the step-by-step setup (`step_session.rs`): the questions the engine asks in turn, the words of a refused answer, Back and
//! its replay, the discard of the answers after a changed one, and the finished draft with its confirmation (specification 35, B1.3).

use serde_json::{json, Value};

use super::*;
use crate::setup::catalog::{CompiledCatalog, CompiledController, CompiledSignal, CompiledTask};
use crate::setup::texts::prompts as words;

fn signal(symbol: &str, address: &str) -> CompiledSignal {
    CompiledSignal {
        symbol: symbol.into(),
        address: address.into(),
        iec_type: "LREAL".into(),
    }
}

/// A program with one PID on a 100 ms task and three signals: a measurement, an output and a second output.
fn catalog() -> CatalogAnswer {
    CatalogAnswer {
        control: CompiledCatalog {
            symbols: Vec::new(),
            controllers: vec![CompiledController {
                instance: "P1.Controller".into(),
                type_name: "PID".into(),
                tasks: vec![CompiledTask {
                    name: "MainTask".into(),
                    interval_ms: 100.0,
                }],
            }],
            links: Vec::new(),
            writes: Vec::new(),
        },
        signals: vec![
            signal("P1.Measure", "%IL0"),
            signal("P1.Output", "%QL0"),
            signal("P1.Other", "%QL1"),
        ],
    }
}

fn start() -> SetupStepResult {
    respond(
        StepSession::default(),
        &catalog(),
        StepAction::Start,
        "simulator",
    )
}

fn session_of_reply(reply: &SetupStepResult) -> StepSession {
    session_of(Some(reply.session.clone())).expect("the session the engine wrote")
}

fn answer(reply: &SetupStepResult, value: Value) -> SetupStepResult {
    respond(
        session_of_reply(reply),
        &catalog(),
        StepAction::Answer(value),
        "simulator",
    )
}

fn back(reply: &SetupStepResult) -> SetupStepResult {
    respond(
        session_of_reply(reply),
        &catalog(),
        StepAction::Back(None),
        "simulator",
    )
}

/// The reply after each answer in turn, from the start of the setup.
fn answered(values: &[Value]) -> SetupStepResult {
    values
        .iter()
        .fold(start(), |reply, value| answer(&reply, value.clone()))
}

fn key(reply: &SetupStepResult) -> Option<&str> {
    reply
        .question
        .as_ref()
        .map(|question| question.key.as_str())
}

#[test]
fn a_start_asks_the_loop_name_first_and_offers_no_back() {
    let reply = start();
    let question = reply.question.as_ref().expect("the first question");
    assert_eq!(question.key, "name");
    assert_eq!(question.phase, 1);
    assert_eq!(question.phase_name, "Loop and signals");
    assert_eq!(question.kind, QuestionKind::Text);
    assert_eq!(question.items, [] as [ChoiceItem; 0]);
    assert!(!reply.can_go_back);
    assert_eq!(reply.refusal, None);
    assert_eq!(reply.draft, None);
}

#[test]
fn a_refused_answer_is_asked_again_with_the_words_of_its_rule() {
    let reply = answer(&start(), json!("   "));
    assert_eq!(reply.refusal.as_deref(), Some(words::enter_value()));
    assert_eq!(key(&reply), Some("name"));
    assert_eq!(reply.session["answers"], json!([]));
}

#[test]
fn a_choice_takes_only_the_values_it_offers_and_null_is_the_custom_controller() {
    let reply = answered(&[json!("Tank")]);
    assert_eq!(key(&reply), Some("controllerInstance"));
    let items = &reply.question.as_ref().expect("the question").items;
    assert_eq!(items[0].value, json!("P1.Controller"));
    assert_eq!(items[1].value, Value::Null);

    let refused = answer(&reply, json!("P9.Unknown"));
    assert_eq!(refused.refusal.as_deref(), Some(words::not_among_offered()));
    assert_eq!(key(&refused), Some("controllerInstance"));

    let custom = answer(&reply, Value::Null);
    assert_eq!(key(&custom), Some("customInstance"));
}

#[test]
fn a_number_is_read_by_the_rule_of_its_question() {
    let reply = answered(&[
        json!("Tank"),
        json!("P1.Controller"),
        json!("%IL0"),
        json!("%QL0"),
        Value::Null,
        json!("observation"),
        json!("degC"),
    ]);
    assert_eq!(key(&reply), Some("processMinimum"));
    assert_eq!(
        answer(&reply, json!("abc")).refusal.as_deref(),
        Some(words::invalid_number())
    );

    let minimum = answer(&reply, json!("0"));
    assert_eq!(key(&minimum), Some("processMaximum"));
    assert_eq!(
        answer(&minimum, json!("0")).refusal.as_deref(),
        Some(words::invalid_number())
    );
    assert_eq!(key(&answer(&minimum, json!("100"))), Some("outputUnit"));
}

#[test]
fn back_returns_to_the_nearest_answer_the_engineer_may_change_with_that_answer() {
    let reply = answered(&[json!("Tank"), json!("P1.Controller"), json!("%IL0")]);
    assert_eq!(key(&reply), Some("controllerOutput"));
    let previous = back(&reply);
    assert_eq!(key(&previous), Some("processVariable"));
    assert_eq!(previous.previous, Some(json!("%IL0")));
    assert!(previous.can_go_back);
}

#[test]
fn an_answer_before_the_back_target_is_replayed_without_asking() {
    let reply = answered(&[
        json!("Tank"),
        json!("P1.Controller"),
        json!("%IL0"),
        json!("%QL0"),
    ]);
    assert_eq!(key(&reply), Some("setpoint"));
    // Back passes the loop name, the controller and the measurement, which are replayed; the output is asked again with its answer.
    let previous = back(&reply);
    assert_eq!(key(&previous), Some("controllerOutput"));
    assert_eq!(previous.previous, Some(json!("%QL0")));
}

#[test]
fn a_changed_answer_discards_the_answers_after_it() {
    let reply = answered(&[
        json!("Tank"),
        json!("P1.Controller"),
        json!("%IL0"),
        json!("%QL0"),
    ]);
    let asked_again = back(&reply);
    let changed = answer(&asked_again, json!("%QL1"));
    assert_eq!(changed.refusal, None);
    assert_eq!(key(&changed), Some("setpoint"));
    assert_eq!(
        changed.previous, None,
        "the setpoint was discarded with the output's old answer"
    );
    assert_eq!(changed.session["answers"].as_array().map(Vec::len), Some(4));
}

#[test]
fn the_answers_of_an_observation_finish_with_a_draft_and_the_sentence_that_confirms_it() {
    let reply = answered(&[
        json!("Tank"),
        json!("P1.Controller"),
        json!("%IL0"),
        json!("%QL0"),
        Value::Null,
        json!("observation"),
        json!("degC"),
        json!("0"),
        json!("100"),
        json!("%"),
        json!("0"),
        json!("100"),
        json!("60"),
        json!("single"),
        json!("continuous"),
        json!(""),
        json!(""),
        json!("output"),
    ]);
    assert!(reply.question.is_none(), "every answer is in");
    assert_eq!(
        reply
            .draft
            .as_ref()
            .and_then(|draft| draft["structure"].as_str()),
        Some("step-by-step")
    );
    assert_eq!(
        reply.summary.as_deref(),
        Some("Save Tank for simulator? PV: P1.Measure (degC); output: P1.Output (%); 100 ms. This loop observes the existing program.")
    );
}

#[test]
fn a_catalog_without_signals_refuses_the_start_with_its_words() {
    let mut empty = catalog();
    empty.signals.clear();
    let reply = respond(
        StepSession::default(),
        &empty,
        StepAction::Start,
        "simulator",
    );
    assert_eq!(reply.refusal.as_deref(), Some(words::no_signals()));
    assert!(reply.question.is_none());
}

#[test]
fn a_session_the_engine_did_not_write_is_refused_and_no_session_is_the_start() {
    assert_eq!(session_of(None), Ok(StepSession::default()));
    assert!(session_of(Some(json!("not a session"))).is_err());
}
