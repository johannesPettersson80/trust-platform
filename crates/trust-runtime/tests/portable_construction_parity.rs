//! Actual authored current STBC through the portable validator and engine.
use trust_runtime::bytecode::{BytecodeVersion, SectionData, SectionId, StorageOwner, StorageRole};
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule, RuntimeState};

const BLOCK: &str = r#"
FUNCTION_BLOCK Counter
VAR_INPUT increment : INT := INT#2; END_VAR
VAR_OUTPUT Total : INT := INT#1; END_VAR
VAR_TEMP delta : INT := INT#1; END_VAR
delta := delta + INT#1;
Total := Total + increment + delta;
END_FUNCTION_BLOCK
PROGRAM Main
VAR counter : Counter; END_VAR
VAR_OUTPUT Observed : INT; END_VAR
counter();
counter(increment := INT#3);
Observed := counter.Total;
END_PROGRAM
"#;

/// Platform services with a debugger attached: every POU runs on the canonical stack path.
struct DebugServices;

impl ExecutionServices for DebugServices {
    fn debug_active(&self) -> bool {
        true
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// The named variables of the root program instance, cloned so that two arms can be compared.
fn root_values(state: &RuntimeState<'_>, root: &str, names: &[&str]) -> Vec<Option<Value>> {
    let Some(Value::Instance(instance)) = state.storage().get_global(root) else {
        panic!("{root} root")
    };
    names
        .iter()
        .map(|name| state.storage().get_instance_var(*instance, name).cloned())
        .collect()
}

#[test]
fn source_free_block_parameters_and_temporaries_have_distinct_storage() {
    let bytes = CompileSession::from_source(BLOCK)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let debug = DebugServices;
    let mut stack = prepared.instantiate_with_services(0, &debug).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("Main root")
    };
    let main = *main;
    state.execute_cycle(Duration::ZERO).unwrap();
    stack.execute_cycle(Duration::ZERO).unwrap();
    assert_eq!(
        state.storage().get_instance_var(main, "Observed"),
        Some(&Value::Int(10))
    );
    assert_eq!(
        root_values(&stack, "Main", &["Observed"]),
        root_values(&state, "Main", &["Observed"]),
        "stack arm after the first cycle"
    );
    state.execute_cycle(Duration::from_millis(1)).unwrap();
    stack.execute_cycle(Duration::from_millis(1)).unwrap();
    assert_eq!(
        state.storage().get_instance_var(main, "Observed"),
        Some(&Value::Int(20))
    );
    assert_eq!(
        root_values(&stack, "Main", &["Observed"]),
        root_values(&state, "Main", &["Observed"]),
        "stack arm after the second cycle"
    );
    assert!(state.storage().current_frame().is_none());
    assert!(stack.storage().current_frame().is_none());
}

#[test]
fn source_free_block_temporary_cannot_forge_a_frame_parameter() {
    let mut module = CompileSession::from_source(BLOCK)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    assert!(module.validate().is_ok());
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("layout")
    };
    let temporary = layout
        .entries
        .iter_mut()
        .find(|entry| entry.owner == StorageOwner::Frame && entry.role == StorageRole::Variable)
        .unwrap();
    temporary.role = StorageRole::Parameter;
    assert!(
        module.validate().is_err(),
        "instance parameters must not be accepted in an FB frame"
    );
}

#[test]
fn source_free_anonymous_pointer_targets_preserve_storage_identity() {
    let source = r#"
PROGRAM Main
VAR
    x : INT := INT#7;
    p : POINTER TO INT;
    pp : POINTER TO POINTER TO INT;
    values : ARRAY[0..2] OF INT := [1, 2, 3];
    array_ptr : POINTER TO ARRAY[0..2] OF INT;
    text : STRING[4] := 'AB';
    text_ptr : POINTER TO STRING[4];
    observed : INT;
END_VAR
p := ADR(x); pp := ADR(p); observed := pp^^;
array_ptr := ADR(values); array_ptr^[2] := INT#9;
text_ptr := ADR(text); text_ptr^[1] := 'Z';
END_PROGRAM
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::ZERO).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("Main root")
    };
    assert_eq!(
        state.storage().get_instance_var(*main, "observed"),
        Some(&Value::Int(7))
    );
    let Some(Value::Array(values)) = state.storage().get_instance_var(*main, "values") else {
        panic!("array")
    };
    assert_eq!(
        values.elements(),
        &[Value::Int(1), Value::Int(2), Value::Int(9)]
    );
    assert_eq!(
        state.storage().get_instance_var(*main, "text"),
        Some(&Value::String("ZB".into()))
    );
    let debug = DebugServices;
    let mut stack = prepared.instantiate_with_services(0, &debug).unwrap();
    stack.execute_cycle(Duration::ZERO).unwrap();
    assert_eq!(
        root_values(&stack, "Main", &["observed", "values", "text"]),
        root_values(&state, "Main", &["observed", "values", "text"]),
        "stack arm"
    );
}

