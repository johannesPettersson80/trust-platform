use std::fmt::Write as _;

use trust_runtime::bytecode::{
    BytecodeModule, PouEntry, PouKind, RefEntry, RefLocation, SectionData, SectionId, StringTable,
};
use trust_runtime::execution_backend::{
    ExecutionBackend, VmRegisterProfileSnapshot, VmTier1SpecializedExecutorSnapshot,
};
use trust_runtime::harness::{bytecode_module_from_source, TestHarness};
use trust_runtime::memory::InstanceId;
use trust_runtime::value::Value;

const TIER1_HOT_CYCLES: usize = 70;

const CLEAN_LOCAL_REF_SOURCE: &str = r#"
VAR_GLOBAL
    gr : REF_TO INT;
END_VAR

FUNCTION Leak : INT
Leak := 42;
END_FUNCTION

PROGRAM Main
VAR_EXTERNAL
    gr : REF_TO INT;
END_VAR
VAR
    observed : INT;
    read_via_ref : INT;
END_VAR
observed := Leak();
read_via_ref := gr^;
END_PROGRAM
"#;

const OWNER_DRIFT_SOURCE: &str = r#"
PROGRAM Main
VAR
    target : DINT := DINT#1;
    observed : DINT := DINT#0;
END_VAR
target := DINT#123;
observed := target;
END_PROGRAM
"#;

const VALIDATOR_DATA_SOURCE: &str = r#"
PROGRAM Main
VAR
    target : DINT := DINT#0;
    flag : BOOL := FALSE;
    observed : DINT := DINT#0;
END_VAR
target := DINT#1;
flag := TRUE;
observed := target;
END_PROGRAM
"#;

const PARAM_VALIDATOR_SOURCE: &str = r#"
FUNCTION_BLOCK Worker
VAR_INPUT
    x : DINT;
END_VAR
VAR_OUTPUT
    y : DINT;
END_VAR
y := x;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : Worker;
    target : DINT := DINT#0;
END_VAR
fb(x := DINT#7);
target := fb.y;
END_PROGRAM
"#;

const CALL_VALIDATOR_SOURCE: &str = r#"
FUNCTION Foo : DINT
Foo := DINT#1;
END_FUNCTION

PROGRAM Main
VAR
    target : DINT := DINT#0;
END_VAR
target := DINT#0;
END_PROGRAM
"#;

#[derive(Clone, Copy, Debug)]
enum VmExecutionPath {
    Stack,
    Register,
    Tier1,
}

impl VmExecutionPath {
    fn label(self) -> &'static str {
        match self {
            Self::Stack => "stack",
            Self::Register => "register-ir",
            Self::Tier1 => "tier1",
        }
    }
}

