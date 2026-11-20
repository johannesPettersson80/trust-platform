//! The located declarations of a source, read from its syntax once per revision, for W017
//! (specification 28 §11): each variable declared `AT` a fully specified flat address of `%I`, `%Q`
//! or `%M`, and each `VAR_CONFIG` entry that gives one, with the bits its address names.
use super::helpers::{config_init_direct_address, name_from_node, var_decl_direct_address};
use super::salsa_backend::{parse_green, SourceInput};
use super::*;

/// A process-image area: `%I`, `%Q` or `%M`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(in crate::db) enum LocatedArea {
    Input,
    Output,
    Memory,
}

/// One located declaration and the storage its address names.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(in crate::db) struct LocatedStorage {
    /// The declared name, or the `VAR_CONFIG` entry's access path.
    pub(in crate::db) name: SmolStr,
    /// The address as written.
    pub(in crate::db) address: SmolStr,
    pub(in crate::db) area: LocatedArea,
    /// The first bit it takes and the bit after its last, counted from bit 0 of the area's byte 0.
    pub(in crate::db) bits: (u64, u64),
    /// Where a warning about it goes: the declared name, or the entry's access path.
    pub(in crate::db) range: TextRange,
}

/// The located declarations of a source, and how many `RESOURCE` declarations it has.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::db) struct SourceLocatedStorage {
    pub(in crate::db) declarations: Vec<LocatedStorage>,
    pub(in crate::db) resources: usize,
}

#[salsa::tracked(returns(ref))]
pub(super) fn source_located_storage(
    db: &dyn salsa::Database,
    input: SourceInput,
) -> Arc<SourceLocatedStorage> {
    let root = SyntaxNode::new_root(parse_green(db, input).clone());
    let mut found = SourceLocatedStorage::default();
    for node in root.descendants() {
        match node.kind() {
            SyntaxKind::Resource => found.resources += 1,
            // A member of a structure is no variable of the process image: a `STRUCT OVERLAP`
            // member's address is relative to its structure.
            SyntaxKind::VarDecl if !node.ancestors().any(|n| n.kind() == SyntaxKind::TypeDecl) => {
                let Some(address) = var_decl_direct_address(&node) else {
                    continue;
                };
                let Some((area, bits)) = flat_storage(&address) else {
                    continue;
                };
                for child in node.children().filter(|n| n.kind() == SyntaxKind::Name) {
                    if let Some((name, range)) = name_from_node(&child) {
                        found.declarations.push(LocatedStorage {
                            name,
                            address: address.clone(),
                            area,
                            bits,
                            range,
                        });
                    }
                }
            }
            SyntaxKind::ConfigInit => {
                let Some(address) = config_init_direct_address(&node) else {
                    continue;
                };
                let Some((area, bits)) = flat_storage(&address) else {
                    continue;
                };
                let Some(path) = node.children().find(|n| n.kind() == SyntaxKind::AccessPath)
                else {
                    continue;
                };
                let name: String = path.text().to_string().split_whitespace().collect();
                found.declarations.push(LocatedStorage {
                    name: SmolStr::new(name),
                    address,
                    area,
                    bits,
                    range: path.text_range(),
                });
            }
            _ => {}
        }
    }
    Arc::new(found)
}

/// The located declarations of every source of the project, by file in id order, and whether the
/// project has more than one `RESOURCE` (each has a process image of its own: specification 11
/// §6.4).
pub(super) fn project_located_storage(
    db: &dyn salsa::Database,
    files: &[(FileId, SourceInput)],
) -> (Vec<(FileId, LocatedStorage)>, bool) {
    let mut ordered: Vec<(FileId, SourceInput)> = files.to_vec();
    ordered.sort_by_key(|(id, _)| id.0);
    let mut declarations = Vec::new();
    let mut resources = 0;
    for (file_id, input) in ordered {
        db.unwind_if_revision_cancelled();
        let source = source_located_storage(db, input);
        resources += source.resources;
        declarations.extend(
            source
                .declarations
                .iter()
                .map(|declaration| (file_id, declaration.clone())),
        );
    }
    (declarations, resources > 1)
}

/// The area and bits a fully specified flat address names (specification 11 §6.4: the number is a
/// byte offset; `X`, or no size, is a bit of a byte, `B` one byte, `W` two, `D` four, `L` eight),
/// compared without regard to case. `None` for a wildcard, a hierarchical address, an address
/// relative to a structure and anything malformed.
pub(in crate::db) fn flat_storage(address: &str) -> Option<(LocatedArea, (u64, u64))> {
    let rest = address.trim().strip_prefix('%')?;
    let mut chars = rest.chars();
    let area = match chars.next()?.to_ascii_uppercase() {
        'I' => LocatedArea::Input,
        'Q' => LocatedArea::Output,
        'M' => LocatedArea::Memory,
        _ => return None,
    };
    let rest = chars.as_str();
    let (size, numbers) = match rest.chars().next()?.to_ascii_uppercase() {
        size @ ('X' | 'B' | 'W' | 'D' | 'L') => (size, &rest[1..]),
        digit if digit.is_ascii_digit() => ('X', rest),
        _ => return None,
    };
    let parts = numbers
        .split('.')
        .map(|part| {
            (!part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
                .then(|| part.parse::<u64>().ok())
                .flatten()
        })
        .collect::<Option<Vec<u64>>>()?;
    let bytes = match (size, parts.as_slice()) {
        ('X', [byte, bit]) if *bit < 8 => {
            let first = byte.checked_mul(8)?.checked_add(*bit)?;
            return Some((area, (first, first + 1)));
        }
        ('B', [byte]) => (*byte, 1),
        ('W', [byte]) => (*byte, 2),
        ('D', [byte]) => (*byte, 4),
        ('L', [byte]) => (*byte, 8),
        _ => return None,
    };
    let first = bytes.0.checked_mul(8)?;
    Some((area, (first, first.checked_add(bytes.1 * 8)?)))
}
