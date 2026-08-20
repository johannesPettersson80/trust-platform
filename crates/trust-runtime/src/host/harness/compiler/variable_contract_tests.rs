use crate::harness::CompileSession;
use crate::io::IoSize;
use crate::memory::IoArea;
use crate::program_model::{Expr, VarDef};
use crate::value::Value;
use crate::{GlobalInitValue, RetainPolicy, Runtime};
use trust_hir::symbols::ParamDirection;
use trust_hir::{Type, TypeId};

fn runtime(source: &str) -> Runtime {
    CompileSession::from_source(source)
        .build_runtime()
        .unwrap_or_else(|error| panic!("variable fixture must compile: {error}"))
}

fn compile_error(source: &str) -> String {
    match CompileSession::from_source(source).build_runtime() {
        Ok(_) => panic!("variable fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn variable<'a>(vars: &'a [VarDef], name: &str) -> &'a VarDef {
    vars.iter()
        .find(|var| var.name == name)
        .unwrap_or_else(|| panic!("missing variable {name}"))
}

#[test]
fn variable_contract_function_var_and_temp_are_automatic_locals() {
    let runtime = runtime(
        r#"
FUNCTION Observe : INT
VAR
    LocalValue : INT := INT#2;
END_VAR
VAR_TEMP
    TempValue : DINT := DINT#3;
END_VAR
Observe := LocalValue;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "Observe")
        .expect("Observe");
    assert_eq!(
        function
            .locals
            .iter()
            .map(|var| (var.name.as_str(), var.type_id))
            .collect::<Vec<_>>(),
        [("LocalValue", TypeId::INT), ("TempValue", TypeId::DINT)]
    );
    assert!(function
        .locals
        .iter()
        .all(|var| !var.static_storage && !var.external));
}

#[test]
fn variable_contract_function_statics_are_separate_static_storage() {
    let runtime = runtime(
        r#"
FUNCTION Observe : INT
VAR_STAT
    Counter : DINT := DINT#4;
END_VAR
Observe := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "Observe")
        .expect("Observe");
    assert!(function.locals.is_empty());
    let counter = variable(&function.static_locals, "Counter");
    assert_eq!(counter.type_id, TypeId::DINT);
    assert!(counter.static_storage);
    assert!(!counter.external);
    assert!(matches!(
        counter.initializer.as_ref(),
        Some(Expr::Literal(Value::DInt(4)))
    ));
}

#[test]
fn variable_contract_function_constant_chain_is_retained_in_metadata() {
    let runtime = runtime(
        r#"
FUNCTION Limits : INT
VAR CONSTANT
    Base : INT := INT#4;
    Derived : INT := Base + INT#1;
END_VAR
Limits := Derived;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "Limits")
        .expect("Limits");
    assert_eq!(
        function
            .locals
            .iter()
            .map(|var| var.name.as_str())
            .collect::<Vec<_>>(),
        ["Base", "Derived"]
    );
    assert!(function.locals.iter().all(|var| var.constant));
    assert!(function.locals.iter().all(|var| var.initializer.is_some()));
}

#[test]
fn variable_contract_function_multi_name_declaration_preserves_order_and_initializer() {
    let runtime = runtime(
        r#"
FUNCTION Values : INT
VAR
    First, Second, Third : INT := INT#6;
END_VAR
Values := First + Second + Third;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "Values")
        .expect("Values");
    assert_eq!(
        function
            .locals
            .iter()
            .map(|var| var.name.as_str())
            .collect::<Vec<_>>(),
        ["First", "Second", "Third"]
    );
    assert!(function
        .locals
        .iter()
        .all(|var| { matches!(var.initializer.as_ref(), Some(Expr::Literal(Value::Int(6)))) }));
}

#[test]
fn variable_contract_function_external_creates_no_local_record() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
FUNCTION ReadShared : INT
VAR_EXTERNAL
    Shared : INT;
END_VAR
ReadShared := Shared;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "ReadShared")
        .expect("ReadShared");
    assert!(function.locals.is_empty());
    assert!(function.static_locals.is_empty());
    assert!(runtime.globals().contains_key("Shared"));
}

