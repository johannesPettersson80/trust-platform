//! W017, overlapping located storage (specification 28 §11): two located declarations of one
//! project whose process-image storage shares a bit without being the same item, such as
//! `%IX0.0` inside `%IW0`, or `%IW0` and `%IW1`, which share byte 1.
use super::super::queries::LocatedStorage;
use super::super::*;

/// The other declarations a message names; the rest are counted.
const MAX_NAMED: usize = 3;

/// W017 on each located declaration of `file_id` whose storage overlaps another's in `project`,
/// the located declarations of every source of the project. Nothing is reported for a project
/// with more than one `RESOURCE`, whose resources have process images of their own.
pub(in crate::db) fn check_overlapping_located_storage(
    file_id: FileId,
    project: &[(FileId, LocatedStorage)],
    several_resources: bool,
    diagnostics: &mut DiagnosticBuilder,
) {
    if several_resources {
        return;
    }
    for (_, declaration) in project.iter().filter(|(file, _)| *file == file_id) {
        let others: Vec<&(FileId, LocatedStorage)> = project
            .iter()
            .filter(|(_, other)| overlaps(declaration, other))
            .collect();
        if others.is_empty() {
            continue;
        }
        let mut named: Vec<String> = others
            .iter()
            .take(MAX_NAMED)
            .map(|(_, other)| format!("'{}' at {}", other.name, other.address))
            .collect();
        if others.len() > MAX_NAMED {
            named.push(format!("{} more", others.len() - MAX_NAMED));
        }
        let message = format!(
            "'{}' at {} shares process-image storage with {}: writing one changes the other; \
             give each its own bytes",
            declaration.name,
            declaration.address,
            named.join(", ")
        );
        let mut diagnostic = Diagnostic::warning(
            DiagnosticCode::OverlappingLocatedStorage,
            declaration.range,
            message,
        );
        for (file, other) in others.iter().take(MAX_NAMED) {
            if *file == file_id {
                diagnostic = diagnostic.with_related(
                    other.range,
                    format!("'{}' at {}", other.name, other.address),
                );
            }
        }
        diagnostics.add(diagnostic);
    }
}

/// Whether `a` and `b` share a bit of one area without being the same item: the same area, offset
/// and size is one item that two declarations read, as two programs may read one input.
fn overlaps(a: &LocatedStorage, b: &LocatedStorage) -> bool {
    a.area == b.area && a.bits != b.bits && a.bits.0 < b.bits.1 && b.bits.0 < a.bits.1
}

#[cfg(test)]
mod contract_tests;
