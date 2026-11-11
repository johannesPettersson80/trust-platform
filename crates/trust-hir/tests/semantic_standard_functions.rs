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
dw := TIME_TO_DWORD(t);
t := DWORD_TO_TIME(dw);
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
c := BYTE_TO_CHAR(BYTE#16#41);
b := CHAR_TO_BYTE(c);
w := WCHAR_TO_WORD(wc);
s := DINT_TO_STRING(di);
s := REAL_TO_STRING(r);
s := DWORD_TO_STRING(dw);
di := STRING_TO_DINT('42');
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
// docs/specs/07-standard-functions.md 2.9: vendor-extension conversions that IEC Tables
// 22-27 do not list (REAL/LREAL to a bit string of another width, numbers to BOOL, BOOL to
// REAL/LREAL), used by CODESYS and TwinCAT projects; and 2.2: the IEC Table 24 bit string
// to BOOL conversions
fn test_vendor_extension_conversions_are_accepted() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    i: INT;
    ud: UDINT;
    r: REAL;
    lr: LREAL;
    b: BYTE;
    w: WORD;
    dw: DWORD;
    lw: LWORD;
    x: BOOL;
END_VAR
b := REAL_TO_BYTE(r);
w := REAL_TO_WORD(r);
lw := REAL_TO_LWORD(r);
b := LREAL_TO_BYTE(lr);
w := LREAL_TO_WORD(lr);
dw := LREAL_TO_DWORD(lr);
x := INT_TO_BOOL(i);
x := UDINT_TO_BOOL(ud);
x := BYTE_TO_BOOL(b);
x := LWORD_TO_BOOL(lw);
x := REAL_TO_BOOL(r);
x := LREAL_TO_BOOL(lr);
x := TO_BOOL(i);
r := BOOL_TO_REAL(x);
lr := BOOL_TO_LREAL(x);
END_PROGRAM
"#,
    );
}

#[test]
fn test_typed_conversion_accepts_positional_outer_named_inner_call() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    dw: DWORD;
    r: REAL;
END_VAR
r := UDINT_TO_REAL(DWORD_TO_UDINT(IN := dw));
END_PROGRAM
"#,
    );
}

#[test]
fn test_converted_numeric_expression_accepts_bit_to_numeric_scaling() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    td_ms: DWORD;
    tr_ms: DWORD;
    scaled: REAL;
END_VAR
scaled := UDINT_TO_REAL(IN := DWORD_TO_UDINT(IN := td_ms)) * REAL#256.0
    / UDINT_TO_REAL(IN := DWORD_TO_UDINT(IN := tr_ms));
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
// IEC 61131-3 Ed.3 Table 39 (validate functions)
fn test_standard_validate_functions() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    r: REAL;
    lr: LREAL;
    b: BYTE;
    w: WORD;
    dw: DWORD;
    lw: LWORD;
    ok: BOOL;
END_VAR
ok := IS_VALID(r);
ok := IS_VALID(lr);
ok := IS_VALID_BCD(b);
ok := IS_VALID_BCD(w);
ok := IS_VALID_BCD(dw);
ok := IS_VALID_BCD(lw);
END_PROGRAM
"#,
    );
}

#[test]
fn test_is_valid_requires_real_argument() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    i: DINT;
    ok: BOOL;
END_VAR
ok := IS_VALID(i);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_is_valid_bcd_rejects_bool() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    flag: BOOL;
    ok: BOOL;
END_VAR
ok := IS_VALID_BCD(flag);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

