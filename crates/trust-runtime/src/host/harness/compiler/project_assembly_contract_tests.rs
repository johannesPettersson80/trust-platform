use crate::harness::{CompileSession, SourceFile, TestHarness};
use crate::program_model::{Expr, FunctionBlockBase};
use crate::value::Value;
use crate::Runtime;
use trust_hir::{Type, TypeId};

fn source_files(files: &[(&str, &str)]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|(path, source)| SourceFile::with_path(*path, *source))
        .collect()
}

fn runtime(files: &[(&str, &str)]) -> Runtime {
    CompileSession::from_sources(source_files(files))
        .build_runtime()
        .unwrap_or_else(|error| panic!("project fixture must compile: {error}"))
}

fn compile_error(files: &[(&str, &str)]) -> String {
    match CompileSession::from_sources(source_files(files)).build_runtime() {
        Ok(_) => panic!("project fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn type_id(runtime: &Runtime, name: &str) -> TypeId {
    runtime
        .registry()
        .lookup(name)
        .unwrap_or_else(|| panic!("missing type {name}"))
}

#[test]
fn project_assembly_contract_resolves_later_alias_in_earlier_alias() {
    let runtime = runtime(&[
        (
            "consumer.st",
            r#"
TYPE BatchValue : RawValue;
END_TYPE
PROGRAM Main
VAR
    Value : BatchValue;
END_VAR
END_PROGRAM
"#,
        ),
        (
            "types.st",
            r#"
TYPE RawValue : DINT;
END_TYPE
"#,
        ),
    ]);
    let raw = type_id(&runtime, "RawValue");
    assert!(matches!(
        runtime.registry().get(type_id(&runtime, "BatchValue")),
        Some(Type::Alias { target, .. }) if *target == raw
    ));
    assert!(matches!(
        runtime.registry().get(raw),
        Some(Type::Alias {
            target: TypeId::DINT,
            ..
        })
    ));
}

#[test]
fn project_assembly_contract_resolves_later_type_in_earlier_structure() {
    let runtime = runtime(&[
        (
            "packet.st",
            r#"
TYPE Packet : STRUCT
    Code : CodeValue;
END_STRUCT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
        ),
        (
            "code.st",
            r#"
TYPE CodeValue : UINT;
END_TYPE
"#,
        ),
    ]);
    let code = type_id(&runtime, "CodeValue");
    let packet = runtime
        .registry()
        .get(type_id(&runtime, "Packet"))
        .expect("Packet type");
    assert!(matches!(
        packet,
        Type::Struct { fields, .. }
            if fields.len() == 1
                && fields[0].name == "Code"
                && fields[0].type_id == code
    ));
}

#[test]
fn project_assembly_contract_resolves_later_array_element_type() {
    let runtime = runtime(&[
        (
            "buffer.st",
            r#"
TYPE CodeBuffer : ARRAY[1..4] OF CodeValue;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
        ),
        (
            "code.st",
            r#"
TYPE CodeValue : WORD;
END_TYPE
"#,
        ),
    ]);
    let code = type_id(&runtime, "CodeValue");
    let array = match runtime
        .registry()
        .get(type_id(&runtime, "CodeBuffer"))
        .expect("CodeBuffer")
    {
        Type::Alias { target, .. } => runtime.registry().get(*target).expect("array target"),
        other => panic!("expected alias, got {other:?}"),
    };
    assert!(matches!(
        array,
        Type::Array {
            element,
            dimensions
        } if *element == code && dimensions.as_slice() == [(1, 4)]
    ));
}

#[test]
fn project_assembly_contract_resolves_later_enum_in_program_initializer() {
    let runtime = runtime(&[
        (
            "main.st",
            r#"
PROGRAM Main
VAR
    Current : Phase := Running;
END_VAR
END_PROGRAM
"#,
        ),
        (
            "phase.st",
            r#"
TYPE Phase : (Idle, Running);
END_TYPE
"#,
        ),
    ]);
    let program = runtime.programs().values().next().expect("Main program");
    let current = program
        .vars
        .iter()
        .find(|var| var.name == "Current")
        .expect("Current");
    assert_eq!(current.type_id, type_id(&runtime, "Phase"));
    assert!(matches!(
        current.initializer.as_ref(),
        Some(Expr::Literal(Value::Enum(value)))
            if value.type_name() == "Phase"
                && value.variant_name() == "Running"
                && value.numeric_value() == 1
    ));
}

#[test]
fn project_assembly_contract_resolves_later_function_block_type_in_program() {
    let runtime = runtime(&[
        (
            "main.st",
            r#"
PROGRAM Main
VAR
    WorkerInstance : Worker;
END_VAR
END_PROGRAM
"#,
        ),
        (
            "worker.st",
            r#"
FUNCTION_BLOCK Worker
VAR
    Count : INT;
END_VAR
END_FUNCTION_BLOCK
"#,
        ),
    ]);
    let worker = type_id(&runtime, "Worker");
    let program = runtime.programs().values().next().expect("Main program");
    assert_eq!(program.vars[0].type_id, worker);
    assert!(runtime
        .function_blocks()
        .values()
        .any(|block| block.name == "Worker"));
}

#[test]
fn project_assembly_contract_resolves_later_interface_for_earlier_class() {
    let runtime = runtime(&[
        (
            "device.st",
            r#"
CLASS Device IMPLEMENTS IDevice
METHOD PUBLIC Read : INT
Read := INT#4;
END_METHOD
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        ),
        (
            "interface.st",
            r#"
INTERFACE IDevice
METHOD Read : INT
END_METHOD
END_INTERFACE
"#,
        ),
    ]);
    let class = runtime
        .classes()
        .values()
        .find(|class| class.name == "Device")
        .expect("Device");
    assert!(class.methods.iter().any(|method| method.name == "Read"));
    assert!(runtime
        .interfaces()
        .values()
        .any(|interface| interface.name == "IDevice"));
}

#[test]
fn project_assembly_contract_resolves_later_class_as_function_block_base() {
    let runtime = runtime(&[
        (
            "controller.st",
            r#"
FUNCTION_BLOCK Controller EXTENDS Device
END_FUNCTION_BLOCK
PROGRAM Main
END_PROGRAM
"#,
        ),
        (
            "device.st",
            r#"
CLASS Device
VAR
    State : INT;
END_VAR
END_CLASS
"#,
        ),
    ]);
    let block = runtime
        .function_blocks()
        .values()
        .find(|block| block.name == "Controller")
        .expect("Controller");
    assert!(matches!(
        block.base.as_ref(),
        Some(FunctionBlockBase::Class(name)) if name == "Device"
    ));
}

#[test]
fn project_assembly_contract_executes_later_function_called_by_earlier_program() {
    let mut harness = TestHarness::from_sources(&[
        r#"
PROGRAM Main
VAR
    ResultValue : INT;
END_VAR
ResultValue := AddOne(INT#8);
END_PROGRAM
"#,
        r#"
FUNCTION AddOne : INT
VAR_INPUT
    Value : INT;
END_VAR
AddOne := Value + INT#1;
END_FUNCTION
"#,
    ])
    .expect("cross-file function call");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(9)));
}

#[test]
fn project_assembly_contract_using_resolves_later_namespaced_function() {
    let mut harness = TestHarness::from_sources(&[
        r#"
USING MathSupport;
PROGRAM Main
VAR
    ResultValue : INT;
END_VAR
ResultValue := Identity(INT#11);
END_PROGRAM
"#,
        r#"
NAMESPACE MathSupport
FUNCTION Identity : INT
VAR_INPUT
    Value : INT;
END_VAR
Identity := Value;
END_FUNCTION
END_NAMESPACE
"#,
    ])
    .expect("cross-file USING function");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(11)));
}

