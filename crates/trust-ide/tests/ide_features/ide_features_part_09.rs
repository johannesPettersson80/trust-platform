//! The program that a configuration's program instance runs, `MainProgram` in
//! `PROGRAM P1 WITH Cycle : MainProgram;` (IEC 61131-3 Ed.3 §6.8.1, Table 62 feature 6a): a
//! reference of the program (specification 14 §6.3.2), which find references, the reference count
//! above the program and rename reach.

use super::*;
use text_size::TextRange;

const PROGRAM: &str = "
PROGRAM MainProgram
VAR
    Count : INT;
END_VAR
Count := Count + 1;
END_PROGRAM
";

const CONFIGURATION: &str = "
CONFIGURATION FillingLine
TASK Cycle (INTERVAL := T#50ms, PRIORITY := 1);
PROGRAM P1 WITH Cycle : MainProgram;
END_CONFIGURATION
";

const PROGRAM_FILE: FileId = FileId(0);
const CONFIGURATION_FILE: FileId = FileId(1);

fn project() -> Database {
    let mut db = Database::new();
    db.set_source_text(PROGRAM_FILE, PROGRAM.to_string());
    db.set_source_text(CONFIGURATION_FILE, CONFIGURATION.to_string());
    db
}

/// `MainProgram` where `PROGRAM MainProgram` declares it.
fn declared_name() -> TextRange {
    let start = PROGRAM.find("MainProgram").unwrap();
    TextRange::at(TextSize::from(start as u32), TextSize::from(11))
}

/// `MainProgram` after the colon of the configuration's program instance.
fn configured_name() -> TextRange {
    let start = CONFIGURATION.find(": MainProgram").unwrap() + 2;
    TextRange::at(TextSize::from(start as u32), TextSize::from(11))
}

fn found(references: &[trust_ide::references::Reference]) -> Vec<(FileId, TextRange)> {
    let mut found: Vec<_> = references
        .iter()
        .map(|reference| (reference.file_id, reference.range))
        .collect();
    found.sort_by_key(|(file, range)| (file.0, range.start()));
    found
}

#[test]
fn a_configuration_that_runs_a_program_is_a_reference_of_the_program() {
    // Catches the configuration left out of the program's references: the program then shows no
    // reference count, although the configuration runs it.
    let db = project();
    let references = find_references(
        &db,
        PROGRAM_FILE,
        declared_name().start(),
        FindReferencesOptions {
            include_declaration: false,
        },
    );
    assert_eq!(
        found(&references),
        vec![(CONFIGURATION_FILE, configured_name())]
    );
}

#[test]
fn find_references_from_a_configuration_reaches_the_program() {
    // Catches the name after the colon resolving to the instance `P1`, the task `Cycle` or nothing,
    // and the search from there missing the configuration itself.
    let db = project();
    let references = find_references(
        &db,
        CONFIGURATION_FILE,
        configured_name().start(),
        FindReferencesOptions {
            include_declaration: true,
        },
    );
    assert_eq!(
        found(&references),
        vec![
            (PROGRAM_FILE, declared_name()),
            (CONFIGURATION_FILE, configured_name())
        ]
    );
}

#[test]
fn renaming_a_program_renames_it_in_the_configuration_that_runs_it() {
    // Catches a rename that edits the declaration but leaves `PROGRAM P1 WITH Cycle : MainProgram;`,
    // which then names no program, so the project no longer builds; and one that renames the
    // instance or the task instead.
    let db = project();
    let result = rename(&db, PROGRAM_FILE, declared_name().start(), "FillingProgram")
        .expect("a program that a configuration runs can be renamed");
    let edits = |file: FileId| -> Vec<(TextRange, String)> {
        result
            .edits
            .get(&file)
            .map(|edits| {
                edits
                    .iter()
                    .map(|edit| (edit.range, edit.new_text.clone()))
                    .collect()
            })
            .unwrap_or_default()
    };
    assert_eq!(
        edits(PROGRAM_FILE),
        vec![(declared_name(), "FillingProgram".to_string())]
    );
    assert_eq!(
        edits(CONFIGURATION_FILE),
        vec![(configured_name(), "FillingProgram".to_string())]
    );
}
