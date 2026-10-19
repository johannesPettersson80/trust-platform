use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::ops::BinaryOp;
use trust_runtime::eval::stmt::Stmt;
use trust_runtime::task::{ProgramDef, TaskConfig};
use trust_runtime::value::{Duration, Value};
use trust_runtime::RuntimeBuilder;

#[test]
fn program_cycle() {
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
        name: "TestProg".into(),
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
        programs: vec!["TestProg".into()],
        fb_instances: Vec::new(),
    });

    let mut runtime = runtime.build().unwrap();
    runtime.write_global("trigger", Value::Bool(true)).unwrap();
    runtime.execute_cycle().unwrap();
    assert_eq!(runtime.storage().get_global("count"), Some(&Value::Int(1)));
}
