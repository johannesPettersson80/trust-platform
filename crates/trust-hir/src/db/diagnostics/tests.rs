use super::super::*;
use crate::diagnostics::DiagnosticCode;
use crate::dialect::Dialect;
use crate::DiagnosticSeverity;

#[test]
fn test_database_basic() {
    let mut db = Database::new();
    let file = FileId(0);

    db.set_source_text(file, "PROGRAM Test END_PROGRAM".to_string());

    let source = db.source_text(file);
    assert!(source.contains("PROGRAM"));
}

#[test]
fn test_expr_id_type_of() {
    let mut db = Database::new();
    let file = FileId(0);
    let source = "PROGRAM Test VAR x : DINT; END_VAR x := 1 + 2; END_PROGRAM";
    db.set_source_text(file, source.to_string());

    let plus_offset = source.find('+').unwrap() as u32;
    let expr_id = db.expr_id_at_offset(file, plus_offset).unwrap();
    let expr_type = db.type_of(file, expr_id);
    assert_eq!(expr_type, TypeId::SINT);
}

#[test]
fn test_expr_id_type_of_based_literal() {
    let mut db = Database::new();
    let file = FileId(0);
    let source = "PROGRAM Test VAR x : UINT; END_VAR x := 16#FF; END_PROGRAM";
    db.set_source_text(file, source.to_string());

    let hash_offset = source.find('#').unwrap() as u32;
    let expr_id = db.expr_id_at_offset(file, hash_offset).unwrap();
    let expr_type = db.type_of(file, expr_id);
    assert_eq!(expr_type, TypeId::USINT);
}

#[test]
fn test_type_of_cache_invalidates_on_change() {
    let mut db = Database::new();
    let file = FileId(0);
    let source = "PROGRAM Test VAR x : DINT; END_VAR x := 1 + 2; END_PROGRAM";
    db.set_source_text(file, source.to_string());

    let plus_offset = source.find('+').unwrap() as u32;
    let expr_id = db.expr_id_at_offset(file, plus_offset).unwrap();
    let expr_type = db.type_of(file, expr_id);
    assert_eq!(expr_type, TypeId::SINT);

    let updated = "PROGRAM Test VAR x : DINT; END_VAR x := REAL#1.0 + REAL#2.0; END_PROGRAM";
    db.set_source_text(file, updated.to_string());
    let plus_offset = updated.find('+').unwrap() as u32;
    let expr_id = db.expr_id_at_offset(file, plus_offset).unwrap();
    let expr_type = db.type_of(file, expr_id);
    assert_eq!(expr_type, TypeId::REAL);
}

// A literal beyond i64 is not evaluable in a type or member default, so it reports this message.
const NOT_CONSTANT_MEMBER_DEFAULT: &str =
    "type/member default initializer must be a constant expression";

/// Error messages reported for `source`, in report order.
fn error_messages(source: &str) -> Vec<String> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

