//! The portable debug expressions of specification 11 §6.13.2, version 1: the form into which a tool
//! compiles a watch, a breakpoint condition, a logpoint fragment or a `debug.evaluate`, and which a
//! runtime decodes and evaluates against its published state without parsing ST text. A node is a
//! tag byte with its operands, in prefix order; a name is resolved by the runtime ([`Names`]), and
//! the operators compute as the runtime's statements do ([`apply_unary`], [`apply_binary`]).

use alloc::boxed::Box;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::program_model::{apply_binary, apply_unary, BinaryOp, UnaryOp};
use trust_runtime_core::value::{DateTimeProfile, Duration, Value};

/// The version of the form this module decodes and encodes.
pub const EXPR_VERSION: u32 = 1;

/// The most bytes an expression takes.
pub const MAX_BYTES: usize = 4096;

/// The deepest nesting of nodes.
pub const MAX_DEPTH: usize = 32;

/// The most nodes one evaluation evaluates.
pub const MAX_WORK: u32 = 1024;

/// The most indices of one index node.
const MAX_INDICES: u8 = 8;

/// An expression of version 1.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// A literal value.
    Literal(Value),
    /// An enumeration's value by the names of its type and of the value, `Mode#Run`, whose number
    /// the runtime takes from the program it runs.
    Enumeration(SmolStr, SmolStr),
    /// A name, as written; the runtime resolves it.
    Name(SmolStr),
    /// A unary operator and its operand.
    Unary(UnaryOp, Box<Expression>),
    /// A binary operator and its operands.
    Binary(BinaryOp, Box<Expression>, Box<Expression>),
    /// An array and its indices.
    Index(Box<Expression>, Vec<Expression>),
    /// A structure or an instance and the name of its member.
    Field(Box<Expression>, SmolStr),
}

/// Why an expression gives no value (specification 11 §6.13.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionError {
    /// `expr_invalid`: the bytes do not decode to an expression the runtime accepts, a name
    /// resolves to no variable, or the result is no value.
    Invalid(String),
    /// `expr_evaluation_failed`: the expression decoded but its evaluation failed.
    Failed(String),
}

impl ExpressionError {
    /// The stable code of the refusal.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Invalid(_) => "expr_invalid",
            Self::Failed(_) => "expr_evaluation_failed",
        }
    }

    /// The detail of the refusal, which is not normative.
    #[must_use]
    pub fn detail(&self) -> &str {
        match self {
            Self::Invalid(detail) | Self::Failed(detail) => detail,
        }
    }
}

fn invalid(detail: impl Into<String>) -> ExpressionError {
    ExpressionError::Invalid(detail.into())
}

fn failed(detail: &str) -> ExpressionError {
    ExpressionError::Failed(String::from(detail))
}

/// Where an expression's names are read: a held scan's frame, then the globals, then the variables
/// of the program instances (specification 11 §6.13.2).
pub trait Names {
    /// The value of the variable `name` names, compared without regard to case; `None` for a name
    /// that names no variable or a variable that holds no value.
    fn read(&self, name: &str) -> Option<Value>;

    /// The value `value` of the enumeration `type_name` as the program declares it, both compared
    /// without regard to case; `None` when the program declares no such enumeration or value.
    fn enumeration(&self, type_name: &str, value: &str) -> Option<Value>;
}

/// The expression of `bytes`.
///
/// # Errors
///
/// [`ExpressionError::Invalid`] for bytes that do not decode to one expression of version 1.
pub fn decode(bytes: &[u8]) -> Result<Expression, ExpressionError> {
    if bytes.len() > MAX_BYTES {
        return Err(invalid("the expression is longer than 4,096 bytes"));
    }
    let mut reader = Reader { bytes, at: 0 };
    let expression = reader.node(0)?;
    if reader.at != bytes.len() {
        return Err(invalid("bytes follow the expression"));
    }
    Ok(expression)
}

