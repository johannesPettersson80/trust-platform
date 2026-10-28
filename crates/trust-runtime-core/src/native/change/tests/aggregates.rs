//! Structures, arrays and function block and class instances across an online change
//! (specification 44 §12.2; specification 11 §6.7.1).

use alloc::string::ToString;
use alloc::vec;

use trust_plc_abi::{RefValue, AREA_GLOBALS};

use super::super::carry;
use super::{converted, dints, ints, new_variable, not_kept, reference, Field, Memory, Program};
use crate::native::image::{
    type_kind as kind, SYMBOL_CONSTANT, SYMBOL_INPUT, SYMBOL_IN_OUT, SYMBOL_OUTPUT,
};

/// A member of a structure whose layout `Program::layout` takes as it is.
fn field(name: &str, ty: u32, offset: u32) -> Field {
    Field {
        name: name.to_string(),
        ty,
        offset,
        flags: 0,
    }
}

/// A structure keeps its members by name, whatever their order; a member whose type changed is
/// kept by the elementary rule and records its own event, and a member the running value does not
/// have keeps its starting value and records `New` with its path, as the interpreter records it
/// (specification 11 §6.7.1). A structure whose members share bytes (`STRUCT OVERLAP`) and a union
/// are kept only into the same layout; a changed one keeps its starting value and records
/// `NotKept` (specification 44 §12.2).
///
/// ```text
/// running: TYPE Point : STRUCT X : INT; Y : INT; END_STRUCT
///          TYPE Flags : STRUCT OVERLAP Word : WORD (0); Low : BYTE (0); High : BYTE (1)
///          TYPE Pair : STRUCT OVERLAP Whole : WORD (0); Part : BYTE (0)
///          TYPE Either : UNION I : DINT; R : REAL
///          Bits : Flags := 16#1234 (G 0); Pos : Point := (X := 3, Y := 4) (G 2);
///          Same : Pair := 16#ABCD (G 6); U : Either := (I := 7) (G 8)
/// new:     TYPE Point : STRUCT Y : DINT; X : INT; Z : INT := 9; END_STRUCT
///          TYPE Flags : STRUCT OVERLAP Word : WORD (0); Low : BYTE (1); High : BYTE (0)
///          Pair unchanged; TYPE Either : UNION I : DINT; R : LREAL
///          Bits (G 0); Pos (G 4); Same (G 12); U (G 16)
/// ```
///
/// Catches members matched by their place, a changed member copied as bytes, a new member
/// overwritten or not reported, a changed layout carried member by member, and an unchanged one
/// not carried.
#[test]
fn a_structure_keeps_its_members_by_name() {
    let mut old = Program::new();
    let (int, word, byte, dint, real) = (
        old.elementary(kind::INT),
        old.elementary(kind::WORD),
        old.elementary(kind::BYTE),
        old.elementary(kind::DINT),
        old.elementary(kind::REAL),
    );
    let point = old.structure(kind::STRUCT, "Point", &[("X", int, 0), ("Y", int, 0)]);
    let flags = old.layout(
        kind::STRUCT,
        "Flags",
        (2, 2),
        vec![
            field("Word", word, 0),
            field("Low", byte, 0),
            field("High", byte, 1),
        ],
    );
    let pair = old.layout(
        kind::STRUCT,
        "Pair",
        (2, 2),
        vec![field("Whole", word, 0), field("Part", byte, 0)],
    );
    let either = old.layout(
        kind::UNION,
        "Either",
        (4, 4),
        vec![field("I", dint, 0), field("R", real, 0)],
    );
    old.global("Bits", flags, AREA_GLOBALS, 0);
    old.global("Pos", point, AREA_GLOBALS, 0);
    old.global("Same", pair, AREA_GLOBALS, 0);
    old.global("U", either, AREA_GLOBALS, 0);
    let mut new = Program::new();
    let (int, word, byte, dint, lreal) = (
        new.elementary(kind::INT),
        new.elementary(kind::WORD),
        new.elementary(kind::BYTE),
        new.elementary(kind::DINT),
        new.elementary(kind::LREAL),
    );
    let point = new.structure(
        kind::STRUCT,
        "Point",
        &[("Y", dint, 0), ("X", int, 0), ("Z", int, 0)],
    );
    let flags = new.layout(
        kind::STRUCT,
        "Flags",
        (2, 2),
        vec![
            field("Word", word, 0),
            field("Low", byte, 1),
            field("High", byte, 0),
        ],
    );
    let pair = new.layout(
        kind::STRUCT,
        "Pair",
        (2, 2),
        vec![field("Whole", word, 0), field("Part", byte, 0)],
    );
    let either = new.layout(
        kind::UNION,
        "Either",
        (8, 8),
        vec![field("I", dint, 0), field("R", lreal, 0)],
    );
    new.global("Bits", flags, AREA_GLOBALS, 0);
    new.global("Pos", point, AREA_GLOBALS, 0);
    new.global("Same", pair, AREA_GLOBALS, 0);
    new.global("U", either, AREA_GLOBALS, 0);
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("BITS", &0x1234u16.to_le_bytes());
    old.set("POS", &ints(&[3, 4]));
    old.set("SAME", &0xABCDu16.to_le_bytes());
    old.set("U", &7i32.to_le_bytes());
    new.set("POS.Z", &9i16.to_le_bytes());

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(
        events,
        [
            not_kept("Bits"),
            converted("Pos.Y"),
            new_variable("Pos.Z"),
            not_kept("U"),
        ]
    );
    assert_eq!(new.get("BITS", 2), [0, 0]);
    // Y : DINT at 0, X : INT at 4, Z : INT at 6.
    assert_eq!(new.get("POS", 8), [4, 0, 0, 0, 3, 0, 9, 0]);
    assert_eq!(new.get("SAME", 2), [0xCD, 0xAB]);
    assert_eq!(new.get("U", 8), [0; 8]);
}

