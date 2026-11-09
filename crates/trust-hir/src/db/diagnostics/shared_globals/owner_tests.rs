//! The owner of each global (control-studio-runtime-protocol annex E2.3): the one task whose programs' bodies are the only places
//! that write it. Every expected owner is worked out by hand from the project below.

use crate::db::{Database, FileId, GlobalOwner, SourceDatabase};

/// The owners of the globals that the first of `sources` declares, by name.
fn owners(sources: &[&str]) -> Vec<(String, GlobalOwner)> {
    let mut database = Database::new();
    for (index, source) in sources.iter().enumerate() {
        database.set_source_text(FileId(index as u32), (*source).to_owned());
    }
    database
        .global_owners(FileId(0))
        .iter()
        .map(|global| (global.name.to_string(), global.owner.clone()))
        .collect()
}

/// A GVL in a file of its own, as a CODESYS project holds it, and two tasks' programs in another file.
const GVL: &str = "
NAMESPACE GVL_IO
VAR_GLOBAL
    bManual : BOOL;
    lrValve : LREAL;
    lrSlowValve : LREAL;
    lrBoth : LREAL;
    lrBlockValve : LREAL;
END_VAR
END_NAMESPACE
";

const PROGRAMS: &str = "
FUNCTION_BLOCK FB_Drive
GVL_IO.lrBlockValve := 1.0;
END_FUNCTION_BLOCK

PROGRAM Tank
VAR
    Drive : FB_Drive;
END_VAR
IF GVL_IO.bManual THEN
    GVL_IO.lrValve := 2.0;
END_IF;
GVL_IO.lrBoth := 1.0;
Drive();
END_PROGRAM

PROGRAM Other
GVL_IO.lrSlowValve := GVL_IO.lrSlowValve + 1.0;
GVL_IO.lrBoth := 2.0;
END_PROGRAM

CONFIGURATION Plant
RESOURCE Res ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    TASK Slow (INTERVAL := T#100ms, PRIORITY := 2);
    PROGRAM TankInst WITH Fast : Tank;
    PROGRAM OtherInst WITH Slow : Other;
END_RESOURCE
END_CONFIGURATION
";

/// Worked from the project: `bManual` is only read, so no program writes it; `lrValve` only `Tank`, which task `Fast` runs, writes;
/// `lrSlowValve` only `Other` in `Slow`; `lrBoth` both programs, so two tasks; `lrBlockValve` the function block `FB_Drive`, which
/// any task may run. Catches a global that one task writes left without its owner, one that two tasks write given to one of
/// them, and a write in a function block attributed to the program that calls it.
#[test]
fn a_global_is_owned_by_the_one_task_whose_programs_write_it() {
    assert_eq!(
        owners(&[GVL, PROGRAMS]),
        [
            ("GVL_IO.bManual".to_string(), GlobalOwner::Unwritten),
            (
                "GVL_IO.lrValve".to_string(),
                GlobalOwner::Task("Fast".into())
            ),
            (
                "GVL_IO.lrSlowValve".to_string(),
                GlobalOwner::Task("Slow".into())
            ),
            ("GVL_IO.lrBoth".to_string(), GlobalOwner::Shared),
            ("GVL_IO.lrBlockValve".to_string(), GlobalOwner::Shared),
        ]
    );
}

/// A project without a configuration runs each program outside every task (specification 18 §2.2), so a global that only its
/// programs write belongs to `$background`. A program that a configuration does not run writes nothing that counts. Catches a
/// background write read as no write, and a program that never runs taken for a writer.
#[test]
fn programs_outside_every_task_own_a_global_as_background() {
    let gvl = "VAR_GLOBAL\n    gLevel : LREAL;\n    gIdle : LREAL;\nEND_VAR\n";
    let unconfigured = "PROGRAM Main\ngLevel := gLevel + 1.0;\nEND_PROGRAM\n";
    assert_eq!(
        owners(&[gvl, unconfigured]),
        [
            ("gLevel".to_string(), GlobalOwner::Background),
            ("gIdle".to_string(), GlobalOwner::Unwritten),
        ]
    );
    let configured = "PROGRAM Main\ngLevel := 1.0;\nEND_PROGRAM\n\
        PROGRAM Spare\ngIdle := 1.0;\nEND_PROGRAM\n\
        CONFIGURATION C\nRESOURCE R ON PLC\n    PROGRAM P1 : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n";
    assert_eq!(
        owners(&[gvl, configured]),
        [
            ("gLevel".to_string(), GlobalOwner::Background),
            ("gIdle".to_string(), GlobalOwner::Unwritten),
        ]
    );
}