/// Error messages for `source` analyzed under `dialect`.
fn error_messages_in(dialect: Dialect, source: &str) -> Vec<String> {
    let mut db = Database::new();
    db.set_dialect(dialect);
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

#[test]
fn ulint_maximum_literal_assigns_and_initializes_without_errors() {
    let assigned = "PROGRAM Main\nVAR u : ULINT; END_VAR\nu := 18446744073709551615;\nEND_PROGRAM";
    assert_eq!(error_messages(assigned), Vec::<String>::new());
    let initialized = "PROGRAM Main\nVAR u : ULINT := 18446744073709551615; END_VAR\nEND_PROGRAM";
    assert_eq!(error_messages(initialized), Vec::<String>::new());
}

#[test]
fn lint_minimum_literal_initializes_typed_and_untyped_without_errors() {
    for initial in ["LINT#-9223372036854775808", "-9223372036854775808"] {
        let source = format!("PROGRAM Main\nVAR x : LINT := {initial}; END_VAR\nEND_PROGRAM");
        assert_eq!(error_messages(&source), Vec::<String>::new(), "{initial}");
    }
}

#[test]
fn literal_one_past_an_integer_bound_is_reported_once() {
    // IEC_DEVIATIONS 2026-10-28: DINT into ULINT is a narrowing binding (warning); the
    // out-of-range literal is reported exactly once as OutOfRange, because
    // the warning no longer suppresses the range check.
    let source = "PROGRAM Main\nVAR u : ULINT; END_VAR\nu := 18446744073709551616;\nEND_PROGRAM";
    assert_only_error_in(
        Dialect::Codesys,
        source,
        DiagnosticCode::OutOfRange,
        "integer literal 18446744073709551616 is outside the range of ULINT",
    );
    assert_one_narrowing_warning_in(
        Dialect::Codesys,
        source,
        DiagnosticCode::IncompatibleAssignment,
    );
    for (initial, message) in [
        (
            "ULINT := 18446744073709551616",
            "integer literal 18446744073709551616 is outside the range of ULINT",
        ),
        (
            "LINT := 9223372036854775808",
            "integer literal 9223372036854775808 is outside the range of LINT",
        ),
        (
            "LINT := -9223372036854775809",
            "integer literal -9223372036854775809 is outside the range of LINT",
        ),
    ] {
        let source = format!("PROGRAM Main\nVAR x : {initial}; END_VAR\nEND_PROGRAM");
        assert_only_error(&source, DiagnosticCode::OutOfRange, message);
    }
}

#[test]
fn integer_subrange_takes_its_base_type_then_checks_its_bounds() {
    assert_no_errors(
        "PROGRAM Main\nVAR x : LINT(0..10000000000) := 5000000000; END_VAR\nEND_PROGRAM",
    );
    assert_only_error(
        "PROGRAM Main\nVAR x : LINT(0..10000000000) := 20000000000; END_VAR\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
        "integer default 20000000000 is outside target type range 0..10000000000",
    );
    assert_no_errors(
        "PROGRAM Main\nVAR x : LINT(0..10000000000); END_VAR\nx := 5000000000;\nEND_PROGRAM",
    );
    assert_only_error(
        "PROGRAM Main\nVAR x : LINT(0..10000000000); END_VAR\nx := 20000000000;\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
        "value 20000000000 outside subrange 0..10000000000",
    );
}

#[test]
fn comparison_literal_takes_the_other_operand_type_in_analysis() {
    // A representable literal contextualizes to UDINT and compares clean.
    let accepted = error_messages(
        "PROGRAM Main\nVAR d : UDINT; r : BOOL; END_VAR\nr := d > 3000000000;\nEND_PROGRAM",
    );
    assert_eq!(accepted, Vec::<String>::new());
    // IEC_DEVIATIONS 2026-10-28: a literal beyond the UDINT range keeps its own integer type
    // and the cross-width comparison admits silently under the CODESYS
    // profile (the mathematical result is exact; the literal's own range is
    // its own business). Other profiles keep the strict comparison.
    let rejected = error_messages_in(
        Dialect::Codesys,
        "PROGRAM Main\nVAR d : UDINT; r : BOOL; END_VAR\nr := d > 5000000000;\nEND_PROGRAM",
    );
    assert_eq!(rejected, Vec::<String>::new());
}

#[test]
fn array_element_and_struct_field_defaults_take_their_declared_types() {
    let array = error_messages(
        "PROGRAM Main\nVAR x : ARRAY[0..1] OF LINT := [3000000000, -3000000000]; \
         END_VAR\nEND_PROGRAM",
    );
    assert_eq!(array, Vec::<String>::new());
    let field = error_messages(
        "TYPE S : STRUCT f : ULINT := 18446744073709551615; END_STRUCT END_TYPE\n\
         PROGRAM Main\nVAR s : S; END_VAR\nEND_PROGRAM",
    );
    assert_eq!(field, Vec::<String>::new());
    let one_past = error_messages(
        "TYPE S : STRUCT f : ULINT := 18446744073709551616; END_STRUCT END_TYPE\n\
         PROGRAM Main\nVAR s : S; END_VAR\nEND_PROGRAM",
    );
    assert!(
        one_past
            .iter()
            .any(|message| message == NOT_CONSTANT_MEMBER_DEFAULT),
        "{one_past:?}"
    );
}

/// Error diagnostics for `source` as (code, message) pairs.
fn error_codes_and_messages(source: &str) -> Vec<(DiagnosticCode, String)> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| (diagnostic.code, diagnostic.message.clone()))
        .collect()
}