#[test]
fn project_assembly_contract_using_resolves_later_namespaced_type() {
    let runtime = runtime(&[
        (
            "main.st",
            r#"
USING SharedTypes;
PROGRAM Main
VAR
    Value : CounterValue;
END_VAR
END_PROGRAM
"#,
        ),
        (
            "types.st",
            r#"
NAMESPACE SharedTypes
TYPE CounterValue : DINT;
END_TYPE
END_NAMESPACE
"#,
        ),
    ]);
    let counter = type_id(&runtime, "SharedTypes.CounterValue");
    let program = runtime.programs().values().next().expect("Main program");
    assert_eq!(program.vars[0].type_id, counter);
}

#[test]
fn project_assembly_contract_configuration_precedes_qualified_program_type() {
    let runtime = runtime(&[
        (
            "configuration.st",
            r#"
CONFIGURATION CellConfiguration
RESOURCE CellResource ON PLC
TASK MainTask (INTERVAL := T#10ms, PRIORITY := 1);
PROGRAM CellProgram WITH MainTask : Cell.Main;
END_RESOURCE
END_CONFIGURATION
"#,
        ),
        (
            "program.st",
            r#"
NAMESPACE Cell
PROGRAM Main
VAR
    Count : INT;
END_VAR
END_PROGRAM
END_NAMESPACE
"#,
        ),
    ]);
    assert_eq!(runtime.resource_name(), "CellResource");
    assert!(runtime
        .programs()
        .values()
        .any(|program| program.name == "CellProgram"));
    assert_eq!(runtime.tasks()[0].programs.as_slice(), ["CellProgram"]);
}

#[test]
fn project_assembly_contract_later_global_is_visible_to_earlier_program() {
    let mut harness = TestHarness::from_sources(&[
        r#"
PROGRAM Main
VAR
    Observed : INT;
END_VAR
Observed := SharedValue;
END_PROGRAM
"#,
        r#"
VAR_GLOBAL
    SharedValue : INT := INT#17;
END_VAR
"#,
    ])
    .expect("later global");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(17)));
}

