use std::fmt;

use crate::error::RuntimeError;
use crate::program_model::ops::{apply_binary, apply_unary};
use crate::program_model::Expr;
use crate::value::{DateTimeProfile, Value};

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
    match expr {
        Expr::Literal(value) => Ok(value.clone()),
        Expr::Unary { op, expr } => {
            let value = eval_const_expr(expr, profile)?;
            Ok(apply_unary(*op, value)?)
        }
        Expr::Binary { op, left, right } => {
            let left = eval_const_expr(left, profile)?;
            let right = eval_const_expr(right, profile)?;
            Ok(apply_binary(*op, left, right, profile)?)
        }
        _ => Err(ConstExprError::UnsupportedExpr),
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
}
