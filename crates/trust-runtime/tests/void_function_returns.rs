//! VOID function returns: a `FUNCTION ... : VOID` declares no result. IEC 61131-3 has no
//! `VOID` return type, so the declaration belongs to the CODESYS profile
//! (docs/IEC_DEVIATIONS.md DEV-043): the function carries no return slot, the body may exit
//! early with a bare `RETURN;`, and callers invoke the function as a statement, capturing no
//! result. The IEC dialect rejects the declaration, and a `RETURN` with a value stays
//! rejected under both.

use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;

/// The MemCopy-shaped vendor idiom: a guard `RETURN;` before the body tail. A call that
/// takes the early exit must leave the in-out untouched; a call that falls through writes
/// it (IEC §6.6.1.2.2: an in-out takes the caller's variable).
#[test]
fn void_function_early_return_skips_the_body_tail() {
    let source = r#"
        FUNCTION CopyIfFresh : VOID
        VAR_IN_OUT
            dst : USINT;
        END_VAR
        VAR_INPUT
            src : USINT;
            fresh : BOOL;
        END_VAR
        IF NOT fresh THEN
            RETURN;
        END_IF
        dst := src;
        END_FUNCTION

        PROGRAM Main
        VAR
            skipped : USINT := 7;
            copied : USINT := 0;
        END_VAR
        CopyIfFresh(skipped, 42, FALSE);
        CopyIfFresh(copied, 42, TRUE);
        END_PROGRAM
    "#;

    let session =
        CompileSession::from_source(source).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(harness.get_output("skipped"), Some(Value::USInt(7)));
    assert_eq!(harness.get_output("copied"), Some(Value::USInt(42)));
}

/// A `RETURN` with a value inside a `: VOID` function stays a type error: the departure
/// admits the absence of a result, not a result where none is declared.
#[test]
fn void_function_return_with_a_value_is_rejected() {
    let source = r#"
        FUNCTION Pick : VOID
        VAR_INPUT
            x : USINT;
        END_VAR
        RETURN x;
        END_FUNCTION

        PROGRAM Main
        END_PROGRAM
    "#;

    let session =
        CompileSession::from_source(source).with_conversion_profile(ConversionProfile::Codesys);
    let Err(error) = TestHarness::from_session(session) else {
        panic!("a VOID function returning a value is rejected");
    };
    let text = error.to_string();
    assert!(text.contains("unexpected return value"), "{text}");
}

/// The IEC dialect keeps the IEC rule: a function declares the data type of its result
/// (IEC 61131-3 Ed.3 §6.6.1.2) and no `VOID` data type exists (§6.3). Catches the CODESYS
/// departure applied to a project under the IEC rules.
#[test]
fn void_function_stays_rejected_in_the_iec_dialect() {
    let source = r#"
        FUNCTION Copy : VOID
        VAR_IN_OUT
            dst : USINT;
        END_VAR
        dst := 1;
        END_FUNCTION

        PROGRAM Main
        END_PROGRAM
    "#;

    let Err(error) = TestHarness::from_source(source) else {
        panic!("the IEC dialect rejects a VOID function return");
    };
    let text = error.to_string();
    assert!(
        text.contains("the VOID return type is a CODESYS-profile extension"),
        "{text}"
    );
}
