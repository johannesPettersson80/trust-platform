use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

use super::*;

const OWNER_CASE_FILE: &str = "verification/cases/bytecode_vm/VM_SEAM_OWNER_001.toml";
const OWNER_CASE_DIGEST: &str =
    "sha256:96b44cff96659e58bdad83bdf7f47fffb1d49eb5cf85addf0cf83b240f562484";
const OWNER_TEST_ID: &str = "TEST_VM_OWNER_TRACE_001";
const REF_CASE_FILE: &str = "verification/cases/bytecode_vm/VM_SEAM_REF_001.toml";
const REF_CASE_DIGEST: &str =
    "sha256:7b3611fd7f35176dc7690518ad5d118cb617afc4b2545ec309ecbb847fce29cb";
const REF_TEST_ID: &str = "TEST_VM_REF_ESCAPE_TRACE_001";

const LOCAL_RANGE_SOURCE: &str = r#"
FUNCTION First : INT
VAR first_local : INT := INT#1; END_VAR
First := first_local;
END_FUNCTION

FUNCTION Second : INT
VAR second_local : INT := INT#2; END_VAR
Second := second_local;
END_FUNCTION

PROGRAM Main
END_PROGRAM
"#;

#[test]
fn vm_owner_trace_cases() {
    run_seam_cases(
        OWNER_TEST_ID,
        OWNER_CASE_FILE,
        OWNER_CASE_DIGEST,
        run_owner_case,
    );
}

#[test]
fn vm_ref_escape_trace_cases() {
    run_seam_cases(REF_TEST_ID, REF_CASE_FILE, REF_CASE_DIGEST, run_ref_case);
}

fn run_seam_cases(
    test_id: &str,
    case_file: &str,
    digest: &str,
    runner: fn(&CaseRecord, &mut SeamProbe) -> Result<CaseExecution, String>,
) {
    let config = RunConfig::new(test_id, workspace_root().join(case_file), digest);
    let mut probe = SeamProbe::default();
    let artifact = run_case_file(&config, &mut probe, runner)
        .unwrap_or_else(|error| panic!("{test_id} artifact must be written: {error}"));
    let failures = artifact
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
        failures.is_empty(),
        "{test_id} failures: {}",
        failures.join("; ")
    );
}

fn run_owner_case(case: &CaseRecord, probe: &mut SeamProbe) -> Result<CaseExecution, String> {
    let scenario = scenario(case)?;
    let status = match scenario {
        "LOCAL_RANGES_SHARE_FRAME_OWNER" => reject_shared_local_owner()?,
        "INSTRUCTION_MIXES_INSTANCE_OWNERS" => reject_multi_instance_owner()?,
        other => return Err(format!("unreviewed ownership scenario {other}")),
    };
    probe.observed = Some(serde_json::json!({"scenario": scenario, "outcome": status}));
    Ok(passed(status))
}

fn run_ref_case(case: &CaseRecord, probe: &mut SeamProbe) -> Result<CaseExecution, String> {
    let scenario = scenario(case)?;
    let status = match scenario {
        "LOCAL_REFERENCE_OUTSIDE_POU_RANGE" => reject_local_ref_outside_range()?,
        "FRAME_LOCAL_REFERENCE_ESCAPES" => reject_persistent_frame_local_ref()?,
        other => return Err(format!("unreviewed reference scenario {other}")),
    };
    probe.observed = Some(serde_json::json!({"scenario": scenario, "outcome": status}));
    Ok(passed(status))
}

fn reject_shared_local_owner() -> Result<&'static str, String> {
    let mut module = bytecode_module_from_source(LOCAL_RANGE_SOURCE)
        .map_err(|error| format!("compile local-range fixture: {error}"))?;
    let (first_owner, second_start, second_count) = local_range_metadata(&module)?;
    if second_count == 0 {
        return Err("Second POU has no local refs".into());
    }
    match module.section_mut(SectionId::RefTable) {
        Some(SectionData::RefTable(refs)) => {
            for entry in refs
                .entries
                .iter_mut()
                .skip(second_start as usize)
                .take(second_count as usize)
            {
                entry.owner_id = first_owner;
            }
        }
        _ => return Err("missing ref table".into()),
    }
    expect_validation_and_apply_rejection(&module, "POU local ref ranges share a frame owner")
}

