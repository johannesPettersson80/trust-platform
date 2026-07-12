use std::collections::BTreeMap;
use std::env;
use std::path::{Path, PathBuf};

use serde_json::json;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};
use verification_cases::{
    run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe, StateSnapshot,
    TraceStep,
};

const TEST_ID: &str = "TEST_IEC_TIMER_TRACE_001";
const CASE_FILE: &str = "verification/cases/compiler_iec/IEC_TIMER_001.toml";
const CASE_FILE_DIGEST: &str =
    "sha256:68567779c24d44fc1ce1cda7696086aac6284b5788305511d2f416f66e12be50";

const TOF_TIME_SOURCE: &str = r#"
PROGRAM Test
VAR
    timer : TOF;
    timer_in : BOOL;
    pt : TIME := T#10ms;
    q : BOOL;
    et : TIME;
END_VAR
timer(IN := timer_in, PT := pt, Q => q, ET => et);
END_PROGRAM
"#;

const TOF_LTIME_SOURCE: &str = r#"
PROGRAM Test
VAR
    timer : TOF_LTIME;
    timer_in : BOOL;
    pt : LTIME := LTIME#10ms;
    q : BOOL;
    et : LTIME;
END_VAR
timer(IN := timer_in, PT := pt, Q => q, ET => et);
END_PROGRAM
"#;

const TON_TIME_SOURCE: &str = r#"
PROGRAM Test
VAR
    timer : TON;
    timer_in : BOOL;
    pt : TIME := T#10ms;
    q : BOOL;
    et : TIME;
END_VAR
timer(IN := timer_in, PT := pt, Q => q, ET => et);
END_PROGRAM
"#;

const TP_TIME_SOURCE: &str = r#"
PROGRAM Test
VAR
    timer : TP;
    timer_in : BOOL;
    pt : TIME := T#10ms;
    q : BOOL;
    et : TIME;
END_VAR
timer(IN := timer_in, PT := pt, Q => q, ET => et);
END_PROGRAM
"#;

#[test]
fn timer_state_machine_trace_cases() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = env::current_dir().expect("current directory must be readable");
    env::set_current_dir(&workspace).expect("timer trace runner must enter workspace root");

    let mut probe = TimerProbe::default();
    let config = RunConfig::new(TEST_ID, CASE_FILE, CASE_FILE_DIGEST);
    let result = run_case_file(&config, &mut probe, run_trace_case);

    env::set_current_dir(original_dir).expect("timer trace runner must restore current directory");
    let artifact = result.expect("timer trace case artifact must be written");
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
        "timer trace failures: {}",
        failed.join("; ")
    );
}

fn run_trace_case(case: &CaseRecord, probe: &mut TimerProbe) -> Result<CaseExecution, String> {
    let source = match case.id.as_str() {
        "IEC_TIMER_001_TOF_TIME_POST_EXPIRY_HOLD" => TOF_TIME_SOURCE,
        "IEC_TIMER_001_TOF_LTIME_POST_EXPIRY_HOLD" => TOF_LTIME_SOURCE,
        "IEC_TIMER_001_TON_TIME_BASIC_DELAY" => TON_TIME_SOURCE,
        "IEC_TIMER_001_TP_TIME_BASIC_PULSE" => TP_TIME_SOURCE,
        other => return Err(format!("unreviewed timer trace case {other}")),
    };
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    let mut harness = TestHarness::from_source(source)
        .map_err(|error| format!("{} failed to compile: {error}", case.id))?;
    let mut mismatches = Vec::new();

    for step in trace {
        execute_trace_step(&mut harness, step, probe, &mut mismatches)?;
    }

    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("trace_passed".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("trace_mismatch".to_string()),
        })
    }
}

fn execute_trace_step(
    harness: &mut TestHarness,
    step: &TraceStep,
    probe: &mut TimerProbe,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    let input = trace_bool(&step.stimulus, "input")?;
    let advance_ms = trace_millis(&step.stimulus, "advance_ms")?;
    let expected_q = trace_bool(&step.expected, "q")?;
    let expected_et_ms = trace_millis(&step.expected, "et_ms")?;

    harness
        .try_set_input("timer_in", input)
        .map_err(|error| format!("step {} input write failed: {error}", step.sequence))?;
    if advance_ms > 0 {
        harness.advance_time(Duration::from_millis(advance_ms));
    }
    let cycle = harness.cycle();
    if !cycle.errors.is_empty() {
        return Err(format!(
            "step {} runtime errors: {:?}",
            step.sequence, cycle.errors
        ));
    }

    let actual_q = match harness.try_get_output("q") {
        Ok(Value::Bool(value)) => value,
        Ok(value) => {
            return Err(format!(
                "step {} q has wrong type: {value:?}",
                step.sequence
            ))
        }
        Err(error) => return Err(format!("step {} q read failed: {error}", step.sequence)),
    };
    let actual_et_ms = match harness.try_get_output("et") {
        Ok(Value::Time(value)) | Ok(Value::LTime(value)) => value.as_nanos() / 1_000_000,
        Ok(value) => {
            return Err(format!(
                "step {} et has wrong type: {value:?}",
                step.sequence
            ))
        }
        Err(error) => return Err(format!("step {} et read failed: {error}", step.sequence)),
    };
    probe.observed = Some(json!({
        "et_ms": actual_et_ms,
        "q": actual_q,
        "sequence": step.sequence,
    }));

    if actual_q != expected_q || actual_et_ms != expected_et_ms {
        mismatches.push(format!(
            "step {} expected q={expected_q}, et_ms={expected_et_ms}; observed q={actual_q}, et_ms={actual_et_ms}",
            step.sequence
        ));
    }
    Ok(())
}

fn trace_bool(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<bool, String> {
    values
        .get(key)
        .and_then(toml::Value::as_bool)
        .ok_or_else(|| format!("trace field {key} must be BOOL"))
}

fn trace_millis(values: &BTreeMap<String, toml::Value>, key: &str) -> Result<i64, String> {
    let value = values
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("trace field {key} must be integer milliseconds"))?;
    if value < 0 {
        return Err(format!("trace field {key} must be non-negative"));
    }
    Ok(value)
}

struct TimerProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl Default for TimerProbe {
    fn default() -> Self {
        Self {
            observed: None,
            next_snapshot_is_before: true,
        }
    }
}

impl StateProbe for TimerProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if self.next_snapshot_is_before {
            self.observed = None;
        }
        self.next_snapshot_is_before = !self.next_snapshot_is_before;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.observed.clone(),
            siblings: BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
