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
/// only `Tank` calls, so its write is `Fast`'s too. Catches a global that one task writes left without its owner, one that two
/// tasks write given to one of them, and a write in a function block that counts for no task, or for a task that never runs it.
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
            (
                "GVL_IO.lrBlockValve".to_string(),
                GlobalOwner::Task("Fast".into())
            ),
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

/// The POUs a program's calls reach, for the owners that follow the call tree (`reach`): a function block that a method calls, a
/// function the method calls, a block that programs of two tasks call, and a method that only an interface's call reaches.
const CALLS: &str = "
VAR_GLOBAL
    gByBlock : LREAL;
    gByFunction : LREAL;
    gByBothTasks : LREAL;
    gByInterface : LREAL;
END_VAR

FUNCTION SetByFunction : BOOL
gByFunction := 1.0;
SetByFunction := TRUE;
END_FUNCTION

FUNCTION_BLOCK FB_Inner
gByBlock := 2.0;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Outer
VAR
    inner : FB_Inner;
END_VAR
METHOD PUBLIC Run : BOOL
inner();
Run := SetByFunction();
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Both
gByBothTasks := 3.0;
END_FUNCTION_BLOCK

INTERFACE I_Act
METHOD Act : BOOL
END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_Act IMPLEMENTS I_Act
METHOD PUBLIC Act : BOOL
gByInterface := 4.0;
Act := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Tank
VAR
    outer : FB_Outer;
    both : FB_Both;
    act : FB_Act;
    itf : I_Act;
    ok : BOOL;
END_VAR
ok := outer.Run();
both();
itf := act;
ok := itf.Act();
END_PROGRAM

PROGRAM Other
VAR
    both : FB_Both;
END_VAR
both();
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

/// Worked from the project: `Tank`, in `Fast`, calls the method `FB_Outer.Run`, which calls `FB_Inner`, which writes `gByBlock`,
/// and the function `SetByFunction`, which writes `gByFunction`, so both are `Fast`'s; `FB_Both` writes `gByBothTasks` and
/// programs of `Fast` and `Slow` call it, so no single task owns it; `FB_Act.Act` writes `gByInterface` and only a call through
/// the interface `I_Act` reaches it, which names no body, so any task may run it. Catches the writes of called POUs counted for
/// no task (a loop's output that a block writes refused), a block that two tasks run counted for one of them, and a method an
/// interface may dispatch to counted for the caller although the compiler cannot tell which body runs.
#[test]
fn a_called_pou_writes_its_globals_in_the_tasks_of_the_programs_that_reach_it() {
    assert_eq!(
        owners(&[CALLS]),
        [
            ("gByBlock".to_string(), GlobalOwner::Task("Fast".into())),
            ("gByFunction".to_string(), GlobalOwner::Task("Fast".into())),
            ("gByBothTasks".to_string(), GlobalOwner::Shared),
            ("gByInterface".to_string(), GlobalOwner::Shared),
        ]
    );
}

/// Actions, whose statements are part of the POU that declares them: the program `Tank` calls its own action, the action of its block
/// instance `step` and the action of its block instance `gate`, which a second file declares; the program `Other`, in `Slow`, calls
/// none of them.
const ACTIONS: &str = "
VAR_GLOBAL
    gByProgramAction : LREAL;
    gByBlockAction : LREAL;
    gByFarAction : LREAL;
END_VAR

FUNCTION_BLOCK FB_Step
ACTION Reset:
gByBlockAction := 2.0;
END_ACTION
END_FUNCTION_BLOCK

PROGRAM Tank
VAR
    step : FB_Step;
    gate : FB_Gate;
END_VAR
step.Reset();
gate.Open();
SetProgram();
ACTION SetProgram:
gByProgramAction := 3.0;
END_ACTION
END_PROGRAM

PROGRAM Other
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

/// The block with the action of the second file.
const FAR_ACTION: &str = "
FUNCTION_BLOCK FB_Gate
ACTION Open:
gByFarAction := 4.0;
END_ACTION
END_FUNCTION_BLOCK
";

/// Specification 28 §6: a program accesses what an action it calls accesses. Worked from the project: `Tank`, in `Fast`, calls its own
/// action `SetProgram`, which writes `gByProgramAction`; the action `Reset` of its block `step`, which writes `gByBlockAction`; and the
/// action `Open` of its block `gate`, declared in a second file, which writes `gByFarAction`. Each write is `Fast`'s. Catches an action
/// call that reaches nothing, so that a global a block's action writes is refused as no task's.
#[test]
fn an_action_a_program_calls_writes_its_globals_in_the_programs_task() {
    assert_eq!(
        owners(&[ACTIONS, FAR_ACTION]),
        [
            (
                "gByProgramAction".to_string(),
                GlobalOwner::Task("Fast".into())
            ),
            (
                "gByBlockAction".to_string(),
                GlobalOwner::Task("Fast".into())
            ),
            ("gByFarAction".to_string(), GlobalOwner::Task("Fast".into())),
        ]
    );
}
