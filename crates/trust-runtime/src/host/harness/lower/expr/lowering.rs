pub(in crate::harness) fn lower_lvalue(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<LValue, CompileError> {
    match node.kind() {
        SyntaxKind::NameRef => Ok(LValue::Name(node_text(node).into())),
        SyntaxKind::IndexExpr => {
            let exprs = direct_expr_children(node);
            if exprs.len() < 2 {
                return Err(CompileError::new("invalid index expression"));
            }
            let target = lower_lvalue(&exprs[0], ctx)?;
            let mut indices = Vec::new();
            for expr in exprs.iter().skip(1) {
                indices.push(lower_expr(expr, ctx)?);
            }
            if indices.is_empty() {
                return Err(CompileError::new("missing index expression"));
            }
            Ok(LValue::Index {
                target: Box::new(target),
                indices,
            })
        }
        SyntaxKind::FieldExpr => {
            let exprs = direct_expr_children(node);
            if exprs.is_empty() {
                return Err(CompileError::new("invalid field expression"));
            }
            let target = lower_lvalue(&exprs[0], ctx)?;
            let field = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))
                .ok_or_else(|| CompileError::new("missing field name"))?;
            Ok(LValue::Field {
                target: Box::new(target),
                field: node_text(&field).into(),
            })
        }
        SyntaxKind::DerefExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing deref target"))?;
            Ok(LValue::Deref(Box::new(lower_expr(&expr, ctx)?)))
        }
        _ => Err(CompileError::new("unsupported assignment target")),
    }
}

pub(in crate::harness) fn lower_expr(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    lower_expr_with_context(node, ctx, None)
}

pub(in crate::harness) fn lower_expr_with_context(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    expected_type: Option<TypeId>,
) -> Result<Expr, CompileError> {
    match node.kind() {
        SyntaxKind::Literal => lower_literal_with_context(node, ctx, expected_type),
        SyntaxKind::NameRef => Ok(Expr::Name(node_text(node).into())),
        SyntaxKind::ThisExpr => Ok(Expr::This),
        SyntaxKind::SuperExpr => Ok(Expr::Super),
        SyntaxKind::UnaryExpr => {
            let op = unary_op_from_node(node)?;
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing unary operand"))?;
            Ok(Expr::Unary {
                op,
                expr: Box::new(lower_expr_with_context(&expr, ctx, expected_type)?),
            })
        }
        SyntaxKind::BinaryExpr => {
            let op = binary_op_from_node(node)?;
            let exprs = direct_expr_children(node);
            if exprs.len() != 2 {
                return Err(CompileError::new("invalid binary expression"));
            }
            let left_type = lower_expression_type(&exprs[0], ctx)?;
            let right_type = lower_expression_type(&exprs[1], ctx)?;
            let operand_context = binary_operand_context(op, expected_type, node, ctx)?;
            let mut left = lower_expr_with_context(&exprs[0], ctx, operand_context)?;
            let mut right = lower_expr_with_context(&exprs[1], ctx, operand_context)?;
            // Let a bare enum variant name on one side resolve against the
            // other side's enum type. Mirrors the v0.18.4 CASE-label fix
            // (commit 8d7f069) for symmetric binary operands such as
            // `state = RUNNING` and `RUNNING = state`.
            if let Some(type_id) = right_type {
                left = resolve_initializer_enum_variant(&exprs[0], left, type_id, ctx)?;
            }
            if let Some(type_id) = left_type {
                right = resolve_initializer_enum_variant(&exprs[1], right, type_id, ctx)?;
            }
            Ok(Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            })
        }
        SyntaxKind::ParenExpr => {
            let expr = first_expr_child(node)
                .ok_or_else(|| CompileError::new("missing parenthesized expression"))?;
            lower_expr_with_context(&expr, ctx, expected_type)
        }
        SyntaxKind::IndexExpr => {
            let exprs = direct_expr_children(node);
            if exprs.len() < 2 {
                return Err(CompileError::new("invalid index expression"));
            }
            let mut indices = Vec::new();
            for expr in exprs.iter().skip(1) {
                indices.push(lower_expr(expr, ctx)?);
            }
            Ok(Expr::Index {
                target: Box::new(lower_expr(&exprs[0], ctx)?),
                indices,
            })
        }
        SyntaxKind::FieldExpr => {
            let exprs = direct_expr_children(node);
            if exprs.is_empty() {
                return Err(CompileError::new("invalid field expression"));
            }
            let field = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))
                .ok_or_else(|| CompileError::new("missing field name"))?;
            let field_name: SmolStr = node_text(&field).into();
            if field_expr_property_accessor_name(node, ctx, PropertyAccessor::Get)?.is_some() {
                return Ok(Expr::Call {
                    target: Box::new(Expr::Field {
                        target: Box::new(lower_expr(&exprs[0], ctx)?),
                        field: field_name,
                    }),
                    args: Vec::new(),
                });
            }
            Ok(Expr::Field {
                target: Box::new(lower_expr(&exprs[0], ctx)?),
                field: field_name,
            })
        }
        SyntaxKind::DerefExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing deref target"))?;
            Ok(Expr::Deref(Box::new(lower_expr(&expr, ctx)?)))
        }
        SyntaxKind::AddrExpr => {
            let expr =
                first_expr_child(node).ok_or_else(|| CompileError::new("missing ADR operand"))?;
            let lvalue = lower_lvalue(&expr, ctx)?;
            Ok(Expr::Ref(lvalue))
        }
        SyntaxKind::CallExpr => lower_call_expr(node, ctx),
        SyntaxKind::SizeOfExpr => lower_sizeof_expr(node, ctx),
        SyntaxKind::ArrayInitializer => lower_array_initializer(node, ctx),
        SyntaxKind::InitializerList => lower_struct_initializer(node, ctx),
        _ => Err(CompileError::new("unsupported expression")),
    }
}

