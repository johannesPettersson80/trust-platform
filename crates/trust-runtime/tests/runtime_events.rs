use trust_runtime::debug::RuntimeEvent;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::ops::BinaryOp;
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

#[test]
fn runtime_events_include_cycle_and_task() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.storage_mut().set_global("count", Value::Int(0));

    let program = ProgramDef {
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
    runtime.register_program(program).unwrap();
    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    let control = runtime.enable_debug();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
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
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("count", Value::Int(0));
    runtime
        .register_program(ProgramDef {
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
        })
        .unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::from_millis(10),
        single: None,
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

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
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.storage_mut().set_global("x", Value::Int(0));
    runtime
        .register_program(ProgramDef {
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
        })
        .unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    let control = runtime.enable_debug();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    assert!(runtime.execute_cycle().is_err());

    let events = control.drain_runtime_events();
    assert!(events
        .iter()
        .any(|event| matches!(event, RuntimeEvent::Fault { .. })));
    assert!(runtime.faulted());
}
