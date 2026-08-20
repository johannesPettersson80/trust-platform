use smol_str::SmolStr;
use trust_hir::db::{FileId, SemanticDatabase};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::debug::SourceLocation;
use crate::value::DateTimeProfile;

use super::super::lower::{lower_expr, resolve_initializer_enum_variant};
use super::super::types::CompileError;
use super::super::util::{collect_using_directives, namespace_qualified_name};
use super::model::{CompileTimeConsts, LoweringContext};
use super::types::{lower_type_ref, qualify_with_namespaces};
use super::vars::{parse_var_decl, var_block_kind, var_block_qualifiers, VarBlockKind};

#[derive(Clone)]
struct PendingConstant {
    names: Vec<SmolStr>,
    type_ref: SyntaxNode,
    initializer: SyntaxNode,
    using: Vec<SmolStr>,
    file_index: usize,
}

pub(crate) fn predeclare_project_types(
    syntaxes: &[SyntaxNode],
    registry: &mut trust_hir::types::TypeRegistry,
) -> Result<Vec<trust_hir::TypeId>, CompileError> {
    let mut reserved = Vec::new();
    for syntax in syntaxes {
        for declaration in syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::TypeDecl)
        {
            for name_node in declaration
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
            {
                let raw = super::super::util::node_text(&name_node);
                let name = qualify_with_namespaces(&declaration, &raw);
                if registry.lookup(name.as_str()).is_some() {
                    return Err(CompileError::new(format!("duplicate type name '{name}'")));
                }
                reserved.push(registry.reserve(name));
            }
        }
    }
    Ok(reserved)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_project_global_constants(
    syntaxes: &[SyntaxNode],
    registry: &mut trust_hir::types::TypeRegistry,
    profile: DateTimeProfile,
    semantic_db: &dyn SemanticDatabase,
    file_ids: &[FileId],
    statement_locations: &mut [Vec<SourceLocation>],
    allow_partial: bool,
) -> Result<CompileTimeConsts, CompileError> {
    let mut pending = Vec::new();
    let mut constants = CompileTimeConsts::default();
    for (file_index, syntax) in syntaxes.iter().enumerate() {
        for var_block in syntax
            .descendants()
            .filter(|node| node.kind() == SyntaxKind::VarBlock)
        {
            if !matches!(var_block_kind(&var_block)?, VarBlockKind::Global)
                || !var_block_qualifiers(&var_block)?.constant
            {
                continue;
            }
            for declaration in var_block
                .children()
                .filter(|node| node.kind() == SyntaxKind::VarDecl)
            {
                let parts = parse_var_decl(&declaration)?;
                let Some(initializer) = parts.initializer else {
                    continue;
                };
                let names = parts
                    .names
                    .iter()
                    .map(|name| namespace_qualified_name(&var_block, name.as_str()))
                    .collect::<Vec<_>>();
                for name in &names {
                    constants.reserve_global(name.as_str());
                }
                pending.push(PendingConstant {
                    names,
                    type_ref: parts.type_ref,
                    initializer,
                    using: collect_using_directives(&var_block),
                    file_index,
                });
            }
        }
    }

    let mut last_error = None;
    while !pending.is_empty() {
        let mut next = Vec::new();
        let mut progress = false;
        for declaration in pending {
            let file_index = declaration.file_index;
            let mut ctx = LoweringContext {
                registry,
                profile,
                using: declaration.using.clone(),
                file_id: file_ids[file_index].0,
                semantic_db: Some(semantic_db),
                semantic_file_id: Some(file_ids[file_index]),
                statement_locations: &mut statement_locations[file_index],
                compile_time_consts: constants.clone(),
            };
            match evaluate_constant(&declaration, &mut ctx) {
                Ok(value) => {
                    for name in &declaration.names {
                        constants.insert_global(name.as_str(), value.clone());
                    }
                    progress = true;
                }
                Err(error) => {
                    last_error = Some(error);
                    next.push(declaration);
                }
            }
        }
        pending = next;
        if !progress {
            break;
        }
    }

    if !pending.is_empty() && !allow_partial {
        return Err(last_error.unwrap_or_else(|| {
            CompileError::new("unable to resolve project constant dependency graph")
        }));
    }
    Ok(constants)
}

pub(crate) fn resolve_pou_local_constants(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<(), CompileError> {
    let mut pending = Vec::new();
    for var_block in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarBlock)
    {
        let kind = var_block_kind(&var_block)?;
        if !matches!(
            kind,
            VarBlockKind::Var | VarBlockKind::Stat | VarBlockKind::Temp
        ) || !var_block_qualifiers(&var_block)?.constant
        {
            continue;
        }
        for declaration in var_block
            .children()
            .filter(|child| child.kind() == SyntaxKind::VarDecl)
        {
            let parts = parse_var_decl(&declaration)?;
            let Some(initializer) = parts.initializer else {
                continue;
            };
            for name in &parts.names {
                ctx.compile_time_consts.reserve_local(name.as_str());
            }
            pending.push(PendingConstant {
                names: parts.names,
                type_ref: parts.type_ref,
                initializer,
                using: ctx.using.clone(),
                file_index: 0,
            });
        }
    }

    let mut last_error = None;
    while !pending.is_empty() {
        let mut next = Vec::new();
        let mut progress = false;
        for declaration in pending {
            match evaluate_constant(&declaration, ctx) {
                Ok(value) => {
                    for name in &declaration.names {
                        ctx.compile_time_consts
                            .insert_local(name.as_str(), value.clone());
                    }
                    progress = true;
                }
                Err(error) => {
                    last_error = Some(error);
                    next.push(declaration);
                }
            }
        }
        pending = next;
        if !progress {
            break;
        }
    }
    if pending.is_empty() {
        Ok(())
    } else {
        Err(last_error.unwrap_or_else(|| {
            CompileError::new("unable to resolve local constant dependency graph")
        }))
    }
}

fn evaluate_constant(
    declaration: &PendingConstant,
    ctx: &mut LoweringContext<'_>,
) -> Result<crate::value::Value, CompileError> {
    let type_id = lower_type_ref(&declaration.type_ref, ctx)?;
    let expression = lower_expr(&declaration.initializer, ctx).and_then(|lowered| {
        resolve_initializer_enum_variant(&declaration.initializer, lowered, type_id, ctx)
    })?;
    ctx.eval_compile_time_const_initializer(&expression, type_id)
}

pub(crate) fn validate_project_aliases(
    type_ids: &[trust_hir::TypeId],
    registry: &trust_hir::types::TypeRegistry,
) -> Result<(), CompileError> {
    for start in type_ids {
        let mut current = *start;
        let mut visited = indexmap::IndexSet::new();
        loop {
            if !visited.insert(current) {
                let name = registry
                    .type_name(*start)
                    .unwrap_or_else(|| SmolStr::new("unknown"));
                return Err(CompileError::new(format!(
                    "cyclic type alias involving '{name}'"
                )));
            }
            match registry.get(current) {
                Some(trust_hir::Type::Alias { target, .. }) => current = *target,
                _ => break,
            }
        }
    }
    Ok(())
}
