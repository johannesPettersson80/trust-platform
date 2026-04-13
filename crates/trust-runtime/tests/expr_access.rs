mod common;

use indexmap::IndexMap;
use trust_hir::types::TypeRegistry;
use trust_runtime::eval::{eval_expr, expr::Expr};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::{ArrayValue, StructValue, Value};

#[test]
fn index_and_field() {
    let mut storage = VariableStorage::new();
    let array = Value::Array(Box::new(ArrayValue {
        elements: vec![Value::Int(1), Value::Int(2), Value::Int(3)],
        dimensions: vec![(0, 2)],
    }));
    storage.set_global("arr", array);

    let mut fields = IndexMap::new();
    fields.insert("a".into(), Value::Int(10));
    let struct_value = Value::Struct(std::sync::Arc::new(StructValue {
        type_name: "S".into(),
        fields,
    }));
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