#[derive(Debug)]
struct ConversionObservation {
    path: &'static str,
    cycles_run: usize,
    errors: Vec<String>,
    values: Vec<(&'static str, Option<Value>)>,
    register_profile: VmRegisterProfileSnapshot,
    tier1_profile: Option<VmTier1SpecializedExecutorSnapshot>,
}

#[derive(Debug, Clone, Copy)]
struct ValidatorDataRefs {
    target_ref: u32,
    dint_one_const: u32,
    bool_true_const: u32,
}

fn bytecode_vm_harness(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile harness");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select bytecode VM backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    harness
}

fn bytecode_vm_harness_for_path(source: &str, path: VmExecutionPath) -> TestHarness {
    let mut harness = bytecode_vm_harness(source);
    let runtime = harness.runtime_mut();
    runtime.set_vm_register_profile_enabled(true);
    runtime.reset_vm_register_profile();
    runtime.set_vm_tier1_specialized_executor_enabled(matches!(path, VmExecutionPath::Tier1));
    runtime.reset_vm_tier1_specialized_executor();
    if matches!(path, VmExecutionPath::Stack) {
        let _ = runtime.enable_debug();
    }
    harness
}

fn observe_conversion_case(
    source: &str,
    path: VmExecutionPath,
    outputs: &[&'static str],
) -> ConversionObservation {
    let mut harness = bytecode_vm_harness_for_path(source, path);
    let target_cycles = if matches!(path, VmExecutionPath::Tier1) {
        TIER1_HOT_CYCLES
    } else {
        1
    };
    let mut errors = Vec::new();
    let mut cycles_run = 0;
    for _ in 0..target_cycles {
        cycles_run += 1;
        let cycle = harness.cycle();
        if !cycle.errors.is_empty() {
            errors = cycle
                .errors
                .iter()
                .map(|error| format!("{error:?}"))
                .collect();
            break;
        }
    }

    ConversionObservation {
        path: path.label(),
        cycles_run,
        errors,
        values: outputs
            .iter()
            .map(|name| (*name, harness.get_output(name)))
            .collect(),
        register_profile: harness.runtime().vm_register_profile_snapshot(),
        tier1_profile: matches!(path, VmExecutionPath::Tier1)
            .then(|| harness.runtime().vm_tier1_specialized_executor_snapshot()),
    }
}

fn assert_stack_path(observation: &ConversionObservation, failures: &mut Vec<String>) {
    if observation.register_profile.register_programs_executed != 0 {
        failures.push(format!(
            "{} expected stack-only execution, got register profile {:?}",
            observation.path, observation.register_profile
        ));
    }
    if observation.register_profile.register_program_fallbacks == 0 {
        failures.push(format!(
            "{} expected debug-mode fallback, got register profile {:?}",
            observation.path, observation.register_profile
        ));
    }
    if !observation
        .register_profile
        .fallback_reasons
        .iter()
        .any(|reason| reason.reason == "debug_mode")
    {
        failures.push(format!(
            "{} expected debug_mode fallback reason, got register profile {:?}",
            observation.path, observation.register_profile
        ));
    }
}

fn assert_register_path(observation: &ConversionObservation, failures: &mut Vec<String>) {
    if observation.register_profile.register_program_fallbacks != 0 {
        failures.push(format!(
            "{} expected no register fallback, got register profile {:?}",
            observation.path, observation.register_profile
        ));
    }
    if observation.errors.is_empty() && observation.register_profile.register_programs_executed == 0
    {
        failures.push(format!(
            "{} expected register execution, got register profile {:?}",
            observation.path, observation.register_profile
        ));
    }
    if !observation.errors.is_empty() && observation.register_profile.hot_blocks.is_empty() {
        failures.push(format!(
            "{} trapped before visible register block activity, got register profile {:?}",
            observation.path, observation.register_profile
        ));
    }
}

fn assert_tier1_path(observation: &ConversionObservation, failures: &mut Vec<String>) {
    let Some(profile) = &observation.tier1_profile else {
        failures.push(format!("{} missing tier1 profile", observation.path));
        return;
    };
    if !profile.enabled {
        failures.push(format!(
            "{} tier1 was not enabled: {profile:?}",
            observation.path
        ));
    }
    if observation.errors.is_empty() {
        if profile.compile_successes == 0 {
            failures.push(format!(
                "{} expected at least one tier1 compile success: {profile:?}",
                observation.path
            ));
        }
        if profile.block_executions == 0 {
            failures.push(format!(
                "{} expected at least one tier1 block execution: {profile:?}",
                observation.path
            ));
        }
        if profile.compile_failures != 0 {
            failures.push(format!(
                "{} unexpected tier1 compile failures: {profile:?}",
                observation.path
            ));
        }
        if profile.deopt_count != 0 {
            failures.push(format!(
                "{} unexpected tier1 deopts: {profile:?}",
                observation.path
            ));
        }
    }
    assert_register_path(observation, failures);
}

fn assert_path_coverage(observation: &ConversionObservation, failures: &mut Vec<String>) {
    match observation.path {
        "stack" => assert_stack_path(observation, failures),
        "register-ir" => assert_register_path(observation, failures),
        "tier1" => assert_tier1_path(observation, failures),
        other => failures.push(format!("unknown VM path label {other}")),
    }
}

fn assert_expected_conversion_results(
    observations: &[ConversionObservation],
    expected: &[(&'static str, Value)],
) {
    let mut failures = Vec::new();
    for observation in observations {
        assert_path_coverage(observation, &mut failures);
        if !observation.errors.is_empty() {
            failures.push(format!(
                "{} expected no cycle errors, got {:?}",
                observation.path, observation.errors
            ));
        }
        for (name, expected_value) in expected {
            let actual = observation
                .values
                .iter()
                .find_map(|(actual_name, value)| (*actual_name == *name).then_some(value))
                .cloned()
                .flatten();
            if actual.as_ref() != Some(expected_value) {
                failures.push(format!(
                    "{} expected {name} = {expected_value:?}, got {actual:?}",
                    observation.path
                ));
            }
        }
    }

    if !failures.is_empty() {
        let mut report = String::new();
        for failure in &failures {
            let _ = writeln!(report, "- {failure}");
        }
        let _ = writeln!(report, "\nobservations:");
        for observation in observations {
            let _ = writeln!(
                report,
                "{} cycles_run={}",
                observation.path, observation.cycles_run
            );
            let _ = writeln!(report, "{observation:#?}");
        }
        panic!("{report}");
    }
}

fn mutate_leak_to_store_local_ref(module: &mut BytecodeModule) -> Result<String, String> {
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings.clone(),
        _ => return Err("missing string table".to_string()),
    };
    let leak = find_function_pou(module, &strings, "Leak")?.clone();
    if leak.local_ref_count == 0 {
        return Err("Leak has no local ref range to mutate".to_string());
    }
    let local_ref_idx = leak.local_ref_start;
    let global_ref_idx = match module.section(SectionId::RefTable) {
        Some(SectionData::RefTable(refs)) => {
            refs.entries
                .iter()
                .position(|entry| entry.location == RefLocation::Global)
                .ok_or_else(|| "no global ref entry".to_string())? as u32
        }
        _ => return Err("missing ref table".to_string()),
    };
    let original_body = body_bytes(module, &leak)?.to_vec();
    let mut mutated_body = original_body.clone();
    mutated_body.push(0x22);
    mutated_body.extend_from_slice(&local_ref_idx.to_le_bytes());
    mutated_body.push(0x21);
    mutated_body.extend_from_slice(&global_ref_idx.to_le_bytes());
    replace_pou_body(module, leak.id, &mutated_body)?;
    module.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });
    Ok(format!(
        "Leak body len {} -> {}; appended LOAD_REF_ADDR local_ref_idx={} then STORE_REF global_ref_idx={}",
        original_body.len(),
        mutated_body.len(),
        local_ref_idx,
        global_ref_idx
    ))
}

fn find_function_pou<'a>(
    module: &'a BytecodeModule,
    strings: &StringTable,
    name: &str,
) -> Result<&'a PouEntry, String> {
    find_pou_by_kind(module, strings, name, PouKind::Function)
}

