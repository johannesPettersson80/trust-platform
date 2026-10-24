//! The portable debug expressions of specification 11 §6.13.2, version 1: the encoding, the
//! refusals and the evaluation against a stand-in's names.

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::memory::InstanceId;
use trust_runtime_core::program_model::{BinaryOp, UnaryOp};
use trust_runtime_core::value::{ArrayValue, Duration, EnumValue, Value};

use super::{decode, encode, evaluate, Expression, ExpressionError, Names};

/// Names read from a map, without regard to case, in a program that declares the enumeration
/// `Mode` of the values `Idle` (0) and `Run` (1).
struct Map(BTreeMap<String, Value>);

impl Names for Map {
    fn read(&self, name: &str) -> Option<Value> {
        self.0.get(&name.to_ascii_uppercase()).cloned()
    }

    fn enumeration(&self, type_name: &str, value: &str) -> Option<Value> {
        if !type_name.eq_ignore_ascii_case("Mode") {
            return None;
        }
        [("Idle", 0), ("Run", 1)]
            .into_iter()
            .find(|(declared, _)| declared.eq_ignore_ascii_case(value))
            .map(|(declared, number)| mode(declared, number))
    }
}

/// The value `variant` (`number`) of the enumeration `Mode`.
fn mode(variant: &str, number: i64) -> Value {
    Value::Enum(Box::new(EnumValue::from_canonical_parts(
        SmolStr::new("Mode"),
        SmolStr::new(variant),
        number,
    )))
}

fn enumeration(type_name: &str, value: &str) -> Expression {
    Expression::Enumeration(SmolStr::new(type_name), SmolStr::new(value))
}

fn names(entries: &[(&str, Value)]) -> Map {
    Map(entries
        .iter()
        .map(|(name, value)| (name.to_ascii_uppercase(), value.clone()))
        .collect())
}

fn name(text: &str) -> Expression {
    Expression::Name(SmolStr::new(text))
}

fn int(value: i16) -> Expression {
    Expression::Literal(Value::Int(value))
}

fn binary(op: BinaryOp, left: Expression, right: Expression) -> Expression {
    Expression::Binary(op, Box::new(left), Box::new(right))
}

/// Specification 11 §6.13.2, version 1: `-speed * 2 + 1` is the binary node `+` (tag 4, operator 1)
/// over the binary node `*` (4, 3) over the unary `-` (3, 1) of the name `speed` (2, length 5,
/// `speed`) and the literal `INT` 2 (1, type 3, `02 00`), and the literal `INT` 1, byte for byte;
/// the bytes decode to the same expression. Catches a node, an operator, a type code or a length in
/// another place or width.
#[test]
fn an_expression_encodes_in_prefix_order_and_decodes_back() {
    let expression = binary(
        BinaryOp::Add,
        binary(
            BinaryOp::Mul,
            Expression::Unary(UnaryOp::Neg, Box::new(name("speed"))),
            int(2),
        ),
        int(1),
    );
    let bytes = encode(&expression).expect("version 1 carries it");
    assert_eq!(
        bytes,
        vec![4, 1, 4, 3, 3, 1, 2, 5, 0, b's', b'p', b'e', b'e', b'd', 1, 3, 2, 0, 1, 3, 1, 0]
    );
    assert_eq!(decode(&bytes), Ok(expression));
}

/// Every literal type of version 1 and the index and member nodes come back from their bytes as
/// they went in. Catches a type code or a width that one side reads otherwise.
#[test]
fn every_literal_type_and_node_round_trips() {
    let literals = [
        Value::Bool(true),
        Value::SInt(-5),
        Value::Int(-300),
        Value::DInt(70_000),
        Value::LInt(-5_000_000_000),
        Value::USInt(200),
        Value::UInt(60_000),
        Value::UDInt(4_000_000_000),
        Value::ULInt(10_000_000_000_000_000_000),
        Value::Byte(0xAB),
        Value::Word(0xABCD),
        Value::DWord(0xDEAD_BEEF),
        Value::LWord(0x0123_4567_89AB_CDEF),
        Value::Real(1.5),
        Value::LReal(-2.25),
        Value::Time(Duration::from_millis(1500)),
        Value::LTime(Duration::from_nanos(-7)),
        Value::String(SmolStr::new("vat")),
        Value::WString(String::from("Wärme")),
        Value::Char(b'A'),
        Value::WChar(0x00E4),
    ];
    for literal in literals {
        let expression = Expression::Literal(literal);
        let bytes = encode(&expression).expect("version 1 carries it");
        assert_eq!(decode(&bytes), Ok(expression));
    }
    let nested = Expression::Field(
        Box::new(Expression::Index(
            Box::new(name("history")),
            vec![int(1), name("i")],
        )),
        SmolStr::new("value"),
    );
    let bytes = encode(&nested).expect("version 1 carries it");
    assert_eq!(decode(&bytes), Ok(nested));
}

