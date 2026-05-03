use std::fmt;

use crate::error::RuntimeError;
use crate::program_model::ops::{apply_binary, apply_unary};
use crate::program_model::Expr;
use crate::value::{size_of_type, ArrayValue, DateTimeProfile, SizeOfError, Value};
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
        Expr::ArrayInitializer(elements) => {
            let values =
                eval_array_initializer_elements(elements, profile, registry, resolve_name)?;
            let len = values.len() as i64;
            ArrayValue::from_untyped_parts(values, vec![(1, len)])
                .map(|value| Value::Array(Box::new(value)))
                .map_err(|_| RuntimeError::TypeMismatch.into())
        }
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

fn eval_array_initializer_elements(
    elements: &[Expr],
    profile: &DateTimeProfile,
    registry: &TypeRegistry,
    resolve_name: &impl Fn(&str) -> Option<Value>,
) -> Result<Vec<Value>, ConstExprError> {
    let mut values = Vec::new();
    for expr in elements {
        if let Some((count, repeated_args)) = array_repeat_group(expr)? {
            for _ in 0..count {
                for arg in repeated_args {
                    let crate::program_model::ArgValue::Expr(value_expr) = &arg.value else {
                        return Err(ConstExprError::UnsupportedExpr);
                    };
                    values.push(eval_const_expr_with_resolver_and_registry(
                        value_expr,
                        profile,
                        registry,
                        resolve_name,
                    )?);
                }
            }
            continue;
        }
        values.push(eval_const_expr_with_resolver_and_registry(
            expr,
            profile,
            registry,
            resolve_name,
        )?);
    }
    Ok(values)
}

fn array_repeat_group(
    expr: &Expr,
) -> Result<Option<(usize, &[crate::program_model::CallArg])>, ConstExprError> {
    let Expr::Call { target, args } = expr else {
        return Ok(None);
    };
    if args.iter().any(|arg| arg.name.is_some()) {
        return Err(ConstExprError::UnsupportedExpr);
    }
    let count = match target.as_ref() {
        Expr::Literal(Value::SInt(v)) => i64::from(*v),
        Expr::Literal(Value::Int(v)) => i64::from(*v),
        Expr::Literal(Value::DInt(v)) => i64::from(*v),
        Expr::Literal(Value::LInt(v)) => *v,
        Expr::Literal(Value::USInt(v)) => i64::from(*v),
        Expr::Literal(Value::UInt(v)) => i64::from(*v),
        Expr::Literal(Value::UDInt(v)) => i64::from(*v),
        Expr::Literal(Value::ULInt(v)) => {
            i64::try_from(*v).map_err(|_| ConstExprError::UnsupportedExpr)?
        }
        _ => return Ok(None),
    };
    if count < 0 {
        return Err(ConstExprError::UnsupportedExpr);
    }
    let count = usize::try_from(count).map_err(|_| ConstExprError::UnsupportedExpr)?;
    Ok(Some((count, args)))
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

    #[test]
    fn array_repetition_initializer_uses_expanded_value_shape() {
        let expr = Expr::ArrayInitializer(vec![Expr::Call {
            target: Box::new(Expr::Literal(Value::Int(3))),
            args: vec![
                crate::program_model::CallArg {
                    name: None,
                    value: crate::program_model::ArgValue::Expr(Expr::Literal(Value::Int(1))),
                },
                crate::program_model::CallArg {
                    name: None,
                    value: crate::program_model::ArgValue::Expr(Expr::Literal(Value::Int(2))),
                },
            ],
        }]);

        let value = eval_const_expr(&expr, &DateTimeProfile::default()).unwrap();
        let Value::Array(array) = value else {
            panic!("expected array value");
        };
        assert_eq!(array.dimensions(), &[(1, 6)]);
        assert_eq!(
            array.elements(),
            &[
                Value::Int(1),
                Value::Int(2),
                Value::Int(1),
                Value::Int(2),
                Value::Int(1),
                Value::Int(2),
            ]
        );
    }
}
