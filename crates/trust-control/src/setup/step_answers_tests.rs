//! The step-by-step answers as the page posts them (`step_answers.rs`): each answer resolves in the compiled catalog, a refusal names its
//! question, and the loop the answers build is planned as the save writes it (specification 35, B1.3).

use serde_json::{json, Value};

use super::*;
use crate::setup::catalog::{CompiledCatalog, CompiledController, CompiledSignal, CompiledTask};
use crate::setup::prompts::{
    ChannelMode, ControllerChoice, FeedbackChoice, RateAnswer, SetpointChoice,
};

fn scope() -> LoopScope {
    LoopScope {
        project_id: "project:test".into(),
        runtime_id: "runtime:pump".into(),
    }
}

/// The pump's program: a custom-typed regulator on a 20 ms task, its power output and its pressure feedback (the custom open loop of
/// `configure_tests.rs`).
fn pump_catalog() -> CatalogAnswer {
    CatalogAnswer {
        control: CompiledCatalog {
            symbols: Vec::new(),
            controllers: vec![CompiledController {
                instance: "PumpStation.Regulator".into(),
                type_name: "MyPumpLaw".into(),
                tasks: vec![CompiledTask {
                    name: "PumpTask".into(),
                    interval_ms: 20.0,
                }],
            }],
            links: Vec::new(),
            writes: Vec::new(),
        },
        signals: vec![
            CompiledSignal {
                symbol: "PumpStation.Power".into(),
                address: "%QL0".into(),
                iec_type: "LREAL".into(),
            },
            CompiledSignal {
                symbol: "PumpStation.Pressure".into(),
                address: "%IL0".into(),
                iec_type: "LREAL".into(),
            },
        ],
    }
}

/// The answers of an observation of the pump, as the page posts them.
fn observation() -> Value {
    json!({
        "structure": "step-by-step",
        "name": "Pump pressure",
        "controllerInstance": "PumpStation.Regulator",
        "processVariable": "%IL0",
        "controllerOutput": "%QL0",
        "setpoint": null,
        "workflow": "observation",
        "processUnit": "bar",
        "processMinimum": 0.0,
        "processMaximum": 12.0,
        "outputUnit": "%",
        "outputMinimum": 0.0,
        "outputMaximum": 100.0,
        "observationDurationSeconds": 60.0,
        "channelMode": "single",
        "actuator": "continuous",
        "risingRate": "",
        "fallingRate": "2.5",
        "feedback": "output"
    })
}

fn refused(draft: &Value) -> Failure {
    decode_answers(draft, &pump_catalog()).expect_err("the answers are refused")
}

#[test]
fn a_step_by_step_draft_is_recognised_by_its_structure_only() {
    assert!(is_step_by_step(&observation()));
    assert!(!is_step_by_step(&json!({ "structure": "pid" })));
    assert!(!is_step_by_step(&json!({ "name": "no structure" })));
}

#[test]
fn the_answers_resolve_their_identities_in_the_compiled_catalog() {
    let answers = decode_answers(&observation(), &pump_catalog()).expect("answers");
    assert_eq!(answers.name.as_deref(), Some("Pump pressure"));
    assert!(matches!(
        answers.controller,
        Some(ControllerChoice::Compiled(ref controller)) if controller.instance == "PumpStation.Regulator"
    ));
    assert_eq!(
        answers
            .process_variable
            .as_ref()
            .map(|signal| signal.symbol.as_str()),
        Some("PumpStation.Pressure")
    );
    assert_eq!(answers.setpoint, Some(SetpointChoice::OpenLoop));
    assert_eq!(answers.channel_mode, Some(ChannelMode::Single));
    assert_eq!(answers.rising_rate, Some(RateAnswer::Unlimited));
    assert_eq!(answers.falling_rate, Some(RateAnswer::PerSecond(2.5)));
    assert_eq!(answers.feedback, Some(FeedbackChoice::UseOutput));
}