fn reject_multi_instance_owner() -> Result<&'static str, String> {
    let harness = TestHarness::from_source(OWNER_DRIFT_SOURCE)
        .map_err(|error| format!("compile owner fixture: {error}"))?;
    let main_id = match harness.runtime().storage().get_global("Main") {
        Some(Value::Instance(id)) => *id,
        other => return Err(format!("expected Main instance, got {other:?}")),
    };
    let alien_id = InstanceId(main_id.0.saturating_add(1));
    let mut module = bytecode_module_from_source(OWNER_DRIFT_SOURCE)
        .map_err(|error| format!("compile owner module: {error}"))?;
    mutate_main_refs_for_owner_drift(&mut module, main_id, alien_id)?;
    expect_validation_and_apply_rejection(&module, "multiple instance owners")
}

fn reject_local_ref_outside_range() -> Result<&'static str, String> {
    let mut module = bytecode_module_from_source(LOCAL_RANGE_SOURCE)
        .map_err(|error| format!("compile local-range fixture: {error}"))?;
    let second_id = {
        let strings = match module.section(SectionId::StringTable) {
            Some(SectionData::StringTable(strings)) => strings.clone(),
            _ => return Err("missing string table".into()),
        };
        find_function_pou(&module, &strings, "Second")?.id
    };
    match module.section_mut(SectionId::PouIndex) {
        Some(SectionData::PouIndex(index)) => {
            let second = index
                .entries
                .iter_mut()
                .find(|entry| entry.id == second_id)
                .ok_or_else(|| "Second POU disappeared".to_string())?;
            second.local_ref_count = 0;
        }
        _ => return Err("missing POU index".into()),
    }
    expect_validation_and_apply_rejection(&module, "local ref outside POU local range")
}

fn reject_persistent_frame_local_ref() -> Result<&'static str, String> {
    let mut module = bytecode_module_from_source(CLEAN_LOCAL_REF_SOURCE)
        .map_err(|error| format!("compile frame-local fixture: {error}"))?;
    mutate_leak_to_store_local_ref(&mut module)?;
    expect_validation_and_apply_rejection(&module, "frame-local reference")
}

fn local_range_metadata(module: &BytecodeModule) -> Result<(u32, u32, u32), String> {
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(strings)) => strings,
        _ => return Err("missing string table".into()),
    };
    let first = find_function_pou(module, strings, "First")?;
    let second = find_function_pou(module, strings, "Second")?;
    let first_owner = match module.section(SectionId::RefTable) {
        Some(SectionData::RefTable(refs)) => {
            refs.entries
                .get(first.local_ref_start as usize)
                .ok_or_else(|| "First local range is out of bounds".to_string())?
                .owner_id
        }
        _ => return Err("missing ref table".into()),
    };
    Ok((first_owner, second.local_ref_start, second.local_ref_count))
}

fn expect_validation_and_apply_rejection(
    module: &BytecodeModule,
    expected_detail: &str,
) -> Result<&'static str, String> {
    let validation = module
        .validate()
        .expect_err("mutated module must fail direct validation");
    if validation.stable_code().as_str() != "bytecode_invalid_section"
        || !validation.to_string().contains(expected_detail)
    {
        return Err(format!(
            "expected bytecode_invalid_section containing {expected_detail:?}, got {validation:?}"
        ));
    }
    let bytes = module
        .encode()
        .map_err(|error| format!("encode mutated module: {error}"))?;
    let mut runtime = trust_runtime::Runtime::new();
    match runtime.apply_bytecode_bytes(&bytes, None) {
        Err(error)
            if error.stable_code().as_str() == "bytecode_invalid_section"
                && error.to_string().contains(expected_detail) =>
        {
            Ok("rejected_before_apply")
        }
        Err(error) => Err(format!(
            "expected product rejection containing {expected_detail:?}, got {error:?}"
        )),
        Ok(()) => Err("product apply accepted mutated bytecode".into()),
    }
}

fn scenario(case: &CaseRecord) -> Result<&str, String> {
    case.input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} requires scenario", case.id))
}

fn passed(status: &str) -> CaseExecution {
    CaseExecution {
        result: CaseResult::Passed,
        observed_error: None,
        observed_status: Some(status.to_string()),
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf()
}

#[derive(Default)]
struct SeamProbe {
    observed: Option<serde_json::Value>,
    before: bool,
}

impl StateProbe for SeamProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.before {
            self.observed = None;
        }
        self.before = !self.before;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.observed.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
