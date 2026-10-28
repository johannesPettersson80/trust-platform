//! Behaviour lock for project-wide diagnostics.
//!
//! A file's diagnostics depend on the other project files: global constants used in array
//! bounds and string lengths, `VAR_CONFIG` entries that complete incomplete direct addresses,
//! and the globals and types imported from them. The project analysis caches those
//! project-wide results. These tests require that an analysis after edits reports exactly
//! the diagnostics of a fresh analysis of the same sources, whatever order the files are
//! queried in, and anchor the cross-file diagnostics this project produces.

use crate::common::*;
use trust_hir::diagnostics::Diagnostic;

const CONSTANTS: &str = "
VAR_GLOBAL CONSTANT
    Size : INT := 4;
    Half : INT := Size / 2;
END_VAR

TYPE Sample :
STRUCT
    value : INT;
END_STRUCT
END_TYPE

VAR_GLOBAL
    Shared : DINT;
END_VAR
";

const PROGRAM: &str = "
PROGRAM Main
VAR
    buffer : ARRAY[1..Size] OF INT;
    name : STRING(Size + Half);
    sample : Sample;
    counter : INT;
    wide : DINT;
    pin AT %I* : BOOL;
END_VAR
buffer[Size] := counter;
name := 'abc';
sample.value := Missing;
wide := Shared;
counter := Shared;
END_PROGRAM
";

const CONFIGURATION: &str = "
CONFIGURATION Plant
RESOURCE Cpu ON PLC
    TASK Cyclic (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Cyclic : Main;
END_RESOURCE
VAR_CONFIG
    P1.counter : DINT := 3;
END_VAR
END_CONFIGURATION
";

fn database(sources: &[String]) -> Database {
    let mut db = Database::new();
    for (index, source) in sources.iter().enumerate() {
        db.set_source_text(FileId(index as u32), source.clone());
    }
    db
}

fn diagnostics_in_order(db: &Database, order: &[usize]) -> Vec<Vec<Diagnostic>> {
    let mut by_file = vec![Vec::new(); order.len()];
    for &index in order {
        by_file[index] = db.diagnostics(FileId(index as u32)).as_ref().clone();
    }
    by_file
}

fn fresh_diagnostics(sources: &[String]) -> Vec<Vec<Diagnostic>> {
    let order = (0..sources.len()).collect::<Vec<_>>();
    diagnostics_in_order(&database(sources), &order)
}

fn range_of(source: &str, needle: &str) -> (usize, usize) {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("'{needle}' in source"));
    (start, start + needle.len())
}

/// A diagnostic with this code on the expression or statement that starts with the text
/// at `range` (a node's range may include the whitespace before it).
fn has(diagnostics: &[Diagnostic], code: DiagnosticCode, range: (usize, usize)) -> bool {
    diagnostics.iter().any(|diagnostic| {
        let start = usize::from(diagnostic.range.start());
        let end = usize::from(diagnostic.range.end());
        diagnostic.code == code && start <= range.0 && range.0 - start <= 1 && end >= range.1
    })
}

fn project() -> Vec<String> {
    let sources = vec![
        CONSTANTS.to_string(),
        PROGRAM.to_string(),
        CONFIGURATION.to_string(),
    ];
    for source in &sources {
        let parse = trust_syntax::parser::parse(source);
        assert!(parse.ok(), "{:?} in {source}", parse.errors());
    }
    sources
}

#[test]
fn project_diagnostics_do_not_depend_on_the_query_order() {
    let sources = project();
    let fresh = fresh_diagnostics(&sources);
    let reversed = diagnostics_in_order(&database(&sources), &[2, 1, 0]);
    assert_eq!(reversed, fresh);
    let middle_first = diagnostics_in_order(&database(&sources), &[1, 2, 0]);
    assert_eq!(middle_first, fresh);
}

#[test]
fn cross_file_constants_globals_and_var_config_entries_are_resolved() {
    let fresh = fresh_diagnostics(&project());
    let errors = |file: usize| {
        fresh[file]
            .iter()
            .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
            .cloned()
            .collect::<Vec<_>>()
    };

    assert!(errors(0).is_empty(), "constants file: {:?}", errors(0));

    // Array bound and string length come from the other file's constants and `Sample` is
    // the other file's type; the program's errors are the unmapped incomplete address, the
    // undefined name and the narrowing assignment from the imported DINT global.
    let program_errors = errors(1);
    assert!(
        has(
            &program_errors,
            DiagnosticCode::InvalidOperation,
            range_of(PROGRAM, "pin")
        ),
        "unmapped incomplete address: {program_errors:?}"
    );
    assert!(
        has(
            &program_errors,
            DiagnosticCode::UndefinedVariable,
            range_of(PROGRAM, "Missing")
        ),
        "undefined name: {program_errors:?}"
    );
    assert!(
        has(
            &program_errors,
            DiagnosticCode::IncompatibleAssignment,
            range_of(PROGRAM, "counter := Shared")
        ),
        "narrowing assignment: {program_errors:?}"
    );
    assert_eq!(program_errors.len(), 3, "{program_errors:?}");

    let configuration_errors = errors(2);
    assert_eq!(
        configuration_errors
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        vec![DiagnosticCode::TypeMismatch],
        "VAR_CONFIG type of P1.counter: {configuration_errors:?}"
    );
}

#[test]
fn project_diagnostics_after_edits_equal_a_fresh_analysis() {
    let mut sources = project();
    let mut db = database(&sources);
    assert_eq!(
        diagnostics_in_order(&db, &[0, 1, 2]),
        fresh_diagnostics(&sources)
    );

    let edits: [(usize, String); 5] = [
        // a constant used for another file's array bound and string length
        (0, CONSTANTS.replace("Size : INT := 4;", "Size : INT := 8;")),
        // the imported global disappears: the program now reports it as undefined
        (
            0,
            CONSTANTS.replace("VAR_GLOBAL\n    Shared : DINT;\nEND_VAR\n", ""),
        ),
        // an edit of the program alone
        (1, PROGRAM.replace("sample.value := Missing;", "")),
        // the configuration gains a VAR_CONFIG entry
        (
            2,
            CONFIGURATION.replace(
                "    P1.counter : DINT := 3;\n",
                "    P1.counter : DINT := 3;\n    P1.pin AT %IX0.1 : BOOL;\n",
            ),
        ),
        // and the constants come back
        (0, CONSTANTS.to_string()),
    ];
    for (step, (file, text)) in edits.into_iter().enumerate() {
        sources[file] = text.clone();
        db.set_source_text(FileId(file as u32), text);
        let fresh = fresh_diagnostics(&sources);
        assert_eq!(
            diagnostics_in_order(&db, &[2, 0, 1]),
            fresh,
            "edit {step} of file {file}"
        );
        if step == 1 {
            assert!(
                has(
                    &fresh[1],
                    DiagnosticCode::UndefinedVariable,
                    range_of(&sources[1], "Shared")
                ),
                "the removed global is undefined in the program: {:?}",
                fresh[1]
            );
        }
    }
}
