#[path = "common/bytecode_helpers.rs"]
mod bytecode_helpers;

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};

use serde_json::json;
use trust_runtime::bytecode::{
    BytecodeError, BytecodeModule, ConstEntry, ParamEntry, RefEntry, RefLocation, SectionData,
    SectionId, BYTECODE_MAX_CONTAINER_BYTES as MAX_CONTAINER_BYTES,
    BYTECODE_MAX_INSTRUCTIONS as MAX_MODULE_INSTRUCTIONS,
    BYTECODE_MAX_LOCALS_PER_POU as MAX_LOCALS_PER_POU,
    BYTECODE_MAX_PARAMETERS_PER_POU as MAX_PARAMETERS_PER_POU,
    BYTECODE_MAX_REFERENCES as MAX_REFERENCES,
};
use trust_runtime::error::{RuntimeError, StableErrorCode};
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::TestHarness;
use trust_runtime::runtime_core::vm::hosted::ensure_global_call_depth;
use trust_runtime::runtime_core::vm::{
    VmTrap, VM_MAX_CALL_DEPTH as MAX_CALL_DEPTH, VM_MAX_OPERAND_STACK as MAX_OPERAND_STACK,
};
use trust_runtime::value::Value;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_VM_RESOURCE_LIMIT_CASES_001";
const CASE_FILE: &str = "verification/cases/bytecode_vm/VM_SEAM_DETERMINISM_LIMITS_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:43b9709939e005fcc9b13203f5ffdd1134f47d7ff4ce2e225fa5374b006184f2";

const EXECUTION_BUDGET_SOURCE: &str = r#"
PROGRAM Main
VAR
    i : DINT := DINT#0;
    total : DINT := DINT#0;
END_VAR
FOR i := DINT#0 TO DINT#250000 BY DINT#1 DO
    total := total + DINT#1;
END_FOR;
END_PROGRAM
"#;

#[test]
fn vm_resource_limit_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("resource-limit runner must enter workspace root");

    let mut probe = ResourceLimitProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_resource_limit_case);

    env::set_current_dir(original_dir)
        .expect("resource-limit runner must restore current directory");
    let artifact = result.expect("resource-limit case artifact must be written");
    let failed = artifact
        .cases
        .iter()
        .filter(|case| case.result != CaseResult::Passed)
        .map(|case| {
            format!(
                "{}: {}",
                case.id,
                case.observed_error.as_deref().unwrap_or("not passed")
            )
        })
        .collect::<Vec<_>>();
    assert!(
        failed.is_empty(),
        "VM resource-limit failures: {}",
        failed.join("; ")
    );
}

fn run_resource_limit_case(
    case: &CaseRecord,
    probe: &mut ResourceLimitProbe,
) -> Result<CaseExecution, String> {
    let step = only_trace_step(case)?;
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} requires a scenario", case.id))?;
    let expected = trace_string(&step.expected, "observed_outcome")?;
    let observed = observe_scenario(scenario)?;
    if observed == "accepted" {
        probe.accepted_scenarios.insert(scenario.to_string());
    }

    if observed == expected {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some(observed),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(format!(
                "expected observed_outcome={expected}, observed {observed}"
            )),
            observed_status: Some(observed),
        })
    }
}

fn observe_scenario(scenario: &str) -> Result<String, String> {
    let observed = match scenario {
        "ENCODED_CONTAINER_ABOVE_FIXED_LIMIT" => rejection_outcome(apply_oversized_container()),
        "MODULE_INSTRUCTIONS_ABOVE_FIXED_LIMIT" => {
            rejection_outcome(module_above_instruction_limit().validate())
        }
        "MODULE_REFERENCES_ABOVE_FIXED_LIMIT" => {
            rejection_outcome(module_above_reference_limit().validate())
        }
        "POU_LOCALS_ABOVE_FIXED_LIMIT" => rejection_outcome(module_above_local_limit().validate()),
        "POU_PARAMETERS_ABOVE_FIXED_LIMIT" => {
            rejection_outcome(module_above_parameter_limit().validate())
        }
        "OPERAND_STACK_ABOVE_FIXED_LIMIT" => {
            rejection_outcome(module_above_stack_limit().validate())
        }
        "CALL_DEPTH_ABOVE_FIXED_LIMIT" => rejection_outcome(ensure_global_call_depth(
            0,
            MAX_CALL_DEPTH.saturating_add(1),
        )),
        "EXECUTED_INSTRUCTIONS_ABOVE_FIXED_LIMIT" => execution_budget_outcome()?,
        "OSCAT_CONTAINER_WITHIN_FIXED_LIMITS" => oscat_outcome()?,
        other => return Err(format!("unreviewed VM resource-limit scenario {other}")),
    };
    Ok(observed.to_string())
}

