//! Source authoring, serialized transport, fresh portable construction and legacy parity.
use std::cell::Cell;
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::{InstanceId, VariableStorage};
use trust_runtime_core::retain::RestartMode;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

fn artifact(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("accepted source")
        .encode()
        .unwrap()
}
// IEC source forbids REF to function-local temporary storage. These two wire
// regressions replace an admitted global reference address with a typed local
// address of the same encoded width, preserving code extents and branch targets.
fn replace_global_with_local_reference(
    module: &mut trust_runtime::bytecode::BytecodeModule,
    owner_name: &str,
    local_name: &str,
    initializer_target: Option<&str>,
) {
    use trust_runtime::bytecode::{
        InitializationStage, InitializationTrigger, SectionData, SectionId, StorageOwner,
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let Some(SectionData::PouIndex(pous)) = module.section(SectionId::PouIndex) else {
        panic!("POUs")
    };
    let owner = pous
        .entries
        .iter()
        .find(|pou| strings.entries[pou.name_idx as usize].eq_ignore_ascii_case(owner_name))
        .unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let find_local = |name: &str| {
        layout
            .entries
            .iter()
            .enumerate()
            .find(|(_, entry)| {
                entry.owner == StorageOwner::Frame
                    && entry.owner_pou_id == Some(owner.id)
                    && strings.entries[entry.name_idx as usize].eq_ignore_ascii_case(name)
            })
            .expect("named owner-local declaration")
    };
    let reference = find_local(local_name).1.ref_idx.unwrap();
    let placeholder = layout
        .entries
        .iter()
        .find(|entry| {
            entry.owner == StorageOwner::Global
                && strings.entries[entry.name_idx as usize] == "reference_placeholder"
        })
        .expect("typed persistent reference placeholder")
        .ref_idx
        .unwrap();
    let (start, length) = if let Some(target) = initializer_target {
        let declaration = find_local(target).0 as u32;
        let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
            panic!("initializers")
        };
        let action = index
            .entries
            .iter()
            .find(|entry| {
                entry.declaration_idx == Some(declaration)
                    && entry.stage == InitializationStage::Explicit
                    && entry.trigger == InitializationTrigger::Ordinary
            })
            .unwrap();
        (action.code_offset, action.code_length)
    } else {
        (owner.code_offset, owner.code_length)
    };
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("bodies")
    };
    let body = &mut code[start as usize..(start + length) as usize];
    redirect_reference_operand(body, placeholder, reference);
}

fn redirect_reference_operand(body: &mut [u8], placeholder: u32, reference: u32) {
    let mut pc = 0;
    let mut placeholders = Vec::new();
    while pc < body.len() {
        let opcode = body[pc];
        let width = trust_runtime_core::vm::hosted::opcode_operand_len(opcode).unwrap();
        if opcode == 0x22 {
            let operand = u32::from_le_bytes(body[pc + 1..pc + 5].try_into().unwrap());
            if operand == placeholder {
                placeholders.push(pc);
            }
        }
        pc += 1 + width;
    }
    assert_eq!(
        placeholders.len(),
        1,
        "one global reference placeholder in the selected body"
    );
    let pc = placeholders[0];
    body[pc + 1..pc + 5].copy_from_slice(&reference.to_le_bytes());
}

fn scheduled(body: &str) -> String {
    format!("{body}\nCONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION")
}
fn instance(value: Option<&Value>) -> InstanceId {
    match value {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected instance: {other:?}"),
    }
}
fn field(storage: &VariableStorage, name: &str) -> Value {
    let plant = instance(storage.get_global("Plant"));
    storage
        .get_instance_var(plant, name)
        .unwrap_or_else(|| panic!("missing {name}"))
        .clone()
}

