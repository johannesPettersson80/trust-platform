//! The Rust source of a Rust POU for the tools (specification 48 RU-31, RU-50): Go to Definition of
//! a Rust POU opens its Rust item in its `.rs` file, from its attribute to its name, where the
//! build's reader of the project's crate finds it. The POU's declaration in the generated file
//! stays what Go to Declaration opens.

use std::path::{Path, PathBuf};

use text_size::TextRange;
use tower_lsp::lsp_types::{Location, Position, Range, Url};
use trust_runtime::native::rust_pous::{self, ItemSpan, DECLARATIONS_FILE};
use trust_syntax::parser::parse;
use trust_syntax::syntax::{SyntaxKind, SyntaxNode};

/// The Rust item of the Rust POU whose name lies at `range` of the generated declarations at
/// `uri`, whose text is `content`; `None` for any other target, and for a crate the reader
/// refuses, whose declarations the build refuses too.
pub(super) fn rust_item(uri: &Url, content: &str, range: TextRange) -> Option<Location> {
    let path = uri.to_file_path().ok()?;
    let root = project_of(&path)?;
    let name = rust_pou_named_at(content, range)?;
    let krate = rust_pous::read(&root).ok().flatten()?;
    let pou = krate
        .pous
        .iter()
        .find(|pou| pou.pou.name.eq_ignore_ascii_case(&name))?;
    let uri = Url::from_file_path(root.join(&pou.file)).ok()?;
    Some(Location {
        uri,
        range: span_range(pou.item),
    })
}

/// The project whose generated declarations lie at `path`.
fn project_of(path: &Path) -> Option<PathBuf> {
    let mut root = path.to_path_buf();
    for _ in Path::new(DECLARATIONS_FILE).components() {
        if !root.pop() {
            return None;
        }
    }
    (root.join(DECLARATIONS_FILE) == path).then_some(root)
}

/// The full name, its namespaces' and its own, of the Rust POU whose own name lies at `range`.
fn rust_pou_named_at(content: &str, range: TextRange) -> Option<String> {
    let root = parse(content).syntax();
    let pou = root.descendants().find(|node| {
        trust_hir::rust_pous::is_rust_pou(node)
            && name_of(node).is_some_and(|name| name.text_range().contains(range.start()))
    })?;
    let mut parts: Vec<String> = pou
        .ancestors()
        .filter(|node| node.kind() == SyntaxKind::Namespace)
        .filter_map(|namespace| name_of(&namespace))
        .map(|name| name.text().to_string().trim().to_string())
        .collect();
    parts.reverse();
    parts.push(name_of(&pou)?.text().to_string().trim().to_string());
    Some(parts.join("."))
}

/// The name a declaration `node` declares.
fn name_of(node: &SyntaxNode) -> Option<SyntaxNode> {
    node.children()
        .find(|child| child.kind() == SyntaxKind::Name)
}

/// The editor's range of `span`: its lines from 0, its columns as the compiler counts them, in
/// characters, which are the editor's UTF-16 units for every character of the Basic Multilingual
/// Plane, all an attribute and a name hold.
fn span_range(span: ItemSpan) -> Range {
    let position = |(line, column): (usize, usize)| Position {
        line: u32::try_from(line.saturating_sub(1)).unwrap_or(u32::MAX),
        character: u32::try_from(column).unwrap_or(u32::MAX),
    };
    Range {
        start: position(span.start),
        end: position(span.end),
    }
}
