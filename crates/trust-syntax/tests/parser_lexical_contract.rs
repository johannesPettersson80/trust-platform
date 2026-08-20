mod common;
use common::*;

fn accepted(source: &str) {
    let parsed = parse(source);
    assert!(
        parsed.ok(),
        "expected lexical source to parse: {:?}",
        parsed.errors()
    );
}

fn rejected(source: &str) {
    assert!(!parse(source).ok(), "expected malformed lexical source");
}

#[test]
fn lexical_parser_accepts_iec_identifier_forms_and_case_insensitive_keywords() {
    accepted(
        "pRoGrAm _MAIN\nVAR LIM_SW_5 : INT; LimSw5 : INT; END_VAR\nLiMsW5 := LIM_SW_5;\neNd_PrOgRaM",
    );
}

#[test]
fn lexical_parser_accepts_comments_and_pragmas_as_whitespace() {
    accepted(
        "PROGRAM Main {AUTHOR JHC}\nVAR (* nested (* comment *) *) value : INT; END_VAR\nvalue /* alt */ := 1; // line\nEND_PROGRAM",
    );
}

#[test]
fn lexical_parser_accepts_decimal_and_based_integer_literals() {
    accepted(
        "PROGRAM Main\nVAR value : DINT; END_VAR\nvalue := 123_4 + 2#1010 + 8#77 + 16#FF;\nEND_PROGRAM",
    );
}

#[test]
fn lexical_parser_accepts_signed_and_exponent_decimal_literals() {
    accepted(
        "PROGRAM Main\nVAR value : LREAL; END_VAR\nvalue := -1.34E-12 + +1.0e+6;\nEND_PROGRAM",
    );
}

#[test]
fn lexical_parser_accepts_typed_numeric_and_boolean_literals() {
    accepted(
        "PROGRAM Main\nVAR i : INT; w : WORD; b : BOOL; END_VAR\ni := INT#-123; w := WORD#16#AFF; b := BOOL#TRUE;\nEND_PROGRAM",
    );
}

#[test]
fn lexical_parser_accepts_narrow_and_wide_string_escape_families() {
    accepted(
        r#"PROGRAM Main
VAR narrow : STRING; wide : WSTRING; END_VAR
narrow := 'A$$B$L$0A$'';
wide := "A$$B$N$00C4$"";
END_PROGRAM"#,
    );
}

#[test]
fn lexical_parser_accepts_duration_aliases_sign_and_composition() {
    accepted(
        "PROGRAM Main\nVAR value : LTIME; END_VAR\nvalue := LTIME#-5d_14h_12m_18s_3.5ms;\nEND_PROGRAM",
    );
}

#[test]
fn lexical_parser_accepts_date_time_aliases_and_fraction_separators() {
    accepted(
        "PROGRAM Main\nVAR date_value : LDATE; time_value : LTOD; date_time_value : LDT; END_VAR\ndate_value := LD#1984-06-25; time_value := LTIME_OF_DAY#15:36:55.360_227_400; date_time_value := LDATE_AND_TIME#1984-06-25-15:36:55.360_227_400;\nEND_PROGRAM",
    );
}

#[test]
fn lexical_parser_keeps_shape_valid_calendar_error_for_later_validation() {
    accepted("PROGRAM Main\nVAR value : DATE; END_VAR\nvalue := D#2023-02-29;\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_multiple_leading_identifier_underscores() {
    rejected("PROGRAM Main\nVAR __value : INT; END_VAR\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_multiple_embedded_identifier_underscores() {
    rejected("PROGRAM Main\nVAR bad__value : INT; END_VAR\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_trailing_identifier_underscore() {
    rejected("PROGRAM Main\nVAR value_ : INT; END_VAR\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_malformed_numeric_separator() {
    rejected("PROGRAM Main\nVAR value : INT := 12__34; END_VAR\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_digit_outside_declared_base() {
    rejected("PROGRAM Main\nVAR value : INT := 2#102; END_VAR\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_real_exponent_without_digits() {
    rejected("PROGRAM Main\nVAR value : REAL := 1.0E+; END_VAR\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_unknown_string_escape() {
    rejected("PROGRAM Main\nVAR value : STRING := '$Q'; END_VAR\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_unterminated_block_comment() {
    rejected("PROGRAM Main\n(* unterminated");
}

#[test]
fn lexical_parser_rejects_unterminated_pragma() {
    rejected("PROGRAM Main\n{AUTHOR JHC");
}

#[test]
fn lexical_parser_rejects_fraction_on_nonleast_duration_unit() {
    rejected("PROGRAM Main\nVAR value : TIME := T#1.5h_30m; END_VAR\nEND_PROGRAM");
}

#[test]
fn lexical_parser_rejects_date_with_wrong_field_width() {
    rejected("PROGRAM Main\nVAR value : DATE := D#1984-6-25; END_VAR\nEND_PROGRAM");
}