#[test]
fn authored_saved_fixture_matches_legacy_at_every_scan() {
    let source = include_str!("fixtures/portability/stbc-2.0/main.st");
    let bytes = CompileSession::from_sources(vec![trust_runtime::harness::SourceFile::with_path(
        "portability/main.st",
        source,
    )])
    .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
    .expect("saved fixture source")
    .encode()
    .expect("saved fixture STBC");
    assert_eq!(
        bytes.as_slice(),
        include_bytes!("fixtures/portability/stbc-2.0/program-v2.stbc")
    );
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut portable = prepared.instantiate(0).unwrap();
    let mut legacy = CompileSession::from_source(source).build_runtime().unwrap();
    for ms in (0..=1000).step_by(10) {
        portable.execute_cycle(Duration::from_millis(ms)).unwrap();
        legacy.set_current_time(Duration::from_millis(ms));
        legacy.execute_cycle().unwrap();
        for name in ["result", "activations", "enabled"] {
            assert_eq!(
                field(portable.storage(), name),
                field(legacy.storage(), name),
                "{name} at {ms}"
            );
        }
        for (owner, names) in [
            ("counter", &["value", "history"][..]),
            ("delay", &["Q", "ET"][..]),
        ] {
            let p = instance(Some(&field(portable.storage(), owner)));
            let h = instance(Some(&field(legacy.storage(), owner)));
            for name in names {
                assert_eq!(
                    portable.storage().get_instance_var(p, name),
                    legacy.storage().get_instance_var(h, name),
                    "{owner}.{name} at {ms}"
                );
            }
        }
    }
}

#[test]
fn automatic_explicit_value_suppresses_faulting_alias_default() {
    let source=scheduled("TYPE Broken : LREAL := LREAL#7.0 / LREAL#0.0; END_TYPE FUNCTION Read : LREAL VAR local : Broken := LREAL#9.0; END_VAR Read := local; END_FUNCTION PROGRAM Main VAR result : LREAL; END_VAR result := Read(); END_PROGRAM");
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(field(state.storage(), "result"), Value::LReal(9.0));
    let failing=scheduled("TYPE Broken : LREAL := LREAL#7.0 / LREAL#0.0; END_TYPE PROGRAM Main VAR bad : Broken; END_VAR END_PROGRAM");
    let bytes = artifact(&failing);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    assert!(
        prepared.instantiate(0).is_err(),
        "unoverridden declared default must execute"
    );
}

#[test]
fn function_statics_persist_between_calls_and_reinitialize_after_restart() {
    let source=scheduled("FUNCTION Next : INT VAR_STAT count : INT := INT#2; END_VAR count := count + INT#1; Next := count; END_FUNCTION PROGRAM Main VAR result : INT; END_VAR result := Next(); END_PROGRAM");
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for (ms, expected) in [(10, 3), (20, 4)] {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
        assert_eq!(field(state.storage(), "result"), Value::Int(expected));
    }
    state.restart(RestartMode::Warm).unwrap();
    state.execute_cycle(Duration::from_millis(30)).unwrap();
    assert_eq!(field(state.storage(), "result"), Value::Int(3));
    state.restart(RestartMode::Cold).unwrap();
    state.execute_cycle(Duration::from_millis(40)).unwrap();
    assert_eq!(field(state.storage(), "result"), Value::Int(3));
}

#[test]
fn inherited_method_static_has_one_declaring_owner() {
    let source=scheduled("CLASS BaseDevice VAR PUBLIC value : INT := INT#5; END_VAR METHOD PUBLIC ReadValue : INT VAR_STAT calls : INT := INT#2; END_VAR calls := calls + INT#1; ReadValue := value + calls; END_METHOD END_CLASS FUNCTION_BLOCK Device EXTENDS BaseDevice END_FUNCTION_BLOCK PROGRAM Main VAR device : Device; result : INT; END_VAR result := device.ReadValue(); END_PROGRAM");
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    // Spec 12 §11.5.9 corrects legacy's non-recursive first-use lookup.
    // Pin the physical ancestor slot rather than reproducing that defect.
    let device = instance(Some(&field(state.storage(), "device")));
    let ancestor = state.storage().instances()[&device]
        .parent
        .expect("declaring class ancestor");
    let static_names: Vec<_> = state.storage().instances()[&ancestor]
        .variables
        .keys()
        .filter(|name| name.contains("calls"))
        .cloned()
        .collect();
    assert_eq!(static_names.len(), 1, "exactly one declaring-owner static");
    assert!(!state.storage().instances()[&device]
        .variables
        .contains_key(&static_names[0]));
    assert_eq!(
        state.storage().get_instance_var(ancestor, &static_names[0]),
        Some(&Value::Int(2))
    );
    for (ms, expected, count) in [(10, 8, 3), (20, 9, 4)] {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
        assert_eq!(field(state.storage(), "result"), Value::Int(expected));
        assert_eq!(
            state.storage().get_instance_var(ancestor, &static_names[0]),
            Some(&Value::Int(count))
        );
    }
}