fn find_program_pou<'a>(
    module: &'a BytecodeModule,
    strings: &StringTable,
    name: &str,
) -> Result<&'a PouEntry, String> {
    find_pou_by_kind(module, strings, name, PouKind::Program)
}

fn find_pou_by_kind<'a>(
    module: &'a BytecodeModule,
    strings: &StringTable,
    name: &str,
    kind: PouKind,
) -> Result<&'a PouEntry, String> {
    let index = match module.section(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index,
        _ => return Err("missing POU index".to_string()),
    };
    index
        .entries
        .iter()
        .find(|entry| {
            entry.kind == kind
                && strings
                    .entries
                    .get(entry.name_idx as usize)
                    .is_some_and(|entry_name| entry_name.eq_ignore_ascii_case(name))
        })
        .ok_or_else(|| format!("{name} POU entry not found"))
}

fn body_bytes<'a>(module: &'a BytecodeModule, entry: &PouEntry) -> Result<&'a [u8], String> {
    let bodies = match module.section(SectionId::PouBodies) {
        Some(SectionData::PouBodies(bodies)) => bodies,
        _ => return Err("missing POU bodies".to_string()),
    };
    let start = entry.code_offset as usize;
    let end = start + entry.code_length as usize;
    bodies
        .get(start..end)
        .ok_or_else(|| "POU body range out of bounds".to_string())
}

fn replace_pou_body(
    module: &mut BytecodeModule,
    pou_id: u32,
    new_body: &[u8],
) -> Result<(), String> {
    let new_offset = match module.section_mut(SectionId::PouBodies) {
        Some(SectionData::PouBodies(code)) => {
            let offset = code.len() as u32;
            code.extend_from_slice(new_body);
            offset
        }
        _ => return Err("missing POU bodies".to_string()),
    };
    match module.section_mut(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => {
            let entry = index
                .entries
                .iter_mut()
                .find(|entry| entry.id == pou_id)
                .ok_or_else(|| "mutated POU id not found".to_string())?;
            entry.code_offset = new_offset;
            entry.code_length = new_body.len() as u32;
            Ok(())
        }
        _ => Err("missing POU index".to_string()),
    }
}

fn mutate_main_refs_for_owner_drift(
    module: &mut BytecodeModule,
    main_id: InstanceId,
    alien_id: InstanceId,
) -> Result<String, String> {
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings.clone(),
        _ => return Err("missing string table".to_string()),
    };
    let main = find_program_pou(module, &strings, "Main")?.clone();
    let body = body_bytes(module, &main)?.to_vec();
    let ref_indexes = ref_operands(&body)?;
    let mut patched_indexes = Vec::new();

    let second_owner_ref = match module.section_mut(SectionId::RefTable) {
        Some(SectionData::RefTable(refs)) => {
            for ref_idx in &ref_indexes {
                let Some(entry) = refs.entries.get_mut(*ref_idx as usize) else {
                    continue;
                };
                if entry.location == RefLocation::Instance && entry.owner_id == main_id.0 {
                    entry.owner_id = alien_id.0;
                    patched_indexes.push(*ref_idx);
                }
            }
            let second_owner_ref = refs.entries.len() as u32;
            refs.entries.push(RefEntry {
                location: RefLocation::Instance,
                owner_id: main_id.0,
                offset: 0,
                segments: Vec::new(),
            });
            second_owner_ref
        }
        _ => return Err("missing ref table".to_string()),
    };

    let mut mutated_body = body.clone();
    mutated_body.push(0x20);
    mutated_body.extend_from_slice(&second_owner_ref.to_le_bytes());
    replace_pou_body(module, main.id, &mutated_body)?;
    module.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });
    Ok(format!(
        "main_id={} alien_id={} patched_ref_indexes={patched_indexes:?} appended_second_owner_ref={} body_len {} -> {}",
        main_id.0,
        alien_id.0,
        second_owner_ref,
        body.len(),
        mutated_body.len()
    ))
}

