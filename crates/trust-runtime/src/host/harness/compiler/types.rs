use smol_str::SmolStr;
use trust_hir::db::FileId;
use trust_hir::semantic::{DeclarationCatalog, DeclarationKind};
use trust_hir::{Type, TypeId};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::debug::SourceLocation;
use crate::program_model::InitializerCatalog;
use crate::value::DateTimeProfile;

use super::super::lower::{
    const_int_from_node, lower_expr, parse_subrange, resolve_initializer_enum_variant,
};
use super::super::types::CompileError;
use super::super::util::{
    builtin_type_name, collect_using_directives, is_expression_kind, node_text,
};
use super::model::LoweringContext;
use super::vars::parse_var_decl;

#[allow(clippy::too_many_arguments)]
pub(crate) fn lower_type_decls(
    syntax: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    initializer_catalog: &mut InitializerCatalog,
    profile: DateTimeProfile,
    semantic_db: &dyn trust_hir::db::SemanticDatabase,
    semantic_file_id: trust_hir::db::FileId,
    file_id: u32,
    statement_locations: &mut Vec<SourceLocation>,
) -> Result<(), CompileError> {
    for type_decl in syntax
        .descendants()
        .filter(|child| child.kind() == SyntaxKind::TypeDecl)
    {
        lower_type_decl_node(
            &type_decl,
            registry,
            initializer_catalog,
            profile,
            semantic_db,
            semantic_file_id,
            file_id,
            statement_locations,
        )?;
    }
    Ok(())
}

pub(crate) fn predeclare_function_blocks(
    catalog: &DeclarationCatalog,
    file_id: FileId,
    registry: &mut trust_hir::types::TypeRegistry,
) -> Result<(), CompileError> {
    for name in catalog_pou_names(catalog, file_id, DeclarationKind::FunctionBlock) {
        if registry.lookup(&name).is_some() {
            return Err(CompileError::new(format!(
                "duplicate FUNCTION_BLOCK name '{name}'"
            )));
        }
        registry.register(name.clone(), Type::FunctionBlock { name });
    }
    Ok(())
}

pub(crate) fn predeclare_classes(
    catalog: &DeclarationCatalog,
    file_id: FileId,
    registry: &mut trust_hir::types::TypeRegistry,
) -> Result<(), CompileError> {
    for name in catalog_pou_names(catalog, file_id, DeclarationKind::Class) {
        if registry.lookup(&name).is_some() {
            return Err(CompileError::new(format!("duplicate CLASS name '{name}'")));
        }
        registry.register(name.clone(), Type::Class { name });
    }
    Ok(())
}

pub(crate) fn predeclare_interfaces(
    catalog: &DeclarationCatalog,
    file_id: FileId,
    registry: &mut trust_hir::types::TypeRegistry,
) -> Result<(), CompileError> {
    for name in catalog_pou_names(catalog, file_id, DeclarationKind::Interface) {
        if registry.lookup(&name).is_some() {
            return Err(CompileError::new(format!(
                "duplicate INTERFACE name '{name}'"
            )));
        }
        registry.register(name.clone(), Type::Interface { name });
    }
    Ok(())
}

