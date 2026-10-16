use verification_cases::{
    check_case_expectation, check_trace_fields, CaseExecution, CaseRecord, CaseResult,
    ObservedOperation, RunConfig, StateProbe, StateSnapshot, TraceStep,
};

/// Trace fields that `compare_nonfinite_mqtt_step` compares.
const NONFINITE_READ_FIELDS: &[&str] = &[
    "driver_health",
    "error_contains",
    "read_result",
    "visible_count",
    "visible_temp_milli",
];
const NONFINITE_TEST_ID: &str = "TEST_RUNTIME_MQTT_NONFINITE_BATCH_TRANSACTION_001";
const NONFINITE_CASE_FILE: &str = "verification/cases/runtime_safety/RT_SAFE_NAN_001.toml";
const NONFINITE_CASE_FILE_DIGEST: &str =
    "sha256:d6bd8d524b98a29f813f04bd0156a09ec8c241b873362b269fde937b1ed7e01a";
const NONFINITE_CASE_ID: &str =
    "RT_SAFE_NAN_001_MQTT_MAPPED_BATCH_REJECTION_AND_RECOVERY_ISOLATION";

fn run_nonfinite_mqtt_case() {
    let workspace = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("trust-runtime must be inside the workspace crates directory")
        .to_path_buf();
    let original_dir = std::env::current_dir().expect("current directory must be readable");
    std::env::set_current_dir(&workspace).expect("MQTT case runner must enter workspace root");

    let mut probe = NonfiniteMqttProbe::default();
    let config = RunConfig::new(
        NONFINITE_TEST_ID,
        NONFINITE_CASE_FILE,
        NONFINITE_CASE_FILE_DIGEST,
    );
    let result =
        verification_cases::run_case_file(&config, &mut probe, run_nonfinite_mqtt_case_record);

    std::env::set_current_dir(original_dir)
        .expect("MQTT case runner must restore current directory");
    let artifact = result.expect("MQTT non-finite case artifact must be written");
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
        "MQTT non-finite transaction failures: {}",
        failed.join("; ")
    );
}

fn run_nonfinite_mqtt_case_record(
    case: &CaseRecord,
    probe: &mut NonfiniteMqttProbe,
) -> Result<CaseExecution, String> {
    if case.id != NONFINITE_CASE_ID {
        return Err(format!("unreviewed MQTT non-finite case {}", case.id));
    }
    let trace = case
        .trace
        .as_deref()
        .ok_or_else(|| format!("{} has no trace", case.id))?;
    if trace.len() != 3 {
        return Err(format!(
            "{} must contain exactly three trace steps",
            case.id
        ));
    }

    let state = Arc::new(Mutex::new(MockState {
        connected: true,
        ..MockState::default()
    }));
    let factory = Arc::new(MockFactory {
        state: Arc::clone(&state),
        attempts: Arc::new(AtomicUsize::new(0)),
        fail_first: false,
        always_fail: false,
    });
    let mut driver = MqttIoDriver::from_params_with_factory(
        &params(
            r#"
broker = "127.0.0.1:1883"
reconnect_ms = 1

[[input_points]]
topic = "line/in/count"
image_offset = 0
data_type = "u16"
payload_format = "text"

[[input_points]]
topic = "line/in/temp"
image_offset = 2
data_type = "f32"
payload_format = "text"
"#,
        ),
        factory,
    )
    .map_err(|error| format!("construct mapped MQTT driver: {error}"))?;

    let mut inputs = [0u8; 6];
    let mut mismatches = Vec::new();
    let mut observations = Vec::new();
    let mut observed_steps = Vec::new();
    // The mapped input image after each step, so the operation under test can be
    // evaluated from the image before it and the image after it.
    let mut input_images = Vec::new();
    for step in trace {
        if let Err(error) = check_trace_fields(&step.expected, NONFINITE_READ_FIELDS) {
            mismatches.push(format!("{} step {}: {error}", case.id, step.sequence));
        }
        let observation = execute_nonfinite_mqtt_step(step, &state, &mut driver, &mut inputs)?;
        compare_nonfinite_mqtt_step(step, &observation, &mut mismatches)?;
        observations.push(observation.to_json());
        observed_steps.push(observation);
        input_images.push(inputs);
    }
    probe.observed = Some(serde_json::json!({ "trace": observations }));

    // The case is titled rejection and recovery, and its expectations describe the
    // rejected non-finite batch followed by its recovery. The operation under test runs
    // from the state the accepted batch before the rejected step left, to the state the
    // final recovery step left. Its diagnostic is the stable error of the rejected step.
    // The per-step trace comparisons above still assert that the rejected step leaves
    // the count and the temperature unchanged.
    let last = observed_steps
        .len()
        .checked_sub(1)
        .ok_or_else(|| format!("{} has no trace steps", case.id))?;
    let operation = trace
        .iter()
        .position(|step| {
            step.expected
                .get("read_result")
                .and_then(|value| value.as_str())
                == Some("rejected")
        })
        .ok_or_else(|| format!("{} has no rejected read step", case.id))?;
    if operation == 0 || operation >= last {
        return Err(format!(
            "{} rejected read must follow an accepted batch and precede a recovery step",
            case.id
        ));
    }
    let rejected = &observed_steps[operation];
    let recovery = &observed_steps[last];
    let before = nonfinite_snapshot(&input_images[operation - 1], Vec::new());
    let after = nonfinite_snapshot(
        &input_images[last],
        rejected.error.iter().cloned().collect(),
    );
    let observed = ObservedOperation {
        outcome: if recovery.read_result == "accepted" {
            "accept_value"
        } else {
            "reject"
        },
        fault_surface: None,
        // The resource run status is not observed here: this runner drives the MQTT
        // driver without a resource. The driver health is not the run status.
        status_before: None,
        status_after: None,
        before: &before,
        after: &after,
    };
    match check_case_expectation(case, &observed) {
        Ok(declared) => mismatches.extend(declared),
        Err(error) => mismatches.push(error),
    }

    if mismatches.is_empty() {
        Ok(CaseExecution {
            result: CaseResult::Passed,
            observed_error: None,
            observed_status: Some("mqtt_nonfinite_transaction_passed".to_string()),
        })
    } else {
        Ok(CaseExecution {
            result: CaseResult::Failed,
            observed_error: Some(mismatches.join("; ")),
            observed_status: Some("mqtt_nonfinite_transaction_mismatch".to_string()),
        })
    }
}

