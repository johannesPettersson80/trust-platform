use crate::common::*;

#[test]
fn test_for_loop_bounds_integer() {
    check_has_error(
        r#"
PROGRAM Test
    VAR i : INT; x : REAL; END_VAR
    FOR i := x TO 10 DO
    END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_case_selector_requires_elementary() {
    check_has_error(
        r#"
TYPE S : STRUCT
    x : INT;
END_STRUCT
END_TYPE

PROGRAM Test
    VAR s : S; END_VAR
    CASE s OF
        1: s.x := 1;
    END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_struct_field_in_callee_position_is_not_callable() {
    check_has_error(
        r#"
TYPE S : STRUCT
    x : INT;
END_STRUCT
END_TYPE

PROGRAM Test
    VAR s : S; y : INT; END_VAR
    y := s.x();
END_PROGRAM
"#,
        DiagnosticCode::UndefinedFunction,
    );
}

#[test]
fn test_case_label_requires_literal_or_constant() {
    check_has_error(
        r#"
PROGRAM Test
    VAR x : INT; y : INT; END_VAR
    CASE x OF
        y: x := 1;
    END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_case_subrange_requires_literal_bounds() {
    check_has_error(
        r#"
PROGRAM Test
    VAR x : INT; y : INT; END_VAR
    CASE x OF
        y..5: x := 1;
    END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_case_enum_label_ok() {
    check_no_errors(
        r#"
TYPE Color : (Red, Green, Blue)
END_TYPE

PROGRAM Test
    VAR c : Color; END_VAR
    CASE c OF
        Red: c := Green;
        Blue: c := Red;
        ELSE
            c := Blue;
    END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn test_case_string_label_ok() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR s : STRING := 'ABC'; END_VAR
    CASE s OF
        'ABC': s := 'DEF';
        'DEF': s := 'ABC';
        ELSE
            s := 'XYZ';
    END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn test_case_wstring_label_ok() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR s : WSTRING := "ABC"; END_VAR
    CASE s OF
        "ABC": s := "DEF";
        "DEF": s := "ABC";
        ELSE
            s := "XYZ";
    END_CASE;
END_PROGRAM
"#,
    );
}

#[test]
fn test_case_string_subrange_rejected() {
    check_has_error(
        r#"
PROGRAM Test
    VAR s : STRING := 'B'; END_VAR
    CASE s OF
        'A'..'Z': s := 'OK';
    END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_string_comparison_operators_ok() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR
        s1 : STRING := 'ABC';
        s2 : STRING := 'ABD';
        w1 : WSTRING := "ABC";
        w2 : WSTRING := "ABD";
        b : BOOL;
    END_VAR
    b := (s1 = 'ABC') AND (s1 < s2) AND (w1 = "ABC") AND (w1 < w2);
END_PROGRAM
"#,
    );
}

#[test]
fn test_fixed_length_string_comparison_operators_ok() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR
        s1 : STRING[1] := ' ';
        s2 : STRING[1] := 'A';
        w1 : WSTRING[1] := " ";
        w2 : WSTRING[1] := "A";
        b : BOOL;
    END_VAR
    b := (s1 = ' ') AND (s1 < s2) AND (w1 = " ") AND (w1 < w2);
END_PROGRAM
"#,
    );
}

#[test]
fn test_case_missing_else_warning() {
    let warnings = check_warnings(
        r#"
PROGRAM Test
    VAR x : INT; END_VAR
    CASE x OF
        1: x := 1;
    END_CASE;
END_PROGRAM
"#,
    );
    assert!(
        warnings.contains(&DiagnosticCode::MissingElse),
        "Expected MissingElse warning, got: {:?}",
        warnings
    );
}

#[test]
fn test_case_enum_exhaustive_no_warning() {
    let warnings = check_warnings(
        r#"
TYPE Mode : (Off, Manual, Auto)
END_TYPE

PROGRAM Test
    VAR m : Mode; END_VAR
    CASE m OF
        Mode#Off: m := Mode#Manual;
        Mode#Manual: m := Mode#Auto;
        Mode#Auto: m := Mode#Off;
    END_CASE;
END_PROGRAM
"#,
    );
    assert!(
        !warnings.contains(&DiagnosticCode::MissingElse),
        "Expected no MissingElse warning, got: {:?}",
        warnings
    );
}

#[test]
fn test_named_argument_order() {
    check_has_error(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
        b : DINT;
    END_VAR
    Add := a + b;
END_FUNCTION

PROGRAM Test
    VAR result : DINT; END_VAR
    result := Add(a := 1, 2);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_named_argument_order_allows_positional_first() {
    check_no_errors(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
        b : DINT;
    END_VAR
    Add := a + b;
END_FUNCTION

PROGRAM Test
    VAR result : DINT; END_VAR
    result := Add(1, b := 2);
END_PROGRAM
"#,
    );
}

#[test]
fn test_output_parameter_connection_ok() {
    check_no_errors(
        r#"
FUNCTION WithOut : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
    VAR_OUTPUT
        out1 : DINT;
    END_VAR
    out1 := a + 1;
    WithOut := out1;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; out1 : DINT; END_VAR
    WithOut(a := a, out1 => out1);
END_PROGRAM
"#,
    );
}

#[test]
fn test_output_parameter_requires_arrow() {
    check_has_error(
        r#"
FUNCTION WithOut : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
    VAR_OUTPUT
        out1 : DINT;
    END_VAR
    out1 := a + 1;
    WithOut := out1;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; out1 : DINT; END_VAR
    WithOut(a := a, out1 := out1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_output_connection_rejects_input_parameter() {
    check_has_error(
        r#"
FUNCTION WithOut : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
    VAR_OUTPUT
        out1 : DINT;
    END_VAR
    out1 := a + 1;
    WithOut := out1;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; out1 : DINT; END_VAR
    WithOut(a => out1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_formal_call_allows_missing_arguments() {
    check_no_errors(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
        b : DINT;
    END_VAR
    Add := a + b;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; res : DINT; END_VAR
    res := Add(a := a);
END_PROGRAM
"#,
    );
}

#[test]
fn test_formal_call_requires_in_out_binding() {
    check_has_error(
        r#"
FUNCTION UseInOut : DINT
    VAR_IN_OUT
        x : DINT;
    END_VAR
    UseInOut := x;
END_FUNCTION

PROGRAM Test
    VAR res : DINT; END_VAR
    res := UseInOut();
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_formal_call_duplicate_parameter_error() {
    check_has_error(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
        b : DINT;
    END_VAR
    Add := a + b;
END_FUNCTION

PROGRAM Test
    VAR res : DINT; END_VAR
    res := Add(a := 1, a := 2);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_formal_call_unknown_parameter_error() {
    check_has_error(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
        b : DINT;
    END_VAR
    Add := a + b;
END_FUNCTION

PROGRAM Test
    VAR res : DINT; END_VAR
    res := Add(c := 1, a := 2);
END_PROGRAM
"#,
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn test_call_argument_unknown_type_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
FUNCTION UseBool : BOOL
    VAR_INPUT
        b : BOOL;
    END_VAR
    UseBool := b;
END_FUNCTION

PROGRAM Test
    VAR result : BOOL; END_VAR
    result := UseBool(missingValue);
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected UndefinedVariable primary diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::InvalidArgumentType),
        "UNKNOWN cascade should not emit wrong-reason InvalidArgumentType, got {errors:?}"
    );
}

#[test]
fn test_non_formal_call_requires_complete_arguments() {
    check_has_error(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
        b : DINT;
    END_VAR
    Add := a + b;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; res : DINT; END_VAR
    res := Add(a);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn test_non_formal_call_skips_en_eno() {
    check_no_errors(
        r#"
FUNCTION WithEn : DINT
    VAR_INPUT
        EN : BOOL;
        a : DINT;
        b : DINT;
    END_VAR
    VAR_OUTPUT
        ENO : BOOL;
    END_VAR
    WithEn := a + b;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; b : DINT; res : DINT; END_VAR
    res := WithEn(a, b);
END_PROGRAM
"#,
    );
}

#[test]
fn test_non_formal_call_rejects_en_eno_positional() {
    check_has_error(
        r#"
FUNCTION WithEn : DINT
    VAR_INPUT
        EN : BOOL;
        a : DINT;
        b : DINT;
    END_VAR
    VAR_OUTPUT
        ENO : BOOL;
    END_VAR
    WithEn := a + b;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; b : DINT; res : DINT; END_VAR
    res := WithEn(TRUE, a, b);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn test_standard_numeric_unknown_argument_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
PROGRAM Test
VAR
    x : DINT;
END_VAR
x := ADD(Missing, 1);
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected primary UndefinedVariable diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::InvalidArgumentType)
            && !errors.contains(&DiagnosticCode::IncompatibleAssignment),
        "unknown standard numeric argument must not emit wrong-reason argument or assignment cascades, got {errors:?}"
    );
}

#[test]
fn test_standard_unary_unknown_argument_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
PROGRAM Test
VAR
    x : DINT;
END_VAR
x := ABS(Missing);
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected primary UndefinedVariable diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::InvalidArgumentType)
            && !errors.contains(&DiagnosticCode::IncompatibleAssignment),
        "unknown standard unary argument must not emit wrong-reason argument or assignment cascades, got {errors:?}"
    );
}

#[test]
fn test_standard_bit_unknown_argument_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
PROGRAM Test
VAR
    x : BYTE;
END_VAR
x := SHL(Missing, 1);
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected primary UndefinedVariable diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::InvalidArgumentType)
            && !errors.contains(&DiagnosticCode::IncompatibleAssignment),
        "unknown standard bit argument must not emit wrong-reason argument or assignment cascades, got {errors:?}"
    );
}

#[test]
fn test_standard_string_unknown_argument_suppression_has_primary_diagnostic() {
    let errors = check_errors(
        r#"
PROGRAM Test
VAR
    x : DINT;
END_VAR
x := LEN(Missing);
END_PROGRAM
"#,
    );

    assert!(
        errors.contains(&DiagnosticCode::UndefinedVariable),
        "expected primary UndefinedVariable diagnostic, got {errors:?}"
    );
    assert!(
        !errors.contains(&DiagnosticCode::InvalidArgumentType)
            && !errors.contains(&DiagnosticCode::IncompatibleAssignment),
        "unknown standard string argument must not emit wrong-reason argument or assignment cascades, got {errors:?}"
    );
}

#[test]
fn test_non_formal_call_allows_output_positional() {
    check_no_errors(
        r#"
FUNCTION WithOut : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
    VAR_OUTPUT
        out1 : DINT;
    END_VAR
    out1 := a;
    WithOut := out1;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; out1 : DINT; END_VAR
    WithOut(a, out1);
END_PROGRAM
"#,
    );
}

#[test]
fn test_call_rejects_ref_assign_argument() {
    check_has_error(
        r#"
FUNCTION Add : DINT
    VAR_INPUT
        a : DINT;
    END_VAR
    Add := a;
END_FUNCTION

PROGRAM Test
    VAR a : DINT; b : DINT; END_VAR
    Add(a ?= b);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_function_block_instance_call() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Counter
    VAR_INPUT
        Enable : BOOL;
    END_VAR
END_FUNCTION_BLOCK

PROGRAM Test
    VAR fb : Counter; END_VAR
    fb(Enable := TRUE);
END_PROGRAM
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 46 + Figure 15 (timer function blocks)
fn test_standard_timer_function_block_call() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR timer : TON; done : BOOL; elapsed : TIME; END_VAR
    timer(IN := TRUE, PT := T#1s);
    timer(IN := FALSE);
    done := timer.Q;
    elapsed := timer.ET;
END_PROGRAM
"#,
    );
}

#[test]
fn test_standard_timer_function_block_ltime_overload() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR timer : TON; elapsed : LTIME; q : BOOL; END_VAR
    timer(IN := TRUE, PT := LTIME#1s, Q => q, ET => elapsed);
END_PROGRAM
"#,
    );
}

#[test]
fn test_standard_timer_function_block_ltime_variant() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR timer : TON_LTIME; elapsed : LTIME; q : BOOL; END_VAR
    timer(IN := TRUE, PT := LTIME#1s, Q => q, ET => elapsed);
END_PROGRAM
"#,
    );
}

#[test]
fn test_standard_timer_function_block_ltime_type_error() {
    check_has_error(
        r#"
PROGRAM Test
    VAR timer : TON_LTIME; elapsed : LTIME; q : BOOL; END_VAR
    timer(IN := TRUE, PT := T#1s, Q => q, ET => elapsed);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 43 (bistable function blocks)
fn test_standard_bistable_function_blocks() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR rs : RS; sr : SR; q1 : BOOL; q2 : BOOL; END_VAR
    rs(S := TRUE, R1 := FALSE, Q1 => q1);
    rs(S := TRUE, RESET1 := FALSE, Q1 => q1);
    sr(S1 := TRUE, R := FALSE, Q1 => q2);
    sr(SET1 := TRUE, RESET := FALSE, Q1 => q2);
END_PROGRAM
"#,
    );
}

#[test]
fn test_standard_bistable_function_block_type_error() {
    check_has_error(
        r#"
PROGRAM Test
    VAR rs : RS; q1 : BOOL; END_VAR
    rs(S := 1, R1 := FALSE, Q1 => q1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 44 (edge detection function blocks)
fn test_standard_edge_function_blocks() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR rtrig : R_TRIG; ftrig : F_TRIG; q1 : BOOL; q2 : BOOL; END_VAR
    rtrig(CLK := TRUE, Q => q1);
    ftrig(CLK := FALSE, Q => q2);
END_PROGRAM
"#,
    );
}

#[test]
fn test_mitsubishi_edge_alias_function_blocks() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR difu : DIFU; difd : DIFD; q_up : BOOL; q_down : BOOL; END_VAR
    difu(CLK := TRUE, Q => q_up);
    difd(CLK := FALSE, Q => q_down);
END_PROGRAM
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Table 45 (counter function blocks)
fn test_standard_counter_function_blocks() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR
        ctu : CTU;
        ctd : CTD;
        ctud : CTUD;
        ctu_int : CTU_INT;
        pv_dint : DINT;
        cv_dint : DINT;
        pv_int : INT;
        cv_int : INT;
        qu : BOOL;
        qd : BOOL;
        q : BOOL;
    END_VAR
    ctu(CU := TRUE, R := FALSE, PV := pv_dint, Q => q, CV => cv_dint);
    ctd(CD := TRUE, LD := FALSE, PV := pv_dint, Q => q, CV => cv_dint);
    ctud(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := pv_dint, QU => qu, QD => qd, CV => cv_dint);
    ctu_int(CU := TRUE, R := FALSE, PV := pv_int, Q => q, CV => cv_int);
END_PROGRAM
"#,
    );
}

#[test]
fn test_standard_counter_function_block_type_error() {
    check_has_error(
        r#"
PROGRAM Test
    VAR ctu : CTU; q : BOOL; cv : INT; END_VAR
    ctu(CU := TRUE, R := FALSE, PV := 1.0, Q => q, CV => cv);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_typed_literal_prefix() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : INT; END_VAR
    x := INT#1;
END_PROGRAM
"#,
    );
}

#[test]
fn test_binary_operator_precedence() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR x : BOOL; END_VAR
    IF 1 * 2 < 3 THEN
        x := TRUE;
    END_IF;
END_PROGRAM
"#,
    );
}

#[test]
fn test_infix_bitwise_any_bit_expressions_are_allowed() {
    check_no_errors(
        r#"
PROGRAM Test
    VAR
        a : WORD := WORD#16#FF00;
        b : WORD := WORD#16#0F0F;
        r : WORD;
    END_VAR
    r := a AND b;
    r := a OR b;
    r := a XOR b;
    r := NOT a;
END_PROGRAM
"#,
    );
}

#[test]
fn test_infix_ampersand_matches_and_for_bit_strings() {
    let errors = check_errors(
        r#"
PROGRAM Test
    VAR
        w : WORD := WORD#16#FF00;
        dw : DWORD := DWORD#16#0000_F0F0;
    END_VAR
    w := w & dw;
END_PROGRAM
"#,
    );
    assert_eq!(
        errors,
        vec![DiagnosticCode::IncompatibleAssignment],
        "Expected '&' to widen like AND and only fail on the narrowing assignment, got: {:?}",
        errors
    );
}

#[test]
fn test_infix_bitwise_mixed_width_results_cannot_shrink_silently() {
    let errors = check_errors(
        r#"
PROGRAM Test
    VAR
        w : WORD := WORD#16#FF00;
        dw : DWORD := DWORD#16#0000_F0F0;
    END_VAR
    w := w AND dw;
END_PROGRAM
"#,
    );
    assert_eq!(
        errors,
        vec![DiagnosticCode::IncompatibleAssignment],
        "Expected only the narrowing assignment error, got: {:?}",
        errors
    );
}
