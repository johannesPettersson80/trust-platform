use crate::harness::CompileSession;
use crate::value::Value;
use crate::Runtime;

fn runtime(source: &str) -> Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .unwrap_or_else(|error| panic!("variable-section fixture must compile: {error}"))
}

#[test]
fn pou_variable_section_function_accepts_complete_owned_section_set() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
FUNCTION CompleteFunction : INT
VAR_INPUT
    InputValue : INT;
END_VAR
VAR_OUTPUT
    OutputValue : INT;
END_VAR
VAR_IN_OUT
    InOutValue : INT;
END_VAR
VAR_EXTERNAL
    Shared : INT;
END_VAR
VAR
    LocalValue : INT;
END_VAR
VAR_TEMP
    TempValue : INT;
END_VAR
VAR_STAT
    StaticValue : INT;
END_VAR
CompleteFunction := InputValue + LocalValue + TempValue + StaticValue + Shared;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );

    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "CompleteFunction")
        .expect("CompleteFunction");
    assert_eq!(function.params.len(), 3);
    assert_eq!(function.locals.len(), 2);
    assert_eq!(function.static_locals.len(), 1);
}

#[test]
fn pou_variable_section_method_accepts_complete_owned_section_set() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
CLASS Container
METHOD PUBLIC CompleteMethod : INT
VAR_INPUT
    InputValue : INT;
END_VAR
VAR_OUTPUT
    OutputValue : INT;
END_VAR
VAR_IN_OUT
    InOutValue : INT;
END_VAR
VAR_EXTERNAL
    Shared : INT;
END_VAR
VAR
    LocalValue : INT;
END_VAR
VAR_TEMP
    TempValue : INT;
END_VAR
VAR_STAT
    StaticValue : INT;
END_VAR
CompleteMethod := InputValue + LocalValue + TempValue + StaticValue + Shared;
END_METHOD
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
    );

    let class = runtime
        .classes()
        .values()
        .find(|class| class.name == "Container")
        .expect("Container");
    let method = class
        .methods
        .iter()
        .find(|method| method.name == "CompleteMethod")
        .expect("CompleteMethod");
    assert_eq!(method.params.len(), 3);
    assert_eq!(method.locals.len(), 2);
    assert_eq!(method.static_locals.len(), 1);
}

#[test]
fn pou_variable_section_function_block_accepts_complete_owned_section_set() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
FUNCTION_BLOCK CompleteBlock
VAR_INPUT
    InputValue : INT;
END_VAR
VAR_OUTPUT
    OutputValue : INT;
END_VAR
VAR_IN_OUT
    InOutValue : INT;
END_VAR
VAR_EXTERNAL
    Shared : INT;
END_VAR
VAR
    StoredValue : INT;
END_VAR
VAR_TEMP
    TempValue : INT;
END_VAR
VAR_STAT
    StaticValue : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );

    let block = runtime
        .function_blocks()
        .values()
        .find(|block| block.name == "CompleteBlock")
        .expect("CompleteBlock");
    assert_eq!(block.params.len(), 3);
    assert_eq!(block.vars.len(), 2);
    assert_eq!(block.temps.len(), 1);
}

#[test]
fn pou_variable_section_program_accepts_complete_owned_section_set() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
PROGRAM Main
VAR_INPUT
    InputValue : INT;
END_VAR
VAR_OUTPUT
    OutputValue : INT;
END_VAR
VAR_IN_OUT
    InOutValue : INT;
END_VAR
VAR_EXTERNAL
    Shared : INT;
END_VAR
VAR
    StoredValue : INT;
END_VAR
VAR_TEMP
    TempValue : INT;
END_VAR
VAR_STAT
    StaticValue : INT;
END_VAR
VAR_GLOBAL
    ProgramGlobal : INT;
END_VAR
END_PROGRAM
"#,
    );

    let program = runtime
        .programs()
        .values()
        .find(|program| program.name == "Main")
        .expect("Main");
    assert_eq!(program.vars.len(), 5);
    assert_eq!(program.temps.len(), 1);
    assert!(runtime.globals().contains_key("ProgramGlobal"));
}

#[test]
fn pou_variable_section_class_accepts_var_external_and_static_extension() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
CLASS Container
VAR
    StoredValue : INT;
END_VAR
VAR_STAT
    StaticValue : INT;
END_VAR
VAR_EXTERNAL
    Shared : INT;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
    );

    let class = runtime
        .classes()
        .values()
        .find(|class| class.name == "Container")
        .expect("Container");
    assert_eq!(
        class
            .vars
            .iter()
            .map(|variable| variable.name.as_str())
            .collect::<Vec<_>>(),
        ["StoredValue", "StaticValue"]
    );
}

#[test]
fn pou_variable_section_program_accepts_local_var_access_declaration() {
    let mut runtime = runtime(
        r#"
PROGRAM Main
VAR
    StoredValue : INT;
END_VAR
VAR_ACCESS
    PublicValue : StoredValue : INT READ_WRITE;
END_VAR
END_PROGRAM
"#,
    );

    assert_eq!(runtime.read_access("PublicValue"), Some(Value::Int(0)));
    runtime
        .write_access("PublicValue", Value::Int(9))
        .expect("READ_WRITE program access must update its owned variable");
    assert_eq!(runtime.read_access("PublicValue"), Some(Value::Int(9)));
}

#[test]
fn pou_variable_section_interface_method_prototype_accepts_parameter_sections() {
    let runtime = runtime(
        r#"
INTERFACE ITransform
METHOD Apply : INT
VAR_INPUT
    InputValue : INT;
END_VAR
VAR_OUTPUT
    OutputValue : INT;
END_VAR
VAR_IN_OUT
    InOutValue : INT;
END_VAR
END_METHOD
END_INTERFACE
PROGRAM Main
END_PROGRAM
"#,
    );

    let interface = runtime
        .interfaces()
        .values()
        .find(|interface| interface.name == "ITransform")
        .expect("ITransform");
    let method = interface
        .methods
        .iter()
        .find(|method| method.name == "Apply")
        .expect("Apply");
    assert_eq!(method.params.len(), 3);
}
