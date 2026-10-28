//! Specification 39, §7.8: what a rename renames, in the terms of paths outside the program, so
//! the HMI bindings follow it.

use text_size::TextSize;
use trust_hir::db::{FileId, SourceDatabase};
use trust_hir::Database;
use trust_ide::rename_identity::{rename_identity, RenameIdentity};

/// The identity at the `occurrence`-th place `marker` starts in source `file`, `offset` bytes
/// into the marker.
fn identity_at(
    sources: &[&str],
    file: usize,
    marker: &str,
    occurrence: usize,
    offset: usize,
) -> Option<RenameIdentity> {
    let mut db = Database::new();
    for (index, source) in sources.iter().enumerate() {
        db.set_source_text(
            FileId(u32::try_from(index).expect("few files")),
            (*source).to_string(),
        );
    }
    let source = sources[file];
    let start = source
        .match_indices(marker)
        .nth(occurrence)
        .unwrap_or_else(|| panic!("`{marker}` occurs {} times", occurrence + 1))
        .0;
    let position = TextSize::from(u32::try_from(start + offset).expect("small source"));
    rename_identity(
        &db,
        FileId(u32::try_from(file).expect("few files")),
        position,
    )
}

fn member(owner: &str, name: &str) -> Option<RenameIdentity> {
    Some(RenameIdentity::Member {
        owner: owner.into(),
        name: name.into(),
    })
}

const PUMP: &str = "FUNCTION_BLOCK Pump\nVAR_INPUT\n    Start : BOOL;\nEND_VAR\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\n    Running := Start;\nEND_FUNCTION_BLOCK\n";
const MAIN: &str = "PROGRAM Main\nVAR\n    P101 : Pump;\n    Level : REAL;\nEND_VAR\n    P101(Start := TRUE);\n    Level := Level + 1.0;\n    IF P101.Running THEN\n        Level := 0.0;\n    END_IF;\nEND_PROGRAM\n";

#[test]
fn a_function_block_member_is_named_with_the_type_that_declares_it() {
    assert_eq!(
        identity_at(&[PUMP, MAIN], 0, "Running : BOOL", 0, 0),
        member("Pump", "Running")
    );
    assert_eq!(
        identity_at(&[PUMP, MAIN], 1, "P101.Running", 0, 5),
        member("Pump", "Running"),
        "from where it is used"
    );
}

#[test]
fn a_program_variable_and_an_instance_are_members_of_the_program() {
    assert_eq!(
        identity_at(&[PUMP, MAIN], 1, "Level : REAL", 0, 0),
        member("Main", "Level")
    );
    assert_eq!(
        identity_at(&[PUMP, MAIN], 1, "P101 : Pump", 0, 0),
        member("Main", "P101")
    );
}

#[test]
fn a_member_of_a_block_in_a_namespace_names_the_qualified_type() {
    let library = "NAMESPACE Lib\nFUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\nEND_NAMESPACE\n";
    assert_eq!(
        identity_at(&[library], 0, "Running : BOOL", 0, 0),
        member("Lib.Pump", "Running")
    );
}

#[test]
fn an_inherited_member_is_named_with_the_base_that_declares_it() {
    let source = "FUNCTION_BLOCK Base\nVAR_OUTPUT\n    Result : INT;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nFUNCTION_BLOCK Derived EXTENDS Base\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    B : Derived;\n    X : INT;\nEND_VAR\n    X := B.Result;\nEND_PROGRAM\n";
    assert_eq!(
        identity_at(&[source], 0, "B.Result", 0, 2),
        member("Base", "Result")
    );
}

#[test]
fn a_structure_field_is_named_with_its_structure_type() {
    let source = "TYPE MotorState : STRUCT\n    Speed : DINT;\nEND_STRUCT\nEND_TYPE\n\nPROGRAM Main\nVAR\n    State : MotorState;\nEND_VAR\n    State.Speed := 1;\nEND_PROGRAM\n";
    let field = Some(RenameIdentity::Field {
        owner: "MotorState".into(),
        name: "Speed".into(),
    });
    assert_eq!(identity_at(&[source], 0, "Speed : DINT", 0, 0), field);
    assert_eq!(
        identity_at(&[source], 0, "State.Speed", 0, 6),
        field,
        "from where it is used"
    );
}

#[test]
fn a_global_and_a_configured_program_instance_are_the_first_part_of_a_path() {
    let globals = "VAR_GLOBAL\n    Speed : INT;\nEND_VAR\n\nPROGRAM Main\nVAR\n    X : INT;\nEND_VAR\n    X := Speed;\nEND_PROGRAM\n";
    assert_eq!(
        identity_at(&[globals], 0, "Speed : INT", 0, 0),
        Some(RenameIdentity::Root {
            name: "Speed".into()
        })
    );
    let configured = "PROGRAM TankProgram\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n\nCONFIGURATION Plant\nRESOURCE Cpu ON PLC\n    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM Tank WITH Fast : TankProgram;\nEND_RESOURCE\nEND_CONFIGURATION\n";
    assert_eq!(
        identity_at(&[configured], 0, "Tank WITH", 0, 0),
        Some(RenameIdentity::Root {
            name: "Tank".into()
        })
    );
}

#[test]
fn a_program_names_itself() {
    assert_eq!(
        identity_at(&[PUMP, MAIN], 1, "Main", 0, 0),
        Some(RenameIdentity::Program {
            name: "Main".into()
        })
    );
}

#[test]
fn a_local_of_a_function_names_nothing_a_path_can_reach() {
    let source = "FUNCTION Twice : INT\nVAR_INPUT\n    Value : INT;\nEND_VAR\n    Twice := Value * 2;\nEND_FUNCTION\n";
    assert_eq!(identity_at(&[source], 0, "Value : INT", 0, 0), None);
}
