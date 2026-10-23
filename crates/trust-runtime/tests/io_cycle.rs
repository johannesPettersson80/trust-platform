use std::sync::mpsc;
use std::time::Duration as StdDuration;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::io::{IoAddress, IoSnapshotValue};
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::{Duration, Value};
use trust_runtime::RuntimeBuilder;

#[test]
fn io_read_write_order() {
    let mut runtime = RuntimeBuilder::new();
    for (name, type_id, value) in [
        ("in", trust_hir::TypeId::BOOL, Value::Bool(false)),
        ("out", trust_hir::TypeId::BOOL, Value::Bool(false)),
        ("trigger", trust_hir::TypeId::BOOL, Value::Bool(false)),
    ] {
        runtime
            .register_global(trust_runtime::program_model::VarDef {
                name: name.into(),
                type_id,
                initializer: Some(Expr::Literal(value)),
                retain: trust_runtime::RetainPolicy::Unspecified,
                static_storage: false,
                external: false,
                in_out: false,
                constant: false,
                address: None,
            })
            .unwrap();
    }

    let program = ProgramDef {
        methods: Vec::new(),
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
    runtime.register_program(program);

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
    let mut runtime = runtime.build().unwrap();

    runtime.write_io(&input_addr, Value::Bool(true)).unwrap();
    runtime.write_global("trigger", Value::Bool(true)).unwrap();

    runtime.execute_cycle().unwrap();

    let out = runtime.io().read(&output_addr).unwrap();
    assert_eq!(out, Value::Bool(true));
}

#[test]
fn io_snapshot_emitted_after_cycle_output_commit() {
    let mut runtime = RuntimeBuilder::new();
    for (name, type_id, value) in [
        ("in", trust_hir::TypeId::BOOL, Value::Bool(false)),
        ("out", trust_hir::TypeId::BOOL, Value::Bool(false)),
        ("trigger", trust_hir::TypeId::BOOL, Value::Bool(false)),
    ] {
        runtime
            .register_global(trust_runtime::program_model::VarDef {
                name: name.into(),
                type_id,
                initializer: Some(Expr::Literal(value)),
                retain: trust_runtime::RetainPolicy::Unspecified,
                static_storage: false,
                external: false,
                in_out: false,
                constant: false,
                address: None,
            })
            .unwrap();
    }

    let program = ProgramDef {
        methods: Vec::new(),
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
    runtime.register_program(program);

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
    let mut runtime = runtime.build().unwrap();

    runtime.write_global("trigger", Value::Bool(true)).unwrap();

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
