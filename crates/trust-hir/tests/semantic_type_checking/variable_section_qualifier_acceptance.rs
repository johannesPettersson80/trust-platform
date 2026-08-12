use crate::common::*;

#[test]
fn qualifier_contract_function_accepts_constant_on_every_owned_storage_section() {
    check_no_errors(
        r#"
VAR_GLOBAL CONSTANT
    SharedConstant : INT := INT#7;
END_VAR

FUNCTION ConstantFunction : INT
VAR_INPUT CONSTANT
    InputValue : INT;
END_VAR
VAR_OUTPUT CONSTANT
    OutputValue : INT;
END_VAR
VAR_IN_OUT CONSTANT
    AliasValue : INT;
END_VAR
VAR_EXTERNAL CONSTANT
    SharedConstant : INT;
END_VAR
VAR CONSTANT
    LocalValue : INT := INT#1;
END_VAR
VAR_TEMP CONSTANT
    TempValue : INT := INT#2;
END_VAR
VAR_STAT CONSTANT
    StaticValue : INT := INT#3;
END_VAR
ConstantFunction := InputValue + AliasValue + SharedConstant + LocalValue + TempValue + StaticValue;
END_FUNCTION

PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_method_accepts_constant_on_every_owned_storage_section() {
    check_no_errors(
        r#"
VAR_GLOBAL CONSTANT
    SharedConstant : INT := INT#7;
END_VAR

CLASS Container
METHOD PUBLIC ConstantMethod : INT
VAR_INPUT CONSTANT
    InputValue : INT;
END_VAR
VAR_OUTPUT CONSTANT
    OutputValue : INT;
END_VAR
VAR_IN_OUT CONSTANT
    AliasValue : INT;
END_VAR
VAR_EXTERNAL CONSTANT
    SharedConstant : INT;
END_VAR
VAR CONSTANT
    LocalValue : INT := INT#1;
END_VAR
VAR_TEMP CONSTANT
    TempValue : INT := INT#2;
END_VAR
VAR_STAT CONSTANT
    StaticValue : INT := INT#3;
END_VAR
ConstantMethod := InputValue + AliasValue + SharedConstant + LocalValue + TempValue + StaticValue;
END_METHOD
END_CLASS

PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_function_block_accepts_constant_on_every_owned_storage_section() {
    check_no_errors(
        r#"
VAR_GLOBAL CONSTANT
    SharedConstant : INT := INT#7;
END_VAR

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
VAR_EXTERNAL CONSTANT
    SharedConstant : INT;
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
}

#[test]
fn qualifier_contract_program_accepts_constant_on_every_owned_storage_section() {
    check_no_errors(
        r#"
VAR_GLOBAL CONSTANT
    SharedConstant : INT := INT#7;
END_VAR

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
VAR_EXTERNAL CONSTANT
    SharedConstant : INT;
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
VAR_GLOBAL CONSTANT
    ProgramConstant : INT := INT#4;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_class_accepts_constant_on_every_owned_storage_section() {
    check_no_errors(
        r#"
VAR_GLOBAL CONSTANT
    SharedConstant : INT := INT#7;
END_VAR

CLASS ConstantClass
VAR CONSTANT
    StoredValue : INT := INT#1;
END_VAR
VAR_STAT CONSTANT
    StaticValue : INT := INT#2;
END_VAR
VAR_EXTERNAL CONSTANT
    SharedConstant : INT;
END_VAR
END_CLASS

PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_function_accepts_retention_only_on_static_extension_storage() {
    check_no_errors(
        r#"
FUNCTION StaticFunction : INT
VAR_STAT RETAIN
    RetainedValue : INT := INT#1;
END_VAR
VAR_STAT NON_RETAIN
    ReinitializedValue : INT := INT#2;
END_VAR
VAR_STAT PERSISTENT
    PersistentValue : INT := INT#3;
END_VAR
StaticFunction := RetainedValue + ReinitializedValue + PersistentValue;
END_FUNCTION

PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_method_accepts_retention_only_on_static_extension_storage() {
    check_no_errors(
        r#"
CLASS Container
METHOD PUBLIC StaticMethod : INT
VAR_STAT RETAIN
    RetainedValue : INT := INT#1;
END_VAR
VAR_STAT NON_RETAIN
    ReinitializedValue : INT := INT#2;
END_VAR
VAR_STAT PERSISTENT
    PersistentValue : INT := INT#3;
END_VAR
StaticMethod := RetainedValue + ReinitializedValue + PersistentValue;
END_METHOD
END_CLASS

PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_function_block_accepts_retention_on_state_owning_sections() {
    check_no_errors(
        r#"
FUNCTION_BLOCK StatefulBlock
VAR_INPUT RETAIN
    RetainedInput : INT;
END_VAR
VAR_INPUT NON_RETAIN
    ReinitializedInput : INT;
END_VAR
VAR_OUTPUT PERSISTENT
    PersistentOutput : INT;
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
}

#[test]
fn qualifier_contract_program_accepts_retention_on_state_owning_sections() {
    check_no_errors(
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
VAR_GLOBAL NON_RETAIN
    ProgramGlobal : INT;
END_VAR
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_class_accepts_retention_on_owned_state() {
    check_no_errors(
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
}

#[test]
fn qualifier_contract_root_globals_accept_every_retention_policy() {
    check_no_errors(
        r#"
VAR_GLOBAL RETAIN
    RetainedGlobal : INT;
END_VAR
VAR_GLOBAL NON_RETAIN
    ReinitializedGlobal : INT;
END_VAR
VAR_GLOBAL PERSISTENT
    PersistentGlobal : INT;
END_VAR

PROGRAM Main
END_PROGRAM
"#,
    );
}

#[test]
fn qualifier_contract_configuration_and_resource_globals_accept_retention_policy() {
    check_no_errors(
        r#"
PROGRAM Main
END_PROGRAM

CONFIGURATION Plant
VAR_GLOBAL RETAIN
    ConfigurationValue : INT;
END_VAR
RESOURCE Line ON PLC
VAR_GLOBAL NON_RETAIN
    ResourceValue : INT;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    );
}
