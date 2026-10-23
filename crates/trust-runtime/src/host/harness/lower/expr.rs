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

include!("expr/lowering.rs");
include!("expr/literals.rs");
include!("expr/integer_literals.rs");
include!("expr/aggregate_initializers.rs");
include!("expr/call_context.rs");
include!("expr/standard_bistable_names.rs");
include!("expr/storage_context.rs");
include!("expr/operators.rs");
include!("expr/constants.rs");
