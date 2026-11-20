//! W017 (specification 28 §11). A flat address is a byte offset of its area (specification 11
//! §6.4): `%IX0.0` is bit 0 of byte 0, `%IW0` bytes 0 and 1, `%IW1` bytes 1 and 2, `%IW2` bytes 2
//! and 3. Every expected count below follows from that by hand.

use crate::db::queries::{flat_storage, LocatedArea};
use crate::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use crate::diagnostics::{Diagnostic, DiagnosticCode};

fn overlap_warnings_in(sources: &[&str], file: usize) -> Vec<Diagnostic> {
    let mut database = Database::new();
    for (index, source) in sources.iter().enumerate() {
        database.set_source_text(FileId(index as u32), (*source).to_owned());
    }
    database
        .diagnostics(FileId(file as u32))
        .iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::OverlappingLocatedStorage)
        .cloned()
        .collect()
}

fn overlap_warnings(source: &str) -> Vec<Diagnostic> {
    overlap_warnings_in(&[source], 0)
}

/// The tank program of tutorial 11 and filling_line with the configuration `main` had: the
/// command bits in byte 0, the level and pump-speed words at offset 0.
const TANK_PROGRAM: &str = r#"
PROGRAM TankProgram
VAR
    StartCmd : BOOL;
    StopCmd : BOOL;
    LevelRaw : INT;
    FillValveOut : BOOL;
    DrainValveOut : BOOL;
    PumpSpeedOut : INT;
END_VAR
END_PROGRAM
"#;

fn tank_configuration(level: &str, pump: &str) -> String {
    format!(
        r#"{TANK_PROGRAM}
CONFIGURATION TankControlConfig
RESOURCE TankControlRes ON PLC
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Cycle : TankProgram;
END_RESOURCE
VAR_CONFIG
    P1.StartCmd AT %IX0.0 : BOOL;
    P1.StopCmd AT %IX0.1 : BOOL;
    P1.LevelRaw AT {level} : INT;
    P1.FillValveOut AT %QX0.0 : BOOL;
    P1.DrainValveOut AT %QX0.1 : BOOL;
    P1.PumpSpeedOut AT {pump} : INT;
END_VAR
END_CONFIGURATION
"#
    )
}

/// %IW0 holds %IX0.0 and %IX0.1, and %QW0 holds %QX0.0 and %QX0.1: each of the six entries
/// overlaps another, so six warnings, and the level's names both command bits. Catches no check,
/// a check of one side only, and one that misses `VAR_CONFIG` entries.
#[test]
fn bits_inside_a_word_are_warned_about_on_each_side() {
    let warnings = overlap_warnings(&tank_configuration("%IW0", "%QW0"));
    assert_eq!(warnings.len(), 6, "{warnings:#?}");
    let level = warnings
        .iter()
        .find(|warning| warning.message.starts_with("'P1.LevelRaw'"))
        .expect("the level's warning");
    assert_eq!(
        level.message,
        "'P1.LevelRaw' at %IW0 shares process-image storage with 'P1.StartCmd' at %IX0.0, \
         'P1.StopCmd' at %IX0.1: writing one changes the other; give each its own bytes"
    );
    assert_eq!(level.related.len(), 2, "both command bits, in this file");
}

/// With the words at offset 2 (bytes 2 and 3), nothing shares a bit: no warning. Catches a check
/// that compares whole bytes of different sizes or areas wrongly.
#[test]
fn words_with_bytes_of_their_own_are_not_warned_about() {
    assert_eq!(
        overlap_warnings(&tank_configuration("%IW2", "%QW2")),
        vec![]
    );
}

/// %IW0 is bytes 0 and 1, %IW1 bytes 1 and 2: they share byte 1, two warnings. %IW0 and %IW2 share
/// nothing. Catches a check that reads `%IW1` as the second word (bytes 2 and 3), as CODESYS
/// numbers words, which truST does not (specification 11 §6.4).
#[test]
fn consecutive_word_offsets_share_a_byte() {
    let source = |second: &str| {
        format!("PROGRAM Main\nVAR\n    A AT %IW0 : INT;\n    B AT {second} : INT;\nEND_VAR\nEND_PROGRAM\n")
    };
    assert_eq!(overlap_warnings(&source("%IW1")).len(), 2);
    assert_eq!(overlap_warnings(&source("%IW2")), vec![]);
}

/// Two variables at the same address and size are one item two declarations read, as two programs
/// read one input: no warning. A double word and a byte inside it are not: two warnings. Catches a
/// check that warns about every shared address, and one that only compares equal sizes.
#[test]
fn the_same_item_twice_is_no_overlap() {
    let source = "PROGRAM Main\nVAR\n    A AT %IX3.1 : BOOL;\n    B AT %IX3.1 : BOOL;\n    C AT %MD4 : DINT;\n    D AT %MB7 : BYTE;\nEND_VAR\nEND_PROGRAM\n";
    let warnings = overlap_warnings(source);
    assert_eq!(warnings.len(), 2, "{warnings:#?}");
    assert!(warnings
        .iter()
        .all(|warning| warning.message.contains("%MD4") && warning.message.contains("%MB7")));
}

