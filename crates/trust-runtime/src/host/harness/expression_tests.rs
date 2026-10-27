//! The tool's compile of debug expressions into version 1 (specification 11 §6.13.2).

use super::*;
use crate::program_model::{BinaryOp, UnaryOp};
use crate::value::Value;
use trust_runtime_services::debug::expression::decode;

/// The expression the compiled bytes of `text` decode to, in a project that declares the
/// enumeration `Mode` of `Idle` (0) and `Run` (1).
fn compile(text: &str) -> Result<Expression, CompileError> {
    let mut registry = TypeRegistry::new();
    registry.register_enum(
        "Mode",
        trust_hir::TypeId::INT,
        vec![("Idle".into(), 0), ("Run".into(), 1)],
    );
    let bytes = compile_debug_expression(text, &mut registry, DateTimeProfile::default(), &[])?;
    Ok(decode(&bytes).expect("the tool's bytes decode"))
}

fn name(text: &str) -> Box<Expression> {
    Box::new(Expression::Name(SmolStr::new(text)))
}

fn int(value: i16) -> Box<Expression> {
    Box::new(Expression::Literal(Value::Int(value)))
}

/// Specification 11 §6.13.2: the tool compiles what version 1 can say into its nodes, as the
/// debugger's parser reads the text: `speed * INT#2 > INT#10` is `>` over `*` over the name and
/// the literal, `NOT ready` the unary `NOT` of the name, and `history[INT#2].value` the member of
/// the element. Catches an operator, a name or a literal that the compile changes or drops, and a
/// member or an index in the other order.
#[test]
fn a_watch_compiles_into_its_nodes() {
    assert_eq!(
        compile("speed * INT#2 > INT#10").expect("it compiles"),
        Expression::Binary(
            BinaryOp::Gt,
            Box::new(Expression::Binary(BinaryOp::Mul, name("speed"), int(2))),
            int(10),
        )
    );
    assert_eq!(
        compile("NOT ready").expect("it compiles"),
        Expression::Unary(UnaryOp::Not, name("ready"))
    );
    assert_eq!(
        compile("history[INT#2].value").expect("it compiles"),
        Expression::Field(
            Box::new(Expression::Index(name("history"), vec![*int(2)])),
            SmolStr::new("value"),
        )
    );
}

/// Specification 11 §6.13.2: `mode = Mode#Run` compiles into `=` over the name and the
/// enumeration literal by its type's and its value's names, as the project declares them. Catches a
/// literal sent as its number, which the runtime would not compare with its own program's value.
#[test]
fn an_enumeration_literal_compiles_by_its_names() {
    assert_eq!(
        compile("mode = Mode#Run").expect("it compiles"),
        Expression::Binary(
            BinaryOp::Eq,
            name("mode"),
            Box::new(Expression::Enumeration(
                SmolStr::new("Mode"),
                SmolStr::new("Run")
            )),
        )
    );
}

/// Specification 11 §6.13.2: an expression with a part that version 1 cannot say is refused by the
/// tool before any request: `ABS(speed)`, a pure call the debugger's parser takes, is refused by
/// the compile, and `REF(speed)` already by the parser, which takes no reference in a watch.
/// Catches a compile that sends a part a runtime would refuse, or drops it and sends something
/// else.
#[test]
fn a_part_version_1_cannot_say_is_refused() {
    let call = compile("ABS(speed)").expect_err("a call is refused");
    assert!(
        call.to_string()
            .contains("a call cannot be compiled into a debug expression of version 1"),
        "{call}"
    );
    let reference = compile("REF(speed)").expect_err("a reference is refused");
    assert!(
        reference.to_string().contains("side-effect free"),
        "{reference}"
    );
}