fn ref_operands(code: &[u8]) -> Result<Vec<u32>, String> {
    let mut refs = Vec::new();
    let mut pc = 0usize;
    while pc < code.len() {
        let opcode = code[pc];
        pc += 1;
        let operand_len = opcode_operand_len(opcode)
            .ok_or_else(|| format!("unknown opcode 0x{opcode:02X} at {}", pc - 1))?;
        if pc + operand_len > code.len() {
            return Err(format!("truncated opcode 0x{opcode:02X}"));
        }
        if matches!(opcode, 0x20..=0x22) && operand_len == 4 {
            let bytes = [code[pc], code[pc + 1], code[pc + 2], code[pc + 3]];
            refs.push(u32::from_le_bytes(bytes));
        }
        pc += operand_len;
    }
    Ok(refs)
}

fn opcode_operand_len(opcode: u8) -> Option<usize> {
    match opcode {
        0x00
        | 0x01
        | 0x06
        | 0x11
        | 0x12
        | 0x13
        | 0x14
        | 0x15
        | 0x23
        | 0x24
        | 0x25
        | 0x31
        | 0x32
        | 0x33
        | 0x40..=0x4E
        | 0x50..=0x55 => Some(0),
        0x02..=0x05 | 0x07 | 0x10 | 0x20..=0x22 | 0x30 | 0x60 | 0x62 | 0x63 | 0x70 => Some(4),
        0x08 => Some(8),
        0x09 => Some(12),
        0x16 => Some(1),
        _ => None,
    }
}

fn validator_data_refs(module: &BytecodeModule) -> Result<ValidatorDataRefs, String> {
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings.clone(),
        _ => return Err("missing string table".to_string()),
    };
    let main = find_program_pou(module, &strings, "Main")?;
    let body = body_bytes(module, main)?;
    let target_ref = ref_by_instance_offset(module, 0)?;
    let flag_ref = ref_by_instance_offset(module, 1)?;
    Ok(ValidatorDataRefs {
        target_ref,
        dint_one_const: const_stored_to_ref(body, target_ref)?,
        bool_true_const: const_stored_to_ref(body, flag_ref)?,
    })
}

fn ref_by_instance_offset(module: &BytecodeModule, offset: u32) -> Result<u32, String> {
    let refs = match module.section(SectionId::RefTable) {
        Some(SectionData::RefTable(refs)) => refs,
        _ => return Err("missing ref table".to_string()),
    };
    refs.entries
        .iter()
        .enumerate()
        .find(|(_, entry)| {
            entry.location == RefLocation::Instance
                && entry.offset == offset
                && entry.segments.is_empty()
        })
        .map(|(idx, _)| idx as u32)
        .ok_or_else(|| format!("instance ref for offset {offset} not found"))
}

fn const_stored_to_ref(code: &[u8], ref_idx: u32) -> Result<u32, String> {
    let mut pc = 0usize;
    while pc + 10 <= code.len() {
        if code[pc] == 0x10 && code[pc + 5] == 0x21 {
            let const_idx =
                u32::from_le_bytes([code[pc + 1], code[pc + 2], code[pc + 3], code[pc + 4]]);
            let store_ref =
                u32::from_le_bytes([code[pc + 6], code[pc + 7], code[pc + 8], code[pc + 9]]);
            if store_ref == ref_idx {
                return Ok(const_idx);
            }
        }
        pc += 1;
    }
    Err(format!(
        "no LOAD_CONST/STORE_REF sequence found for ref {ref_idx}"
    ))
}

fn load_const(const_idx: u32) -> Vec<u8> {
    let mut code = vec![0x10];
    code.extend_from_slice(&const_idx.to_le_bytes());
    code
}

fn store_ref(ref_idx: u32) -> Vec<u8> {
    let mut code = vec![0x21];
    code.extend_from_slice(&ref_idx.to_le_bytes());
    code
}

fn replace_main_body(module: &mut BytecodeModule, new_body: &[u8]) -> Result<(), String> {
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings.clone(),
        _ => return Err("missing string table".to_string()),
    };
    let main = find_program_pou(module, &strings, "Main")?.clone();
    replace_pou_body(module, main.id, new_body)?;
    module.sections.retain(|section| {
        section.id != SectionId::DebugMap.as_raw()
            && section.id != SectionId::DebugStringTable.as_raw()
    });
    Ok(())
}

fn expect_validator_rejects_main_body(name: &str, new_body: Vec<u8>) {
    let mut module =
        bytecode_module_from_source(VALIDATOR_DATA_SOURCE).expect("compile validator data module");
    let refs = validator_data_refs(&module).expect("collect validator data refs");
    replace_main_body(&mut module, &new_body).expect("replace Main body");
    let validate_result = module.validate();
    assert!(
        validate_result.is_err(),
        "expected bytecode validator to reject {name}; refs={refs:?}; \
         body={new_body:02X?}; validate={validate_result:?}"
    );
}

