mod bytecode_helpers;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use trust_runtime::bytecode::{BytecodeError, BytecodeModule, SectionData, SectionId};
use trust_runtime::Runtime;
use verification_cases::{
    case_file_digest, run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe,
    StateSnapshot,
};

const TEST_ID: &str = "TEST_BYTECODE_VALIDATOR_CASES_001";
const CASE_FILE: &str = "verification/cases/bytecode_vm/VM_SEAM_VALID_001.toml";

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root")
        .to_path_buf()
}

fn committed_seed_bytes() -> Vec<u8> {
    let path = workspace_root().join("verification/seeds/bytecode_vm/minimal-stbc-seed.toml");
    let source = fs::read_to_string(path).expect("read bytecode transform seed");
    let seed: toml::Value = toml::from_str(&source).expect("parse bytecode transform seed");
    let hex = seed
        .get("bytes_hex")
        .and_then(toml::Value::as_str)
        .expect("seed bytes_hex");
    decode_hex(hex)
}

fn decode_hex(value: &str) -> Vec<u8> {
    assert!(value.len().is_multiple_of(2), "hex length must be even");
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let text = std::str::from_utf8(pair).expect("hex is ASCII");
            u8::from_str_radix(text, 16).expect("valid hex byte")
        })
        .collect()
}

struct RuntimeProbe {
    runtime: Runtime,
}

impl RuntimeProbe {
    fn new() -> Self {
        let mut runtime = Runtime::new();
        runtime
            .apply_bytecode_bytes(&committed_seed_bytes(), None)
            .expect("apply valid baseline module");
        Self { runtime }
    }
}

impl StateProbe for RuntimeProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        let metadata = self.runtime.metadata_snapshot();
        let process_image = [
            self.runtime.io().inputs(),
            self.runtime.io().outputs(),
            self.runtime.io().memory(),
        ]
        .concat();
        let mut siblings = BTreeMap::new();
        siblings.insert(
            "tasks".to_string(),
            serde_json::json!(metadata
                .tasks()
                .iter()
                .map(|task| task.name.as_str())
                .collect::<Vec<_>>()),
        );
        siblings.insert(
            "process_image_lengths".to_string(),
            serde_json::json!({
                "inputs": self.runtime.io().inputs().len(),
                "outputs": self.runtime.io().outputs().len(),
                "memory": self.runtime.io().memory().len(),
            }),
        );
        Ok(StateSnapshot {
            process_image_hash: Some(format!("sha256:{:x}", Sha256::digest(process_image))),
            retain_hash: None,
            target: Some(serde_json::json!({"module": "baseline-valid"})),
            siblings,
            diagnostics: Vec::new(),
        })
    }
}

fn case_bytes(case: &CaseRecord) -> Result<Vec<u8>, String> {
    let encoded = case
        .input
        .get("bytes_hex")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} is missing input.bytes_hex", case.id))?;
    let bytes = decode_hex(encoded);
    let expected_digest = case
        .input
        .get("mutated_digest")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} is missing input.mutated_digest", case.id))?;
    let actual_digest = format!("sha256:{:x}", Sha256::digest(&bytes));
    if actual_digest != expected_digest {
        return Err(format!(
            "{} byte digest mismatch: expected {expected_digest}, actual {actual_digest}",
            case.id
        ));
    }
    Ok(bytes)
}

fn expected_rejection(case: &CaseRecord) -> Result<(&str, &str, Option<&str>), String> {
    let expect = case
        .expect
        .as_ref()
        .and_then(toml::Value::as_table)
        .ok_or_else(|| format!("{} is missing a runnable expectation", case.id))?;
    if expect.get("outcome").and_then(toml::Value::as_str) != Some("reject")
        || expect
            .get("no_partial_apply")
            .and_then(toml::Value::as_bool)
            != Some(true)
    {
        return Err(format!("{} must require transactional rejection", case.id));
    }
    if expect.get("oracle_ref").and_then(toml::Value::as_str)
        != Some("SPEC_BYTECODE_FORMAT_001#validator-before-apply")
    {
        return Err(format!(
            "{} is missing the reviewed validator oracle",
            case.id
        ));
    }
    let transform = case
        .input
        .get("transform")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} is missing input.transform", case.id))?;
    match transform {
        "container_truncate" => match case
            .input
            .get("truncate_point")
            .and_then(toml::Value::as_str)
        {
            Some("before_section_table") => Ok((
                "invalid_section_table",
                "bytecode_invalid_section_table",
                None,
            )),
            Some("before_pou_bodies") => Ok((
                "section_out_of_bounds",
                "bytecode_section_out_of_bounds",
                None,
            )),
            other => Err(format!("{} has unknown truncate point {other:?}", case.id)),
        },
        "unknown_opcode" => Ok(("invalid_opcode", "bytecode_invalid_opcode", None)),
        "jump_target" => Ok(("invalid_jump_target", "bytecode_invalid_jump_target", None)),
        "stack_underflow" => Ok((
            "invalid_section",
            "bytecode_invalid_section",
            Some("operand stack underflow"),
        )),
        other => Err(format!("{} has unknown transform {other:?}", case.id)),
    }
}

