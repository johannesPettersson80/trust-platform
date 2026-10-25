//! The front of the build (specification 44 §3): the sources checked and lowered to their
//! declarations, the declarations view the tools read, the debug expressions the tools compile, and
//! the test harness that runs a compiled program's cycles.

#![allow(missing_docs)]

mod api;
mod build;
mod coerce;
mod compiler;
mod config;
mod expression;
#[allow(clippy::module_inception)]
mod harness;
pub(crate) mod initializer;
mod io;
mod lower;
mod parse;
mod places;
mod protocol;
mod types;
mod util;

pub use crate::boundary::{BoundaryEntry, BoundaryError};
pub use api::{CompileSession, Declarations};
#[cfg(test)]
pub(crate) use build::lower_application;
pub(crate) use build::lower_application_and_frontend;
pub(crate) use build::LoweredApplication;
pub use coerce::{coerce_initializer_value_to_type, coerce_value_to_type};
pub use expression::{compile_debug_expression, project_expression_types};
pub use harness::TestHarness;
pub(crate) use lower::{import_hir_type_to_runtime, literal_text};
pub use parse::{parse_debug_expression, parse_debug_lvalue};
pub use protocol::{
    decode_json_value, encode_json_value, HarnessAutomation, HarnessAutomationError,
    HarnessLoadSummary, HarnessRunUntilSummary, HarnessStateSummary, HarnessValueSnapshot,
    HarnessWatchSnapshot,
};
pub use types::{CompileError, CycleResult, SourceFile};

pub(crate) use compiler::{
    class_type_name, function_block_type_name, AccessDecl, AccessPart, AccessPath, ConfigInit,
    ConfigModel, GlobalInit, NamedAccess, ProgramInstanceConfig, ResolvedAccess,
    WildcardRequirement,
};
use compiler::{
    collect_edge_declarations, contains_textual_action, interface_type_name, lower_classes,
    lower_configuration, lower_function_blocks, lower_functions, lower_interfaces, lower_programs,
    lower_type_decls, lower_type_ref, predeclare_classes, predeclare_function_blocks,
    predeclare_interfaces, predeclare_project_types, resolve_program_type_name,
    resolve_project_global_constants, resolve_type_name, validate_project_aliases, LoweringContext,
};
use lower::lower_expr;