/// %IX0.0, %QW0 and %MW0 lie in three areas, which share nothing. Catches a check that compares
/// offsets across areas.
#[test]
fn different_areas_do_not_overlap() {
    let source = "PROGRAM Main\nVAR\n    A AT %IX0.0 : BOOL;\n    B AT %QW0 : INT;\n    C AT %MW0 : INT;\nEND_VAR\nEND_PROGRAM\n";
    assert_eq!(overlap_warnings(source), vec![]);
}

/// A wildcard (`%I*`), a hierarchical address (`%IX1.2.3`, outside the flat bytes) and the relative
/// addresses of `STRUCT OVERLAP` members (`%B0`, overlapping by design) are not checked: no warning
/// next to %IB1 and %IX0.0. Catches a check that reads `%IX1.2.3` as byte 1, one that compares
/// structure members, and one that takes a wildcard as offset 0.
#[test]
fn wildcards_hierarchical_addresses_and_structure_members_are_not_checked() {
    let source = r#"
TYPE Overlay : STRUCT OVERLAP
    Whole AT %B0 : DWORD;
    Low AT %B0 : WORD;
END_STRUCT;
END_TYPE
PROGRAM Main
VAR
    Free AT %I* : BOOL;
    Deep AT %IX1.2.3 : BOOL;
    Second AT %IB1 : BYTE;
    First AT %IX0.0 : BOOL;
END_VAR
END_PROGRAM
"#;
    assert_eq!(overlap_warnings(source), vec![]);
}

/// A program in one file locates a bit in byte 0, the configuration in another file a word on it:
/// each file gets its warning. Catches a check limited to one file.
#[test]
fn an_overlap_across_files_is_warned_about_in_each() {
    let program =
        "PROGRAM Pump\nVAR\n    Run AT %QX0.3 : BOOL;\n    Speed : INT;\nEND_VAR\nEND_PROGRAM\n";
    let configuration = r#"
CONFIGURATION Plant
RESOURCE Line ON PLC
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Cycle : Pump;
END_RESOURCE
VAR_CONFIG
    P1.Speed AT %QW0 : INT;
END_VAR
END_CONFIGURATION
"#;
    let sources = [program, configuration];
    let in_program = overlap_warnings_in(&sources, 0);
    let in_configuration = overlap_warnings_in(&sources, 1);
    assert_eq!(in_program.len(), 1, "{in_program:#?}");
    assert_eq!(in_configuration.len(), 1, "{in_configuration:#?}");
    assert!(in_program[0].message.contains("'P1.Speed' at %QW0"));
    assert!(
        in_program[0].related.is_empty(),
        "the other declaration is in another file"
    );
}

/// Each `RESOURCE` has a process image of its own (specification 11 §6.4), so a project with two
/// is not checked: no warning for %IX0.0 and %IW0 in different resources. Catches a check that
/// compares declarations of different process images.
#[test]
fn a_project_with_several_resources_is_not_checked() {
    let source = r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION Plant
RESOURCE Left ON PLC
    VAR_GLOBAL
        Start AT %IX0.0 : BOOL;
    END_VAR
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH Cycle : Main;
END_RESOURCE
RESOURCE Right ON PLC
    VAR_GLOBAL
        Level AT %IW0 : INT;
    END_VAR
    TASK Cycle (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P2 WITH Cycle : Main;
END_RESOURCE
END_CONFIGURATION
"#;
    assert_eq!(overlap_warnings(source), vec![]);
}

/// A long word overlapping five bytes names three and counts the rest: "2 more". Catches an
/// unbounded message.
#[test]
fn the_message_names_three_and_counts_the_rest() {
    let source = "PROGRAM Main\nVAR\n    Whole AT %QL0 : LWORD;\n    B0 AT %QB0 : BYTE;\n    B1 AT %QB1 : BYTE;\n    B2 AT %QB2 : BYTE;\n    B3 AT %QB3 : BYTE;\n    B4 AT %QB4 : BYTE;\nEND_VAR\nEND_PROGRAM\n";
    let warnings = overlap_warnings(source);
    let whole = warnings
        .iter()
        .find(|warning| warning.message.starts_with("'Whole'"))
        .expect("the long word's warning");
    assert_eq!(
        whole.message,
        "'Whole' at %QL0 shares process-image storage with 'B0' at %QB0, 'B1' at %QB1, \
         'B2' at %QB2, 2 more: writing one changes the other; give each its own bytes"
    );
    assert_eq!(warnings.len(), 6);
}

/// The storage an address names, in bits from the area's byte 0 (specification 11 §6.4).
#[test]
fn flat_addresses_name_their_bits() {
    assert_eq!(flat_storage("%IX0.0"), Some((LocatedArea::Input, (0, 1))));
    assert_eq!(flat_storage("%i3.7"), Some((LocatedArea::Input, (31, 32))));
    assert_eq!(flat_storage("%QW4"), Some((LocatedArea::Output, (32, 48))));
    assert_eq!(flat_storage("%MD1"), Some((LocatedArea::Memory, (8, 40))));
    assert_eq!(flat_storage("%IL0"), Some((LocatedArea::Input, (0, 64))));
    assert_eq!(flat_storage("%qb2"), Some((LocatedArea::Output, (16, 24))));
    for not_flat in [
        "%IX0.8", "%I*", "%IX1.2.3", "%B0", "%IW", "%IW1.0", "%XW0", "IW0",
    ] {
        assert_eq!(flat_storage(not_flat), None, "{not_flat}");
    }
}
