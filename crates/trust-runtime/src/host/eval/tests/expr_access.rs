use super::common;

use indexmap::IndexMap;
use trust_hir::types::TypeRegistry;
use trust_runtime::eval::{eval_expr, expr::Expr};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::{ArrayValue, StructValue, Value};

#[test]
fn index_and_field() {
    let mut storage = VariableStorage::new();
    let array = Value::Array(Box::new(
        ArrayValue::from_untyped_parts(
            vec![Value::Int(1), Value::Int(2), Value::Int(3)],
            vec![(0, 2)],
        )
        .unwrap(),
    ));
    storage.set_global("arr", array);

    let mut fields = IndexMap::new();
    fields.insert("a".into(), Value::Int(10));
    let struct_value = Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
        "S".into(),
        fields,
    )));
    storage.set_global("st", struct_value);

    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let index_expr = Expr::Index {
        target: Box::new(Expr::Name("arr".into())),
        indices: vec![Expr::Literal(Value::Int(1))],
    };
    let field_expr = Expr::Field {
        target: Box::new(Expr::Name("st".into())),
        field: "a".into(),
    };

    let index_value = eval_expr(&mut ctx, &index_expr).unwrap();
    let field_value = eval_expr(&mut ctx, &field_expr).unwrap();

    assert_eq!(index_value, Value::Int(2));
    assert_eq!(field_value, Value::Int(10));
}

#[test]
fn nested_index_and_field_chains() {
    let mut storage = VariableStorage::new();
    let nested_structs = Value::Array(Box::new(
        ArrayValue::from_untyped_parts(
            vec![
                Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                    "Item".into(),
                    IndexMap::from([("value".into(), Value::Int(10))]),
                ))),
                Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
                    "Item".into(),
                    IndexMap::from([("value".into(), Value::Int(20))]),
                ))),
            ],
            vec![(0, 1)],
        )
        .unwrap(),
    ));
    storage.set_global("items", nested_structs);

    let nested_array = Value::Struct(std::sync::Arc::new(StructValue::from_untyped_parts(
        "Outer".into(),
        IndexMap::from([(
            "arr".into(),
            Value::Array(Box::new(
                ArrayValue::from_untyped_parts(
                    vec![Value::Int(3), Value::Int(4), Value::Int(5)],
                    vec![(0, 2)],
                )
                .unwrap(),
            )),
        )]),
    )));
    storage.set_global("outer", nested_array);

    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let field_after_index = Expr::Field {
        target: Box::new(Expr::Index {
            target: Box::new(Expr::Name("items".into())),
            indices: vec![Expr::Literal(Value::Int(1))],
        }),
        field: "value".into(),
    };
    let index_after_field = Expr::Index {
        target: Box::new(Expr::Field {
            target: Box::new(Expr::Name("outer".into())),
            field: "arr".into(),
        }),
        indices: vec![Expr::Literal(Value::Int(1))],
    };

    let field_after_index_value = eval_expr(&mut ctx, &field_after_index).unwrap();
    let index_after_field_value = eval_expr(&mut ctx, &index_after_field).unwrap();

    assert_eq!(field_after_index_value, Value::Int(20));
    assert_eq!(index_after_field_value, Value::Int(4));
}
