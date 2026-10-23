use crate::harness::CompileSession;
use crate::program_model::VarDef;
use crate::{RetainPolicy, Runtime};

fn runtime(source: &str) -> Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .unwrap_or_else(|error| panic!("variable-qualifier fixture must compile: {error}"))
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("invalid variable-qualifier fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn variable<'a>(variables: &'a [VarDef], name: &str) -> &'a VarDef {
    variables
        .iter()
        .find(|variable| variable.name == name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
}

#[test]
fn qualifier_projection_function_preserves_constant_storage_metadata() {
    let runtime = runtime(
        r#"
FUNCTION ConstantFunction : INT
VAR CONSTANT
    LocalValue : INT := INT#1;
END_VAR
VAR_TEMP CONSTANT
    TempValue : INT := INT#2;
END_VAR
VAR_STAT CONSTANT
    StaticValue : INT := INT#3;
END_VAR
ConstantFunction := LocalValue + TempValue + StaticValue;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );

    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "ConstantFunction")
        .expect("ConstantFunction");
    assert!(variable(&function.locals, "LocalValue").constant);
    assert!(variable(&function.locals, "TempValue").constant);
    assert!(variable(&function.static_locals, "StaticValue").constant);
}

#[test]
fn qualifier_projection_method_preserves_constant_storage_metadata() {
    let runtime = runtime(
        r#"
CLASS Container
METHOD PUBLIC ConstantMethod : INT
VAR CONSTANT
    LocalValue : INT := INT#1;
END_VAR
VAR_TEMP CONSTANT
    TempValue : INT := INT#2;
END_VAR
VAR_STAT CONSTANT
    StaticValue : INT := INT#3;
END_VAR
ConstantMethod := LocalValue + TempValue + StaticValue;
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
        .find(|method| method.name == "ConstantMethod")
        .expect("ConstantMethod");
    assert!(variable(&method.locals, "LocalValue").constant);
    assert!(variable(&method.locals, "TempValue").constant);
    assert!(variable(&method.static_locals, "StaticValue").constant);
}

#[test]
fn qualifier_projection_function_block_preserves_constant_storage_metadata() {
    let runtime = runtime(
        r#"
FUNCTION_BLOCK ConstantBlock
VAR_INPUT CONSTANT
    InputValue : INT;
END_VAR
VAR_OUTPUT CONSTANT
    OutputValue : INT;
END_VAR
VAR_IN_OUT CONSTANT
    AliasValue : INT;
END_VAR
VAR CONSTANT
    StoredValue : INT := INT#1;
END_VAR
VAR_TEMP CONSTANT
    TempValue : INT := INT#2;
END_VAR
VAR_STAT CONSTANT
    StaticValue : INT := INT#3;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
    );

    let block = runtime
        .function_blocks()
        .values()
        .find(|block| block.name == "ConstantBlock")
        .expect("ConstantBlock");
    assert_eq!(block.params.len(), 3);
    assert!(variable(&block.vars, "StoredValue").constant);
    assert!(variable(&block.vars, "StaticValue").constant);
    assert!(variable(&block.temps, "TempValue").constant);
}

#[test]
fn qualifier_projection_program_preserves_constant_storage_metadata() {
    let runtime = runtime(
        r#"
PROGRAM Main
VAR_INPUT CONSTANT
    InputValue : INT;
END_VAR
VAR_OUTPUT CONSTANT
    OutputValue : INT;
END_VAR
VAR_IN_OUT CONSTANT
    AliasValue : INT;
END_VAR
VAR CONSTANT
    StoredValue : INT := INT#1;
END_VAR
VAR_TEMP CONSTANT
    TempValue : INT := INT#2;
END_VAR
VAR_STAT CONSTANT
    StaticValue : INT := INT#3;
END_VAR
END_PROGRAM
"#,
    );

    let program = runtime.programs().values().next().expect("Main");
    for name in [
        "InputValue",
        "OutputValue",
        "AliasValue",
        "StoredValue",
        "StaticValue",
    ] {
        assert!(variable(&program.vars, name).constant, "{name}");
    }
    assert!(variable(&program.temps, "TempValue").constant);
}

#[test]
fn qualifier_projection_class_preserves_constant_storage_metadata() {
    let runtime = runtime(
        r#"
CLASS ConstantClass
VAR CONSTANT
    StoredValue : INT := INT#1;
END_VAR
VAR_STAT CONSTANT
    StaticValue : INT := INT#2;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
    );

    let class = runtime
        .classes()
        .values()
        .find(|class| class.name == "ConstantClass")
        .expect("ConstantClass");
    assert!(variable(&class.vars, "StoredValue").constant);
    assert!(variable(&class.vars, "StaticValue").constant);
}

#[test]
fn qualifier_projection_function_static_storage_preserves_all_retention_policies() {
    let runtime = runtime(
        r#"
FUNCTION StatefulFunction : INT
VAR_STAT RETAIN
    RetainedValue : INT;
END_VAR
VAR_STAT NON_RETAIN
    ReinitializedValue : INT;
END_VAR
VAR_STAT PERSISTENT
    PersistentValue : INT;
END_VAR
StatefulFunction := RetainedValue + ReinitializedValue + PersistentValue;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );

    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "StatefulFunction")
        .expect("StatefulFunction");
    assert_eq!(
        variable(&function.static_locals, "RetainedValue").retain,
        RetainPolicy::Retain
    );
    assert_eq!(
        variable(&function.static_locals, "ReinitializedValue").retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        variable(&function.static_locals, "PersistentValue").retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn qualifier_projection_method_static_storage_preserves_all_retention_policies() {
    let runtime = runtime(
        r#"
CLASS Container
METHOD PUBLIC StatefulMethod : INT
VAR_STAT RETAIN
    RetainedValue : INT;
END_VAR
VAR_STAT NON_RETAIN
    ReinitializedValue : INT;
END_VAR
VAR_STAT PERSISTENT
    PersistentValue : INT;
END_VAR
StatefulMethod := RetainedValue + ReinitializedValue + PersistentValue;
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
        .find(|method| method.name == "StatefulMethod")
        .expect("StatefulMethod");
    assert_eq!(
        variable(&method.static_locals, "RetainedValue").retain,
        RetainPolicy::Retain
    );
    assert_eq!(
        variable(&method.static_locals, "ReinitializedValue").retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        variable(&method.static_locals, "PersistentValue").retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn qualifier_projection_function_block_state_preserves_retention_policies() {
    let runtime = runtime(
        r#"
FUNCTION_BLOCK StatefulBlock
VAR_INPUT RETAIN
    RetainedInput : INT;
END_VAR
VAR_OUTPUT NON_RETAIN
    ReinitializedOutput : INT;
END_VAR
VAR RETAIN
    RetainedValue : INT;
END_VAR
VAR NON_RETAIN
    ReinitializedValue : INT;
END_VAR
VAR PERSISTENT
    PersistentValue : INT;
END_VAR
VAR_STAT RETAIN
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
        .find(|block| block.name == "StatefulBlock")
        .expect("StatefulBlock");
    assert_eq!(block.params.len(), 2);
    assert_eq!(
        variable(&block.vars, "RetainedValue").retain,
        RetainPolicy::Retain
    );
    assert_eq!(
        variable(&block.vars, "ReinitializedValue").retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        variable(&block.vars, "PersistentValue").retain,
        RetainPolicy::Persistent
    );
    assert_eq!(
        variable(&block.vars, "StaticValue").retain,
        RetainPolicy::Retain
    );
}

#[test]
fn qualifier_projection_program_state_preserves_retention_policies() {
    let runtime = runtime(
        r#"
PROGRAM Main
VAR_INPUT RETAIN
    RetainedInput : INT;
END_VAR
VAR_OUTPUT NON_RETAIN
    ReinitializedOutput : INT;
END_VAR
VAR PERSISTENT
    PersistentValue : INT;
END_VAR
VAR_STAT RETAIN
    StaticValue : INT;
END_VAR
END_PROGRAM
"#,
    );

    let program = runtime.programs().values().next().expect("Main");
    assert_eq!(
        variable(&program.vars, "RetainedInput").retain,
        RetainPolicy::Retain
    );
    assert_eq!(
        variable(&program.vars, "ReinitializedOutput").retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        variable(&program.vars, "PersistentValue").retain,
        RetainPolicy::Persistent
    );
    assert_eq!(
        variable(&program.vars, "StaticValue").retain,
        RetainPolicy::Retain
    );
}

#[test]
fn qualifier_projection_class_state_preserves_retention_policies() {
    let runtime = runtime(
        r#"
CLASS StatefulClass
VAR RETAIN
    RetainedValue : INT;
END_VAR
VAR NON_RETAIN
    ReinitializedValue : INT;
END_VAR
VAR PERSISTENT
    PersistentValue : INT;
END_VAR
VAR_STAT RETAIN
    StaticValue : INT;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
    );

    let class = runtime
        .classes()
        .values()
        .find(|class| class.name == "StatefulClass")
        .expect("StatefulClass");
    assert_eq!(
        variable(&class.vars, "RetainedValue").retain,
        RetainPolicy::Retain
    );
    assert_eq!(
        variable(&class.vars, "ReinitializedValue").retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        variable(&class.vars, "PersistentValue").retain,
        RetainPolicy::Persistent
    );
    assert_eq!(
        variable(&class.vars, "StaticValue").retain,
        RetainPolicy::Retain
    );
}

#[test]
fn qualifier_projection_configuration_and_resource_globals_preserve_retention_policies() {
    let runtime = runtime(
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION Plant
VAR_GLOBAL RETAIN
    ConfigurationValue : INT;
END_VAR
RESOURCE Line ON PLC
VAR_GLOBAL PERSISTENT
    ResourceValue : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );

    assert_eq!(
        runtime.globals()["ConfigurationValue"].retain,
        RetainPolicy::Retain
    );
    assert_eq!(
        runtime.globals()["ResourceValue"].retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn qualifier_projection_duplicate_modifier_stops_runtime_assembly() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR RETAIN RETAIN
    Value : INT;
END_VAR
END_PROGRAM
"#,
    );
    assert!(!error.trim().is_empty(), "{error}");
}

#[test]
fn qualifier_projection_conflicting_modifiers_stop_runtime_assembly() {
    let error = compile_error(
        r#"
PROGRAM Main
VAR CONSTANT PERSISTENT
    Value : INT := INT#1;
END_VAR
END_PROGRAM
"#,
    );
    assert!(!error.trim().is_empty(), "{error}");
}

#[test]
fn qualifier_projection_call_local_retention_stops_runtime_assembly() {
    let error = compile_error(
        r#"
FUNCTION InvalidFunction : INT
VAR RETAIN
    Value : INT;
END_VAR
InvalidFunction := Value;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(!error.trim().is_empty(), "{error}");
}

#[test]
fn qualifier_projection_alias_retention_stops_runtime_assembly() {
    let error = compile_error(
        r#"
VAR_GLOBAL
    SharedValue : INT;
END_VAR
PROGRAM Main
VAR_EXTERNAL PERSISTENT
    SharedValue : INT;
END_VAR
END_PROGRAM
"#,
    );
    assert!(!error.trim().is_empty(), "{error}");
}

// IEC_DECISIONS 2026-10-11: `RETAIN PERSISTENT` is the accepted
// WAGO/CODESYS vendor extension — persistent storage implies retention, so
// the combined section keeps the Persistent policy (either spelling order).
#[test]
fn qualifier_projection_global_state_accepts_retain_persistent_vendor_extension() {
    let runtime = runtime(
        r#"
VAR_GLOBAL RETAIN PERSISTENT
    BootState : INT;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_eq!(
        runtime.globals()["BootState"].retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn qualifier_projection_global_state_accepts_persistent_retain_order() {
    let runtime = runtime(
        r#"
VAR_GLOBAL PERSISTENT RETAIN
    BootState : INT;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_eq!(
        runtime.globals()["BootState"].retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn qualifier_projection_global_state_rejects_every_other_qualifier_combination() {
    // Only the RETAIN PERSISTENT pair passes. Which layer rejects the rest
    // (front-end E202 or the harness qualifier rule) is an implementation
    // detail; the contract is that nothing else compiles.
    for (fixture, label) in [
        (
            "VAR_GLOBAL RETAIN NON_RETAIN\n    x : INT;\nEND_VAR\n",
            "RETAIN NON_RETAIN",
        ),
        (
            "VAR_GLOBAL NON_RETAIN PERSISTENT\n    x : INT;\nEND_VAR\n",
            "NON_RETAIN PERSISTENT",
        ),
        (
            "VAR_GLOBAL RETAIN RETAIN\n    x : INT;\nEND_VAR\n",
            "duplicate",
        ),
        (
            "VAR_GLOBAL RETAIN CONSTANT\n    x : INT;\nEND_VAR\n",
            "RETAIN CONSTANT",
        ),
        (
            "VAR_GLOBAL RETAIN PERSISTENT CONSTANT\n    x : INT;\nEND_VAR\n",
            "pair plus CONSTANT",
        ),
    ] {
        let error = compile_error(&format!("{fixture}\nPROGRAM Main\nEND_PROGRAM\n"));
        assert!(!error.trim().is_empty(), "{label} must still be rejected");
    }
}