#[derive(Debug)]
struct NonfiniteMqttStepObservation {
    sequence: usize,
    read_result: &'static str,
    error: Option<String>,
    visible_count: u16,
    visible_temp_milli: i64,
    driver_health: &'static str,
}

impl NonfiniteMqttStepObservation {
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "driver_health": self.driver_health,
            "error": self.error,
            "read_result": self.read_result,
            "sequence": self.sequence,
            "visible_count": self.visible_count,
            "visible_temp_milli": self.visible_temp_milli,
        })
    }
}

fn execute_nonfinite_mqtt_step(
    step: &TraceStep,
    state: &Arc<Mutex<MockState>>,
    driver: &mut MqttIoDriver,
    inputs: &mut [u8; 6],
) -> Result<NonfiniteMqttStepObservation, String> {
    let action = nonfinite_trace_string(&step.stimulus, "action")?;
    {
        let mut guard = state.lock().unwrap_or_else(|error| error.into_inner());
        match action.as_str() {
            "deliver_finite_batch" => {
                let count = nonfinite_trace_nonnegative_integer(&step.stimulus, "count")?;
                let temp_milli = nonfinite_trace_nonnegative_integer(&step.stimulus, "temp_milli")?;
                guard
                    .payloads
                    .push_back(packet("line/in/count", count.to_string().into_bytes()));
                guard.payloads.push_back(packet(
                    "line/in/temp",
                    format_milli_payload(temp_milli).into_bytes(),
                ));
            }
            "deliver_nonfinite_batch" => {
                let count = nonfinite_trace_nonnegative_integer(&step.stimulus, "count")?;
                let temp_token = nonfinite_trace_string(&step.stimulus, "temp_token")?;
                guard
                    .payloads
                    .push_back(packet("line/in/count", count.to_string().into_bytes()));
                guard
                    .payloads
                    .push_back(packet("line/in/temp", temp_token.into_bytes()));
            }
            "deliver_finite_point_after_wait" => {
                let temp_milli = nonfinite_trace_nonnegative_integer(&step.stimulus, "temp_milli")?;
                guard.payloads.push_back(packet(
                    "line/in/temp",
                    format_milli_payload(temp_milli).into_bytes(),
                ));
            }
            _ => return Err(format!("unreviewed MQTT trace action {action}")),
        }
    }
    if action == "deliver_finite_point_after_wait" {
        let wait_ms = nonfinite_trace_nonnegative_integer(&step.stimulus, "wait_ms")?;
        thread::sleep(StdDuration::from_millis(
            u64::try_from(wait_ms).map_err(|_| "wait_ms does not fit u64".to_string())?,
        ));
    }

    let result = driver.read_inputs(inputs);
    let (read_result, error) = match result {
        Ok(()) => ("accepted", None),
        Err(error) => ("rejected", Some(error.to_string())),
    };
    let visible_count = u16::from_le_bytes([inputs[0], inputs[1]]);
    let visible_temp = f32::from_le_bytes(
        inputs[2..6]
            .try_into()
            .map_err(|_| "MQTT input image did not contain four REAL bytes".to_string())?,
    );

    Ok(NonfiniteMqttStepObservation {
        sequence: usize::try_from(step.sequence)
            .map_err(|_| "trace sequence does not fit usize".to_string())?,
        read_result,
        error,
        visible_count,
        visible_temp_milli: (f64::from(visible_temp) * 1_000.0).round() as i64,
        driver_health: nonfinite_health_label(driver.health()),
    })
}

