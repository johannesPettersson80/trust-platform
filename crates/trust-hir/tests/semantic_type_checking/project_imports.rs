//! What one file's analysis holds of the other files of its project (specification 14 §10.2): its
//! own declarations and the declarations it names, with those that their declarations name in
//! turn, never a copy of the whole project. Completion, the learner's suggestions and rename's
//! conflict check read every declaration of the project instead (§6.1).

use crate::common::*;

/// Named by `Main`; its declaration names `FB_Helper`, which `Main` does not name.
const USED: &str = "
FUNCTION_BLOCK FB_Used
VAR_INPUT
    x : INT;
END_VAR
VAR
    helper : FB_Helper;
END_VAR
END_FUNCTION_BLOCK
";

const HELPER: &str = "
FUNCTION_BLOCK FB_Helper
VAR_OUTPUT
    done : BOOL;
END_VAR
END_FUNCTION_BLOCK
";

/// Named by `Main`; its field's type `FB_Sensor` is declared in a third file, which `Main` does not
/// name, and `Main` reads `r.sensor.q` through it.
const READING: &str = "
TYPE ST_Reading :
STRUCT
    value : INT;
    sensor : FB_Sensor;
END_STRUCT
END_TYPE
";

const SENSOR: &str = "
FUNCTION_BLOCK FB_Sensor
VAR_OUTPUT
    q : BOOL;
END_VAR
END_FUNCTION_BLOCK
";

/// Named by nothing in `Main`.
const UNUSED: &str = "
FUNCTION_BLOCK FB_Unused
VAR
    unusedLocal : INT;
END_VAR
END_FUNCTION_BLOCK
";

/// `Main` names its global `g_Count`, which a configuration makes visible everywhere.
const CONFIGURATION: &str = "
CONFIGURATION Cell
VAR_GLOBAL
    g_Count : INT;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

const MAIN: &str = "
PROGRAM Main
VAR_EXTERNAL
    g_Count : INT;
END_VAR
VAR
    u : FB_Used;
    r : ST_Reading;
    seen : BOOL;
END_VAR
u(x := 1);
seen := r.sensor.q;
g_Count := g_Count + 1;
END_PROGRAM
";

fn project() -> (Database, FileId) {
    let mut db = Database::new();
    let sources = [USED, HELPER, READING, SENSOR, UNUSED, CONFIGURATION, MAIN];
    for (index, source) in sources.iter().enumerate() {
        db.set_source_text(FileId(index as u32), (*source).to_string());
    }
    (db, FileId(sources.len() as u32 - 1))
}

fn imported_names(symbols: &trust_hir::symbols::SymbolTable) -> Vec<String> {
    symbols
        .iter()
        .filter(|symbol| symbol.origin.is_some())
        .map(|symbol| symbol.name.to_string())
        .collect()
}

/// Catches an analysis that copies every declaration of the project into each file (G275c: the
/// memory of a project of n files grew with n²), and one that leaves out a declaration that a
/// named declaration names, which would leave `r.sensor.q` unresolved.
#[test]
fn a_files_analysis_holds_the_declarations_it_names_and_not_the_others() {
    let (db, main) = project();

    let errors: Vec<String> = db
        .diagnostics(main)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.message.clone())
        .collect();
    assert!(
        errors.is_empty(),
        "Main resolves every name it writes: {errors:?}"
    );

    let imported = imported_names(&db.file_symbols_with_project(main));
    for named in [
        "FB_Used",    // named
        "x",          // a member of a named declaration
        "FB_Helper",  // named by FB_Used's declaration
        "ST_Reading", // named
        "FB_Sensor",  // named by ST_Reading's declaration
        "q",          // a member of FB_Sensor, which Main reads
        "Cell",       // the configuration that makes g_Count visible everywhere
        "g_Count",    // named
    ] {
        assert!(
            imported.iter().any(|name| name == named),
            "Main's analysis holds '{named}': {imported:?}"
        );
    }
    for unnamed in ["FB_Unused", "unusedLocal"] {
        assert!(
            !imported.iter().any(|name| name == unnamed),
            "Main's analysis does not hold '{unnamed}', which it does not name: {imported:?}"
        );
    }
}

/// Catches a completion, learner suggestion or rename check that sees only the declarations the
/// file names already: the table they read holds every declaration of the project.
#[test]
fn every_declaration_of_the_project_stays_reachable_for_completion_and_rename() {
    let (db, main) = project();
    let every = imported_names(&db.file_symbols_with_every_declaration(main));
    for name in [
        "FB_Unused",
        "unusedLocal",
        "FB_Used",
        "FB_Helper",
        "g_Count",
    ] {
        assert!(
            every.iter().any(|imported| imported == name),
            "every declaration holds '{name}': {every:?}"
        );
    }
}
