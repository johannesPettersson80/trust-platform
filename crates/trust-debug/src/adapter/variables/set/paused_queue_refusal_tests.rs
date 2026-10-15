//! Paused setVariable forces refused by a full request queue change nothing (spec 11,
//! "request queue full").
use super::*;
use crate::protocol::{MessageType, Response};
use crate::DebugSession;
use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::harness::CompileSession;
use trust_runtime::value::Duration;

const COUNTER_SOURCE: &str = r#"
VAR_GLOBAL
    Counter : INT := 0;
END_VAR
PROGRAM Main
END_PROGRAM
"#;

#[test]
fn paused_set_variable_force_refused_by_a_full_request_queue_changes_nothing() {
    let runtime = CompileSession::from_source(COUNTER_SOURCE)
        .build_runtime()
        .expect("fixture compiles");
    let control = DebugControl::new();
    control.refresh_snapshot_from_storage(runtime.storage(), Duration::ZERO);
    let mut adapter = DebugAdapter::new(DebugSession::with_control(runtime, control.clone()));
    for index in 0..1_024 {
        control
            .try_force_global(format!("filler{index}"), RuntimeValue::Int(0))
            .expect("a filler fits the bound");
    }
    let globals = adapter.alloc_variable_handle(VariableHandle::Globals);

    let outcome = adapter.dispatch_request(Request {
        seq: 1,
        message_type: MessageType::Request,
        command: "setVariable".into(),
        arguments: Some(serde_json::json!({
            "variablesReference": globals,
            "name": "Counter",
            "value": "force: 41",
        })),
    });

    let response: Response<Value> =
        serde_json::from_value(outcome.responses[0].clone()).expect("a response");
    assert!(
        !response.success,
        "a full queue refuses the paused setVariable force"
    );
    assert_eq!(response.message.as_deref(), Some("request queue full"));
    let snapshot = control.snapshot().expect("the debugger stays paused");
    assert_eq!(
        snapshot.storage.get_global("Counter"),
        Some(&RuntimeValue::Int(0)),
        "the refused setVariable must not change the detached destination"
    );
    assert!(
        control
            .forced_var_value(&ForcedVarTarget::Global("Counter".into()))
            .is_none(),
        "the refused force must not enter the force set"
    );
}
