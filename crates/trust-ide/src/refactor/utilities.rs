use smol_str::SmolStr;
use text_size::{TextRange, TextSize};

use trust_hir::symbols::{SymbolKind, SymbolTable};
use trust_hir::{is_reserved_keyword, is_valid_identifier, SymbolId};
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::util::namespace_path_for_symbol;

pub(super) fn parse_namespace_path(path: &str) -> Option<Vec<SmolStr>> {
    let mut parts = Vec::new();
    for part in path.split('.') {
        if part.is_empty() {
            return None;
        }
        if !is_valid_identifier(part) || is_reserved_keyword(part) {
            return None;
        }
        parts.push(SmolStr::new(part));
    }
    if parts.is_empty() {
        None
    } else {
        Some(parts)
    }
}

pub(super) fn namespace_full_path(
    symbols: &SymbolTable,
    symbol_id: SymbolId,
) -> Option<Vec<SmolStr>> {
    let symbol = symbols.get(symbol_id)?;
    if !matches!(symbol.kind, SymbolKind::Namespace) {
        return None;
    }
    let mut parts = namespace_path_for_symbol(symbols, symbol);
    parts.push(symbol.name.clone());
    Some(parts)
}

pub(super) fn symbol_qualified_name(symbols: &SymbolTable, symbol_id: SymbolId) -> Option<String> {
    let symbol = symbols.get(symbol_id)?;
    let mut parts = namespace_path_for_symbol(symbols, symbol);
    parts.push(symbol.name.clone());
    Some(join_namespace_path(&parts))
}

pub(super) fn indent_unit_for(indent: &str) -> &str {
    if indent.contains('\t') {
        "\t"
    } else {
        "    "
    }
}

pub(super) fn reindent_block(block: &str, indent: &str) -> String {
    block
        .lines()
        .map(|line| {
            if line.trim().is_empty() {
                String::new()
            } else {
                format!("{indent}{}", line.trim_start())
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn line_indent_at_offset(source: &str, offset: TextSize) -> String {
    let offset = usize::from(offset);
    let bytes = source.as_bytes();
    let mut line_start = offset;
    while line_start > 0 {
        let b = bytes[line_start - 1];
        if b == b'\n' || b == b'\r' {
            break;
        }
        line_start -= 1;
    }
    let mut end = line_start;
    while end < bytes.len() && (bytes[end] == b' ' || bytes[end] == b'\t') {
        end += 1;
    }
    source[line_start..end].to_string()
}

pub(super) fn trim_range_to_non_whitespace(source: &str, range: TextRange) -> Option<TextRange> {
    let mut start = usize::from(range.start());
    let mut end = usize::from(range.end());
    if start >= end || start >= source.len() {
        return None;
    }
    end = end.min(source.len());
    let bytes = source.as_bytes();
    while start < end && bytes[start].is_ascii_whitespace() {
        start += 1;
    }
    while end > start && bytes[end - 1].is_ascii_whitespace() {
        end -= 1;
    }

    if start >= end {
        None
    } else {
        Some(TextRange::new(
            TextSize::from(start as u32),
            TextSize::from(end as u32),
        ))
    }
}

pub(super) fn range_contains(outer: TextRange, inner: TextRange) -> bool {
    outer.start() <= inner.start() && outer.end() >= inner.end()
}

pub(super) fn ranges_overlap(a: TextRange, b: TextRange) -> bool {
    a.start() < b.end() && b.start() < a.end()
}

pub(super) fn text_for_range(source: &str, range: TextRange) -> String {
    let start: usize = range.start().into();
    let end: usize = range.end().into();
    source
        .get(start..end)
        .map(|text| text.trim().to_string())
        .unwrap_or_default()
}

pub(super) fn qualified_name_parts(node: &SyntaxNode) -> Vec<SmolStr> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| token.kind() == SyntaxKind::Ident)
        .map(|token| SmolStr::new(token.text()))
        .collect()
}

pub(super) fn path_eq_ignore_ascii_case(a: &[SmolStr], b: &[SmolStr]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter()
        .zip(b.iter())
        .all(|(left, right)| left.eq_ignore_ascii_case(right.as_str()))
}

pub(super) fn path_starts_with_ignore_ascii_case(path: &[SmolStr], prefix: &[SmolStr]) -> bool {
    if path.len() < prefix.len() {
        return false;
    }
    path.iter()
        .zip(prefix.iter())
        .all(|(left, right)| left.eq_ignore_ascii_case(right.as_str()))
}

pub(super) fn join_namespace_path(parts: &[SmolStr]) -> String {
    let mut out = String::new();
    for (idx, part) in parts.iter().enumerate() {
        if idx > 0 {
            out.push('.');
        }
        out.push_str(part.as_str());
    }
    out
}

pub(super) fn node_token_range(node: &SyntaxNode) -> TextRange {
    let mut first = None;
    let mut last = None;
    for token in node
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        if token.kind().is_trivia() {
            continue;
        }
        if first.is_none() {
            first = Some(token.clone());
        }
        last = Some(token);
    }
    match (first, last) {
        (Some(first), Some(last)) => {
            TextRange::new(first.text_range().start(), last.text_range().end())
        }
        _ => node.text_range(),
    }
}