fn rejection_outcome<T, E>(result: Result<T, E>) -> &'static str {
    if result.is_err() {
        "rejected"
    } else {
        "accepted"
    }
}

/// One byte past the fixed container limit: a valid base module padded with zeros.
fn oversized_container_bytes() -> Vec<u8> {
    let mut module = bytecode_helpers::base_module();
    module.flags = 0;
    let mut bytes = module.encode().expect("encode base module");
    bytes.resize(MAX_CONTAINER_BYTES.saturating_add(1), 0);
    bytes
}

fn apply_oversized_container() -> Result<(), RuntimeError> {
    trust_runtime::RuntimeBuilder::new()
        .build()
        .expect("admit empty application")
        .apply_bytecode_bytes(&oversized_container_bytes(), None)
}

/// A module whose main body decodes to exactly `count` NOP instructions.
fn module_with_instruction_count(count: usize) -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    set_main_body(&mut module, vec![0x00; count]);
    module
}

fn module_above_instruction_limit() -> BytecodeModule {
    module_with_instruction_count(MAX_MODULE_INSTRUCTIONS + 1)
}

/// A module whose REF_TABLE holds exactly `count` global references.
fn module_with_references(count: usize) -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    let references = (0..count)
        .map(|offset| RefEntry {
            location: RefLocation::Global,
            owner_id: 0,
            offset: u32::try_from(offset).expect("reference offset fits u32"),
            segments: Vec::new(),
        })
        .collect();
    set_ref_entries(&mut module, references);
    module
}

fn module_above_reference_limit() -> BytecodeModule {
    module_with_references(MAX_REFERENCES + 1)
}

/// A module whose main POU declares `local_count` local references, backed by `ref_count`
/// local REF_TABLE entries. Keeping the REF_TABLE within its own limit means only the POU
/// count is over-limit, which is what the declared-count check then reports.
fn module_with_locals(local_count: usize, ref_count: usize) -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    let references = (0..ref_count)
        .map(|offset| RefEntry {
            location: RefLocation::Local,
            owner_id: 1,
            offset: u32::try_from(offset).expect("local offset fits u32"),
            segments: Vec::new(),
        })
        .collect();
    set_ref_entries(&mut module, references);
    main_pou_mut(&mut module).local_ref_count =
        u32::try_from(local_count).expect("local count fits u32");
    module
}

fn module_above_local_limit() -> BytecodeModule {
    module_with_locals(MAX_LOCALS_PER_POU + 1, 0)
}

/// A module whose main POU declares exactly `count` parameters.
fn module_with_parameters(count: usize) -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    main_pou_mut(&mut module).params = (0..count)
        .map(|_| ParamEntry {
            name_idx: 0,
            type_id: 0,
            direction: 0,
            default_const_idx: None,
        })
        .collect();
    module
}

fn module_above_parameter_limit() -> BytecodeModule {
    module_with_parameters(MAX_PARAMETERS_PER_POU + 1)
}

/// A module whose main body pushes `depth` constants, pops them all, and returns.
fn module_with_stack_depth(depth: usize) -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    if let Some(SectionData::ConstPool(pool)) = module.section_mut(SectionId::ConstPool) {
        pool.entries.push(ConstEntry {
            type_id: 0,
            payload: vec![0],
        });
    } else {
        panic!("base module must contain CONST_POOL");
    }

    let mut body = Vec::with_capacity(depth * 6 + depth + 1);
    for _ in 0..depth {
        body.push(0x10);
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    body.extend(std::iter::repeat_n(0x12, depth));
    body.push(0x06);
    set_main_body(&mut module, body);
    module
}

fn module_above_stack_limit() -> BytecodeModule {
    module_with_stack_depth(MAX_OPERAND_STACK + 1)
}

fn execution_budget_outcome() -> Result<&'static str, String> {
    let mut harness = TestHarness::from_source(EXECUTION_BUDGET_SOURCE)
        .map_err(|error| format!("compile instruction-budget source: {error}"))?;
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .map_err(|error| format!("select VM backend: {error}"))?;
    harness.runtime_mut().set_execution_deadline(None);
    let cycle = harness.cycle();
    if cycle.errors.iter().any(|error| {
        matches!(
            error,
            RuntimeError::ExecutionLimit {
                kind: trust_runtime_core::error::ExecutionLimitKind::Work,
                used,
                requested,
                limit,
            } if used <= limit && *requested > limit - used
        ) && error.stable_code() == RuntimeError::ExecutionTimeout.stable_code()
    }) {
        Ok("execution_timeout")
    } else if cycle.errors.is_empty() {
        Ok("accepted")
    } else {
        Err(format!(
            "instruction-budget workload produced an unexpected error: {:?}",
            cycle.errors
        ))
    }
}