#[test]
fn variable_contract_function_local_direct_address_retains_full_coordinates() {
    let runtime = runtime(
        r#"
FUNCTION ReadPort : INT
VAR
    Port AT %MW3.7.2 : INT;
END_VAR
ReadPort := Port;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "ReadPort")
        .expect("ReadPort");
    let address = variable(&function.locals, "Port")
        .address
        .as_ref()
        .expect("Port address");
    assert_eq!(address.area, IoArea::Memory);
    assert_eq!(address.size, IoSize::Word);
    assert_eq!(address.byte, 3);
    assert_eq!(address.bit, 0);
    assert_eq!(address.path.as_slice(), [3, 7, 2]);
    assert!(!address.wildcard);
}

#[test]
fn variable_contract_rejects_invalid_concrete_address_before_metadata() {
    let error = compile_error(
        r#"
FUNCTION ReadPort : BOOL
VAR
    Port AT %QX1.8 : BOOL;
END_VAR
ReadPort := Port;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("invalid I/O address") || error.contains("%QX1.8"),
        "{error}"
    );
}

#[test]
fn variable_contract_rejects_function_global_section() {
    let error = compile_error(
        r#"
FUNCTION Broken : INT
VAR_GLOBAL
    HiddenGlobal : INT;
END_VAR
Broken := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("VAR_GLOBAL")
            || error.contains("unsupported VAR block")
            || error.contains("not allowed"),
        "{error}"
    );
}

#[test]
fn variable_contract_function_block_splits_instance_and_temporary_storage() {
    let runtime = runtime(
        r#"
FUNCTION_BLOCK Controller
VAR
    State : INT;
END_VAR
VAR_STAT
    Statistic : DINT;
END_VAR
VAR_TEMP
    Scratch : BOOL;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Instance : Controller;
END_VAR
END_PROGRAM
"#,
    );
    let block = runtime
        .function_blocks()
        .values()
        .find(|block| block.name == "Controller")
        .expect("Controller");
    assert_eq!(
        block
            .vars
            .iter()
            .map(|var| (var.name.as_str(), var.type_id))
            .collect::<Vec<_>>(),
        [("State", TypeId::INT), ("Statistic", TypeId::DINT)]
    );
    assert_eq!(
        block
            .temps
            .iter()
            .map(|var| (var.name.as_str(), var.type_id))
            .collect::<Vec<_>>(),
        [("Scratch", TypeId::BOOL)]
    );
    assert!(block
        .vars
        .iter()
        .chain(&block.temps)
        .all(|var| !var.static_storage && !var.external));
}