#[test]
fn source_free_configuration_stages_the_selected_scalar_or_aggregate() {
    let source = r#"
TYPE Packet : STRUCT Code : INT := INT#1; Other : INT := INT#9; END_STRUCT; END_TYPE
PROGRAM Main
VAR first : Packet; second : Packet; values : ARRAY[0..1] OF INT := [2, 3]; END_VAR
END_PROGRAM
CONFIGURATION C
PROGRAM P : Main;
VAR_CONFIG
    P.first.Code : INT := INT#21;
    P.second : Packet := (Code := INT#31);
    P.values[1] : INT := INT#41;
END_VAR
END_CONFIGURATION
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let state = prepared.instantiate(0).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("P") else {
        panic!("P root")
    };
    for (name, code) in [("first", 21), ("second", 31)] {
        let Some(Value::Struct(packet)) = state.storage().get_instance_var(*main, name) else {
            panic!("packet")
        };
        assert_eq!(packet.field("Code"), Some(&Value::Int(code)));
        assert_eq!(packet.field("Other"), Some(&Value::Int(9)));
    }
    let Some(Value::Array(values)) = state.storage().get_instance_var(*main, "values") else {
        panic!("array")
    };
    assert_eq!(values.elements(), &[Value::Int(2), Value::Int(41)]);
    let debug = DebugServices;
    let stack = prepared.instantiate_with_services(0, &debug).unwrap();
    assert_eq!(
        root_values(&stack, "P", &["first", "second", "values"]),
        root_values(&state, "P", &["first", "second", "values"]),
        "stack arm"
    );
}

#[test]
fn source_free_qualified_function_calls_preserve_instance_receivers_and_in_out() {
    let source = r#"
NAMESPACE Support
NAMESPACE Nested
FUNCTION Add : INT
VAR_INPUT A : INT; B : INT; EN : BOOL := TRUE; END_VAR
Add := A + B;
END_FUNCTION
FUNCTION Bump : BOOL
VAR_IN_OUT Number : INT; END_VAR
Number := Number + INT#1;
Bump := TRUE;
END_FUNCTION
END_NAMESPACE
END_NAMESPACE
NAMESPACE Shadow
FUNCTION Add : INT
VAR_INPUT Amount : INT; END_VAR
Add := INT#1000 + Amount;
END_FUNCTION
END_NAMESPACE
FUNCTION_BLOCK CountBlock
VAR_INPUT Delta : INT; END_VAR
VAR_OUTPUT Total : INT; END_VAR
Total := Total + Delta;
END_FUNCTION_BLOCK
CLASS Meter
VAR current : INT := INT#5; END_VAR
METHOD PUBLIC Add : INT
VAR_INPUT Amount : INT; END_VAR
current := current + Amount;
Add := current;
END_METHOD
END_CLASS
PROGRAM Main
VAR counter : CountBlock; meter : Meter; Shadow : Meter; number : INT := INT#4; END_VAR
VAR_OUTPUT Observed : INT; Bumped : BOOL; Disabled : INT; Shadowed : INT; END_VAR
number := support.nested.Add(A := number, B := INT#2);
Bumped := Support.Nested.Bump(Number := number);
counter(Delta := number);
Observed := meter.Add(Amount := counter.Total);
Disabled := Support.Nested.Add(EN := FALSE, A := INT#100, B := INT#200);
Shadowed := Shadow.Add(Amount := INT#3);
END_PROGRAM
"#;
    let bytes = {
        let session = CompileSession::from_source(source);
        session
            .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
            .unwrap()
            .encode()
            .unwrap()
    };
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let debug = DebugServices;
    let mut stack = prepared.instantiate_with_services(0, &debug).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("Main root")
    };
    let main = *main;
    let names = ["number", "Observed", "Bumped", "Disabled", "Shadowed"];
    for (millis, number, observed, shadowed) in [(0, 7, 12, 8), (1, 10, 29, 11)] {
        state.execute_cycle(Duration::from_millis(millis)).unwrap();
        stack.execute_cycle(Duration::from_millis(millis)).unwrap();
        assert_eq!(
            state.storage().get_instance_var(main, "number"),
            Some(&Value::Int(number))
        );
        assert_eq!(
            state.storage().get_instance_var(main, "Observed"),
            Some(&Value::Int(observed))
        );
        assert_eq!(
            state.storage().get_instance_var(main, "Bumped"),
            Some(&Value::Bool(true))
        );
        assert_eq!(
            state.storage().get_instance_var(main, "Disabled"),
            Some(&Value::Int(0))
        );
        assert_eq!(
            state.storage().get_instance_var(main, "Shadowed"),
            Some(&Value::Int(shadowed))
        );
        assert_eq!(
            root_values(&stack, "Main", &names),
            root_values(&state, "Main", &names),
            "stack arm"
        );
    }
    assert!(state.storage().current_frame().is_none());
    assert!(stack.storage().current_frame().is_none());
}

#[test]
fn source_free_return_defaults_and_parameter_presence_keep_phase_order() {
    let source = r#"
TYPE ReturnDefault : INT := INT#7; END_TYPE
FUNCTION Pick : ReturnDefault
VAR_INPUT number : ReturnDefault := INT#9; END_VAR
IF number <> INT#9 THEN Pick := number; END_IF
END_FUNCTION
PROGRAM Main
VAR omitted : INT; supplied : INT; END_VAR
omitted := Pick();
supplied := Pick(number := INT#3);
END_PROGRAM
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap()
        .encode()
        .unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::ZERO).unwrap();
    let Some(Value::Instance(main)) = state.storage().get_global("Main") else {
        panic!("Main root")
    };
    // An unwritten return uses the resolved intrinsic INT default. The type's
    // declaration initializer is not a return-slot initialization recipe.
    assert_eq!(
        state.storage().get_instance_var(*main, "omitted"),
        Some(&Value::Int(0))
    );
    assert_eq!(
        state.storage().get_instance_var(*main, "supplied"),
        Some(&Value::Int(3))
    );
    assert!(state.storage().current_frame().is_none());
    let debug = DebugServices;
    let mut stack = prepared.instantiate_with_services(0, &debug).unwrap();
    stack.execute_cycle(Duration::ZERO).unwrap();
    assert_eq!(
        root_values(&stack, "Main", &["omitted", "supplied"]),
        root_values(&state, "Main", &["omitted", "supplied"]),
        "stack arm"
    );
    assert!(stack.storage().current_frame().is_none());
}