fn oscat_outcome() -> Result<&'static str, String> {
    let bytes = std::fs::read("crates/trust-runtime/tests/fixtures/oscat/core/program.stbc")
        .map_err(|error| format!("read OSCAT fixture: {error}"))?;
    if bytes.len() > MAX_CONTAINER_BYTES {
        return Ok("rejected");
    }
    let module =
        BytecodeModule::decode(&bytes).map_err(|error| format!("decode OSCAT fixture: {error}"))?;
    module
        .validate()
        .map_err(|error| format!("validate OSCAT fixture: {error}"))?;
    Ok("accepted")
}

fn set_ref_entries(module: &mut BytecodeModule, entries: Vec<RefEntry>) {
    if let Some(SectionData::RefTable(table)) = module.section_mut(SectionId::RefTable) {
        table.entries = entries;
    } else {
        panic!("base module must contain REF_TABLE");
    }
}

fn set_main_body(module: &mut BytecodeModule, body: Vec<u8>) {
    let body_len = u32::try_from(body.len()).expect("case body length fits u32");
    if let Some(SectionData::PouBodies(existing)) = module.section_mut(SectionId::PouBodies) {
        *existing = body;
    } else {
        panic!("base module must contain POU_BODIES");
    }
    let main = main_pou_mut(module);
    main.code_offset = 0;
    main.code_length = body_len;
}

fn main_pou_mut(module: &mut BytecodeModule) -> &mut trust_runtime::bytecode::PouEntry {
    match module.section_mut(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => index
            .entries
            .first_mut()
            .expect("base module must contain a POU"),
        _ => panic!("base module must contain POU_INDEX"),
    }
}

fn only_trace_step(case: &CaseRecord) -> Result<&TraceStep, String> {
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 1 {
        return Err(format!("{} must contain exactly one trace step", case.id));
    }
    Ok(&trace[0])
}

fn trace_string(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace field {key} must be a string"))
}

#[derive(Default)]
struct ResourceLimitProbe {
    accepted_scenarios: HashSet<String>,
}

impl StateProbe for ResourceLimitProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        let mut accepted = self.accepted_scenarios.iter().cloned().collect::<Vec<_>>();
        accepted.sort();
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: Some(json!({ "accepted_scenarios": accepted })),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}

/// The text of a structural rejection; any other variant fails the test.
fn invalid_section_text(error: &BytecodeError) -> String {
    match error {
        BytecodeError::InvalidSection(detail) => detail.to_string(),
        other => panic!("expected InvalidSection, got {other:?}"),
    }
}

#[test]
fn container_limit_refuses_one_byte_past_64_mib_with_the_exact_header_error() {
    // spec 12 §4.6: the encoded-container limit is checked before checksum or section
    // decoding (module_decode.rs:18-21). On std builds the runtime wraps it as Bytecode.
    match BytecodeModule::decode(&oversized_container_bytes()) {
        Err(BytecodeError::InvalidHeader(text)) => assert_eq!(
            text.as_str(),
            "encoded container exceeds fixed resource limit"
        ),
        Err(other) => panic!("expected InvalidHeader, got {other:?}"),
        Ok(_) => panic!("a container one byte above the fixed limit decoded"),
    }
    assert_eq!(
        apply_oversized_container(),
        Err(RuntimeError::Bytecode {
            code: StableErrorCode::BytecodeInvalidHeader,
            detail: "invalid bytecode header: encoded container exceeds fixed resource limit"
                .into(),
        })
    );
}

#[test]
fn oversized_container_refusal_keeps_the_installed_program_and_its_outputs() {
    let source = r#"
PROGRAM Main
VAR
    counter : DINT := DINT#0;
END_VAR
counter := counter + DINT#1;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).expect("compile installed program");
    let first = harness.cycle();
    assert!(first.errors.is_empty(), "{:?}", first.errors);
    assert_eq!(harness.get_output("counter"), Some(Value::DInt(1)));
    let cycles_before = harness.cycle_count();

    let refused = harness
        .runtime_mut()
        .apply_bytecode_bytes(&oversized_container_bytes(), None);
    assert_eq!(
        refused,
        Err(RuntimeError::Bytecode {
            code: StableErrorCode::BytecodeInvalidHeader,
            detail: "invalid bytecode header: encoded container exceeds fixed resource limit"
                .into(),
        })
    );
    assert_eq!(
        harness.cycle_count(),
        cycles_before,
        "a refused container must not advance the cycle counter"
    );
    assert_eq!(
        harness.get_output("counter"),
        Some(Value::DInt(1)),
        "a refused container must leave the outputs unchanged"
    );

    let second = harness.cycle();
    assert!(second.errors.is_empty(), "{:?}", second.errors);
    assert_eq!(
        harness.get_output("counter"),
        Some(Value::DInt(2)),
        "the installed program must still run after the refusal"
    );
}