#[test]
fn partial_access_configuration_permissions_and_global_constants() {
    let source="VAR_GLOBAL input : INT; END_VAR VAR_GLOBAL CONSTANT fixed : INT := INT#7; END_VAR PROGRAM Main VAR bits : WORD; END_VAR END_PROGRAM CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0); PROGRAM Plant WITH Periodic : Main; END_RESOURCE VAR_ACCESS selected : Controller.Plant.bits.%X3 : BOOL READ_WRITE; observed : Controller.Plant.bits : WORD READ_ONLY; END_VAR VAR_CONFIG Plant.bits : WORD := WORD#7; END_VAR END_CONFIGURATION";
    let bytes = artifact(source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    assert_eq!(state.read_access("selected").unwrap(), Value::Bool(false));
    assert_eq!(
        state.read_access("missing_alias"),
        Err(RuntimeError::InvalidAlias)
    );
    assert_eq!(
        state.write_access("missing_alias", Value::Bool(true)),
        Err(RuntimeError::InvalidAlias)
    );
    state.write_access("SELECTED", Value::Bool(true)).unwrap();
    assert_eq!(state.read_access("observed").unwrap(), Value::Word(15));
    assert_eq!(
        state.write_access("observed", Value::Word(0)),
        Err(RuntimeError::InvalidAlias)
    );
    assert!(state.write_access("selected", Value::Int(1)).is_err());
    assert_eq!(state.read_access("observed").unwrap(), Value::Word(15));
    state.write_global("INPUT", Value::Int(12)).unwrap();
    assert_eq!(state.storage().get_global("input"), Some(&Value::Int(12)));
    assert_eq!(
        state.write_global("fixed", Value::Int(8)),
        Err(RuntimeError::ConstantWrite)
    );
    assert_eq!(state.storage().get_global("fixed"), Some(&Value::Int(7)));
}

struct Deadline {
    expired: Cell<bool>,
}
impl ExecutionServices for Deadline {
    fn deadline_exceeded(&self) -> bool {
        self.expired.get()
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}
#[test]
fn initializer_limits_and_failed_restart_leave_original_state_intact() {
    let source = scheduled(
        "PROGRAM Main VAR count : INT := INT#3; END_VAR count := count + INT#1; END_PROGRAM",
    );
    let bytes = artifact(&source);
    for limits in [
        PreparationLimits {
            max_work: 0,
            ..Default::default()
        },
        PreparationLimits {
            max_call_depth: 0,
            ..Default::default()
        },
        PreparationLimits {
            max_construction_bytes: 0,
            ..Default::default()
        },
    ] {
        let result = PreparedModule::from_bytes(&bytes, limits);
        assert!(match result {
            Ok(module) => module.instantiate(0).is_err(),
            Err(_) => true,
        });
    }
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let services = Deadline {
        expired: Cell::new(false),
    };
    let mut state = prepared.instantiate_with_services(0, &services).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(field(state.storage(), "count"), Value::Int(4));
    services.expired.set(true);
    assert!(prepared.instantiate_with_services(0, &services).is_err());
    assert!(state.restart(RestartMode::Cold).is_err());
    assert_eq!(field(state.storage(), "count"), Value::Int(4));
    assert!(state.fault().is_none());
    services.expired.set(false);
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(field(state.storage(), "count"), Value::Int(5));
}

#[test]
fn native_initializer_outputs_are_confined_to_the_staging_result() {
    use trust_runtime::bytecode::{InitializationStage, SectionData, SectionId, StorageOwner};
    // Source admission intentionally disallows standard-function initializers.
    // Author a valid lowered wire body to exercise the consumer's wider contract.
    let source=scheduled("VAR_GLOBAL calendar_date : DATE := DATE#2024-02-15; day : INT := INT#7; END_VAR PROGRAM Main END_PROGRAM");
    let original = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    for outside_staging in [false, true] {
        let mut module = original.clone();
        let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable)
        else {
            panic!("strings")
        };
        let native = strings.entries.len() as u32;
        strings.entries.push("SPLIT_DATE|E|T|T|T".into());
        let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout)
        else {
            panic!("layout")
        };
        let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
            panic!("strings")
        };
        let (declaration, day) = layout
            .entries
            .iter()
            .enumerate()
            .find(|(_, d)| {
                d.owner == StorageOwner::Global && strings.entries[d.name_idx as usize] == "day"
            })
            .unwrap();
        let ty = day.type_id.unwrap();
        let global = day.ref_idx.unwrap();
        let date = layout
            .entries
            .iter()
            .find(|d| {
                d.owner == StorageOwner::Global
                    && strings.entries[d.name_idx as usize] == "calendar_date"
            })
            .unwrap()
            .ref_idx
            .unwrap();
        let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
            panic!("initializers")
        };
        let action = index
            .entries
            .iter()
            .position(|entry| {
                entry.declaration_idx == Some(declaration as u32)
                    && entry.stage == InitializationStage::Explicit
            })
            .unwrap();
        let staging = index.entries[action].result_ref_idx;
        let mut body = Vec::new();
        fn operand(body: &mut Vec<u8>, opcode: u8, value: u32) {
            body.push(opcode);
            body.extend_from_slice(&value.to_le_bytes());
        }
        operand(
            &mut body,
            trust_runtime::bytecode::opcodes::DEFAULT_TYPED,
            ty,
        );
        operand(&mut body, 0x21, staging); // initialize the typed result before native output writes
        operand(&mut body, 0x20, date);
        for _ in 0..3 {
            operand(
                &mut body,
                0x22,
                if outside_staging { global } else { staging },
            );
        }
        operand(
            &mut body,
            0x09,
            trust_runtime::bytecode::NATIVE_CALL_KIND_STDLIB,
        );
        body.extend_from_slice(&native.to_le_bytes());
        body.extend_from_slice(&4u32.to_le_bytes());
        body.push(0x12);
        let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
            panic!("code")
        };
        let offset = code.len() as u32;
        let length = body.len() as u32;
        code.extend(body);
        let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers)
        else {
            panic!("initializers")
        };
        index.entries[action].code_offset = offset;
        index.entries[action].code_length = length;
        let bytes = module.encode().unwrap();
        let result = PreparedModule::from_bytes(&bytes, PreparationLimits::default());
        if outside_staging {
            assert!(
                result.is_err(),
                "native output cannot write an existing global during construction"
            );
        } else {
            let prepared = result.unwrap();
            let state = prepared.instantiate(0).unwrap();
            assert_eq!(state.storage().get_global("day"), Some(&Value::Int(15)));
        }
    }
}

