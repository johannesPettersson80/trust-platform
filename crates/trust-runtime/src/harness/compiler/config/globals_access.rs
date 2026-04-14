fn lower_global_var_block(
    var_block: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<GlobalInit>, CompileError> {
    let mut globals = Vec::new();
    let kind = var_block_kind(var_block)?;
    let qualifiers = var_block_qualifiers(var_block);
    for var_decl in var_block
        .children()
        .filter(|child| child.kind() == SyntaxKind::VarDecl)
    {
        let (names, type_ref, initializer, address) = parse_var_decl(&var_decl)?;
        let type_id = lower_type_ref(&type_ref, ctx)?;
        let init_expr = initializer.map(|expr| lower_expr(&expr, ctx)).transpose()?;
        match kind {
            VarBlockKind::Global
            | VarBlockKind::Var
            | VarBlockKind::Input
            | VarBlockKind::Output
            | VarBlockKind::InOut => {
                for name in names {
                    globals.push(GlobalInit {
                        name: namespace_qualified_name(var_block, name.as_str()),
                        type_id,
                        initializer: init_expr.clone(),
                        retain: qualifiers.retain,
                        address: address.clone(),
                    });
                }
            }
            VarBlockKind::External => {
                continue;
            }
            _ => {
                return Err(CompileError::new(
                    "unsupported VAR block in CONFIGURATION/RESOURCE",
                ));
            }
        }
    }
    Ok(globals)
}

#[derive(Default)]
struct VarAccessResult {
    globals: Vec<GlobalInit>,
    access: Vec<AccessDecl>,
}

fn lower_var_access_block(
    var_block: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<VarAccessResult, CompileError> {
    let mut result = VarAccessResult::default();
    for access_decl in var_block
        .children()
        .filter(|child| child.kind() == SyntaxKind::AccessDecl)
    {
        let name_node = access_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::Name)
            .ok_or_else(|| CompileError::new("missing VAR_ACCESS name"))?;
        let name = SmolStr::new(node_text(&name_node));
        let path_node = access_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::AccessPath)
            .ok_or_else(|| CompileError::new("missing VAR_ACCESS path"))?;
        let type_ref = access_decl
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
            .ok_or_else(|| CompileError::new("missing VAR_ACCESS type"))?;
        let type_id = lower_type_ref(&type_ref, ctx)?;
        let path = parse_access_path(&path_node, ctx)?;

        match &path {
            AccessPath::Direct { text, .. } => {
                result.globals.push(GlobalInit {
                    name: namespace_qualified_name(var_block, name.as_str()),
                    type_id,
                    initializer: None,
                    retain: crate::RetainPolicy::Unspecified,
                    address: Some(text.clone()),
                });
            }
            AccessPath::Parts(_) => {
                result.access.push(AccessDecl { name, path });
            }
        }
    }
    Ok(result)
}

fn lower_var_config_block(
    var_block: &SyntaxNode,
    ctx: &mut LoweringContext<'_>,
) -> Result<Vec<ConfigInit>, CompileError> {
    let mut inits = Vec::new();
    for config_init in var_block
        .children()
        .filter(|child| child.kind() == SyntaxKind::ConfigInit)
    {
        let path_node = config_init
            .children()
            .find(|child| child.kind() == SyntaxKind::AccessPath)
            .ok_or_else(|| CompileError::new("missing VAR_CONFIG path"))?;
        let type_ref = config_init
            .children()
            .find(|child| child.kind() == SyntaxKind::TypeRef)
            .ok_or_else(|| CompileError::new("missing VAR_CONFIG type"))?;
        let path = parse_access_path(&path_node, ctx)?;
        let type_id = lower_type_ref(&type_ref, ctx)?;
        let initializer = config_init
            .children()
            .find(|child| is_expression_kind(child.kind()))
            .map(|expr| lower_expr(&expr, ctx))
            .transpose()?;
        let address = config_init_address(&config_init)?;
        inits.push(ConfigInit {
            path,
            address,
            type_id,
            initializer,
        });
    }
    Ok(inits)
}
