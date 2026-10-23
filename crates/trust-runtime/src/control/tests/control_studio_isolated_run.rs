/// One channel's value in a compact sample; the channel order follows the run's `channels`.
fn channel_value(run: &serde_json::Value, sample: usize, channel: &str) -> serde_json::Value {
    let position = run["channels"]
        .as_array()
        .unwrap()
        .iter()
        .position(|candidate| candidate["id"] == channel)
        .expect("observed channel");
    run["samples"][sample]["values"][position].clone()
}
/// One channel's value in a run's last sample.
fn last_channel_value(run: &serde_json::Value, channel: &str) -> serde_json::Value {
    let last = run["samples"].as_array().unwrap().len() - 1;
    channel_value(run, last, channel)
}
fn isolated_run_request(revision: serde_json::Value) -> serde_json::Value {
    json!({"expected_project_revision":revision,"execution":"isolated-simulation","step_ms":100,"duration_ms":2000,"seed":7,"max_total_cycles":21,"wall_budget_ms":5000,"initial_values":{},"events":[],"observations":[{"symbol":"Main.H1","unit":"m"},{"symbol":"Main.H2","unit":"m"}],"parameter_overrides":[]})
}
#[test]
fn isolated_run_starts_at_true_nonequilibrium_initial_state_without_dummy_parameters() {
    let (mut state, root) = search_fixture();
    let source = "PROGRAM Main VAR H1 AT %IL0 : LREAL; H2 AT %IL8 : LREAL; Pump AT %QL0 : LREAL := 0.010; END_VAR END_PROGRAM";
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("Main.st"),
        text: source.into(),
    }]);
    std::fs::write(root.path().join("simulation.toml"),"[simulation]\nenabled=true\nplant_step_ms=100\n[[plants]]\nid=\"tanks\"\nmodel=\"gravity_tank_cascade\"\ncommand_sources=[\"%QL0\"]\nfeedback_targets=[\"%IL0\",\"%IL8\"]\ninitial_state=[0.27,0.25]\ninitial_input=[0.010]\n[plants.parameters]\narea_m2=0.5\noutflow_coefficient_m2_sqrt_m_per_s=0.020\n").unwrap();
    let readiness = handle_request_value(
        json!({"id":8800,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    );
    assert!(
        readiness.ok,
        "ordinary isolated Test must be supported: {:?}",
        readiness.error
    );
    let revision = readiness.result.unwrap()["project_revision"].clone();
    std::thread::sleep(std::time::Duration::from_millis(40));
    let start = handle_request_value(
        json!({"id":8801,"type":"control.simulation.run.start","params":isolated_run_request(revision.clone())}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let response = handle_request_value(
            json!({"id":8802,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),
            &state,
            None,
        );
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.unwrap();
        if result["status"] == "completed" {
            assert!(result.get("ranking").is_none());
            let run = &result["run"];
            assert_eq!(run["evidence"], "ST-executed");
            assert_eq!(run["parameters"], json!({}));
            assert_eq!(run["samples"][0]["timeSeconds"], 0.0);
            assert_eq!(channel_value(run, 0, "Main.H1"), 0.27);
            assert_eq!(channel_value(run, 0, "Main.H2"), 0.25);
            assert_ne!(last_channel_value(run, "Main.H1"), 0.27);
            break;
        }
        assert_ne!(result["status"], "failed", "{result}");
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn isolated_run_materializes_the_selected_inline_plant_with_model_revision_and_provenance() {
    let (mut state, _root) = search_fixture();
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("Main.st"),
        text: "PROGRAM Main VAR H1 AT %IL0 : LREAL; H2 AT %IL8 : LREAL; Pump AT %QL0 : LREAL := 0.010; END_VAR END_PROGRAM".into(),
    }]);
    let revision = handle_request_value(
        json!({"id":8805,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let selected_plant = r#"[simulation]
enabled = true
plant_step_ms = 100
[[plants]]
id = "selected-gravity-cascade"
model = "gravity_tank_cascade"
command_sources = ["%QL0"]
feedback_targets = ["%IL0", "%IL8"]
initial_state = [0.61, 0.52]
initial_input = [0.010]
[plants.parameters]
area_m2 = 0.5
outflow_coefficient_m2_sqrt_m_per_s = 0.020
"#;
    let mut request = isolated_run_request(revision.clone());
    request["simulation_override"] = json!({
        "model_revision":"selected-gravity-cascade:v2",
        "model_provenance":"declared-supplied-model",
        "simulation_toml":selected_plant,
    });
    let start = handle_request_value(
        json!({"id":8806,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(
        start.ok,
        "selected compatible inline plant must start an isolated actual-ST run: {:?}",
        start.error
    );
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let response = handle_request_value(
            json!({"id":8807,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),
            &state,
            None,
        );
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.unwrap();
        if result["status"] == "completed" {
            let run = &result["run"];
            assert_eq!(result["project_revision"], revision);
            assert_eq!(
                run["conditions"]["modelRevision"],
                "selected-gravity-cascade:v2"
            );
            assert_eq!(
                run["conditions"]["modelProvenance"],
                "declared-supplied-model"
            );
            use sha2::Digest;
            assert_eq!(
                run["conditions"]["simulationConfigRevision"],
                format!(
                    "sha256:{}",
                    hex::encode(sha2::Sha256::digest(selected_plant.as_bytes()))),
                "configuration identity must hash the exact UTF-8 TOML bytes"
            );
            assert_eq!(channel_value(run, 0, "Main.H1"), 0.61);
            assert_eq!(channel_value(run, 0, "Main.H2"), 0.52);
            assert_eq!(
                result["io_provenance"],
                "isolated-simulation-no-physical-drivers"
            );
            break;
        }
        assert_ne!(result["status"], "failed", "{result}");
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn isolated_run_drives_the_selected_plant_through_program_variables_without_addresses() {
    // Specification 46 §13.1–§13.2 and Specification 35 §17.1: a controller output and a measurement without I/O addresses.
    let (mut state, _root) = search_fixture();
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("Main.st"),
        text: "PROGRAM Main VAR Output : LREAL := 4.0; Measurement : LREAL; END_VAR END_PROGRAM"
            .into(),
    }]);
    let revision = handle_request_value(
        json!({"id":8830,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let selected_plant = "[simulation]\nenabled = true\nplant_step_ms = 100\n[[plants]]\nid = \"selected-model\"\nmodel = \"static_gain\"\ncommand_sources = [\"Main.Output\"]\nfeedback_targets = [\"Main.Measurement\"]\n[plants.parameters]\ngain = 0.5\n";
    let request = json!({"expected_project_revision":revision,"execution":"isolated-simulation","step_ms":100,"duration_ms":1000,"seed":7,
        "max_total_cycles":11,"wall_budget_ms":5000,"initial_values":{},"events":[],
        "observations":[{"symbol":"Main.Output","unit":"%"},{"symbol":"Main.Measurement","unit":"m"}],"parameter_overrides":[],
        "simulation_override":{"model_revision":"symbolic:v1","model_provenance":"declared-fitted-validated","simulation_toml":selected_plant}});
    let start = handle_request_value(
        json!({"id":8831,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(
        start.ok,
        "a plant on unaddressed program variables starts an isolated run: {:?}",
        start.error
    );
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let response = handle_request_value(
            json!({"id":8832,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),
            &state,
            None,
        );
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.unwrap();
        if result["status"] == "completed" {
            let run = &result["run"];
            assert_eq!(last_channel_value(run, "Main.Output"), 4.0);
            assert_eq!(
                last_channel_value(run, "Main.Measurement"), 2.0,
                "the plant read the output and wrote the measurement: {run}"
            );
            break;
        }
        assert_ne!(result["status"], "failed", "{result}");
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn isolated_run_rejects_unmaterialized_or_nonplant_selected_model_configuration() {
    let (state, _root) = search_fixture();
    let revision = handle_request_value(
        json!({"id":8808,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    for simulation_toml in [
        "path = '/outside/simulation.toml'",
        "[simulation]\nenabled = true",
    ] {
        let mut request = isolated_run_request(revision.clone());
        request["simulation_override"] = json!({
            "model_revision":"candidate-revision",
            "model_provenance":"declared-supplied-model",
            "simulation_toml":simulation_toml,
        });
        let response = handle_request_value(
            json!({"id":8809,"type":"control.simulation.run.start","params":request}),
            &state,
            None,
        );
        assert!(!response.ok, "{simulation_toml:?} must be rejected");
        assert_eq!(
            response.error_code.as_deref(),
            Some("invalid_simulation_run_request")
        );
    }
}
#[test]
fn isolated_run_names_the_exceeded_recording_bound() {
    let (state, _root) = search_fixture();
    let ready = handle_request_value(
        json!({"id":8812,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap();
    let channels = |count: usize| {
        json!((0..count)
            .map(|index| json!({"symbol":format!("Main.Signal{index}"),"unit":"1"}))
            .collect::<Vec<_>>())
    };
    let refusal = |request: serde_json::Value| {
        let response = handle_request_value(
            json!({"id":8813,"type":"control.simulation.run.start","params":request}),
            &state,
            None,
        );
        assert!(!response.ok);
        response.error.unwrap_or_default()
    };
    let mut request = isolated_run_request(ready["project_revision"].clone());
    // A decoupled PID group's transfer check records 34 channels (Specification 35 §17.15); the sample budget still bounds a run.
    request["observations"] = channels(65);
    assert!(
        refusal(request).contains("65 observed channels exceed the limit of 64"),
        "the refusal names the channel bound"
    );
    let mut request = isolated_run_request(ready["project_revision"].clone());
    request["step_ms"] = json!(20);
    request["duration_ms"] = json!(1_199_980);
    request["max_total_cycles"] = json!(60_000);
    request["observations"] = channels(30);
    assert!(
        refusal(request)
            .contains("60000 samples × 30 channels exceed the limit of 1600000 channel samples"),
        "the refusal names the channel-sample budget"
    );
    // A run keeps at most 100000 samples (A7) unless record_every averages them (A4.2).
    let mut request = isolated_run_request(ready["project_revision"].clone());
    request["step_ms"] = json!(1);
    request["duration_ms"] = json!(100_000);
    request["max_total_cycles"] = json!(100_001);
    let message = refusal(request);
    assert!(message.contains("100001 samples per run"), "{message}");
    assert!(message.contains("100000"), "{message}");
    let mut request = isolated_run_request(ready["project_revision"].clone());
    request["step_ms"] = json!(1);
    request["duration_ms"] = json!(86_400_001);
    request["max_total_cycles"] = json!(10_000_000);
    assert!(
        refusal(request).contains("86400000"),
        "a duration beyond 24 h is refused by name"
    );
}

#[test]
fn isolated_run_rejects_parameter_initial_conflicts_and_search_fields() {
    let (state, _root) = search_fixture();
    let ready = handle_request_value(
        json!({"id":8810,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap();
    let mut request = isolated_run_request(ready["project_revision"].clone());
    request["observations"] = json!([{"symbol":"Main.PV","unit":"C"}]);
    request["parameter_overrides"] = json!([{"symbol":"Main.Kp","iec_type":"LREAL","unit":"ratio","convention":"parallel proportional gain","minimum":0,"maximum":20,"value":2}]);
    request["initial_values"] = json!({"Main.Kp":3});
    let response = handle_request_value(
        json!({"id":8811,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(!response.ok);
    assert_eq!(
        response.error_code.as_deref(),
        Some("invalid_simulation_run_request")
    );
}

#[test]
fn isolated_run_parameter_candidates_share_conditions_and_keep_real_parameter_values() {
    let (state, _root) = search_fixture();
    let revision = handle_request_value(
        json!({"id":8820,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut runs = Vec::new();
    for gain in [1.0, 2.0] {
        let mut request = isolated_run_request(revision.clone());
        request["step_ms"] = json!(20);
        request["duration_ms"] = json!(200);
        request["max_total_cycles"] = json!(11);
        request["observations"] = json!([{"symbol":"Main.PV","unit":"C"}]);
        request["parameter_overrides"] = json!([{"symbol":"Main.Kp","iec_type":"LREAL","unit":"ratio","convention":"parallel proportional gain","minimum":0,"maximum":20,"value":gain}]);
        let start = handle_request_value(
            json!({"id":8821,"type":"control.simulation.run.start","params":request}),
            &state,
            None,
        );
        assert!(start.ok, "{:?}", start.error);
        let job = start.result.unwrap()["job_id"].clone();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let result=handle_request_value(json!({"id":8822,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),&state,None).result.unwrap();
            if result["status"] == "completed" {
                assert_eq!(result["run"]["parameters"]["Main.Kp"], gain);
                runs.push(result["run"].clone());
                break;
            }
            assert_ne!(result["status"], "failed", "{result}");
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        let cross = handle_request_value(
            json!({"id":8823,"type":"control.search.status","params":{"job_id":job,"expected_project_revision":revision}}),
            &state,
            None,
        );
        assert!(
            !cross.ok,
            "search namespace must not inspect ordinary Test job"
        );
    }
    assert_eq!(runs[0]["conditions"], runs[1]["conditions"]);
    assert_ne!(last_channel_value(&runs[0], "Main.PV"), last_channel_value(&runs[1], "Main.PV"));
}

#[test]
fn isolated_run_bool_configuration_is_a_fixed_parameter_not_initial_state() {
    let (mut state, _root) = search_fixture();
    state.sources=SourceRegistry::new(vec![SourceFile{id:1,path:PathBuf::from("Main.st"),text:"PROGRAM Main VAR SP : LREAL := 1.0; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; HighGainConfig : BOOL := TRUE; END_VAR IF HighGainConfig THEN MV := 2.0 * (SP-PV); ELSE MV := SP-PV; END_IF; END_PROGRAM".into()}]);
    let revision = handle_request_value(
        json!({"id":8830,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut runs = Vec::new();
    for value in [0.0, 1.0] {
        let mut request = isolated_run_request(revision.clone());
        request["step_ms"] = json!(20);
        request["duration_ms"] = json!(200);
        request["max_total_cycles"] = json!(11);
        request["observations"] = json!([{"symbol":"Main.PV","unit":"C"}]);
        request["parameter_overrides"] = json!([{"symbol":"Main.HighGainConfig","iec_type":"BOOL","unit":"1","convention":"fixed controller configuration","minimum":0,"maximum":1,"value":value}]);
        let start = handle_request_value(
            json!({"id":8831,"type":"control.simulation.run.start","params":request}),
            &state,
            None,
        );
        assert!(
            start.ok,
            "simulation-only fixed BOOL configuration must be a parameter: {:?}",
            start.error
        );
        let job = start.result.unwrap()["job_id"].clone();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let result=handle_request_value(json!({"id":8832,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),&state,None).result.unwrap();
            if result["status"] == "completed" {
                assert_eq!(result["run"]["parameters"]["Main.HighGainConfig"], value);
                runs.push(result["run"].clone());
                break;
            }
            assert_ne!(result["status"], "failed", "{result}");
            assert!(std::time::Instant::now() < deadline);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
    }
    assert_eq!(runs[0]["conditions"], runs[1]["conditions"]);
    assert_eq!(runs[0]["conditions"]["initialState"], json!({}));
    assert_ne!(last_channel_value(&runs[0], "Main.PV"), last_channel_value(&runs[1], "Main.PV"));
}

#[test]
fn isolated_run_writes_exact_integer_scenario_initial_values_and_events() {
    let (mut state, _root) = search_fixture();
    state.sources=SourceRegistry::new(vec![SourceFile{id:1,path:PathBuf::from("Main.st"),text:"PROGRAM Main VAR TrialKind AT %MW0 : INT; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; END_VAR IF TrialKind = 1 THEN MV := 0.1; ELSE MV := 0.2; END_IF; END_PROGRAM".into()}]);
    let revision = handle_request_value(
        json!({"id":8840,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision.clone());
    request["step_ms"] = json!(20);
    request["duration_ms"] = json!(200);
    request["max_total_cycles"] = json!(11);
    request["observations"] =
        json!([{"symbol":"Main.TrialKind","unit":"1"},{"symbol":"Main.PV","unit":"C"}]);
    request["initial_values"] = json!({"Main.TrialKind":1});
    request["events"] = json!([{"at_ms":100,"symbol":"Main.TrialKind","value":2}]);
    let start = handle_request_value(
        json!({"id":8841,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let result=handle_request_value(json!({"id":8842,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),&state,None).result.unwrap();
        if result["status"] == "completed" {
            assert_eq!(result["run"]["parameters"], json!({}));
            assert_eq!(channel_value(&result["run"], 0, "Main.TrialKind"), 1.0);
            assert_eq!(channel_value(&result["run"], 5, "Main.TrialKind"), 2.0);
            break;
        }
        assert_ne!(
            result["status"], "failed",
            "exact integer scenario inputs must execute: {result}"
        );
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn isolated_run_cancel_is_scoped_and_reports_terminal_cancellation() {
    let (state, _root) = search_fixture();
    let revision = handle_request_value(
        json!({"id":8850,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision.clone());
    request["step_ms"] = json!(20);
    request["duration_ms"] = json!(200000);
    request["max_total_cycles"] = json!(10001);
    request["observations"] = json!([{"symbol":"Main.PV","unit":"C"}]);
    let start = handle_request_value(
        json!({"id":8851,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let wrong = handle_request_value(
        json!({"id":8852,"type":"control.search.cancel","params":{"job_id":job,"expected_project_revision":revision}}),
        &state,
        None,
    );
    assert!(!wrong.ok);
    let cancel = handle_request_value(
        json!({"id":8853,"type":"control.simulation.run.cancel","params":{"job_id":job,"expected_project_revision":revision,"include_results":false}}),
        &state,
        None,
    );
    assert!(cancel.ok, "{:?}", cancel.error);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let result=handle_request_value(json!({"id":8854,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision,"include_results":false}}),&state,None).result.unwrap();
        assert!(result.get("run").is_none());
        if result["status"] == "cancelled" {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn isolated_run_fault_retains_the_coherent_prefix_without_claiming_completion() {
    let (mut state, _root) = search_fixture();
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("Main.st"),
        text: "PROGRAM Main VAR Cycle : INT := 0; Divisor : INT := 1; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; END_VAR Cycle := Cycle + 1; IF Cycle >= 3 THEN Divisor := 0; END_IF; MV := 1.0 / Divisor; END_PROGRAM".into(),
    }]);
    let revision = handle_request_value(
        json!({"id":8855,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision.clone());
    request["observations"] = json!([{"symbol":"Main.PV","unit":"C"}]);
    let started = handle_request_value(
        json!({"id":8856,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(started.ok, "{:?}", started.error);
    let job = started.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let thin = handle_request_value(
            json!({"id":8857,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision,"include_results":false}}),
            &state,
            None,
        );
        assert!(thin.ok, "{:?}", thin.error);
        let thin = thin.result.unwrap();
        assert!(
            thin.get("run").is_none(),
            "thin status must not carry retained samples"
        );
        if thin["status"] == "failed" {
            assert!(
                thin["reason"]
                    .as_str()
                    .is_some_and(|reason| reason.to_lowercase().contains("division")),
                "{thin}"
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{thin}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let detailed = handle_request_value(
        json!({"id":8858,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision,"include_results":true}}),
        &state,
        None,
    );
    assert!(detailed.ok, "{:?}", detailed.error);
    let detailed = detailed.result.unwrap();
    assert_eq!(detailed["status"], "failed");
    assert_eq!(detailed["source_basis"], "loaded-runtime-sources");
    assert_eq!(
        detailed["io_provenance"],
        "isolated-simulation-no-physical-drivers"
    );
    let run = &detailed["run"];
    assert_eq!(run["evidence"], "ST-executed");
    assert_eq!(run["parameters"], json!({}));
    assert_eq!(run["channels"], json!([{"id":"Main.PV","unit":"C"}]));
    assert_eq!(run["conditions"]["durationSeconds"], 2.0);
    assert_eq!(
        run["conditions"]["sampleTimesSeconds"],
        json!([
            0.0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.1, 1.2, 1.3, 1.4, 1.5, 1.6,
            1.7, 1.8, 1.9, 2.0
        ])
    );
    assert_eq!(run["samples"].as_array().unwrap().len(), 2);
    assert_eq!(run["samples"][0]["timeSeconds"], 0.0);
    assert_eq!(run["samples"][1]["timeSeconds"], 0.1);
    assert_eq!(run["samples"][0]["sequence"], 0);
    assert_eq!(run["samples"][1]["sequence"], 1);
    // Every channel is good, so the compact sample omits its quality array.
    assert_eq!(run["samples"][0]["values"].as_array().unwrap().len(), 1, "one value per channel");
    assert!(run["samples"][0].get("quality").is_none());
    assert!(run["samples"][1].get("quality").is_none());
}
#[test]
fn isolated_run_rejects_search_grid_and_fractional_integer_state() {
    let (mut state, _root) = search_fixture();
    state.sources=SourceRegistry::new(vec![SourceFile{id:1,path:PathBuf::from("Main.st"),text:"PROGRAM Main VAR TrialKind : INT; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; END_VAR MV := 0.1; END_PROGRAM".into()}]);
    let revision = handle_request_value(
        json!({"id":8860,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision.clone());
    request["observations"] = json!([{"symbol":"Main.PV","unit":"C"}]);
    request["grid"] = json!([]);
    let grid = handle_request_value(
        json!({"id":8861,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(!grid.ok);
    request.as_object_mut().unwrap().remove("grid");
    request["initial_values"] = json!({"Main.TrialKind":1.5});
    let start = handle_request_value(
        json!({"id":8862,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let result=handle_request_value(json!({"id":8863,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),&state,None).result.unwrap();
        if result["status"] == "failed" {
            assert!(result["reason"]
                .as_str()
                .unwrap()
                .contains("exactly integral"));
            assert!(result.get("run").is_none());
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn isolated_run_cancel_retains_samples_already_received_from_the_worker() {
    let (mut state, _root) = search_fixture();
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("Main.st"),
        text: "PROGRAM Main VAR PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; Work : DINT; Scratch : LREAL; END_VAR FOR Work := 1 TO 100 DO Scratch := Scratch + 0.0001; END_FOR; MV := 1.0; END_PROGRAM".into(),
    }]);
    let revision = handle_request_value(
        json!({"id":8870,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision.clone());
    request["step_ms"] = json!(1);
    request["duration_ms"] = json!(11999);
    request["max_total_cycles"] = json!(12000);
    request["observations"] =
        json!([{"symbol":"Main.PV","unit":"C"},{"symbol":"Main.MV","unit":"V"}]);
    let start = handle_request_value(
        json!({"id":8871,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    let observed = loop {
        let response = handle_request_value(
            json!({"id":8872,"type":"control.simulation.run.status","params":{
            "job_id":job,"expected_project_revision":revision,"include_results":false}}),
            &state,
            None,
        );
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.unwrap();
        assert!(result.get("run").is_none());
        let count = result["captured_sample_count"]
            .as_u64()
            .expect("Thin progress must report complete sample frames received from the worker");
        if count > 0 {
            break count;
        }
        assert!(
            matches!(result["status"].as_str(), Some("queued" | "running")),
            "{result}"
        );
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    let cancel = handle_request_value(
        json!({"id":8873,"type":"control.simulation.run.cancel","params":{
        "job_id":job,"expected_project_revision":revision,"include_results":false}}),
        &state,
        None,
    );
    assert!(cancel.ok, "{:?}", cancel.error);
    loop {
        let result = handle_request_value(
            json!({"id":8874,"type":"control.simulation.run.status","params":{
            "job_id":job,"expected_project_revision":revision,"include_results":false}}),
            &state,
            None,
        )
        .result
        .unwrap();
        if result["status"] == "cancelled" {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let details = handle_request_value(
        json!({"id":8875,"type":"control.simulation.run.status","params":{
        "job_id":job,"expected_project_revision":revision,"include_results":true}}),
        &state,
        None,
    )
    .result
    .unwrap();
    assert_eq!(details["status"], "cancelled");
    let run = &details["run"];
    assert_eq!(run["evidence"], "ST-executed");
    let rows = run["samples"]
        .as_array()
        .expect("Cancellation retains received raw samples");
    assert!(rows.len() >= observed as usize);
    assert!(
        rows.len() < 12000,
        "The test must interrupt an unfinished run, not relabel a complete record"
    );
    assert_eq!(
        run["conditions"]["sampleTimesSeconds"]
            .as_array()
            .unwrap()
            .len(),
        12000
    );
    let mv = run["channels"]
        .as_array()
        .unwrap()
        .iter()
        .position(|channel| channel["id"] == "Main.MV")
        .expect("observed MV");
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["sequence"], index);
        assert_eq!(row["timeSeconds"], index as f64 / 1000.0);
        assert_eq!(row["values"][mv], 1.0);
    }
}

#[test]
fn isolated_run_wall_budget_retains_the_received_prefix_as_failed_evidence() {
    let (mut state, _root) = search_fixture();
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1, path: PathBuf::from("Main.st"),
        text: "PROGRAM Main VAR PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; Work : DINT; Scratch : LREAL; END_VAR FOR Work := 1 TO 1000 DO Scratch := Scratch + 0.0001; END_FOR; MV := 1.0; END_PROGRAM".into(),
    }]);
    let revision = handle_request_value(
        json!({"id":8880,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision.clone());
    request["step_ms"] = json!(1);
    request["duration_ms"] = json!(11999);
    request["max_total_cycles"] = json!(12000);
    request["wall_budget_ms"] = json!(1000);
    request["observations"] =
        json!([{"symbol":"Main.PV","unit":"C"},{"symbol":"Main.MV","unit":"V"}]);
    let start = handle_request_value(
        json!({"id":8881,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let result = handle_request_value(
            json!({"id":8882,"type":"control.simulation.run.status","params":{
            "job_id":job,"expected_project_revision":revision,"include_results":false}}),
            &state,
            None,
        )
        .result
        .unwrap();
        assert!(result.get("run").is_none());
        if result["status"] == "failed" {
            break;
        }
        assert_ne!(
            result["status"], "completed",
            "The test must exhaust its wall budget before completing"
        );
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let details = handle_request_value(
        json!({"id":8883,"type":"control.simulation.run.status","params":{
        "job_id":job,"expected_project_revision":revision,"include_results":true}}),
        &state,
        None,
    )
    .result
    .unwrap();
    assert!(
        details["reason"].as_str().unwrap().contains("wall budget"),
        "{details}"
    );
    let run = &details["run"];
    assert_eq!(run["evidence"], "ST-executed");
    let rows = run["samples"]
        .as_array()
        .expect("The supervisor retains complete frames before its wall-budget kill");
    assert!(!rows.is_empty() && rows.len() < 12000);
    assert_eq!(
        rows.len() as u64,
        details["captured_sample_count"].as_u64().unwrap()
    );
    assert_eq!(
        run["conditions"]["sampleTimesSeconds"]
            .as_array()
            .unwrap()
            .len(),
        12000
    );
    let mv = run["channels"]
        .as_array()
        .unwrap()
        .iter()
        .position(|channel| channel["id"] == "Main.MV")
        .expect("observed MV");
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["sequence"], index);
        assert_eq!(row["timeSeconds"], index as f64 / 1000.0);
        assert_eq!(row["values"][mv], 1.0);
    }
}

#[test]
fn isolated_run_long_channel_names_complete_within_the_compact_checkpoint() {
    let (mut state, _root) = search_fixture();
    // 64 channels with 180-byte names: the compact frames name each channel once, in the header.
    let names: Vec<_> = (0..64)
        .map(|index| format!("Signal{index}_{}", "x".repeat(180)))
        .collect();
    let declarations = names
        .iter()
        .map(|name| format!("{name} : LREAL := 1.0;"))
        .collect::<String>();
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1, path: PathBuf::from("Main.st"),
        text: format!("PROGRAM Main VAR PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; {declarations} END_VAR MV := 1.0; END_PROGRAM"),
    }]);
    let revision = handle_request_value(
        json!({"id":8890,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision.clone());
    request["step_ms"] = json!(1);
    request["duration_ms"] = json!(11999);
    request["max_total_cycles"] = json!(12000);
    request["wall_budget_ms"] = json!(10000);
    request["observations"] = json!(names
        .iter()
        .map(|name| json!({"symbol":format!("Main.{name}"),"unit":"1"}))
        .collect::<Vec<_>>());
    let start = handle_request_value(
        json!({"id":8891,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    loop {
        let result = handle_request_value(
            json!({"id":8892,"type":"control.simulation.run.status","params":{
            "job_id":job,"expected_project_revision":revision,"include_results":false}}),
            &state,
            None,
        )
        .result
        .unwrap();
        if result["status"] == "completed" {
            break;
        }
        assert_ne!(result["status"], "failed", "{result}");
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let details = handle_request_value(
        json!({"id":8893,"type":"control.simulation.run.status","params":{
        "job_id":job,"expected_project_revision":revision,"include_results":true}}),
        &state,
        None,
    )
    .result
    .unwrap();
    assert_eq!(details["status"], "completed", "the largest valid run fits its budget");
    let run = &details["run"];
    assert_eq!(run["evidence"], "ST-executed");
    let samples = run["samples"].as_array().unwrap();
    assert_eq!(samples.len(), 12000);
    assert_eq!(samples[0]["values"].as_array().unwrap().len(), 64);
    assert!(samples[0].get("quality").is_none(), "every channel is good");
    assert!(serde_json::to_vec(run).unwrap().len() < 64 * 1024 * 1024);
    assert_eq!(
        run["conditions"]["sampleTimesSeconds"]
            .as_array()
            .unwrap()
            .len(),
        12000
    );
}

/// Ten grid points of 1 ms, averaged in periods of three (A4.2): four samples, the last one holding
/// the single point that remains, each timed at the mean of its grid times.
#[test]
fn isolated_run_record_every_averages_each_period_and_keeps_the_partial_last_one() {
    let (state, _root) = search_fixture();
    let ready = handle_request_value(
        json!({"id":8900,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap();
    let revision = ready["project_revision"].clone();
    let mut request = isolated_run_request(revision.clone());
    request["step_ms"] = json!(1);
    request["duration_ms"] = json!(9);
    request["max_total_cycles"] = json!(10);
    request["record_every"] = json!(3);
    request["observations"] = json!([{"symbol":"Main.SP","unit":"C"}]);
    let start = handle_request_value(
        json!({"id":8901,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let result = handle_request_value(
            json!({"id":8902,"type":"control.simulation.run.status","params":{
            "job_id":job,"expected_project_revision":revision,"include_results":false}}),
            &state,
            None,
        )
        .result
        .unwrap();
        if result["status"] == "completed" {
            break;
        }
        assert!(
            matches!(result["status"].as_str(), Some("queued" | "running")),
            "{result}"
        );
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let details = handle_request_value(
        json!({"id":8903,"type":"control.simulation.run.status","params":{
        "job_id":job,"expected_project_revision":revision,"include_results":true}}),
        &state,
        None,
    )
    .result
    .unwrap();
    let run = &details["run"];
    assert_eq!(run["conditions"]["recordEvery"], json!(3), "{details}");
    assert_eq!(
        run["conditions"]["sampleTimesSeconds"],
        json!([0.001, 0.004, 0.007, 0.009]),
        "{details}"
    );
    let samples = run["samples"].as_array().expect("run samples");
    assert_eq!(samples.len(), 4, "{details}");
    for (index, sample) in samples.iter().enumerate() {
        assert_eq!(sample["sequence"], json!(index), "{details}");
        assert_eq!(sample["values"][0], json!(1.0), "{details}");
        assert!(sample.get("quality").is_none(), "every channel is good: {details}");
    }
    assert_eq!(samples[3]["timeSeconds"], json!(0.009), "{details}");
}

/// Starts an isolated run of `request` and polls it until its job is terminal; returns the last status answer.
fn isolated_run_to_end(state: &ControlState, request: serde_json::Value) -> serde_json::Value {
    let revision = request["expected_project_revision"].clone();
    let start = handle_request_value(
        json!({"id":8900,"type":"control.simulation.run.start","params":request}),
        state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let result = handle_request_value(
            json!({"id":8901,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),
            state,
            None,
        )
        .result
        .unwrap();
        if matches!(result["status"].as_str(), Some("completed" | "failed" | "cancelled")) {
            return result;
        }
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

/// A candidate gain of 2 for `Main.Kp`, over one second of a P loop on the fixture's first-order plant.
fn startup_candidate_request(state: &ControlState) -> serde_json::Value {
    startup_candidate_request_for(state, "Main")
}

/// A candidate gain of 2 for the gain `Kp` of program instance `program`, over one second of a P loop on the fixture's plant.
fn startup_candidate_request_for(state: &ControlState, program: &str) -> serde_json::Value {
    let revision = handle_request_value(
        json!({"id":8902,"type":"control.search.status","params":{}}),
        state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = isolated_run_request(revision);
    request["step_ms"] = json!(20);
    request["duration_ms"] = json!(1000);
    request["max_total_cycles"] = json!(51);
    request["observations"] = json!([{"symbol":format!("{program}.Kp"),"unit":"ratio"},{"symbol":format!("{program}.PV"),"unit":"C"}]);
    request["parameter_overrides"] = json!([{"symbol":format!("{program}.Kp"),"iec_type":"LREAL","unit":"ratio","convention":"parallel proportional gain","minimum":0,"maximum":20,"value":2.0}]);
    request
}

/// A program that designs its own gain during its first scan, its startup, keeps the candidate that the run writes again at
/// the task boundary after that scan, as it keeps a record that Apply writes into the running program (E8.3).
#[test]
fn isolated_run_adopts_a_candidate_that_the_program_sets_only_during_its_startup_scan() {
    let (state, _root) = search_fixture_with(
        "PROGRAM Main VAR Kp : LREAL := 1.0; Started : BOOL := FALSE; SP : LREAL := 1.0; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; END_VAR IF NOT Started THEN Kp := 0.5; Started := TRUE; END_IF; MV := Kp * (SP - PV); END_PROGRAM",
    );
    let result = isolated_run_to_end(&state, startup_candidate_request(&state));
    assert_eq!(result["status"], "completed", "{result}");
    let run = &result["run"];
    assert_eq!(run["parameters"]["Main.Kp"], 2.0);
    let samples = run["samples"].as_array().unwrap().len();
    assert_eq!(samples, 51);
    for sample in 0..samples {
        assert_eq!(channel_value(run, sample, "Main.Kp"), json!(2.0), "sample {sample}");
    }
    // P control with Kp = 2 on the unit-gain plant settles at Kp/(1 + Kp)·SP = 2/3; the startup gain 0.5 would give 1/3.
    let settled = last_channel_value(run, "Main.PV").as_f64().unwrap();
    assert!((settled - 2.0 / 3.0).abs() < 0.01, "{settled}");
}

/// A program that writes its gain at every scan would overwrite a record that Apply writes too, so it does not adopt the
/// candidate: the run fails once the candidate has been written again after the startup scan (E8.3).
#[test]
fn isolated_run_refuses_a_candidate_that_the_program_overwrites_after_its_startup_scan() {
    let (state, _root) = search_fixture_with(
        "PROGRAM Main VAR Kp : LREAL := 1.0; SP : LREAL := 1.0; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; END_VAR Kp := 0.5; MV := Kp * (SP - PV); END_PROGRAM",
    );
    let result = isolated_run_to_end(&state, startup_candidate_request(&state));
    assert_eq!(result["status"], "failed", "{result}");
    assert!(
        result["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("ST changed controller parameter Main.Kp; candidate not adopted")),
        "{result}"
    );
}

/// A program of a periodic task first runs one interval after the start, so its startup is the second scan of the run, not the
/// first: the candidate it overwrites then is written again at the boundary after that scan, and holds from there (E8.3).
#[test]
fn isolated_run_adopts_a_candidate_that_a_periodic_task_program_sets_during_its_first_execution() {
    let (state, _root) = search_fixture_with(
        "PROGRAM Main VAR Kp : LREAL := 1.0; Started : BOOL := FALSE; SP : LREAL := 1.0; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; END_VAR IF NOT Started THEN Kp := 0.5; Started := TRUE; END_IF; MV := Kp * (SP - PV); END_PROGRAM CONFIGURATION Config RESOURCE Res ON PLC TASK Fast(INTERVAL := T#20ms, PRIORITY := 1); PROGRAM P1 WITH Fast : Main; END_RESOURCE END_CONFIGURATION",
    );
    let result = isolated_run_to_end(&state, startup_candidate_request_for(&state, "P1"));
    assert_eq!(result["status"], "completed", "{result}");
    let run = &result["run"];
    let samples = run["samples"].as_array().unwrap().len();
    for sample in 0..samples {
        assert_eq!(channel_value(run, sample, "P1.Kp"), json!(2.0), "sample {sample}");
    }
    let settled = last_channel_value(run, "P1.PV").as_f64().unwrap();
    assert!((settled - 2.0 / 3.0).abs() < 0.01, "{settled}");
}