#[test]
fn edge_inputs_and_flat_images_keep_scan_order() {
    let source=scheduled("FUNCTION_BLOCK EdgeCounter VAR_INPUT pulse : BOOL R_EDGE; END_VAR VAR_OUTPUT count : INT; END_VAR IF pulse THEN count := count + INT#1; END_IF; END_FUNCTION_BLOCK PROGRAM Main VAR input AT %IX0.0 : BOOL; output AT %QW2 : WORD; counter : EdgeCounter; END_VAR counter(pulse := input); output := INT_TO_WORD(counter.count); END_PROGRAM");
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for (ms, input, expected) in [(10, 1, 1u16), (20, 1, 1), (30, 0, 1), (40, 1, 2)] {
        state.inputs_mut()[0] = input;
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
        assert_eq!(
            &state.outputs()[2..4],
            &expected.to_le_bytes(),
            "sample {ms}"
        );
    }
}

#[test]
fn typed_host_write_rejects_references_hidden_in_an_aggregate() {
    use trust_runtime_core::memory::{FrameId, MemoryLocation};
    use trust_runtime_core::value::{ArrayValue, ValueRef};
    let source=scheduled("TYPE IntegerRef : REF_TO INT; END_TYPE VAR_GLOBAL pointers : ARRAY[1..1] OF IntegerRef; END_VAR PROGRAM Main END_PROGRAM");
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let before = state.storage().get_global("pointers").unwrap().clone();
    let forged = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Reference(Some(ValueRef {
            location: MemoryLocation::Local(FrameId(u32::MAX)),
            offset: 0,
            path: vec![],
        }))],
        vec![(1, 1)],
    )));
    assert_eq!(
        state.write_global("pointers", forged),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(state.storage().get_global("pointers"), Some(&before));
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert!(state.fault().is_none());
}

