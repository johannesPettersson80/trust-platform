use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::ops::BinaryOp;
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

fn inc_program(name: &str, var: &str) -> ProgramDef {
    ProgramDef {
        name: name.into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name(var.into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name(var.into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
    }
}

#[test]
fn periodic_interval() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("count", Value::Int(0));
    runtime.register_program(inc_program("P", "count")).unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::from_millis(10),
        single: None,
        priority: 1,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(0))
    );

    runtime.advance_time(Duration::from_millis(5));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(0))
    );

    runtime.advance_time(Duration::from_millis(5));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(1))
    );
}

#[test]
fn interval_zero_disables_periodic() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("count", Value::Int(0));
    runtime.register_program(inc_program("P", "count")).unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: None,
        priority: 1,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    runtime.advance_time(Duration::from_millis(20));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(0))
    );
}

#[test]
fn event_single_rise() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.storage_mut().set_global("count", Value::Int(0));
    runtime.register_program(inc_program("P", "count")).unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 1,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(0))
    );

    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(1))
    );

    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(1))
    );

    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.execute_cycle().unwrap();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(2))
    );
}

#[test]
fn single_blocks_periodic() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.storage_mut().set_global("count", Value::Int(0));
    runtime.register_program(inc_program("P", "count")).unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::from_millis(10),
        single: Some("trigger".into()),
        priority: 1,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    runtime.advance_time(Duration::from_millis(10));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(0))
    );

    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.advance_time(Duration::from_millis(10));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(1))
    );
}

#[test]
fn event_edge_coalescing_between_samples() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.storage_mut().set_global("count", Value::Int(0));
    runtime.register_program(inc_program("P", "count")).unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 1,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.execute_cycle().unwrap();

    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(1))
    );
}

#[test]
fn priority_order() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.storage_mut().set_global("x", Value::Int(0));
    runtime.storage_mut().set_global("y", Value::Int(0));

    let prog_a = ProgramDef {
        name: "A".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("x".into()),
            value: Expr::Literal(Value::Int(1)),
            location: None,
        }],
    };
    let prog_b = ProgramDef {
        name: "B".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("y".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("x".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
    };

    runtime.register_program(prog_a).unwrap();
    runtime.register_program(prog_b).unwrap();

    runtime.register_task(TaskConfig {
        name: "TaskA".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["A".into()],
        fb_instances: Vec::new(),
    });
    runtime.register_task(TaskConfig {
        name: "TaskB".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 10,
        programs: vec!["B".into()],
        fb_instances: Vec::new(),
    });

    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage_mut().get_global("y"), Some(&Value::Int(2)));
}

#[test]
fn background_programs() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.storage_mut().set_global("count", Value::Int(0));
    runtime.storage_mut().set_global("bg", Value::Int(0));

    runtime
        .register_program(inc_program("TaskProg", "count"))
        .unwrap();
    runtime
        .register_program(inc_program("BgProg", "bg"))
        .unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 1,
        programs: vec!["TaskProg".into()],
        fb_instances: Vec::new(),
    });

    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage_mut().get_global("bg"), Some(&Value::Int(1)));
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(0))
    );

    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage_mut().get_global("bg"), Some(&Value::Int(2)));
    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(1))
    );
}

#[test]
fn fifo_order_by_due_time_within_priority() {
    let mut runtime = Runtime::new();
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(false));
    runtime.storage_mut().set_global("x", Value::Int(0));
    runtime.storage_mut().set_global("y", Value::Int(0));

    let prog_a = ProgramDef {
        name: "A".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("x".into()),
            value: Expr::Literal(Value::Int(1)),
            location: None,
        }],
    };
    let prog_b = ProgramDef {
        name: "B".into(),
        vars: Vec::new(),
        temps: Vec::new(),
        using: Vec::new(),
        body: vec![Stmt::Assign {
            target: LValue::Name("y".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("x".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
    };
    runtime.register_program(prog_a).unwrap();
    runtime.register_program(prog_b).unwrap();

    runtime.register_task(TaskConfig {
        name: "Periodic".into(),
        interval: Duration::from_millis(10),
        single: None,
        priority: 0,
        programs: vec!["A".into()],
        fb_instances: Vec::new(),
    });
    runtime.register_task(TaskConfig {
        name: "Event".into(),
        interval: Duration::ZERO,
        single: Some("trigger".into()),
        priority: 0,
        programs: vec!["B".into()],
        fb_instances: Vec::new(),
    });

    runtime.advance_time(Duration::from_millis(20));
    runtime
        .storage_mut()
        .set_global("trigger", Value::Bool(true));
    runtime.execute_cycle().unwrap();

    assert_eq!(runtime.storage_mut().get_global("y"), Some(&Value::Int(2)));
}

#[test]
fn task_overrun_drops_missed_intervals() {
    let mut runtime = Runtime::new();
    runtime.storage_mut().set_global("count", Value::Int(0));
    runtime.register_program(inc_program("P", "count")).unwrap();

    runtime.register_task(TaskConfig {
        name: "T".into(),
        interval: Duration::from_millis(10),
        single: None,
        priority: 0,
        programs: vec!["P".into()],
        fb_instances: Vec::new(),
    });

    runtime.advance_time(Duration::from_millis(35));
    runtime.execute_cycle().unwrap();

    assert_eq!(
        runtime.storage_mut().get_global("count"),
        Some(&Value::Int(1))
    );
    assert_eq!(runtime.task_overrun_count("T"), Some(2));
}

#[test]
fn simulation_time_advance_saturates_at_duration_limit() {
    let mut runtime = Runtime::new();
    runtime.set_current_time(Duration::from_nanos(i64::MAX - 5));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        runtime.advance_time(Duration::from_nanos(10));
    }));

    assert!(result.is_ok(), "simulation-time overflow must not panic");
    assert_eq!(runtime.current_time(), Duration::from_nanos(i64::MAX));
}
