use crate::common::*;

fn diagnostics(source: &str) -> Vec<trust_hir::diagnostics::Diagnostic> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(file, source.to_string());
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .cloned()
        .collect()
}

fn assert_invalid_operation(source: &str) {
    let diagnostics = diagnostics(source);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::InvalidOperation),
        "expected InvalidOperation, got {diagnostics:?}"
    );
}

fn assert_rejected(source: &str) {
    let diagnostics = diagnostics(source);
    assert!(
        !diagnostics.is_empty(),
        "expected declaration to be rejected"
    );
}

fn function_section(section: &str, modifier: &str) -> String {
    let name = if section == "VAR_EXTERNAL" {
        "SharedValue"
    } else {
        "Value"
    };
    format!(
        r#"
VAR_GLOBAL
    SharedValue : INT;
END_VAR
FUNCTION InvalidFunction : INT
{section} {modifier}
    {name} : INT;
END_VAR
InvalidFunction := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#
    )
}

fn method_section(section: &str, modifier: &str) -> String {
    let name = if section == "VAR_EXTERNAL" {
        "SharedValue"
    } else {
        "Value"
    };
    format!(
        r#"
VAR_GLOBAL
    SharedValue : INT;
END_VAR
CLASS Container
METHOD PUBLIC InvalidMethod : INT
{section} {modifier}
    {name} : INT;
END_VAR
InvalidMethod := INT#0;
END_METHOD
END_CLASS
PROGRAM Main
END_PROGRAM
"#
    )
}

fn program_section(section: &str, modifier: &str) -> String {
    let name = if section == "VAR_EXTERNAL" {
        "SharedValue"
    } else {
        "Value"
    };
    format!(
        r#"
VAR_GLOBAL
    SharedValue : INT;
END_VAR
PROGRAM Main
{section} {modifier}
    {name} : INT;
END_VAR
END_PROGRAM
"#
    )
}

#[test]
fn qualifier_contract_rejects_duplicate_constant() {
    assert_invalid_operation(&program_section("VAR", "CONSTANT CONSTANT"));
}

#[test]
fn qualifier_contract_rejects_duplicate_retain() {
    assert_invalid_operation(&program_section("VAR", "RETAIN RETAIN"));
}

#[test]
fn qualifier_contract_rejects_duplicate_non_retain() {
    assert_invalid_operation(&program_section("VAR", "NON_RETAIN NON_RETAIN"));
}

#[test]
fn qualifier_contract_rejects_duplicate_persistent() {
    assert_invalid_operation(&program_section("VAR", "PERSISTENT PERSISTENT"));
}

#[test]
fn qualifier_contract_rejects_constant_retain_combination() {
    assert_invalid_operation(&program_section("VAR", "CONSTANT RETAIN"));
}

#[test]
fn qualifier_contract_rejects_constant_non_retain_combination() {
    assert_invalid_operation(&program_section("VAR", "CONSTANT NON_RETAIN"));
}

#[test]
fn qualifier_contract_rejects_constant_persistent_combination() {
    assert_invalid_operation(&program_section("VAR", "CONSTANT PERSISTENT"));
}

#[test]
fn qualifier_contract_rejects_retain_non_retain_combination() {
    assert_invalid_operation(&program_section("VAR", "RETAIN NON_RETAIN"));
}

#[test]
fn qualifier_contract_rejects_retain_persistent_combination() {
    assert_invalid_operation(&program_section("VAR", "RETAIN PERSISTENT"));
}

#[test]
fn qualifier_contract_rejects_non_retain_persistent_combination() {
    assert_invalid_operation(&program_section("VAR", "NON_RETAIN PERSISTENT"));
}

#[test]
fn qualifier_contract_function_var_rejects_retain() {
    assert_invalid_operation(&function_section("VAR", "RETAIN"));
}

