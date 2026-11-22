//! What one file's analysis imports from the other files of its project (specification 14
//! §10.2): the declarations the file names, and the declarations that theirs name, never a copy of
//! the whole project.
//!
//! A declaration another file can import is a *unit*: a declaration at the top of its file or
//! directly inside a namespace. A file selects a unit when it writes one of the unit's names: the
//! unit's own name, or a name the unit makes visible everywhere (an enumeration's values, the
//! global variables and constants of a configuration or resource). A selected unit selects in
//! turn the units its declaration names outside statement bodies, so that the types of its
//! members, its bases and its interfaces come with it. Namespaces are always imported, so that
//! `USING` and qualified names find them.

use super::*;

/// The units one file declares, by symbol in the file's own table.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct FileUnits {
    units: Vec<Unit>,
}

#[derive(Debug, PartialEq, Eq)]
struct Unit {
    symbol: SymbolId,
    /// The names that select the unit, in upper case.
    keys: Vec<SmolStr>,
    /// The names the unit's declaration writes outside statement bodies, in upper case.
    declaration_names: Vec<SmolStr>,
}

/// Every unit of the project by the names that select it.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct UnitIndex {
    by_name: FxHashMap<SmolStr, Vec<(FileId, SymbolId)>>,
    declaration_names: FxHashMap<(FileId, SymbolId), Vec<SmolStr>>,
}

/// The units of the other files that one file's analysis imports.
pub(in crate::db) type UnitSelection = FxHashSet<(FileId, SymbolId)>;

/// The topmost declaration above `id` that is not a namespace (`id` itself included): the unit
/// a symbol is imported with. `None` for a namespace.
pub(in crate::db) fn unit_of(
    table: &SymbolTable,
    id: SymbolId,
    cache: &mut FxHashMap<SymbolId, Option<SymbolId>>,
) -> Option<SymbolId> {
    if let Some(unit) = cache.get(&id) {
        return *unit;
    }
    let mut unit = None;
    let mut current = table.get(id);
    while let Some(symbol) = current {
        if !matches!(symbol.kind, SymbolKind::Namespace) {
            unit = Some(symbol.id);
        }
        current = symbol.parent.and_then(|parent| table.get(parent));
    }
    cache.insert(id, unit);
    unit
}

pub(super) fn file_units(table: &SymbolTable, root: &SyntaxNode) -> FileUnits {
    let mut cache = FxHashMap::default();
    let mut keys: FxHashMap<SymbolId, Vec<SmolStr>> = FxHashMap::default();
    for symbol in table.iter_in_id_order() {
        let Some(unit_id) = unit_of(table, symbol.id, &mut cache) else {
            continue;
        };
        let Some(unit) = table.get(unit_id) else {
            continue;
        };
        if unit.range.is_empty() {
            continue;
        }
        if unit_id == symbol.id || makes_name_visible_everywhere(table, symbol) {
            keys.entry(unit_id)
                .or_default()
                .push(SmolStr::new(symbol.name.to_ascii_uppercase()));
        }
    }
    let mut units: Vec<Unit> = keys
        .into_iter()
        .map(|(symbol, keys)| {
            let declaration_names = table
                .get(symbol)
                .and_then(|unit| declaration_node(root, unit.range))
                .map(|node| declaration_names(&node))
                .unwrap_or_default();
            Unit {
                symbol,
                keys,
                declaration_names,
            }
        })
        .collect();
    units.sort_by_key(|unit| unit.symbol.0);
    FileUnits { units }
}

/// An enumeration's value, or a configuration's or resource's global variable or constant: the
/// importer defines these in the global or namespace scope (`symbol_import`), so a file reaches
/// them by their own names.
fn makes_name_visible_everywhere(table: &SymbolTable, symbol: &Symbol) -> bool {
    let Some(parent) = symbol.parent.and_then(|parent| table.get(parent)) else {
        return false;
    };
    matches!(
        (&symbol.kind, &parent.kind),
        (SymbolKind::EnumValue { .. }, SymbolKind::Type)
            | (
                SymbolKind::Variable {
                    qualifier: VarQualifier::Global | VarQualifier::Access,
                } | SymbolKind::Constant,
                SymbolKind::Configuration | SymbolKind::Resource,
            )
    )
}