#[test]
fn test_is_valid_bcd_rejects_non_bit_string() {
    check_has_error(
        r#"
PROGRAM Test
VAR
    r: REAL;
    ok: BOOL;
END_VAR
ok := IS_VALID_BCD(r);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
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
t := TIME();
dt_val := CURRENT_DT();
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
fn current_dt_returns_dt_and_rejects_arguments() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    stamp: DT;
END_VAR
stamp := CURRENT_DT();
END_PROGRAM
"#,
    );
    check_has_error(
        r#"
PROGRAM Test
VAR
    stamp: DT;
END_VAR
stamp := CURRENT_DT(1);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

#[test]
fn test_assert_equal_accepts_char_and_wchar() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    c: CHAR;
    wc: WCHAR;
END_VAR
c := STRING_TO_CHAR('B');
wc := CHAR_TO_WCHAR(STRING_TO_CHAR('Y'));
ASSERT_EQUAL(EXPECTED := STRING_TO_CHAR('B'), ACTUAL := c);
ASSERT_EQUAL(EXPECTED := CHAR_TO_WCHAR(STRING_TO_CHAR('Y')), ACTUAL := wc);
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
// IEC 61131-3 Ed.3 Table 35: `ADD` takes the time of day or the date and time first and the
// duration second (items 2a and 3a), as the `+` operator does (specification 05). Catches `ADD`
// accepting the duration first, which the build then refused with "a comparison of TIME with
// TIME_OF_DAY is not compiled yet" (checklist G32).
fn add_takes_the_duration_second_as_table_35_lists() {
    let program = |body: &str| {
        format!(
            "PROGRAM Test\nVAR\n    t : TIME;\n    lt : LTIME;\n    tod_val : TOD;\n    \
             dt_val : DT;\n    ltod_val : LTIME_OF_DAY;\n    ldt_val : LDATE_AND_TIME;\n\
             END_VAR\n{body}\nEND_PROGRAM\n"
        )
    };
    check_no_errors(&program(
        "t := ADD(t, t);\nlt := ADD(lt, lt);\ntod_val := ADD(tod_val, t);\ndt_val := ADD(dt_val, t);\n\
         ltod_val := ADD(ltod_val, lt);\nldt_val := ADD(ldt_val, lt);",
    ));
    for swapped in [
        "tod_val := ADD(t, tod_val);",
        "dt_val := ADD(t, dt_val);",
        "ltod_val := ADD(lt, ltod_val);",
        "ldt_val := ADD(lt, ldt_val);",
    ] {
        check_has_error(&program(swapped), DiagnosticCode::InvalidArgumentType);
    }
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

/// A typed literal names its type as a declaration does: through a namespace that a USING directive
/// names, and not without it (IEC §6.9.4). Catches the prefix looked up only by its global name,
/// "undefined typed literal prefix 'PIDMode'", as before 24 October 2026, and a lookup that finds
/// a namespace's type that no USING names.
#[test]
fn test_using_directive_resolves_typed_literal_prefix() {
    let namespace = "
NAMESPACE Ctl
TYPE PIDMode : (Manual, Auto); END_TYPE
END_NAMESPACE
";
    check_no_errors(&format!(
        "{namespace}
PROGRAM Test
USING Ctl;
VAR
    mode : PIDMode;
END_VAR
mode := PIDMode#Auto;
END_PROGRAM
"
    ));
    check_has_error(
        &format!(
            "{namespace}
PROGRAM Test
VAR
    mode : Ctl.PIDMode;
END_VAR
mode := PIDMode#Auto;
END_PROGRAM
"
        ),
        DiagnosticCode::UndefinedType,
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
        DiagnosticCode::InvalidArgumentType,
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
ASSERT_NOT_EQUAL(x, INT#3);
ASSERT_GREATER(INT#3, INT#2);
ASSERT_LESS(INT#2, INT#3);
ASSERT_GREATER_OR_EQUAL(INT#3, INT#3);
ASSERT_LESS_OR_EQUAL(INT#3, INT#3);
ASSERT_NEAR(r, REAL#1.1, REAL#0.2);
END_PROGRAM
"#,
    );
}

/// Specification 07 and 22: `TEST_FINISHED()` takes no inputs and returns nothing, and the compile refuses a test attribute
/// whose value does not read (E309). Catches the function unknown to the checker (every multi-cycle test would fail to compile),
/// an input accepted, and a zero cycle accepted.
#[test]
fn test_test_finished_and_the_test_attributes() {
    check_no_errors(
        r#"
{attribute 'test_cycle' := 'T#20ms'}
{attribute 'test_max_cycles' := '500'}
TEST_PROGRAM Finishes
TEST_FINISHED();
END_TEST_PROGRAM
"#,
    );
    check_has_error(
        r#"
TEST_PROGRAM Finishes
TEST_FINISHED(TRUE);
END_TEST_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
    check_has_error(
        r#"
{attribute 'test_cycle' := 'T#0ms'}
TEST_PROGRAM Finishes
TEST_FINISHED();
END_TEST_PROGRAM
"#,
        DiagnosticCode::InvalidTestAttribute,
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
fn test_assert_greater_requires_comparable_types() {
    check_has_error(
        r#"
PROGRAM Test
ASSERT_GREATER(TRUE, INT#1);
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

#[test]
fn test_assert_less_or_equal_wrong_arity() {
    check_has_error(
        r#"
PROGRAM Test
ASSERT_LESS_OR_EQUAL(INT#1);
END_PROGRAM
"#,
        DiagnosticCode::WrongArgumentCount,
    );
}

// docs/IEC_DEVIATIONS.md DEV-031: under the CODESYS profile a reference in an argument is the value
// it names, so the string functions take a REFERENCE TO STRING or WSTRING as their text and give the
// string's type (contributor issue #454). Catches the functions refusing such a reference with E205,
// "expected STRING or WSTRING input" (LEN) or "expected STRING or WSTRING type" (CONCAT).
#[test]
fn string_functions_take_a_codesys_reference_to_a_string() {
    check_no_errors_in(
        Dialect::Codesys,
        r#"
PROGRAM Main
VAR
    text : STRING(20) := 'id:42';
    wide : WSTRING(20) := "wide";
    p : REFERENCE TO STRING(20);
    pw : REFERENCE TO WSTRING(20);
    length : INT;
    wideLength : INT;
    head : STRING(20);
    joined : STRING(40);
    position : INT;
    middle : STRING(20);
END_VAR
p REF= text;
pw REF= wide;
length := LEN(p);
wideLength := LEN(pw);
head := LEFT(p, 2);
joined := CONCAT(p, '!');
position := FIND(p, ':');
middle := MID(p, 2, 4);
END_PROGRAM
"#,
    );
}

// The IEC rules dereference only with `^` (IEC 61131-3 Ed.3 §6.4.4.10.3), so LEN of a REF_TO STRING
// stays E205. Catches DEV-031's reading leaking into a project under the IEC rules.
#[test]
fn the_iec_rules_refuse_a_reference_as_a_string_function_argument() {
    check_has_error_in(
        Dialect::Iec,
        r#"
PROGRAM Main
VAR
    text : STRING(20);
    r : REF_TO STRING(20);
    length : INT;
END_VAR
r := REF(text);
length := LEN(r);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}