#[test]
fn failed_lazy_static_keeps_earlier_completed_static_without_publishing_a_result() {
    use trust_runtime::bytecode::{SectionData, SectionId, StorageRole};
    let source=scheduled("VAR_GLOBAL data : ARRAY[1..1] OF INT := [INT#9]; index : DINT := DINT#1; END_VAR FUNCTION Read : INT VAR_STAT first : REF_TO INT := REF(data[1]); second : REF_TO INT := REF(data[index]); END_VAR Read := first^ + second^; END_FUNCTION PROGRAM Main VAR result : INT; END_VAR result := Read(); END_PROGRAM");
    let module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let first = layout
        .entries
        .iter()
        .find(|d| {
            d.role == StorageRole::Static
                && d.source_name_idx
                    .is_some_and(|n| strings.entries[n as usize] == "first")
        })
        .map(|d| strings.entries[d.name_idx as usize].clone())
        .unwrap();
    let bytes = module.encode().unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(field(state.storage(), "result"), Value::Int(18));
    state.restart(RestartMode::Cold).unwrap();
    state.write_global("index", Value::DInt(2)).unwrap();
    assert!(state.execute_cycle(Duration::from_millis(20)).is_err());
    assert!(matches!(
        state.storage().get_global(&first),
        Some(Value::Reference(Some(_)))
    ));
    assert_eq!(field(state.storage(), "result"), Value::Int(0));
    assert!(state.fault().is_some());
}

#[test]
fn recursive_calls_can_mutate_the_suspended_ancestors_reference_target() {
    let source=scheduled("VAR_GLOBAL reference_placeholder : INT; END_VAR FUNCTION Bump : INT VAR_INPUT target : REF_TO INT; END_VAR target^ := target^ + INT#5; Bump := target^; END_FUNCTION FUNCTION Relay : INT VAR_INPUT target : REF_TO INT; END_VAR VAR unrelated : INT := INT#100; END_VAR Relay := Bump(target) + unrelated; END_FUNCTION FUNCTION Outer : INT VAR local : INT := INT#7; returned : INT; END_VAR returned := Relay(REF(reference_placeholder)); Outer := local + returned; END_FUNCTION PROGRAM Main VAR result : INT; END_VAR result := Outer(); END_PROGRAM");
    let mut module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    replace_global_with_local_reference(&mut module, "Outer", "local", None);
    module
        .validated_source_free(Default::default())
        .expect("ancestor-local reference is admitted by the wire contract");
    let bytes = module.encode().unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for ms in [10, 20] {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
        assert_eq!(field(state.storage(), "result"), Value::Int(124));
    }
}

#[test]
fn recipe_recursion_and_dispatch_loops_share_bounded_initialization_work() {
    use trust_runtime::bytecode::{InitializerBodyKind, SectionData, SectionId};
    let source=scheduled("TYPE Initial : INT := INT#7; END_TYPE VAR_GLOBAL value : Initial; END_VAR PROGRAM Main END_PROGRAM");
    let original = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let baseline_bytes = original.encode().unwrap();
    let baseline = PreparedModule::from_bytes(
        &baseline_bytes,
        PreparationLimits {
            max_work: 1000,
            ..Default::default()
        },
    )
    .unwrap();
    baseline
        .instantiate(0)
        .expect("the identical limit admits ordinary construction");
    for cycle in [true, false] {
        let mut module = original.clone();
        let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
            panic!("initializers")
        };
        let id = index
            .entries
            .iter()
            .position(|entry| entry.body_kind == InitializerBodyKind::TypeDefault)
            .unwrap();
        let recipe = index.entries[id].clone();
        let mut body = Vec::new();
        if cycle {
            body.push(trust_runtime::bytecode::opcodes::DEFAULT_TYPED);
            body.extend_from_slice(&recipe.recipe_type_id.unwrap().to_le_bytes());
            body.push(0x21);
            body.extend_from_slice(&recipe.result_ref_idx.to_le_bytes());
        } else {
            body.push(0x02);
            body.extend_from_slice(&(-5i32).to_le_bytes());
        }
        let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
            panic!("code")
        };
        let offset = code.len() as u32;
        let length = body.len() as u32;
        code.extend(body);
        let Some(SectionData::Initializers(index)) = module.section_mut(SectionId::Initializers)
        else {
            panic!("initializers")
        };
        index.entries[id].code_offset = offset;
        index.entries[id].code_length = length;
        let bytes = module.encode().unwrap();
        let prepared = PreparedModule::from_bytes(
            &bytes,
            PreparationLimits {
                max_work: 1000,
                ..Default::default()
            },
        )
        .expect("bounded execution, not syntactic rejection, owns recipe recursion and loops");
        let error = match prepared.instantiate(0) {
            Ok(_) => panic!("unbounded initializer succeeded"),
            Err(error) => error,
        };
        if cycle {
            assert_eq!(
                error.stable_code(),
                trust_runtime_core::error::StableErrorCode::VmBytecodeDecode
            );
            assert!(error
                .to_string()
                .contains("cyclic typed default construction"));
        } else {
            assert_eq!(
                error,
                trust_runtime_core::vm::VmTrap::BudgetExceeded.into_runtime_error()
            );
        }
    }
}

