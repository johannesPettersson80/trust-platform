use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::expr::{Expr, LValue};
use trust_runtime::eval::stmt::{exec_stmt, CaseLabel, Stmt};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn case_labels() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::Int(0));
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let stmt = Stmt::Case {
        selector: Expr::Literal(Value::Int(2)),
        branches: vec![
            (
                vec![CaseLabel::Single(Value::Int(1))],
                vec![Stmt::Assign {
                    target: LValue::Name("x".into()),
                    value: Expr::Literal(Value::Int(1)),
                    location: None,
                }],
            ),
            (
                vec![CaseLabel::Range(Value::Int(2), Value::Int(3))],
                vec![Stmt::Assign {
                    target: LValue::Name("x".into()),
                    value: Expr::Literal(Value::Int(9)),
                    location: None,
                }],
            ),
        ],
        else_block: vec![],
        location: None,
    };

    exec_stmt(&mut ctx, &stmt).unwrap();
    assert_eq!(storage.get_global("x"), Some(&Value::Int(9)));
}

#[test]
fn case_string_labels() {
    let mut storage = VariableStorage::new();
    storage.set_global("x", Value::Int(0));
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let stmt = Stmt::Case {
        selector: Expr::Literal(Value::String("B".into())),
        branches: vec![
            (
                vec![CaseLabel::Single(Value::String("A".into()))],
                vec![Stmt::Assign {
                    target: LValue::Name("x".into()),
                    value: Expr::Literal(Value::Int(1)),
                    location: None,
                }],
            ),
            (
                vec![CaseLabel::Single(Value::String("B".into()))],
                vec![Stmt::Assign {
                    target: LValue::Name("x".into()),
                    value: Expr::Literal(Value::Int(9)),
                    location: None,
                }],
            ),
        ],
        else_block: vec![],
        location: None,
    };

    exec_stmt(&mut ctx, &stmt).unwrap();
    assert_eq!(storage.get_global("x"), Some(&Value::Int(9)));
}

#[test]
fn case_range_labels_match_unsigned_and_bit_string_selectors() {
    let cases = [
        (Value::USInt(3), Value::USInt(1), Value::USInt(5)),
        (Value::UDInt(3), Value::UDInt(1), Value::UDInt(5)),
        (Value::Byte(3), Value::Byte(1), Value::Byte(5)),
        (Value::Word(3), Value::Word(1), Value::Word(5)),
    ];
    for (selector, lower, upper) in cases {
        let mut storage = VariableStorage::new();
        storage.set_global("x", Value::Int(0));
        let registry = TypeRegistry::new();
        let mut ctx = common::make_context(&mut storage, &registry);

        let stmt = Stmt::Case {
            selector: Expr::Literal(selector.clone()),
            branches: vec![(
                vec![CaseLabel::Range(lower, upper)],
                vec![Stmt::Assign {
                    target: LValue::Name("x".into()),
                    value: Expr::Literal(Value::Int(9)),
                    location: None,
                }],
            )],
            else_block: vec![],
            location: None,
        };

        exec_stmt(&mut ctx, &stmt).unwrap_or_else(|err| panic!("{selector:?}: {err:?}"));
        assert_eq!(
            storage.get_global("x"),
            Some(&Value::Int(9)),
            "{selector:?}"
        );
    }
}