fn catalog_pou_names(
    catalog: &DeclarationCatalog,
    file_id: FileId,
    kind: DeclarationKind,
) -> Vec<SmolStr> {
    catalog
        .entries()
        .iter()
        .filter(|entry| entry.source().file_id() == file_id && entry.kind() == kind)
        .map(|entry| SmolStr::new(entry.qualified_name().display()))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn lower_type_decl_node(
    node: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    initializer_catalog: &mut InitializerCatalog,
    profile: DateTimeProfile,
    semantic_db: &dyn trust_hir::db::SemanticDatabase,
    semantic_file_id: trust_hir::db::FileId,
    file_id: u32,
    statement_locations: &mut Vec<SourceLocation>,
) -> Result<(), CompileError> {
    let using = collect_using_directives(node);
    let mut ctx = LoweringContext {
        registry,
        profile,
        using,
        file_id,
        semantic_db: Some(semantic_db),
        semantic_file_id: Some(semantic_file_id),
        statement_locations,
        compile_time_consts: Default::default(),
    };
    let mut pending_name: Option<SmolStr> = None;
    let mut last_type_id: Option<TypeId> = None;
    for child in node.children() {
        match child.kind() {
            SyntaxKind::Name => {
                let raw = node_text(&child);
                pending_name = Some(qualify_with_namespaces(node, &raw));
            }
            SyntaxKind::StructDef => {
                let name = pending_name
                    .take()
                    .ok_or_else(|| CompileError::new("missing type name"))?;
                if ctx.registry.lookup(name.as_ref()).is_some() {
                    return Err(CompileError::new(format!("duplicate type name '{name}'")));
                }
                let type_id = ctx.registry.reserve(name.clone());
                let fields = lower_struct_def(&child, &mut ctx, initializer_catalog)?;
                ctx.registry
                    .replace(type_id, trust_hir::Type::Struct { name, fields });
                last_type_id = Some(type_id);
            }
            SyntaxKind::UnionDef => {
                let name = pending_name
                    .take()
                    .ok_or_else(|| CompileError::new("missing type name"))?;
                if ctx.registry.lookup(name.as_ref()).is_some() {
                    return Err(CompileError::new(format!("duplicate type name '{name}'")));
                }
                let type_id = ctx.registry.reserve(name.clone());
                let fields = lower_struct_def(&child, &mut ctx, initializer_catalog)?;
                let variants = fields
                    .into_iter()
                    .map(|field| trust_hir::types::UnionVariant {
                        name: field.name,
                        type_id: field.type_id,
                        address: field.address,
                        default_initializer: field.default_initializer,
                    })
                    .collect();
                ctx.registry
                    .replace(type_id, trust_hir::Type::Union { name, variants });
                last_type_id = Some(type_id);
            }
            SyntaxKind::EnumDef => {
                let name = pending_name
                    .take()
                    .ok_or_else(|| CompileError::new("missing type name"))?;
                if ctx.registry.lookup(name.as_ref()).is_some() {
                    return Err(CompileError::new(format!("duplicate type name '{name}'")));
                }
                let (base, values) = lower_enum_def(&child, &mut ctx)?;
                last_type_id = Some(ctx.registry.register_enum(name, base, values));
            }
            SyntaxKind::ArrayType => {
                let name = pending_name
                    .take()
                    .ok_or_else(|| CompileError::new("missing type name"))?;
                if ctx.registry.lookup(name.as_ref()).is_some() {
                    return Err(CompileError::new(format!("duplicate type name '{name}'")));
                }
                let target = lower_array_type_node(&child, &mut ctx)?;
                last_type_id = Some(ctx.registry.register(
                    name.clone(),
                    trust_hir::Type::Alias {
                        name: name.clone(),
                        target,
                    },
                ));
            }
            SyntaxKind::TypeRef => {
                let name = pending_name
                    .take()
                    .ok_or_else(|| CompileError::new("missing type name"))?;
                if ctx.registry.lookup(name.as_ref()).is_some() {
                    return Err(CompileError::new(format!("duplicate type name '{name}'")));
                }
                let target = lower_type_ref(&child, &mut ctx)?;
                last_type_id = Some(ctx.registry.register(
                    name.clone(),
                    trust_hir::Type::Alias {
                        name: name.clone(),
                        target,
                    },
                ));
            }
            kind if is_expression_kind(kind) => {
                let Some(type_id) = last_type_id else {
                    continue;
                };
                let expr = lower_expr(&child, &mut ctx).and_then(|lowered| {
                    resolve_initializer_enum_variant(&child, lowered, type_id, &ctx)
                })?;
                let initializer_id = initializer_catalog.insert(expr);
                initializer_catalog.set_type_default(type_id, initializer_id);
            }
            _ => {}
        }
    }
    Ok(())
}

fn lower_struct_def(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    initializer_catalog: &mut InitializerCatalog,
) -> Result<Vec<trust_hir::types::StructField>, CompileError> {
    let mut fields = Vec::new();
    for var_decl in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarDecl)
    {
        let parts = parse_var_decl(&var_decl)?;
        let type_id = lower_type_ref(&parts.type_ref, ctx)?;
        let default_initializer = parts
            .initializer
            .as_ref()
            .map(|expr| {
                lower_expr(expr, ctx)
                    .and_then(|lowered| {
                        resolve_initializer_enum_variant(expr, lowered, type_id, ctx)
                    })
                    .map(|lowered| initializer_catalog.insert(lowered))
            })
            .transpose()?;
        for name in parts.names {
            fields.push(trust_hir::types::StructField {
                name,
                type_id,
                address: parts.address.clone(),
                default_initializer,
            });
        }
    }
    Ok(fields)
}