fn binary_operand_context(
    op: BinaryOp,
    expected_type: Option<TypeId>,
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Option<TypeId>, CompileError> {
    match op {
        BinaryOp::Add
        | BinaryOp::Sub
        | BinaryOp::Mul
        | BinaryOp::Div
        | BinaryOp::Mod
        | BinaryOp::Pow
        | BinaryOp::And
        | BinaryOp::Or
        | BinaryOp::Xor => Ok(expected_type.or(lower_expression_type(node, ctx)?)),
        BinaryOp::Eq | BinaryOp::Ne | BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
            Ok(None)
        }
    }
}

#[derive(Clone, Copy)]
pub(in crate::harness) enum PropertyAccessor {
    Get,
    Set,
}

pub(in crate::harness) fn field_expr_property_accessor_name(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    accessor: PropertyAccessor,
) -> Result<Option<SmolStr>, CompileError> {
    if node.kind() != SyntaxKind::FieldExpr {
        return Ok(None);
    }
    let exprs = direct_expr_children(node);
    let Some(receiver) = exprs.first() else {
        return Ok(None);
    };
    let Some(field) = node
        .children()
        .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))
    else {
        return Ok(None);
    };
    let field_name: SmolStr = node_text(&field).into();
    let Some((symbols, type_id)) = hir_expression_type(receiver, ctx) else {
        return Ok(None);
    };
    let Some(symbol_id) = symbols.resolve_member_symbol_in_type(type_id, field_name.as_str())
    else {
        return Ok(None);
    };
    let Some(symbol) = symbols.get(symbol_id) else {
        return Ok(None);
    };
    let SymbolKind::Property {
        has_get, has_set, ..
    } = symbol.kind
    else {
        return Ok(None);
    };
    let available = match accessor {
        PropertyAccessor::Get => has_get,
        PropertyAccessor::Set => has_set,
    };
    Ok(available.then_some(field_name))
}

