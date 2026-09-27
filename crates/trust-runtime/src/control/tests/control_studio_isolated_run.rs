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
            assert_eq!(run["samples"][0]["values"]["Main.H1"]["value"], 0.27);
            assert_eq!(run["samples"][0]["values"]["Main.H2"]["value"], 0.25);
            assert_ne!(
                run["samples"].as_array().unwrap().last().unwrap()["values"]["Main.H1"]["value"],
                0.27
            );
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
                    "sha256:{:x}",
                    sha2::Sha256::digest(selected_plant.as_bytes())
                ),
                "configuration identity must hash the exact UTF-8 TOML bytes"
            );
            assert_eq!(run["samples"][0]["values"]["Main.H1"]["value"], 0.61);
            assert_eq!(run["samples"][0]["values"]["Main.H2"]["value"], 0.52);
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
    assert_ne!(
        runs[0]["samples"].as_array().unwrap().last().unwrap()["values"]["Main.PV"]["value"],
        runs[1]["samples"].as_array().unwrap().last().unwrap()["values"]["Main.PV"]["value"]
    );
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
    assert_ne!(
        runs[0]["samples"].as_array().unwrap().last().unwrap()["values"]["Main.PV"]["value"],
        runs[1]["samples"].as_array().unwrap().last().unwrap()["values"]["Main.PV"]["value"]
    );
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
            assert_eq!(
                result["run"]["samples"][0]["values"]["Main.TrialKind"]["value"],
                1.0
            );
            assert_eq!(
                result["run"]["samples"][5]["values"]["Main.TrialKind"]["value"],
                2.0
            );
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
    assert_eq!(run["samples"][0]["values"]["Main.PV"]["quality"], "good");
    assert_eq!(run["samples"][1]["values"]["Main.PV"]["quality"], "good");
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
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["sequence"], index);
        assert_eq!(row["timeSeconds"], index as f64 / 1000.0);
        assert_eq!(row["values"]["Main.MV"]["value"], 1.0);
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
    for (index, row) in rows.iter().enumerate() {
        assert_eq!(row["sequence"], index);
        assert_eq!(row["timeSeconds"], index as f64 / 1000.0);
        assert_eq!(row["values"]["Main.MV"]["value"], 1.0);
    }
}

#[test]
fn isolated_run_checkpoint_budget_fails_with_its_received_prefix() {
    let (mut state, _root) = search_fixture();
    let names: Vec<_> = (0..16)
        .map(|index| format!("Signal{index}_{}", "x".repeat(190)))
        .collect();
    let declarations = names
        .iter()
        .map(|name| format!("{name} : LREAL := 1.0;"))
        .collect::<String>();
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1, path: PathBuf::from("Main.st"),
        text: format!("PROGRAM Main VAR PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; {declarations} END_VAR MV := 1.0; END_PROGRAM").into(),
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
        if result["status"] == "failed" {
            break;
        }
        assert_ne!(
            result["status"], "completed",
            "The test must exceed the checkpoint byte budget"
        );
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
    assert_eq!(
        details["reason"],
        "worker checkpoint output budget exhausted"
    );
    let run = &details["run"];
    assert_eq!(run["evidence"], "ST-executed");
    let count = run["samples"].as_array().unwrap().len();
    assert!(count > 0 && count < 12000);
    assert_eq!(
        count as u64,
        details["captured_sample_count"].as_u64().unwrap()
    );
    assert!(serde_json::to_vec(run).unwrap().len() < 15 * 1024 * 1024);
    assert_eq!(
        run["conditions"]["sampleTimesSeconds"]
            .as_array()
            .unwrap()
            .len(),
        12000
    );
}
