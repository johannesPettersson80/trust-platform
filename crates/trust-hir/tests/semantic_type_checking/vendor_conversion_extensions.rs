use crate::common::*;

#[test]
fn an_untyped_literal_is_the_input_type_of_a_typed_conversion() {
    // Specification 05 §5.2.1: an untyped literal in a call argument takes the parameter's type
    // when its value fits, and a typed conversion's `IN` is such an argument, under every profile.
    // Catches the conversion check that compared the literal's default type (`LREAL`, `SINT`) with
    // `IN`'s and refused `REAL_TO_INT(2.5)` with E205.
    let source = r#"
PROGRAM Test
VAR
    i : INT;
    r : REAL;
    b : BYTE;
END_VAR
i := REAL_TO_INT(2.5);
i := SINT_TO_INT(5);
r := INT_TO_REAL(300);
b := INT_TO_BYTE(IN := 255);
END_PROGRAM
"#;
    check_no_errors_in(Dialect::Codesys, source);
    check_no_errors_in(Dialect::Iec, source);
}

#[test]
fn a_literal_that_does_not_fit_the_conversion_input_stays_an_error() {
    // The literal takes the input's type only when its value fits: 300 is no SINT, and a string is
    // no REAL. Catches the literal rule accepting any literal.
    check_has_error(
        "PROGRAM Test\nVAR i : INT; END_VAR\ni := SINT_TO_INT(300);\nEND_PROGRAM\n",
        DiagnosticCode::InvalidArgumentType,
    );
    check_has_error(
        "PROGRAM Test\nVAR i : INT; END_VAR\ni := REAL_TO_INT('2.5');\nEND_PROGRAM\n",
        DiagnosticCode::InvalidArgumentType,
    );
}

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

#[test]
fn codesys_accepts_the_round_2_time_and_tod_inverse_families() {
    // IEC_DEVIATIONS 2026-11-06 (DEV-045, round 2 of DEV-036): the inverses
    // of the admitted duration conversions, as whole families. An integer is
    // a millisecond count, a REAL/LREAL value is read as milliseconds, and a
    // DWORD is a ms-of-day count. The IEC dialect keeps rejecting them (the
    // DWORD_TO_TIME pair is the older both-dialect helper, not part of this).
    check_no_errors_in(
        Dialect::Codesys,
        r#"
PROGRAM Test
VAR
    s : SINT;
    i2 : INT;
    u : UINT;
    ul : ULINT;
    r : REAL;
    lr : LREAL;
    w : DWORD;
    t : TIME;
    day : TIME_OF_DAY;
END_VAR
t := SINT_TO_TIME(s);
t := INT_TO_TIME(i2);
t := UINT_TO_TIME(u);
t := ULINT_TO_TIME(ul);
t := REAL_TO_TIME(r);
t := LREAL_TO_TIME(lr);
day := DWORD_TO_TOD(w);
END_PROGRAM
"#,
    );
}

#[test]
fn codesys_accepts_string_to_integer_and_bit_string_conversions() {
    // DEV-045: string to an integer or a bit string, typed and overloaded,
    // all with the one CODESYS parse rule (specification 07 2.7, 2.10).
    check_no_errors_in(
        Dialect::Codesys,
        r#"
PROGRAM Test
VAR
    text : STRING[80];
    i : DINT;
    u : UINT;
    b : BYTE;
    w : WORD;
    dw : DWORD;
END_VAR
i := STRING_TO_DINT(text);
u := STRING_TO_UINT(text);
b := STRING_TO_BYTE(text);
b := TO_BYTE(text);
w := TO_WORD(text);
dw := TO_DWORD(text);
END_PROGRAM
"#,
    );
}

#[test]
fn the_iec_dialect_rejects_the_round_2_families() {
    // IEC_DEVIATIONS 2026-11-06: the profile keeps the tables' reading. The
    // string-to-integer acceptance is pre-existing in both dialects (the
    // strict representative parse, specification 07 2.7), so its IEC row
    // stays an acceptance; what the IEC dialect keeps rejecting here is the
    // string-to-bit-string pair.
    let declarations = r#"
PROGRAM Test
VAR
    i : DINT;
    r : REAL;
    lr : LREAL;
    w : DWORD;
    day : TIME_OF_DAY;
    t : TIME;
    text : STRING[80];
    b : BYTE;
END_VAR
"#;
    for statement in [
        "t := INT_TO_TIME(i);",
        "t := REAL_TO_TIME(r);",
        "t := LREAL_TO_TIME(lr);",
        "day := DWORD_TO_TOD(w);",
        "b := TO_BYTE(text);",
        "b := STRING_TO_BYTE(text);",
    ] {
        let source = format!("{declarations}{statement}\nEND_PROGRAM\n");
        let errors = check_errors_in(Dialect::Iec, &source);
        assert!(
            errors.contains(&DiagnosticCode::InvalidArgumentType),
            "{statement} is rejected in the IEC dialect: {errors:?}"
        );
    }
    // Pre-existing in both dialects: the typed string-to-integer acceptance.
    check_no_errors_in(
        Dialect::Iec,
        &format!("{declarations}i := STRING_TO_DINT(text);\nEND_PROGRAM\n"),
    );
}