/// The bytes of `expression`.
///
/// # Errors
///
/// [`ExpressionError::Invalid`] for an expression that version 1 cannot carry: a literal of
/// another type (an enumeration's value is an [`Expression::Enumeration`]), a text longer than 65,535 bytes, an index node of no or more than 8 indices, a
/// nesting deeper than 32 nodes, or more than 4,096 bytes.
pub fn encode(expression: &Expression) -> Result<Vec<u8>, ExpressionError> {
    let mut bytes = Vec::new();
    write_node(&mut bytes, expression, 0)?;
    if bytes.len() > MAX_BYTES {
        return Err(invalid("the expression is longer than 4,096 bytes"));
    }
    Ok(bytes)
}

/// The value of `expression`, its names read from `names`.
///
/// # Errors
///
/// [`ExpressionError::Invalid`] for a name that names no variable, an enumeration value the program
/// does not declare and a result that is no value;
/// [`ExpressionError::Failed`] for an evaluation that fails: an operation the runtime refuses for
/// its operands, an index outside its bounds, or more than 1,024 nodes evaluated.
pub fn evaluate(expression: &Expression, names: &dyn Names) -> Result<Value, ExpressionError> {
    let mut work = 0;
    let value = eval(expression, names, &mut work)?;
    if matches!(value, Value::Instance(_)) {
        return Err(invalid("the result is an instance, which holds no value"));
    }
    Ok(value)
}

struct Reader<'b> {
    bytes: &'b [u8],
    at: usize,
}

