//! POINTER TO in a running program: ADR, dereference, indexed dereference, NULL and `?=`.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{ArrayValue, Value};

#[test]
fn pointer_types_support_adr_deref_index_and_null_in_source_free_and_contextual_legacy() {
    let source = r#"
        PROGRAM Main
        VAR
            x : INT := INT#5;
            arr : ARRAY[1..3] OF INT;
            p_int : POINTER TO INT;
            p_arr : POINTER TO ARRAY[1..3] OF INT;
            out_x : INT := INT#0;
            out_arr : INT := INT#0;
            was_null_before : BOOL := FALSE;
            was_null_after : BOOL := FALSE;
        END_VAR
        arr[1] := INT#11;
        arr[2] := INT#22;
        arr[3] := INT#33;
        IF p_int = NULL THEN
            was_null_before := TRUE;
        END_IF;
        p_int := ADR(x);
        p_arr := ADR(arr);
        p_int^ := INT#9;
        p_arr^[2] := INT#44;
        out_x := p_int^;
        out_arr := p_arr^[2];
        p_arr := NULL;
        p_int ?= NULL;
        IF p_int = NULL THEN
            was_null_after := TRUE;
        END_IF;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("x", 9i16);
    harness.assert_eq(
        "arr",
        Value::Array(Box::new(
            ArrayValue::from_untyped_parts(
                vec![11i16.into(), 44i16.into(), 33i16.into()],
                vec![(1, 3)],
            )
            .expect("valid expected array"),
        )),
    );
    harness.assert_eq("out_x", 9i16);
    harness.assert_eq("out_arr", 44i16);
    harness.assert_eq("was_null_before", true);
    harness.assert_eq("was_null_after", true);
}

#[test]
fn pointer_to_string_supports_indexed_deref_read_and_write_in_source_free_and_contextual_legacy() {
    let source = r#"
        TYPE Text5 : STRING[5];
        END_TYPE

        PROGRAM Main
        VAR
            text : Text5 := 'ABCD';
            p_text : POINTER TO Text5;
            first_char : CHAR;
            second_char : CHAR;
            out_text : Text5 := '';
        END_VAR

        p_text := ADR(text);
        first_char := p_text^[1];
        p_text^[2] := BYTE_TO_CHAR(BYTE#90);
        second_char := p_text^[2];
        out_text := text;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("first_char", Value::Char(b'A'));
    harness.assert_eq("second_char", Value::Char(b'Z'));
    harness.assert_eq("out_text", Value::String("AZCD".into()));
}
