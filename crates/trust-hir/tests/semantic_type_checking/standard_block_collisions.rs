//! Project-import collisions with a standard function block (`TON`, `R_TRIG`, ...), which
//! has no source location: reported where the name is declared or used, never at the start
//! of a file.

use super::assignments_and_var_access::{duplicate_ranges, text_range_of};
use crate::common::*;

fn duplicates(db: &Database, file: FileId) -> Vec<trust_hir::diagnostics::Diagnostic> {
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::DuplicateDeclaration)
        .cloned()
        .collect()
}

#[test]
fn test_collision_with_a_standard_function_block_has_a_source_location() {
    // `TON` is a standard function block: it has no source text to point at, so the
    // collision is reported at the declaration that reuses the name and at its uses in
    // other files, never at the start of a file.
    let declaring = "\nVAR_GLOBAL\n    TON : INT;\nEND_VAR\n";
    let using = "\nPROGRAM Main\nVAR\n    t : TON;\nEND_VAR\nEND_PROGRAM\n";
    let unrelated =
        "\nFUNCTION_BLOCK Other\nVAR\n    x : INT;\nEND_VAR\nx := 1;\nEND_FUNCTION_BLOCK\n";
    let mut db = Database::new();
    db.set_source_text(FileId(0), declaring.to_string());
    db.set_source_text(FileId(1), using.to_string());
    db.set_source_text(FileId(2), unrelated.to_string());

    let own = duplicates(&db, FileId(0));
    assert_eq!(
        own.iter()
            .map(|diagnostic| (
                usize::from(diagnostic.range.start()),
                usize::from(diagnostic.range.end())
            ))
            .collect::<Vec<_>>(),
        vec![text_range_of(declaring, "TON")],
        "{own:?}"
    );
    assert_eq!(
        own[0].message,
        "duplicate declaration of 'TON', the name of a standard function block"
    );
    assert!(
        own[0].related.is_empty(),
        "no location for the standard block: {own:?}"
    );

    let used = duplicates(&db, FileId(1));
    let reference = using.find("TON").expect("reference");
    assert_eq!(
        duplicate_ranges(&db, FileId(1)),
        vec![(reference, reference + "TON".len())],
        "{used:?}"
    );
    assert_eq!(
        used[0].message,
        "duplicate imported declaration of 'TON', the name of a standard function block"
    );
    assert!(used[0].related.is_empty(), "{used:?}");

    assert_eq!(
        duplicate_ranges(&db, FileId(2)),
        Vec::<(usize, usize)>::new()
    );
}
