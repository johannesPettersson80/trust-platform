mod common;
use common::*;

// Standard Function Tests
#[test]
// IEC 61131-3 Ed.3 Tables 22-27 (standard conversion functions)
fn test_standard_conversion_functions() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    i: INT;
    di: DINT;
    r: REAL;
    lr: LREAL;
    b: BYTE;
    w: WORD;
    dw: DWORD;
    lw: LWORD;
    t: TIME;
    lt: LTIME;
    d: DATE;
    ld: LDATE;
    dt_val: DT;
    ldt_val: LDT;
    tod_val: TOD;
    ltod_val: LTOD;
    s: STRING;
    ws: WSTRING;
    c: CHAR;
    wc: WCHAR;
    u: UINT;
END_VAR
di := INT_TO_DINT(i);
i := DINT_TO_INT(di);
r := DINT_TO_REAL(di);
di := REAL_TO_DINT(r);
t := LTIME_TO_TIME(LT#1s);
lt := TIME_TO_LTIME(T#1s);
dt_val := CONCAT_DATE_TOD(DATE#2024-01-01, TOD#00:00:00);
d := DT_TO_DATE(dt_val);
ld := LDATE#2024-01-01;
tod_val := DT_TO_TOD(dt_val);
ldt_val := DT_TO_LDT(dt_val);
ltod_val := LDT_TO_LTOD(LDT#2024-01-01-00:00:00);
s := WSTRING_TO_STRING(WSTRING#"A");
ws := STRING_TO_WSTRING('A');
c := STRING_TO_CHAR('A');
wc := CHAR_TO_WCHAR(c);
s := CHAR_TO_STRING(c);
ws := WCHAR_TO_WSTRING(wc);
u := BCD_TO_UINT(WORD#16#0042);
w := TO_BCD_WORD(u);
dw := REAL_TO_DWORD(REAL#1.0);
lr := LWORD_TO_LREAL(LWORD#16#0000_0000_0000_0001);
END_PROGRAM
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Tables 28-33 (numeric/bitwise functions)
fn test_standard_numeric_and_bitwise_functions() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    a: DINT;
    b: DINT;
    r: REAL;
    lr: LREAL;
    w: WORD;
    w2: WORD;
    ok: BOOL;
    sel_out: DINT;
END_VAR
sel_out := SEL(TRUE, a, b);
r := DIV(REAL#4.0, REAL#2.0);
lr := ADD(REAL#1.0, LREAL#2.0);
w2 := SHL(w, 2);
w2 := AND(w, 16#00FF);
ok := GT(5, 3, 1);
ok := NE(a, b);
END_PROGRAM
"#,
    );
}

#[test]
// IEC 61131-3 Ed.3 Tables 34-36 (string/time functions)
fn test_standard_string_and_time_functions() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    s: STRING[20];
    t: TIME;
    dt_val: DT;
    tod_val: TOD;
    year: UINT;
    month: USINT;
    day: USINT;
    dow: INT;
    idx: INT;
END_VAR
s := CONCAT('A', 'B', 'C');
s := LEFT(s, 2);
s := RIGHT(s, 2);
s := MID(s, 2, 2);
s := INSERT('AC', 'B', 2);
s := DELETE('ABCD', 2, 2);
s := REPLACE('ABCD', 'XX', 2, 2);
idx := FIND('ABCABC', 'BC');
t := ADD_TIME(T#1s, T#2s);
tod_val := ADD_TOD_TIME(TOD#00:00:00, T#1s);
dt_val := ADD_DT_TIME(DT#2024-01-01-00:00:00, T#1s);
SPLIT_DATE(DATE#2024-01-01, year, month, day);
dow := DAY_OF_WEEK(DATE#2024-01-01);
END_PROGRAM
"#,
    );
}

#[test]
fn test_standard_function_wrong_arity() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    x: DINT;
END_VAR
x := ADD(1);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn test_standard_function_type_mismatch() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    x: DINT;
END_VAR
x := ADD(TRUE, 1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_using_directive_resolves_type() {
    check_no_errors(
        r#"
NAMESPACE Standard.Timers
FUNCTION_BLOCK TON
END_FUNCTION_BLOCK
END_NAMESPACE

PROGRAM Test
USING Standard.Timers;
VAR
    T1 : TON;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn test_using_directive_unknown_namespace_error() {
    check_has_error(
        r#"
PROGRAM Test
USING Missing.Namespace;
VAR
    T1 : INT;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn test_using_directive_nested_namespace_not_imported() {
    check_has_error(
        r#"
NAMESPACE Standard.Timers
FUNCTION_BLOCK TON
END_FUNCTION_BLOCK
END_NAMESPACE

PROGRAM Test
USING Standard;
VAR
    T1 : Timers.TON;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UndefinedType,
    );
}

#[test]
fn test_exit_outside_loop_error() {
    check_has_error(
        r#"
PROGRAM Test
    EXIT;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_continue_outside_loop_error() {
    check_has_error(
        r#"
PROGRAM Test
    CONTINUE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_for_loop_control_var_modified_error() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    i : INT;
END_VAR
FOR i := 1 TO 10 DO
    i := i + 1;
END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_for_loop_bounds_type_mismatch_error() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    i : INT;
END_VAR
FOR i := DINT#1 TO 10 DO
    i := i + 1;
END_FOR;
END_PROGRAM
"#,
        DiagnosticCode::TypeMismatch,
    );
}

#[test]
fn test_jmp_unknown_label_error() {
    check_has_error(
        r#"
PROGRAM Test
    JMP Missing;
END_PROGRAM
"#,
        DiagnosticCode::CannotResolve,
    );
}

#[test]
fn test_jmp_label_ok() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    x : DINT;
END_VAR
Start: x := 1;
JMP Start;
END_PROGRAM
"#,
    );
}

#[test]
fn test_duplicate_case_label_error() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    Mode : DINT;
END_VAR
CASE Mode OF
    1: Mode := 2;
    1: Mode := 3;
END_CASE;
END_PROGRAM
"#,
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn test_duplicate_label_declaration_error() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    x : DINT;
END_VAR
Start: x := 1;
Start: x := 2;
END_PROGRAM
"#,
        DiagnosticCode::DuplicateDeclaration,
    );
}

#[test]
fn test_new_delete_calls_ok() {
    check_no_errors(
        r#"
FUNCTION_BLOCK FB
END_FUNCTION_BLOCK

PROGRAM Test
VAR
    RefVar : REF_TO FB;
END_VAR
RefVar := NEW(FB);
__DELETE(RefVar);
END_PROGRAM
"#,
    );
}

#[test]
fn test_new_requires_type_error() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    x : INT;
END_VAR
NEW(x);
END_PROGRAM
"#,
        DiagnosticCode::UndefinedType,
    );
}

#[test]
fn test_assert_standard_functions_ok() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    b : BOOL := TRUE;
    x : INT := INT#2;
    y : DINT := DINT#2;
    r : REAL := REAL#1.0;
END_VAR
ASSERT_TRUE(b);
ASSERT_FALSE(FALSE);
ASSERT_EQUAL(x, y);
ASSERT_NEAR(r, REAL#1.1, REAL#0.2);
END_PROGRAM
"#,
    );
}

#[test]
fn test_assert_true_requires_bool() {
    check_has_error(
        r#"
PROGRAM Test
ASSERT_TRUE(INT#1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_assert_equal_requires_comparable_types() {
    check_has_error(
        r#"
PROGRAM Test
ASSERT_EQUAL(TRUE, INT#1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_assert_near_requires_numeric_types() {
    check_has_error(
        r#"
PROGRAM Test
ASSERT_NEAR(TRUE, REAL#1.0, REAL#0.1);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_assert_equal_wrong_arity() {
    check_has_error(
        r#"
PROGRAM Test
ASSERT_EQUAL(INT#1);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}