/// An array keeps the elements at the indices both arrays have, per dimension, and the others keep
/// their starting values (specification 44 §12.2; specification 11 §6.7.1). Elements of the same
/// type are copied as bytes; elements of a changed type are each kept by the elementary rule, and
/// the array records one event, as the interpreter records it: `Converted`, or `NotKept` when an
/// element was not kept. A reference among such elements is muted as well: the interpreter, which
/// carries references last, records a `Cleared` for each, while here the array's one event says
/// that an element was lost. A different number of dimensions keeps nothing.
///
/// ```text
/// T1 : ARRAY[1..5] OF INT [10, 20, 30, 40, 50] to ARRAY[3..8] OF INT, all 7
///      3..5 kept, 6..8 start: [30, 40, 50, 7, 7, 7], Converted
/// T2 : ARRAY[1..3] OF INT [1, 2, 3] to ARRAY[1..3] OF DINT: [1, 2, 3], Converted
/// T3 : ARRAY[1..3] OF DINT [1, 70000, 3] to ARRAY[1..3] OF INT, all 9: [1, 9, 3], NotKept
/// T4 : ARRAY[1..2, 1..2] OF INT to ARRAY[1..4] OF INT, all 5: unchanged, NotKept
/// T5 : ARRAY[1..2, 1..3] OF INT [1, 2, 3, 4, 5, 6] to ARRAY[1..3, 1..2] OF INT:
///      (1,1) 1, (1,2) 2, (2,1) 4, (2,2) 5, (3,1) and (3,2) start 0, Converted
/// T6 : ARRAY[0..2] OF INT [7, 8, 9] unchanged: copied, nothing recorded
/// Target : INT (G 54, new G 56)
/// T7 : ARRAY[1..2] OF REF_TO INT [REF(Target), NULL] to ARRAY[1..2] OF REF_TO DINT starting
///      [REF(Target), NULL]: a DINT reference cannot name the INT Target: [NULL, NULL], NotKept
/// ```
///
/// Catches elements matched by their position instead of their index, an index the running array
/// does not have taken from beyond it, an event per element, a lost element reported as converted,
/// and a change of dimensions carried.
#[test]
fn an_array_keeps_the_elements_at_the_indices_both_have() {
    let mut old = Program::new();
    let (int, dint) = (old.elementary(kind::INT), old.elementary(kind::DINT));
    let declarations = [
        ("T1", old.array(int, &[(1, 5)])),
        ("T2", old.array(int, &[(1, 3)])),
        ("T3", old.array(dint, &[(1, 3)])),
        ("T4", old.array(int, &[(1, 2), (1, 2)])),
        ("T5", old.array(int, &[(1, 2), (1, 3)])),
        ("T6", old.array(int, &[(0, 2)])),
    ];
    for (name, ty) in declarations {
        old.global(name, ty, AREA_GLOBALS, 0);
    }
    old.global("Target", int, AREA_GLOBALS, 0);
    let to_int = old.reference(kind::REFERENCE, int);
    let references = old.array(to_int, &[(1, 2)]);
    old.global("T7", references, AREA_GLOBALS, 0);
    let mut new = Program::new();
    let (int, dint) = (new.elementary(kind::INT), new.elementary(kind::DINT));
    let declarations = [
        ("T1", new.array(int, &[(3, 8)])),
        ("T2", new.array(dint, &[(1, 3)])),
        ("T3", new.array(int, &[(1, 3)])),
        ("T4", new.array(int, &[(1, 4)])),
        ("T5", new.array(int, &[(1, 3), (1, 2)])),
        ("T6", new.array(int, &[(0, 2)])),
    ];
    for (name, ty) in declarations {
        new.global(name, ty, AREA_GLOBALS, 0);
    }
    new.global("Target", int, AREA_GLOBALS, 0);
    let to_dint = new.reference(kind::REFERENCE, dint);
    let references = new.array(to_dint, &[(1, 2)]);
    new.global("T7", references, AREA_GLOBALS, 0);
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("T1", &ints(&[10, 20, 30, 40, 50]));
    old.set("T2", &ints(&[1, 2, 3]));
    old.set("T3", &dints(&[1, 70_000, 3]));
    old.set("T4", &ints(&[1, 2, 3, 4]));
    old.set("T5", &ints(&[1, 2, 3, 4, 5, 6]));
    old.set("T6", &ints(&[7, 8, 9]));
    old.set("T7", &reference(AREA_GLOBALS, RefValue::VARIABLE, 0, 54, 2));
    new.set("T1", &ints(&[7; 6]));
    new.set("T3", &ints(&[9; 3]));
    new.set("T4", &ints(&[5; 4]));
    new.set("T7", &reference(AREA_GLOBALS, RefValue::VARIABLE, 0, 56, 2));

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(
        events,
        [
            converted("T1"),
            converted("T2"),
            not_kept("T3"),
            not_kept("T4"),
            converted("T5"),
            not_kept("T7"),
        ]
    );
    assert_eq!(new.get("T7", 32), [0; 32]);
    assert_eq!(new.get("T1", 12), ints(&[30, 40, 50, 7, 7, 7]));
    assert_eq!(new.get("T2", 12), dints(&[1, 2, 3]));
    assert_eq!(new.get("T3", 6), ints(&[1, 9, 3]));
    assert_eq!(new.get("T4", 8), ints(&[5; 4]));
    assert_eq!(new.get("T5", 12), ints(&[1, 2, 4, 5, 0, 0]));
    assert_eq!(new.get("T6", 6), ints(&[7, 8, 9]));
}

