use super::*;

use crate::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use crate::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};
use trust_syntax::parser::parse;

fn diagnostics(source: &str) -> Vec<Diagnostic> {
    let mut database = Database::new();
    let file = FileId(0);
    database.set_source_text(file, source.to_owned());
    database.diagnostics(file).as_ref().clone()
}

fn task_source(globals: &str, task_init: &str) -> String {
    format!(
        r#"
CONFIGURATION Conf
{globals}
RESOURCE R ON CPU
    TASK Event ({task_init});
END_RESOURCE
END_CONFIGURATION
"#
    )
}

fn task_errors(source: &str) -> Vec<Diagnostic> {
    diagnostics(source)
        .into_iter()
        .filter(|diagnostic| {
            diagnostic.severity == DiagnosticSeverity::Error
                && diagnostic.code == DiagnosticCode::InvalidTaskConfig
        })
        .collect()
}

fn assert_task_valid(source: &str) {
    let errors = task_errors(source);
    assert!(errors.is_empty(), "unexpected task errors: {errors:?}");
}

fn assert_task_invalid(source: &str) {
    let errors = task_errors(source);
    assert!(!errors.is_empty(), "expected InvalidTaskConfig diagnostic");
}

fn parsed_task(source: &str) -> SyntaxNode {
    parse(source)
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::TaskConfig)
        .expect("task configuration")
}

#[test]
fn priority_accepts_zero_decimal_and_u32_maximum() {
    for priority in ["0", "1", "4_294_967_295"] {
        assert_task_valid(&task_source("", &format!("PRIORITY := {priority}")));
    }
}

#[test]
fn priority_rejects_value_above_runtime_u32_range() {
    assert_task_invalid(&task_source("", "PRIORITY := 4_294_967_296"));
    assert_task_invalid(&task_source("", "PRIORITY := 18_446_744_073_709_551_615"));
}

#[test]
fn priority_rejects_expression_even_when_it_contains_integer_literals() {
    for priority in ["1 + 1", "2 * 3", "(4)", "ABS(5)"] {
        assert_task_invalid(&task_source("", &format!("PRIORITY := {priority}")));
    }
}

#[test]
fn priority_rejects_signed_typed_based_real_and_string_forms() {
    for priority in ["-1", "+1", "UINT#1", "16#1", "1.0", "'1'"] {
        assert_task_invalid(&task_source("", &format!("PRIORITY := {priority}")));
    }
}

#[test]
fn priority_is_required_exactly_once() {
    assert_task_invalid(&task_source("", "INTERVAL := T#10ms"));
    assert_task_invalid(&task_source("", "PRIORITY := 1, PRIORITY := 2"));
}

#[test]
fn task_field_names_are_case_insensitive_but_unknown_fields_are_rejected() {
    assert_task_valid(&task_source("", "interval := T#10ms, priority := 1"));
    assert_task_invalid(&task_source("", "DEADLINE := T#10ms, PRIORITY := 1"));
}

#[test]
fn single_accepts_visible_bool_storage_reference() {
    assert_task_valid(&task_source(
        "VAR_GLOBAL\n    Trigger : BOOL;\nEND_VAR",
        "SINGLE := Trigger, PRIORITY := 1",
    ));
}

#[test]
fn single_rejects_boolean_literal_because_it_is_not_a_storage_source() {
    for value in ["TRUE", "FALSE"] {
        assert_task_invalid(&task_source(
            "",
            &format!("SINGLE := {value}, PRIORITY := 1"),
        ));
    }
}

#[test]
fn single_rejects_non_bool_or_missing_storage_reference() {
    assert_task_invalid(&task_source(
        "VAR_GLOBAL\n    Count : INT;\nEND_VAR",
        "SINGLE := Count, PRIORITY := 1",
    ));
    assert_task_invalid(&task_source("", "SINGLE := Missing, PRIORITY := 1"));
}

#[test]
fn interval_accepts_nonnegative_time_literal_including_zero() {
    for interval in ["T#0ms", "T#10ms", "TIME#1s"] {
        assert_task_valid(&task_source(
            "",
            &format!("INTERVAL := {interval}, PRIORITY := 1"),
        ));
    }
}

#[test]
fn interval_rejects_negative_non_time_and_nonliteral_sources() {
    for interval in ["T#-1ms", "1", "TRUE", "Period"] {
        assert_task_invalid(&task_source(
            "VAR_GLOBAL\n    Period : TIME;\nEND_VAR",
            &format!("INTERVAL := {interval}, PRIORITY := 1"),
        ));
    }
}

