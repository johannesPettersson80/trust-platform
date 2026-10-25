//! References across an online change, the bound of its events and its tables that do not read
//! (specification 44 §7.3, §12.2; specification 11 §6.7.1).

use alloc::format;
use alloc::string::ToString;
use alloc::vec::Vec;

use trust_plc_abi::{RefValue, AREA_GLOBALS, AREA_INSTANCES, AREA_RETAIN};

use super::super::carry;
use super::{cleared, converted, ints, new_variable, reference, Field, Memory, Program, Row};
use crate::error::RuntimeError;
use crate::native::change::{CarriedValueEvent, MAX_CARRIED_VALUE_EVENTS};
use crate::native::image::{type_kind as kind, SYMBOL_RETAINED};

/// A reference names the same variable in the new program when the new program has it with the
/// type the new reference names, wherever it lies now, and is `NULL` otherwise, which records
/// `Cleared`: a variable the new program does not have, one of another type, an element at an
/// index the new array does not have, a character the new text's capacity does not hold. A
/// reference into an array names the element at the same index; a character reference keeps its
/// index; an interface value names the instance at the same place, which holds a class id; a
/// `NULL` stays `NULL`, also over a starting value that is not (specification 44 §7.3, §12.2;
/// specification 11 §6.7.1).
///
/// ```text
/// running, all in G: Target : INT (0); Table : ARRAY[1..5] OF INT (2); Text : STRING[10] 'abc'
///   (12); Gone : INT (24); Wide : INT (26); Pump : Pump, a class (__CLASS 1, Speed 3) (28);
///   R1..R6 : REF_TO INT (36, 52, 68, 84, 100, 116) := REF(Target), REF(Table[3]), NULL,
///   REF(Gone), REF(Wide), REF(Table[5]); C1, C2 : REF_TO CHAR (132, 148) := character 4 and
///   character 8 of Text; I1 : Drive (164) := Pump; Refs : ARRAY[1..2] OF REF_TO INT (180)
///   := [REF(Target), REF(Gone)]
/// new: RETAIN Target : INT (R 0); in G Table : ARRAY[2..4] OF INT (0); Text : STRING[5] (6);
///   Wide : DINT (16); Pump (20, __CLASS 2); R1..R6 (28, 44, 60, 76, 92, 108), R3 starting at
///   REF(Target); C1, C2 (124, 140); I1 (156); Refs (172); no Gone
/// ```
///
/// Catches a reference copied as its bytes, one named by its old place, one kept to a variable of
/// another type, to an index or a character the new variable does not have, or to a removed
/// variable, an interface value left on the old place, a `NULL` replaced by the new starting
/// value, and a missing or extra event.
#[test]
fn a_reference_names_the_same_variable_in_the_new_program_or_is_null() {
    let mut old = Program::new();
    let (int, char_type) = (old.elementary(kind::INT), old.elementary(kind::CHAR));
    let (udint, table) = (old.elementary(kind::UDINT), old.array(int, &[(1, 5)]));
    let text = old.text(false, 10);
    let pump = old.structure(
        kind::CLASS,
        "Pump",
        &[("__CLASS", udint, 0), ("Speed", int, 0)],
    );
    let (to_int, to_character) = (
        old.reference(kind::REFERENCE, int),
        old.reference(kind::REFERENCE, char_type),
    );
    let drive = old.interface("Drive");
    let refs = old.array(to_int, &[(1, 2)]);
    for (name, ty) in [
        ("Target", int),
        ("Table", table),
        ("Text", text),
        ("Gone", int),
        ("Wide", int),
        ("Pump", pump),
        ("R1", to_int),
        ("R2", to_int),
        ("R3", to_int),
        ("R4", to_int),
        ("R5", to_int),
        ("R6", to_int),
        ("C1", to_character),
        ("C2", to_character),
        ("I1", drive),
        ("Refs", refs),
    ] {
        old.global(name, ty, AREA_GLOBALS, 0);
    }
    let mut new = Program::new();
    let (int, char_type) = (new.elementary(kind::INT), new.elementary(kind::CHAR));
    let (udint, dint, table) = (
        new.elementary(kind::UDINT),
        new.elementary(kind::DINT),
        new.array(int, &[(2, 4)]),
    );
    let text = new.text(false, 5);
    let pump = new.structure(
        kind::CLASS,
        "Pump",
        &[("__CLASS", udint, 0), ("Speed", int, 0)],
    );
    let (to_int, to_character) = (
        new.reference(kind::REFERENCE, int),
        new.reference(kind::REFERENCE, char_type),
    );
    let drive = new.interface("Drive");
    let refs = new.array(to_int, &[(1, 2)]);
    new.global("Target", int, AREA_RETAIN, SYMBOL_RETAINED);
    for (name, ty) in [
        ("Table", table),
        ("Text", text),
        ("Wide", dint),
        ("Pump", pump),
        ("R1", to_int),
        ("R2", to_int),
        ("R3", to_int),
        ("R4", to_int),
        ("R5", to_int),
        ("R6", to_int),
        ("C1", to_character),
        ("C2", to_character),
        ("I1", drive),
        ("Refs", refs),
    ] {
        new.global(name, ty, AREA_GLOBALS, 0);
    }
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    let (variable, instance, character) =
        (RefValue::VARIABLE, RefValue::INSTANCE, RefValue::CHARACTER);
    old.set("TARGET", &5i16.to_le_bytes());
    old.set("TABLE", &ints(&[10, 20, 30, 40, 50]));
    old.set("TEXT", &[3, 0, b'a', b'b', b'c']);
    old.set("PUMP", &[1, 0, 0, 0, 3, 0]);
    old.set("R1", &reference(4, variable, 0, 0, 2));
    old.set("R2", &reference(4, variable, 0, 6, 2));
    old.set("R4", &reference(4, variable, 0, 24, 2));
    old.set("R5", &reference(4, variable, 0, 26, 2));
    old.set("R6", &reference(4, variable, 0, 10, 2));
    old.set("C1", &reference(4, character, 4, 12, 12));
    old.set("C2", &reference(4, character, 8, 12, 12));
    old.set("I1", &reference(4, instance, 0, 28, 8));
    old.set("REFS", &reference(4, variable, 0, 0, 2));
    old.set_at("REFS", 16, &reference(4, variable, 0, 24, 2));
    new.set("PUMP", &[2, 0, 0, 0]);
    new.set("R3", &reference(AREA_RETAIN, variable, 0, 0, 2));

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(
        events,
        [
            cleared("C2"),
            cleared("R4"),
            cleared("R5"),
            cleared("R6"),
            cleared("Refs[2]"),
            converted("Table"),
            converted("Text"),
            converted("Wide"),
        ]
    );
    let null = [0u8; 16];
    assert_eq!(new.get("TARGET", 2), 5i16.to_le_bytes());
    assert_eq!(new.get("R1", 16), reference(AREA_RETAIN, variable, 0, 0, 2));
    // Table[3] of ARRAY[2..4] OF INT at G 0 lies at 0 + (3 - 2) × 2.
    assert_eq!(new.get("R2", 16), reference(4, variable, 0, 2, 2));
    assert_eq!(new.get("R3", 16), null);
    assert_eq!(new.get("R4", 16), null);
    assert_eq!(new.get("R5", 16), null);
    assert_eq!(new.get("R6", 16), null);
    // Character 4 of Text, a STRING[5] of 8 bytes at G 6.
    assert_eq!(new.get("C1", 16), reference(4, character, 4, 6, 8));
    assert_eq!(new.get("C2", 16), null);
    assert_eq!(new.get("I1", 16), reference(4, instance, 0, 20, 8));
    assert_eq!(
        new.get("REFS", 16),
        reference(AREA_RETAIN, variable, 0, 0, 2)
    );
    assert_eq!(new.get_at("REFS", 16, 16), null);
    assert_eq!(new.get("PUMP", 6), [2, 0, 0, 0, 3, 0]);
}

