//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;

#[test]
fn function_call_expr() {
    let source = r#"
        FUNCTION Add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        Add := a + b;
        END_FUNCTION

        PROGRAM Test
        VAR
            res : INT := 0;
        END_VAR
        res := Add(INT#2, INT#3);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    harness.assert_eq("res", 5i16);
}

#[test]
fn function_call_named_args() {
    let source = r#"
        FUNCTION Add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        Add := a + b;
        END_FUNCTION

        PROGRAM Test
        VAR
            res : INT := 0;
        END_VAR
        res := Add(b := INT#2, a := INT#3);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    harness.assert_eq("res", 5i16);
}

#[test]
fn function_call_output_positional() {
    let source = r#"
        FUNCTION WithOut : INT
        VAR_INPUT
            a : INT;
        END_VAR
        VAR_OUTPUT
            out1 : INT;
        END_VAR
        out1 := a;
        WithOut := out1;
        END_FUNCTION

        PROGRAM Test
        VAR
            a : INT := INT#4;
            out1 : INT := 0;
        END_VAR
        WithOut(a, out1);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    harness.assert_eq("out1", 4i16);
}

#[test]
fn function_block_call() {
    let source = r#"
        FUNCTION_BLOCK Counter
        VAR_INPUT
            inc : BOOL;
        END_VAR
        VAR_OUTPUT
            value : INT;
        END_VAR
        VAR
            count : INT := INT#0;
        END_VAR
        IF inc THEN
            count := count + INT#1;
        END_IF;
        value := count;
        END_FUNCTION_BLOCK

        PROGRAM Test
        VAR
            fb : Counter;
            out : INT := 0;
        END_VAR
        fb(inc := TRUE, value => out);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    harness.assert_eq("out", 1i16);
}

#[test]
fn stdlib_named_args() {
    let source = r#"
        PROGRAM Test
        VAR
            out : INT := 0;
        END_VAR
        out := SEL(G := TRUE, IN0 := INT#4, IN1 := INT#7);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    harness.assert_eq("out", 7i16);
}

#[test]
fn function_in_out_with_conversion_expression_regression_issue_13() {
    let source = r#"
        FUNCTION incer2 : VOID
        VAR_IN_OUT
            i : USINT;
        END_VAR
        VAR_INPUT
            inc : USINT;
            dec : UINT;
        END_VAR
        i := i + inc - uint_to_usint(dec);
        END_FUNCTION

        PROGRAM Main
        VAR
            i : USINT := 100;
            j : UINT := 100;
            countDown : BOOL := FALSE;
        END_VAR

        IF i > 200 THEN
            countDown := TRUE;
        ELSIF i < 10 THEN
            countDown := FALSE;
        END_IF

        j := usint_to_uint(i);
        i := uint_to_usint(j);

        IF countDown THEN
            incer2(i, 0, 4);
        ELSE
            incer2(i, 4, 0);
        END_IF
        END_PROGRAM
    "#;

    // A USINT in-out takes the caller's variable (IEC §6.6.1.2.2): i = 100, countDown FALSE, so
    // the call adds 4 and subtracts 0, leaving 104 after one cycle. A copy that is not written back
    // would leave 100; an increment in the wrong type would fault.
    let mut harness = TestHarness::from_source(source).expect("the in-out program compiles");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(
        harness.get_output("i"),
        Some(trust_runtime::value::Value::USInt(104))
    );
}

#[test]
fn function_in_out_without_conversion_expression_baseline() {
    let source = r#"
        FUNCTION incer : VOID
        VAR_IN_OUT
            i : USINT;
        END_VAR
        VAR_INPUT
            inc : USINT;
            dec : USINT;
        END_VAR
        i := i + inc - dec;
        END_FUNCTION

        PROGRAM Main
        VAR
            i : USINT := 100;
            countDown : BOOL := FALSE;
        END_VAR

        IF countDown THEN
            incer(i, 0, 4);
        ELSE
            incer(i, 4, 0);
        END_IF
        END_PROGRAM
    "#;

    // A USINT in-out takes the caller's variable (IEC §6.6.1.2.2): i = 100, countDown FALSE, so
    // the call adds 4 and subtracts 0, leaving 104 after one cycle. A copy that is not written back
    // would leave 100; an increment in the wrong type would fault.
    let mut harness = TestHarness::from_source(source).expect("the in-out program compiles");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(
        harness.get_output("i"),
        Some(trust_runtime::value::Value::USInt(104))
    );
}