fn decode_or_validate_error(bytes: &[u8]) -> Result<BytecodeError, String> {
    match BytecodeModule::decode(bytes) {
        Err(error) => Ok(error),
        Ok(module) => module
            .validate()
            .err()
            .ok_or_else(|| "validator case unexpectedly passed direct validation".to_string()),
    }
}

fn error_variant(error: &BytecodeError) -> &'static str {
    match error {
        BytecodeError::UnexpectedEof => "unexpected_eof",
        BytecodeError::InvalidSectionTable(_) => "invalid_section_table",
        BytecodeError::SectionOutOfBounds => "section_out_of_bounds",
        BytecodeError::InvalidOpcode(_) => "invalid_opcode",
        BytecodeError::InvalidJumpTarget(_) => "invalid_jump_target",
        BytecodeError::InvalidSection(_) => "invalid_section",
        _ => "other",
    }
}

#[test]
fn committed_transform_seed_is_a_complete_applicable_module() {
    let mut reviewed = bytecode_helpers::base_module();
    // Transform cases need to reach semantic catchers rather than fail first at
    // the independently tested container checksum boundary.
    reviewed.flags = 0;
    if let Some(SectionData::PouBodies(body)) = reviewed.section_mut(SectionId::PouBodies) {
        *body = vec![0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01];
    } else {
        panic!("reviewed base module is missing POU_BODIES");
    }
    if let Some(SectionData::PouIndex(index)) = reviewed.section_mut(SectionId::PouIndex) {
        index.entries[0].code_length = 8;
    } else {
        panic!("reviewed base module is missing POU_INDEX");
    }
    if let Some(SectionData::ResourceMeta(meta)) = reviewed.section_mut(SectionId::ResourceMeta) {
        meta.resources[0].tasks.clear();
    } else {
        panic!("reviewed base module is missing RESOURCE_META");
    }
    let expected = reviewed.encode().expect("encode reviewed base module");
    let actual = committed_seed_bytes();

    assert_eq!(
        actual,
        expected,
        "transform seed must be the complete reviewed base module; expected bytes_hex={}",
        expected
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );

    let module = BytecodeModule::decode(&actual).expect("decode transform seed");
    module.validate().expect("validate transform seed");
    Runtime::new()
        .apply_bytecode_bytes(&actual, None)
        .expect("apply transform seed through the product path");
}

#[test]
fn bytecode_validator_cases_reject_before_partial_apply() {
    let path = workspace_root().join(CASE_FILE);
    let digest = case_file_digest(&path).expect("digest validator case file");
    let config = RunConfig::new(TEST_ID, &path, digest);
    let mut probe = RuntimeProbe::new();
    let artifact = run_case_file(&config, &mut probe, |case, probe| -> Result<_, String> {
        let bytes = case_bytes(case)?;
        let (expected_variant, expected_code, message_contains) = expected_rejection(case)?;
        let direct_error = decode_or_validate_error(&bytes)?;
        let direct_text = direct_error.to_string();
        let mut failures = Vec::new();
        if error_variant(&direct_error) != expected_variant {
            failures.push(format!(
                "expected {expected_variant}, got {} ({direct_error:?})",
                error_variant(&direct_error)
            ));
        }
        if message_contains.is_some_and(|needle| !direct_text.contains(needle)) {
            failures.push(format!(
                "direct error {direct_text:?} lacks {message_contains:?}"
            ));
        }
        let direct_code = direct_error.stable_code();
        if direct_code.as_str() != expected_code {
            failures.push(format!(
                "direct error code mismatch: expected {expected_code}, got {}",
                direct_code.as_str()
            ));
        }
        match probe.runtime.apply_bytecode_bytes(&bytes, None) {
            Err(error) => {
                let product_code = error.stable_code();
                if product_code.as_str() != expected_code {
                    failures.push(format!(
                        "product error code mismatch: expected {expected_code}, got {} ({error:?})",
                        product_code.as_str()
                    ));
                }
            }
            Ok(()) => failures.push("product path accepted invalid bytecode".to_string()),
        }
        Ok(CaseExecution {
            result: if failures.is_empty() {
                CaseResult::Passed
            } else {
                CaseResult::Failed
            },
            observed_error: (!failures.is_empty()).then(|| failures.join("; ")),
            observed_status: Some(format!("rejected:{expected_variant}")),
        })
    })
    .expect("run bytecode validator case file");

    assert_eq!(artifact.test_id, TEST_ID, "test identity drifted");
    assert_eq!(artifact.case_file, CASE_FILE, "case-file identity drifted");
    assert_eq!(artifact.cases.len(), 7, "validator case count drifted");
    for case in &artifact.cases {
        assert_eq!(
            case.result,
            CaseResult::Passed,
            "case {} failed: observed_error={:?}, observed_status={:?}",
            case.id,
            case.observed_error,
            case.observed_status,
        );
        assert_eq!(
            case.state_delta.as_deref(),
            Some("unchanged"),
            "case {} partially changed runtime state",
            case.id
        );
    }
}
