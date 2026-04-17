use std::fmt;

use crate::error::RuntimeError;
use crate::program_model::ops::{apply_binary, apply_unary};
use crate::program_model::Expr;
use crate::value::{size_of_type, DateTimeProfile, SizeOfError, Value};
use trust_hir::types::TypeRegistry;

#[derive(Debug)]
pub(crate) enum ConstExprError {
    UnsupportedExpr,
    Runtime(RuntimeError),
}

impl From<RuntimeError> for ConstExprError {
    fn from(value: RuntimeError) -> Self {
        Self::Runtime(value)
    }
}

impl fmt::Display for ConstExprError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedExpr => write!(f, "expression is not a compile-time constant"),
            Self::Runtime(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for ConstExprError {}

pub(crate) fn eval_const_expr(
    expr: &Expr,
    profile: &DateTimeProfile,
) -> Result<Value, ConstExprError> {
    eval_const_expr_with_resolver(expr, profile, &|_| None)
}

pub(crate) fn eval_const_expr_with_resolver(
    expr: &Expr,
    profile: &DateTimeProfile,
    resolve_name: &impl Fn(&str) -> Option<Value>,
) -> Result<Value, ConstExprError> {
    let registry = TypeRegistry::new();
    eval_const_expr_with_resolver_and_registry(expr, profile, &registry, resolve_name)
}

pub(crate) fn eval_const_expr_with_resolver_and_registry(
    expr: &Expr,
    profile: &DateTimeProfile,
    registry: &TypeRegistry,
    resolve_name: &impl Fn(&str) -> Option<Value>,
) -> Result<Value, ConstExprError> {
    if let Some(name) = qualified_const_name(expr) {
        if let Some(value) = resolve_name(&name) {
            return Ok(value);
        }
    }

    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::Unary { op, expr } => {
            let value = eval_const_expr_with_resolver(expr, profile, resolve_name)?;
            Ok(apply_unary(*op, value)?)
        }
        Expr::Binary { op, left, right } => {
            let left =
                eval_const_expr_with_resolver_and_registry(left, profile, registry, resolve_name)?;
            let right =
                eval_const_expr_with_resolver_and_registry(right, profile, registry, resolve_name)?;
            Ok(apply_binary(*op, left, right, profile)?)
        }
        Expr::SizeOf(crate::program_model::SizeOfTarget::Type(type_id)) => {
            let size = size_of_type(*type_id, registry).map_err(size_error_to_const)?;
            let size =
                i32::try_from(size).map_err(|_| ConstExprError::Runtime(RuntimeError::Overflow))?;
            Ok(Value::DInt(size))
        }
        _ => Err(ConstExprError::UnsupportedExpr),
    }
}

fn size_error_to_const(err: SizeOfError) -> ConstExprError {
    let runtime = match err {
        SizeOfError::Overflow => RuntimeError::Overflow,
        SizeOfError::UnknownType | SizeOfError::UnsupportedType => RuntimeError::TypeMismatch,
    };
    ConstExprError::Runtime(runtime)
}

fn qualified_const_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Name(name) => Some(name.to_string()),
        Expr::Field { target, field } => {
            let prefix = qualified_const_name(target)?;
            Some(format!("{prefix}.{field}"))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::program_model::ops::{BinaryOp, UnaryOp};

    #[test]
    fn evaluates_nested_const_expression() {
        let expr = Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expr::Binary {
                op: BinaryOp::Add,
                left: Box::new(Expr::Literal(Value::Int(2))),
                right: Box::new(Expr::Literal(Value::Int(3))),
            }),
            right: Box::new(Expr::Unary {
                op: UnaryOp::Neg,
                expr: Box::new(Expr::Literal(Value::Int(4))),
            }),
        };

        let value = eval_const_expr(&expr, &DateTimeProfile::default()).unwrap();
        assert_eq!(value, Value::Int(-20));
    }

    #[test]
    fn rejects_non_const_access() {
        let expr = Expr::Name("count".into());
        assert!(matches!(
            eval_const_expr(&expr, &DateTimeProfile::default()),
            Err(ConstExprError::UnsupportedExpr)
        ));
    }

    #[test]
    fn resolves_named_const_with_resolver() {
        let expr = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Name("LEN".into())),
            right: Box::new(Expr::Literal(Value::Int(2))),
        };

        let value = eval_const_expr_with_resolver(&expr, &DateTimeProfile::default(), &|name| {
            (name == "LEN").then_some(Value::Int(10))
        })
        .unwrap();
        assert_eq!(value, Value::Int(12));
    }
}