#[test]
fn out_of_family_pairs_keep_their_rules_under_codesys() {
    // DEV-045 mitigation: the extension set is deliberately bounded. The
    // LTIME/LTOD inverses and WSTRING stay out, so these stay errors under
    // the profile; REAL_TO_LWORD is the existing 2.9 extension (accepted in
    // both dialects, outside this change) and TO_BYTE of an integer keeps
    // its existing numeric-to-bit-string acceptance.
    let declarations = r#"
PROGRAM Test
VAR
    ul : ULINT;
    w : DWORD;
    wtext : WSTRING;
    lt : LTIME;
    evening : LTIME_OF_DAY;
    b : BYTE;
    li : LINT;
END_VAR
"#;
    for statement in [
        "lt := ULINT_TO_LTIME(ul);",
        "evening := DWORD_TO_LTOD(w);",
        "b := TO_BYTE(wtext);",
        "li := STRING_TO_LINT(wtext);",
    ] {
        let source = format!("{declarations}{statement}\nEND_PROGRAM\n");
        let errors = check_errors_in(Dialect::Codesys, &source);
        assert!(
            errors.contains(&DiagnosticCode::InvalidArgumentType),
            "{statement} stays rejected under codesys: {errors:?}"
        );
    }
}

#[test]
fn a_conversion_of_constants_is_a_constant_initial_value_under_codesys() {
    // Specification 03 and contributor issue #428: under the CODESYS profiles a type conversion of
    // constant arguments is a constant expression of an initial value, in a program's VAR, scalar
    // and inside a structure's initializer, and in a global variable list, as in a structure's
    // member default since #352. `iLocal` is an integer, which the integer evaluator judges and
    // which takes the conversion only through the CODESYS rule; the BYTE ones are bit strings.
    // Under the IEC profile it stays E202. Catches the conversion refused as "variable initializer
    // must be a literal or constant expression" outside a structure's member defaults.
    let source = r#"
TYPE stCfg :
STRUCT
    bMinOn : BYTE;
    bXMax : BYTE;
END_STRUCT
END_TYPE
VAR_GLOBAL
    gMax : BYTE := ANY_TO_BYTE(255);
END_VAR
PROGRAM Main
VAR
    bLocal : BYTE := INT_TO_BYTE(255);
    iLocal : INT := ANY_TO_INT(70000);
    stCfgLocal : stCfg := (bMinOn := 150, bXMax := ANY_TO_BYTE(255));
END_VAR
END_PROGRAM
"#;
    check_no_errors_in(Dialect::Codesys, source);
    check_has_error_in(
        Dialect::Iec,
        "PROGRAM Main\nVAR\n    b : BYTE := INT_TO_BYTE(255);\nEND_VAR\nEND_PROGRAM\n",
        DiagnosticCode::InvalidOperation,
    );
}

#[test]
fn a_conversion_of_a_mutable_variable_stays_refused_as_an_initial_value() {
    // The conversion is constant only when its arguments are: `ANY_TO_BYTE(gLevel)` names a
    // mutable global, which an initial value must not (specification 03), in a scalar and inside a
    // structure's initializer. Catches every conversion taken as a constant.
    let scalar = "VAR_GLOBAL\n    gLevel : DINT := 300;\nEND_VAR\n\
                  PROGRAM Main\nVAR\n    b : BYTE := ANY_TO_BYTE(gLevel);\nEND_VAR\nEND_PROGRAM\n";
    check_has_error_in(Dialect::Codesys, scalar, DiagnosticCode::InvalidOperation);
    let grouped = "TYPE stCfg :\nSTRUCT\n    bXMax : BYTE;\nEND_STRUCT\nEND_TYPE\n\
                   VAR_GLOBAL\n    gLevel : DINT := 300;\nEND_VAR\n\
                   PROGRAM Main\nVAR\n    c : stCfg := (bXMax := ANY_TO_BYTE(gLevel));\nEND_VAR\nEND_PROGRAM\n";
    check_has_error_in(Dialect::Codesys, grouped, DiagnosticCode::InvalidOperation);
}
