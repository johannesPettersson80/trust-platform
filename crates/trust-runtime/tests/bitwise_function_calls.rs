//! The bitwise and modulo functions called by their names, run as the compiled program (IEC
//! 61131-3 Ed.3 Table 31, whose usage example is `A:= AND(B, C, D)`, and Table 29 for `MOD`;
//! specification 07 §5; checklist G226). Every expected value is worked out by hand.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

const SOURCE: &str = r#"
PROGRAM Main
VAR
    w : WORD := 16#1234;
    masked : WORD;
    merged : WORD;
    flipped : BYTE;
    open : BYTE;
    rest : INT;
    both : BOOL := TRUE;
END_VAR
masked := AND(w, 16#00FF);
merged := OR(w, 16#F000, 16#000F);
flipped := XOR(BYTE#16#0F, 16#FF);
open := AND(16#F0, 16#3C);
rest := MOD(17, 5);
both := AND(TRUE, FALSE);
END_PROGRAM
"#;

#[test]
fn the_functions_called_by_name_compute_as_their_operators() {
    // masked: 16#1234 AND 16#00FF is 16#0034. merged: 16#1234 OR 16#F000 OR 16#000F is 16#F23F, the
    // three inputs of the extensible function. flipped: 16#0F XOR 16#FF is 16#F0. open: the
    // literals alone take the BYTE of their target, 16#F0 AND 16#3C is 16#30. rest: 17 MOD 5 is 2.
    // both: TRUE AND FALSE is FALSE. Catches a call by name that does not compile, a third input
    // dropped, a literal beside a WORD refused, and MOD by name computing another operation.
    let mut harness =
        TestHarness::from_source(SOURCE).unwrap_or_else(|error| panic!("compiles: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(harness.get_output("masked"), Some(Value::Word(0x0034)));
    assert_eq!(harness.get_output("merged"), Some(Value::Word(0xF23F)));
    assert_eq!(harness.get_output("flipped"), Some(Value::Byte(0xF0)));
    assert_eq!(harness.get_output("open"), Some(Value::Byte(0x30)));
    assert_eq!(harness.get_output("rest"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("both"), Some(Value::Bool(false)));
}