fn lower_enum_def(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<(TypeId, Vec<(SmolStr, i64)>), CompileError> {
    let mut base_type = None;
    for child in node.children() {
        if child.kind() == SyntaxKind::TypeRef {
            base_type = Some(lower_type_ref(&child, ctx)?);
            break;
        }
    }
    let base = base_type.unwrap_or(TypeId::INT);
    let mut values = Vec::new();
    let mut next_value = 0i64;
    for enum_value in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::EnumValue)
    {
        let name_node = enum_value
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .ok_or_else(|| CompileError::new("missing enum value name"))?;
        let name = node_text(&name_node);
        let assigned_expr = enum_value
            .children()
            .find(|child| is_expression_kind(child.kind()));
        let value = if let Some(expr) = assigned_expr {
            let value = const_int_from_node(&expr, ctx)?;
            next_value = value.saturating_add(1);
            value
        } else {
            let value = next_value;
            next_value = next_value.saturating_add(1);
            value
        };
        values.push((name.into(), value));
    }
    Ok((base, values))
}

fn lower_array_type_node(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<TypeId, CompileError> {
    let mut dimensions = Vec::new();
    for range in node
        .children()
        .filter(|child| child.kind() == SyntaxKind::Subrange)
    {
        dimensions.push(parse_subrange(&range, ctx)?);
    }
    let element = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
        .ok_or_else(|| CompileError::new("missing array element type"))?;
    let element_id = lower_type_ref(&element, ctx)?;
    Ok(ctx.registry.register_array(element_id, dimensions))
}

pub(crate) fn qualify_with_namespaces(node: &SyntaxNode, name: &str) -> SmolStr {
    let mut parts = vec![name.to_string()];
    for ancestor in node.ancestors() {
        if ancestor.kind() != SyntaxKind::Namespace {
            continue;
        }
        if let Some(ns_name) = ancestor
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
        {
            parts.push(node_text(&ns_name));
        }
    }
    parts.reverse();
    parts.join(".").into()
}

pub(crate) fn lower_type_ref(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<TypeId, CompileError> {
    let mut type_name = None;
    let mut subrange = None;

    for child in node.children() {
        match child.kind() {
            SyntaxKind::ReferenceType => {
                let inner = child
                    .children()
                    .find(|n| n.kind() == SyntaxKind::TypeRef)
                    .ok_or_else(|| CompileError::new("missing REF_TO target type"))?;
                let target = lower_type_ref(&inner, ctx)?;
                return Ok(ctx.registry.register_reference(target));
            }
            SyntaxKind::PointerType => {
                let inner = child
                    .children()
                    .find(|n| n.kind() == SyntaxKind::TypeRef)
                    .ok_or_else(|| CompileError::new("missing POINTER TO target type"))?;
                let target = lower_type_ref(&inner, ctx)?;
                return Ok(ctx.registry.register_pointer(target));
            }
            SyntaxKind::StringType => {
                let is_wide = child
                    .children_with_tokens()
                    .filter_map(|e| e.into_token())
                    .any(|token| token.kind() == SyntaxKind::KwWString);
                let length_expr = child.children().find(|n| is_expression_kind(n.kind()));
                if let Some(expr) = length_expr {
                    let len = const_int_from_node(&expr, ctx)?;
                    let len = u32::try_from(len).map_err(|_| {
                        CompileError::new("STRING length must be a positive integer")
                    })?;
                    if is_wide {
                        return Ok(ctx.registry.register_wstring_with_length(len));
                    }
                    return Ok(ctx.registry.register_string_with_length(len));
                }
                let name = if is_wide { "WSTRING" } else { "STRING" };
                return ctx
                    .registry
                    .lookup(name)
                    .ok_or_else(|| CompileError::new("unknown STRING type"));
            }
            SyntaxKind::ArrayType => {
                let mut dimensions = Vec::new();
                for range in child
                    .children()
                    .filter(|n| n.kind() == SyntaxKind::Subrange)
                {
                    dimensions.push(parse_subrange(&range, ctx)?);
                }
                let element = child
                    .children()
                    .find(|n| n.kind() == SyntaxKind::TypeRef)
                    .ok_or_else(|| CompileError::new("missing array element type"))?;
                let element_id = lower_type_ref(&element, ctx)?;
                return Ok(ctx.registry.register_array(element_id, dimensions));
            }
            SyntaxKind::Name | SyntaxKind::QualifiedName => {
                type_name = Some(node_text(&child));
            }
            SyntaxKind::Subrange => {
                subrange = Some(parse_subrange(&child, ctx)?);
            }
            _ => {}
        }
    }

    if type_name.is_none() {
        for element in node.children_with_tokens() {
            let token = match element.into_token() {
                Some(token) => token,
                None => continue,
            };
            if let Some(name) = builtin_type_name(token.kind()) {
                type_name = Some(name.to_string());
                break;
            }
        }
    }

    let Some(name) = type_name else {
        return Err(CompileError::new("unsupported type reference"));
    };
    let base_id = resolve_type_name(&name, ctx)?;
    if let Some((lower, upper)) = subrange {
        let type_name = ctx
            .registry
            .type_name(base_id)
            .unwrap_or_else(|| SmolStr::new("UNKNOWN"));
        let name = format!("{type_name}({lower}..{upper})");
        return Ok(ctx.registry.register(
            name,
            trust_hir::Type::Subrange {
                base: base_id,
                lower,
                upper,
            },
        ));
    }
    Ok(base_id)
}

pub(crate) fn resolve_type_name(
    name: &str,
    ctx: &LoweringContext<'_>,
) -> Result<TypeId, CompileError> {
    if let Some(id) = ctx.registry.lookup(name) {
        return Ok(id);
    }
    if !name.contains('.') {
        for namespace in &ctx.using {
            let qualified = format!("{namespace}.{name}");
            if let Some(id) = ctx.registry.lookup(&qualified) {
                return Ok(id);
            }
        }
    }
    Err(CompileError::new(format!("unknown type '{name}'")))
}

pub(crate) fn resolve_named_type(
    registry: &trust_hir::types::TypeRegistry,
    type_name: &str,
    using: &[SmolStr],
) -> Result<SmolStr, CompileError> {
    if let Some(id) = registry.lookup(type_name) {
        return registry
            .type_name(id)
            .ok_or_else(|| CompileError::new("unknown type"));
    }
    if !type_name.contains('.') {
        for namespace in using {
            let qualified = format!("{namespace}.{type_name}");
            if let Some(id) = registry.lookup(qualified.as_str()) {
                return registry
                    .type_name(id)
                    .ok_or_else(|| CompileError::new("unknown type"));
            }
        }
    }
    Err(CompileError::new(format!("unknown type '{type_name}'")))
}

pub(crate) fn function_block_type_name(
    type_id: TypeId,
    registry: &trust_hir::types::TypeRegistry,
) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        Type::FunctionBlock { name } => Some(name.clone()),
        Type::Alias { target, .. } => function_block_type_name(*target, registry),
        _ => None,
    }
}

pub(crate) fn class_type_name(
    type_id: TypeId,
    registry: &trust_hir::types::TypeRegistry,
) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        Type::Class { name } => Some(name.clone()),
        Type::Alias { target, .. } => class_type_name(*target, registry),
        _ => None,
    }
}

pub(crate) fn interface_type_name(
    type_id: TypeId,
    registry: &trust_hir::types::TypeRegistry,
) -> Option<SmolStr> {
    let ty = registry.get(type_id)?;
    match ty {
        Type::Interface { name } => Some(name.clone()),
        Type::Alias { target, .. } => interface_type_name(*target, registry),
        _ => None,
    }
}