/// A function block instance keeps its members by name; an in-out member, which every call binds
/// again, and a constant are not carried and record nothing; a member the running instance does
/// not have records `New` with its path. A library block's own state (`__LAST`) is kept into an
/// instance of the same block, and only there, and records nothing either way, while the block's
/// declared members are kept by name into another block (specification 44 §12.2; specification 11
/// §6.7.1).
///
/// ```text
/// FUNCTION_BLOCK TON / TOF: IN : BOOL (0); Q : BOOL (1); ET : TIME (8); __LAST : LTIME (16)
/// running: FUNCTION_BLOCK Motor  Speed : INT (0); Limit : INT CONSTANT (2);
///                                Link : VAR_IN_OUT INT (4); Timer : TON (24)
///          FUNCTION_BLOCK Holder Timer : TON (0)
///          M : Motor := (Speed 5, Limit 10, Link REF(Target),
///                        Timer (IN TRUE, Q TRUE, ET T#2s, __LAST 123)) (G 8)
///          W : Holder := (Timer (IN TRUE, Q FALSE, ET T#3s, __LAST 456)) (G 56)
/// new:     Motor gains Count : INT := 4 (48), starts with Limit 20 and its in-out 16#EE..;
///          Holder's Timer is a TOF whose __LAST starts at 777
///          M (G 8); W (G 64)
/// ```
///
/// Catches a timer's state carried into another block or lost in the same one, a constant or an
/// in-out carried (or cleared, with an event), a member matched by place, and a missing `New`.
#[test]
fn an_instance_keeps_its_members_and_its_own_state_only_into_the_same_block() {
    let mut old = Program::new();
    let blocks = |program: &mut Program, holder_timer: &str| {
        let (boolean, time, ltime, int) = (
            program.elementary(kind::BOOL),
            program.elementary(kind::TIME),
            program.elementary(kind::LTIME),
            program.elementary(kind::INT),
        );
        let timer = |program: &mut Program, name: &str| {
            program.structure(
                kind::FUNCTION_BLOCK,
                name,
                &[
                    ("IN", boolean, SYMBOL_INPUT),
                    ("Q", boolean, SYMBOL_OUTPUT),
                    ("ET", time, SYMBOL_OUTPUT),
                    ("__LAST", ltime, 0),
                ],
            )
        };
        let ton = timer(program, "TON");
        let holder = timer(program, holder_timer);
        let link = program.reference(kind::REFERENCE, int);
        (int, ton, holder, link)
    };
    let (int, ton, holder_timer, link) = blocks(&mut old, "TON");
    let motor = old.structure(
        kind::FUNCTION_BLOCK,
        "Motor",
        &[
            ("Speed", int, 0),
            ("Limit", int, SYMBOL_CONSTANT),
            ("Link", link, SYMBOL_IN_OUT),
            ("Timer", ton, 0),
        ],
    );
    let holder = old.structure(
        kind::FUNCTION_BLOCK,
        "Holder",
        &[("Timer", holder_timer, 0)],
    );
    old.global("Target", int, AREA_GLOBALS, 0);
    old.global("M", motor, AREA_GLOBALS, 0);
    old.global("W", holder, AREA_GLOBALS, 0);
    let mut new = Program::new();
    let (int, ton, holder_timer, link) = blocks(&mut new, "TOF");
    let motor = new.structure(
        kind::FUNCTION_BLOCK,
        "Motor",
        &[
            ("Speed", int, 0),
            ("Limit", int, SYMBOL_CONSTANT),
            ("Link", link, SYMBOL_IN_OUT),
            ("Timer", ton, 0),
            ("Count", int, 0),
        ],
    );
    let holder = new.structure(
        kind::FUNCTION_BLOCK,
        "Holder",
        &[("Timer", holder_timer, 0)],
    );
    new.global("Target", int, AREA_GLOBALS, 0);
    new.global("M", motor, AREA_GLOBALS, 0);
    new.global("W", holder, AREA_GLOBALS, 0);
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    let link_to_target = reference(AREA_GLOBALS, RefValue::VARIABLE, 0, 0, 2);
    old.set("M.SPEED", &5i16.to_le_bytes());
    old.set("M.LIMIT", &10i16.to_le_bytes());
    old.set("M.LINK", &link_to_target);
    old.set("M.TIMER", &[1, 1]);
    old.set("M.TIMER.ET", &2_000_000_000i64.to_le_bytes());
    old.set_at("M.TIMER", 16, &123i64.to_le_bytes());
    old.set("W.TIMER", &[1, 0]);
    old.set("W.TIMER.ET", &3_000_000_000i64.to_le_bytes());
    old.set_at("W.TIMER", 16, &456i64.to_le_bytes());
    new.set("M.LIMIT", &20i16.to_le_bytes());
    new.set("M.LINK", &[0xEE; 16]);
    new.set("M.COUNT", &4i16.to_le_bytes());
    new.set_at("W.TIMER", 16, &777i64.to_le_bytes());

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(events, [new_variable("M.Count")]);
    assert_eq!(new.get("M.SPEED", 2), 5i16.to_le_bytes());
    assert_eq!(new.get("M.LIMIT", 2), 20i16.to_le_bytes());
    assert_eq!(new.get("M.LINK", 16), [0xEE; 16]);
    assert_eq!(new.get("M.TIMER", 2), [1, 1]);
    assert_eq!(new.get("M.TIMER.ET", 8), 2_000_000_000i64.to_le_bytes());
    assert_eq!(new.get_at("M.TIMER", 16, 8), 123i64.to_le_bytes());
    assert_eq!(new.get("M.COUNT", 2), 4i16.to_le_bytes());
    assert_eq!(new.get("W.TIMER", 2), [1, 0]);
    assert_eq!(new.get("W.TIMER.ET", 8), 3_000_000_000i64.to_le_bytes());
    assert_eq!(new.get_at("W.TIMER", 16, 8), 777i64.to_le_bytes());
}

