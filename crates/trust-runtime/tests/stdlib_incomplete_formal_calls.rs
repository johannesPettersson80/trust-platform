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