/// Asserts that `source` has no error diagnostics.
fn assert_no_errors(source: &str) {
    let errors = error_codes_and_messages(source);
    assert!(errors.is_empty(), "{errors:?}");
}

/// Asserts that `source` has exactly one error diagnostic, with this code and message.
fn assert_only_error(source: &str, code: DiagnosticCode, message: &str) {
    let errors = error_codes_and_messages(source);
    assert!(
        errors.len() == 1 && errors[0].0 == code && errors[0].1 == message,
        "expected exactly one {code:?}: {message}; got {errors:?}"
    );
}

/// Dialect-aware variant of [`assert_only_error`].
fn assert_only_error_in(dialect: Dialect, source: &str, code: DiagnosticCode, message: &str) {
    let mut db = Database::new();
    db.set_dialect(dialect);
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let errors: Vec<(DiagnosticCode, String)> = db
        .diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.is_error())
        .map(|diagnostic| (diagnostic.code, diagnostic.message.clone()))
        .collect();
    assert!(
        errors.len() == 1 && errors[0].0 == code && errors[0].1 == message,
        "expected exactly one {code:?}: {message}; got {errors:?}"
    );
}

/// Asserts that `source` has exactly one error diagnostic, with this code.
fn assert_only_error_code(source: &str, code: DiagnosticCode) {
    let errors = error_codes_and_messages(source);
    assert!(
        errors.len() == 1 && errors[0].0 == code,
        "expected exactly one {code:?}; got {errors:?}"
    );
}

/// Narrowing-warning variant analyzed under `dialect`.
fn assert_one_narrowing_warning_in(dialect: Dialect, source: &str, code: DiagnosticCode) {
    let mut db = Database::new();
    db.set_dialect(dialect);
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    let narrowing = db
        .diagnostics(file)
        .iter()
        .filter(|diagnostic| {
            diagnostic.code == code && diagnostic.severity == DiagnosticSeverity::Warning
        })
        .count();
    assert_eq!(
        narrowing, 1,
        "expected exactly one narrowing warning for {code:?} in {source}"
    );
}

#[test]
fn literal_above_lint_maximum_is_rejected_by_assignment_analysis() {
    assert_only_error(
        "PROGRAM Main\nVAR x : LINT; END_VAR\nx := 9223372036854775808;\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
        "integer literal 9223372036854775808 is outside the range of LINT",
    );
    assert_no_errors(
        "PROGRAM Main\nVAR x : LINT; END_VAR\nx := -9223372036854775808;\nEND_PROGRAM",
    );
}

#[test]
fn narrow_literal_assignment_reports_range_once_and_narrowing_warning() {
    // IEC_DEVIATIONS 2026-10-28: under the CODESYS profile the
    // assignment-level rejection of 128 into SINT is a narrowing warning,
    // so the range check still runs and reports the out-of-range literal
    // exactly once. Other profiles keep both diagnostics as errors.
    let source = "PROGRAM Main\nVAR small : SINT; END_VAR\nsmall := 128;\nEND_PROGRAM";
    assert_only_error_in(
        Dialect::Codesys,
        source,
        DiagnosticCode::OutOfRange,
        "integer literal 128 is outside the range of SINT",
    );
    assert_one_narrowing_warning_in(
        Dialect::Codesys,
        source,
        DiagnosticCode::IncompatibleAssignment,
    );
}

#[test]
fn udint_call_argument_outside_its_range_is_reported_once() {
    let function = "FUNCTION F : INT\nVAR_INPUT x : UDINT; END_VAR\nF := 1;\nEND_FUNCTION\n";
    let outside = format!(
        "{function}PROGRAM Main\nVAR r : INT; END_VAR\nr := F(x := 4294967296);\nEND_PROGRAM"
    );
    // IEC_DEVIATIONS 2026-10-28: under the CODESYS profile the narrowed
    // argument is a warning; the out-of-range literal is the only error,
    // reported once. Other profiles keep both diagnostics as errors.
    assert_only_error_in(
        Dialect::Codesys,
        &outside,
        DiagnosticCode::OutOfRange,
        "integer literal 4294967296 is outside the range of UDINT",
    );
    assert_one_narrowing_warning_in(
        Dialect::Codesys,
        &outside,
        DiagnosticCode::InvalidArgumentType,
    );
    let inside = format!(
        "{function}PROGRAM Main\nVAR r : INT; END_VAR\nr := F(x := 4294967295);\nEND_PROGRAM"
    );
    assert_no_errors(&inside);
}

