use trust_runtime::debug::RuntimeEvent;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::ops::BinaryOp;
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::{Duration, Value};
use trust_runtime::RuntimeBuilder;

#[test]
fn runtime_events_include_cycle_and_task() {
    let mut runtime = RuntimeBuilder::new();
    for (name, type_id, value) in [
        ("trigger", trust_hir::TypeId::BOOL, Value::Bool(false)),
        ("count", trust_hir::TypeId::INT, Value::Int(0)),
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
            target: LValue::Name("count".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("count".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
    };
    runtime.register_program(program);
    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });
    let mut runtime = runtime.build().unwrap();

    let control = runtime.enable_debug();
    runtime.write_global("trigger", Value::Bool(true)).unwrap();
    runtime.execute_cycle().unwrap();

    let events = control.drain_runtime_events();
    let mut iter = events.iter();
    assert!(matches!(iter.next(), Some(RuntimeEvent::CycleStart { .. })));
    assert!(matches!(iter.next(), Some(RuntimeEvent::TaskStart { .. })));
    assert!(matches!(iter.next(), Some(RuntimeEvent::TaskEnd { .. })));
    assert!(matches!(iter.next(), Some(RuntimeEvent::CycleEnd { .. })));
}

#[test]
fn runtime_event_overrun_emitted() {
    let mut runtime = RuntimeBuilder::new();
    {
        let (name, type_id, value) = ("count", trust_hir::TypeId::INT, Value::Int(0));
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
    runtime.register_program(ProgramDef {
        methods: Vec::new(),
        name: "P".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("count".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("count".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
    });

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::from_millis(10),
        single: None,
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });
    let mut runtime = runtime.build().unwrap();

    let control = runtime.enable_debug();
    runtime.advance_time(Duration::from_millis(35));
    runtime.execute_cycle().unwrap();

    let events = control.drain_runtime_events();
    assert!(events
        .iter()
        .any(|event| matches!(event, RuntimeEvent::TaskOverrun { missed: 2, .. })));
}

#[test]
fn runtime_event_fault_emitted() {
    let mut runtime = RuntimeBuilder::new();
    for (name, type_id, value) in [
        ("trigger", trust_hir::TypeId::BOOL, Value::Bool(false)),
        ("x", trust_hir::TypeId::INT, Value::Int(0)),
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
    runtime.register_program(ProgramDef {
        methods: Vec::new(),
        name: "P".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("x".into()),
            value: Expr::Binary {
                op: BinaryOp::Div,
                left: Box::new(Expr::Literal(Value::Int(1))),
                right: Box::new(Expr::Literal(Value::Int(0))),
            },
            location: None,
        }],
    });

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });
    let mut runtime = runtime.build().unwrap();

    let control = runtime.enable_debug();
    runtime.write_global("trigger", Value::Bool(true)).unwrap();
    assert!(runtime.execute_cycle().is_err());

    let events = control.drain_runtime_events();
    assert!(events
        .iter()
        .any(|event| matches!(event, RuntimeEvent::Fault { .. })));
    assert!(runtime.faulted());
}
