//! The process image around a scan: the inputs are sampled before the program runs, the outputs
//! are committed after it, and the scan's I/O snapshot is published after the output commit.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::mpsc;
use std::time::Duration as StdDuration;

use trust_runtime::debug::DebugControl;
use trust_runtime::harness::TestHarness;
use trust_runtime::io::{IoAddress, IoSnapshotValue};
use trust_runtime::value::Value;

/// `out` copies `in` in an event task that a rising edge of `trigger` starts.
const COPY: &str = r#"
CONFIGURATION C
VAR_GLOBAL
    in AT %IX0.0 : BOOL;
    out AT %QX0.1 : BOOL;
    trigger : BOOL;
END_VAR
RESOURCE R ON PLC
    TASK T (SINGLE := trigger, PRIORITY := 0);
    PROGRAM P1 WITH T : P;
END_RESOURCE
END_CONFIGURATION

PROGRAM P
out := in;
END_PROGRAM
"#;

#[test]
fn io_read_write_order() {
    let mut harness = TestHarness::from_source(COPY).expect("the copy program compiles");
    let input_addr = IoAddress::parse("%IX0.0").unwrap();
    let output_addr = IoAddress::parse("%QX0.1").unwrap();
    let runtime = harness.runtime_mut();

    runtime.write_io(&input_addr, Value::Bool(true)).unwrap();
    runtime
        .write_variable("trigger", &Value::Bool(true))
        .unwrap();

    runtime.execute_cycle().unwrap();

    assert_eq!(runtime.read_io(&output_addr), Ok(Value::Bool(true)));
}

#[test]
fn io_snapshot_emitted_after_cycle_output_commit() {
    let mut harness = TestHarness::from_source(COPY).expect("the copy program compiles");
    let input_addr = IoAddress::parse("%IX0.0").unwrap();
    let output_addr = IoAddress::parse("%QX0.1").unwrap();
    let runtime = harness.runtime_mut();

    runtime.write_io(&input_addr, Value::Bool(true)).unwrap();
    runtime.write_io(&output_addr, Value::Bool(false)).unwrap();
    runtime
        .write_variable("trigger", &Value::Bool(true))
        .unwrap();

    let control = DebugControl::new();
    runtime.set_debug_control(control.clone());
    let (tx, rx) = mpsc::channel();
    control.set_io_sender(tx);

    runtime.execute_cycle().unwrap();

    let snapshot = rx
        .recv_timeout(StdDuration::from_secs(1))
        .expect("post-cycle I/O snapshot");
    assert!(
        rx.try_recv().is_err(),
        "only one post-cycle snapshot is expected"
    );

    let snapshot_in = snapshot
        .inputs
        .iter()
        .find(|entry| entry.name.as_deref() == Some("in"))
        .expect("input entry");
    match &snapshot_in.value {
        IoSnapshotValue::Value(Value::Bool(true)) => {}
        other => panic!("unexpected input snapshot: {other:?}"),
    }

    let snapshot_out = snapshot
        .outputs
        .iter()
        .find(|entry| entry.name.as_deref() == Some("out"))
        .expect("output entry");
    match &snapshot_out.value {
        IoSnapshotValue::Value(Value::Bool(true)) => {}
        other => panic!("unexpected output snapshot after output commit: {other:?}"),
    }
}