#[test]
fn fb_member_override_failure_preserves_the_previous_instance_fields() {
    use trust_runtime::bytecode::{
        InitializationStage, InitializationTrigger, SectionData, SectionId, StorageRole,
    };
    let source=scheduled("VAR_GLOBAL divisor : LREAL := LREAL#1.0; END_VAR FUNCTION_BLOCK Cell VAR PUBLIC first : LREAL := LREAL#3.0; second : LREAL := LREAL#4.0; END_VAR END_FUNCTION_BLOCK FUNCTION Read : LREAL VAR_STAT fb : Cell := (first := LREAL#9.0, second := LREAL#12.0 / LREAL#1.0); END_VAR Read := fb.first + fb.second; END_FUNCTION PROGRAM Main VAR result : LREAL; END_VAR result := Read(); END_PROGRAM");
    let mut module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let divisor = layout
        .entries
        .iter()
        .find(|d| strings.entries[d.name_idx as usize] == "divisor")
        .unwrap()
        .ref_idx
        .unwrap();
    let (fb_index, fb) = layout
        .entries
        .iter()
        .enumerate()
        .find(|(_, d)| {
            d.role == StorageRole::Static
                && d.source_name_idx
                    .is_some_and(|n| strings.entries[n as usize] == "fb")
        })
        .unwrap();
    let fb_name = strings.entries[fb.name_idx as usize].clone();
    let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
        panic!("initializers")
    };
    let action = index
        .entries
        .iter()
        .find(|entry| {
            entry.declaration_idx == Some(fb_index as u32)
                && entry.stage == InitializationStage::Explicit
                && entry.trigger == InitializationTrigger::AfterRestart
        })
        .unwrap()
        .clone();
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("code")
    };
    let body =
        &mut code[action.code_offset as usize..(action.code_offset + action.code_length) as usize];
    let loads: Vec<_> = body
        .windows(6)
        .enumerate()
        .filter(|(_, bytes)| bytes[0] == 0x10 && bytes[5] == 0x43)
        .map(|(offset, _)| offset)
        .collect();
    assert_eq!(
        loads.len(),
        1,
        "one literal denominator immediately precedes DIV"
    );
    // This broader wire case is intentional: mutable scalar initializers are not
    // source-admitted. Replace only the denominator with a visible global read.
    body[loads[0]] = 0x20;
    body[loads[0] + 1..loads[0] + 5].copy_from_slice(&divisor.to_le_bytes());
    module
        .validated_source_free(Default::default())
        .expect("static FB override retains both valid lifecycle plans");
    let bytes = module.encode().unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(field(state.storage(), "result"), Value::LReal(21.0));
    state.restart(RestartMode::Cold).unwrap();
    state.execute_cycle(Duration::from_millis(20)).unwrap();
    assert_eq!(field(state.storage(), "result"), Value::LReal(21.0));
    let fb = instance(state.storage().get_global(&fb_name));
    assert_eq!(
        state.storage().get_instance_var(fb, "first"),
        Some(&Value::LReal(9.0))
    );
    assert_eq!(
        state.storage().get_instance_var(fb, "second"),
        Some(&Value::LReal(12.0))
    );
    state.restart(RestartMode::Cold).unwrap();
    state.write_global("divisor", Value::LReal(0.0)).unwrap();
    assert!(state.execute_cycle(Duration::from_millis(30)).is_err());
    let fb = instance(state.storage().get_global(&fb_name));
    assert_eq!(
        state.storage().get_instance_var(fb, "first"),
        Some(&Value::LReal(3.0))
    );
    assert_eq!(
        state.storage().get_instance_var(fb, "second"),
        Some(&Value::LReal(4.0))
    );
    assert_eq!(field(state.storage(), "result"), Value::LReal(0.0));
}