impl<'b> Reader<'b> {
    fn take(&mut self, count: usize) -> Result<&'b [u8], ExpressionError> {
        let end = self
            .at
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| invalid("the expression is truncated"))?;
        let bytes = &self.bytes[self.at..end];
        self.at = end;
        Ok(bytes)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], ExpressionError> {
        let mut bytes = [0; N];
        bytes.copy_from_slice(self.take(N)?);
        Ok(bytes)
    }

    fn byte(&mut self) -> Result<u8, ExpressionError> {
        Ok(self.array::<1>()?[0])
    }

    fn text(&mut self) -> Result<&'b str, ExpressionError> {
        let length = usize::from(u16::from_le_bytes(self.array()?));
        core::str::from_utf8(self.take(length)?).map_err(|_| invalid("a text is not UTF-8"))
    }

    fn node(&mut self, depth: usize) -> Result<Expression, ExpressionError> {
        if depth >= MAX_DEPTH {
            return Err(invalid("the expression is nested deeper than 32 nodes"));
        }
        Ok(match self.byte()? {
            1 => self.literal()?,
            2 => Expression::Name(SmolStr::new(self.text()?)),
            3 => {
                let op = unary_op(self.byte()?)?;
                Expression::Unary(op, Box::new(self.node(depth + 1)?))
            }
            4 => {
                let op = binary_op(self.byte()?)?;
                let left = self.node(depth + 1)?;
                let right = self.node(depth + 1)?;
                Expression::Binary(op, Box::new(left), Box::new(right))
            }
            5 => {
                let count = self.byte()?;
                if count == 0 || count > MAX_INDICES {
                    return Err(invalid("an index node has no or more than 8 indices"));
                }
                let target = self.node(depth + 1)?;
                let indices = (0..count)
                    .map(|_| self.node(depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                Expression::Index(Box::new(target), indices)
            }
            6 => {
                let member = SmolStr::new(self.text()?);
                Expression::Field(Box::new(self.node(depth + 1)?), member)
            }
            tag => return Err(invalid(format!("unknown node tag {tag}"))),
        })
    }

    fn literal(&mut self) -> Result<Expression, ExpressionError> {
        Ok(Expression::Literal(match self.byte()? {
            1 => match self.byte()? {
                0 => Value::Bool(false),
                1 => Value::Bool(true),
                other => return Err(invalid(format!("a BOOL literal of {other}"))),
            },
            2 => Value::SInt(i8::from_le_bytes(self.array()?)),
            3 => Value::Int(i16::from_le_bytes(self.array()?)),
            4 => Value::DInt(i32::from_le_bytes(self.array()?)),
            5 => Value::LInt(i64::from_le_bytes(self.array()?)),
            6 => Value::USInt(self.byte()?),
            7 => Value::UInt(u16::from_le_bytes(self.array()?)),
            8 => Value::UDInt(u32::from_le_bytes(self.array()?)),
            9 => Value::ULInt(u64::from_le_bytes(self.array()?)),
            10 => Value::Byte(self.byte()?),
            11 => Value::Word(u16::from_le_bytes(self.array()?)),
            12 => Value::DWord(u32::from_le_bytes(self.array()?)),
            13 => Value::LWord(u64::from_le_bytes(self.array()?)),
            14 => Value::Real(f32::from_le_bytes(self.array()?)),
            15 => Value::LReal(f64::from_le_bytes(self.array()?)),
            16 => Value::Time(Duration::from_nanos(i64::from_le_bytes(self.array()?))),
            17 => Value::LTime(Duration::from_nanos(i64::from_le_bytes(self.array()?))),
            18 => Value::String(SmolStr::new(self.text()?)),
            19 => {
                let count = usize::from(u16::from_le_bytes(self.array()?));
                let units = (0..count)
                    .map(|_| self.array().map(u16::from_le_bytes))
                    .collect::<Result<Vec<_>, _>>()?;
                Value::WString(
                    String::from_utf16(&units).map_err(|_| invalid("a WSTRING is not UTF-16"))?,
                )
            }
            20 => Value::Char(self.byte()?),
            21 => Value::WChar(u16::from_le_bytes(self.array()?)),
            22 => {
                let type_name = SmolStr::new(self.text()?);
                let value = SmolStr::new(self.text()?);
                return Ok(Expression::Enumeration(type_name, value));
            }
            code => return Err(invalid(format!("unknown literal type {code}"))),
        }))
    }
}

/// The operator of `code` in a unary node.
fn unary_op(code: u8) -> Result<UnaryOp, ExpressionError> {
    Ok(match code {
        1 => UnaryOp::Neg,
        2 => UnaryOp::Pos,
        3 => UnaryOp::Not,
        other => return Err(invalid(format!("unknown unary operator {other}"))),
    })
}

/// The code of `op` in a unary node.
fn unary_code(op: UnaryOp) -> u8 {
    match op {
        UnaryOp::Neg => 1,
        UnaryOp::Pos => 2,
        UnaryOp::Not => 3,
    }
}

/// The binary operators by their codes, from 1.
const BINARY_OPS: [BinaryOp; 17] = [
    BinaryOp::Add,
    BinaryOp::Sub,
    BinaryOp::Mul,
    BinaryOp::Div,
    BinaryOp::Mod,
    BinaryOp::Pow,
    BinaryOp::And,
    BinaryOp::Or,
    BinaryOp::AndThen,
    BinaryOp::OrElse,
    BinaryOp::Xor,
    BinaryOp::Eq,
    BinaryOp::Ne,
    BinaryOp::Lt,
    BinaryOp::Le,
    BinaryOp::Gt,
    BinaryOp::Ge,
];

/// The operator of `code` in a binary node.
fn binary_op(code: u8) -> Result<BinaryOp, ExpressionError> {
    usize::from(code)
        .checked_sub(1)
        .and_then(|index| BINARY_OPS.get(index))
        .copied()
        .ok_or_else(|| invalid(format!("unknown binary operator {code}")))
}

/// The code of `op` in a binary node.
fn binary_code(op: BinaryOp) -> u8 {
    BINARY_OPS
        .iter()
        .position(|known| *known == op)
        .and_then(|index| u8::try_from(index + 1).ok())
        .unwrap_or(0)
}

fn write_text(bytes: &mut Vec<u8>, text: &str) -> Result<(), ExpressionError> {
    let length =
        u16::try_from(text.len()).map_err(|_| invalid("a text is longer than 65,535 bytes"))?;
    bytes.extend_from_slice(&length.to_le_bytes());
    bytes.extend_from_slice(text.as_bytes());
    Ok(())
}

fn write_node(
    bytes: &mut Vec<u8>,
    expression: &Expression,
    depth: usize,
) -> Result<(), ExpressionError> {
    if depth >= MAX_DEPTH {
        return Err(invalid("the expression is nested deeper than 32 nodes"));
    }
    match expression {
        Expression::Literal(value) => {
            bytes.push(1);
            write_value(bytes, value)?;
        }
        Expression::Enumeration(type_name, value) => {
            bytes.extend_from_slice(&[1, 22]);
            write_text(bytes, type_name)?;
            write_text(bytes, value)?;
        }
        Expression::Name(name) => {
            bytes.push(2);
            write_text(bytes, name)?;
        }
        Expression::Unary(op, operand) => {
            bytes.extend_from_slice(&[3, unary_code(*op)]);
            write_node(bytes, operand, depth + 1)?;
        }
        Expression::Binary(op, left, right) => {
            bytes.extend_from_slice(&[4, binary_code(*op)]);
            write_node(bytes, left, depth + 1)?;
            write_node(bytes, right, depth + 1)?;
        }
        Expression::Index(target, indices) => {
            let count = u8::try_from(indices.len())
                .ok()
                .filter(|count| (1..=MAX_INDICES).contains(count))
                .ok_or_else(|| invalid("an index node has no or more than 8 indices"))?;
            bytes.extend_from_slice(&[5, count]);
            write_node(bytes, target, depth + 1)?;
            for index in indices {
                write_node(bytes, index, depth + 1)?;
            }
        }
        Expression::Field(target, member) => {
            bytes.push(6);
            write_text(bytes, member)?;
            write_node(bytes, target, depth + 1)?;
        }
    }
    Ok(())
}

fn write_value(bytes: &mut Vec<u8>, value: &Value) -> Result<(), ExpressionError> {
    match value {
        Value::Bool(value) => bytes.extend_from_slice(&[1, u8::from(*value)]),
        Value::SInt(value) => push_typed(bytes, 2, &value.to_le_bytes()),
        Value::Int(value) => push_typed(bytes, 3, &value.to_le_bytes()),
        Value::DInt(value) => push_typed(bytes, 4, &value.to_le_bytes()),
        Value::LInt(value) => push_typed(bytes, 5, &value.to_le_bytes()),
        Value::USInt(value) => push_typed(bytes, 6, &[*value]),
        Value::UInt(value) => push_typed(bytes, 7, &value.to_le_bytes()),
        Value::UDInt(value) => push_typed(bytes, 8, &value.to_le_bytes()),
        Value::ULInt(value) => push_typed(bytes, 9, &value.to_le_bytes()),
        Value::Byte(value) => push_typed(bytes, 10, &[*value]),
        Value::Word(value) => push_typed(bytes, 11, &value.to_le_bytes()),
        Value::DWord(value) => push_typed(bytes, 12, &value.to_le_bytes()),
        Value::LWord(value) => push_typed(bytes, 13, &value.to_le_bytes()),
        Value::Real(value) => push_typed(bytes, 14, &value.to_le_bytes()),
        Value::LReal(value) => push_typed(bytes, 15, &value.to_le_bytes()),
        Value::Time(value) => push_typed(bytes, 16, &value.as_nanos().to_le_bytes()),
        Value::LTime(value) => push_typed(bytes, 17, &value.as_nanos().to_le_bytes()),
        Value::String(text) => {
            bytes.push(18);
            write_text(bytes, text)?;
        }
        Value::WString(text) => {
            let units: Vec<u16> = text.encode_utf16().collect();
            let count = u16::try_from(units.len())
                .map_err(|_| invalid("a WSTRING is longer than 65,535 units"))?;
            bytes.push(19);
            bytes.extend_from_slice(&count.to_le_bytes());
            for unit in units {
                bytes.extend_from_slice(&unit.to_le_bytes());
            }
        }
        Value::Char(value) => push_typed(bytes, 20, &[*value]),
        Value::WChar(value) => push_typed(bytes, 21, &value.to_le_bytes()),
        other => {
            return Err(invalid(format!(
                "a literal {other:?} that version 1 cannot carry"
            )));
        }
    }
    Ok(())
}

fn push_typed(bytes: &mut Vec<u8>, code: u8, value: &[u8]) {
    bytes.push(code);
    bytes.extend_from_slice(value);
}

/// The dotted name of a chain of members over a name, `a.b.c`, which the runtime resolves as one
/// name: a member of an instance or a variable of a program instance.
fn dotted(expression: &Expression) -> Option<String> {
    match expression {
        Expression::Name(name) => Some(name.to_string()),
        Expression::Field(target, member) => Some(format!("{}.{member}", dotted(target)?)),
        _ => None,
    }
}

fn eval(
    expression: &Expression,
    names: &dyn Names,
    work: &mut u32,
) -> Result<Value, ExpressionError> {
    *work += 1;
    if *work > MAX_WORK {
        return Err(failed("the evaluation took more than 1,024 nodes"));
    }
    match expression {
        Expression::Literal(value) => Ok(value.clone()),
        Expression::Enumeration(type_name, value) => names
            .enumeration(type_name, value)
            .ok_or_else(|| invalid(format!("the program declares no {type_name}#{value}"))),
        Expression::Name(name) => names
            .read(name)
            .ok_or_else(|| invalid(format!("'{name}' names no variable"))),
        Expression::Unary(op, operand) => apply_unary(*op, eval(operand, names, work)?)
            .map_err(|error| failed(&error.to_string())),
        Expression::Binary(op, left, right) => {
            let left = eval(left, names, work)?;
            // `AND_THEN` and `OR_ELSE` leave the right operand alone when the left decides.
            match (op, &left) {
                (BinaryOp::AndThen, Value::Bool(false)) => return Ok(Value::Bool(false)),
                (BinaryOp::OrElse, Value::Bool(true)) => return Ok(Value::Bool(true)),
                _ => {}
            }
            let right = eval(right, names, work)?;
            apply_binary(*op, left, right, &DateTimeProfile::default())
                .map_err(|error| failed(&error.to_string()))
        }
        Expression::Index(target, indices) => {
            let target = eval(target, names, work)?;
            let Value::Array(array) = target else {
                return Err(invalid("an index of a value that is no array"));
            };
            let mut offset = 0_usize;
            let dimensions = array.dimensions();
            if dimensions.len() != indices.len() {
                return Err(invalid(format!(
                    "{} indices of an array of {} dimensions",
                    indices.len(),
                    dimensions.len()
                )));
            }
            for (index, (lower, upper)) in indices.iter().zip(dimensions) {
                let index = integer(&eval(index, names, work)?)?;
                if index < *lower || index > *upper {
                    return Err(failed(&format!(
                        "index {index} outside its bounds {lower}..{upper}"
                    )));
                }
                let extent = upper
                    .checked_sub(*lower)
                    .and_then(|extent| extent.checked_add(1))
                    .and_then(|extent| usize::try_from(extent).ok())
                    .ok_or_else(|| failed("the array's bounds overflow"))?;
                let position = index
                    .checked_sub(*lower)
                    .and_then(|position| usize::try_from(position).ok())
                    .ok_or_else(|| failed("the index overflows"))?;
                offset = offset
                    .checked_mul(extent)
                    .and_then(|offset| offset.checked_add(position))
                    .ok_or_else(|| failed("the index overflows"))?;
            }
            array
                .elements()
                .get(offset)
                .cloned()
                .ok_or_else(|| failed("the index lies outside the array"))
        }
        Expression::Field(target, member) => {
            // A member of an instance has no value to take it from: the runtime reads the whole
            // dotted name.
            if let Some(path) = dotted(expression)
                && let Some(value) = names.read(&path)
            {
                return Ok(value);
            }
            match eval(target, names, work)? {
                Value::Struct(value) => value
                    .fields()
                    .iter()
                    .find(|(name, _)| name.eq_ignore_ascii_case(member))
                    .map(|(_, value)| value.clone())
                    .ok_or_else(|| invalid(format!("'{member}' is no member"))),
                _ => Err(invalid(format!("'{member}' is no member of a value"))),
            }
        }
    }
}

/// The integer `value` holds, as an index.
fn integer(value: &Value) -> Result<i64, ExpressionError> {
    Ok(match value {
        Value::SInt(value) => i64::from(*value),
        Value::Int(value) => i64::from(*value),
        Value::DInt(value) => i64::from(*value),
        Value::LInt(value) => *value,
        Value::USInt(value) | Value::Byte(value) => i64::from(*value),
        Value::UInt(value) | Value::Word(value) => i64::from(*value),
        Value::UDInt(value) | Value::DWord(value) => i64::from(*value),
        Value::ULInt(value) | Value::LWord(value) => {
            i64::try_from(*value).map_err(|_| failed("the index overflows"))?
        }
        _ => return Err(invalid("an index that is no integer")),
    })
}

#[cfg(test)]
#[path = "expression/tests.rs"]
mod tests;