/// An instance's class id (`__CLASS`) is always the new program's, which may number its classes
/// otherwise, also when the instance's type did not change and every other byte of it is carried;
/// a base's members, whether the type lists them as its own, as the build writes a derived type,
/// or holds them through `__BASE`, are carried by name (specification 44 §6.4, §7.2, §12.2).
///
/// ```text
/// CLASS Derived, as the build lists it: __CLASS : UDINT (0); A : INT (4); B : INT (6)
/// CLASS Root: __CLASS : UDINT (0); A : INT (4); CLASS Leaf EXTENDS Root, as __BASE: Root (0);
///   B : INT (8), and in the new program C : INT := 8 (10) too
/// running: D : Derived := (class 3, A 10, B 20) (G 0); L : Leaf := (class 2, A 11, B 21) (G 8)
/// new:     D with class 7 (G 0); L with class 5 (G 8)
/// ```
///
/// Catches a class id carried from the running program, an identical instance copied whole, a
/// base's member left behind through `__BASE`, and a missing `New` for the derived type's new
/// member.
#[test]
fn the_class_id_is_always_the_new_programs() {
    let classes = |program: &mut Program, extra: bool| {
        let (udint, int) = (
            program.elementary(kind::UDINT),
            program.elementary(kind::INT),
        );
        let derived = program.structure(
            kind::CLASS,
            "Derived",
            &[("__CLASS", udint, 0), ("A", int, 0), ("B", int, 0)],
        );
        let root = program.structure(kind::CLASS, "Root", &[("__CLASS", udint, 0), ("A", int, 0)]);
        let leaf = if extra {
            program.structure(
                kind::CLASS,
                "Leaf",
                &[("__BASE", root, 0), ("B", int, 0), ("C", int, 0)],
            )
        } else {
            program.structure(kind::CLASS, "Leaf", &[("__BASE", root, 0), ("B", int, 0)])
        };
        program.global("D", derived, AREA_GLOBALS, 0);
        program.global("L", leaf, AREA_GLOBALS, 0);
    };
    let mut old = Program::new();
    classes(&mut old, false);
    let mut new = Program::new();
    classes(&mut new, true);
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("D", &[3, 0, 0, 0, 10, 0, 20, 0]);
    old.set("L", &[2, 0, 0, 0, 11, 0, 0, 0, 21, 0, 0, 0]);
    new.set("D", &[7, 0, 0, 0, 0, 0, 0, 0]);
    new.set("L", &[5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 8, 0]);

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(events, [new_variable("L.C")]);
    assert_eq!(new.get("D", 8), [7, 0, 0, 0, 10, 0, 20, 0]);
    assert_eq!(new.get("L", 12), [5, 0, 0, 0, 11, 0, 0, 0, 21, 0, 8, 0]);
}

