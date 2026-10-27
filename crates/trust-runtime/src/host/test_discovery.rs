//! The test declarations of a project's Structured Text (specification 22, "Test POU discovery";
//! specification 25 §11.1): every `TEST_PROGRAM` and `TEST_FUNCTION_BLOCK`, with its kind, its
//! namespace-qualified name, its file, its one-based declaration line and that line's text.
//!
//! `trust-dev test` runs what this finds, and the VS Code extension's Testing view and its Run Test
//! lenses show it, so a test has one name wherever it appears.

use std::path::{Path, PathBuf};

use smol_str::SmolStr;
use trust_syntax::parser;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode, SyntaxToken};

/// The kind of a test declaration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestKind {
    /// `TEST_PROGRAM`.
    Program,
    /// `TEST_FUNCTION_BLOCK`.
    FunctionBlock,
}

impl TestKind {
    /// The keyword that declares the kind: `TEST_PROGRAM` or `TEST_FUNCTION_BLOCK`.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Program => "TEST_PROGRAM",
            Self::FunctionBlock => "TEST_FUNCTION_BLOCK",
        }
    }
}

/// One test declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredTest {
    /// Its kind.
    pub kind: TestKind,
    /// Its name, qualified by the namespaces around it (`Ns.Inner.Test`).
    pub name: SmolStr,
    /// The file that declares it.
    pub file: PathBuf,
    /// The byte offset of its declaring keyword in the file.
    pub byte_offset: u32,
    /// The one-based line of its declaring keyword.
    pub line: usize,
    /// The trimmed text of that line, unless it is empty.
    pub source_line: Option<String>,
}

/// The test declarations of `sources` (each a file and its text), ordered by file, declaration
/// offset and name, so that the same sources always give the same list.
pub fn discover_tests<'a>(
    sources: impl IntoIterator<Item = (&'a Path, &'a str)>,
) -> Vec<DiscoveredTest> {
    let mut tests = Vec::new();
    for (path, text) in sources {
        let parse = parser::parse(text);
        let syntax = parse.syntax();
        for node in syntax.descendants() {
            let kind = match node.kind() {
                SyntaxKind::Program | SyntaxKind::FunctionBlock => test_kind_for_node(&node),
                _ => None,
            };
            let Some(kind) = kind else {
                continue;
            };
            let Some(name) = qualified_pou_name(&node) else {
                continue;
            };
            let byte_offset = node
                .children_with_tokens()
                .filter_map(|element| element.into_token())
                .find(|token| !token.kind().is_trivia())
                .map_or_else(
                    || u32::from(node.text_range().start()),
                    |token| u32::from(token.text_range().start()),
                );
            tests.push(DiscoveredTest {
                kind,
                name,
                file: path.to_path_buf(),
                byte_offset,
                line: line_for_offset(text, byte_offset as usize),
                source_line: source_line_for_offset(text, byte_offset as usize),
            });
        }
    }
    tests.sort_by(|left, right| {
        left.file
            .cmp(&right.file)
            .then(left.byte_offset.cmp(&right.byte_offset))
            .then(left.name.cmp(&right.name))
    });
    tests
}

fn test_kind_for_node(node: &SyntaxNode) -> Option<TestKind> {
    let first_token = node
        .children_with_tokens()
        .filter_map(|element| element.into_token())
        .find(|token| !token.kind().is_trivia())?;
    match first_token.kind() {
        SyntaxKind::KwTestProgram => Some(TestKind::Program),
        SyntaxKind::KwTestFunctionBlock => Some(TestKind::FunctionBlock),
        _ => None,
    }
}

/// The name of a test POU with the namespaces around it, outermost first. A dotted namespace name,
/// `NAMESPACE NS.Core`, counts with every part: `NS.Core.Test`.
fn qualified_pou_name(node: &SyntaxNode) -> Option<SmolStr> {
    let mut parts = Vec::new();
    let name_node = node
        .children()
        .find(|child| child.kind() == SyntaxKind::Name)?;
    parts.push(name_part_from_name_node(&name_node)?);
    for ancestor in node.ancestors() {
        if ancestor.kind() != SyntaxKind::Namespace {
            continue;
        }
        if let Some(namespace) = ancestor
            .children()
            .find(|child| matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName))
            .and_then(|name_node| namespace_path(&name_node))
        {
            parts.push(namespace);
        }
    }
    parts.reverse();
    Some(parts.join(".").into())
}

fn name_part_from_name_node(node: &SyntaxNode) -> Option<String> {
    let text = ident_tokens(node).next()?.text().trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// Every name part of a namespace's (possibly dotted) name, joined by dots.
fn namespace_path(node: &SyntaxNode) -> Option<String> {
    let parts: Vec<String> = ident_tokens(node)
        .map(|token| token.text().trim().to_string())
        .filter(|part| !part.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("."))
}

fn ident_tokens(node: &SyntaxNode) -> impl Iterator<Item = SyntaxToken> {
    node.descendants_with_tokens()
        .filter_map(|element| element.into_token())
        .filter(|token| {
            matches!(
                token.kind(),
                SyntaxKind::Ident | SyntaxKind::KwEn | SyntaxKind::KwEno
            )
        })
}

fn line_for_offset(text: &str, byte_offset: usize) -> usize {
    let offset = byte_offset.min(text.len());
    text[..offset].bytes().filter(|byte| *byte == b'\n').count() + 1
}

fn source_line_for_offset(text: &str, byte_offset: usize) -> Option<String> {
    let offset = byte_offset.min(text.len());
    let line_start = text[..offset].rfind('\n').map_or(0, |index| index + 1);
    let line_end = text[offset..]
        .find('\n')
        .map_or(text.len(), |relative| offset + relative);
    let line = text[line_start..line_end].trim();
    (!line.is_empty()).then(|| line.to_string())
}

#[cfg(test)]
#[path = "test_discovery_tests.rs"]
mod tests;
