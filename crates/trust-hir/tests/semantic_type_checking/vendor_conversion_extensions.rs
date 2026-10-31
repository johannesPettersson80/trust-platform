use crate::common::*;

#[test]
fn to_string_accepts_base_less_enum() {
    // CODESYS TO_STRING parity (IEC_DEVIATIONS 2026-10-28): an enumerated
    // value converts to its member name string; base-less project enums
    // included, since the runtime enum value carries its variant name.
    check_no_errors(
        r#"
TYPE eArea : (areaA, areaB); END_TYPE

PROGRAM Test
VAR
    area : eArea;
    text : STRING;
END_VAR
text := TO_STRING(area);
END_PROGRAM
"#,
    );
}

#[test]
fn to_string_accepts_bool_time_and_string_operands() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    flag : BOOL;
    elapsed : TIME;
    long : LTIME;
    text : STRING[80];
    out : STRING;
END_VAR
out := TO_STRING(flag);
out := TO_STRING(elapsed);
out := TO_STRING(long);
out := TO_STRING(text);
END_PROGRAM
"#,
    );
}

#[test]
fn vendor_time_conversions_accept_integer_and_dword_targets() {
    check_no_errors(
        r#"
PROGRAM Test
VAR
    elapsed : TIME;
    long : LTIME;
    day : TIME_OF_DAY;
    evening : LTIME_OF_DAY;
    i : DINT;
    u : UDINT;
    li : LINT;
    w : DWORD;
END_VAR
i := TIME_TO_DINT(elapsed);
u := TIME_TO_UDINT(elapsed);
li := LTIME_TO_LINT(long);
w := TOD_TO_DWORD(day);
w := LTOD_TO_DWORD(evening);
END_PROGRAM
"#,
    );
}

#[test]
fn to_string_keeps_time_of_day_strict() {
    // Scope boundary: TO_STRING over time-of-day stays rejected until its
    // CODESYS clock format is specified (IEC_DEVIATIONS 2026-10-28 records the
    // extension set; TOD/date string forms are deliberately not in it).
    check_has_error(
        r#"
PROGRAM Test
VAR
    day : TIME_OF_DAY;
    out : STRING;
END_VAR
out := TO_STRING(day);
END_PROGRAM
"#,
        DiagnosticCode::InvalidArgumentType,
    );
}

