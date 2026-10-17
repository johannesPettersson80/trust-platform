//! A `POINTER TO ARRAY[*] OF T` accepts the address of a concrete `ARRAY[a..b] OF T`.
//!
//! Spec 12 §6.3 gives `ARRAY[*]` the sentinel bounds `(0, i64::MAX)`, and the concrete caller
//! shape supplies the bounds (spec 10 §2.1). These tests pin the OSCAT `_ARRAY_*` idiom: a
//! dereferenced open-array pointer reads and writes the caller's array, still refuses another
//! element type, and still bounds-checks against the concrete array.

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{ArrayValue, Value};

/// The expected `ARRAY[0..3] OF REAL` value with the given elements.
fn real_array(values: [f32; 4]) -> Value {
    Value::Array(Box::new(
        ArrayValue::from_untyped_parts(values.into_iter().map(Value::Real).collect(), vec![(0, 3)])
            .expect("valid expected array"),
    ))
}

/// A four-element REAL array addressed by `ADR` into a `POINTER TO ARRAY[*] OF REAL` must be
/// readable and writable element by element, as the OSCAT `_ARRAY_*` helpers do.
#[test]
fn a_pointer_to_an_open_array_reads_and_writes_the_target_array() {
    let source = r#"
        FUNCTION AbsAll : REAL
        VAR_INPUT
            PT : POINTER TO ARRAY[*] OF REAL;
        END_VAR
        VAR
            I : INT;
            Total : REAL;
        END_VAR
        Total := REAL#0.0;
        FOR I := 0 TO 3 DO
            PT^[I] := ABS(PT^[I]);
            Total := Total + PT^[I];
        END_FOR;
        AbsAll := Total;
        END_FUNCTION

        PROGRAM Main
        VAR
            Data : ARRAY[0..3] OF REAL;
            Total : REAL;
        END_VAR
        Data[0] := REAL#-1.0;
        Data[1] := REAL#2.0;
        Data[2] := REAL#-3.0;
        Data[3] := REAL#4.0;
        Total := AbsAll(PT := ADR(Data));
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile open-array pointer program");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors, Vec::<RuntimeError>::new());
    // ABS on {-1.0, 2.0, -3.0, 4.0} is {1.0, 2.0, 3.0, 4.0}; their sum is 10.0.
    harness.assert_eq("Data", real_array([1.0, 2.0, 3.0, 4.0]));
    harness.assert_eq("Total", Value::Real(10.0));
}

/// The open-array wildcard accepts bounds, never the element type: a
/// `POINTER TO ARRAY[*] OF INT` must still refuse the address of an `ARRAY OF REAL`.
#[test]
fn a_pointer_to_an_open_array_still_refuses_another_element_type() {
    let source = r#"
        FUNCTION FirstInt : INT
        VAR_INPUT
            PT : POINTER TO ARRAY[*] OF INT;
        END_VAR
        FirstInt := PT^[0];
        END_FUNCTION

        PROGRAM Main
        VAR
            Data : ARRAY[0..3] OF REAL;
            Out : INT;
        END_VAR
        Data[0] := REAL#1.5;
        Out := FirstInt(PT := ADR(Data));
        END_PROGRAM
    "#;

    let error = TestHarness::from_source(source)
        .err()
        .expect("a POINTER TO ARRAY[*] OF INT must not accept an ARRAY OF REAL");
    assert!(
        error.to_string().contains("pointer target mismatch"),
        "expected a pointer target mismatch, got: {error}"
    );
}

/// Indexing an open-array pointer uses the concrete array's bounds: index 4 of an
/// `ARRAY[0..3]` is refused with `IndexOutOfBounds` and leaves the array unchanged.
#[test]
fn indexing_past_the_target_array_is_still_refused() {
    let source = r#"
        FUNCTION WriteAt : BOOL
        VAR_INPUT
            PT : POINTER TO ARRAY[*] OF REAL;
            I : INT;
            X : REAL;
        END_VAR
        PT^[I] := X;
        WriteAt := TRUE;
        END_FUNCTION

        PROGRAM Main
        VAR
            Data : ARRAY[0..3] OF REAL;
            Ok : BOOL;
        END_VAR
        Data[0] := REAL#1.0;
        Data[1] := REAL#2.0;
        Data[2] := REAL#3.0;
        Data[3] := REAL#4.0;
        Ok := WriteAt(PT := ADR(Data), I := INT#4, X := REAL#9.0);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).expect("compile open-array pointer program");
    let cycle = harness.cycle();
    assert_eq!(
        cycle.errors,
        vec![RuntimeError::IndexOutOfBounds {
            index: 4,
            lower: 0,
            upper: 3,
        }]
    );
    harness.assert_eq("Data", real_array([1.0, 2.0, 3.0, 4.0]));
}
