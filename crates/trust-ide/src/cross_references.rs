//! The cross-reference list of a symbol (specification 14 §6.3.2): its declaration and every
//! reference, each with its access and the POU it sits in, as CODESYS's cross-reference list shows.

use rustc_hash::FxHashMap;
use smol_str::SmolStr;
use text_size::{TextRange, TextSize};
use trust_hir::access::Access;
use trust_hir::db::{FileId, SourceDatabase};
use trust_hir::Database;
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

use crate::references::{find_references, FindReferencesOptions};

/// How an entry of a cross-reference list reaches the symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrossReferenceKind {
    /// The symbol's declaration.
    Declaration,
    /// A use, read, written or called.
    Use(Access),
}

/// One entry of a cross-reference list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrossReference {
    /// The file of the entry.
    pub file_id: FileId,
    /// The range of the entry's name.
    pub range: TextRange,
    /// The declaration, or a use with its access.
    pub kind: CrossReferenceKind,
    /// The qualified name of the namespace, POU, method or action the entry sits in
    /// (`Plant.FB_Tank.Start`), if any.
    pub pou: Option<String>,
}

/// The cross-reference list of the symbol at `position` in `file_id`: its name, and its
/// declaration and references ordered by file and offset. `None` when no symbol is there.
#[must_use]
pub fn cross_references(
    db: &Database,
    file_id: FileId,
    position: TextSize,
) -> Option<(SmolStr, Vec<CrossReference>)> {
    let with_declaration = find_references(
        db,
        file_id,
        position,
        FindReferencesOptions {
            include_declaration: true,
        },
    );
    if with_declaration.is_empty() {
        return None;
    }
    let uses = find_references(
        db,
        file_id,
        position,
        FindReferencesOptions {
            include_declaration: false,
        },
    );
    let mut roots: FxHashMap<FileId, SyntaxNode> = FxHashMap::default();
    let mut root = |file: FileId| {
        roots
            .entry(file)
            .or_insert_with(|| parse(&db.source_text(file)).syntax())
            .clone()
    };
    let mut entries: Vec<CrossReference> = with_declaration
        .iter()
        .map(|reference| {
            let kind = uses
                .iter()
                .find(|used| used.file_id == reference.file_id && used.range == reference.range)
                .map_or(CrossReferenceKind::Declaration, |used| {
                    CrossReferenceKind::Use(used.access)
                });
            CrossReference {
                file_id: reference.file_id,
                range: reference.range,
                kind,
                pou: pou_path(&root(reference.file_id), reference.range.start()),
            }
        })
        .collect();
    entries.sort_by_key(|entry| (entry.file_id.0, entry.range.start()));

    let named = entries
        .iter()
        .find(|entry| entry.kind == CrossReferenceKind::Declaration)
        .unwrap_or(&entries[0]);
    let text = db.source_text(named.file_id);
    let name =
        SmolStr::new(text.get(usize::from(named.range.start())..usize::from(named.range.end()))?);
    Some((name, entries))
}

/// The names of the namespaces, POUs, methods and actions around `offset`, outermost first, joined
/// by dots.
fn pou_path(root: &SyntaxNode, offset: TextSize) -> Option<String> {
    let token = root.token_at_offset(offset).right_biased()?;
    let mut names: Vec<String> = token
        .parent_ancestors()
        .filter(|node| {
            matches!(
                node.kind(),
                SyntaxKind::Namespace
                    | SyntaxKind::Program
                    | SyntaxKind::Function
                    | SyntaxKind::FunctionBlock
                    | SyntaxKind::Class
                    | SyntaxKind::Interface
                    | SyntaxKind::Method
                    | SyntaxKind::Property
                    | SyntaxKind::Action
            )
        })
        .filter_map(|node| {
            let name = node.children().find(|child| {
                matches!(child.kind(), SyntaxKind::Name | SyntaxKind::QualifiedName)
            })?;
            let text: String = name
                .descendants_with_tokens()
                .filter_map(|element| element.into_token())
                .filter(|token| !token.kind().is_trivia())
                .map(|token| token.text().to_owned())
                .collect();
            (!text.is_empty()).then_some(text)
        })
        .collect();
    names.reverse();
    (!names.is_empty()).then(|| names.join("."))
}

#[cfg(test)]
#[path = "cross_references/contract_tests.rs"]
mod contract_tests;
