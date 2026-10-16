//! setExpression forces refused by a full request queue change nothing, and a failed write puts
//! back the force its target had before (spec 11, "request queue full").
use super::*;
use crate::protocol::{MessageType, Response};
use crate::DebugSession;
use trust_runtime::debug::ForcedVarTarget;
use trust_runtime::harness::CompileSession;

const COUNTER_SOURCE: &str = r#"
VAR_GLOBAL
    Counter : INT := 0;
END_VAR
PROGRAM Main
    Counter := Counter + 1;
END_PROGRAM
"#;

#[test]
fn set_expression_force_refused_by_a_full_request_queue_changes_nothing() {
    let runtime = CompileSession::from_source(COUNTER_SOURCE)
        .build_runtime()
        .expect("fixture compiles");
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    let debug = adapter.session().debug_control();
    for index in 0..1_024 {
        debug
            .try_force_global(format!("filler{index}"), RuntimeValue::Int(0))
            .expect("a filler fits the bound");
    }

    let outcome = adapter.dispatch_request(Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setExpression".into(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "Counter".into(),
                value: "force: 41".into(),
                frame_id: None,
            })
            .expect("arguments serialize"),
        ),
    });

    let response: Response<Value> =
        serde_json::from_value(outcome.responses[0].clone()).expect("a response");
    assert!(!response.success, "a full queue refuses the force");
    assert_eq!(response.message.as_deref(), Some("request queue full"));
    let runtime_handle = adapter.session().runtime_handle();
    let mut runtime = runtime_handle.lock().expect("runtime lock");
    runtime.execute_cycle().expect("the scan runs");
    assert_eq!(
        runtime.storage().get_global("Counter"),
        Some(&RuntimeValue::Int(1)),
        "the refused force must not hold Counter, so the scan only increments it"
    );
}

const CONSTANT_SOURCE: &str = r#"
VAR_GLOBAL CONSTANT
    Limit : INT := 5;
END_VAR
PROGRAM Main
END_PROGRAM
"#;

#[test]
fn failed_write_restores_the_force_the_target_had_before() {
    let runtime = CompileSession::from_source(CONSTANT_SOURCE)
        .build_runtime()
        .expect("fixture compiles");
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    let debug = adapter.session().debug_control();
    debug
        .try_force_global("Limit", RuntimeValue::Int(3))
        .expect("the existing force is accepted");

    let outcome = adapter.dispatch_request(Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setExpression".into(),
        arguments: Some(
            serde_json::to_value(SetExpressionArguments {
                expression: "Limit".into(),
                value: "force: 7".into(),
                frame_id: None,
            })
            .expect("arguments serialize"),
        ),
    });

    let response: Response<Value> =
        serde_json::from_value(outcome.responses[0].clone()).expect("a response");
    assert!(!response.success, "a constant refuses the write");
    assert_eq!(
        debug.forced_var_value(&ForcedVarTarget::Global("Limit".into())),
        Some(RuntimeValue::Int(3)),
        "the failed write puts back the force the target had before"
    );
}
