//! The projects of the analysis (specification 14 §10.2): a file is analyzed among the files of its
//! own project and of that project's libraries, never among another project's files; without a
//! layout every file belongs to one project.

use crate::common::*;
use trust_hir::db::{ProjectId, ProjectLayout};

/// Line A's program: its pump's declaration names the library's `FB_Limit`.
const LINE_A_MAIN: &str = "
PROGRAM Main
VAR
    pump : FB_Pump;
END_VAR
pump(run := TRUE);
END_PROGRAM
";

const LINE_A_PUMP: &str = "
FUNCTION_BLOCK FB_Pump
VAR_INPUT
    run : BOOL;
END_VAR
VAR
    limit : FB_Limit;
END_VAR
END_FUNCTION_BLOCK
";

/// Line B's program, which has the same name as line A's.
const LINE_B_MAIN: &str = "
PROGRAM Main
VAR
    valve : FB_Valve;
END_VAR
valve(open := TRUE);
END_PROGRAM
";

const LINE_B_VALVE: &str = "
FUNCTION_BLOCK FB_Valve
VAR_INPUT
    open : BOOL;
END_VAR
END_FUNCTION_BLOCK
";

const LIBRARY_LIMIT: &str = "
FUNCTION_BLOCK FB_Limit
VAR_INPUT
    value : INT;
END_VAR
END_FUNCTION_BLOCK
";

const LINE_A: ProjectId = ProjectId(1);
const LINE_B: ProjectId = ProjectId(2);
const LIBRARY: ProjectId = ProjectId(3);

/// Line A's files 0 and 1, line B's files 2 and 3, and the library's file 4.
fn database() -> Database {
    let mut db = Database::new();
    let sources = [
        LINE_A_MAIN,
        LINE_A_PUMP,
        LINE_B_MAIN,
        LINE_B_VALVE,
        LIBRARY_LIMIT,
    ];
    for (index, source) in sources.iter().enumerate() {
        db.set_source_text(FileId(index as u32), (*source).to_string());
    }
    db
}

/// Each file in its project; line A uses the library when `line_a_uses_library`.
fn layout(line_a_uses_library: bool) -> ProjectLayout {
    let mut layout = ProjectLayout::default();
    for (file, project) in [
        (0, LINE_A),
        (1, LINE_A),
        (2, LINE_B),
        (3, LINE_B),
        (4, LIBRARY),
    ] {
        layout.files.insert(FileId(file), project);
    }
    if line_a_uses_library {
        layout.library_files.insert(LINE_A, vec![FileId(4)]);
    }
    layout
}

fn errors(db: &Database, file: u32) -> Vec<String> {
    db.diagnostics(FileId(file))
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| format!("{}: {}", diagnostic.code.code(), diagnostic.message))
        .collect()
}

fn every_declaration(db: &Database, file: u32) -> Vec<String> {
    db.file_symbols_with_every_declaration(FileId(file))
        .iter()
        .map(|symbol| symbol.name.to_string())
        .collect()
}

/// Catches projects analyzed as one, the G276 finding: every example's `PROGRAM Main` collided
/// with the others' in a folder of several projects. Without a layout the two programs are one
/// project's and collide; with one, neither line has an error.
#[test]
fn two_projects_that_each_declare_main_do_not_collide() {
    let mut db = database();
    assert!(
        errors(&db, 0)
            .iter()
            .any(|error| error.starts_with("E104") && error.contains("Main")),
        "without a layout the files are one project and the programs collide: {:?}",
        errors(&db, 0)
    );
    db.set_project_layout(layout(true));
    for file in 0..5 {
        assert!(
            errors(&db, file).is_empty(),
            "file {file} resolves every name it writes in its own project: {:?}",
            errors(&db, file)
        );
    }
}

/// Catches a project that sees another project's files, or not its own libraries': line A's pump
/// finds the library's `FB_Limit` only while line A uses the library, and the declarations that
/// completion and rename read in line B hold nothing of line A or the library.
#[test]
fn a_project_sees_its_libraries_and_not_another_projects_files() {
    let mut db = database();
    db.set_project_layout(layout(false));
    assert!(
        errors(&db, 1)
            .iter()
            .any(|error| error.contains("FB_Limit")),
        "without the library line A's pump cannot find FB_Limit: {:?}",
        errors(&db, 1)
    );
    db.set_project_layout(layout(true));
    assert!(errors(&db, 1).is_empty(), "{:?}", errors(&db, 1));
    let line_b = every_declaration(&db, 2);
    for other in ["FB_Pump", "FB_Limit", "pump"] {
        assert!(
            !line_b.iter().any(|name| name == other),
            "line B's declarations hold nothing of line A or of a library it does not use: \
             '{other}' in {line_b:?}"
        );
    }
    assert!(line_b.iter().any(|name| name == "FB_Valve"), "{line_b:?}");
}

/// Catches a library judged among the files of the projects that use it: the library's file sees
/// none of line A's declarations, although line A uses it.
#[test]
fn a_library_is_judged_in_its_own_project() {
    let mut db = database();
    db.set_project_layout(layout(true));
    let library = every_declaration(&db, 4);
    assert!(
        !library
            .iter()
            .any(|name| name == "FB_Pump" || name == "Main"),
        "the library's declarations hold none of its user's: {library:?}"
    );
    assert!(library.iter().any(|name| name == "FB_Limit"), "{library:?}");
}
