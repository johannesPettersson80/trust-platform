use super::*;

const RESERVED_WORDS: &str = "
PROGRAM END_PROGRAM FUNCTION END_FUNCTION FUNCTION_BLOCK END_FUNCTION_BLOCK
CLASS END_CLASS METHOD END_METHOD PROPERTY END_PROPERTY INTERFACE END_INTERFACE
NAMESPACE END_NAMESPACE USING ACTION END_ACTION
VAR END_VAR VAR_INPUT VAR_OUTPUT VAR_IN_OUT VAR_TEMP VAR_GLOBAL VAR_EXTERNAL
VAR_ACCESS VAR_CONFIG VAR_STAT CONSTANT RETAIN NON_RETAIN PERSISTENT
PUBLIC PRIVATE PROTECTED INTERNAL FINAL ABSTRACT OVERRIDE
TYPE END_TYPE STRUCT END_STRUCT UNION END_UNION ARRAY OF STRING WSTRING POINTER
REF REF_TO TO EXTENDS IMPLEMENTS THIS SUPER NEW __NEW __DELETE
IF THEN ELSIF ELSE END_IF CASE END_CASE FOR END_FOR BY DO WHILE END_WHILE
REPEAT UNTIL END_REPEAT RETURN EXIT CONTINUE JMP
AND OR XOR NOT MOD
BOOL SINT INT DINT LINT USINT UINT UDINT ULINT REAL LREAL BYTE WORD DWORD LWORD
TIME LTIME DATE LDATE TIME_OF_DAY TOD LTIME_OF_DAY LTOD DATE_AND_TIME DT
LDATE_AND_TIME LDT CHAR WCHAR
ANY ANY_DERIVED ANY_ELEMENTARY ANY_MAGNITUDE ANY_INT ANY_UNSIGNED ANY_SIGNED
ANY_REAL ANY_NUM ANY_DURATION ANY_BIT ANY_CHARS ANY_STRING ANY_CHAR ANY_DATE
TRUE FALSE NULL
CONFIGURATION END_CONFIGURATION RESOURCE END_RESOURCE ON TASK WITH AT EN ENO
R_EDGE F_EDGE ADR SIZEOF READ_ONLY READ_WRITE GET END_GET SET END_SET
STEP END_STEP INITIAL_STEP TRANSITION END_TRANSITION FROM
";

#[test]
fn valid_identifier_matrix_covers_each_allowed_ascii_position() {
    for name in [
        "A",
        "z",
        "_A",
        "_12V7",
        "A0",
        "a9",
        "A_B",
        "a_b_c",
        "Mixed_Case_123",
    ] {
        assert!(is_valid_identifier(name), "{name:?}");
    }
}

#[test]
fn invalid_start_end_and_repeated_underscore_matrix_is_rejected() {
    for name in ["", "_", "__", "__A", "A__B", "A___B", "A_", "1A", "9_value"] {
        assert!(!is_valid_identifier(name), "{name:?}");
    }
}

#[test]
fn punctuation_whitespace_and_non_ascii_are_rejected() {
    for name in [
        "A-B",
        "A.B",
        "A B",
        " A",
        "A ",
        "A/B",
        "A#B",
        "A$B",
        "Ångström",
        "变量",
        "A\u{0301}",
    ] {
        assert!(!is_valid_identifier(name), "{name:?}");
    }
}

#[test]
fn implementation_specific_identifier_length_is_not_artificially_truncated() {
    let long_name = format!("A{}", "b".repeat(16_384));

    assert!(is_valid_identifier(&long_name));
}

#[test]
fn complete_declared_keyword_vocabulary_is_reserved_case_insensitively() {
    for keyword in RESERVED_WORDS.split_whitespace() {
        assert!(is_reserved_keyword(keyword), "{keyword}");
        assert!(
            is_reserved_keyword(&keyword.to_ascii_lowercase()),
            "{} lower-case",
            keyword
        );
        let mixed = keyword
            .chars()
            .enumerate()
            .map(|(index, ch)| {
                if index % 2 == 0 {
                    ch.to_ascii_lowercase()
                } else {
                    ch.to_ascii_uppercase()
                }
            })
            .collect::<String>();
        assert!(is_reserved_keyword(&mixed), "{mixed}");
    }
}

#[test]
fn near_misses_and_ordinary_names_are_not_reserved() {
    for name in [
        "",
        "PROGRAMMER",
        "ENDPROGRAM",
        "VARIABLE",
        "INTEGER",
        "BOOL_VALUE",
        "CURRENT_DT",
        "TIMEOUT",
        "WITHIN",
        "CLASSIC",
        "_PROGRAM",
    ] {
        assert!(!is_reserved_keyword(name), "{name:?}");
    }
}

#[test]
fn lexical_validity_and_keyword_reservation_are_separate_decisions() {
    for keyword in ["PROGRAM", "int", "END_IF"] {
        assert!(is_valid_identifier(keyword), "{keyword}");
        assert!(is_reserved_keyword(keyword), "{keyword}");
    }
    assert!(!is_valid_identifier("__NEW"));
    assert!(is_reserved_keyword("__NEW"));

    assert!(is_valid_identifier("OperatorName"));
    assert!(!is_reserved_keyword("OperatorName"));
}

#[test]
fn reserved_check_does_not_trim_or_rewrite_input() {
    for name in [" PROGRAM", "PROGRAM ", "END IF", "VAR-INPUT"] {
        assert!(!is_reserved_keyword(name), "{name:?}");
    }
}
