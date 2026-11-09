//! Find references through the names the analysis resolved (specification 14 §6.3.2): globals
//! used through their namespace's qualifier, members and methods of an instance declared in
//! another file, and method calls.

use super::*;
use text_size::TextRange;
use trust_ide::{incoming_calls, prepare_call_hierarchy};

fn project(sources: &[&str]) -> Database {
    let mut db = Database::new();
    for (index, source) in sources.iter().enumerate() {
        db.set_source_text(FileId(index as u32), (*source).to_string());
    }
    db
}

/// The references found at `at` in `file`, as (file, range, write), ordered by file and offset.
fn found(db: &Database, file: FileId, at: usize) -> Vec<(u32, TextRange, bool)> {
    let mut references: Vec<_> = find_references(
        db,
        file,
        TextSize::from(at as u32),
        FindReferencesOptions {
            include_declaration: false,
        },
    )
    .into_iter()
    .map(|reference| (reference.file_id.0, reference.range, reference.is_write()))
    .collect();
    references.sort_by_key(|(file, range, _)| (*file, range.start()));
    references
}

/// The range of `member` at the end of each occurrence of `qualified` in `text`.
fn member_ranges(text: &str, qualified: &str, member: &str) -> Vec<TextRange> {
    text.match_indices(qualified)
        .map(|(at, _)| {
            let start = at + qualified.len() - member.len();
            TextRange::new(
                TextSize::from(start as u32),
                TextSize::from((start + member.len()) as u32),
            )
        })
        .collect()
}

const GVL: &str =
    "NAMESPACE GVL_IO\nVAR_GLOBAL\n    lrLT101_Level : LREAL;\nEND_VAR\nEND_NAMESPACE\n";
const TANK: &str = "PROGRAM PRG_Tank\nVAR\n    level : LREAL;\nEND_VAR\nlevel := GVL_IO.lrLT101_Level;\nGVL_IO.lrLT101_Level := level + 1.0;\nEND_PROGRAM\n";

/// Specification 14 §6.3.2: a global used through its namespace's qualifier, as a CODESYS GVL's
/// globals are, is a reference, a read or a write, and rename edits it. Catches the gap a CODESYS
/// toolkit reported on 7 November (`GVL_IO.lrLT101_Level` found only its declaration), from the
/// declaration and from a use, and a rename that leaves the qualified uses behind, which no longer
/// build.
#[test]
fn references_find_a_global_used_through_its_namespace_qualifier() {
    let db = project(&[GVL, TANK]);
    let uses = member_ranges(TANK, "GVL_IO.lrLT101_Level", "lrLT101_Level");
    let expected = vec![(1, uses[0], false), (1, uses[1], true)];
    assert_eq!(
        found(&db, FileId(0), GVL.find("lrLT101_Level").unwrap()),
        expected
    );
    assert_eq!(
        found(&db, FileId(1), usize::from(uses[1].start())),
        expected
    );

    let renamed = rename(
        &db,
        FileId(0),
        TextSize::from(GVL.find("lrLT101_Level").unwrap() as u32),
        "lrLT101_Height",
    )
    .expect("the rename goes through");
    let mut tank_edits: Vec<TextRange> = renamed
        .edits
        .get(&FileId(1))
        .expect("the program's uses are renamed")
        .iter()
        .map(|edit| edit.range)
        .collect();
    tank_edits.sort_by_key(|range| range.start());
    assert_eq!(tank_edits, uses);
}

const FB: &str = "FUNCTION_BLOCK FB_Tank\nVAR_OUTPUT\n    level : INT;\nEND_VAR\nMETHOD PUBLIC Start\nlevel := 1;\nEND_METHOD\nEND_FUNCTION_BLOCK\n";
const PUMP: &str = "FUNCTION_BLOCK FB_Pump\nMETHOD PUBLIC Start\nEND_METHOD\nEND_FUNCTION_BLOCK\n";
const GLOBALS: &str = "NAMESPACE GVL_IO\nVAR_GLOBAL\n    gTank : FB_Tank;\n    gPump : FB_Pump;\nEND_VAR\nEND_NAMESPACE\nVAR_GLOBAL\n    gTankPlain : FB_Tank;\nEND_VAR\n";
const LINE: &str = "PROGRAM PRG_Line\nVAR\n    seen : INT;\nEND_VAR\nGVL_IO.gTank.Start();\nseen := GVL_IO.gTank.level;\nGVL_IO.gPump.Start();\ngTankPlain.Start();\nEND_PROGRAM\n";

/// Specification 14 §6.3.2: a method's call and a member's use through an instance that a GVL holds,
/// reached through the GVL's qualifier as in a CODESYS project, are references, and so are the same
/// through an unqualified global instance. `FB_Pump` has a `Start` method too, so a search by name
/// finds the wrong one. Catches the gap reported on 7 November (method references and calls not
/// found) and a change that loses the unqualified uses, which were found before.
#[test]
fn references_find_members_and_methods_of_an_instance_in_a_gvl() {
    let db = project(&[FB, PUMP, GLOBALS, LINE]);
    let tank_start = member_ranges(LINE, "GVL_IO.gTank.Start", "Start");
    let plain_start = member_ranges(LINE, "gTankPlain.Start", "Start");
    assert_eq!(
        found(&db, FileId(0), FB.find("Start\n").unwrap()),
        vec![(3, tank_start[0], false), (3, plain_start[0], false)]
    );
    let pump_start = member_ranges(LINE, "GVL_IO.gPump.Start", "Start");
    assert_eq!(
        found(&db, FileId(1), PUMP.find("Start\n").unwrap()),
        vec![(3, pump_start[0], false)]
    );
    let level_write = TextRange::at(
        TextSize::from(FB.find("level := 1").unwrap() as u32),
        TextSize::from("level".len() as u32),
    );
    let level_read = member_ranges(LINE, "GVL_IO.gTank.level", "level");
    assert_eq!(
        found(&db, FileId(0), FB.find("level : INT").unwrap()),
        vec![(0, level_write, true), (3, level_read[0], false)]
    );
}

/// Specification 14 §6.3.2: the call hierarchy's incoming calls of a method hold its calls through
/// a GVL's qualifier, with the call's range, and not the calls of another block's method of the same
/// name. Catches the gap reported on 7 November (calls of methods not found) and a callee found by
/// its name alone.
#[test]
fn incoming_calls_of_a_method_hold_its_calls_through_a_gvl() {
    let db = project(&[FB, PUMP, GLOBALS, LINE]);
    let item = prepare_call_hierarchy(
        &db,
        FileId(0),
        TextSize::from(FB.find("Start\n").unwrap() as u32),
    )
    .expect("the method is a call hierarchy item");
    let calls = incoming_calls(&db, &item);
    assert_eq!(calls.len(), 1, "{calls:?}");
    assert_eq!(calls[0].from.name.as_str(), "PRG_Line");
    let mut ranges = calls[0].from_ranges.clone();
    ranges.sort_by_key(|range| range.start());
    let texts: Vec<&str> = ranges
        .iter()
        .map(|range| LINE[usize::from(range.start())..usize::from(range.end())].trim())
        .collect();
    assert_eq!(texts, ["GVL_IO.gTank.Start()", "gTankPlain.Start()"]);
}
