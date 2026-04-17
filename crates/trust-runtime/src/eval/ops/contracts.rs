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
    if let Some(result) = time_arith(op, &left, &right, profile) {
        return result;
    }
    if let Some(result) = time_cmp(op, &left, &right) {
        return result;
    }
    match op {
        BinaryOp::And | BinaryOp::Or | BinaryOp::Xor => logical_or_bitwise(op, left, right),
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
