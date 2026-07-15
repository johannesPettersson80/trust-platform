mod bytecode_helpers;

use std::collections::{BTreeMap, HashSet};
use std::env;
use std::path::{Path, PathBuf};

use serde_json::json;
use trust_runtime::bytecode::{
    BytecodeModule, ConstEntry, ParamEntry, RefEntry, RefLocation, SectionData, SectionId,
};
use trust_runtime::error::RuntimeError;
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::TestHarness;
use trust_runtime::runtime_core::vm::ensure_global_call_depth;
use trust_runtime::Runtime;
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_VM_RESOURCE_LIMIT_CASES_001";
const CASE_FILE: &str = "verification/cases/bytecode_vm/VM_SEAM_DETERMINISM_LIMITS_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:2f824eabeed68ec5f224d64f9c7bc5e866ec1bd4095e7346c8d61a3410c2ee5f";

const MAX_CONTAINER_BYTES: usize = 64 * 1024 * 1024;
const MAX_MODULE_INSTRUCTIONS: usize = 1_000_000;
const MAX_REFERENCES: usize = 65_536;
const MAX_LOCALS_PER_POU: usize = 65_536;
const MAX_PARAMETERS_PER_POU: usize = 1_024;
const MAX_OPERAND_STACK: usize = 16 * 1024;
const MAX_CALL_DEPTH: usize = 1_024;

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

fn apply_oversized_container() -> Result<(), RuntimeError> {
    let mut module = bytecode_helpers::base_module();
    module.flags = 0;
    let mut bytes = module.encode().expect("encode base module");
    bytes.resize(MAX_CONTAINER_BYTES.saturating_add(1), 0);
    Runtime::new().apply_bytecode_bytes(&bytes, None)
}

fn module_above_instruction_limit() -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    set_main_body(&mut module, vec![0x00; MAX_MODULE_INSTRUCTIONS + 1]);
    module
}

fn module_above_reference_limit() -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    let references = (0..=MAX_REFERENCES)
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

fn module_above_local_limit() -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    let references = (0..=MAX_LOCALS_PER_POU)
        .map(|offset| RefEntry {
            location: RefLocation::Local,
            owner_id: 1,
            offset: u32::try_from(offset).expect("local offset fits u32"),
            segments: Vec::new(),
        })
        .collect();
    set_ref_entries(&mut module, references);
    main_pou_mut(&mut module).local_ref_count =
        u32::try_from(MAX_LOCALS_PER_POU + 1).expect("local limit fits u32");
    module
}

fn module_above_parameter_limit() -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    main_pou_mut(&mut module).params = (0..=MAX_PARAMETERS_PER_POU)
        .map(|_| ParamEntry {
            name_idx: 0,
            type_id: 0,
            direction: 0,
            default_const_idx: None,
        })
        .collect();
    module
}

fn module_above_stack_limit() -> BytecodeModule {
    let mut module = bytecode_helpers::base_module();
    if let Some(SectionData::ConstPool(pool)) = module.section_mut(SectionId::ConstPool) {
        pool.entries.push(ConstEntry {
            type_id: 0,
            payload: vec![0],
        });
    } else {
        panic!("base module must contain CONST_POOL");
    }

    let mut body = Vec::with_capacity((MAX_OPERAND_STACK + 1) * 6 + 1);
    for _ in 0..=MAX_OPERAND_STACK {
        body.push(0x10);
        body.extend_from_slice(&0u32.to_le_bytes());
    }
    body.extend(std::iter::repeat_n(0x12, MAX_OPERAND_STACK + 1));
    body.push(0x06);
    set_main_body(&mut module, body);
    module
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
    if cycle
        .errors
        .iter()
        .any(|error| matches!(error, RuntimeError::ExecutionTimeout))
    {
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
