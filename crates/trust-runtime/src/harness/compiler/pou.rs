use smol_str::SmolStr;
use trust_hir::db::FileId;
use trust_hir::semantic::{DeclarationCatalog, DeclarationKind};
use trust_hir::symbols::ParamDirection;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::io::IoAddress;
use crate::program_model::{
    ClassDef, FunctionBlockBase, FunctionBlockDef, FunctionDef, InterfaceDef, MethodDef, Param,
    VarDef,
};
use crate::task::ProgramDef;

use super::super::lower::{lower_expr, lower_stmt_list, resolve_initializer_enum_variant};
use super::super::types::CompileError;
use super::super::util::{collect_using_directives, namespace_qualified_name, node_text};
use super::model::{GlobalInit, LoweredProgram, LoweringContext, LoweringInputs, ProgramVars};
use super::vars::{parse_var_decl, var_block_kind, var_block_qualifiers, VarBlockKind};
use super::{lower_type_ref, resolve_named_type};

include!("pou/entry_points.rs");
include!("pou/node_lowering.rs");
include!("pou/names.rs");
include!("pou/program_vars.rs");
include!("pou/function_vars.rs");
include!("pou/class_vars.rs");
include!("pou/function_block_vars.rs");
