use crate::common::*;

#[test]
fn to_string_accepts_base_less_enum() {
    // CODESYS TO_STRING parity (IEC_DEVIATIONS 2026-10-28): an enumerated
    // value converts to its member name string; base-less project enums
    // included, since the runtime enum value carries its variant name.
    check_no_errors_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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
    check_no_errors_in(
        Dialect::Codesys,
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

#[test]
fn vendor_conversions_stay_rejected_in_the_iec_dialect() {
    // IEC_DEVIATIONS 2026-10-28 (DEV-036): the vendor conversions are a CODESYS
    // departure, so the IEC dialect keeps the conversion tables of IEC 61131-3
    // Ed.3 (Tables 22 to 27), which list none of them. Catches the extension
    // applying without the CODESYS profile.
    let declarations = r#"
TYPE eArea : (areaA, areaB); END_TYPE

PROGRAM Test
VAR
    area : eArea;
    flag : BOOL;
    elapsed : TIME;
    day : TIME_OF_DAY;
    i : DINT;
    w : DWORD;
    text : STRING[80];
    out : STRING;
END_VAR
"#;
    for statement in [
        "out := TO_STRING(area);",
        "out := TO_STRING(flag);",
        "out := TO_STRING(elapsed);",
        "i := TIME_TO_DINT(elapsed);",
        "w := TOD_TO_DWORD(day);",
    ] {
        let source = format!("{declarations}{statement}\nEND_PROGRAM\n");
        let errors = check_errors_in(Dialect::Iec, &source);
        assert!(
            errors.contains(&DiagnosticCode::InvalidArgumentType),
            "{statement} is rejected in the IEC dialect: {errors:?}"
        );
    }
    // A STRING[80] value is a STRING: converting it is the identity in every
    // dialect.
    check_no_errors(&format!(
        "{declarations}out := TO_STRING(text);\nEND_PROGRAM\n"
    ));
}
