use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::{eval_expr, expr::Expr, ops::BinaryOp};
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::{
    DateTimeValue, DateValue, Duration, LDateTimeValue, TimeOfDayValue, Value,
};

#[test]
fn time_arithmetic_and_compare() {
    let mut storage = VariableStorage::new();
    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);

    let expr = Expr::Binary {
        op: BinaryOp::Add,
        left: Box::new(Expr::Literal(Value::Time(Duration::from_millis(1000)))),
        right: Box::new(Expr::Literal(Value::Time(Duration::from_millis(500)))),
    };
    assert_eq!(
        eval_expr(&mut ctx, &expr).unwrap(),
        Value::Time(Duration::from_millis(1500))
    );

    let expr = Expr::Binary {
        op: BinaryOp::Add,
        left: Box::new(Expr::Literal(Value::Tod(TimeOfDayValue::new(1000)))),
        right: Box::new(Expr::Literal(Value::Time(Duration::from_millis(500)))),
    };
    assert_eq!(
        eval_expr(&mut ctx, &expr).unwrap(),
        Value::Tod(TimeOfDayValue::new(1500))
    );

    let expr = Expr::Binary {
        op: BinaryOp::Sub,
        left: Box::new(Expr::Literal(Value::Dt(DateTimeValue::new(2500)))),
        right: Box::new(Expr::Literal(Value::Dt(DateTimeValue::new(1000)))),
    };
    assert_eq!(
        eval_expr(&mut ctx, &expr).unwrap(),
        Value::Time(Duration::from_millis(1500))
    );

    let expr = Expr::Binary {
        op: BinaryOp::Mul,
        left: Box::new(Expr::Literal(Value::LTime(Duration::from_secs(2)))),
        right: Box::new(Expr::Literal(Value::Int(3))),
    };
    assert_eq!(
        eval_expr(&mut ctx, &expr).unwrap(),
        Value::LTime(Duration::from_secs(6))
    );

    let expr = Expr::Binary {
        op: BinaryOp::Div,
        left: Box::new(Expr::Literal(Value::Time(Duration::from_millis(1000)))),
        right: Box::new(Expr::Literal(Value::Int(2))),
    };
    assert_eq!(
        eval_expr(&mut ctx, &expr).unwrap(),
        Value::Time(Duration::from_millis(500))
    );

    let expr = Expr::Binary {
        op: BinaryOp::Lt,
        left: Box::new(Expr::Literal(Value::Date(DateValue::new(2)))),
        right: Box::new(Expr::Literal(Value::Date(DateValue::new(5)))),
    };
    assert_eq!(eval_expr(&mut ctx, &expr).unwrap(), Value::Bool(true));

    let expr = Expr::Binary {
        op: BinaryOp::Sub,
        left: Box::new(Expr::Literal(Value::Ldt(LDateTimeValue::new(10)))),
        right: Box::new(Expr::Literal(Value::Ldt(LDateTimeValue::new(3)))),
    };
    assert_eq!(
        eval_expr(&mut ctx, &expr).unwrap(),
        Value::LTime(Duration::from_nanos(7))
    );
}
