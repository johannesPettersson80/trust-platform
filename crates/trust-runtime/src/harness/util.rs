use smol_str::SmolStr;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

pub(super) fn extract_name_from_expr(node: &SyntaxNode) -> Option<SmolStr> {
    match node.kind() {
        SyntaxKind::NameRef | SyntaxKind::Name => Some(SmolStr::new(node_text(node))),
        _ => None,
    }
}

pub(super) fn node_text(node: &SyntaxNode) -> String {
    let mut text = String::new();
    for element in node.descendants_with_tokens() {
        let token = match element.into_token() {
            Some(token) => token,
            None => continue,
        };
        match token.kind() {
            SyntaxKind::Ident | SyntaxKind::Dot | SyntaxKind::KwEn | SyntaxKind::KwEno => {
                text.push_str(token.text());
            }
            _ => {}
        }
    }
    if text.is_empty() {
        node.text().to_string().split_whitespace().collect()
    } else {
        text
    }
}

pub(super) fn collect_using_directives(node: &SyntaxNode) -> Vec<SmolStr> {
    let mut ancestors: Vec<SyntaxNode> = node.ancestors().collect();
    ancestors.reverse();
    let mut implicit_namespaces = Vec::new();
    let mut explicit_usings = Vec::new();
    let mut namespace_parts = Vec::new();
    for ancestor in ancestors {
        if ancestor.kind() == SyntaxKind::Namespace {
            if let Some(namespace_name) = ancestor
                .children()
                .find(|child| matches!(child.kind(), SyntaxKind::QualifiedName | SyntaxKind::Name))
            {
                namespace_parts.push(node_text(&namespace_name));
                implicit_namespaces.push(SmolStr::new(namespace_parts.join(".")));
            }
        }
        for using in ancestor
            .children()
            .filter(|child| child.kind() == SyntaxKind::UsingDirective)
        {
            explicit_usings.extend(using_directive_names(&using));
        }
    }
    implicit_namespaces.reverse();
    let mut names = implicit_namespaces;
    names.extend(explicit_usings);
    names
}

pub(super) fn namespace_qualified_name(node: &SyntaxNode, name: &str) -> SmolStr {
    let mut parts = Vec::new();
    for ancestor in node.ancestors() {
        if ancestor.kind() != SyntaxKind::Namespace {
            continue;
        }
        let Some(namespace_name) = ancestor
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::QualifiedName | SyntaxKind::Name))
        else {
            continue;
        };
        parts.push(node_text(&namespace_name));
    }
    parts.reverse();
    parts.push(name.to_string());
    SmolStr::new(parts.join("."))
}

pub(super) fn using_directive_names(node: &SyntaxNode) -> Vec<SmolStr> {
    node.children()
        .filter(|child| {
            matches!(
                child.kind(),
                SyntaxKind::QualifiedName | SyntaxKind::Name | SyntaxKind::NameRef
            )
        })
        .map(|child| SmolStr::new(node_text(&child)))
        .collect()
}

pub(super) fn builtin_type_name(kind: SyntaxKind) -> Option<&'static str> {
    match kind {
        SyntaxKind::KwBool => Some("BOOL"),
        SyntaxKind::KwSInt => Some("SINT"),
        SyntaxKind::KwInt => Some("INT"),
        SyntaxKind::KwDInt => Some("DINT"),
        SyntaxKind::KwLInt => Some("LINT"),
        SyntaxKind::KwUSInt => Some("USINT"),
        SyntaxKind::KwUInt => Some("UINT"),
        SyntaxKind::KwUDInt => Some("UDINT"),
        SyntaxKind::KwULInt => Some("ULINT"),
        SyntaxKind::KwByte => Some("BYTE"),
        SyntaxKind::KwWord => Some("WORD"),
        SyntaxKind::KwDWord => Some("DWORD"),
        SyntaxKind::KwLWord => Some("LWORD"),
        SyntaxKind::KwReal => Some("REAL"),
        SyntaxKind::KwLReal => Some("LREAL"),
        SyntaxKind::KwTime => Some("TIME"),
        SyntaxKind::KwLTime => Some("LTIME"),
        SyntaxKind::KwDate => Some("DATE"),
        SyntaxKind::KwLDate => Some("LDATE"),
        SyntaxKind::KwTimeOfDay => Some("TOD"),
        SyntaxKind::KwLTimeOfDay => Some("LTOD"),
        SyntaxKind::KwDateAndTime => Some("DT"),
        SyntaxKind::KwLDateAndTime => Some("LDT"),
        SyntaxKind::KwString => Some("STRING"),
        SyntaxKind::KwWString => Some("WSTRING"),
        SyntaxKind::KwChar => Some("CHAR"),
        SyntaxKind::KwWChar => Some("WCHAR"),
        _ => None,
    }
}

pub(super) fn is_expression_kind(kind: SyntaxKind) -> bool {
    kind.is_initializer_expression_node()
}

pub(super) fn is_statement_kind(kind: SyntaxKind) -> bool {
    kind.is_statement_node()
}

pub(super) fn direct_expr_children(node: &SyntaxNode) -> Vec<SyntaxNode> {
    node.children()
        .filter(|child| is_expression_kind(child.kind()))
        .collect()
}

pub(super) fn first_expr_child(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children()
        .find(|child| is_expression_kind(child.kind()))
}