fn mutate_worker_first_param_direction(
    module: &mut BytecodeModule,
    direction: u8,
) -> Result<(), String> {
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings.clone(),
        _ => return Err("missing string table".to_string()),
    };
    let worker_id = find_pou_by_kind(module, &strings, "Worker", PouKind::FunctionBlock)?.id;
    match module.section_mut(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => {
            let entry = index
                .entries
                .iter_mut()
                .find(|entry| entry.id == worker_id)
                .ok_or_else(|| "Worker POU id not found".to_string())?;
            let param = entry
                .params
                .first_mut()
                .ok_or_else(|| "Worker has no params".to_string())?;
            param.direction = direction;
            Ok(())
        }
        _ => Err("missing POU index".to_string()),
    }
}

#[test]
fn declared_real_keeps_real_semantics_after_integer_assignment() {
    let source = r#"
PROGRAM Main
VAR
    i : INT := INT#2;
    r : REAL := REAL#0.0;
    r2 : REAL := REAL#0.0;
END_VAR
r := INT#1;
r2 := r / i;
END_PROGRAM
"#;

    let mut harness = bytecode_vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "declared REAL assignment/division must not fault: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("r"), Some(Value::Real(1.0)));
    assert_eq!(harness.get_output("r2"), Some(Value::Real(0.5)));
}

#[test]
fn declared_dint_keeps_dint_width_after_int_assignment() {
    let source = r#"
PROGRAM Main
VAR
    i : INT := INT#200;
    d : DINT := DINT#0;
    e : DINT := DINT#0;
END_VAR
d := i;
e := d * d;
END_PROGRAM
"#;

    let mut harness = bytecode_vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "declared DINT arithmetic must not overflow at INT width: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("d"), Some(Value::DInt(200)));
    assert_eq!(harness.get_output("e"), Some(Value::DInt(40_000)));
}

#[test]
fn declared_real_conversion_matches_iec_on_stack_register_and_tier1_paths() {
    let source = r#"
PROGRAM Main
VAR
    i : INT := INT#2;
    r : REAL := REAL#0.0;
    r2 : REAL := REAL#0.0;
END_VAR
r := INT#1;
r2 := r / i;
END_PROGRAM
"#;

    let observations = [
        observe_conversion_case(source, VmExecutionPath::Stack, &["r", "r2"]),
        observe_conversion_case(source, VmExecutionPath::Register, &["r", "r2"]),
        observe_conversion_case(source, VmExecutionPath::Tier1, &["r", "r2"]),
    ];

    assert_expected_conversion_results(
        &observations,
        &[("r", Value::Real(1.0)), ("r2", Value::Real(0.5))],
    );
}

#[test]
fn declared_dint_conversion_matches_iec_on_stack_register_and_tier1_paths() {
    let source = r#"
PROGRAM Main
VAR
    i : INT := INT#200;
    d : DINT := DINT#0;
    e : DINT := DINT#0;
END_VAR
d := i;
e := d * d;
END_PROGRAM
"#;

    let observations = [
        observe_conversion_case(source, VmExecutionPath::Stack, &["d", "e"]),
        observe_conversion_case(source, VmExecutionPath::Register, &["d", "e"]),
        observe_conversion_case(source, VmExecutionPath::Tier1, &["d", "e"]),
    ];

    assert_expected_conversion_results(
        &observations,
        &[("d", Value::DInt(200)), ("e", Value::DInt(40_000))],
    );
}

#[test]
fn parameter_copy_in_materializes_declared_numeric_widening() {
    let source = r#"
FUNCTION Half : REAL
VAR_INPUT
    x : REAL;
    divisor : INT;
END_VAR
Half := x / divisor;
END_FUNCTION

FUNCTION WideProduct : DINT
VAR_INPUT
    x : DINT;
END_VAR
WideProduct := x * x;
END_FUNCTION

FUNCTION_BLOCK Scale
VAR_INPUT
    r : REAL;
    divisor : INT;
    d : DINT;
END_VAR
VAR_OUTPUT
    r2 : REAL;
    e : DINT;
END_VAR
r2 := r / divisor;
e := d * d;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    one : INT := INT#1;
    two : INT := INT#2;
    wideSource : INT := INT#200;
    fb : Scale;
    fbHalf : REAL;
    fbProduct : DINT;
    fnHalf : REAL;
    fnProduct : DINT;
END_VAR
fb(r := one, divisor := two, d := wideSource);
fbHalf := fb.r2;
fbProduct := fb.e;
fnHalf := Half(one, two);
fnProduct := WideProduct(wideSource);
END_PROGRAM
"#;

    let mut harness = bytecode_vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "declared parameter widening must not fault: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("fbHalf"), Some(Value::Real(0.5)));
    assert_eq!(harness.get_output("fbProduct"), Some(Value::DInt(40_000)));
    assert_eq!(harness.get_output("fnHalf"), Some(Value::Real(0.5)));
    assert_eq!(harness.get_output("fnProduct"), Some(Value::DInt(40_000)));
}