#[test]
fn project_assembly_contract_source_permutation_preserves_declaration_identities() {
    const TYPE_SOURCE: &str = r#"
NAMESPACE Cell
TYPE CountValue : DINT;
END_TYPE
END_NAMESPACE
"#;
    const FUNCTION_SOURCE: &str = r#"
NAMESPACE Cell
FUNCTION Identity : DINT
VAR_INPUT
    Value : DINT;
END_VAR
Identity := Value;
END_FUNCTION
END_NAMESPACE
"#;
    const PROGRAM_SOURCE: &str = r#"
USING Cell;
PROGRAM Main
VAR
    Count : CountValue;
END_VAR
Count := Identity(DINT#3);
END_PROGRAM
"#;
    let first = runtime(&[
        ("types.st", TYPE_SOURCE),
        ("function.st", FUNCTION_SOURCE),
        ("program.st", PROGRAM_SOURCE),
    ]);
    let second = runtime(&[
        ("program.st", PROGRAM_SOURCE),
        ("types.st", TYPE_SOURCE),
        ("function.st", FUNCTION_SOURCE),
    ]);
    for runtime in [&first, &second] {
        assert!(runtime.registry().lookup("Cell.CountValue").is_some());
        assert!(runtime
            .functions()
            .values()
            .any(|function| function.name == "Cell.Identity"));
        assert!(runtime
            .programs()
            .values()
            .any(|program| program.name == "Main"));
    }
}

#[test]
fn project_assembly_contract_paths_do_not_change_semantic_identity() {
    const TYPE_SOURCE: &str = r#"
TYPE CounterValue : DINT;
END_TYPE
"#;
    const PROGRAM_SOURCE: &str = r#"
PROGRAM Main
VAR
    Count : CounterValue;
END_VAR
END_PROGRAM
"#;
    let left = runtime(&[
        ("alpha/types.st", TYPE_SOURCE),
        ("alpha/main.st", PROGRAM_SOURCE),
    ]);
    let right = runtime(&[
        ("renamed/library.st", TYPE_SOURCE),
        ("renamed/application.st", PROGRAM_SOURCE),
    ]);
    assert_eq!(
        left.registry()
            .type_name(type_id(&left, "CounterValue"))
            .as_deref(),
        right
            .registry()
            .type_name(type_id(&right, "CounterValue"))
            .as_deref()
    );
    assert_eq!(
        left.programs()
            .values()
            .map(|program| program.name.as_str())
            .collect::<Vec<_>>(),
        right
            .programs()
            .values()
            .map(|program| program.name.as_str())
            .collect::<Vec<_>>()
    );
}

#[test]
fn project_assembly_contract_rejects_case_only_type_duplicates_across_files() {
    let error = compile_error(&[
        (
            "first.st",
            r#"
TYPE BatchCode : INT;
END_TYPE
"#,
        ),
        (
            "second.st",
            r#"
TYPE batchcode : DINT;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);
    assert!(
        error.to_ascii_lowercase().contains("duplicate")
            && error.to_ascii_lowercase().contains("batchcode"),
        "{error}"
    );
}

#[test]
fn project_assembly_contract_rejects_cross_kind_type_collision_across_files() {
    let error = compile_error(&[
        (
            "type.st",
            r#"
TYPE Device : INT;
END_TYPE
"#,
        ),
        (
            "class.st",
            r#"
CLASS device
END_CLASS
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);
    assert!(
        error.to_ascii_lowercase().contains("duplicate")
            && error.to_ascii_lowercase().contains("device"),
        "{error}"
    );
}

#[test]
fn project_assembly_contract_rejects_program_duplicate_across_files() {
    let error = compile_error(&[
        (
            "first.st",
            r#"
PROGRAM Main
END_PROGRAM
"#,
        ),
        (
            "second.st",
            r#"
PROGRAM main
END_PROGRAM
"#,
        ),
    ]);
    assert!(
        error.to_ascii_lowercase().contains("duplicate")
            && error.to_ascii_lowercase().contains("main"),
        "{error}"
    );
}

#[test]
fn project_assembly_contract_rejects_function_duplicate_across_files() {
    let error = compile_error(&[
        (
            "first.st",
            r#"
FUNCTION Calculate : INT
Calculate := INT#1;
END_FUNCTION
"#,
        ),
        (
            "second.st",
            r#"
FUNCTION calculate : INT
calculate := INT#2;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);
    assert!(
        error.to_ascii_lowercase().contains("duplicate")
            && error.to_ascii_lowercase().contains("calculate"),
        "{error}"
    );
}

#[test]
fn project_assembly_contract_rejects_multiple_configurations_across_files() {
    let error = compile_error(&[
        (
            "first.st",
            r#"
CONFIGURATION First
END_CONFIGURATION
"#,
        ),
        (
            "second.st",
            r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION Second
PROGRAM MainInstance : Main;
END_CONFIGURATION
"#,
        ),
    ]);
    assert!(error.contains("multiple CONFIGURATION"), "{error}");
}

#[test]
fn project_assembly_contract_rejects_ambiguous_using_type_across_files() {
    let error = compile_error(&[
        (
            "left.st",
            r#"
NAMESPACE Left
TYPE Code : INT;
END_TYPE
END_NAMESPACE
"#,
        ),
        (
            "right.st",
            r#"
NAMESPACE Right
TYPE Code : DINT;
END_TYPE
END_NAMESPACE
"#,
        ),
        (
            "consumer.st",
            r#"
USING Left, Right;
PROGRAM Main
VAR
    Value : Code;
END_VAR
END_PROGRAM
"#,
        ),
    ]);
    assert!(
        error.to_ascii_lowercase().contains("ambiguous")
            && error.to_ascii_lowercase().contains("code"),
        "{error}"
    );
}

#[test]
fn project_assembly_contract_explicit_qualified_type_does_not_fallback_to_using() {
    let error = compile_error(&[
        (
            "types.st",
            r#"
NAMESPACE Available
TYPE Code : INT;
END_TYPE
END_NAMESPACE
"#,
        ),
        (
            "consumer.st",
            r#"
USING Available;
PROGRAM Main
VAR
    Value : Missing.Code;
END_VAR
END_PROGRAM
"#,
        ),
    ]);
    assert!(
        error.contains("E102") && error.contains("Missing.Code"),
        "{error}"
    );
}

#[test]
fn project_assembly_contract_rejects_cross_file_alias_cycle() {
    let error = compile_error(&[
        (
            "left.st",
            r#"
TYPE LeftValue : RightValue;
END_TYPE
"#,
        ),
        (
            "right.st",
            r#"
TYPE RightValue : LeftValue;
END_TYPE
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);
    assert!(
        error.to_ascii_lowercase().contains("cycle")
            || error.to_ascii_lowercase().contains("cyclic")
            || error.to_ascii_lowercase().contains("recursive"),
        "{error}"
    );
}

#[test]
fn project_assembly_contract_semantic_error_uses_owning_source_path() {
    let error = compile_error(&[
        (
            "library.st",
            r#"
FUNCTION Identity : INT
VAR_INPUT
    Value : INT;
END_VAR
Identity := Value;
END_FUNCTION
"#,
        ),
        (
            "application/main.st",
            r#"
PROGRAM Main
VAR
    Value : MissingType;
END_VAR
END_PROGRAM
"#,
        ),
    ]);
    assert!(error.contains("application/main.st:"), "{error}");
    assert!(error.contains("MissingType"), "{error}");
}
