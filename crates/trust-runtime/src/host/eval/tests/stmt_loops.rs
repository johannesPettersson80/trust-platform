use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::ops::BinaryOp;
use trust_runtime::eval::stmt::{exec_stmt, Stmt};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn loop_control() {
    let mut storage = VariableStorage::new();
    storage.set_global("sum", Value::Int(0));
    storage.set_global("i", Value::Int(0));
    storage.set_global("w", Value::Int(0));
    storage.set_global("r", Value::Int(0));
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let for_loop = Stmt::For {
        control: "i".into(),
        start: Expr::Literal(Value::Int(0)),
        end: Expr::Literal(Value::Int(4)),
        step: Expr::Literal(Value::Int(1)),
        body: vec![
            Stmt::If {
                condition: Expr::Binary {
                    op: BinaryOp::Eq,
                    left: Box::new(Expr::Name("i".into())),
                    right: Box::new(Expr::Literal(Value::Int(1))),
                },
                then_block: vec![Stmt::Continue { location: None }],
                else_if: vec![],
                else_block: vec![],
                location: None,
            },
            Stmt::If {
                condition: Expr::Binary {
                    op: BinaryOp::Eq,
                    left: Box::new(Expr::Name("i".into())),
                    right: Box::new(Expr::Literal(Value::Int(3))),
                },
                then_block: vec![Stmt::Exit { location: None }],
                else_if: vec![],
                else_block: vec![],
                location: None,
            },
            Stmt::Assign {
                target: LValue::Name("sum".into()),
                value: Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Name("sum".into())),
                    right: Box::new(Expr::Name("i".into())),
                },
                location: None,
            },
        ],
        location: None,
    };

    exec_stmt(
        &mut ctx,
        &Stmt::Assign {
            target: LValue::Name("sum".into()),
            value: Expr::Literal(Value::Int(0)),
            location: None,
        },
    )
    .unwrap();
    exec_stmt(
        &mut ctx,
        &Stmt::Assign {
            target: LValue::Name("i".into()),
            value: Expr::Literal(Value::Int(0)),
            location: None,
        },
    )
    .unwrap();

    exec_stmt(&mut ctx, &for_loop).unwrap();
    assert_eq!(ctx.storage.get_global("sum"), Some(&Value::Int(2)));

    let while_loop = Stmt::While {
        condition: Expr::Binary {
            op: BinaryOp::Lt,
            left: Box::new(Expr::Name("w".into())),
            right: Box::new(Expr::Literal(Value::Int(2))),
        },
        body: vec![Stmt::Assign {
            target: LValue::Name("w".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("w".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
        location: None,
    };

    exec_stmt(
        &mut ctx,
        &Stmt::Assign {
            target: LValue::Name("w".into()),
            value: Expr::Literal(Value::Int(0)),
            location: None,
        },
    )
    .unwrap();
    exec_stmt(&mut ctx, &while_loop).unwrap();
    assert_eq!(ctx.storage.get_global("w"), Some(&Value::Int(2)));

    let repeat_loop = Stmt::Repeat {
        body: vec![Stmt::Assign {
            target: LValue::Name("r".into()),
            value: Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Name("r".into())),
                right: Box::new(Expr::Literal(Value::Int(1))),
            },
            location: None,
        }],
        until: Expr::Binary {
            op: BinaryOp::Ge,
            left: Box::new(Expr::Name("r".into())),
            right: Box::new(Expr::Literal(Value::Int(2))),
        },
        location: None,
    };

    exec_stmt(
        &mut ctx,
        &Stmt::Assign {
            target: LValue::Name("r".into()),
            value: Expr::Literal(Value::Int(0)),
            location: None,
        },
    )
    .unwrap();
    exec_stmt(&mut ctx, &repeat_loop).unwrap();
    assert_eq!(ctx.storage.get_global("r"), Some(&Value::Int(2)));
}
