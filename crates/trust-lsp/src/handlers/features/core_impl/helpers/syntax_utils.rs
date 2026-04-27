//! Responsibility: focused helper module in the core LSP feature layer.
use super::*;

pub(in super::super) fn find_case_stmt_for_range(
    root: &SyntaxNode,
    range: TextRange,
) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::CaseStmt)
        .filter(|node| {
            let node_range = node.text_range();
            node_range.contains(range.start()) && node_range.contains(range.end())
        })
        .min_by_key(|node| node.text_range().len())
}

pub(in super::super) fn line_start_offset(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    match source[..offset].rfind('\n') {
        Some(pos) => pos + 1,
        None => 0,
    }
}

pub(in super::super) fn indent_at_offset(source: &str, offset: usize) -> String {
    let line_start = line_start_offset(source, offset);
    let bytes = source.as_bytes();
    let mut end = line_start;
    while end < bytes.len() {
        match bytes[end] {
            b' ' | b'\t' => end += 1,
            _ => break,
        }
    }
    source[line_start..end].to_string()
}

pub(in super::super) fn is_foldable_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Program
            | SyntaxKind::Function
            | SyntaxKind::FunctionBlock
            | SyntaxKind::Method
            | SyntaxKind::Property
            | SyntaxKind::PropertyGet
            | SyntaxKind::PropertySet
            | SyntaxKind::Interface
            | SyntaxKind::Namespace
            | SyntaxKind::Action
            | SyntaxKind::TypeDecl
            | SyntaxKind::StructDef
            | SyntaxKind::UnionDef
            | SyntaxKind::EnumDef
            | SyntaxKind::VarBlock
            | SyntaxKind::StmtList
            | SyntaxKind::IfStmt
            | SyntaxKind::CaseStmt
            | SyntaxKind::CaseBranch
            | SyntaxKind::ForStmt
            | SyntaxKind::WhileStmt
            | SyntaxKind::RepeatStmt
    )
}

pub(in super::super) fn selection_range_to_lsp(
    source: &str,
    range: trust_ide::SelectionRange,
) -> SelectionRange {
    let parent = range
        .parent
        .map(|parent| Box::new(selection_range_to_lsp(source, *parent)));
    SelectionRange {
        range: Range {
            start: offset_to_position(source, range.range.start().into()),
            end: offset_to_position(source, range.range.end().into()),
        },
        parent,
    }
}

pub(in super::super) fn newline_for_source(source: &str) -> &'static str {
    if source.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

pub(in super::super) fn infer_indent_unit(source: &str) -> String {
    let mut min_spaces = usize::MAX;
    for line in source.lines() {
        if line.starts_with('\t') {
            return "\t".to_string();
        }
        let spaces = line.chars().take_while(|c| *c == ' ').count();
        if spaces > 0 {
            min_spaces = min_spaces.min(spaces);
        }
    }
    if min_spaces != usize::MAX {
        return " ".repeat(min_spaces);
    }
    "    ".to_string()
}

pub(in super::super) fn line_end_offset(source: &str, offset: usize) -> usize {
    let offset = offset.min(source.len());
    match source[offset..].find('\n') {
        Some(pos) => offset + pos + 1,
        None => source.len(),
    }
}

pub(in super::super) fn position_leq(a: Position, b: Position) -> bool {
    a.line < b.line || (a.line == b.line && a.character <= b.character)
}

pub(in super::super) fn ranges_intersect(a: Range, b: Range) -> bool {
    position_leq(a.start, b.end) && position_leq(b.start, a.end)
}

pub(in super::super) fn expected_end_keyword(message: &str) -> Option<&str> {
    let rest = message.strip_prefix("expected ")?;
    let token = rest.split_whitespace().next()?;
    if token.starts_with("END_") {
        Some(token)
    } else {
        None
    }
}

pub(in super::super) fn node_kind_for_end_keyword(keyword: &str) -> Option<SyntaxKind> {
    match keyword {
        "END_IF" => Some(SyntaxKind::IfStmt),
        "END_CASE" => Some(SyntaxKind::CaseStmt),
        "END_FOR" => Some(SyntaxKind::ForStmt),
        "END_WHILE" => Some(SyntaxKind::WhileStmt),
        "END_REPEAT" => Some(SyntaxKind::RepeatStmt),
        "END_PROGRAM" => Some(SyntaxKind::Program),
        "END_FUNCTION" => Some(SyntaxKind::Function),
        "END_FUNCTION_BLOCK" => Some(SyntaxKind::FunctionBlock),
        "END_CLASS" => Some(SyntaxKind::Class),
        "END_INTERFACE" => Some(SyntaxKind::Interface),
        "END_NAMESPACE" => Some(SyntaxKind::Namespace),
        "END_TYPE" => Some(SyntaxKind::TypeDecl),
        "END_VAR" => Some(SyntaxKind::VarBlock),
        "END_METHOD" => Some(SyntaxKind::Method),
        "END_PROPERTY" => Some(SyntaxKind::Property),
        "END_ACTION" => Some(SyntaxKind::Action),
        "END_CONFIGURATION" => Some(SyntaxKind::Configuration),
        "END_RESOURCE" => Some(SyntaxKind::Resource),
        _ => None,
    }
}

pub(in super::super) fn find_enclosing_node_of_kind(
    root: &SyntaxNode,
    range: TextRange,
    kind: SyntaxKind,
) -> Option<SyntaxNode> {
    root.descendants()
        .filter(|node| node.kind() == kind)
        .filter(|node| {
            let node_range = node.text_range();
            node_range.contains(range.start()) && node_range.contains(range.end())
        })
        .min_by_key(|node| node.text_range().len())
}

pub(in super::super) fn is_execution_param(name: &str) -> bool {
    name.eq_ignore_ascii_case("EN") || name.eq_ignore_ascii_case("ENO")
}

pub(in super::super) fn is_expression_kind(kind: SyntaxKind) -> bool {
    kind.is_initializer_expression_node()
}

pub(in super::super) fn is_pou_kind(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::Program
            | SyntaxKind::Function
            | SyntaxKind::FunctionBlock
            | SyntaxKind::Class
            | SyntaxKind::Method
            | SyntaxKind::Property
            | SyntaxKind::Interface
    )
}