#[test]
fn duplicate_single_and_interval_fields_are_rejected() {
    assert_task_invalid(&task_source(
        "VAR_GLOBAL\n    A : BOOL;\n    B : BOOL;\nEND_VAR",
        "SINGLE := A, SINGLE := B, PRIORITY := 1",
    ));
    assert_task_invalid(&task_source(
        "",
        "INTERVAL := T#1ms, INTERVAL := T#2ms, PRIORITY := 1",
    ));
}

#[test]
fn task_names_are_unique_under_ascii_case_insensitive_comparison() {
    let source = r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    TASK fAsT (PRIORITY := 2);
END_RESOURCE
END_CONFIGURATION
"#;
    let errors = diagnostics(source);

    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::DuplicateDeclaration),
        "expected duplicate task diagnostic: {errors:?}"
    );
}

#[test]
fn program_task_binding_is_case_insensitive_within_same_scope() {
    let source = r#"
PROGRAM Main
END_PROGRAM

CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM Instance WITH fAsT : Main;
END_RESOURCE
END_CONFIGURATION
"#;
    let errors = diagnostics(source);

    assert!(
        !errors
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnknownTask),
        "case-insensitive task binding should resolve: {errors:?}"
    );
}

#[test]
fn program_cannot_bind_task_declared_in_sibling_resource() {
    let source = r#"
PROGRAM Main
END_PROGRAM

CONFIGURATION Conf
RESOURCE First ON CPU
    TASK Fast (PRIORITY := 1);
END_RESOURCE
RESOURCE Second ON CPU
    PROGRAM Instance WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;
    let errors = diagnostics(source);

    assert!(
        errors
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::UnknownTask),
        "sibling task must not resolve: {errors:?}"
    );
}

#[test]
fn task_field_extractor_keeps_named_fields_independent_of_order() {
    let task = parsed_task(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Event (INTERVAL := T#10ms, PRIORITY := 7, SINGLE := Trigger);
END_RESOURCE
END_CONFIGURATION
"#,
    );
    let init = task
        .children()
        .find(|node| node.kind() == SyntaxKind::TaskInit)
        .expect("task init");
    let fields = task_init_fields(&init);

    assert_eq!(
        fields
            .priority_expr
            .as_ref()
            .and_then(parse_unsigned_int_literal),
        Some(7)
    );
    assert!(fields.single_expr.as_ref().and_then(literal_kind).is_none());
    assert!(matches!(
        fields.interval_expr.as_ref().and_then(literal_kind),
        Some(LiteralKind::Time)
    ));
}

#[test]
fn unsigned_priority_parser_accepts_underscores_and_rejects_nested_literals() {
    let valid = parsed_task(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Event (PRIORITY := 4_294_967_295);
END_RESOURCE
END_CONFIGURATION
"#,
    );
    let valid_init = valid
        .children()
        .find(|node| node.kind() == SyntaxKind::TaskInit)
        .expect("valid task init");
    let valid_fields = task_init_fields(&valid_init);
    assert_eq!(
        valid_fields
            .priority_expr
            .as_ref()
            .and_then(parse_unsigned_int_literal),
        Some(u64::from(u32::MAX))
    );

    let nested = parsed_task(
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Event (PRIORITY := 1 + 1);
END_RESOURCE
END_CONFIGURATION
"#,
    );
    let nested_init = nested
        .children()
        .find(|node| node.kind() == SyntaxKind::TaskInit)
        .expect("nested task init");
    let nested_fields = task_init_fields(&nested_init);
    assert_eq!(
        nested_fields
            .priority_expr
            .as_ref()
            .and_then(parse_unsigned_int_literal),
        None
    );
}

#[test]
fn collected_task_identity_is_direct_scope_and_case_normalized() {
    let syntax = parse(
        r#"
CONFIGURATION Conf
    TASK ConfigTask (PRIORITY := 1);
    RESOURCE R ON CPU
        TASK ResourceTask (PRIORITY := 2);
    END_RESOURCE
END_CONFIGURATION
"#,
    )
    .syntax();
    let configuration = syntax
        .descendants()
        .find(|node| node.kind() == SyntaxKind::Configuration)
        .expect("configuration");
    let resource = syntax
        .descendants()
        .find(|node| node.kind() == SyntaxKind::Resource)
        .expect("resource");

    let config_tasks = collect_tasks_in_scope(&configuration);
    let resource_tasks = collect_tasks_in_scope(&resource);
    assert!(config_tasks.contains_key("CONFIGTASK"));
    assert!(!config_tasks.contains_key("RESOURCETASK"));
    assert!(resource_tasks.contains_key("RESOURCETASK"));
}