#[test]
fn explicit_null_parameter_does_not_invoke_the_omitted_reference_default() {
    let source = scheduled(
        r#"
VAR_GLOBAL global_value : INT := INT#17; END_VAR
FUNCTION ReadRef : INT
VAR_INPUT ref_arg : REF_TO INT := REF(global_value); END_VAR
IF ref_arg = NULL THEN ReadRef := INT#-1; ELSE ReadRef := ref_arg^; END_IF;
END_FUNCTION
PROGRAM Main
VAR omitted : INT; supplied_null : INT; supplied_reference : INT; END_VAR
omitted := ReadRef();
supplied_null := ReadRef(ref_arg := NULL);
supplied_reference := ReadRef(ref_arg := REF(global_value));
END_PROGRAM
"#,
    );
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    for (ms, value) in [(10, 17), (20, 23)] {
        state
            .write_global("global_value", Value::Int(value))
            .unwrap();
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
        assert_eq!(field(state.storage(), "omitted"), Value::Int(value));
        assert_eq!(field(state.storage(), "supplied_null"), Value::Int(-1));
        assert_eq!(
            field(state.storage(), "supplied_reference"),
            Value::Int(value)
        );
    }
}

#[test]
fn dynamically_constructed_three_level_class_keeps_ancestors_and_method_statics_alive() {
    let source = scheduled(
        r#"
CLASS A
VAR PUBLIC base_value : INT := INT#5; END_VAR
METHOD PUBLIC Tick : INT
VAR_STAT calls : INT := INT#2; END_VAR
calls := calls + INT#1;
Tick := base_value + calls;
END_METHOD
END_CLASS
CLASS B EXTENDS A
VAR PUBLIC middle_value : INT := INT#7; END_VAR
END_CLASS
CLASS C EXTENDS B
VAR PUBLIC leaf_value : INT := INT#11; END_VAR
END_CLASS
VAR_GLOBAL reference_placeholder : C; END_VAR
FUNCTION Exercise : INT
VAR
    object : C;
    base : REF_TO A := REF(reference_placeholder);
    first : INT;
    second : INT;
END_VAR
base^.base_value := INT#6;
first := object.Tick();
second := object.Tick();
Exercise := first * INT#100 + second * INT#10 + object.middle_value + object.leaf_value;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Exercise();
END_PROGRAM
"#,
    );
    let mut module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    replace_global_with_local_reference(&mut module, "Exercise", "object", Some("base"));
    module
        .validated_source_free(Default::default())
        .expect("typed base reference initializer is admitted by the wire contract");
    let bytes = module.encode().unwrap();
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let persistent_instances = state.storage().instances().len();
    for ms in [10, 20, 30] {
        state.execute_cycle(Duration::from_millis(ms)).unwrap();
        assert_eq!(field(state.storage(), "result"), Value::Int(1018));
        assert_eq!(
            state.storage().instances().len(),
            persistent_instances,
            "local inheritance chain retires with the frame"
        );
        assert!(state.fault().is_none());
    }
}

#[test]
fn repeated_aggregate_loads_exhaust_the_copy_allocation_budget_before_final_commit() {
    let source=scheduled("PROGRAM Main VAR data : ARRAY[1..128] OF LREAL; copy : ARRAY[1..128] OF LREAL; index : DINT; committed : BOOL; END_VAR FOR index := DINT#1 TO DINT#100000 DO copy := data; END_FOR; committed := TRUE; END_PROGRAM");
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_construction_bytes: 1024 * 1024,
            max_work: 10_000_000,
            ..Default::default()
        },
    )
    .unwrap();
    let mut state = prepared
        .instantiate(0)
        .expect("construction fits the copy budget");
    let error = state.execute_cycle(Duration::from_millis(10)).unwrap_err();
    assert_eq!(
        error,
        trust_runtime_core::vm::VmTrap::BudgetExceeded.into_runtime_error()
    );
    assert_eq!(field(state.storage(), "committed"), Value::Bool(false));
    assert_eq!(state.fault(), Some(&error));
}

