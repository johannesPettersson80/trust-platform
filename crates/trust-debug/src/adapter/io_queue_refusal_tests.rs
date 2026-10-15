//! DAP I/O writes and forces refused by a full request queue change nothing (spec 11,
//! "request queue full"; spec 34 section 4.3.1).
use super::*;
use crate::protocol::{MessageType, Response};
use crate::DebugSession;
use trust_runtime::harness::CompileSession;

const IO_SOURCE: &str = r#"
PROGRAM Main
VAR
    Pulse : BOOL;
END_VAR
END_PROGRAM

CONFIGURATION DapIo
PROGRAM P1 : Main;
VAR_CONFIG
    P1.Pulse AT %IX0.0 : BOOL;
END_VAR
END_CONFIGURATION
"#;

fn io_request(adapter: &mut DebugAdapter, command: &str, address: &str) -> Response<Value> {
    let outcome = adapter.dispatch_request(Request {
        seq: 1,
        message_type: MessageType::Request,
        command: command.into(),
        arguments: Some(
            serde_json::to_value(IoWriteArguments {
                address: address.into(),
                value: "TRUE".into(),
            })
            .expect("arguments serialize"),
        ),
    });
    serde_json::from_value(outcome.responses[0].clone()).expect("a response")
}

#[test]
fn io_write_refused_by_a_full_request_queue_changes_nothing() {
    let runtime = CompileSession::from_source(IO_SOURCE)
        .build_runtime()
        .expect("fixture compiles");
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    let debug = adapter.session().debug_control();
    for _ in 0..1_024 {
        debug
            .try_enqueue_io_write(
                IoAddress::parse("%QX0.0").expect("output address parses"),
                RuntimeValue::Bool(true),
            )
            .expect("a filler fits the bound");
    }

    let response = io_request(&mut adapter, "stIoWrite", "%IX0.0");

    assert!(!response.success, "a full queue refuses the DAP write");
    assert_eq!(response.message.as_deref(), Some("request queue full"));
    assert_eq!(
        debug.drain_io_writes().len(),
        1_024,
        "only the fillers are queued"
    );
    let input = IoAddress::parse("%IX0.0").expect("input address parses");
    let runtime_handle = adapter.session().runtime_handle();
    let runtime = runtime_handle.lock().expect("runtime lock");
    assert_eq!(
        runtime.io().read(&input).expect("input image reads"),
        RuntimeValue::Bool(false),
        "the refused write must not change the input image"
    );
}

#[test]
fn io_force_refused_by_a_full_request_queue_changes_nothing() {
    let runtime = CompileSession::from_source(IO_SOURCE)
        .build_runtime()
        .expect("fixture compiles");
    let mut adapter = DebugAdapter::new(DebugSession::new(runtime));
    let debug = adapter.session().debug_control();
    for word in 0..1_024 {
        debug
            .try_force_io(
                IoAddress::parse(&format!("%MW{word}")).expect("word address parses"),
                RuntimeValue::LInt(1),
            )
            .expect("a filler fits the bound");
    }

    let response = io_request(&mut adapter, "stIoForce", "%IX0.0");

    assert!(!response.success, "a full queue refuses the DAP force");
    assert_eq!(response.message.as_deref(), Some("request queue full"));
    let input = IoAddress::parse("%IX0.0").expect("input address parses");
    assert!(
        debug.forced_io_value(&input).is_none(),
        "a refused force is not installed"
    );
    assert!(adapter.forced_io_addresses.lock().unwrap().is_empty());
    let runtime_handle = adapter.session().runtime_handle();
    let runtime = runtime_handle.lock().expect("runtime lock");
    assert_eq!(
        runtime.io().read(&input).expect("input image reads"),
        RuntimeValue::Bool(false),
        "the refused force must not change the input image"
    );
}