/// Specification 11 §6.13.2: `Mode#Run` is a literal (tag 1) of type 22 carrying the type's name
/// and the value's name, each a 2-byte length and its bytes, and no number; the bytes decode to the
/// same node. Catches a number sent by the tool, which a project edited since its deployment could
/// get wrong, and the two names in the other order.
#[test]
fn an_enumeration_literal_travels_by_its_names() {
    let literal = enumeration("Mode", "Run");
    let bytes = encode(&literal).expect("version 1 carries it");
    assert_eq!(
        bytes,
        vec![1, 22, 4, 0, b'M', b'o', b'd', b'e', 3, 0, b'R', b'u', b'n']
    );
    assert_eq!(decode(&bytes), Ok(literal));
}

/// Specification 11 §6.13.2: bytes that are truncated or longer than their expression, an unknown
/// tag, operator or type code, a `BOOL` other than 0 or 1, an index node of no indices, a text that
/// is not UTF-8, more than 4,096 bytes and a nesting deeper than 32 nodes are each `expr_invalid`.
/// Catches a decoder that reads past its bytes, ignores what follows, or takes any of these.
#[test]
fn malformed_bytes_are_refused_as_invalid() {
    let refused: [&[u8]; 9] = [
        &[4, 1, 1, 3, 2],
        &[1, 3, 2, 0, 9],
        &[7],
        &[4, 18, 1, 3, 1, 0, 1, 3, 1, 0],
        &[1, 99],
        &[1, 1, 2],
        &[5, 0, 2, 1, 0, b'a'],
        &[2, 1, 0, 0xFF],
        &[1, 3],
    ];
    for bytes in refused {
        assert!(
            matches!(decode(bytes), Err(ExpressionError::Invalid(_))),
            "{bytes:?}"
        );
    }
    assert!(matches!(
        decode(&vec![0; 4097]),
        Err(ExpressionError::Invalid(_))
    ));
    let mut deep = Vec::new();
    for _ in 0..32 {
        deep.extend_from_slice(&[3, 1]);
    }
    deep.extend_from_slice(&[1, 3, 1, 0]);
    assert!(matches!(decode(&deep), Err(ExpressionError::Invalid(_))));
}

/// The operators compute as the runtime's statements compute: with `speed` 20, `-speed * 2 + 1` is
/// -39, `speed > 10` is TRUE, and `NOT (speed = 20)` is FALSE; with `mode` the value `Run` (1) of
/// `Mode`, `mode = Mode#Run` is TRUE and `mode = Mode#Idle` (0) FALSE. Catches an operator that
/// computes another operation or reads another name, and an enumeration literal that never equals
/// its variable.
#[test]
fn the_operators_compute_as_statements_compute() {
    let names = names(&[("Speed", Value::Int(20)), ("mode", mode("Run", 1))]);
    assert_eq!(
        evaluate(
            &binary(BinaryOp::Eq, name("mode"), enumeration("MODE", "run")),
            &names
        ),
        Ok(Value::Bool(true))
    );
    assert_eq!(
        evaluate(
            &binary(BinaryOp::Eq, name("mode"), enumeration("Mode", "Idle")),
            &names
        ),
        Ok(Value::Bool(false))
    );
    let arithmetic = binary(
        BinaryOp::Add,
        binary(
            BinaryOp::Mul,
            Expression::Unary(UnaryOp::Neg, Box::new(name("speed"))),
            int(2),
        ),
        int(1),
    );
    assert_eq!(evaluate(&arithmetic, &names), Ok(Value::Int(-39)));
    assert_eq!(
        evaluate(&binary(BinaryOp::Gt, name("SPEED"), int(10)), &names),
        Ok(Value::Bool(true))
    );
    let negated = Expression::Unary(
        UnaryOp::Not,
        Box::new(binary(BinaryOp::Eq, name("speed"), int(20))),
    );
    assert_eq!(evaluate(&negated, &names), Ok(Value::Bool(false)));
}

