use crate::db::{Database, FileId, SemanticDatabase, SourceDatabase};
use crate::diagnostics::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

fn diagnostics_for(sources: &[&str], file: usize) -> Vec<Diagnostic> {
    let mut database = Database::new();
    for (index, source) in sources.iter().enumerate() {
        database.set_source_text(FileId(index as u32), (*source).to_owned());
    }
    database.diagnostics(FileId(file as u32)).as_ref().clone()
}

fn warnings(source: &str) -> Vec<Diagnostic> {
    diagnostics_for(&[source], 0)
        .into_iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Warning)
        .collect()
}

fn messages_for(source: &str, code: DiagnosticCode) -> Vec<String> {
    warnings(source)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == code)
        .map(|diagnostic| diagnostic.message)
        .collect()
}

#[test]
fn unused_local_and_temporary_variables_warn_with_exact_kind_and_name() {
    let messages = messages_for(
        r#"
PROGRAM Main
VAR
    LocalValue : INT;
END_VAR
VAR_TEMP
    Scratch : INT;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UnusedVariable,
    );

    assert!(messages.contains(&"unused variable 'LocalValue'".to_owned()));
    assert!(messages.contains(&"unused variable 'Scratch'".to_owned()));
}

#[test]
fn used_local_and_temporary_variables_do_not_warn() {
    let messages = messages_for(
        r#"
PROGRAM Main
VAR
    LocalValue : INT;
END_VAR
VAR_TEMP
    Scratch : INT;
END_VAR
Scratch := 1;
LocalValue := Scratch;
END_PROGRAM
"#,
        DiagnosticCode::UnusedVariable,
    );

    assert!(messages
        .iter()
        .all(|message| !message.contains("LocalValue") && !message.contains("Scratch")));
}

#[test]
fn unused_input_warns_but_output_and_inout_parameters_do_not() {
    let messages = messages_for(
        r#"
FUNCTION Operation : INT
VAR_INPUT
    InputValue : INT;
END_VAR
VAR_OUTPUT
    OutputValue : INT;
END_VAR
VAR_IN_OUT
    LinkedValue : INT;
END_VAR
Operation := 0;
END_FUNCTION
"#,
        DiagnosticCode::UnusedParameter,
    );

    assert_eq!(messages, vec!["unused parameter 'InputValue'"]);
}

#[test]
fn interface_method_input_parameter_is_a_prototype_not_an_unused_value() {
    let messages = messages_for(
        r#"
INTERFACE IWorker
METHOD Run
VAR_INPUT
    Command : INT;
END_VAR
END_METHOD
END_INTERFACE
"#,
        DiagnosticCode::UnusedParameter,
    );

    assert!(messages.is_empty());
}

#[test]
fn local_constant_warns_but_global_constant_does_not() {
    let messages = messages_for(
        r#"
VAR_GLOBAL CONSTANT
    GlobalLimit : INT := 10;
END_VAR
PROGRAM Main
VAR CONSTANT
    LocalLimit : INT := 5;
END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UnusedVariable,
    );

    assert_eq!(messages, vec!["unused constant 'LocalLimit'"]);
}

#[test]
fn unused_program_function_and_function_block_each_warn() {
    let messages = messages_for(
        r#"
PROGRAM Idle
END_PROGRAM
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
FUNCTION_BLOCK Controller
END_FUNCTION_BLOCK
"#,
        DiagnosticCode::UnusedPou,
    );

    assert!(messages.contains(&"unused program 'Idle'".to_owned()));
    assert!(messages.contains(&"unused function 'Calculate'".to_owned()));
    assert!(messages.contains(&"unused function block 'Controller'".to_owned()));
}

#[test]
fn function_result_assignment_does_not_make_function_externally_used() {
    let messages = messages_for(
        r#"
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
"#,
        DiagnosticCode::UnusedPou,
    );

    assert_eq!(messages, vec!["unused function 'Calculate'"]);
}

#[test]
fn configuration_program_instance_marks_program_type_used() {
    let messages = messages_for(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM MainInstance WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#,
        DiagnosticCode::UnusedPou,
    );

    assert!(messages
        .iter()
        .all(|message| message != "unused program 'Main'"));
}

