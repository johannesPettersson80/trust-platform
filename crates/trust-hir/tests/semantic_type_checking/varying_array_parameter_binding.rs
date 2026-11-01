use crate::common::*;

// A fixed-size array bound to an `ARRAY[*]` parameter is a view binding: the parameter
// aliases the argument's storage for the call and no element conversion takes place
// (IEC 61131-3 varying arrays; CODESYS admits it silently in every parameter direction).
// Compatibility owns bound agreement (the wildcard arms there); the fidelity warning
// must stay quiet for the binding pair.

#[test]
fn fixed_array_into_varying_array_parameter_is_no_implicit_conversion() {
    let warnings = check_warnings(
        r#"
FUNCTION_BLOCK Sink
    VAR_IN_OUT
        Data : ARRAY[*] OF BYTE;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM Test
    VAR
        Buf : ARRAY[0..254] OF BYTE;
        Out : Sink;
    END_VAR
    Out(Data := Buf);
END_PROGRAM
"#,
    );
    assert!(
        !warnings.contains(&DiagnosticCode::ImplicitConversion),
        "{warnings:?}"
    );
}

#[test]
fn varying_array_output_into_fixed_array_is_no_implicit_conversion() {
    let warnings = check_warnings(
        r#"
FUNCTION_BLOCK Source
    VAR_OUTPUT
        Data : ARRAY[*] OF BYTE;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM Test
    VAR
        Buf : ARRAY[0..15] OF BYTE;
        Src : Source;
    END_VAR
    Src(Data => Buf);
END_PROGRAM
"#,
    );
    assert!(
        !warnings.contains(&DiagnosticCode::ImplicitConversion),
        "{warnings:?}"
    );
}

#[test]
fn fixed_array_into_varying_array_input_is_no_implicit_conversion() {
    let warnings = check_warnings(
        r#"
FUNCTION AcceptPayload : BOOL
    VAR_INPUT
        Data : ARRAY[*] OF BYTE;
    END_VAR
    AcceptPayload := TRUE;
END_FUNCTION

PROGRAM Test
    VAR
        Buf : ARRAY[0..2047] OF BYTE;
        ok : BOOL;
    END_VAR
    ok := AcceptPayload(Data := Buf);
END_PROGRAM
"#,
    );
    assert!(
        !warnings.contains(&DiagnosticCode::ImplicitConversion),
        "{warnings:?}"
    );
}
