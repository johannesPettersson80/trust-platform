//! Warm restart transfers live object ownership, not stale instance numbers.
use trust_runtime::bytecode::{
    BytecodeModule, BytecodeVersion, InitializationStage, SectionData, SectionId, StorageOwner,
};
use trust_runtime::harness::CompileSession;
use trust_runtime_core::memory::InstanceId;
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn artifact(body: &str) -> Vec<u8> {
    let source = format!("{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Tick (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Tick : Main; END_RESOURCE END_CONFIGURATION");
    CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted ST source")
        .encode()
        .expect("encode STBC")
}

fn instance(value: Option<&Value>) -> InstanceId {
    match value {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected instance: {other:?}"),
    }
}

#[test]
fn warm_restart_retains_nested_objects_aliases_and_self_references() {
    let bytes = artifact(
        r#"
FUNCTION_BLOCK Leaf
VAR_OUTPUT value : INT; link : REF_TO INT; END_VAR
value := value + INT#1;
link := REF(value);
END_FUNCTION_BLOCK
FUNCTION_BLOCK Holder
VAR child : Leaf; END_VAR
child();
END_FUNCTION_BLOCK
VAR_GLOBAL RETAIN kept : Holder; alias : Holder; END_VAR
PROGRAM Main
kept();
END_PROGRAM
"#,
    );
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let kept = state.storage().get_global("kept").unwrap().clone();
    state.write_global("alias", kept).unwrap();
    for (sample, expected) in [(10, 1), (20, 2), (30, 3)] {
        state.execute_cycle(Duration::from_millis(sample)).unwrap();
        state.restart(RestartMode::Warm).unwrap();
        let holder = instance(state.storage().get_global("kept"));
        assert_eq!(instance(state.storage().get_global("alias")), holder);
        let child = instance(state.storage().get_instance_var(holder, "child"));
        assert_eq!(
            state.storage().get_instance_var(child, "value"),
            Some(&Value::Int(expected))
        );
        let Some(Value::Reference(Some(link))) = state.storage().get_instance_var(child, "link")
        else {
            panic!("retained reference")
        };
        assert_eq!(
            state.storage().read_by_ref_ref(link),
            Some(&Value::Int(expected))
        );
    }
    state.restart(RestartMode::Cold).unwrap();
    let holder = instance(state.storage().get_global("kept"));
    let child = instance(state.storage().get_instance_var(holder, "child"));
    assert_eq!(
        state.storage().get_instance_var(child, "value"),
        Some(&Value::Int(0))
    );
}