#[test]
fn variable_contract_function_block_retain_policy_is_preserved() {
    let runtime = runtime(
        r#"
FUNCTION_BLOCK Controller
VAR RETAIN
    Retained : INT;
END_VAR
VAR NON_RETAIN
    Reinitialized : INT;
END_VAR
VAR PERSISTENT
    PersistentValue : INT;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Instance : Controller;
END_VAR
END_PROGRAM
"#,
    );
    let block = runtime
        .function_blocks()
        .values()
        .find(|block| block.name == "Controller")
        .expect("Controller");
    assert_eq!(
        variable(&block.vars, "Retained").retain,
        RetainPolicy::Retain
    );
    assert_eq!(
        variable(&block.vars, "Reinitialized").retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        variable(&block.vars, "PersistentValue").retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn variable_contract_function_block_constant_state_and_initializer_are_preserved() {
    let runtime = runtime(
        r#"
FUNCTION_BLOCK Controller
VAR CONSTANT
    Limit : INT := INT#12;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Instance : Controller;
END_VAR
END_PROGRAM
"#,
    );
    let block = runtime
        .function_blocks()
        .values()
        .find(|block| block.name == "Controller")
        .expect("Controller");
    let limit = variable(&block.vars, "Limit");
    assert!(limit.constant);
    assert!(matches!(
        limit.initializer.as_ref(),
        Some(Expr::Literal(Value::Int(12)))
    ));
}

#[test]
fn variable_contract_function_block_direct_address_retains_hierarchy() {
    let runtime = runtime(
        r#"
FUNCTION_BLOCK Controller
VAR
    OutputWord AT %QW2.5.7 : WORD;
END_VAR
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Instance : Controller;
END_VAR
END_PROGRAM
"#,
    );
    let block = runtime
        .function_blocks()
        .values()
        .find(|block| block.name == "Controller")
        .expect("Controller");
    let address = variable(&block.vars, "OutputWord")
        .address
        .as_ref()
        .expect("OutputWord address");
    assert_eq!(address.area, IoArea::Output);
    assert_eq!(address.size, IoSize::Word);
    assert_eq!(address.path.as_slice(), [2, 5, 7]);
}

#[test]
fn variable_contract_program_sections_preserve_source_order_as_instance_variables() {
    let runtime = runtime(
        r#"
PROGRAM Main
VAR_INPUT
    InputValue : INT;
END_VAR
VAR_OUTPUT
    OutputValue : DINT;
END_VAR
VAR_IN_OUT
    LinkedValue : UINT;
END_VAR
VAR
    LocalValue : BOOL;
END_VAR
VAR_STAT
    StaticValue : LINT;
END_VAR
END_PROGRAM
"#,
    );
    let program = runtime.programs().values().next().expect("Main program");
    assert_eq!(
        program
            .vars
            .iter()
            .map(|var| (var.name.as_str(), var.type_id))
            .collect::<Vec<_>>(),
        [
            ("InputValue", TypeId::INT),
            ("OutputValue", TypeId::DINT),
            ("LinkedValue", TypeId::UINT),
            ("LocalValue", TypeId::BOOL),
            ("StaticValue", TypeId::LINT)
        ]
    );
}

#[test]
fn variable_contract_program_temp_is_kept_out_of_persistent_vars() {
    let runtime = runtime(
        r#"
PROGRAM Main
VAR
    StoredValue : INT;
END_VAR
VAR_TEMP
    CycleValue : DINT;
END_VAR
END_PROGRAM
"#,
    );
    let program = runtime.programs().values().next().expect("Main program");
    assert_eq!(
        program
            .vars
            .iter()
            .map(|var| var.name.as_str())
            .collect::<Vec<_>>(),
        ["StoredValue"]
    );
    assert_eq!(
        program
            .temps
            .iter()
            .map(|var| var.name.as_str())
            .collect::<Vec<_>>(),
        ["CycleValue"]
    );
}

#[test]
fn variable_contract_program_retain_policies_are_preserved_per_section() {
    let runtime = runtime(
        r#"
PROGRAM Main
VAR RETAIN
    Retained : INT;
END_VAR
VAR NON_RETAIN
    Reinitialized : INT;
END_VAR
VAR PERSISTENT
    PersistentValue : INT;
END_VAR
END_PROGRAM
"#,
    );
    let program = runtime.programs().values().next().expect("Main program");
    assert_eq!(
        variable(&program.vars, "Retained").retain,
        RetainPolicy::Retain
    );
    assert_eq!(
        variable(&program.vars, "Reinitialized").retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        variable(&program.vars, "PersistentValue").retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn variable_contract_program_constant_chain_retains_records() {
    let runtime = runtime(
        r#"
PROGRAM Main
VAR CONSTANT
    Base : INT := INT#5;
    Derived : INT := Base * INT#2;
END_VAR
END_PROGRAM
"#,
    );
    let program = runtime.programs().values().next().expect("Main program");
    assert!(variable(&program.vars, "Base").constant);
    assert!(variable(&program.vars, "Derived").constant);
    assert!(variable(&program.vars, "Base").initializer.is_some());
    assert!(variable(&program.vars, "Derived").initializer.is_some());
}

#[test]
fn variable_contract_program_external_creates_no_program_field() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
PROGRAM Main
VAR_EXTERNAL
    Shared : INT;
END_VAR
END_PROGRAM
"#,
    );
    let program = runtime.programs().values().next().expect("Main program");
    assert!(program.vars.is_empty());
    assert!(program.temps.is_empty());
    assert!(runtime.globals().contains_key("Shared"));
}

#[test]
fn variable_contract_program_global_is_lifted_with_retain_metadata() {
    let runtime = runtime(
        r#"
PROGRAM Main
VAR_GLOBAL RETAIN
    ProgramGlobal : DINT := DINT#8;
END_VAR
END_PROGRAM
"#,
    );
    let program = runtime.programs().values().next().expect("Main program");
    assert!(program.vars.iter().all(|var| var.name != "ProgramGlobal"));
    let global = &runtime.globals()["ProgramGlobal"];
    assert_eq!(global.type_id, TypeId::DINT);
    assert_eq!(global.retain, RetainPolicy::Retain);
    assert!(matches!(
        &global.init,
        GlobalInitValue::Value(Value::DInt(8))
    ));
    assert_eq!(
        runtime.storage().get_global("ProgramGlobal"),
        Some(&Value::DInt(8))
    );
}

#[test]
fn variable_contract_namespaced_program_global_uses_qualified_name() {
    let runtime = runtime(
        r#"
NAMESPACE Cell
PROGRAM Main
VAR_GLOBAL
    Shared : INT := INT#3;
END_VAR
END_PROGRAM
END_NAMESPACE
"#,
    );
    assert!(runtime.globals().contains_key("Cell.Shared"));
    assert_eq!(
        runtime.storage().get_global("Cell.Shared"),
        Some(&Value::Int(3))
    );
    assert!(!runtime.globals().contains_key("Shared"));
}

#[test]
fn variable_contract_class_fields_retain_type_constant_and_initializer() {
    let runtime = runtime(
        r#"
CLASS Device
VAR
    State : INT;
END_VAR
VAR CONSTANT
    Limit : DINT := DINT#20;
END_VAR
END_CLASS
PROGRAM Main
VAR
    Instance : Device;
END_VAR
END_PROGRAM
"#,
    );
    let class = runtime
        .classes()
        .values()
        .find(|class| class.name == "Device")
        .expect("Device");
    assert_eq!(variable(&class.vars, "State").type_id, TypeId::INT);
    let limit = variable(&class.vars, "Limit");
    assert_eq!(limit.type_id, TypeId::DINT);
    assert!(limit.constant);
    assert!(matches!(
        limit.initializer.as_ref(),
        Some(Expr::Literal(Value::DInt(20)))
    ));
}

#[test]
fn variable_contract_class_external_creates_no_field() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
CLASS Device
VAR_EXTERNAL
    Shared : INT;
END_VAR
END_CLASS
PROGRAM Main
VAR
    Instance : Device;
END_VAR
END_PROGRAM
"#,
    );
    let class = runtime
        .classes()
        .values()
        .find(|class| class.name == "Device")
        .expect("Device");
    assert!(class.vars.is_empty());
    assert!(runtime.globals().contains_key("Shared"));
}

