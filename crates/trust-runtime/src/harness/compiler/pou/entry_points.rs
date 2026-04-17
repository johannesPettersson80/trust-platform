pub(crate) fn lower_programs(
    syntax: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<LoweredProgram>, CompileError> {
    let mut programs = Vec::new();
    for program_node in syntax
        .children()
        .filter(|child| child.kind() == SyntaxKind::Program)
    {
        programs.push(lower_program_node(&program_node, registry, inputs)?);
    }
    Ok(programs)
}

pub(crate) fn lower_functions(
    syntax: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<FunctionDef>, CompileError> {
    let mut functions = Vec::new();
    for func_node in syntax
        .descendants()
        .filter(|child| child.kind() == SyntaxKind::Function)
    {
        let using = collect_using_directives(&func_node);
        let mut ctx = inputs.context(registry, using);
        functions.push(lower_function_node(&func_node, &mut ctx)?);
    }
    Ok(functions)
}

pub(crate) fn lower_function_blocks(
    syntax: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<FunctionBlockDef>, CompileError> {
    let mut function_blocks = Vec::new();
    for fb_node in syntax
        .descendants()
        .filter(|child| child.kind() == SyntaxKind::FunctionBlock)
    {
        let using = collect_using_directives(&fb_node);
        let mut ctx = inputs.context(registry, using);
        function_blocks.push(lower_function_block_node(&fb_node, &mut ctx)?);
    }
    Ok(function_blocks)
}

pub(crate) fn lower_classes(
    syntax: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<ClassDef>, CompileError> {
    let mut classes = Vec::new();
    for class_node in syntax
        .descendants()
        .filter(|child| child.kind() == SyntaxKind::Class)
    {
        let using = collect_using_directives(&class_node);
        let mut ctx = inputs.context(registry, using);
        classes.push(lower_class_node(&class_node, &mut ctx)?);
    }
    Ok(classes)
}

pub(crate) fn lower_interfaces(
    syntax: &SyntaxNode,
    registry: &mut trust_hir::types::TypeRegistry,
    inputs: &mut LoweringInputs<'_>,
) -> Result<Vec<InterfaceDef>, CompileError> {
    let mut interfaces = Vec::new();
    for interface_node in syntax
        .descendants()
        .filter(|child| child.kind() == SyntaxKind::Interface)
    {
        let using = collect_using_directives(&interface_node);
        let mut ctx = inputs.context(registry, using);
        interfaces.push(lower_interface_node(&interface_node, &mut ctx)?);
    }
    Ok(interfaces)
}