#[test]
fn usint_return_value_outside_its_range_is_reported_once() {
    // The return check already rejects the value, so only that diagnostic is reported.
    assert_only_error_code(
        "FUNCTION G : USINT\nRETURN 256;\nEND_FUNCTION",
        DiagnosticCode::InvalidReturnType,
    );
    assert_no_errors("FUNCTION G : USINT\nRETURN 255;\nEND_FUNCTION");
}

#[test]
fn call_argument_is_checked_against_its_subrange_bounds() {
    let function = "FUNCTION F : INT\nVAR_INPUT x : INT(0..100); END_VAR\nF := x;\nEND_FUNCTION\n";
    let outside =
        format!("{function}PROGRAM Main\nVAR r : INT; END_VAR\nr := F(x := 500);\nEND_PROGRAM");
    assert_only_error(
        &outside,
        DiagnosticCode::OutOfRange,
        "value 500 outside subrange 0..100",
    );
    let inside =
        format!("{function}PROGRAM Main\nVAR r : INT; END_VAR\nr := F(x := 50);\nEND_PROGRAM");
    assert_no_errors(&inside);
}

#[test]
fn return_value_is_checked_against_its_subrange_bounds() {
    let declarations = "TYPE Small : INT(0..100); END_TYPE\nFUNCTION G : Small\n";
    assert_only_error(
        &format!("{declarations}RETURN 500;\nEND_FUNCTION"),
        DiagnosticCode::OutOfRange,
        "value 500 outside subrange 0..100",
    );
    assert_no_errors(&format!("{declarations}RETURN 50;\nEND_FUNCTION"));
}

#[test]
fn for_bound_is_checked_against_its_control_variable_subrange() {
    // A subrange is not a valid FOR iterator, so such a loop also reports the iterator itself.
    let declarations = "TYPE Small : INT(0..100); END_TYPE\nPROGRAM Main\nVAR i : Small; END_VAR\n";
    let outside = error_codes_and_messages(&format!(
        "{declarations}FOR i := 0 TO 500 DO\nEND_FOR;\nEND_PROGRAM"
    ));
    let range_errors = outside
        .iter()
        .filter(|(code, _)| *code == DiagnosticCode::OutOfRange)
        .count();
    assert_eq!(range_errors, 1, "{outside:?}");
    assert!(
        outside.iter().any(|(code, message)| {
            *code == DiagnosticCode::OutOfRange && message == "value 500 outside subrange 0..100"
        }),
        "{outside:?}"
    );
    let inside = error_codes_and_messages(&format!(
        "{declarations}FOR i := 0 TO 50 DO\nEND_FOR;\nEND_PROGRAM"
    ));
    assert!(
        inside
            .iter()
            .all(|(code, _)| *code != DiagnosticCode::OutOfRange),
        "{inside:?}"
    );
}

#[test]
fn comparison_with_subrange_operand_keeps_literals_in_the_base_type() {
    assert_no_errors(
        "PROGRAM Main\nVAR x : UDINT(0..100); r : BOOL; END_VAR\n\
         r := x > 3000000000;\nEND_PROGRAM",
    );
}

#[test]
fn lint_initializer_above_i64_maximum_is_a_range_error() {
    assert_only_error(
        "PROGRAM Main\nVAR x : LINT := 9223372036854775808; END_VAR\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
        "integer literal 9223372036854775808 is outside the range of LINT",
    );
}

#[test]
fn usint_initializer_above_its_range_is_a_range_error_in_constant_blocks_too() {
    assert_only_error(
        "PROGRAM Main\nVAR x : USINT := 256; END_VAR\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
        "integer literal 256 is outside the range of USINT",
    );
    assert_only_error(
        "PROGRAM Main\nVAR CONSTANT c : USINT := 256; END_VAR\nEND_PROGRAM",
        DiagnosticCode::OutOfRange,
        "integer literal 256 is outside the range of USINT",
    );
    assert_no_errors("PROGRAM Main\nVAR x : USINT := 255; END_VAR\nEND_PROGRAM");
}
