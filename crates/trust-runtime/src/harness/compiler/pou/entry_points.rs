pub(crate) fn lower_programs(
    syntax: &SyntaxNode,
    catalog: &DeclarationCatalog,
    file_id: FileId,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<LoweredProgram>, CompileError> {
    let mut programs = Vec::new();
    for program_node in catalog_pou_nodes(
        syntax,
        catalog,
        file_id,
        DeclarationKind::Program,
        SyntaxKind::Program,
    )? {
        programs.push(lower_program_node(&program_node, registry, inputs)?);
    }
    Ok(programs)
}

pub(crate) fn lower_functions(
    syntax: &SyntaxNode,
    catalog: &DeclarationCatalog,
    file_id: FileId,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<FunctionDef>, CompileError> {
    let mut functions = Vec::new();
    for func_node in catalog_pou_nodes(
        syntax,
        catalog,
        file_id,
        DeclarationKind::Function,
        SyntaxKind::Function,
    )? {
        let using = collect_using_directives(&func_node);
        let mut ctx = inputs.context(registry, using);
        functions.push(lower_function_node(&func_node, &mut ctx)?);
    }
    Ok(functions)
}

pub(crate) fn lower_function_blocks(
    syntax: &SyntaxNode,
    catalog: &DeclarationCatalog,
    file_id: FileId,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<FunctionBlockDef>, CompileError> {
    let mut function_blocks = Vec::new();
    for fb_node in catalog_pou_nodes(
        syntax,
        catalog,
        file_id,
        DeclarationKind::FunctionBlock,
        SyntaxKind::FunctionBlock,
    )? {
        let using = collect_using_directives(&fb_node);
        let mut ctx = inputs.context(registry, using);
        function_blocks.push(lower_function_block_node(&fb_node, &mut ctx)?);
    }
    Ok(function_blocks)
}

pub(crate) fn lower_classes(
    syntax: &SyntaxNode,
    catalog: &DeclarationCatalog,
    file_id: FileId,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<ClassDef>, CompileError> {
    let mut classes = Vec::new();
    for class_node in catalog_pou_nodes(
        syntax,
        catalog,
        file_id,
        DeclarationKind::Class,
        SyntaxKind::Class,
    )? {
        let using = collect_using_directives(&class_node);
        let mut ctx = inputs.context(registry, using);
        classes.push(lower_class_node(&class_node, &mut ctx)?);
    }
    Ok(classes)
}

pub(crate) fn lower_interfaces(
    syntax: &SyntaxNode,
    catalog: &DeclarationCatalog,
    file_id: FileId,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<InterfaceDef>, CompileError> {
    let mut interfaces = Vec::new();
    for interface_node in catalog_pou_nodes(
        syntax,
        catalog,
        file_id,
        DeclarationKind::Interface,
        SyntaxKind::Interface,
    )? {
        let using = collect_using_directives(&interface_node);
        let mut ctx = inputs.context(registry, using);
        interfaces.push(lower_interface_node(&interface_node, &mut ctx)?);
    }
    Ok(interfaces)
}

fn catalog_pou_nodes(
    syntax: &SyntaxNode,
    catalog: &DeclarationCatalog,
    file_id: FileId,
    declaration_kind: DeclarationKind,
    syntax_kind: SyntaxKind,
) -> Result<Vec<SyntaxNode>, CompileError> {
    let mut nodes = Vec::new();
    for entry in catalog.entries().iter().filter(|entry| {
        entry.source().file_id() == file_id && entry.kind() == declaration_kind
    }) {
        let range = entry.source().range();
        let matches = syntax
            .descendants()
            .filter(|node| {
                node.kind() == syntax_kind && declaration_name_range(node) == Some(range)
            })
            .collect::<Vec<_>>();

        match matches.as_slice() {
            [node] => nodes.push(node.clone()),
            [] => {
                return Err(CompileError::new(format!(
                    "HIR declaration catalog/lowering mismatch: catalog entry '{}' at {:?} has no matching {:?} syntax node in file {:?}",
                    entry.qualified_name().display(),
                    range,
                    syntax_kind,
                    file_id,
                )));
            }
            _ => {
                return Err(CompileError::new(format!(
                    "HIR declaration catalog/lowering mismatch: catalog entry '{}' at {:?} matched multiple {:?} syntax nodes in file {:?}",
                    entry.qualified_name().display(),
                    range,
                    syntax_kind,
                    file_id,
                )));
            }
        }
    }
    Ok(nodes)
}

fn declaration_name_range(node: &SyntaxNode) -> Option<text_size::TextRange> {
    let name_node = node
        .children()
        .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
        .unwrap_or_else(|| node.clone());
    name_node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| {
            matches!(
                token.kind(),
                SyntaxKind::Ident
                    | SyntaxKind::KwEn
                    | SyntaxKind::KwEno
                    | SyntaxKind::KwGet
                    | SyntaxKind::KwSet
            )
        })
        .map(|token| token.text_range())
}
