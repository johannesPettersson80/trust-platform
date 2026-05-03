//! Test harness for driving runtime cycles.

#![allow(missing_docs)]

mod api;
mod build;
mod coerce;
mod compiler;
mod config;
#[allow(clippy::module_inception)]
mod harness;
pub(crate) mod initializer;
mod io;
mod lower;
mod parse;
mod protocol;
mod types;
mod util;

pub use crate::boundary::{BoundaryEntry, BoundaryError};
pub use api::{
    bytecode_bytes_from_source, bytecode_bytes_from_source_with_path, bytecode_bytes_from_sources,
    bytecode_bytes_from_sources_with_paths, bytecode_module_from_source,
    bytecode_module_from_source_with_path, bytecode_module_from_sources,
    bytecode_module_from_sources_with_paths, CompileSession,
};
pub use coerce::{coerce_initializer_value_to_type, coerce_value_to_type};
pub use harness::TestHarness;
pub use parse::{parse_debug_expression, parse_debug_lvalue};
pub use protocol::{
    decode_json_value, encode_json_value, HarnessAutomation, HarnessAutomationError,
    HarnessLoadSummary, HarnessRunUntilSummary, HarnessStateSummary, HarnessValueSnapshot,
    HarnessWatchSnapshot,
};
pub use types::{CompileError, CycleResult, SourceFile};

use compiler::{
    class_type_name, function_block_type_name, interface_type_name, lower_classes,
    lower_configuration, lower_function_blocks, lower_functions, lower_interfaces, lower_programs,
    lower_type_decls, lower_type_ref, predeclare_classes, predeclare_function_blocks,
    predeclare_interfaces, resolve_program_type_name, resolve_type_name, LoweringContext,
};
use compiler::{
    AccessDecl, AccessPart, AccessPath, ConfigInit, GlobalInit, ProgramInstanceConfig,
    ResolvedAccess, WildcardRequirement,
};
use lower::lower_expr;