#[test]
fn retained_reference_to_nonretained_root_observes_reset_target() {
    let bytes = artifact(
        r#"
FUNCTION_BLOCK Leaf
VAR_INPUT seed : INT; END_VAR
VAR_OUTPUT value : INT := INT#4; END_VAR
value := seed;
END_FUNCTION_BLOCK
VAR_GLOBAL fresh : Leaf; scalar : INT := INT#9; END_VAR
VAR_GLOBAL RETAIN object_ref : REF_TO INT; scalar_ref : REF_TO INT; END_VAR
PROGRAM Main
fresh(seed := INT#42);
scalar := INT#77;
object_ref := REF(fresh.value);
scalar_ref := REF(scalar);
END_PROGRAM
"#,
    );
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let fresh_before = instance(state.storage().get_global("fresh"));
    assert_eq!(
        state.storage().get_instance_var(fresh_before, "value"),
        Some(&Value::Int(42))
    );
    assert_eq!(state.storage().get_global("scalar"), Some(&Value::Int(77)));

    state.restart(RestartMode::Warm).unwrap();
    for (name, expected) in [("object_ref", 4), ("scalar_ref", 9)] {
        let Some(Value::Reference(Some(reference))) = state.storage().get_global(name) else {
            panic!("retained global reference")
        };
        assert_eq!(
            state.storage().read_by_ref_ref(reference),
            Some(&Value::Int(expected))
        );
    }
}

#[test]
fn restart_initializers_observe_preserved_time_and_failure_is_transactional() {
    let bytes = artifact(
        r#"
VAR_GLOBAL RETAIN divisor : INT := INT#1; END_VAR
VAR_GLOBAL stamp : TIME := T#0ms; quotient : INT := INT#8 / INT#1; END_VAR
VAR_GLOBAL incoming AT %IB0 : BYTE; outgoing AT %QB0 : BYTE; END_VAR
PROGRAM Main outgoing := BYTE#42; END_PROGRAM
"#,
    );
    // Source initializers intentionally exclude native calls. Exercise the wider
    // admitted wire contract with the real CALL_NATIVE and STORE_REF opcodes.
    let mut module = BytecodeModule::decode(&bytes).unwrap();
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let native = strings.entries.len() as u32;
    strings.entries.push("TIME".into());
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let declaration = layout
        .entries
        .iter()
        .position(|entry| {
            entry.owner == StorageOwner::Global
                && strings.entries[entry.name_idx as usize] == "stamp"
        })
        .unwrap();
    let Some(SectionData::Initializers(initializers)) = module.section(SectionId::Initializers)
    else {
        panic!("initializers")
    };
    let action = initializers
        .entries
        .iter()
        .position(|entry| {
            entry.declaration_idx == Some(declaration as u32)
                && entry.stage == InitializationStage::Explicit
        })
        .unwrap();
    let result = initializers.entries[action].result_ref_idx;
    let quotient = layout
        .entries
        .iter()
        .position(|entry| {
            entry.owner == StorageOwner::Global
                && strings.entries[entry.name_idx as usize] == "quotient"
        })
        .unwrap();
    let divisor = layout
        .entries
        .iter()
        .find(|entry| {
            entry.owner == StorageOwner::Global
                && strings.entries[entry.name_idx as usize] == "divisor"
        })
        .unwrap()
        .ref_idx
        .unwrap();
    let quotient_action = initializers
        .entries
        .iter()
        .find(|entry| {
            entry.declaration_idx == Some(quotient as u32)
                && entry.stage == InitializationStage::Explicit
        })
        .unwrap()
        .clone();
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("code")
    };
    let quotient_body = &mut code[quotient_action.code_offset as usize
        ..(quotient_action.code_offset + quotient_action.code_length) as usize];
    let denominator = quotient_body
        .windows(6)
        .enumerate()
        .filter(|(_, bytes)| bytes[0] == 0x10 && bytes[5] == 0x43)
        .map(|(offset, _)| offset)
        .collect::<Vec<_>>();
    assert_eq!(
        denominator.len(),
        1,
        "one denominator LOAD_CONST directly precedes DIV"
    );
    quotient_body[denominator[0]] = 0x20;
    quotient_body[denominator[0] + 1..denominator[0] + 5].copy_from_slice(&divisor.to_le_bytes());
    let mut body = vec![0x09];
    body.extend_from_slice(&trust_runtime::bytecode::NATIVE_CALL_KIND_STDLIB.to_le_bytes());
    body.extend_from_slice(&native.to_le_bytes());
    body.extend_from_slice(&0u32.to_le_bytes());
    body.push(0x21);
    body.extend_from_slice(&result.to_le_bytes());
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("code")
    };
    let offset = code.len() as u32;
    let length = body.len() as u32;
    code.extend(body);
    let Some(SectionData::Initializers(initializers)) = module.section_mut(SectionId::Initializers)
    else {
        panic!("initializers")
    };
    initializers.entries[action].code_offset = offset;
    initializers.entries[action].code_length = length;
    let bytes = module.encode().unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.inputs_mut()[0] = 0x5a;
    state.execute_cycle(Duration::from_millis(50)).unwrap();
    state.restart(RestartMode::Warm).unwrap();
    assert_eq!(
        state.storage().get_global("stamp"),
        Some(&Value::Time(Duration::from_millis(50)))
    );
    state.write_global("divisor", Value::Int(0)).unwrap();
    let before = state.storage().globals().clone();
    let before_inputs = state.inputs_mut().to_vec();
    let before_outputs = state.outputs().to_vec();
    assert_eq!(before_outputs, [42]);
    let tasks = state
        .task_states()
        .iter()
        .map(|task| (task.last_single, task.last_run, task.overrun_count))
        .collect::<Vec<_>>();
    assert!(state.restart(RestartMode::Warm).is_err());
    assert_eq!(state.storage().globals(), &before);
    assert_eq!(&*state.inputs_mut(), before_inputs.as_slice());
    assert_eq!(state.outputs(), before_outputs.as_slice());
    assert_eq!(
        state
            .task_states()
            .iter()
            .map(|task| (task.last_single, task.last_run, task.overrun_count))
            .collect::<Vec<_>>(),
        tasks
    );
    assert!(state.fault().is_none());
    state.write_global("divisor", Value::Int(1)).unwrap();
    state.restart(RestartMode::Cold).unwrap();
    assert_eq!(
        state.storage().get_global("stamp"),
        Some(&Value::Time(Duration::from_millis(50)))
    );
}

#[test]
fn retained_interfaces_do_not_retain_nonowned_root_state() {
    let bytes = artifact(
        r#"
INTERFACE Reader
METHOD ReadValue : INT END_METHOD
END_INTERFACE
FUNCTION_BLOCK Counter IMPLEMENTS Reader
VAR_INPUT seed : INT; END_VAR
VAR_OUTPUT value : INT := INT#4; END_VAR
METHOD PUBLIC ReadValue : INT ReadValue := value; END_METHOD
value := seed;
END_FUNCTION_BLOCK
TYPE HandleBox : STRUCT view : Reader; END_STRUCT; END_TYPE
VAR_GLOBAL fresh : Counter; END_VAR
VAR_GLOBAL RETAIN direct_view : Reader; nested_view : HandleBox; END_VAR
PROGRAM Main
fresh(seed := INT#42);
direct_view := fresh;
nested_view.view := fresh;
END_PROGRAM
"#,
    );
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    let fresh_before = instance(state.storage().get_global("fresh"));
    assert_eq!(
        state.storage().get_instance_var(fresh_before, "value"),
        Some(&Value::Int(42))
    );

    state.restart(RestartMode::Warm).unwrap();
    let fresh = instance(state.storage().get_global("fresh"));
    assert_eq!(
        state.storage().get_instance_var(fresh, "value"),
        Some(&Value::Int(4))
    );
    assert_eq!(instance(state.storage().get_global("direct_view")), fresh);
    let Some(Value::Struct(nested)) = state.storage().get_global("nested_view") else {
        panic!("interface aggregate")
    };
    assert_eq!(instance(nested.field("view")), fresh);
}

#[test]
fn retain_qualified_function_static_keeps_after_restart_initialization_policy() {
    let bytes = artifact(
        r#"
FUNCTION NextValue : INT
VAR_STAT RETAIN count : INT := INT#2; END_VAR
count := count + INT#1;
NextValue := count;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := NextValue();
END_PROGRAM
"#,
    );
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for time in [10, 20] {
        state.execute_cycle(Duration::from_millis(time)).unwrap();
    }
    state.restart(RestartMode::Warm).unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    let program = instance(state.storage().get_global("Plant"));
    assert_eq!(
        state.storage().get_instance_var(program, "result"),
        Some(&Value::Int(3))
    );
}