#[test]
fn ref_return_name_is_rejected_before_runtime_lowering() {
    let source = r#"
VAR_GLOBAL
    gr : REF_TO INT;
    observed : INT;
END_VAR

FUNCTION Leak : INT
VAR_EXTERNAL
    gr : REF_TO INT;
END_VAR
Leak := 42;
gr := REF(Leak);
END_FUNCTION

PROGRAM Main
VAR
    tmp : INT;
END_VAR
tmp := Leak();
observed := gr^;
END_PROGRAM
"#;

    match TestHarness::from_source(source) {
        Err(error) => assert!(
            error.to_string().contains(
                "error[E202]: REF cannot take a reference to a function or method return variable"
            ),
            "expected the REF(return variable) diagnostic, got: {error}"
        ),
        Ok(mut harness) => {
            harness
                .runtime_mut()
                .set_execution_backend(ExecutionBackend::BytecodeVm)
                .expect("select bytecode VM backend");
            harness
                .runtime_mut()
                .restart(trust_runtime::RestartMode::Cold)
                .expect("restart runtime");
            let cycle = harness.cycle();
            panic!(
                "expected REF(return_name) to fail before runtime lowering; build succeeded; \
                 cycle_errors={:?}; gr={:?}; observed={:?}",
                cycle.errors,
                harness.get_output("gr"),
                harness.get_output("observed")
            );
        }
    }
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-008"]
fn crafted_multi_owner_instance_refs_are_rejected_before_execution() {
    let mut harness = TestHarness::from_source(OWNER_DRIFT_SOURCE).expect("compile harness");
    let main_id = match harness.runtime().storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main global instance, got {other:?}"),
    };
    let alien_id = harness.runtime_mut().storage_mut().create_instance("Alien");
    harness
        .runtime_mut()
        .storage_mut()
        .set_instance_var(alien_id, "target", Value::DInt(-10));
    harness
        .runtime_mut()
        .storage_mut()
        .set_instance_var(alien_id, "observed", Value::DInt(-20));

    let mut module = bytecode_module_from_source(OWNER_DRIFT_SOURCE).expect("compile module");
    let mutation =
        mutate_main_refs_for_owner_drift(&mut module, main_id, alien_id).expect("mutate module");
    let validate_result = module.validate();
    let validate_debug = format!("{validate_result:?}");
    let bytes = module.encode().expect("encode mutated module");
    let apply_result = harness.runtime_mut().apply_bytecode_bytes(&bytes, None);
    let apply_debug = format!("{apply_result:?}");

    if validate_result.is_err() || apply_result.is_err() {
        return;
    }

    let cycle = harness.cycle();
    let storage = harness.runtime().storage();
    let main_target = storage.read_instance_field_by_offset(main_id, 0).cloned();
    let main_observed = storage.read_instance_field_by_offset(main_id, 1).cloned();
    let alien_target = storage.read_instance_field_by_offset(alien_id, 0).cloned();
    let alien_observed = storage.read_instance_field_by_offset(alien_id, 1).cloned();
    panic!(
        "expected stale or multi-owner instance bytecode to fail validation/load before execution; \
         mutation={mutation}; validate={validate_debug}; apply={apply_debug}; \
         cycle_errors={:?}; main=({main_target:?}, {main_observed:?}); \
         alien=({alien_target:?}, {alien_observed:?}); \
         harness_target={:?}; harness_observed={:?}",
        cycle.errors,
        harness.get_output("target"),
        harness.get_output("observed")
    );
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009B"]
fn validator_rejects_multi_owner_instance_ref_contract() {
    let harness = TestHarness::from_source(OWNER_DRIFT_SOURCE).expect("compile harness");
    let main_id = match harness.runtime().storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main global instance, got {other:?}"),
    };
    let alien_id = InstanceId(main_id.0.saturating_add(1));
    let mut module = bytecode_module_from_source(OWNER_DRIFT_SOURCE).expect("compile module");
    let mutation =
        mutate_main_refs_for_owner_drift(&mut module, main_id, alien_id).expect("mutate module");
    let validate_result = module.validate();
    assert!(
        validate_result.is_err(),
        "expected bytecode validator to reject stale or multi-owner instance refs; \
         mutation={mutation}; validate={validate_result:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009C"]
fn validator_rejects_stack_underflow_store_ref() {
    let module =
        bytecode_module_from_source(VALIDATOR_DATA_SOURCE).expect("compile validator data module");
    let refs = validator_data_refs(&module).expect("collect validator data refs");
    expect_validator_rejects_main_body("stack_underflow_store_ref", store_ref(refs.target_ref));
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009C"]
fn validator_rejects_leftover_stack_at_pou_return() {
    let module =
        bytecode_module_from_source(VALIDATOR_DATA_SOURCE).expect("compile validator data module");
    let refs = validator_data_refs(&module).expect("collect validator data refs");
    expect_validator_rejects_main_body("leftover_stack_at_return", load_const(refs.dint_one_const));
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009C"]
fn validator_rejects_bool_operands_for_arithmetic_opcode() {
    let module =
        bytecode_module_from_source(VALIDATOR_DATA_SOURCE).expect("compile validator data module");
    let refs = validator_data_refs(&module).expect("collect validator data refs");
    let body = [
        load_const(refs.bool_true_const),
        load_const(refs.bool_true_const),
        vec![0x40],
        store_ref(refs.target_ref),
    ]
    .concat();
    expect_validator_rejects_main_body("bool_operands_for_add", body);
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009D"]
fn validator_rejects_const_type_incompatible_with_store_ref_target() {
    let module =
        bytecode_module_from_source(VALIDATOR_DATA_SOURCE).expect("compile validator data module");
    let refs = validator_data_refs(&module).expect("collect validator data refs");
    let body = [load_const(refs.bool_true_const), store_ref(refs.target_ref)].concat();
    expect_validator_rejects_main_body("bool_const_stored_to_dint_ref", body);
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009E"]
fn validator_rejects_invalid_parameter_direction_metadata() {
    let mut module =
        bytecode_module_from_source(PARAM_VALIDATOR_SOURCE).expect("compile param module");
    mutate_worker_first_param_direction(&mut module, 99).expect("mutate direction");
    let validate_result = module.validate();
    assert!(
        validate_result.is_err(),
        "expected bytecode validator to reject invalid parameter direction 99; \
         validate={validate_result:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009E"]
fn validator_rejects_inout_parameter_bound_to_literal_argument() {
    let mut module =
        bytecode_module_from_source(PARAM_VALIDATOR_SOURCE).expect("compile param module");
    mutate_worker_first_param_direction(&mut module, 2).expect("mutate direction to IN_OUT");
    let validate_result = module.validate();
    assert!(
        validate_result.is_err(),
        "expected bytecode validator to reject IN_OUT parameter metadata when caller binds \
         a literal value argument; validate={validate_result:?}"
    );
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009F"]
fn validator_rejects_legacy_call_opcode_even_when_target_exists() {
    let mut module =
        bytecode_module_from_source(CALL_VALIDATOR_SOURCE).expect("compile call module");
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings.clone(),
        _ => panic!("missing string table"),
    };
    let foo_id = find_function_pou(&module, &strings, "Foo")
        .expect("Foo function")
        .id;
    let mut body = vec![0x05];
    body.extend_from_slice(&foo_id.to_le_bytes());
    body.push(0x06);
    replace_main_body(&mut module, &body).expect("replace Main body");
    let validate_result = module.validate();
    assert!(
        validate_result.is_err(),
        "expected bytecode validator to reject legacy CALL opcode 0x05 even when target exists; \
         foo_id={foo_id}; body={body:02X?}; validate={validate_result:?}"
    );
}

#[test]
fn unsupported_array_initializer_assignment_fails_build_instead_of_nop() {
    let source = r#"
PROGRAM Main
VAR
    target : ARRAY[0..1] OF DINT;
END_VAR
target := [DINT#1, DINT#2];
END_PROGRAM
"#;

    match TestHarness::from_source(source) {
        Err(_) => {}
        Ok(mut harness) => {
            harness
                .runtime_mut()
                .set_execution_backend(ExecutionBackend::BytecodeVm)
                .expect("select bytecode VM backend");
            harness
                .runtime_mut()
                .restart(trust_runtime::RestartMode::Cold)
                .expect("restart runtime");
            let cycle = harness.cycle();
            panic!(
                "expected unsupported array-initializer assignment to fail build instead of \
                 lowering to NOP; cycle_errors={:?}; target={:?}",
                cycle.errors,
                harness.get_output("target")
            );
        }
    }
}

#[test]
fn unsupported_struct_initializer_assignment_fails_build_instead_of_nop() {
    let source = r#"
TYPE
    Pair : STRUCT
        left : DINT;
        right : DINT;
    END_STRUCT;
END_TYPE

PROGRAM Main
VAR
    target : Pair;
END_VAR
target := (left := DINT#1, right := DINT#2);
END_PROGRAM
"#;

    match TestHarness::from_source(source) {
        Err(_) => {}
        Ok(mut harness) => {
            harness
                .runtime_mut()
                .set_execution_backend(ExecutionBackend::BytecodeVm)
                .expect("select bytecode VM backend");
            harness
                .runtime_mut()
                .restart(trust_runtime::RestartMode::Cold)
                .expect("restart runtime");
            let cycle = harness.cycle();
            panic!(
                "expected unsupported structure-initializer assignment to fail build instead of \
                 lowering to NOP; cycle_errors={:?}; target={:?}",
                cycle.errors,
                harness.get_output("target")
            );
        }
    }
}

#[test]
fn computed_subrange_assignment_fails_visibly_without_committing_out_of_range_value() {
    let source = r#"
PROGRAM Main
VAR
    source : INT := INT#100;
    limited : INT(0..10) := INT#0;
END_VAR
limited := source;
END_PROGRAM
"#;

    let mut harness = bytecode_vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        !cycle.errors.is_empty(),
        "out-of-range subrange assignment must fail visibly; limited={:?}",
        harness.get_output("limited")
    );
    assert_eq!(
        harness.get_output("limited"),
        Some(Value::Int(0)),
        "out-of-range subrange assignment must leave the prior value unchanged"
    );
}

#[test]
fn computed_subrange_fb_input_binding_fails_visibly_without_committing_out_of_range_value() {
    let source = r#"
FUNCTION_BLOCK Clamp
VAR_INPUT
    limited : INT(0..10) := INT#0;
END_VAR
VAR_OUTPUT
    observed : INT := INT#0;
END_VAR
observed := limited;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    source : INT := INT#100;
    fb : Clamp;
END_VAR
fb(limited := source);
END_PROGRAM
"#;

    let mut harness = bytecode_vm_harness(source);
    let cycle = harness.cycle();
    let storage = harness.runtime().storage();
    let main_id = match storage.get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main global instance, got {other:?}"),
    };
    let fb_id = match storage.read_instance_field_by_offset(main_id, 1) {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected Main.fb instance field, got {other:?}"),
    };
    let fb_limited = storage.read_instance_field_by_offset(fb_id, 0).cloned();
    let fb_observed = storage.read_instance_field_by_offset(fb_id, 1).cloned();
    assert!(
        !cycle.errors.is_empty(),
        "out-of-range subrange FB input binding must fail visibly; \
         fb.limited={fb_limited:?}; fb.observed={fb_observed:?}"
    );
    assert_eq!(
        fb_limited,
        Some(Value::Int(0)),
        "out-of-range subrange FB input binding must leave the prior value unchanged"
    );
}

#[test]
fn computed_subrange_ref_write_fails_visibly_without_committing_out_of_range_value() {
    let source = r#"
PROGRAM Main
VAR
    source : INT := INT#100;
    limited : INT(0..10) := INT#0;
    rLimited : REF_TO INT;
END_VAR
rLimited := REF(limited);
rLimited^ := source;
END_PROGRAM
"#;

    let mut harness = bytecode_vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        !cycle.errors.is_empty(),
        "out-of-range subrange REF write must fail visibly; limited={:?}; rLimited={:?}",
        harness.get_output("limited"),
        harness.get_output("rLimited")
    );
    assert_eq!(
        harness.get_output("limited"),
        Some(Value::Int(0)),
        "out-of-range subrange REF write must leave the prior value unchanged"
    );
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-007"]
fn crafted_frame_local_ref_cannot_persist_to_global_storage() {
    let mut module =
        bytecode_module_from_source(CLEAN_LOCAL_REF_SOURCE).expect("compile clean module");
    let mutation = mutate_leak_to_store_local_ref(&mut module).expect("mutate module");
    let validate_result = module.validate();
    let validate_debug = format!("{validate_result:?}");
    let bytes = module.encode().expect("encode mutated module");
    let mut harness = TestHarness::from_source(CLEAN_LOCAL_REF_SOURCE).expect("compile runtime");
    let apply_result = harness.runtime_mut().apply_bytecode_bytes(&bytes, None);
    let apply_debug = format!("{apply_result:?}");

    if validate_result.is_err() || apply_result.is_err() {
        return;
    }

    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select bytecode VM backend");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("restart runtime");
    let cycle = harness.cycle();
    panic!(
        "expected crafted frame-local reference persistence to fail validation/load before execution; \
         mutation={mutation}; validate={validate_debug}; apply={apply_debug}; \
         cycle_errors={:?}; gr={:?}; observed={:?}; read_via_ref={:?}",
        cycle.errors,
        harness.get_output("gr"),
        harness.get_output("observed"),
        harness.get_output("read_via_ref")
    );
}

#[test]
#[ignore = "red test for runtime-safety Phase 11 SEAM-TEST-009A"]
fn validator_rejects_persistent_frame_local_ref_escape() {
    let mut module =
        bytecode_module_from_source(CLEAN_LOCAL_REF_SOURCE).expect("compile clean module");
    let mutation = mutate_leak_to_store_local_ref(&mut module).expect("mutate module");
    let validate_result = module.validate();
    assert!(
        validate_result.is_err(),
        "expected bytecode validator to reject persistent frame-local reference escape; \
         mutation={mutation}; validate={validate_result:?}"
    );
}

#[test]
fn literal_context_coercions_stay_declared_width_and_type() {
    let source = r#"
PROGRAM Main
VAR
    r : REAL := REAL#0.0;
    half : REAL := REAL#0.0;
    d : DINT := DINT#0;
    e : DINT := DINT#0;
END_VAR
r := 1;
half := r / REAL#2.0;
d := 200;
e := d * d;
END_PROGRAM
"#;

    let mut harness = bytecode_vm_harness(source);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "literal context coercions should remain valid: {:?}",
        cycle.errors
    );
    assert_eq!(harness.get_output("r"), Some(Value::Real(1.0)));
    assert_eq!(harness.get_output("half"), Some(Value::Real(0.5)));
    assert_eq!(harness.get_output("d"), Some(Value::DInt(200)));
    assert_eq!(harness.get_output("e"), Some(Value::DInt(40_000)));
}
