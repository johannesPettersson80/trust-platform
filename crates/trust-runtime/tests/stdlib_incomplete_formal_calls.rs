//! Incomplete formal calls of the standard functions of fixed inputs run with each omitted input at
//! the default value of its type (IEC 61131-3 Ed.3 §6.6.1.4.2; specification 07 §1, "Calls"): 0,
//! `FALSE` for `SEL`'s selector, `T#0s`, and the empty text.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// A call of each function with an input omitted, every literal typed where the type matters.
const INCOMPLETE_CALLS: &str = r#"
PROGRAM Main
VAR
    limitedHigh : INT;
    limitedLow : INT;
    selectedFalse : INT;
    selectedTrue : INT;
    difference : INT;
    quotient : INT;
    rightAngle : REAL;
    straightAngle : REAL;
    power : REAL;
    shifted : BYTE;
    rotated : WORD;
    differs : BOOL;
    same : BOOL;
    emptyChosen : STRING;
    textCapped : STRING;
    textRaised : STRING;
    timeCapped : TIME;
    timeRaised : TIME;
END_VAR
limitedHigh := LIMIT(IN := 120, MX := 100);
limitedLow := LIMIT(IN := -5, MX := 100);
selectedFalse := SEL(IN1 := 7);
selectedTrue := SEL(G := TRUE, IN0 := 3);
difference := SUB(IN2 := 4);
quotient := DIV(IN2 := 4);
rightAngle := ATAN2(Y := REAL#1.0);
straightAngle := ATAN2(X := REAL#-1.0);
power := EXPT(IN1 := REAL#3.0);
shifted := SHL(IN := BYTE#16#81);
rotated := ROR(IN := WORD#16#8001);
differs := NE(IN2 := 5);
same := NE(IN1 := 0);
emptyChosen := SEL(G := TRUE, IN0 := 'x');
textCapped := LIMIT(IN := 'm', MX := 'k');
textRaised := LIMIT(MN := 'c', MX := 'k');
timeCapped := LIMIT(IN := T#5s, MX := T#2s);
timeRaised := LIMIT(MN := T#1s, MX := T#3s);
END_PROGRAM
"#;

// Worked by hand, each omitted input at its default value:
// - LIMIT(MN := 0, IN := 120, MX := 100) is 100, and LIMIT(MN := 0, IN := -5, MX := 100) raises -5
//   to 0;
// - SEL(G := FALSE, IN0 := 0, IN1 := 7) selects IN0, 0, and SEL(G := TRUE, IN0 := 3, IN1 := 0)
//   selects IN1, 0;
// - SUB(0, 4) is -4 and DIV(0, 4) is 0;
// - ATAN2(Y := 1.0, X := 0.0) is the angle of the positive y-axis, pi/2, and ATAN2(Y := +0.0,
//   X := -1.0) the angle of the negative x-axis, +pi; EXPT(3.0, 0) is 1.0;
// - a shift or a rotation by 0 leaves 16#81 and 16#8001 as they are;
// - NE(0, 5) is TRUE and NE(0, 0) FALSE;
// - SEL(TRUE, 'x', '') is the empty text, LIMIT('', 'm', 'k') caps 'm' at 'k', and
//   LIMIT('c', '', 'k') raises the empty text to 'c';
// - LIMIT(T#0s, T#5s, T#2s) is T#2s, and LIMIT(T#1s, T#0s, T#3s) raises T#0s to T#1s.
// Catches a build that refused each ("LIMIT without its MN"), an omitted input that is no default
// value (an uninitialised value, or the given input taken twice), a negative zero for an omitted
// real (which makes the straight angle -pi), and an omitted selector read as TRUE.
#[test]
fn an_incomplete_formal_call_runs_with_each_omitted_input_at_its_default_value() {
    let mut harness = TestHarness::from_source(INCOMPLETE_CALLS).expect("the program builds");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    for (name, expected) in [
        ("limitedHigh", Value::Int(100)),
        ("limitedLow", Value::Int(0)),
        ("selectedFalse", Value::Int(0)),
        ("selectedTrue", Value::Int(0)),
        ("difference", Value::Int(-4)),
        ("quotient", Value::Int(0)),
        ("rightAngle", Value::Real(core::f32::consts::FRAC_PI_2)),
        ("straightAngle", Value::Real(core::f32::consts::PI)),
        ("power", Value::Real(1.0)),
        ("shifted", Value::Byte(0x81)),
        ("rotated", Value::Word(0x8001)),
        ("differs", Value::Bool(true)),
        ("same", Value::Bool(false)),
        ("emptyChosen", Value::String("".into())),
        ("textCapped", Value::String("k".into())),
        ("textRaised", Value::String("c".into())),
        ("timeCapped", Value::Time(Duration::from_millis(2_000))),
        ("timeRaised", Value::Time(Duration::from_millis(1_000))),
    ] {
        assert_eq!(harness.get_output(name), Some(expected), "{name}");
    }
}

/// A call of each string function with an input omitted.
const INCOMPLETE_STRING_CALLS: &str = r#"
PROGRAM Main
VAR
    leftNone : STRING;
    rightNone : STRING;
    midFirst : STRING;
    deletedNone : STRING;
    deletedFirst : STRING;
    insertedFront : STRING;
    insertedAlone : STRING;
    replacedInsert : STRING;
    replacedFirst : STRING;
    foundEmpty : INT;
    foundNothing : INT;
    wideInserted : WSTRING;
END_VAR
leftNone := LEFT(IN := 'Hello');
rightNone := RIGHT(IN := 'Hello');
midFirst := MID(IN := 'Hello', L := 2);
deletedNone := DELETE(IN := 'ABCD', P := 2);
deletedFirst := DELETE(IN := 'ABCD', L := 2);
insertedFront := INSERT(IN1 := 'AC', IN2 := 'B');
insertedAlone := INSERT(IN2 := 'B', P := 1);
replacedInsert := REPLACE(IN1 := 'ABCD', IN2 := 'XX', P := 2);
replacedFirst := REPLACE(IN1 := 'ABCD', IN2 := 'XX', L := 2);
foundEmpty := FIND(IN1 := 'ABC');
foundNothing := FIND(IN2 := 'B');
wideInserted := INSERT(IN2 := "w");
END_PROGRAM
"#;

// Worked by hand from specification 07 §8.1, each omitted length or position 0 and each omitted
// text empty:
// - LEFT and RIGHT of a non-positive length are the empty string;
// - MID('Hello', 2, 0) clamps the position to the first character: 'He';
// - DELETE with a non-positive length leaves 'ABCD' unchanged, and DELETE('ABCD', 2, 0) deletes
//   two characters from the first: 'CD';
// - INSERT('AC', 'B', 0) inserts before the first character: 'BAC'; INSERT('', 'B', 1) appends to
//   the empty text: 'B';
// - REPLACE with a non-positive length inserts at its start: REPLACE('ABCD', 'XX', 0, 2) is
//   'AXXBCD'; REPLACE('ABCD', 'XX', 2, 0) replaces from the first character: 'XXCD';
// - an empty IN2 is found at position 1, and nothing is found in an empty IN1: 0;
// - INSERT("", "w", 0) is "w", a WSTRING, as an omitted text takes the family of the other.
// Catches a build that refused each ("LEFT without its L"), an omitted position taken as 1 where
// the rule clamps 0 (they agree here, so the inserts and the found positions decide it), an omitted
// text that is no empty text, and an empty text of the wrong family.
#[test]
fn an_incomplete_formal_string_call_runs_with_empty_texts_and_zero_counts() {
    let mut harness =
        TestHarness::from_source(INCOMPLETE_STRING_CALLS).expect("the program builds");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    for (name, expected) in [
        ("leftNone", Value::String("".into())),
        ("rightNone", Value::String("".into())),
        ("midFirst", Value::String("He".into())),
        ("deletedNone", Value::String("ABCD".into())),
        ("deletedFirst", Value::String("CD".into())),
        ("insertedFront", Value::String("BAC".into())),
        ("insertedAlone", Value::String("B".into())),
        ("replacedInsert", Value::String("AXXBCD".into())),
        ("replacedFirst", Value::String("XXCD".into())),
        ("foundEmpty", Value::Int(1)),
        ("foundNothing", Value::Int(0)),
        ("wideInserted", Value::WString("w".into())),
    ] {
        assert_eq!(harness.get_output(name), Some(expected), "{name}");
    }
}

/// A call of date and time functions with inputs and outputs omitted; each temporal result is
/// compared with its literal, and `SPLIT_*` writes integers whose starting values show what it
/// wrote.
const INCOMPLETE_DATE_CALLS: &str = r#"
PROGRAM Main
VAR
    timeSumOk : BOOL;
    timeNegatedOk : BOOL;
    afterMidnightOk : BOOL;
    sinceEpochOk : BOOL;
    scaledNoneOk : BOOL;
    noonOk : BOOL;
    mayDayOk : BOOL;
    sixLongOk : BOOL;
    year : INT := 99;
    month : INT := 9;
    day : INT := 9;
    hour : INT := 7;
END_VAR
timeSumOk := ADD_TIME(IN1 := T#1500ms) = T#1500ms;
timeNegatedOk := SUB_TIME(IN2 := T#1s) = T#-1s;
afterMidnightOk := ADD_TOD_TIME(IN2 := T#1h) = TOD#01:00:00;
sinceEpochOk := SUB_DATE_DATE(IN1 := D#1970-01-02) = T#24h;
scaledNoneOk := MUL_TIME(IN1 := T#2s) = T#0s;
noonOk := CONCAT_TOD(HOUR := 12) = TOD#12:00:00;
mayDayOk := CONCAT_DT(YEAR := 2024, MONTH := 5, DAY := 1) = DT#2024-05-01-00:00:00;
sixLongOk := CONCAT_LTOD(HOUR := 6) = LTOD#06:00:00;
SPLIT_DATE(IN := D#2024-05-17, MONTH => month);
SPLIT_TOD(HOUR => hour);
END_PROGRAM
"#;

// Worked by hand, each omitted input at its default value (specification 07 §1):
// - ADD_TIME(T#1500ms, T#0s) is T#1500ms, and SUB_TIME(T#0s, T#1s) is T#-1s;
// - ADD_TOD_TIME(TOD#00:00:00, T#1h) is one in the morning, and
//   SUB_DATE_DATE(D#1970-01-02, D#1970-01-01) one day, T#24h;
// - MUL_TIME(T#2s, 0) is T#0s;
// - CONCAT_TOD(12, 0, 0, 0) is noon, CONCAT_LTOD(6, 0, 0, 0) six in the morning as a long time of
//   day, and CONCAT_DT(2024, 5, 1, 0, 0, 0, 0) the first of May at midnight;
// - SPLIT_DATE of 17 May 2024 writes only its month, 5, and leaves the year and the day it does not
//   name at 99 and 9; SPLIT_TOD of midnight writes the hour 0 over 7.
// Catches a build that refused each ("ADD_TIME without its IN2"), an omitted date that is no epoch
// or an omitted time of day that is no midnight, and a SPLIT that wrote an output the call left
// out.
#[test]
fn an_incomplete_formal_date_call_runs_with_defaults_and_writes_only_its_outputs() {
    let mut harness = TestHarness::from_source(INCOMPLETE_DATE_CALLS).expect("the program builds");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    for name in [
        "timeSumOk",
        "timeNegatedOk",
        "afterMidnightOk",
        "sinceEpochOk",
        "scaledNoneOk",
        "noonOk",
        "mayDayOk",
        "sixLongOk",
    ] {
        assert_eq!(harness.get_output(name), Some(Value::Bool(true)), "{name}");
    }
    for (name, expected) in [("year", 99), ("month", 5), ("day", 9), ("hour", 0)] {
        assert_eq!(
            harness.get_output(name),
            Some(Value::Int(expected)),
            "{name}"
        );
    }
}
