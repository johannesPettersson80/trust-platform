//! STRING and WSTRING lengths (docs/IEC_DECISIONS.md 2026-10-17): a declaration without a length
//! is STRING[80] or WSTRING[80]; a declared length runs from 1 to 65,535; capacities count UTF-8
//! bytes (STRING) and UTF-16 code units (WSTRING).
use crate::common::*;
use trust_hir::TypeId;

fn error_messages(source: &str) -> Vec<(DiagnosticCode, String)> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| (diagnostic.code, diagnostic.message.clone()))
        .collect()
}

/// The resolved type of the global or program variable `name`.
fn variable_type(source: &str, name: &str) -> Type {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let symbols = db.file_symbols(file);
    let symbol = symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap_or_else(|| panic!("variable {name}"));
    let type_id = symbols.resolve_alias_type(symbol.type_id);
    symbols.type_by_id(type_id).cloned().expect("resolved type")
}

#[test]
fn a_string_without_a_length_is_string_80() {
    let source = r#"
PROGRAM Main
VAR
    plain : STRING;
    wide : WSTRING;
    plainBytes : ARRAY[0..SIZEOF(plain) - 1] OF BYTE;
    wideBytes : ARRAY[0..SIZEOF(wide) - 1] OF BYTE;
END_VAR
END_PROGRAM
"#;
    check_no_errors(source);
    assert_eq!(
        variable_type(source, "plain"),
        Type::String { max_len: Some(80) }
    );
    assert_eq!(
        variable_type(source, "wide"),
        Type::WString { max_len: Some(80) }
    );
    let Type::Array { dimensions, .. } = variable_type(source, "plainBytes") else {
        panic!("expected an array");
    };
    assert_eq!(dimensions, vec![(0, 79)]);
    let Type::Array { dimensions, .. } = variable_type(source, "wideBytes") else {
        panic!("expected an array");
    };
    assert_eq!(dimensions, vec![(0, 159)]);
}

#[test]
fn string_80_is_the_same_type_as_string() {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        "PROGRAM Main\nVAR plain : STRING; sized : STRING[80]; wide : WSTRING[80]; END_VAR\nEND_PROGRAM\n"
            .to_string(),
    );
    let symbols = db.file_symbols(file);
    let type_of = |name: &str| {
        symbols
            .iter()
            .find(|symbol| symbol.name == name)
            .map(|symbol| symbol.type_id)
            .expect("declared variable")
    };
    assert_eq!(type_of("plain"), TypeId::STRING);
    assert_eq!(type_of("sized"), TypeId::STRING);
    assert_eq!(type_of("wide"), TypeId::WSTRING);
}

#[test]
fn in_out_binds_string_to_string_80_in_both_directions() {
    check_no_errors(
        r#"
FUNCTION_BLOCK Bind
VAR_IN_OUT
    plain : STRING;
    sized : STRING[80];
    wide : WSTRING;
    wideSized : WSTRING[80];
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Bind;
    plain : STRING;
    sized : STRING[80];
    wide : WSTRING;
    wideSized : WSTRING[80];
    reference : REF_TO STRING;
END_VAR
fb(plain := sized, sized := plain, wide := wideSized, wideSized := wide);
reference := REF(sized);
END_PROGRAM
"#,
    );
}

#[test]
fn in_out_refuses_a_string_whose_length_is_not_80() {
    let errors = check_errors(
        r#"
FUNCTION_BLOCK Bind
VAR_IN_OUT
    plain : STRING;
    wide : WSTRING;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Bind;
    longer : STRING[81];
    wideShorter : WSTRING[79];
END_VAR
fb(plain := longer, wide := wideShorter);
END_PROGRAM
"#,
    );
    assert_eq!(
        errors
            .iter()
            .filter(|code| **code == DiagnosticCode::InvalidArgumentType)
            .count(),
        2,
        "STRING and WSTRING bind only to length 80: {errors:?}"
    );
}