#[test]
fn variable_contract_rejects_class_temp_section() {
    let error = compile_error(
        r#"
CLASS Device
VAR_TEMP
    Scratch : INT;
END_VAR
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(
        error.contains("VAR_TEMP")
            || error.contains("unsupported VAR block in CLASS")
            || error.contains("not allowed"),
        "{error}"
    );
}

#[test]
fn variable_contract_root_global_retain_policies_are_preserved() {
    let runtime = runtime(
        r#"
VAR_GLOBAL RETAIN
    Retained : INT := INT#1;
END_VAR
VAR_GLOBAL NON_RETAIN
    Reinitialized : INT := INT#2;
END_VAR
VAR_GLOBAL PERSISTENT
    PersistentValue : INT := INT#3;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_eq!(runtime.globals()["Retained"].retain, RetainPolicy::Retain);
    assert_eq!(
        runtime.globals()["Reinitialized"].retain,
        RetainPolicy::NonRetain
    );
    assert_eq!(
        runtime.globals()["PersistentValue"].retain,
        RetainPolicy::Persistent
    );
}

#[test]
fn variable_contract_root_global_multi_name_declaration_initializes_each_name() {
    let runtime = runtime(
        r#"
VAR_GLOBAL
    First, Second, Third : INT := INT#7;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
    for name in ["First", "Second", "Third"] {
        assert_eq!(runtime.storage().get_global(name), Some(&Value::Int(7)));
        assert_eq!(runtime.globals()[name].type_id, TypeId::INT);
    }
}

#[test]
fn variable_contract_root_global_constant_chain_is_evaluated_in_order() {
    let runtime = runtime(
        r#"
VAR_GLOBAL CONSTANT
    Base : INT := INT#4;
    Derived : INT := Base + INT#3;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    );
    assert_eq!(runtime.storage().get_global("Base"), Some(&Value::Int(4)));
    assert_eq!(
        runtime.storage().get_global("Derived"),
        Some(&Value::Int(7))
    );
}

#[test]
fn variable_contract_namespace_global_retains_qualified_identity() {
    let runtime = runtime(
        r#"
NAMESPACE Cell
VAR_GLOBAL
    Counter : DINT := DINT#9;
END_VAR
END_NAMESPACE
PROGRAM Main
END_PROGRAM
"#,
    );
    assert!(runtime.globals().contains_key("Cell.Counter"));
    assert_eq!(
        runtime.storage().get_global("Cell.Counter"),
        Some(&Value::DInt(9))
    );
}

#[test]
fn variable_contract_parameter_addresses_retain_area_size_and_bit_path() {
    let runtime = runtime(
        r#"
FUNCTION Ports : BOOL
VAR_INPUT
    InputBit AT %IX2.3 : BOOL;
END_VAR
VAR_OUTPUT
    OutputWord AT %QW5.7.1 : WORD;
END_VAR
VAR_IN_OUT
    MemoryLong AT %ML4 : LWORD;
END_VAR
Ports := InputBit;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    );
    let function = runtime
        .functions()
        .values()
        .find(|function| function.name == "Ports")
        .expect("Ports");
    assert_eq!(
        function
            .params
            .iter()
            .map(|param| param.direction)
            .collect::<Vec<_>>(),
        [
            ParamDirection::In,
            ParamDirection::Out,
            ParamDirection::InOut
        ]
    );
    let input = function.params[0].address.as_ref().expect("InputBit");
    assert_eq!(input.area, IoArea::Input);
    assert_eq!(input.size, IoSize::Bit);
    assert_eq!((input.byte, input.bit), (2, 3));
    assert_eq!(input.path.as_slice(), [2]);

    let output = function.params[1].address.as_ref().expect("OutputWord");
    assert_eq!(output.area, IoArea::Output);
    assert_eq!(output.size, IoSize::Word);
    assert_eq!(output.path.as_slice(), [5, 7, 1]);

    let memory = function.params[2].address.as_ref().expect("MemoryLong");
    assert_eq!(memory.area, IoArea::Memory);
    assert_eq!(memory.size, IoSize::LWord);
    assert_eq!(memory.path.as_slice(), [4]);
}

#[test]
fn variable_contract_user_alias_identity_is_retained_on_variable_record() {
    let runtime = runtime(
        r#"
TYPE BatchCode : DINT;
END_TYPE
PROGRAM Main
VAR
    Code : BatchCode;
END_VAR
END_PROGRAM
"#,
    );
    let alias = runtime.registry().lookup("BatchCode").expect("BatchCode");
    assert!(matches!(
        runtime.registry().get(alias),
        Some(Type::Alias {
            target: TypeId::DINT,
            ..
        })
    ));
    let program = runtime.programs().values().next().expect("Main program");
    assert_eq!(variable(&program.vars, "Code").type_id, alias);
}

#[test]
fn variable_contract_enum_initializer_is_resolved_to_canonical_value() {
    let runtime = runtime(
        r#"
TYPE Phase : (Idle, Running);
END_TYPE
PROGRAM Main
VAR
    Current : Phase := Running;
END_VAR
END_PROGRAM
"#,
    );
    let program = runtime.programs().values().next().expect("Main program");
    assert!(matches!(
        variable(&program.vars, "Current").initializer.as_ref(),
        Some(Expr::Literal(Value::Enum(value)))
            if value.type_name() == "Phase"
                && value.variant_name() == "Running"
                && value.numeric_value() == 1
    ));
}