/// The events name each variable by its path as the interpreter names it, a member after a dot
/// and an element of one dimension by its index (`Plant.motor.speed`, `Plant.table[3]`), and stop
/// at `MAX_CARRIED_VALUE_EVENTS`, after which one `More` counts the rest (specification 44 §12.2;
/// specification 11 §6.7.1).
///
/// ```text
/// running: PROGRAM Plant : Line with motor : MotorData (STRUCT speed : INT := 7), table :
///          ARRAY[1..3] OF REF_TO INT := [REF(keep), REF(keep), REF(gone)], gone : INT and
///          keep : INT, in P
/// new:     Plant : Line with motor : MotorData (STRUCT speed : DINT), table and keep, without
///          gone; VAR_GLOBAL V00 .. V39 : INT, which the running program does not have
/// ```
///
/// The symbols come in their order: Plant's two events, then `New` for V00 to V29, which fill the
/// 32, and `More` for the 10 globals V30 to V39. Catches a path formed otherwise, an event past the
/// bound, a count of the events listed, and no count.
#[test]
fn events_stop_at_their_bound_and_count_the_rest() {
    let mut old = Program::new();
    let int = old.elementary(kind::INT);
    let motor = old.structure(kind::STRUCT, "MotorData", &[("speed", int, 0)]);
    let to_int = old.reference(kind::REFERENCE, int);
    let table = old.array(to_int, &[(1, 3)]);
    old.program(
        "Plant",
        "Line",
        &[
            ("motor", motor, AREA_INSTANCES, 0),
            ("table", table, AREA_INSTANCES, 0),
            ("gone", int, AREA_INSTANCES, 0),
            ("keep", int, AREA_INSTANCES, 0),
        ],
    );
    let mut new = Program::new();
    let (int, dint) = (new.elementary(kind::INT), new.elementary(kind::DINT));
    let motor = new.structure(kind::STRUCT, "MotorData", &[("speed", dint, 0)]);
    let to_int = new.reference(kind::REFERENCE, int);
    let table = new.array(to_int, &[(1, 3)]);
    new.program(
        "Plant",
        "Line",
        &[
            ("motor", motor, AREA_INSTANCES, 0),
            ("table", table, AREA_INSTANCES, 0),
            ("keep", int, AREA_INSTANCES, 0),
        ],
    );
    for number in 0..40 {
        new.global(&format!("V{number:02}"), int, AREA_GLOBALS, 0);
    }
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    // Line in the running program: motor (0), table (4), gone (52), keep (54).
    let to_keep = reference(AREA_INSTANCES, RefValue::VARIABLE, 0, 54, 2);
    old.set("PLANT.MOTOR.SPEED", &7i16.to_le_bytes());
    old.set("PLANT.TABLE", &to_keep);
    old.set_at("PLANT.TABLE", 16, &to_keep);
    old.set_at(
        "PLANT.TABLE",
        32,
        &reference(AREA_INSTANCES, RefValue::VARIABLE, 0, 52, 2),
    );

    let events = carry(&old, &mut new).expect("the change carries the values");

    let mut expected = alloc::vec![converted("Plant.motor.speed"), cleared("Plant.table[3]")];
    expected.extend((0..30).map(|number| new_variable(&format!("V{number:02}"))));
    expected.push(CarriedValueEvent::More { count: 10 });
    assert_eq!(MAX_CARRIED_VALUE_EVENTS, 32);
    assert_eq!(events, expected);
    // Line in the new program: motor (0), table (4), keep (52).
    let to_keep = reference(AREA_INSTANCES, RefValue::VARIABLE, 0, 52, 2);
    assert_eq!(new.get("PLANT.MOTOR.SPEED", 4), 7i32.to_le_bytes());
    assert_eq!(new.get_at("PLANT.TABLE", 16, 16), to_keep);
}

