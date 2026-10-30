use std::fmt;

use super::storage_context::EvaluationTypes;
use crate::error::RuntimeError;
use crate::program_model::ops::{apply_binary, apply_unary};
use crate::program_model::{BinaryOp, Expr};
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

#[cfg(test)]
pub(crate) fn eval_const_expr(
    expr: &Expr,
    profile: &DateTimeProfile,
) -> Result<Value, ConstExprError> {
    eval_const_expr_with_resolver(expr, profile, &|_| None)
}

#[cfg(test)]
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
    eval_const_expr_with_types(expr, profile, &EvaluationTypes { registry }, resolve_name)
}

fn eval_const_expr_with_types(
    expr: &Expr,
    profile: &DateTimeProfile,
    registry: &EvaluationTypes<'_>,
    resolve_name: &impl Fn(&str) -> Option<Value>,
) -> Result<Value, ConstExprError> {
    if let Some(name) = qualified_const_name(expr)
        && let Some(value) = resolve_name(&name)
    {
        return Ok(value);
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
            let value = eval_const_expr_with_types(expr, profile, registry, resolve_name)?;
            Ok(apply_unary(*op, value)?)
        }
        Expr::Binary { op, left, right } => {
            let left = eval_const_expr_with_types(left, profile, registry, resolve_name)?;
            let right = eval_const_expr_with_types(right, profile, registry, resolve_name)?;
            // CODESYS-parity constant evaluation (IEC_DEVIATIONS DEV-027):
            // mixed-sign integer constants evaluate exactly in i128 —
            // compile-time math is exact, so `SIZEOF(x) - 1` with an
            // unsigned SIZEOF cannot fault. Same-sign pairs and every
            // non-integer pair keep the shared apply_binary contract (the
            // VM execution path is unchanged); when a narrow-typed pair
            // faults only on materialization, the exact fold below keeps
            // a representable magnitude from failing at compile time.
            if let Some(result) = eval_mixed_sign_integer_binary(*op, &left, &right)? {
                return Ok(result);
            }
            let exact_narrow = exact_narrow_integer_binary(*op, &left, &right)?;
            match apply_binary(*op, left, right, profile) {
                Ok(value) => Ok(value),
                Err(RuntimeError::Overflow) => match exact_narrow {
                    Some(result) => Ok(result),
                    None => Err(RuntimeError::Overflow.into()),
                },
                Err(error) => Err(error.into()),
            }
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

/// CODESYS-parity constant arithmetic (IEC_DEVIATIONS DEV-027): a true
/// mixed signed×unsigned integer pair evaluates exactly in i128 and
/// materializes as LINT (negative) or ULINT (non-negative), so bounds and
/// initializers like `SIZEOF(x) - 1` keep working when SIZEOF carries its
/// natural unsigned magnitude. Returns `None` for same-family pairs and
/// non-integers, which keep the shared `apply_binary` contract.
fn eval_mixed_sign_integer_binary(
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, RuntimeError> {
    fn signedness(value: &Value) -> Option<bool> {
        // Some(true) = signed integer, Some(false) = unsigned integer.
        match value {
            Value::SInt(_) | Value::Int(_) | Value::DInt(_) | Value::LInt(_) => Some(true),
            Value::USInt(_)
            | Value::UInt(_)
            | Value::UDInt(_)
            | Value::ULInt(_)
            | Value::Byte(_)
            | Value::Word(_)
            | Value::DWord(_)
            | Value::LWord(_) => Some(false),
            _ => None,
        }
    }
    fn magnitude(value: &Value) -> Option<i128> {
        match value {
            Value::SInt(v) => Some(i128::from(*v)),
            Value::Int(v) => Some(i128::from(*v)),
            Value::DInt(v) => Some(i128::from(*v)),
            Value::LInt(v) => Some(i128::from(*v)),
            Value::USInt(v) => Some(i128::from(*v)),
            Value::UInt(v) => Some(i128::from(*v)),
            Value::UDInt(v) => Some(i128::from(*v)),
            Value::ULInt(v) => Some(i128::from(*v)),
            Value::Byte(v) => Some(i128::from(*v)),
            Value::Word(v) => Some(i128::from(*v)),
            Value::DWord(v) => Some(i128::from(*v)),
            Value::LWord(v) => Some(i128::from(*v)),
            _ => None,
        }
    }
    let (Some(left_is_signed), Some(right_is_signed)) = (signedness(left), signedness(right))
    else {
        return Ok(None);
    };
    if left_is_signed == right_is_signed {
        return Ok(None);
    }
    let (Some(a), Some(b)) = (magnitude(left), magnitude(right)) else {
        return Ok(None);
    };
    let result = match op {
        BinaryOp::Add => a.checked_add(b).ok_or(RuntimeError::Overflow)?,
        BinaryOp::Sub => a.checked_sub(b).ok_or(RuntimeError::Overflow)?,
        BinaryOp::Mul => a.checked_mul(b).ok_or(RuntimeError::Overflow)?,
        BinaryOp::Div => {
            if b == 0 {
                return Err(RuntimeError::DivisionByZero);
            }
            a.checked_div(b).ok_or(RuntimeError::Overflow)?
        }
        BinaryOp::Mod => {
            if b == 0 {
                return Err(RuntimeError::ModuloByZero);
            }
            a.checked_rem(b).ok_or(RuntimeError::Overflow)?
        }
        _ => return Ok(None),
    };
    if result < 0 {
        i64::try_from(result)
            .map(|value| Some(Value::LInt(value)))
            .map_err(|_| RuntimeError::Overflow)
    } else {
        u64::try_from(result)
            .map(|value| Some(Value::ULInt(value)))
            .map_err(|_| RuntimeError::Overflow)
    }
}

/// CODESYS-parity constant folding (IEC_DEVIATIONS DEV-027): retry a
/// narrow-typed same-sign integer pair that faulted on materialization,
/// exactly in i128, so a representable magnitude cannot fail at compile
/// time. Untyped literals materialize as the narrowest fitting tag, so
/// `16 * 8` arrives as `SInt * SInt` and faults on the i8 narrowing, not
/// on the arithmetic; the exact fold materializes as LINT (negative) or
/// ULINT (non-negative) like the mixed-sign helper. Returns `None` for
/// anything but the narrow tags (SInt/Int/USInt/UInt): typed DINT/LINT
/// pairs keep their fault contract, bit-string tags keep DEV-027's
/// wrapping arithmetic, and `Pow` stays out of the exact-folding set.
fn exact_narrow_integer_binary(
    op: BinaryOp,
    left: &Value,
    right: &Value,
) -> Result<Option<Value>, RuntimeError> {
    fn narrow_magnitude(value: &Value) -> Option<i128> {
        match value {
            Value::SInt(v) => Some(i128::from(*v)),
            Value::Int(v) => Some(i128::from(*v)),
            Value::USInt(v) => Some(i128::from(*v)),
            Value::UInt(v) => Some(i128::from(*v)),
            _ => None,
        }
    }
    let (Some(a), Some(b)) = (narrow_magnitude(left), narrow_magnitude(right)) else {
        return Ok(None);
    };
    let result = match op {
        BinaryOp::Add => a.checked_add(b).ok_or(RuntimeError::Overflow)?,
        BinaryOp::Sub => a.checked_sub(b).ok_or(RuntimeError::Overflow)?,
        BinaryOp::Mul => a.checked_mul(b).ok_or(RuntimeError::Overflow)?,
        BinaryOp::Div => {
            if b == 0 {
                return Err(RuntimeError::DivisionByZero);
            }
            a.checked_div(b).ok_or(RuntimeError::Overflow)?
        }
        BinaryOp::Mod => {
            if b == 0 {
                return Err(RuntimeError::ModuloByZero);
            }
            a.checked_rem(b).ok_or(RuntimeError::Overflow)?
        }
        _ => return Ok(None),
    };
    if result < 0 {
        i64::try_from(result)
            .map(|value| Some(Value::LInt(value)))
            .map_err(|_| RuntimeError::Overflow)
    } else {
        u64::try_from(result)
            .map(|value| Some(Value::ULInt(value)))
            .map_err(|_| RuntimeError::Overflow)
    }
}

fn eval_array_initializer_elements(
    elements: &[Expr],
    profile: &DateTimeProfile,
    registry: &EvaluationTypes<'_>,
    resolve_name: &impl Fn(&str) -> Option<Value>,
) -> Result<Vec<Value>, ConstExprError> {
    let mut values = Vec::new();
    for expr in elements {
        eval_array_initializer_element(profile, registry, resolve_name, expr, &mut values, 0)?;
    }
    Ok(values)
}

fn eval_array_initializer_element(
    profile: &DateTimeProfile,
    registry: &EvaluationTypes<'_>,
    resolve_name: &impl Fn(&str) -> Option<Value>,
    expr: &Expr,
    values: &mut Vec<Value>,
    depth: u8,
) -> Result<(), ConstExprError> {
    if depth > 64 {
        return Err(ConstExprError::UnsupportedExpr);
    }
    if let Some((count, repeated_args)) = array_repeat_group(expr)? {
        for _ in 0..count {
            for arg in repeated_args {
                let crate::program_model::ArgValue::Expr(value_expr) = &arg.value else {
                    return Err(ConstExprError::UnsupportedExpr);
                };
                eval_array_initializer_element(
                    profile,
                    registry,
                    resolve_name,
                    value_expr,
                    values,
                    depth + 1,
                )?;
            }
        }
        return Ok(());
    }
    values.push(eval_const_expr_with_types(
        expr,
        profile,
        registry,
        resolve_name,
    )?);
    Ok(())
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
#[path = "const_expr/contract_tests.rs"]
mod contract_tests;

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

    #[test]
    fn narrow_same_sign_folds_materialize_wide_instead_of_faulting() {
        // IEC_DEVIATIONS DEV-027: untyped literals materialize narrowest
        // fitting, so `16 * 8` arrives as SInt * SInt and faults on the
        // i8 narrowing. The compile-time exact fold keeps a representable
        // magnitude (the CODESYS syslog `N * 8` idiom).
        let mul = Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expr::Literal(Value::SInt(16))),
            right: Box::new(Expr::Literal(Value::SInt(8))),
        };
        assert_eq!(
            eval_const_expr(&mul, &DateTimeProfile::default()).unwrap(),
            Value::ULInt(128)
        );

        let unsigned = Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expr::Literal(Value::USInt(200))),
            right: Box::new(Expr::Literal(Value::USInt(2))),
        };
        assert_eq!(
            eval_const_expr(&unsigned, &DateTimeProfile::default()).unwrap(),
            Value::ULInt(400)
        );

        let negative = Expr::Binary {
            op: BinaryOp::Mul,
            left: Box::new(Expr::Literal(Value::SInt(-64))),
            right: Box::new(Expr::Literal(Value::SInt(3))),
        };
        assert_eq!(
            eval_const_expr(&negative, &DateTimeProfile::default()).unwrap(),
            Value::LInt(-192)
        );
    }

    #[test]
    fn narrow_folds_without_fault_keep_their_result_tag() {
        let add = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Literal(Value::Int(2))),
            right: Box::new(Expr::Literal(Value::Int(3))),
        };
        assert_eq!(
            eval_const_expr(&add, &DateTimeProfile::default()).unwrap(),
            Value::Int(5)
        );
    }

    #[test]
    fn typed_wide_pairs_keep_their_overflow_fault() {
        let add = Expr::Binary {
            op: BinaryOp::Add,
            left: Box::new(Expr::Literal(Value::LInt(i64::MAX))),
            right: Box::new(Expr::Literal(Value::LInt(1))),
        };
        assert!(matches!(
            eval_const_expr(&add, &DateTimeProfile::default()),
            Err(ConstExprError::Runtime(RuntimeError::Overflow))
        ));
    }
}
