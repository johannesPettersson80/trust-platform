use crate::harness::CompileSession;

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("invalid variable-section fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn assert_rejected(source: &str, owner: &str, section: &str) {
    let error = compile_error(source);
    assert!(
        !error.trim().is_empty(),
        "{owner} {section} rejection must be observable"
    );
}

#[test]
fn pou_variable_section_function_rejects_var_global() {
    assert_rejected(
        r#"
FUNCTION Broken : INT
VAR_GLOBAL
    Value : INT;
END_VAR
Broken := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
        "FUNCTION",
        "VAR_GLOBAL",
    );
}

#[test]
fn pou_variable_section_function_rejects_var_access() {
    assert_rejected(
        r#"
FUNCTION Broken : INT
VAR_ACCESS
    PublicValue : Value : INT READ_ONLY;
END_VAR
Broken := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
        "FUNCTION",
        "VAR_ACCESS",
    );
}

#[test]
fn pou_variable_section_function_rejects_var_config() {
    assert_rejected(
        r#"
FUNCTION Broken : INT
VAR_CONFIG
    Main.Value : INT := INT#1;
END_VAR
Broken := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
        "FUNCTION",
        "VAR_CONFIG",
    );
}

#[test]
fn pou_variable_section_method_rejects_var_global() {
    assert_rejected(
        r#"
CLASS Container
METHOD PUBLIC Broken : INT
VAR_GLOBAL
    Value : INT;
END_VAR
Broken := INT#0;
END_METHOD
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "METHOD",
        "VAR_GLOBAL",
    );
}

#[test]
fn pou_variable_section_method_rejects_var_access() {
    assert_rejected(
        r#"
CLASS Container
METHOD PUBLIC Broken : INT
VAR_ACCESS
    PublicValue : Value : INT READ_ONLY;
END_VAR
Broken := INT#0;
END_METHOD
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "METHOD",
        "VAR_ACCESS",
    );
}

#[test]
fn pou_variable_section_method_rejects_var_config() {
    assert_rejected(
        r#"
CLASS Container
METHOD PUBLIC Broken : INT
VAR_CONFIG
    Main.Value : INT := INT#1;
END_VAR
Broken := INT#0;
END_METHOD
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "METHOD",
        "VAR_CONFIG",
    );
}

#[test]
fn pou_variable_section_function_block_rejects_var_global() {
    assert_rejected(
        r#"
FUNCTION_BLOCK Broken
VAR_GLOBAL
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
        "FUNCTION_BLOCK",
        "VAR_GLOBAL",
    );
}

#[test]
fn pou_variable_section_function_block_rejects_var_access() {
    assert_rejected(
        r#"
FUNCTION_BLOCK Broken
VAR_ACCESS
    PublicValue : Value : INT READ_ONLY;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
        "FUNCTION_BLOCK",
        "VAR_ACCESS",
    );
}

#[test]
fn pou_variable_section_function_block_rejects_var_config() {
    assert_rejected(
        r#"
FUNCTION_BLOCK Broken
VAR_CONFIG
    Main.Value : INT := INT#1;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
        "FUNCTION_BLOCK",
        "VAR_CONFIG",
    );
}

#[test]
fn pou_variable_section_program_rejects_var_config() {
    assert_rejected(
        r#"
PROGRAM Main
VAR_CONFIG
    Main.Value : INT := INT#1;
END_VAR
END_PROGRAM
"#,
        "PROGRAM",
        "VAR_CONFIG",
    );
}

#[test]
fn pou_variable_section_class_rejects_var_input() {
    assert_rejected(
        r#"
CLASS Broken
VAR_INPUT
    Value : INT;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "CLASS",
        "VAR_INPUT",
    );
}

#[test]
fn pou_variable_section_class_rejects_var_output() {
    assert_rejected(
        r#"
CLASS Broken
VAR_OUTPUT
    Value : INT;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "CLASS",
        "VAR_OUTPUT",
    );
}

#[test]
fn pou_variable_section_class_rejects_var_in_out() {
    assert_rejected(
        r#"
CLASS Broken
VAR_IN_OUT
    Value : INT;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "CLASS",
        "VAR_IN_OUT",
    );
}

#[test]
fn pou_variable_section_class_rejects_var_temp() {
    assert_rejected(
        r#"
CLASS Broken
VAR_TEMP
    Value : INT;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "CLASS",
        "VAR_TEMP",
    );
}

#[test]
fn pou_variable_section_class_rejects_var_global() {
    assert_rejected(
        r#"
CLASS Broken
VAR_GLOBAL
    Value : INT;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "CLASS",
        "VAR_GLOBAL",
    );
}

#[test]
fn pou_variable_section_class_rejects_var_access() {
    assert_rejected(
        r#"
CLASS Broken
VAR_ACCESS
    PublicValue : Value : INT READ_ONLY;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "CLASS",
        "VAR_ACCESS",
    );
}

#[test]
fn pou_variable_section_class_rejects_var_config() {
    assert_rejected(
        r#"
CLASS Broken
VAR_CONFIG
    Main.Value : INT := INT#1;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        "CLASS",
        "VAR_CONFIG",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR
    Value : INT;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_stat() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_STAT
    Value : INT;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_STAT",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_temp() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_TEMP
    Value : INT;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_TEMP",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_input() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_INPUT
    Value : INT;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_INPUT",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_output() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_OUTPUT
    Value : INT;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_OUTPUT",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_in_out() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_IN_OUT
    Value : INT;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_IN_OUT",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_external() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_EXTERNAL
    Value : INT;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_EXTERNAL",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_global() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_GLOBAL
    Value : INT;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_GLOBAL",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_access() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_ACCESS
    PublicValue : Value : INT READ_ONLY;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_ACCESS",
    );
}

#[test]
fn pou_variable_section_interface_rejects_var_config() {
    assert_rejected(
        r#"
INTERFACE IBroken
VAR_CONFIG
    Main.Value : INT := INT#1;
END_VAR
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
        "INTERFACE",
        "VAR_CONFIG",
    );
}