#[test]
fn qualifier_contract_function_var_rejects_non_retain() {
    assert_invalid_operation(&function_section("VAR", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_function_var_rejects_persistent() {
    assert_invalid_operation(&function_section("VAR", "PERSISTENT"));
}

#[test]
fn qualifier_contract_function_input_rejects_retain() {
    assert_invalid_operation(&function_section("VAR_INPUT", "RETAIN"));
}

#[test]
fn qualifier_contract_function_input_rejects_non_retain() {
    assert_invalid_operation(&function_section("VAR_INPUT", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_function_input_rejects_persistent() {
    assert_invalid_operation(&function_section("VAR_INPUT", "PERSISTENT"));
}

#[test]
fn qualifier_contract_function_output_rejects_retain() {
    assert_invalid_operation(&function_section("VAR_OUTPUT", "RETAIN"));
}

#[test]
fn qualifier_contract_function_output_rejects_non_retain() {
    assert_invalid_operation(&function_section("VAR_OUTPUT", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_function_output_rejects_persistent() {
    assert_invalid_operation(&function_section("VAR_OUTPUT", "PERSISTENT"));
}

#[test]
fn qualifier_contract_method_var_rejects_retain() {
    assert_invalid_operation(&method_section("VAR", "RETAIN"));
}

#[test]
fn qualifier_contract_method_var_rejects_non_retain() {
    assert_invalid_operation(&method_section("VAR", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_method_var_rejects_persistent() {
    assert_invalid_operation(&method_section("VAR", "PERSISTENT"));
}

#[test]
fn qualifier_contract_method_input_rejects_retain() {
    assert_invalid_operation(&method_section("VAR_INPUT", "RETAIN"));
}

#[test]
fn qualifier_contract_method_input_rejects_non_retain() {
    assert_invalid_operation(&method_section("VAR_INPUT", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_method_input_rejects_persistent() {
    assert_invalid_operation(&method_section("VAR_INPUT", "PERSISTENT"));
}

#[test]
fn qualifier_contract_method_output_rejects_retain() {
    assert_invalid_operation(&method_section("VAR_OUTPUT", "RETAIN"));
}

#[test]
fn qualifier_contract_method_output_rejects_non_retain() {
    assert_invalid_operation(&method_section("VAR_OUTPUT", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_method_output_rejects_persistent() {
    assert_invalid_operation(&method_section("VAR_OUTPUT", "PERSISTENT"));
}

#[test]
fn qualifier_contract_in_out_rejects_retain() {
    assert_invalid_operation(&program_section("VAR_IN_OUT", "RETAIN"));
}

#[test]
fn qualifier_contract_in_out_rejects_non_retain() {
    assert_invalid_operation(&program_section("VAR_IN_OUT", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_in_out_rejects_persistent() {
    assert_invalid_operation(&program_section("VAR_IN_OUT", "PERSISTENT"));
}

#[test]
fn qualifier_contract_temp_rejects_retain() {
    assert_invalid_operation(&program_section("VAR_TEMP", "RETAIN"));
}

#[test]
fn qualifier_contract_temp_rejects_non_retain() {
    assert_invalid_operation(&program_section("VAR_TEMP", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_temp_rejects_persistent() {
    assert_invalid_operation(&program_section("VAR_TEMP", "PERSISTENT"));
}

#[test]
fn qualifier_contract_external_rejects_retain() {
    assert_invalid_operation(&program_section("VAR_EXTERNAL", "RETAIN"));
}

#[test]
fn qualifier_contract_external_rejects_non_retain() {
    assert_invalid_operation(&program_section("VAR_EXTERNAL", "NON_RETAIN"));
}

#[test]
fn qualifier_contract_external_rejects_persistent() {
    assert_invalid_operation(&program_section("VAR_EXTERNAL", "PERSISTENT"));
}

#[test]
fn qualifier_contract_access_rejects_constant() {
    assert_rejected(
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
VAR_ACCESS CONSTANT
    PublicValue : Value : INT READ_ONLY;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_access_rejects_retain() {
    assert_rejected(
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
VAR_ACCESS RETAIN
    PublicValue : Value : INT READ_ONLY;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_access_rejects_non_retain() {
    assert_rejected(
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
VAR_ACCESS NON_RETAIN
    PublicValue : Value : INT READ_ONLY;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_access_rejects_persistent() {
    assert_rejected(
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
VAR_ACCESS PERSISTENT
    PublicValue : Value : INT READ_ONLY;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_config_rejects_constant() {
    assert_rejected(
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
END_PROGRAM
CONFIGURATION Plant
PROGRAM P : Main;
VAR_CONFIG CONSTANT
    P.Value : INT := INT#1;
END_VAR
END_CONFIGURATION
"#,
    );
}

#[test]
fn qualifier_contract_config_rejects_retain() {
    assert_rejected(
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
END_PROGRAM
CONFIGURATION Plant
PROGRAM P : Main;
VAR_CONFIG RETAIN
    P.Value : INT := INT#1;
END_VAR
END_CONFIGURATION
"#,
    );
}

#[test]
fn qualifier_contract_config_rejects_non_retain() {
    assert_rejected(
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
END_PROGRAM
CONFIGURATION Plant
PROGRAM P : Main;
VAR_CONFIG NON_RETAIN
    P.Value : INT := INT#1;
END_VAR
END_CONFIGURATION
"#,
    );
}

#[test]
fn qualifier_contract_config_rejects_persistent() {
    assert_rejected(
        r#"
PROGRAM Main
VAR
    Value : INT;
END_VAR
END_PROGRAM
CONFIGURATION Plant
PROGRAM P : Main;
VAR_CONFIG PERSISTENT
    P.Value : INT := INT#1;
END_VAR
END_CONFIGURATION
"#,
    );
}
