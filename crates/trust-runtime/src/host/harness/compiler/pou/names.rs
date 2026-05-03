pub(crate) fn qualified_pou_name(node: &SyntaxNode) -> Result<SmolStr, CompileError> {
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)
        .ok_or_else(|| CompileError::new("missing POU name"))?;
    let mut parts = Vec::new();
    parts.push(node_text(&name_node));
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
    Ok(parts.join(".").into())
}