#[test]
fn instruction_limit_admits_the_fixed_count_and_refuses_one_more_with_its_exact_error() {
    // spec 12 §4.6.2: the exact maximum is accepted. validate/instructions.rs:66 charges each
    // decoded instruction before its opcode is read; resource_limits.rs:15-25 gives the text.
    module_with_instruction_count(MAX_MODULE_INSTRUCTIONS)
        .validate()
        .expect("the exact instruction limit must validate");
    let error = module_above_instruction_limit()
        .validate()
        .expect_err("one instruction past the limit must be refused");
    assert_eq!(
        invalid_section_text(&error),
        "decoded module instructions exceed fixed resource limit"
    );
}

#[test]
fn reference_limit_admits_the_fixed_count_and_refuses_one_more_with_its_exact_error() {
    module_with_references(MAX_REFERENCES)
        .validate()
        .expect("the exact reference limit must validate");
    let error = module_above_reference_limit()
        .validate()
        .expect_err("one reference past the limit must be refused");
    assert_eq!(
        invalid_section_text(&error),
        "REF_TABLE entries exceed fixed resource limit"
    );
}

#[test]
fn local_limit_admits_the_fixed_count_and_refuses_one_more_with_its_exact_error() {
    module_with_locals(MAX_LOCALS_PER_POU, MAX_LOCALS_PER_POU)
        .validate()
        .expect("the exact local-reference limit must validate");
    let error = module_above_local_limit()
        .validate()
        .expect_err("one local reference past the limit must be refused");
    assert_eq!(
        invalid_section_text(&error),
        "POU local references exceed fixed resource limit"
    );
}

#[test]
fn parameter_limit_admits_the_fixed_count_and_refuses_one_more_with_its_exact_error() {
    module_with_parameters(MAX_PARAMETERS_PER_POU)
        .validate()
        .expect("the exact parameter limit must validate");
    let error = module_above_parameter_limit()
        .validate()
        .expect_err("one parameter past the limit must be refused");
    assert_eq!(
        invalid_section_text(&error),
        "POU parameters exceed fixed resource limit"
    );
}

#[test]
fn operand_stack_limit_admits_the_fixed_depth_and_refuses_one_more_with_its_exact_error() {
    // Stack depth is checked when a value is pushed (stack_shape.rs push_stack via
    // budget.rs:82-87), so the refused push is the one that makes the depth 16,385.
    module_with_stack_depth(MAX_OPERAND_STACK)
        .validate()
        .expect("the exact operand-stack depth must validate");
    let error = module_above_stack_limit()
        .validate()
        .expect_err("one value past the operand-stack limit must be refused");
    assert_eq!(
        invalid_section_text(&error),
        "operand stack values exceed fixed resource limit"
    );
}

#[test]
fn call_depth_admits_the_fixed_frame_count_and_refuses_one_more_as_call_stack_overflow() {
    // frames.rs:11-18 returns VmTrap::CallStackOverflow for a total depth above 1,024.
    ensure_global_call_depth(0, MAX_CALL_DEPTH).expect("the exact call depth must be admitted");
    assert!(matches!(
        ensure_global_call_depth(0, MAX_CALL_DEPTH + 1),
        Err(VmTrap::CallStackOverflow)
    ));
}

#[test]
fn executed_instruction_refusal_is_the_exact_work_limit() {
    // Each executed instruction charges one unit before it runs (dispatch.rs:275), and a
    // refused charge reports the units already used and the single unit requested.
    let mut harness = TestHarness::from_source(EXECUTION_BUDGET_SOURCE)
        .expect("compile instruction-budget source");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("select VM backend");
    harness.runtime_mut().set_execution_deadline(None);
    let cycle = harness.cycle();
    assert_eq!(
        cycle.errors,
        vec![RuntimeError::ExecutionLimit {
            kind: trust_runtime_core::error::ExecutionLimitKind::Work,
            used: trust_runtime_core::vm::VM_MAX_EXECUTED_INSTRUCTIONS,
            requested: 1,
            limit: trust_runtime_core::vm::VM_MAX_EXECUTED_INSTRUCTIONS,
        }]
    );
}
