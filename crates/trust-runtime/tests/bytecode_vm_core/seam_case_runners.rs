use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
};

const ENCODER_CASE_FILE: &str = "verification/cases/bytecode_vm/VM_SEAM_ENC_001.toml";
const ENCODER_CASE_DIGEST: &str =
    "sha256:a57db88204f9b7815f0a548fad35eb20fb2bda6337efdeb1690191560e73e7cf";
const ENCODER_TEST_ID: &str = "TEST_VM_ENCODER_FAIL_CLOSED_TRACE_001";

#[test]
fn vm_encoder_fail_closed_trace_cases() {
    let mut probe = EncoderProbe::default();
    let config = RunConfig::new(
        ENCODER_TEST_ID,
        workspace_root_for_encoder_cases().join(ENCODER_CASE_FILE),
        ENCODER_CASE_DIGEST,
    );
    let artifact = run_case_file(&config, &mut probe, run_encoder_case)
        .unwrap_or_else(|error| panic!("{ENCODER_TEST_ID} artifact must be written: {error}"));
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
        "{ENCODER_TEST_ID} failures: {}",
        failures.join("; ")
    );
}

fn run_encoder_case(
    case: &CaseRecord,
    probe: &mut EncoderProbe,
) -> Result<CaseExecution, String> {
    let scenario = case
        .input
        .get("scenario")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| format!("{} requires scenario", case.id))?;
    let status = match scenario {
        "SUPPORTED_LOOP_CONTROL" => run_supported_loop_control()?,
        "UNSUPPORTED_STATEMENT" => reject_unsupported_statement()?,
        "UNSUPPORTED_EXPRESSION_AFTER_VALID_PREFIX" => reject_array_initializer_after_prefix()?,
        other => return Err(format!("unreviewed encoder scenario {other}")),
    };
    probe.observed = Some(serde_json::json!({"scenario": scenario, "outcome": status}));
    Ok(CaseExecution {
        result: CaseResult::Passed,
        observed_error: None,
        observed_status: Some(status.to_string()),
    })
}

fn run_supported_loop_control() -> Result<&'static str, String> {
    let source = r#"
PROGRAM Main
VAR
    i : INT := INT#0;
    w : INT := INT#0;
    r : INT := INT#0;
    sum_for : INT := INT#0;
    sum_while : INT := INT#0;
    sum_repeat : INT := INT#0;
END_VAR
FOR i := INT#0 TO INT#4 BY INT#1 DO
    IF i = INT#1 THEN CONTINUE; END_IF;
    IF i = INT#3 THEN EXIT; END_IF;
    sum_for := sum_for + i;
END_FOR;
WHILE w < INT#5 DO
    w := w + INT#1;
    IF w = INT#2 THEN CONTINUE; END_IF;
    IF w = INT#4 THEN EXIT; END_IF;
    sum_while := sum_while + w;
END_WHILE;
REPEAT
    r := r + INT#1;
    IF r = INT#2 THEN CONTINUE; END_IF;
    IF r = INT#4 THEN EXIT; END_IF;
    sum_repeat := sum_repeat + r;
UNTIL r >= INT#6 END_REPEAT;
END_PROGRAM
"#;
    let module = bytecode_module_from_source(source)
        .map_err(|error| format!("supported source failed bytecode lowering: {error}"))?;
    module
        .validate()
        .map_err(|error| format!("supported module failed validation: {error}"))?;
    let mut harness = vm_harness(source);
    let cycle = harness.cycle();
    if !cycle.errors.is_empty() {
        return Err(format!("supported loop execution failed: {:?}", cycle.errors));
    }
    let observed = [
        harness.get_output("sum_for"),
        harness.get_output("sum_while"),
        harness.get_output("sum_repeat"),
    ];
    let expected = [Some(Value::Int(2)), Some(Value::Int(4)), Some(Value::Int(4))];
    if observed != expected {
        return Err(format!("expected loop sums {expected:?}, observed {observed:?}"));
    }
    Ok("accepted_and_executed")
}

fn reject_unsupported_statement() -> Result<&'static str, String> {
    let source = r#"
PROGRAM Main
VAR x : INT := INT#0; END_VAR
JMP L1;
x := INT#1;
L1: x := x + INT#2;
END_PROGRAM
"#;
    expect_lowering_rejection(source, "unsupported C5 edge-case lowering path")
}

fn reject_array_initializer_after_prefix() -> Result<&'static str, String> {
    let source = r#"
PROGRAM Main
VAR target : ARRAY[0..1] OF DINT; END_VAR
target[0] := DINT#7;
target := [DINT#1, DINT#2];
END_PROGRAM
"#;
    let session = trust_runtime::harness::CompileSession::from_source(source);
    session
        .build_runtime()
        .map_err(|error| format!("fixture must pass source analysis: {error}"))?;
    match session.build_bytecode_module() {
        Err(_) => Ok("rejected_before_partial_bytecode"),
        Ok(_) => Err("unsupported array initializer produced bytecode after a valid prefix".into()),
    }
}

fn expect_lowering_rejection(
    source: &str,
    expected_message: &str,
) -> Result<&'static str, String> {
    match bytecode_module_from_source(source) {
        Err(error) if error.to_string().contains(expected_message) => Ok("rejected_before_bytecode"),
        Err(error) => Err(format!(
            "expected lowering rejection containing {expected_message:?}, got {error}"
        )),
        Ok(_) => Err(format!(
            "source requiring {expected_message:?} unexpectedly produced bytecode"
        )),
    }
}

fn workspace_root_for_encoder_cases() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf()
}

#[derive(Default)]
struct EncoderProbe {
    observed: Option<serde_json::Value>,
    before: bool,
}

impl StateProbe for EncoderProbe {
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
