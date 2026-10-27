//! Named and cross-file global constants drive the declared lengths of strings.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn named_constants_drive_parenthesized_string_lengths() {
    let source = r#"
        VAR_GLOBAL CONSTANT
            STRING_LENGTH : INT := INT#12;
            EXTRA : INT := INT#2;
        END_VAR

        PROGRAM Main
        VAR
            s : STRING(STRING_LENGTH);
            ws : WSTRING(STRING_LENGTH + EXTRA);
            out_s : STRING(STRING_LENGTH);
            out_ws : WSTRING(STRING_LENGTH + EXTRA);
        END_VAR
        s := 'HELLO';
        ws := "WIDE";
        out_s := s;
        out_ws := ws;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_s", Value::String("HELLO".into()));
    harness.assert_eq("out_ws", Value::WString("WIDE".to_string()));
}

#[test]
fn cross_file_global_constants_drive_string_lengths() {
    let globals = r#"
        VAR_GLOBAL CONSTANT
            STRING_LENGTH : INT := INT#12;
            EXTRA : INT := INT#2;
        END_VAR
    "#;

    let program = r#"
        PROGRAM Main
        VAR
            s : STRING(STRING_LENGTH);
            ws : WSTRING(STRING_LENGTH + EXTRA);
            out_s : STRING(STRING_LENGTH);
            out_ws : WSTRING(STRING_LENGTH + EXTRA);
        END_VAR
        s := 'HELLO';
        ws := "WIDE";
        out_s := s;
        out_ws := ws;
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_sources(&[globals, program]).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "runtime execution failed: {:?}",
        cycle.errors
    );
    harness.assert_eq("out_s", Value::String("HELLO".into()));
    harness.assert_eq("out_ws", Value::WString("WIDE".to_string()));
}
