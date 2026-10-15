//! setVariable writes and forces refused by a full request queue change nothing (spec 11,
//! "request queue full").
use super::*;
use crate::protocol::{MessageType, Response};
use crate::DebugSession;
use trust_runtime::harness::CompileSession;

const COUNTER_SOURCE: &str = r#"
VAR_GLOBAL
    Counter : INT := 0;
END_VAR
PROGRAM Main
END_PROGRAM
"#;

#[test]
fn set_variable_force_refused_by_a_full_request_queue_changes_nothing() {
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
        "a full queue refuses the setVariable force"
    );
    assert_eq!(response.message.as_deref(), Some("request queue full"));
    let runtime_handle = adapter.session().runtime_handle();
    let runtime = runtime_handle.lock().expect("runtime lock");
    assert_eq!(
        runtime.storage().get_global("Counter"),
        Some(&RuntimeValue::Int(0)),
        "the refused setVariable must not write Counter"
    );
    assert!(
        debug
            .forced_var_value(&ForcedVarTarget::Global("Counter".into()))
            .is_none(),
        "the refused force must not enter the force set"
    );
}
