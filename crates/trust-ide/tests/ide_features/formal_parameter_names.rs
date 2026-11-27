//! A formal parameter name in a call names the callee's parameter (specification 14 §6.3.2;
//! IEC 61131-3 Ed.3 §6.6.1.4.2, Table 20): hover, go to definition, references and rename reach
//! it from the call, and reach the call from the parameter.

use super::*;
use std::collections::BTreeSet;
use trust_hir::access::Access;

/// The blocks and the function, in a file of their own as a library's would be.
const BLOCKS: &str = r#"
FUNCTION_BLOCK FB_Valve
VAR_INPUT
    Open : BOOL;
END_VAR
VAR_OUTPUT
    IsOpen : BOOL;
END_VAR
IsOpen := Open;
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_FastValve EXTENDS FB_Valve
METHOD PUBLIC Prime
VAR_INPUT
    Speed : INT;
END_VAR
END_METHOD
END_FUNCTION_BLOCK

FUNCTION Limit : INT
VAR_INPUT
    Value : INT;
END_VAR
Limit := Value;
END_FUNCTION

TYPE ST_Bundle :
STRUCT
    Valve : FB_Valve;
    Timer : TON;
END_STRUCT
END_TYPE
"#;

/// The program that calls them, with formal parameter names.
const MAIN: &str = r#"
PROGRAM Main
VAR
    Valve : FB_Valve;
    Fast : FB_FastValve;
    Timer : TON;
    Done : BOOL;
    Level : INT;
    Bundle : ST_Bundle;