fn compare_nonfinite_mqtt_step(
    step: &TraceStep,
    observation: &NonfiniteMqttStepObservation,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    for (field, expected) in &step.expected {
        let matches = match field.as_str() {
            "driver_health" => expected.as_str() == Some(observation.driver_health),
            "error_contains" => expected.as_str().is_some_and(|needle| {
                observation
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains(needle))
            }),
            "read_result" => expected.as_str() == Some(observation.read_result),
            "visible_count" => expected.as_integer() == Some(i64::from(observation.visible_count)),
            "visible_temp_milli" => expected.as_integer() == Some(observation.visible_temp_milli),
            _ => return Err(format!("unreviewed MQTT expected field {field}")),
        };
        if !matches {
            mismatches.push(format!(
                "trace step {} expected {field}={expected}, observed {}",
                step.sequence,
                observed_nonfinite_field(field, observation)
            ));
        }
    }
    Ok(())
}

fn observed_nonfinite_field(field: &str, observation: &NonfiniteMqttStepObservation) -> String {
    match field {
        "driver_health" => observation.driver_health.to_string(),
        "error_contains" => observation.error.as_deref().unwrap_or("none").to_string(),
        "read_result" => observation.read_result.to_string(),
        "visible_count" => observation.visible_count.to_string(),
        "visible_temp_milli" => observation.visible_temp_milli.to_string(),
        _ => "unreviewed".to_string(),
    }
}

fn nonfinite_health_label(health: IoDriverHealth) -> &'static str {
    match health {
        IoDriverHealth::Ok => "ok",
        IoDriverHealth::Degraded { .. } => "degraded",
        IoDriverHealth::Faulted { .. } => "faulted",
    }
}

/// Decodes the visible count (UINT at offset 0) and temperature (REAL at offset 2, in milli-units).
fn decode_visible(inputs: &[u8; 6]) -> (u16, i64) {
    let count = u16::from_le_bytes([inputs[0], inputs[1]]);
    let temp = f32::from_le_bytes([inputs[2], inputs[3], inputs[4], inputs[5]]);
    (count, (f64::from(temp) * 1_000.0).round() as i64)
}

/// Snapshot of the mapped input image that this runner reads.
fn nonfinite_snapshot(inputs: &[u8; 6], diagnostics: Vec<String>) -> StateSnapshot {
    let (count, temp_milli) = decode_visible(inputs);
    StateSnapshot {
        process_image_hash: Some(format!("{inputs:?}")),
        retain_hash: None,
        target: Some(serde_json::json!(temp_milli)),
        siblings: std::collections::BTreeMap::from([(
            "visible_count".to_string(),
            serde_json::json!(count),
        )]),
        diagnostics,
    }
}

fn format_milli_payload(value: i64) -> String {
    let whole = value / 1_000;
    let fraction = value % 1_000;
    format!("{whole}.{fraction:03}")
}

fn nonfinite_trace_nonnegative_integer(
    values: &std::collections::BTreeMap<String, toml::Value>,
    key: &str,
) -> Result<i64, String> {
    let value = values
        .get(key)
        .and_then(toml::Value::as_integer)
        .ok_or_else(|| format!("trace field {key} must be an integer"))?;
    if value < 0 {
        return Err(format!("trace field {key} must be non-negative"));
    }
    Ok(value)
}

fn nonfinite_trace_string(
    values: &std::collections::BTreeMap<String, toml::Value>,
    key: &str,
) -> Result<String, String> {
    values
        .get(key)
        .and_then(toml::Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| format!("trace field {key} must be a string"))
}

#[derive(Default)]
struct NonfiniteMqttProbe {
    observed: Option<serde_json::Value>,
    next_snapshot_is_before: bool,
}

impl StateProbe for NonfiniteMqttProbe {
    type Error = String;

    fn snapshot(&mut self) -> Result<StateSnapshot, Self::Error> {
        if !self.next_snapshot_is_before {
            self.observed = None;
        }
        self.next_snapshot_is_before = !self.next_snapshot_is_before;
        Ok(StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: self.observed.clone(),
            siblings: std::collections::BTreeMap::new(),
            diagnostics: Vec::new(),
        })
    }
}
