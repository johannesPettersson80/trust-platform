use smol_str::SmolStr;

use crate::datetime::{
    days_from_civil, days_to_ticks, nanos_to_ticks, DateTimeCalcError, DivisionMode, NANOS_PER_DAY,
};
use crate::harness::initializer::coerce_evaluated_initializer_value;
use crate::program_model::{ArgValue, BinaryOp, CallArg, Expr, LValue, UnaryOp};
use crate::stdlib::fbs::{builtin_kind, BuiltinFbKind};
use crate::value::{
    DateTimeProfile, DateTimeValue, DateValue, Duration, EnumValue, LDateTimeValue, LDateValue,
    LTimeOfDayValue, TimeOfDayValue, Value,
};
use trust_hir::symbols::{ScopeId, ScopeKind, SymbolId, SymbolKind, SymbolTable, UsingResolution};
use trust_hir::types::TypeRegistry;
use trust_hir::{Type, TypeId};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::util::{direct_expr_children, first_expr_child, is_expression_kind, node_text};
use super::super::{
    coerce_value_to_type, lower_type_ref, resolve_type_name, CompileError, LoweringContext,
};

/// The literal parsers that the compiled-program generator shares with this lowering
/// (specification 44 §6.6), so that the same text gives the same value in both.
pub(crate) mod literal_text {
    use super::{CompileError, Value};

    /// The magnitude of an integer literal, with its base prefix and separators.
    pub(crate) fn int(text: &str) -> Result<u64, CompileError> {
        super::parse_int_literal(text)
    }

    /// A real literal in binary64.
    pub(crate) fn real(text: &str) -> Result<f64, CompileError> {
        super::parse_real_literal(text)
    }

    /// A real literal rounded once to binary32, as `REAL#` rounds it.
    pub(crate) fn real32(text: &str) -> Result<f32, CompileError> {
        super::parse_real_literal_f32(text)
    }

    /// The text of a string literal, its `$` escapes resolved.
    pub(crate) fn string(text: &str, wide: bool) -> Result<String, CompileError> {
        super::parse_string_literal(text, wide)
    }

    /// A duration literal.
    pub(crate) fn time(text: &str) -> Result<Value, CompileError> {
        super::parse_time_literal(text)
    }
}

include!("expr/lowering.rs");
include!("expr/literals.rs");
include!("expr/integer_literals.rs");
include!("expr/aggregate_initializers.rs");
include!("expr/call_context.rs");
include!("expr/standard_bistable_names.rs");
include!("expr/storage_context.rs");
include!("expr/operators.rs");
include!("expr/constants.rs");