/// The declaration node of a unit whose name is at `name_range`: the node below the file or the
/// namespace that holds the name.
fn declaration_node(root: &SyntaxNode, name_range: TextRange) -> Option<SyntaxNode> {
    let token = root.token_at_offset(name_range.start()).right_biased()?;
    token.parent_ancestors().find(|node| {
        node.parent().is_some_and(|parent| {
            matches!(
                parent.kind(),
                SyntaxKind::SourceFile | SyntaxKind::Namespace
            )
        })
    })
}

/// The names a declaration writes outside its statement bodies, in upper case.
fn declaration_names(node: &SyntaxNode) -> Vec<SmolStr> {
    let mut names = FxHashSet::default();
    let mut walk = node.preorder_with_tokens();
    while let Some(event) = walk.next() {
        let rowan::WalkEvent::Enter(element) = event else {
            continue;
        };
        match element {
            rowan::NodeOrToken::Node(node) if node.kind() == SyntaxKind::StmtList => {
                walk.skip_subtree();
            }
            rowan::NodeOrToken::Token(token) if token.kind() == SyntaxKind::Ident => {
                names.insert(SmolStr::new(token.text().to_ascii_uppercase()));
            }
            _ => {}
        }
    }
    let mut names: Vec<SmolStr> = names.into_iter().collect();
    names.sort();
    names
}

/// Every name a file writes, in upper case: its identifiers, and the identifier-like words of its
/// literals and pragmas (an attribute may name a declaration in a string). Comments name nothing.
pub(super) fn written_names(root: &SyntaxNode) -> FxHashSet<SmolStr> {
    let mut names = FxHashSet::default();
    for token in root
        .descendants_with_tokens()
        .filter_map(|element| element.into_token())
    {
        match token.kind() {
            SyntaxKind::Ident => {
                names.insert(SmolStr::new(token.text().to_ascii_uppercase()));
            }
            SyntaxKind::Whitespace | SyntaxKind::LineComment | SyntaxKind::BlockComment => {}
            _ => {
                for word in identifier_words(token.text()) {
                    names.insert(SmolStr::new(word.to_ascii_uppercase()));
                }
            }
        }
    }
    names
}

fn identifier_words(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| {
            word.chars()
                .next()
                .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        })
}

pub(super) fn build_unit_index(files: &[(FileId, Arc<FileUnits>)]) -> UnitIndex {
    let mut index = UnitIndex::default();
    for (file_id, units) in files {
        for unit in &units.units {
            for key in &unit.keys {
                index
                    .by_name
                    .entry(key.clone())
                    .or_default()
                    .push((*file_id, unit.symbol));
            }
            index
                .declaration_names
                .insert((*file_id, unit.symbol), unit.declaration_names.clone());
        }
    }
    index
}

/// Every unit of the other files: what a file whose analysis follows the code of every file
/// imports (the owners of the globals it declares, specification 28 §6).
pub(super) fn select_all_units(file_id: FileId, index: &UnitIndex) -> UnitSelection {
    index
        .declaration_names
        .keys()
        .filter(|(unit_file, _)| *unit_file != file_id)
        .copied()
        .collect()
}

/// The units of the other files that `file_id` names, directly or through the declarations of the
/// units it names.
pub(super) fn select_units(
    file_id: FileId,
    names: FxHashSet<SmolStr>,
    index: &UnitIndex,
) -> UnitSelection {
    let mut selected = UnitSelection::default();
    let mut seen: FxHashSet<SmolStr> = FxHashSet::default();
    let mut pending: Vec<SmolStr> = names.into_iter().collect();
    while let Some(name) = pending.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(units) = index.by_name.get(&name) else {
            continue;
        };
        for &(unit_file, unit) in units {
            if unit_file == file_id || !selected.insert((unit_file, unit)) {
                continue;
            }
            if let Some(declared) = index.declaration_names.get(&(unit_file, unit)) {
                pending.extend(
                    declared
                        .iter()
                        .filter(|name| !seen.contains(*name))
                        .cloned(),
                );
            }
        }
    }
    selected
}