/// A reference lost inside an element of an array whose element type changed makes the array's
/// one event `NotKept`, also when the reference lies in an inner array whose own type did not
/// change: the inner array records nothing itself and passes the loss on (specification 44 §12.2,
/// one event for one variable).
///
/// ```text
/// running: TYPE Cell : STRUCT A : INT; R : ARRAY[1..1] OF REF_TO INT; END_STRUCT
///          Gone : INT (G 0); Grid : ARRAY[1..1] OF Cell := [(A := 1, R := [REF(Gone)])] (G 4)
/// new:     TYPE Cell : STRUCT A : DINT; R : ARRAY[1..1] OF REF_TO INT; END_STRUCT
///          Grid (G 0), no Gone: Grid[1].A is 1 and Grid[1].R[1] NULL
/// ```
///
/// Catches a loss in the inner array dropped, which reports the outer array as converted.
#[test]
fn a_loss_inside_an_inner_array_is_the_outer_arrays() {
    let cells = |program: &mut Program, a: u32| {
        let int = program.elementary(kind::INT);
        let to_int = program.reference(kind::REFERENCE, int);
        let row = program.array(to_int, &[(1, 1)]);
        let a = program.elementary(a);
        let cell = program.structure(kind::STRUCT, "Cell", &[("A", a, 0), ("R", row, 0)]);
        (int, program.array(cell, &[(1, 1)]))
    };
    let mut old = Program::new();
    let (int, grid) = cells(&mut old, kind::INT);
    old.global("Gone", int, AREA_GLOBALS, 0);
    old.global("Grid", grid, AREA_GLOBALS, 0);
    let mut new = Program::new();
    let (_, grid) = cells(&mut new, kind::DINT);
    new.global("Grid", grid, AREA_GLOBALS, 0);
    let (mut old, mut new) = (Memory::of(&old), Memory::of(&new));
    old.set("GRID", &1i16.to_le_bytes());
    old.set_at(
        "GRID",
        4,
        &reference(AREA_GLOBALS, RefValue::VARIABLE, 0, 0, 2),
    );

    let events = carry(&old, &mut new).expect("the change carries the values");

    assert_eq!(events, [not_kept("Grid")]);
    let mut grid = vec![1u8, 0, 0, 0];
    grid.resize(20, 0);
    assert_eq!(new.get("GRID", 20), grid);
}
