//! The tool's side of the portable debug expressions (specification 11 §6.13.2, version 1): a
//! watch, a breakpoint condition, a logpoint fragment or a `debug.evaluate`, parsed from ST text as
//! the runtime's debugger parses it, compiled into the form that every runtime evaluates without
//! parsing ST text.

use smol_str::SmolStr;
use trust_hir::types::TypeRegistry;
use trust_runtime_services::debug::expression::{encode, Expression};

use crate::program_model::Expr;
use crate::value::{DateTimeProfile, Value};

use super::{parse_debug_expression, CompileError, SourceFile};

/// The compiled form of version 1 of the debug expression `text`, whose names the runtime resolves
/// where it evaluates.
///
/// # Errors
///
/// The error of text that does not parse as an expression, and of an expression with a part that
/// version 1 cannot say: a call, a reference, `THIS`, `SUPER`, `SIZEOF` or an initializer.
pub fn compile_debug_expression(
    text: &str,
    registry: &mut TypeRegistry,
    profile: DateTimeProfile,
    using: &[SmolStr],
) -> Result<Vec<u8>, CompileError> {
    let expr = parse_debug_expression(text, registry, profile, using)?;
    let expression = portable(&expr)?;
    encode(&expression).map_err(|error| CompileError::new(error.detail()))
}

/// The types and the date and time profile of the project of `sources`, from its declarations
/// alone (no statement is lowered), with which a tool compiles the debug expressions of a runtime
/// that runs the project compiled.
///
/// # Errors
///
/// The errors of sources that do not check.
pub fn project_expression_types(
    sources: &[SourceFile],
) -> Result<(TypeRegistry, DateTimeProfile), CompileError> {
    let (lowered, _) = super::build::lower_application_and_frontend(
        sources,
        true,
        crate::value::StorageAbi::UniformTemporal64,
        false,
    )?;
    Ok((
        lowered.builder.registry().clone(),
        lowered.builder.profile(),
    ))
}

/// The expression of version 1 that says what `expr` says.
fn portable(expr: &Expr) -> Result<Expression, CompileError> {
    Ok(match expr {
        // The runtime takes an enumeration value's number from the program it runs.
        Expr::Literal(Value::Enum(value)) => {
            Expression::Enumeration(value.type_name().clone(), value.variant_name().clone())
        }
        Expr::Literal(value) => Expression::Literal(value.clone()),
        Expr::Name(name) => Expression::Name(name.clone()),
        Expr::Unary { op, expr } => Expression::Unary(*op, Box::new(portable(expr)?)),
        Expr::Binary { op, left, right } => {
            Expression::Binary(*op, Box::new(portable(left)?), Box::new(portable(right)?))
        }
        Expr::Index { target, indices } => Expression::Index(
            Box::new(portable(target)?),
            indices.iter().map(portable).collect::<Result<_, _>>()?,
        ),
        Expr::Field { target, field } => {
            Expression::Field(Box::new(portable(target)?), field.clone())
        }
        Expr::Call { .. } => return Err(unsaid("a call")),
        Expr::Ref(_) | Expr::Deref(_) => return Err(unsaid("a reference")),
        Expr::This => return Err(unsaid("THIS")),
        Expr::Super => return Err(unsaid("SUPER")),
        Expr::SizeOf(_) => return Err(unsaid("SIZEOF")),
        Expr::ArrayInitializer(_) | Expr::StructInitializer(_) => {
            return Err(unsaid("an initializer"));
        }
    })
}

/// The refusal of `part`, which version 1 cannot say.
fn unsaid(part: &str) -> CompileError {
    CompileError::new(format!(
        "{part} cannot be compiled into a debug expression of version 1"
    ))
}

#[cfg(test)]
#[path = "expression_tests.rs"]
mod tests;