#[test]
fn a_null_controller_is_a_custom_controller_and_a_named_feedback_is_its_signal() {
    let mut draft = observation();
    draft["controllerInstance"] = Value::Null;
    draft["customInstance"] = json!("PumpStation.Typed");
    draft["customType"] = json!("MyPumpLaw");
    draft["taskPeriodMs"] = json!(50.0);
    draft["feedback"] = json!("%IL0");
    let answers = decode_answers(&draft, &pump_catalog()).expect("answers");
    assert_eq!(answers.controller, Some(ControllerChoice::Custom));
    assert_eq!(
        answers.custom_instance.as_deref(),
        Some("PumpStation.Typed")
    );
    assert_eq!(answers.task_period_ms, Some(50.0));
    assert!(
        matches!(answers.feedback, Some(FeedbackChoice::Signal(ref signal)) if signal.address == "%IL0")
    );
}

#[test]
fn a_draft_the_catalog_builds_is_planned_as_the_loop_file_its_name_gives() {
    let planned = plan(&observation(), &pump_catalog(), &scope()).expect("a planned loop");
    assert_eq!(planned.file_name, "control-loops/pump-pressure.json");
    assert_eq!(planned.loop_definition.id, "loop:pump-pressure");
    assert_eq!(planned.loop_definition.name, "Pump pressure");
    assert_eq!(planned.loop_definition.project_id, "project:test");
    assert_eq!(planned.loop_definition.runtime_id, "runtime:pump");
    assert_eq!(
        planned.loop_definition.controller.instance,
        "PumpStation.Regulator"
    );
}

#[test]
fn an_identity_the_catalog_does_not_hold_is_refused_under_its_question() {
    let mut draft = observation();
    draft["controllerInstance"] = json!("PumpStation.Missing");
    let failure = refused(&draft);
    assert_eq!(failure.field.as_deref(), Some("controllerInstance"));
    assert!(
        failure.message.contains("PumpStation.Missing"),
        "{}",
        failure.message
    );

    let mut draft = observation();
    draft["processVariable"] = json!("%IW9");
    let failure = refused(&draft);
    assert_eq!(failure.field.as_deref(), Some("processVariable"));
    assert!(failure.message.contains("%IW9"), "{}", failure.message);

    let mut draft = observation();
    draft["gain"] = json!("Main.Kp");
    let failure = refused(&draft);
    assert_eq!(failure.field.as_deref(), Some("gain"));
}

#[test]
fn a_question_the_setup_does_not_ask_is_refused_by_name() {
    let mut draft = observation();
    draft["bogus"] = json!(1);
    let failure = refused(&draft);
    assert_eq!(failure.field.as_deref(), Some("bogus"));
    assert_eq!(
        failure.message,
        "The step-by-step setup asks no question named bogus."
    );
}

#[test]
fn an_answer_of_the_wrong_kind_is_refused_under_its_question() {
    let mut draft = observation();
    draft["processMinimum"] = json!("zero");
    let failure = refused(&draft);
    assert_eq!(failure.field.as_deref(), Some("processMinimum"));
    assert_eq!(
        failure.message,
        "The answer to processMinimum must be a number."
    );

    let mut draft = observation();
    draft["workflow"] = json!("tuning");
    let failure = refused(&draft);
    assert_eq!(failure.field.as_deref(), Some("workflow"));
    assert!(failure.message.contains("tuning"), "{}", failure.message);
}

#[test]
fn a_negative_rate_limit_is_refused_under_its_field_and_a_blank_one_is_no_limit() {
    let mut draft = observation();
    draft["fallingRate"] = json!("-1");
    let failure = refused(&draft);
    assert_eq!(failure.field.as_deref(), Some("fallingRate"));
}

#[test]
fn an_on_off_workflow_is_built_by_no_loop_yet_and_the_engine_says_so() {
    let mut draft = observation();
    draft["workflow"] = json!("on-off");
    let failure = plan(&draft, &pump_catalog(), &scope()).expect_err("ON/OFF is parked");
    // The sentence has two sentences: the message is the first, and the detail is the whole text.
    assert_eq!(
        failure.detail.as_deref(),
        Some(crate::setup::texts::configure::on_off_controller_not_available())
    );
}
