use indexmap::IndexMap;
use smol_str::SmolStr;

use crate::io::IoAddress;
use crate::task::ProgramDef;
use crate::value::Duration;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use super::super::lower::{const_duration_from_node, const_int_from_node, lower_expr};
use super::super::types::CompileError;
use super::super::util::{
    collect_using_directives, extract_name_from_expr, is_expression_kind, namespace_qualified_name,
    node_text,
};
use super::lower_type_ref;
use super::model::{
    AccessDecl, AccessPart, AccessPath, CompileTimeConsts, ConfigInit, ConfigModel, FbTaskBinding,
    GlobalInit, LoweringContext, ProgramInstanceConfig,
};
use super::vars::{parse_var_decl, var_block_kind, var_block_qualifiers, VarBlockKind};

include!("config/entry.rs");
include!("config/globals_access.rs");
include!("config/access_path.rs");
include!("config/tasks_programs.rs");
include!("config/resolve.rs");

pub(crate) fn lower_root_global_var_blocks(
    node: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    profile: crate::value::DateTimeProfile,
    compile_time_consts: &mut CompileTimeConsts,
    file_id: u32,
    statement_locations: &mut Vec<crate::debug::SourceLocation>,
) -> Result<Vec<GlobalInit>, CompileError> {
    let using = collect_using_directives(node);
    let mut ctx = LoweringContext {
        registry,
        profile,
        using,
        file_id,
        statement_locations,
        compile_time_consts: compile_time_consts.clone(),
    };
    let globals = lower_root_global_var_blocks_in_scope(node, &mut ctx)?;
    *compile_time_consts = ctx.compile_time_consts.clone();
    Ok(globals)
}

fn lower_root_global_var_blocks_in_scope(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<GlobalInit>, CompileError> {
    let mut globals = Vec::new();
    for child in node.children() {
        match child.kind() {
            SyntaxKind::VarBlock => {
                if !matches!(var_block_kind(&child)?, VarBlockKind::Global) {
                    continue;
                }
                let saved_using = ctx.using.clone();
                ctx.using = collect_using_directives(&child);
                let lowered = lower_global_var_block(&child, ctx)?;
                ctx.using = saved_using;
                globals.extend(lowered);
            }
            SyntaxKind::Namespace => {
                globals.extend(lower_root_global_var_blocks_in_scope(&child, ctx)?);
            }
            _ => {}
        }
    }
    Ok(globals)
}
