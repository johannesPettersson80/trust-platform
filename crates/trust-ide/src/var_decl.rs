use text_size::TextRange;

use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::util::ident_token_in_name;

#[derive(Default, Clone)]
pub(crate) struct VarDeclInfo {
    pub(crate) initializer: Option<String>,
    pub(crate) retention: Option<&'static str>,
    pub(crate) declared_type: Option<String>,
}

pub(crate) fn var_decl_info_for_symbol(
    root: &SyntaxNode,
    source: &str,
    symbol_range: TextRange,
) -> VarDeclInfo {
    let Some(var_decl) = find_var_decl_for_range(root, symbol_range) else {
        return VarDeclInfo::default();
    };
    let initializer = initializer_from_var_decl(source, &var_decl);
    let declared_type = declared_type_from_var_decl(source, &var_decl);
    let retention = var_decl
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::VarBlock)
        .and_then(|node| retention_from_var_block(&node));
    VarDeclInfo {
        initializer,
        retention,
        declared_type,
    }
}

pub(crate) fn var_decl_info_for_name(root: &SyntaxNode, source: &str, name: &str) -> VarDeclInfo {
    let Some(var_decl) = find_var_decl_for_name(root, name) else {
        return VarDeclInfo::default();
    };
    let initializer = initializer_from_var_decl(source, &var_decl);
    let declared_type = declared_type_from_var_decl(source, &var_decl);
    let retention = var_decl
        .ancestors()
        .find(|node| node.kind() == SyntaxKind::VarBlock)
        .and_then(|node| retention_from_var_block(&node));
    VarDeclInfo {
        initializer,
        retention,
        declared_type,
    }
}

/// Finds the variable declaration whose identifier matches or encloses `symbol_range`.
pub fn find_var_decl_for_range(root: &SyntaxNode, symbol_range: TextRange) -> Option<SyntaxNode> {
    let by_exact_ident = root
        .descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .find(|var_decl| {
            var_decl
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
                .filter_map(|node| ident_token_in_name(&node))
                .any(|ident| ident.text_range() == symbol_range)
        });
    if by_exact_ident.is_some() {
        return by_exact_ident;
    }

    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .find(|var_decl| {
            let range = var_decl.text_range();
            range.start() <= symbol_range.start() && range.end() >= symbol_range.end()
        })
}

fn find_var_decl_for_name(root: &SyntaxNode, name: &str) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .find(|var_decl| {
            var_decl
                .children()
                .filter(|node| node.kind() == SyntaxKind::Name)
                .filter_map(|node| ident_token_in_name(&node))
                .any(|ident| ident.text().eq_ignore_ascii_case(name))
        })
}

pub(crate) fn initializer_from_var_decl(source: &str, var_decl: &SyntaxNode) -> Option<String> {
    let expr = var_decl
        .children()
        .find(|node| is_expression_kind(node.kind()))?;
    let text = text_for_range(source, expr.text_range());
    (!text.is_empty()).then_some(text)
}

pub(crate) fn declared_type_from_var_decl(source: &str, var_decl: &SyntaxNode) -> Option<String> {
    let text = text_for_range(source, var_decl.text_range());
    let (_, rhs) = text.split_once(':')?;
    let rhs = rhs.trim();
    let rhs = rhs.split_once(":=").map_or(rhs, |(before, _)| before);
    let rhs = rhs.split_once(';').map_or(rhs, |(before, _)| before);
    let rhs = rhs.trim();
    (!rhs.is_empty()).then_some(rhs.to_string())
}

fn retention_from_var_block(block: &SyntaxNode) -> Option<&'static str> {
    for token in block
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::KwRetain => return Some("RETAIN"),
            SyntaxKind::KwNonRetain => return Some("NON_RETAIN"),
            SyntaxKind::KwPersistent => return Some("PERSISTENT"),
            _ => {}
        }
    }
    None
}

fn text_for_range(source: &str, range: TextRange) -> String {
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    source
        .get(start..end)
        .map(|text| text.trim().to_string())
        .unwrap_or_default()
}

fn is_expression_kind(kind: SyntaxKind) -> bool {
    kind.is_initializer_expression_node()
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_syntax::parser::parse;

    fn var_decl_for_name(root: &SyntaxNode, name: &str) -> SyntaxNode {
        root.descendants()
            .filter(|node| node.kind() == SyntaxKind::VarDecl)
            .find(|decl| {
                decl.children()
                    .filter(|node| node.kind() == SyntaxKind::Name)
                    .filter_map(|node| ident_token_in_name(&node))
                    .any(|token| token.text().eq_ignore_ascii_case(name))
            })
            .expect("variable declaration exists")
    }

    #[test]
    fn declared_type_extracts_plain_and_initialized_declarations() {
        let source = r#"
PROGRAM Demo
VAR
    Command : ST_PumpCommand;
    SpeedSet : REAL := 0.0;
    SampleWindow : ARRAY[1..4] OF INT;
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let root = parsed.syntax();

        let command = var_decl_for_name(&root, "Command");
        let speed_set = var_decl_for_name(&root, "SpeedSet");
        let sample_window = var_decl_for_name(&root, "SampleWindow");

        assert_eq!(
            declared_type_from_var_decl(source, &command).as_deref(),
            Some("ST_PumpCommand")
        );
        assert_eq!(
            declared_type_from_var_decl(source, &speed_set).as_deref(),
            Some("REAL")
        );
        assert_eq!(
            declared_type_from_var_decl(source, &sample_window).as_deref(),
            Some("ARRAY[1..4] OF INT")
        );
    }

    #[test]
    fn var_decl_info_carries_declared_type_for_symbol_range() {
        let source = r#"
PROGRAM Demo
VAR
    Status : ST_PumpStatus;
END_VAR
END_PROGRAM
"#;
        let parsed = parse(source);
        let root = parsed.syntax();
        let status_decl = var_decl_for_name(&root, "Status");
        let symbol_range = status_decl
            .children()
            .find(|node| node.kind() == SyntaxKind::Name)
            .and_then(|node| ident_token_in_name(&node))
            .map(|token| token.text_range())
            .expect("symbol range");

        let info = var_decl_info_for_symbol(&root, source, symbol_range);
        assert_eq!(info.declared_type.as_deref(), Some("ST_PumpStatus"));
    }
}