fn hir_expression_type(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Option<(std::sync::Arc<SymbolTable>, TypeId)> {
    if node.kind() == SyntaxKind::ParenExpr {
        let inner = first_expr_child(node)?;
        return hir_expression_type(&inner, ctx);
    }

    let (semantic_db, semantic_file_id) = (ctx.semantic_db?, ctx.semantic_file_id?);
    let expr_id = semantic_db.expr_id_at_offset(semantic_file_id, offset_for_type_lookup(node))?;
    let type_id = semantic_db.type_of(semantic_file_id, expr_id);
    if type_id == TypeId::UNKNOWN {
        return None;
    }
    let analysis = semantic_db.analyze(semantic_file_id);
    Some((analysis.symbols.clone(), type_id))
}

fn lower_array_initializer(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    let mut elements = Vec::new();
    for child in node.children() {
        if is_expression_kind(child.kind()) {
            elements.push(lower_expr(&child, ctx)?);
        }
    }
    Ok(Expr::ArrayInitializer(elements))
}

fn lower_struct_initializer(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    let mut fields = Vec::new();
    let mut children = node.children().peekable();
    while let Some(child) = children.next() {
        if child.kind() != SyntaxKind::Name {
            continue;
        }
        let field = node_text(&child).into();
        let Some(value_node) = children.find(|candidate| is_expression_kind(candidate.kind()))
        else {
            return Err(CompileError::new("missing aggregate initializer value"));
        };
        fields.push((field, lower_expr(&value_node, ctx)?));
    }
    Ok(Expr::StructInitializer(fields))
}

fn lower_sizeof_expr(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Expr, CompileError> {
    if let Some(type_ref) = node
        .children()
        .find(|child| child.kind() == SyntaxKind::TypeRef)
    {
        let type_id = lower_type_ref(&type_ref, ctx)?;
        return Ok(Expr::SizeOf(crate::program_model::SizeOfTarget::Type(type_id)));
    }
    if let Some(expr_node) = node
        .children()
        .find(|child| is_expression_kind(child.kind()))
    {
        let type_id = lower_sizeof_operand_type(&expr_node, ctx)?;
        return Ok(Expr::SizeOf(crate::program_model::SizeOfTarget::Type(type_id)));
    }
    Err(CompileError::new("SIZEOF expects a type or expression"))
}

fn lower_sizeof_operand_type(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<TypeId, CompileError> {
    // Namespace-qualified type operands like Demo.Packet parse as FieldExpr,
    // so prefer the type path before semantic value typing for that shape.
    if node.kind() == SyntaxKind::FieldExpr {
        if let Some(type_id) = lower_sizeof_named_type_operand(node, ctx)? {
            return Ok(type_id);
        }
    }
    if let Some(type_id) = lower_sizeof_value_operand_type(node, ctx)? {
        return Ok(type_id);
    }
    if let Some(type_id) = lower_sizeof_named_type_operand(node, ctx)? {
        return Ok(type_id);
    }

    if node.kind() == SyntaxKind::NameRef {
        return Err(CompileError::new(format!(
            "SIZEOF operand '{}' is neither a variable nor a type",
            node_text(node)
        )));
    }

    Err(CompileError::new(
        "SIZEOF expects a type name or storage operand",
    ))
}

fn lower_sizeof_value_operand_type(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Option<TypeId>, CompileError> {
    if node.kind() == SyntaxKind::ParenExpr {
        let Some(inner) = first_expr_child(node) else {
            return Ok(None);
        };
        return lower_sizeof_value_operand_type(&inner, ctx);
    }

    if !matches!(
        node.kind(),
        SyntaxKind::NameRef
            | SyntaxKind::FieldExpr
            | SyntaxKind::IndexExpr
            | SyntaxKind::DerefExpr
            | SyntaxKind::ThisExpr
            | SyntaxKind::SuperExpr
    ) {
        return Ok(None);
    }

    let (semantic_db, semantic_file_id) = match (ctx.semantic_db, ctx.semantic_file_id) {
        (Some(db), Some(file_id)) => (db, file_id),
        _ => return Ok(None),
    };

    let range = node.text_range();
    let start = u32::from(range.start());
    let end = u32::from(range.end());
    let Some(expr_id) = semantic_db
        .expr_id_for_range(semantic_file_id, start, end)
        .or_else(|| semantic_db.expr_id_at_offset(semantic_file_id, offset_for_type_lookup(node)))
    else {
        return Ok(None);
    };
    let hir_type_id = semantic_db.type_of(semantic_file_id, expr_id);
    if hir_type_id == TypeId::UNKNOWN {
        return Ok(None);
    }

    let analysis = semantic_db.analyze(semantic_file_id);
    let Some(hir_type) = analysis.symbols.type_by_id(hir_type_id) else {
        return Ok(None);
    };
    if matches!(
        hir_type,
        Type::Unknown
            | Type::Any
            | Type::AnyDerived
            | Type::AnyElementary
            | Type::AnyMagnitude
            | Type::AnyInt
            | Type::AnyUnsigned
            | Type::AnySigned
            | Type::AnyReal
            | Type::AnyNum
            | Type::AnyDuration
            | Type::AnyBit
            | Type::AnyChars
            | Type::AnyString
            | Type::AnyChar
            | Type::AnyDate
    ) {
        return Ok(None);
    }
    let runtime_type_id = import_hir_type_to_runtime(ctx.registry, analysis.symbols.as_ref(), hir_type_id)?;
    Ok(Some(runtime_type_id))
}

/// Pick an offset inside `node` that makes HIR's "smallest expression
/// containing this offset" heuristic land on `node` itself rather than a
/// leading child such as the leftmost `NameRef` of an `IndexExpr` or
/// `FieldExpr`. The last byte of the text range is inside the outer
/// expression but outside the half-open ranges of its prefix children,
/// so it disambiguates `arr[i]`, `c.p`, and `arr[i].p` without affecting
/// simple `NameRef` / `DerefExpr` / `ThisExpr` cases, whose ranges
/// already contain their own last byte.
fn offset_for_type_lookup(node: &SyntaxNode) -> u32 {
    let range = node.text_range();
    let end = u32::from(range.end());
    let start = u32::from(range.start());
    if end > start {
        end - 1
    } else {
        start
    }
}

pub(in crate::harness) fn lower_expression_type(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Option<TypeId>, CompileError> {
    if node.kind() == SyntaxKind::ParenExpr {
        let Some(inner) = first_expr_child(node) else {
            return Ok(None);
        };
        return lower_expression_type(&inner, ctx);
    }

    let (semantic_db, semantic_file_id) = match (ctx.semantic_db, ctx.semantic_file_id) {
        (Some(db), Some(file_id)) => (db, file_id),
        _ => return Ok(None),
    };

    let offset = offset_for_type_lookup(node);
    let Some(expr_id) = semantic_db.expr_id_at_offset(semantic_file_id, offset) else {
        return Ok(None);
    };
    let hir_type_id = semantic_db.type_of(semantic_file_id, expr_id);
    if hir_type_id == TypeId::UNKNOWN {
        return Ok(None);
    }

    let analysis = semantic_db.analyze(semantic_file_id);
    let Some(hir_type) = analysis.symbols.type_by_id(hir_type_id) else {
        return Ok(None);
    };
    if matches!(
        hir_type,
        Type::Unknown
            | Type::Any
            | Type::AnyDerived
            | Type::AnyElementary
            | Type::AnyMagnitude
            | Type::AnyInt
            | Type::AnyUnsigned
            | Type::AnySigned
            | Type::AnyReal
            | Type::AnyNum
            | Type::AnyDuration
            | Type::AnyBit
            | Type::AnyChars
            | Type::AnyString
            | Type::AnyChar
            | Type::AnyDate
    ) {
        return Ok(None);
    }

    let runtime_type_id =
        import_hir_type_to_runtime(ctx.registry, analysis.symbols.as_ref(), hir_type_id)?;
    Ok(Some(runtime_type_id))
}

fn lower_sizeof_named_type_operand(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Option<TypeId>, CompileError> {
    if node.kind() == SyntaxKind::ParenExpr {
        let Some(inner) = first_expr_child(node) else {
            return Ok(None);
        };
        return lower_sizeof_named_type_operand(&inner, ctx);
    }

    match node.kind() {
        SyntaxKind::NameRef => {
            let resolved = resolve_type_name_from_operand(&node_text(node), ctx)?;
            Ok(resolved)
        }
        SyntaxKind::FieldExpr => {
            let Some(qualified) = qualified_type_operand_name(node) else {
                return Ok(None);
            };
            let resolved = resolve_type_name_from_operand(&qualified, ctx)?;
            Ok(resolved)
        }
        _ => Ok(None),
    }
}

fn resolve_type_name_from_operand(
    name: &str,
    ctx: &mut LoweringContext<'_>,
) -> Result<Option<TypeId>, CompileError> {
    if let Some(type_id) = ctx.registry.lookup(name) {
        return Ok(Some(type_id));
    }
    if name.contains('.') {
        return Ok(None);
    }
    match resolve_type_name(name, ctx) {
        Ok(type_id) => Ok(Some(type_id)),
        Err(_) => Ok(None),
    }
}

fn qualified_type_operand_name(node: &SyntaxNode) -> Option<String> {
    match node.kind() {
        SyntaxKind::NameRef => Some(node_text(node)),
        SyntaxKind::FieldExpr => {
            let exprs = direct_expr_children(node);
            let target = exprs.first()?;
            let prefix = qualified_type_operand_name(target)?;
            let field = node
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::Literal))?;
            Some(format!("{prefix}.{}", node_text(&field)))
        }
        SyntaxKind::ParenExpr => {
            let inner = first_expr_child(node)?;
            qualified_type_operand_name(&inner)
        }
        _ => None,
    }
}

fn import_hir_type_to_runtime(
    registry: &mut TypeRegistry,
    symbols: &SymbolTable,
    hir_type_id: TypeId,
) -> Result<TypeId, CompileError> {
    if hir_type_id.builtin_name().is_some() {
        return Ok(hir_type_id);
    }

    let Some(type_name) = symbols.type_name(hir_type_id) else {
        return Err(CompileError::new("unable to resolve SIZEOF operand type"));
    };
    if let Some(existing) = registry.lookup(type_name.as_str()) {
        return Ok(existing);
    }

    let hir_type = symbols
        .type_by_id(hir_type_id)
        .ok_or_else(|| CompileError::new("unable to resolve SIZEOF operand type"))?
        .clone();

    match hir_type {
        Type::Alias { name, target } => {
            let target = import_hir_type_to_runtime(registry, symbols, target)?;
            Ok(registry.register(name.clone(), Type::Alias { name, target }))
        }
        Type::Struct { name, fields } => {
            let mut lowered = Vec::with_capacity(fields.len());
            for field in fields {
                lowered.push(trust_hir::types::StructField {
                    name: field.name,
                    type_id: import_hir_type_to_runtime(registry, symbols, field.type_id)?,
                    address: field.address,
                    default_initializer: field.default_initializer,
                });
            }
            Ok(registry.register_struct(name, lowered))
        }
        Type::Union { name, variants } => {
            let mut lowered = Vec::with_capacity(variants.len());
            for variant in variants {
                lowered.push(trust_hir::types::UnionVariant {
                    name: variant.name,
                    type_id: import_hir_type_to_runtime(registry, symbols, variant.type_id)?,
                    address: variant.address,
                    default_initializer: variant.default_initializer,
                });
            }
            Ok(registry.register_union(name, lowered))
        }
        Type::Enum { name, base, values } => {
            let base = import_hir_type_to_runtime(registry, symbols, base)?;
            Ok(registry.register_enum(name, base, values))
        }
        Type::Array {
            element,
            dimensions,
        } => {
            let element = import_hir_type_to_runtime(registry, symbols, element)?;
            Ok(registry.register_array(element, dimensions))
        }
        Type::Pointer { target } => {
            let target = import_hir_type_to_runtime(registry, symbols, target)?;
            Ok(registry.register_pointer(target))
        }
        Type::Reference { target } => {
            let target = import_hir_type_to_runtime(registry, symbols, target)?;
            Ok(registry.register_reference(target))
        }
        Type::Subrange { base, lower, upper } => {
            let base = import_hir_type_to_runtime(registry, symbols, base)?;
            let name = format!(
                "{}({lower}..{upper})",
                registry
                    .type_name(base)
                    .unwrap_or_else(|| smol_str::SmolStr::new("UNKNOWN"))
            );
            Ok(registry.register(name, Type::Subrange { base, lower, upper }))
        }
        Type::String {
            max_len: Some(max_len),
        } => Ok(registry.register_string_with_length(max_len)),
        Type::WString {
            max_len: Some(max_len),
        } => Ok(registry.register_wstring_with_length(max_len)),
        Type::FunctionBlock { name } => Ok(registry
            .lookup(name.as_str())
            .unwrap_or_else(|| registry.register(name.clone(), Type::FunctionBlock { name }))),
        Type::Class { name } => Ok(registry
            .lookup(name.as_str())
            .unwrap_or_else(|| registry.register(name.clone(), Type::Class { name }))),
        Type::Interface { name } => Ok(registry
            .lookup(name.as_str())
            .unwrap_or_else(|| registry.register(name.clone(), Type::Interface { name }))),
        Type::Unknown => Ok(TypeId::UNKNOWN),
        other => {
            let name = type_name_for_anonymous_hir_type(registry, &other);
            Ok(registry.register(name, other))
        }
    }
}

fn type_name_for_anonymous_hir_type(registry: &TypeRegistry, ty: &Type) -> String {
    match ty {
        Type::String {
            max_len: Some(max_len),
        } => format!("STRING[{max_len}]"),
        Type::WString {
            max_len: Some(max_len),
        } => format!("WSTRING[{max_len}]"),
        Type::Pointer { target } => format!(
            "POINTER TO {}",
            registry
                .type_name(*target)
                .unwrap_or_else(|| smol_str::SmolStr::new("?"))
        ),
        Type::Reference { target } => format!(
            "REF_TO {}",
            registry
                .type_name(*target)
                .unwrap_or_else(|| smol_str::SmolStr::new("?"))
        ),
        Type::Array {
            element,
            dimensions,
        } => {
            let dims = dimensions
                .iter()
                .map(|(lower, upper)| {
                    if *lower == 0 && *upper == i64::MAX {
                        "*".to_string()
                    } else {
                        format!("{lower}..{upper}")
                    }
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "ARRAY[{dims}] OF {}",
                registry
                    .type_name(*element)
                    .unwrap_or_else(|| smol_str::SmolStr::new("?"))
            )
        }
        _ => format!("{ty:?}"),
    }
}

fn lower_call_expr(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Expr, CompileError> {
    let target_node =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing call target"))?;
    if target_node.kind() == SyntaxKind::NameRef
        && node_text(&target_node).eq_ignore_ascii_case("REF")
    {
        return lower_ref_call_expr(node, ctx);
    }
    if let Some(type_id) = aggregate_initializer_call_type(&target_node, ctx)? {
        let fields = lower_aggregate_call_args(node, ctx, type_id)?;
        return Ok(Expr::StructInitializer(fields));
    }
    let target = lower_expr(&target_node, ctx)?;
    let args = lower_call_args(node, ctx)?;
    Ok(Expr::Call {
        target: Box::new(target),
        args,
    })
}

fn lower_ref_call_expr(node: &SyntaxNode, ctx: &mut LoweringContext<'_>) -> Result<Expr, CompileError> {
    let arg_list = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList)
        .ok_or_else(|| CompileError::new("REF requires one target"))?;
    let mut args = arg_list
        .children()
        .filter(|child| child.kind() == SyntaxKind::Arg);
    let arg = args
        .next()
        .ok_or_else(|| CompileError::new("REF requires one target"))?;
    if args.next().is_some() {
        return Err(CompileError::new("REF requires exactly one target"));
    }
    if arg.children().any(|child| child.kind() == SyntaxKind::Name) {
        return Err(CompileError::new("REF target must be positional"));
    }
    let expr = first_expr_child(&arg).ok_or_else(|| CompileError::new("REF target missing"))?;
    Ok(Expr::Ref(lower_lvalue(&expr, ctx)?))
}

fn aggregate_initializer_call_type(
    target: &SyntaxNode,
    ctx: &LoweringContext<'_>,
) -> Result<Option<TypeId>, CompileError> {
    if target.kind() != SyntaxKind::NameRef {
        return Ok(None);
    }
    if name_ref_resolves_to_value_symbol(target, ctx) {
        return Ok(None);
    }
    let name = node_text(target);
    let Ok(type_id) = resolve_type_name(name.as_str(), ctx) else {
        return Ok(None);
    };
    if aggregate_runtime_type(type_id, ctx.registry).is_some() {
        return Ok(Some(type_id));
    }
    if matches!(
        ctx.registry.get(type_id),
        Some(Type::Class { .. } | Type::Interface { .. } | Type::FunctionBlock { .. })
    ) {
        return Err(CompileError::new(
            "class and function block types do not support call-style aggregate initialization here",
        ));
    }
    Ok(None)
}

fn name_ref_resolves_to_value_symbol(target: &SyntaxNode, ctx: &LoweringContext<'_>) -> bool {
    let (Some(semantic_db), Some(semantic_file_id)) = (ctx.semantic_db, ctx.semantic_file_id)
    else {
        return false;
    };
    let analysis = semantic_db.analyze(semantic_file_id);
    let symbols = analysis.symbols.as_ref();
    let scope_context = expression_scope_context(symbols, target);
    let name = node_text(target);
    let Some(symbol_id) = resolve_name_symbol_in_scope(
        symbols,
        scope_context.scope_id,
        scope_context.current_pou_symbol,
        scope_context.this_type,
        name.as_str(),
    ) else {
        return false;
    };
    let Some(symbol) = symbols.get(symbol_id) else {
        return false;
    };
    matches!(
        symbol.kind,
        SymbolKind::Variable { .. }
            | SymbolKind::Constant
            | SymbolKind::Parameter { .. }
            | SymbolKind::Function { .. }
            | SymbolKind::Method { .. }
            | SymbolKind::Property { .. }
    )
}

fn aggregate_runtime_type(type_id: TypeId, registry: &TypeRegistry) -> Option<TypeId> {
    let mut current = type_id;
    for _ in 0..16 {
        match registry.get(current)? {
            Type::Alias { target, .. } => current = *target,
            Type::Struct { .. } | Type::Union { .. } => return Some(current),
            _ => return None,
        }
    }
    None
}

fn lower_aggregate_call_args(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
    type_id: TypeId,
) -> Result<Vec<(SmolStr, Expr)>, CompileError> {
    let Some(arg_list) = node.children().find(|child| child.kind() == SyntaxKind::ArgList) else {
        return Ok(Vec::new());
    };
    let mut fields = Vec::new();
    for arg in arg_list
        .children()
        .filter(|child| child.kind() == SyntaxKind::Arg)
    {
        let name = arg
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .map(|name| node_text(&name).into())
            .ok_or_else(|| {
                CompileError::new(
                    "positional struct initializers are not supported; use named field initializers",
                )
            })?;
        let expr_node =
            first_expr_child(&arg).ok_or_else(|| CompileError::new("missing initializer value"))?;
        let value = lower_expr(&expr_node, ctx).and_then(|lowered| {
            resolve_initializer_enum_variant(&expr_node, lowered, type_id, ctx)
        })?;
        fields.push((name, value));
    }
    Ok(fields)
}

fn lower_call_args(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<CallArg>, CompileError> {
    let arg_list = node
        .children()
        .find(|child| child.kind() == SyntaxKind::ArgList);
    let Some(arg_list) = arg_list else {
        return Ok(Vec::new());
    };
    let mut args = Vec::new();
    for arg in arg_list
        .children()
        .filter(|child| child.kind() == SyntaxKind::Arg)
    {
        args.push(lower_call_arg(&arg, ctx)?);
    }
    Ok(args)
}

fn lower_call_arg(
    node: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<CallArg, CompileError> {
    let name = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .map(|name| node_text(&name).into());

    let mut has_arrow = false;
    for token in node
        .children_with_tokens()
        .filter_map(|child| child.into_token())
    {
        if token.kind() == SyntaxKind::Arrow {
            has_arrow = true;
        }
    }

    let expr_node =
        first_expr_child(node).ok_or_else(|| CompileError::new("missing call argument"))?;
    let value = if has_arrow {
        ArgValue::Target(lower_lvalue(&expr_node, ctx)?)
    } else if field_expr_property_accessor_name(&expr_node, ctx, PropertyAccessor::Get)?.is_some()
    {
        ArgValue::Expr(lower_expr(&expr_node, ctx)?)
    } else {
        match lower_lvalue(&expr_node, ctx) {
            Ok(target) => ArgValue::Target(target),
            Err(_) => ArgValue::Expr(lower_expr(&expr_node, ctx)?),
        }
    };

    Ok(CallArg { name, value })
}