/// A table that does not read as the build wrote it fails the change with the damaged program
/// error, and nothing panics or slices outside its bytes: a symbol whose type lies outside the
/// type table, a symbol outside its area, a member outside its structure, and an array whose
/// elements do not fit its bytes (specification 44 §10.2). Catches a panic, an out-of-bounds read,
/// a damaged table carried as if it read, and another error.
#[test]
fn a_damaged_table_fails_the_carry_without_a_panic() {
    let damaged = |old: &Program, new: &Program| {
        let (old, mut new) = (Memory::unchecked(old), Memory::unchecked(new));
        matches!(carry(&old, &mut new), Err(RuntimeError::InvalidBundle(_)))
    };
    let global = |program: &mut Program, ty: u32, offset: u32, size: u32| {
        program.areas[usize::from(AREA_GLOBALS)].resize(2, 0);
        program.rows.push(Row {
            canonical: "G".to_string(),
            display: "G".to_string(),
            ty,
            area: AREA_GLOBALS,
            flags: 0,
            offset,
            size,
            parent: None,
        });
    };

    // A type index past the type table, in the new program.
    let mut old = Program::new();
    let int = old.elementary(kind::INT);
    old.global("G", int, AREA_GLOBALS, 0);
    let mut new = Program::new();
    global(&mut new, 99, 0, 2);
    assert!(damaged(&old, &new), "a type outside the type table");

    // A variable past the end of its area, in the running program.
    let mut old = Program::new();
    let int = old.elementary(kind::INT);
    global(&mut old, int, 100, 2);
    let mut new = Program::new();
    let int = new.elementary(kind::INT);
    new.global("G", int, AREA_GLOBALS, 0);
    assert!(damaged(&old, &new), "a variable outside its area");

    // A member at 4 of a structure of 2 bytes, in the new program.
    let structure = |program: &mut Program, members: Vec<Field>| {
        let int = program.elementary(kind::INT);
        let fields = members
            .into_iter()
            .map(|field| Field { ty: int, ..field })
            .collect();
        let ty = program.layout(kind::STRUCT, "S", (2, 2), fields);
        program.global("G", ty, AREA_GLOBALS, 0);
    };
    let member = |name: &str, offset: u32| Field {
        name: name.to_string(),
        ty: 0,
        offset,
        flags: 0,
    };
    let mut old = Program::new();
    structure(&mut old, alloc::vec![member("A", 0)]);
    let mut new = Program::new();
    structure(&mut new, alloc::vec![member("A", 0), member("B", 4)]);
    assert!(damaged(&old, &new), "a member outside its structure");

    // An array of 4 elements of 2 bytes in a type of 2 bytes, in the running program.
    let mut old = Program::new();
    let int = old.elementary(kind::INT);
    let array = old.array(int, &[(1, 4)]);
    old.types[array as usize].0.size = 2;
    old.global("G", array, AREA_GLOBALS, 0);
    let mut new = Program::new();
    let int = new.elementary(kind::INT);
    let array = new.array(int, &[(1, 3)]);
    new.global("G", array, AREA_GLOBALS, 0);
    assert!(damaged(&old, &new), "an array larger than its type");
}