#[test]
fn function_call_marks_function_used() {
    let messages = messages_for(
        r#"
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Calculate();
END_PROGRAM
"#,
        DiagnosticCode::UnusedPou,
    );

    assert!(messages
        .iter()
        .all(|message| message != "unused function 'Calculate'"));
}

#[test]
fn function_block_type_reference_marks_function_block_used() {
    let messages = messages_for(
        r#"
FUNCTION_BLOCK Controller
END_FUNCTION_BLOCK
PROGRAM Main
VAR instance : Controller; END_VAR
END_PROGRAM
"#,
        DiagnosticCode::UnusedPou,
    );

    assert!(messages
        .iter()
        .all(|message| message != "unused function block 'Controller'"));
}

#[test]
fn resolved_var_config_path_marks_member_used_across_files() {
    let sources = [
        r#"
CONFIGURATION Conf
RESOURCE R ON CPU
    TASK Fast (PRIORITY := 1);
    PROGRAM MainInstance WITH Fast : Main;
END_RESOURCE
VAR_CONFIG
    MainInstance.InputSignal : BOOL;
END_VAR
END_CONFIGURATION
"#,
        r#"
PROGRAM Main
VAR
    InputSignal : BOOL;
END_VAR
END_PROGRAM
"#,
    ];
    let messages = diagnostics_for(&sources, 1)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedVariable)
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();

    assert!(messages
        .iter()
        .all(|message| message != "unused variable 'InputSignal'"));
}

#[test]
fn project_wide_function_call_marks_declaration_used_in_owner_file() {
    let sources = [
        r#"
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
"#,
        r#"
PROGRAM Main
VAR result : INT; END_VAR
result := Calculate();
END_PROGRAM
"#,
    ];
    let messages = diagnostics_for(&sources, 0)
        .into_iter()
        .filter(|diagnostic| diagnostic.code == DiagnosticCode::UnusedPou)
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();

    assert!(messages
        .iter()
        .all(|message| message != "unused function 'Calculate'"));
}

#[test]
fn members_of_function_blocks_classes_and_interfaces_are_excluded() {
    let diagnostics = warnings(
        r#"
FUNCTION_BLOCK Controller
VAR
    State : INT;
END_VAR
METHOD Step
END_METHOD
END_FUNCTION_BLOCK
CLASS Holder
VAR
    Value : INT;
END_VAR
METHOD Read
END_METHOD
END_CLASS
INTERFACE IWorker
METHOD Run
VAR_INPUT Request : INT; END_VAR
END_METHOD
END_INTERFACE
"#,
    );

    for member in ["State", "Value", "Request"] {
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| !diagnostic.message.contains(member)),
            "member {member} received an unused warning: {diagnostics:?}"
        );
    }
}

#[test]
fn imported_declarations_do_not_receive_duplicate_unused_warnings() {
    let sources = [
        r#"
FUNCTION Calculate : INT
Calculate := 1;
END_FUNCTION
"#,
        "PROGRAM Main END_PROGRAM",
    ];
    let imported_messages = diagnostics_for(&sources, 1)
        .into_iter()
        .filter(|diagnostic| {
            matches!(
                diagnostic.code,
                DiagnosticCode::UnusedPou
                    | DiagnosticCode::UnusedVariable
                    | DiagnosticCode::UnusedParameter
            )
        })
        .map(|diagnostic| diagnostic.message)
        .collect::<Vec<_>>();

    assert!(imported_messages
        .iter()
        .all(|message| !message.contains("Calculate")));
}

#[test]
fn warning_range_selects_the_unused_declaration_name() {
    let source = r#"
PROGRAM Main
VAR
    Forgotten : INT;
END_VAR
END_PROGRAM
"#;
    let warning = warnings(source)
        .into_iter()
        .find(|diagnostic| {
            diagnostic.code == DiagnosticCode::UnusedVariable
                && diagnostic.message.contains("Forgotten")
        })
        .expect("unused variable warning");
    let expected = source.find("Forgotten").expect("declaration name") as u32;

    assert_eq!(u32::from(warning.range.start()), expected);
    assert_eq!(warning.message, "unused variable 'Forgotten'");
}
