/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Pos,
    Not,
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    And,
    Or,
    /// Boolean AND with source-level short-circuit evaluation.
    AndThen,
    /// Boolean OR with source-level short-circuit evaluation.
    OrElse,
    Xor,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
}

pub fn apply_unary(op: UnaryOp, value: Value) -> Result<Value, RuntimeError> {
    match op {
        UnaryOp::Neg => match value {
            Value::SInt(v) => Ok(Value::SInt(v.checked_neg().ok_or(RuntimeError::Overflow)?)),
            Value::Int(v) => Ok(Value::Int(v.checked_neg().ok_or(RuntimeError::Overflow)?)),
            Value::DInt(v) => Ok(Value::DInt(v.checked_neg().ok_or(RuntimeError::Overflow)?)),
            Value::LInt(v) => Ok(Value::LInt(v.checked_neg().ok_or(RuntimeError::Overflow)?)),
            Value::Real(v) => Ok(Value::Real(-v)),
            Value::LReal(v) => Ok(Value::LReal(-v)),
            _ => Err(RuntimeError::TypeMismatch),
        },
        UnaryOp::Pos => Ok(value),
        UnaryOp::Not => match value {
            Value::Bool(v) => Ok(Value::Bool(!v)),
            Value::Byte(v) => Ok(Value::Byte(!v)),
            Value::Word(v) => Ok(Value::Word(!v)),
            Value::DWord(v) => Ok(Value::DWord(!v)),
            Value::LWord(v) => Ok(Value::LWord(!v)),
            _ => Err(RuntimeError::TypeMismatch),
        },
    }
}

pub fn apply_binary(
    op: BinaryOp,
    left: Value,
    right: Value,
    profile: &DateTimeProfile,
) -> Result<Value, RuntimeError> {
    if let Some(value) = same_type_binary(op, &left, &right) {
        return Ok(value);
    }
    if let Some(result) = time_arith(op, &left, &right, profile) {
        return result;
    }
    if let Some(result) = time_cmp(op, &left, &right) {
        return result;
    }
    match op {
        BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::AndThen
        | BinaryOp::OrElse
        | BinaryOp::Xor => logical_or_bitwise(op, left, right),
        BinaryOp::Eq => numeric_eq(left, right, true),
        BinaryOp::Ne => numeric_eq(left, right, false),
        BinaryOp::Add => numeric_arith(op, left, right),
        BinaryOp::Sub => numeric_arith(op, left, right),
        BinaryOp::Mul => numeric_arith(op, left, right),
        BinaryOp::Div => numeric_arith(op, left, right),
        BinaryOp::Mod => numeric_arith(op, left, right),
        BinaryOp::Pow => numeric_arith(op, left, right),
        BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            if let Some(result) = non_numeric_cmp(op, &left, &right) {
                return result;
            }
            numeric_cmp(op, left, right)
        }
    }
}

/// A binary operation on two operands of the same common elementary type, computed exactly as the generic path computes it.
/// Any other operand pair, operator, overflow, zero divisor or non-finite result returns `None`, and the generic path then
/// produces the same value or error it always has.
fn same_type_binary(op: BinaryOp, left: &Value, right: &Value) -> Option<Value> {
    match (left, right) {
        (Value::Bool(a), Value::Bool(b)) => Some(Value::Bool(match op {
            BinaryOp::And | BinaryOp::AndThen => *a && *b,
            BinaryOp::Or | BinaryOp::OrElse => *a || *b,
            BinaryOp::Xor => *a ^ *b,
            BinaryOp::Eq => a == b,
            BinaryOp::Ne => a != b,
            _ => return None,
        })),
        (Value::Int(a), Value::Int(b)) => same_type_integer(op, *a, *b, Value::Int),
        (Value::DInt(a), Value::DInt(b)) => same_type_integer(op, *a, *b, Value::DInt),
        (Value::LInt(a), Value::LInt(b)) => same_type_integer(op, *a, *b, Value::LInt),
        (Value::LReal(a), Value::LReal(b)) => same_type_float(op, *a, *b, |result| Some(Value::LReal(result))),
        (Value::Real(a), Value::Real(b)) => same_type_float(op, f64::from(*a), f64::from(*b), |result| {
            let narrowed = result as f32;
            narrowed.is_finite().then_some(Value::Real(narrowed))
        }),
        _ => None,
    }
}

/// Checked signed integer arithmetic for the same-type fast path.
trait SameTypeInteger: Copy + Ord {
    fn checked(op: BinaryOp, a: Self, b: Self) -> Option<Self>;
}

macro_rules! impl_same_type_integer {
    ($($ty:ty),*) => {$(
        impl SameTypeInteger for $ty {
            fn checked(op: BinaryOp, a: Self, b: Self) -> Option<Self> {
                match op {
                    BinaryOp::Add => a.checked_add(b),
                    BinaryOp::Sub => a.checked_sub(b),
                    BinaryOp::Mul => a.checked_mul(b),
                    BinaryOp::Div => a.checked_div(b),
                    BinaryOp::Mod => a.checked_rem(b),
                    _ => None,
                }
            }
        }
    )*};
}

impl_same_type_integer!(i16, i32, i64);

/// Signed integer arithmetic and comparison with the generic path's results; overflow, a zero divisor and `MIN / -1`
/// return `None`.
fn same_type_integer<T: SameTypeInteger>(op: BinaryOp, a: T, b: T, wrap: fn(T) -> Value) -> Option<Value> {
    Some(match op {
        BinaryOp::Eq => Value::Bool(a == b),
        BinaryOp::Ne => Value::Bool(a != b),
        BinaryOp::Lt => Value::Bool(a < b),
        BinaryOp::Le => Value::Bool(a <= b),
        BinaryOp::Gt => Value::Bool(a > b),
        BinaryOp::Ge => Value::Bool(a >= b),
        _ => wrap(T::checked(op, a, b)?),
    })
}

/// Floating-point arithmetic in f64 or comparison, as the generic path computes them; a zero divisor or a non-finite
/// result returns `None`.
fn same_type_float(op: BinaryOp, a: f64, b: f64, wrap: fn(f64) -> Option<Value>) -> Option<Value> {
    let result = match op {
        BinaryOp::Add => a + b,
        BinaryOp::Sub => a - b,
        BinaryOp::Mul => a * b,
        BinaryOp::Div if b != 0.0 => a / b,
        BinaryOp::Eq => return Some(Value::Bool(a == b)),
        BinaryOp::Ne => return Some(Value::Bool(a != b)),
        BinaryOp::Lt => return Some(Value::Bool(a < b)),
        BinaryOp::Le => return Some(Value::Bool(a <= b)),
        BinaryOp::Gt => return Some(Value::Bool(a > b)),
        BinaryOp::Ge => return Some(Value::Bool(a >= b)),
        _ => return None,
    };
    if result.is_finite() {
        wrap(result)
    } else {
        None
    }
}
