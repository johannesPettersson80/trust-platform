mod trace_cases {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};

    use serde_json::{json, Value as JsonValue};
    use verification_cases::{
        run_case_file, CaseExecution, CaseRecord, CaseResult, RunConfig, StateProbe,
        StateSnapshot, TraceStep,
    };

    use super::{EthercatIoDriver, IoDriver, IoDriverHealth};

    const TEST_ID: &str = "TEST_ETHERCAT_UNAVAILABLE_RESOURCE_TRACE_001";
    const CASE_FILE: &str = "verification/cases/protocols/PROTO_ETHERCAT_001.toml";
    const CASE_FILE_DIGEST: &str =
        "sha256:07c0e48f9133daa2b31968e2ee3991d360fd5f03087c869cc0606952ebab0149";

    #[test]
    #[cfg(all(feature = "ethercat-wire", unix))]
    fn ethercat_unavailable_resource_trace_cases() {
        let mut probe = EthercatProbe::default();
        let config = RunConfig::new(TEST_ID, workspace_root().join(CASE_FILE), CASE_FILE_DIGEST);
        let artifact = run_case_file(&config, &mut probe, run_ethercat_case)
            .expect("EtherCAT unavailable-resource artifact must be written");
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
            "EtherCAT unavailable-resource failures: {}",
            failed.join("; ")
        );
    }

    fn run_ethercat_case(
        case: &CaseRecord,
        probe: &mut EthercatProbe,
    ) -> Result<CaseExecution, String> {
        let trace = case
            .trace
            .as_deref()
            .ok_or_else(|| format!("{} has no EtherCAT trace", case.id))?;
        let mut fixture = EthercatFixture::default();
        let mut mismatches = Vec::new();
        for step in trace {
            let observed = fixture.execute(step)?;
            compare_expected(step, &observed, &mut mismatches)?;
            probe.observed = Some(observed);
        }
        Ok(case_execution(mismatches))
    }

    #[derive(Default)]
    struct EthercatFixture {
        driver: Option<EthercatIoDriver>,
    }

    impl EthercatFixture {
        fn execute(&mut self, step: &TraceStep) -> Result<JsonValue, String> {
            match trace_string(&step.stimulus, "action")?.as_str() {
                "construct_missing" => {
                    let params: toml::Value = toml::from_str(
                        r#"
adapter = "trust-missing-ethercat-verification0"
timeout_ms = 10
on_error = "warn"
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
[[modules]]
model = "EL2008"
slot = 2
"#,
                    )
                    .map_err(|error| error.to_string())?;
                    let driver = EthercatIoDriver::from_params(&params)
                        .map_err(|error| format!("missing-adapter construction failed: {error}"))?;
                    let health = health_name(driver.health());
                    self.driver = Some(driver);
                    Ok(json!({
                        "construction": "accepted",
                        "health": health,
                        "hardware_proof": false,
                    }))
                }
                "read_missing" => {
                    let driver = self
                        .driver
                        .as_mut()
                        .ok_or_else(|| "missing-adapter driver was not constructed".to_string())?;
                    let mut inputs = [0u8; 1];
                    let error = driver
                        .read_inputs(&mut inputs)
                        .expect_err("missing adapter must reject the wire operation")
                        .to_string();
                    Ok(json!({
                        "operation": "rejected",
                        "health": health_name(driver.health()),
                        "error_contains": if error.contains("ethercat hardware unavailable until driver rebuild") {
                            "ethercat hardware unavailable until driver rebuild"
                        } else {
                            error.as_str()
                        },
                        "retry_scheduled": error.contains("retry in"),
                        "hardware_proof": false,
                    }))
                }
                "construct_mock" => {
                    let params: toml::Value = toml::from_str(
                        r#"
adapter = "mock"
mock_inputs = ["01"]
[[modules]]
model = "EK1100"
slot = 0
[[modules]]
model = "EL1008"
slot = 1
[[modules]]
model = "EL2008"
slot = 2
"#,
                    )
                    .map_err(|error| error.to_string())?;
                    let driver = EthercatIoDriver::from_params(&params)
                        .map_err(|error| format!("mock construction failed: {error}"))?;
                    let health = health_name(driver.health());
                    self.driver = Some(driver);
                    Ok(json!({
                        "construction": "accepted",
                        "health": health,
                        "hardware_proof": false,
                    }))
                }
                "read_mock" => {
                    let driver = self
                        .driver
                        .as_mut()
                        .ok_or_else(|| "mock driver was not constructed".to_string())?;
                    let mut inputs = [0u8; 1];
                    driver
                        .read_inputs(&mut inputs)
                        .map_err(|error| format!("mock read failed: {error}"))?;
                    Ok(json!({
                        "operation": "accepted",
                        "health": health_name(driver.health()),
                        "input_byte": inputs[0],
                        "hardware_proof": false,
                    }))
                }
                other => Err(format!("unreviewed EtherCAT trace action {other}")),
            }
        }
    }

    fn health_name(health: IoDriverHealth) -> &'static str {
        match health {
            IoDriverHealth::Ok => "ok",
            IoDriverHealth::Degraded { .. } => "degraded",
            IoDriverHealth::Faulted { .. } => "faulted",
        }
    }

    fn compare_expected(
        step: &TraceStep,
        observed: &JsonValue,
        mismatches: &mut Vec<String>,
    ) -> Result<(), String> {
        for (key, expected) in &step.expected {
            let expected = serde_json::to_value(expected).map_err(|error| error.to_string())?;
            let actual = observed.get(key).cloned().unwrap_or(JsonValue::Null);
            if actual != expected {
                mismatches.push(format!(
                    "step {} {key}: expected {expected}, observed {actual}",
                    step.sequence
                ));
            }
        }
        Ok(())
    }

    fn trace_string(
        values: &BTreeMap<String, toml::Value>,
        key: &str,
    ) -> Result<String, String> {
        values
            .get(key)
            .and_then(toml::Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| format!("trace {key} must be a string"))
    }

    fn case_execution(mismatches: Vec<String>) -> CaseExecution {
        if mismatches.is_empty() {
            CaseExecution {
                result: CaseResult::Passed,
                observed_error: None,
                observed_status: Some("ethercat_resource_trace_passed".to_string()),
            }
        } else {
            CaseExecution {
                result: CaseResult::Failed,
                observed_error: Some(mismatches.join("; ")),
                observed_status: Some("ethercat_resource_trace_mismatch".to_string()),
            }
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
    struct EthercatProbe {
        observed: Option<JsonValue>,
        next_snapshot_is_before: bool,
    }

    impl StateProbe for EthercatProbe {
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
                siblings: BTreeMap::new(),
                diagnostics: Vec::new(),
            })
        }
    }
}