/// IEC 61131-3 §7.3.2: `AND_THEN` evaluates its right operand only when the left is TRUE, and
/// `OR_ELSE` only when it is FALSE; with `ready` FALSE, `ready AND_THEN missing` is FALSE without
/// reading `missing`, and `NOT ready OR_ELSE missing` is TRUE. Catches a short circuit that still
/// evaluates its right operand, which names no variable.
#[test]
fn and_then_and_or_else_leave_the_right_operand_alone() {
    let names = names(&[("ready", Value::Bool(false))]);
    assert_eq!(
        evaluate(
            &binary(BinaryOp::AndThen, name("ready"), name("missing")),
            &names
        ),
        Ok(Value::Bool(false))
    );
    let not_ready = Expression::Unary(UnaryOp::Not, Box::new(name("ready")));
    assert_eq!(
        evaluate(
            &binary(BinaryOp::OrElse, not_ready, name("missing")),
            &names
        ),
        Ok(Value::Bool(true))
    );
}

/// An index node reads an array's element by its declared bounds and a member node a structure's
/// member, and a member of an instance is read as the whole dotted name: `history[2]` of
/// `ARRAY[1..3] OF INT := [7, 8, 9]` is 8, `grid[1, 0]` of `ARRAY[0..1, 0..1]` holding 1, 2, 3, 4
/// is 3, and `timer.Q` is the value the runtime gives `timer.Q`. Catches an index from zero, a
/// row-major order turned around, and a member of an instance that is never read.
#[test]
fn indices_and_members_reach_their_elements() {
    let history = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(7), Value::Int(8), Value::Int(9)],
        vec![(1, 3)],
    )));
    let grid = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1), Value::Int(2), Value::Int(3), Value::Int(4)],
        vec![(0, 1), (0, 1)],
    )));
    let names = names(&[
        ("history", history),
        ("grid", grid),
        ("timer", Value::Instance(InstanceId(4))),
        ("timer.Q", Value::Bool(true)),
    ]);
    let element = Expression::Index(Box::new(name("history")), vec![int(2)]);
    assert_eq!(evaluate(&element, &names), Ok(Value::Int(8)));
    let cell = Expression::Index(Box::new(name("grid")), vec![int(1), int(0)]);
    assert_eq!(evaluate(&cell, &names), Ok(Value::Int(3)));
    let output = Expression::Field(Box::new(name("timer")), SmolStr::new("Q"));
    assert_eq!(evaluate(&output, &names), Ok(Value::Bool(true)));
}

/// A balanced tree of `+` over the literal 1, `levels` levels deep: 2^levels - 1 nodes.
fn sums(levels: u32) -> Expression {
    if levels == 1 {
        return int(1);
    }
    binary(BinaryOp::Add, sums(levels - 1), sums(levels - 1))
}

/// Specification 11 §6.13.2: a name that names no variable, an enumeration value or an
/// enumeration the program does not declare (`Mode#Stopped`, `Colour#Red`) and a result that is an
/// instance are `expr_invalid`; a division by zero, an index outside its bounds and more than 1,024
/// nodes evaluated are `expr_evaluation_failed`. Catches a refusal under the other code, or a value
/// made up for any of them.
#[test]
fn evaluation_refusals_carry_their_codes() {
    let history = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(7), Value::Int(8), Value::Int(9)],
        vec![(1, 3)],
    )));
    let names = names(&[
        ("speed", Value::Int(20)),
        ("history", history),
        ("timer", Value::Instance(InstanceId(4))),
    ]);
    let code = |expression: &Expression| evaluate(expression, &names).map_err(|error| error.code());
    assert_eq!(code(&name("missing")), Err("expr_invalid"));
    assert_eq!(code(&enumeration("Mode", "Stopped")), Err("expr_invalid"));
    assert_eq!(code(&enumeration("Colour", "Red")), Err("expr_invalid"));
    assert_eq!(code(&name("timer")), Err("expr_invalid"));
    assert_eq!(
        code(&binary(BinaryOp::Div, name("speed"), int(0))),
        Err("expr_evaluation_failed")
    );
    assert_eq!(
        code(&Expression::Index(Box::new(name("history")), vec![int(4)])),
        Err("expr_evaluation_failed")
    );
    // A balanced tree of `+` eleven levels deep holds 2,047 nodes.
    assert_eq!(code(&sums(11)), Err("expr_evaluation_failed"));
}
