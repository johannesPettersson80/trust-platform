use std::sync::mpsc;
use std::time::Duration as StdDuration;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::io::{IoAddress, IoSnapshotValue};
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

#[test]
fn io_read_write_order() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("in", Value::Bool(false));
    runtime.storage_mut().set_global("out", Value::Bool(false));
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));

    let program = ProgramDef {
        name: "P".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("out".into()),
            value: Expr::Name("in".into()),
            location: None,
        }],
    };
    runtime.register_program(program).unwrap();

    let input_addr = IoAddress::parse("%IX0.0").unwrap();
    let output_addr = IoAddress::parse("%QX0.1").unwrap();
    runtime.io_mut().bind("in", input_addr.clone());
    runtime.io_mut().bind("out", output_addr.clone());

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    runtime
        .io_mut()
        .write(&input_addr, Value::Bool(true))
        .unwrap();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));

    runtime.execute_cycle().unwrap();

    let out = runtime.io().read(&output_addr).unwrap();
    assert_eq!(out, Value::Bool(true));
}

#[test]
fn io_snapshot_emitted_after_cycle_output_commit() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("in", Value::Bool(false));
    runtime.storage_mut().set_global("out", Value::Bool(false));
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));

    let program = ProgramDef {
        name: "P".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("out".into()),
            value: Expr::Name("in".into()),
            location: None,
        }],
    };
    runtime.register_program(program).unwrap();

    let input_addr = IoAddress::parse("%IX0.0").unwrap();
    let output_addr = IoAddress::parse("%QX0.1").unwrap();
    runtime.io_mut().bind("in", input_addr.clone());
    runtime.io_mut().bind("out", output_addr.clone());

    runtime
        .io_mut()
        .write(&input_addr, Value::Bool(true))
        .unwrap();
    runtime
        .io_mut()
        .write(&output_addr, Value::Bool(false))
        .unwrap();
    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));

    let control = runtime.enable_debug();
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