END_VAR
Valve(Open := TRUE, IsOpen => Done);
Fast(Open := Done);
Fast.Prime(Speed := 3);
Level := Limit(Value := 5);
Timer(IN := Done, PT := T#1s);
Bundle.Valve(Open := Done);
Bundle.Timer(IN := Done, PT := T#2s);
END_PROGRAM
"#;

const BLOCKS_FILE: FileId = FileId(0);
const MAIN_FILE: FileId = FileId(1);

fn database() -> Database {
    let mut db = Database::new();
    db.set_source_text(BLOCKS_FILE, BLOCKS.to_string());
    db.set_source_text(MAIN_FILE, MAIN.to_string());
    db
}

/// The offset of the `nth` (from 0) occurrence of `needle` in `text`.
fn offset(text: &str, needle: &str, nth: usize) -> u32 {
    text.match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("no occurrence {nth} of {needle}"))
        .0 as u32
}

/// The references of the symbol at `at` in `file`, with the declaration, as (file, offset,
/// access).
fn references(db: &Database, file: FileId, at: u32) -> BTreeSet<(u32, u32, String)> {
    find_references(
        db,
        file,
        TextSize::from(at),
        FindReferencesOptions {
            include_declaration: true,
        },
    )
    .into_iter()
    .map(|reference| {
        (
            reference.file_id.0,
            u32::from(reference.range.start()),
            format!("{:?}", reference.access),
        )
    })
    .collect()
}

/// Catches a formal name that hover passes over: it shows the callee's parameter, a block's,
/// an inherited one, a method's, a function's and a standard block's.
#[test]
fn hovering_a_formal_parameter_name_shows_the_callees_parameter() {
    let db = database();
    for (needle, nth, shown) in [
        ("Open :=", 0, "```st\n(IN) Open : BOOL"),
        ("IsOpen =>", 0, "```st\n(OUT) IsOpen : BOOL"),
        ("Open :=", 1, "```st\n(IN) Open : BOOL"),
        ("Speed :=", 0, "```st\n(IN) Speed : INT"),
        ("Value :=", 0, "```st\n(IN) Value : INT"),
        ("IN :=", 0, "```st\n(IN) IN : BOOL"),
        ("PT :=", 0, "```st\n(IN) PT : TIME"),
        // Instances held in a structure's fields (Codex's review of #844).
        ("Open :=", 2, "```st\n(IN) Open : BOOL"),
        ("IN :=", 1, "```st\n(IN) IN : BOOL"),
    ] {
        let at = TextSize::from(offset(MAIN, needle, nth) + 1);
        let contents = hover(&db, MAIN_FILE, at)
            .unwrap_or_else(|| panic!("no hover at {needle} ({nth})"))
            .contents;
        assert!(contents.starts_with(shown), "{needle} ({nth}): {contents}");
    }
}

/// Catches a formal name from which go to definition reaches nothing, or the instance instead of
/// the parameter declared in the other file.
#[test]
fn go_to_definition_of_a_formal_parameter_name_reaches_the_parameter() {
    let db = database();
    for (needle, nth, declared) in [
        ("Open :=", 0, "Open : BOOL"),
        ("Open :=", 1, "Open : BOOL"),
        ("IsOpen =>", 0, "IsOpen : BOOL"),
        ("Speed :=", 0, "Speed : INT"),
        ("Value :=", 0, "Value : INT"),
        ("Open :=", 2, "Open : BOOL"),
    ] {
        let at = TextSize::from(offset(MAIN, needle, nth) + 1);
        let definition = goto_definition(&db, MAIN_FILE, at)
            .unwrap_or_else(|| panic!("no definition at {needle} ({nth})"));
        assert_eq!(definition.file_id, BLOCKS_FILE, "{needle} ({nth})");
        assert_eq!(
            u32::from(definition.range.start()),
            offset(BLOCKS, declared, 0),
            "{needle} ({nth})"
        );
    }
}

/// Catches references of a parameter that leave out the calls' formal names, or that give them the
/// access of the variable the call binds: `:=` writes the input, `=>` reads the output, and the
/// derived block's call binds the input it inherits. Asked from the declaration and from a call,
/// the answer is the same.
#[test]
fn references_of_a_parameter_include_every_calls_formal_name() {
    let db = database();
    let write = format!("{:?}", Access::Write);
    let read = format!("{:?}", Access::Read);
    let positions = |found: &BTreeSet<(u32, u32, String)>| -> BTreeSet<(u32, u32)> {
        found.iter().map(|(file, at, _)| (*file, *at)).collect()
    };
    let open: BTreeSet<(u32, u32)> = [
        (0, offset(BLOCKS, "Open : BOOL", 0)),
        (0, offset(BLOCKS, "Open;", 0)),
        (1, offset(MAIN, "Open :=", 0)),
        (1, offset(MAIN, "Open :=", 1)),
        (1, offset(MAIN, "Open :=", 2)),
    ]
    .into_iter()
    .collect();
    let from_declaration = references(&db, BLOCKS_FILE, offset(BLOCKS, "Open : BOOL", 0));
    assert_eq!(positions(&from_declaration), open);
    let from_call = references(&db, MAIN_FILE, offset(MAIN, "Open :=", 1) + 1);
    assert_eq!(from_call, from_declaration);
    for nth in [0, 1, 2] {
        let formal = (1, offset(MAIN, "Open :=", nth), write.clone());
        assert!(from_call.contains(&formal), "{formal:?} in {from_call:?}");
    }
    let body = (0, offset(BLOCKS, "Open;", 0), read.clone());
    assert!(from_call.contains(&body), "{body:?} in {from_call:?}");

    let is_open = references(&db, MAIN_FILE, offset(MAIN, "IsOpen =>", 0) + 1);
    let formal = (1, offset(MAIN, "IsOpen =>", 0), read);
    assert!(is_open.contains(&formal), "{formal:?} in {is_open:?}");
    let assigned = (0, offset(BLOCKS, "IsOpen := Open", 0), write);
    assert!(is_open.contains(&assigned), "{assigned:?} in {is_open:?}");
}

/// Catches a rename of a parameter that leaves the calls' formal names behind, so that each call
/// names a parameter that no longer exists (it did until 27 November 2026), from the declaration
/// and from a call, for a block's, a method's and a function's parameter.
#[test]
fn renaming_a_parameter_renames_every_calls_formal_name() {
    let db = database();
    for (file, source, needle, nth, new_name, expected) in [
        (
            BLOCKS_FILE,
            BLOCKS,
            "Open : BOOL",
            0,
            "OpenCmd",
            vec![
                (0, offset(BLOCKS, "Open : BOOL", 0)),
                (0, offset(BLOCKS, "Open;", 0)),
                (1, offset(MAIN, "Open :=", 0)),
                (1, offset(MAIN, "Open :=", 1)),
                (1, offset(MAIN, "Open :=", 2)),
            ],
        ),
        (
            MAIN_FILE,
            MAIN,
            "Speed :=",
            0,
            "Rate",
            vec![
                (0, offset(BLOCKS, "Speed : INT", 0)),
                (1, offset(MAIN, "Speed :=", 0)),
            ],
        ),
        (
            MAIN_FILE,
            MAIN,
            "Value :=",
            0,
            "Input",
            vec![
                (0, offset(BLOCKS, "Value : INT", 0)),
                (0, offset(BLOCKS, "Value;", 0)),
                (1, offset(MAIN, "Value :=", 0)),
            ],
        ),
    ] {
        let at = TextSize::from(offset(source, needle, nth) + 1);
        let result = rename(&db, file, at, new_name)
            .unwrap_or_else(|| panic!("the rename of {needle} to {new_name} was refused"));
        let mut edits: Vec<(u32, u32)> = result
            .edits
            .iter()
            .flat_map(|(file, edits)| {
                edits.iter().map(move |edit| {
                    assert_eq!(edit.new_text, new_name);
                    (file.0, u32::from(edit.range.start()))
                })
            })
            .collect();
        edits.sort_unstable();
        assert_eq!(edits, expected, "renaming {needle} to {new_name}");
    }
}
