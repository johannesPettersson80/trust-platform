//! The project's own function blocks as My equipment sees them (specification 39, §21.9): each
//! block declared in the project's sources, not in a library, with the members a panel shows, the
//! settings it offers and the commands its interfaces declare.

use std::path::{Path, PathBuf};

use trust_hir::semantic::SemanticOutcome;
use trust_hir::symbols::{ParamDirection, Symbol, SymbolId, SymbolKind, VarQualifier, Visibility};
use trust_hir::types::TypeId;
use trust_runtime::hmi::catalogue::ValueKind;
use trust_syntax::parser::parse;
use trust_syntax::SyntaxKind;

use crate::handlers::commands::hmi_authoring::declarations::Declarations;

/// Where a member is declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Section {
    Input,
    Output,
    /// A `VAR` member another may read: declared `PUBLIC`.
    Public,
}

/// A member a panel may show.
#[derive(Debug, Clone)]
pub(super) struct Member {
    pub(super) name: String,
    pub(super) section: Section,
    /// The kinds of value it holds, as the catalogue names them.
    pub(super) kinds: Vec<ValueKind>,
    /// Its declared type's name, such as `REAL` or `INT`.
    pub(super) type_name: String,
    /// Declared `PUBLIC` in a `RETAIN` section: a setting (Plant §9).
    pub(super) setting: bool,
}

/// A method a button may call: declared by an interface the block implements, without inputs,
/// giving BOOL (specification 39, §21.2 item 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Command {
    pub(super) interface: String,
    pub(super) method: String,
}

/// One of the project's own function blocks.
#[derive(Debug, Clone)]
pub(super) struct OwnBlock {
    /// Its name as the program names it, with its namespace.
    pub(super) name: String,
    /// The block it extends, if any.
    pub(super) base: Option<String>,
    /// Its members and its bases', its own first.
    pub(super) members: Vec<Member>,
    pub(super) commands: Vec<Command>,
}

/// The project's own function blocks: declared under the project's folder and not in one of its
/// libraries (which may lie inside it).
pub(super) fn own_blocks(
    index: &Declarations,
    root: &Path,
    libraries: &[PathBuf],
) -> Vec<OwnBlock> {
    let own = |file: &PathBuf| {
        file.starts_with(root) && !libraries.iter().any(|library| file.starts_with(library))
    };
    let mut blocks: Vec<OwnBlock> = index
        .symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, SymbolKind::FunctionBlock))
        .filter(|symbol| {
            symbol
                .origin
                .and_then(|origin| index.source_paths.get(&origin.file_id))
                .is_some_and(own)
        })
        .map(|symbol| {
            let name = index.name(symbol.id);
            OwnBlock {
                base: index.base_of(&name),
                members: members(index, symbol.id),
                commands: commands(index, symbol.id, &name),
                name,
            }
        })
        .collect();
    blocks.sort_by(|a, b| a.name.cmp(&b.name));
    blocks
}

/// The block's members a panel may show, its own and then its bases', each name once.
fn members(index: &Declarations, block: SymbolId) -> Vec<Member> {
    let mut found: Vec<Member> = Vec::new();
    for owner in chain(index, block) {
        for symbol in index
            .symbols
            .iter()
            .filter(|symbol| symbol.parent == Some(owner))
        {
            let Some(section) = section(symbol) else {
                continue;
            };
            if found
                .iter()
                .any(|member| member.name.eq_ignore_ascii_case(&symbol.name))
            {
                continue;
            }
            let node = index.node(&symbol.name, &symbol.name, symbol.type_id, false, 0);
            found.push(Member {
                name: symbol.name.to_string(),
                section,
                kinds: node.kinds.clone(),
                type_name: node.type_name.clone(),
                setting: section == Section::Public && retained(index, symbol),
            });
        }
    }
    found
}

/// The block and the blocks it extends, nearest first.
fn chain(index: &Declarations, block: SymbolId) -> Vec<SymbolId> {
    let mut owners = vec![block];
    while let Some(next) = index
        .symbols
        .extends_reference(*owners.last().expect("one owner"))
        .and_then(|reference| match reference.outcome() {
            SemanticOutcome::Resolved(target) => Some(*target),
            _ => None,
        })
    {
        if owners.contains(&next) || owners.len() > 32 {
            break;
        }
        owners.push(next);
    }
    owners
}

/// Where a member is declared, for the members a panel may show.
fn section(symbol: &Symbol) -> Option<Section> {
    match &symbol.kind {
        SymbolKind::Parameter {
            direction: ParamDirection::In,
        } => Some(Section::Input),
        SymbolKind::Parameter {
            direction: ParamDirection::Out,
        } => Some(Section::Output),
        SymbolKind::Variable {
            qualifier: VarQualifier::Local,
        } if symbol.visibility == Visibility::Public => Some(Section::Public),
        _ => None,
    }
}

/// Whether a member is declared in a `RETAIN` section.
fn retained(index: &Declarations, symbol: &Symbol) -> bool {
    let Some(text) = symbol
        .origin
        .and_then(|origin| index.source_texts.get(&origin.file_id))
    else {
        return false;
    };
    let root = parse(text).syntax();
    root.descendants()
        .filter(|node| node.kind() == SyntaxKind::VarDecl)
        .find(|node| node.text_range().contains_range(symbol.range))
        .and_then(|declaration| declaration.parent())
        .is_some_and(|block| {
            block
                .children_with_tokens()
                .any(|element| element.kind() == SyntaxKind::KwRetain)
        })
}

/// The block's methods a button may call: declared by an interface it implements, without
/// inputs, giving BOOL.
fn commands(index: &Declarations, block: SymbolId, name: &str) -> Vec<Command> {
    let interfaces: Vec<(String, SymbolId)> = index
        .symbols
        .iter()
        .filter(|symbol| matches!(symbol.kind, SymbolKind::Interface))
        .map(|symbol| (index.name(symbol.id), symbol.id))
        .filter(|(interface, _)| index.implements_capability(name, interface))
        .collect();
    let mut found = Vec::new();
    for owner in chain(index, block) {
        for method in index
            .symbols
            .iter()
            .filter(|symbol| symbol.parent == Some(owner))
        {
            let SymbolKind::Method { return_type, .. } = &method.kind else {
                continue;
            };
            let takes_inputs = index.symbols.iter().any(|symbol| {
                symbol.parent == Some(method.id)
                    && matches!(
                        symbol.kind,
                        SymbolKind::Parameter {
                            direction: ParamDirection::In | ParamDirection::InOut
                        }
                    )
            });
            if takes_inputs
                || *return_type != Some(TypeId::BOOL)
                || found
                    .iter()
                    .any(|command: &Command| command.method.eq_ignore_ascii_case(&method.name))
            {
                continue;
            }
            let declaring = interfaces.iter().find(|(_, interface)| {
                index.symbols.iter().any(|symbol| {
                    symbol.parent == Some(*interface)
                        && matches!(symbol.kind, SymbolKind::Method { .. })
                        && symbol.name.eq_ignore_ascii_case(&method.name)
                })
            });
            if let Some((interface, _)) = declaring {
                found.push(Command {
                    interface: interface.clone(),
                    method: method.name.to_string(),
                });
            }
        }
    }
    found
}