#[test]
fn wall_clock_capability_is_admitted_before_initializer_execution() {
    let source =
        scheduled("PROGRAM Main VAR sampled : DT; END_VAR sampled := CURRENT_DT(); END_PROGRAM");
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    assert!(
        prepared.instantiate(0).is_err(),
        "logical-only composition cannot admit wall-clock imports"
    );
    struct Clock(Cell<usize>);
    impl ExecutionServices for Clock {
        fn deadline_exceeded(&self) -> bool {
            false
        }
        fn has_wall_clock(&self) -> bool {
            true
        }
        fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
            self.0.set(self.0.get() + 1);
            Ok(DateTimeValue::new(1234))
        }
    }
    let clock = Clock(Cell::new(0));
    let mut state = prepared.instantiate_with_services(0, &clock).unwrap();
    assert_eq!(clock.0.get(), 0);
    state.execute_cycle(Duration::from_millis(10)).unwrap();
    assert_eq!(
        field(state.storage(), "sampled"),
        Value::Dt(DateTimeValue::new(1234))
    );
    assert_eq!(clock.0.get(), 1);
}

#[test]
fn scalar_construction_budget_is_cumulative_across_repeated_frame_entries() {
    let source=scheduled("FUNCTION Work : INT VAR local : INT := INT#1; END_VAR Work := local; END_FUNCTION PROGRAM Main VAR index : DINT; result : INT; committed : BOOL; END_VAR FOR index := DINT#1 TO DINT#1000 DO result := Work(); END_FOR; committed := TRUE; END_PROGRAM");
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(
        &bytes,
        PreparationLimits {
            max_construction_values: 100,
            ..Default::default()
        },
    )
    .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let error = state.execute_cycle(Duration::from_millis(10)).unwrap_err();
    assert_eq!(
        error,
        trust_runtime_core::vm::VmTrap::BudgetExceeded.into_runtime_error()
    );
    assert_eq!(field(state.storage(), "committed"), Value::Bool(false));
}

#[test]
fn native_split_late_output_overflow_rolls_back_earlier_outputs() {
    let source = scheduled(
        r#"
PROGRAM Main
VAR
    hour_value : INT := INT#7;
    minute_value : INT := INT#8;
    second_value : INT := INT#9;
    millis_value : USINT := USINT#10;
END_VAR
SPLIT_TOD(TOD#12:34:56.789, hour_value, minute_value, second_value, millis_value);
END_PROGRAM
"#,
    );
    let bytes = artifact(&source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let error = state.execute_cycle(Duration::from_millis(10)).unwrap_err();
    assert_eq!(error, RuntimeError::Overflow);
    for (name, expected) in [
        ("hour_value", Value::Int(7)),
        ("minute_value", Value::Int(8)),
        ("second_value", Value::Int(9)),
        ("millis_value", Value::USInt(10)),
    ] {
        assert_eq!(field(state.storage(), name), expected);
    }
    assert_eq!(state.fault(), Some(&RuntimeError::Overflow));
}

#[test]
fn sparse_native_variadic_import_is_rejected_before_argument_expansion() {
    use trust_runtime::bytecode::{SectionData, SectionId};
    let source = scheduled(
        "PROGRAM Main VAR result : INT; END_VAR result := MAX(INT#1, INT#2); END_PROGRAM",
    );
    let mut module = CompileSession::from_source(&source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section_mut(SectionId::StringTable) else {
        panic!("strings")
    };
    let symbol = strings
        .entries
        .iter_mut()
        .find(|name| name.starts_with("MAX|"))
        .unwrap();
    *symbol = "MAX|E:IN1|E:IN100000000".into();
    let bytes = module.encode().unwrap();
    let decoded = trust_runtime_core::bytecode::BytecodeModule::decode(&bytes).unwrap();
    decoded
        .validated_source_free(Default::default())
        .expect("wire validity does not size variadic binding vectors");
    assert!(PreparedModule::from_decoded(&decoded, PreparationLimits::default()).is_err());
}