#[test]
fn a_declared_length_is_at_most_65535() {
    check_no_errors(
        r#"
PROGRAM Main
VAR
    largest : STRING[65535];
    wideLargest : WSTRING(65535);
END_VAR
END_PROGRAM
"#,
    );
    let above = (
        DiagnosticCode::OutOfRange,
        "string length exceeds the maximum of 65535".to_string(),
    );
    for source in [
        "PROGRAM Main\nVAR\n    text : STRING[65536];\nEND_VAR\nEND_PROGRAM\n",
        "PROGRAM Main\nVAR\n    text : WSTRING[65536];\nEND_VAR\nEND_PROGRAM\n",
        "PROGRAM Main\nVAR\n    text : STRING(70000);\nEND_VAR\nEND_PROGRAM\n",
        "PROGRAM Main\nVAR size : DINT; END_VAR\nsize := SIZEOF(STRING[65536]);\nEND_PROGRAM\n",
    ] {
        let errors = error_messages(source);
        assert!(
            !errors.is_empty() && errors.iter().all(|error| *error == above),
            "{source}: {errors:?}"
        );
    }
}

#[test]
fn a_literal_longer_than_80_bytes_does_not_fit_a_string_without_a_length() {
    let fits = "x".repeat(80);
    check_no_errors(&format!(
        "PROGRAM Main\nVAR text : STRING := '{fits}'; END_VAR\ntext := '{fits}';\nEND_PROGRAM\n"
    ));
    let longer = "x".repeat(81);
    let refused = (
        DiagnosticCode::OutOfRange,
        "STRING literal length 81 exceeds STRING capacity".to_string(),
    );
    // Once for the initializer and once for the assignment.
    let errors = error_messages(&format!(
        "PROGRAM Main\nVAR text : STRING := '{longer}'; END_VAR\ntext := '{longer}';\nEND_PROGRAM\n"
    ));
    assert_eq!(errors, vec![refused.clone(), refused]);
}

#[test]
fn literal_bounds_count_utf8_bytes_and_utf16_code_units() {
    // "Ä" is two bytes of UTF-8 and one UTF-16 code unit; "🙂" is four bytes and two units.
    check_no_errors(
        r#"
PROGRAM Main
VAR
    two : STRING[3] := 'ÄB';
    escaped : STRING[2] := '$C4';
    wide : WSTRING[3] := "ÄÖÜ";
    emoji : WSTRING[2] := "🙂";
END_VAR
END_PROGRAM
"#,
    );
    let mut errors = error_messages(
        r#"
PROGRAM Main
VAR
    three : STRING[3] := 'ÄÖ';
    escaped : STRING[1] := '$C4';
    emoji : WSTRING[3] := "🙂🙂";
END_VAR
END_PROGRAM
"#,
    );
    errors.sort_by(|left, right| left.1.cmp(&right.1));
    assert_eq!(
        errors,
        vec![
            (
                DiagnosticCode::OutOfRange,
                "STRING literal length 2 exceeds STRING[1] capacity".to_string()
            ),
            (
                DiagnosticCode::OutOfRange,
                "STRING literal length 4 exceeds STRING[3] capacity".to_string()
            ),
            (
                DiagnosticCode::OutOfRange,
                "WSTRING literal length 4 exceeds WSTRING[3] capacity".to_string()
            ),
        ]
    );
}

#[test]
fn a_one_character_literal_outside_ascii_still_names_one_character() {
    // A contextual CHAR literal is one character, whatever its UTF-8 length.
    check_no_errors(
        "PROGRAM Main\nVAR text : STRING[4] := 'AB'; END_VAR\ntext[1] := 'Ä';\nEND_PROGRAM\n",
    );
}

#[test]
fn an_index_above_80_is_out_of_range_for_a_string_without_a_length() {
    check_no_errors(
        "PROGRAM Main\nVAR text : STRING; ch : CHAR; END_VAR\nch := text[80];\nEND_PROGRAM\n",
    );
    check_has_error(
        "PROGRAM Main\nVAR text : STRING; ch : CHAR; END_VAR\nch := text[81];\nEND_PROGRAM\n",
        DiagnosticCode::OutOfRange,
    );
}
