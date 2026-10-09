#[test]
fn finite_search_status_reports_explicit_missing_project_readiness() {
    let state = control_test_state("PROGRAM Main END_PROGRAM");
    let response = handle_request_value(
        json!({"id":8600,"type":"control.search.status","params":{}}),
        &state,
        None,
    );
    assert!(
        response.ok,
        "search status must be a supported operation: {:?}",
        response.error
    );
    let result = response.result.unwrap();
    assert_eq!(result["ready"], false);
    assert!(result["reason"].as_str().unwrap().contains("project"));
}

static SEARCH_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
struct SearchTestRoot {
    path: PathBuf,
    _serial: std::sync::MutexGuard<'static, ()>,
}
impl SearchTestRoot {
    fn path(&self) -> &std::path::Path {
        &self.path
    }
}
impl Drop for SearchTestRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}
fn search_fixture() -> (ControlState, SearchTestRoot) {
    let serial = SEARCH_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut state=control_test_state("PROGRAM Main VAR Kp : LREAL := 1.0; SP : LREAL := 1.0; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; END_VAR MV := Kp * (SP - PV); END_PROGRAM");
    let root = SearchTestRoot {
        path: temp_dir(&format!("finite-search-{}", std::process::id())),
        _serial: serial,
    };
    std::fs::write(
        root.path().join("simulation.toml"),
        r#"[simulation]
enabled = true
plant_step_ms = 20
[[plants]]
id = "engineer-plant"
model = "first_order"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = 1.0
time_constant_s = 0.1
"#,
    )
    .unwrap();
    let configuration =
        crate::simulation::SimulationConfig::load(root.path().join("simulation.toml")).unwrap();
    let simulator = crate::simulation::SimulationController::new(configuration);
    let native = simulator.runtime_snapshot_for(&crate::Runtime::new());
    let (resource, commands) = ResourceControl::stub(StdClock::new());
    std::thread::spawn(move || {
        while let Ok(command) = commands.recv() {
            if let ResourceCommand::SimulationSnapshot { respond_to } = command {
                let _ = respond_to.send(Some(native.clone()));
            }
        }
    });
    state.resource = resource;
    state.project_root = Some(root.path().into());
    (state, root)
}
fn search_request(revision: serde_json::Value) -> serde_json::Value {
    json!({"expected_project_revision":revision,"execution":"isolated-simulation",
        "grid":[{"symbol":"Main.Kp","unit":"ratio","minimum":0,"maximum":20,"values":[1,2,20]}],"trial_count":3,
        "step_ms":20,"duration_ms":1000,"seed":7,"max_total_cycles":153,"wall_budget_ms":20000,"per_trial_wall_ms":5000,
        "initial_values":{},"events":[],"observations":[{"symbol":"Main.SP","unit":"C"},{"symbol":"Main.PV","unit":"C"},{"symbol":"Main.MV","unit":"points"}],
        "requirements":[{"id":"actuator-bound","symbol":"Main.MV","minimum":-10,"maximum":10,"start_ms":0,"end_ms":1000}],
        "objective":{"reference_symbol":"Main.SP","feedback_symbol":"Main.PV","error_unit":"C"}})
}
#[test]
fn finite_search_executes_actual_st_with_resets_and_rejects_constraints_before_ranking() {
    let (state, _root) = search_fixture();
    let status = handle_request_value(
        json!({"id":8601,"type":"control.search.status","params":{}}),
        &state,
        None,
    );
    assert!(status.ok, "{:?}", status.error);
    let revision = status.result.unwrap()["project_revision"].clone();
    let start = handle_request_value(
        json!({"id":8602,"type":"control.search.start","params":search_request(revision.clone())}),
        &state,
        None,
    );
    assert!(
        start.ok,
        "search start must execute actual ST: {:?}",
        start.error
    );
    let job = start.result.unwrap()["job_id"].clone();
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        let status = handle_request_value(
            json!({"id":8603,"type":"control.search.status","params":{"job_id":job,"expected_project_revision":revision}}),
            &state,
            None,
        );
        assert!(status.ok, "{:?}", status.error);
        let result = status.result.unwrap();
        if result["status"] == "completed" {
            assert_eq!(result["trials"].as_array().unwrap().len(), 3);
            assert_eq!(result["ranking"][0], 1);
            assert_eq!(
                result["trials"][0]["worker_memory_limit_bytes"], 1_073_741_824_u64,
                "actual OS memory limit must be reported"
            );
            assert_eq!(result["trials"][2]["status"], "rejected");
            assert_eq!(result["trials"][0]["run"]["evidence"], "ST-executed");
            assert_eq!(
                result["trials"][0]["run"]["conditions"],
                result["trials"][1]["run"]["conditions"]
            );
            assert_eq!(channel_value(&result["trials"][0]["run"], 0, "Main.PV"), 0.0);
            assert_eq!(channel_value(&result["trials"][1]["run"], 0, "Main.PV"), 0.0);
            assert!(
                state.debug.drain_var_writes().is_empty(),
                "search must never mutate selected runtime"
            );
            break;
        }
        assert!(result["status"] != "failed", "{result}");
        assert!(std::time::Instant::now() < limit, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[test]
fn finite_search_rejects_grid_count_physical_execution_and_event_parameter_conflicts() {
    let (state, _root) = search_fixture();
    let status = handle_request_value(
        json!({"id":8610,"type":"control.search.status","params":{}}),
        &state,
        None,
    );
    let revision = status.result.unwrap()["project_revision"].clone();
    for (key, value) in [
        ("trial_count", json!(2)),
        ("execution", json!("physical")),
        ("initial_values", json!({"Main.Kp":4})),
        ("events", json!([{"at_ms":20,"symbol":"Main.Kp","value":5}])),
    ] {
        let mut request = search_request(revision.clone());
        request[key] = value;
        let result = handle_request_value(
            json!({"id":8611,"type":"control.search.start","params":request}),
            &state,
            None,
        );
        assert!(!result.ok, "{key} must be rejected");
        assert_eq!(result.error_code.as_deref(), Some("invalid_search_request"));
    }
}
#[test]
fn finite_search_cancel_is_scoped_and_terminates_worker() {
    let (state, _root) = search_fixture();
    let revision = handle_request_value(
        json!({"id":8620,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let start = handle_request_value(
        json!({"id":8621,"type":"control.search.start","params":search_request(revision.clone())}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let bad = handle_request_value(
        json!({"id":8622,"type":"control.search.cancel","params":{"job_id":job,"expected_project_revision":"other"}}),
        &state,
        None,
    );
    assert!(!bad.ok);
    let cancel = handle_request_value(
        json!({"id":8623,"type":"control.search.cancel","params":{"job_id":job,"expected_project_revision":revision}}),
        &state,
        None,
    );
    assert!(cancel.ok, "{:?}", cancel.error);
    let limit = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let result=handle_request_value(json!({"id":8624,"type":"control.search.status","params":{"job_id":job,"expected_project_revision":revision}}),&state,None).result.unwrap();
        if result["status"] == "cancelled" {
            assert_eq!(result["ranking"], json!([]));
            break;
        }
        assert!(std::time::Instant::now() < limit, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn finite_search_runaway_st_is_bounded_and_never_qualifies() {
    let (mut state, _root) = search_fixture();
    state.sources=SourceRegistry::new(vec![SourceFile{id:1,path:PathBuf::from("Main.st"),text:"PROGRAM Main VAR Kp : LREAL := 1.0; SP : LREAL := 1.0; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; END_VAR WHILE TRUE DO MV := Kp; END_WHILE; END_PROGRAM".into()}]);
    let revision = handle_request_value(
        json!({"id":8630,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = search_request(revision.clone());
    request["grid"][0]["values"] = json!([1]);
    request["trial_count"] = json!(1);
    request["wall_budget_ms"] = json!(2000);
    request["per_trial_wall_ms"] = json!(1000);
    let start = handle_request_value(
        json!({"id":8631,"type":"control.search.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let status=handle_request_value(json!({"id":8632,"type":"control.search.status","params":{"job_id":job,"expected_project_revision":revision}}),&state,None).result.unwrap();
        if status["status"] == "completed" {
            assert_eq!(status["ranking"], json!([]));
            assert_eq!(status["trials"][0]["status"], "failed");
            assert!(
                status["trials"][0]["reason"]
                    .as_str()
                    .unwrap()
                    .to_ascii_lowercase()
                    .contains("timed out"),
                "{status}"
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{status}");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

#[test]
fn finite_search_caps_total_retained_results_without_silent_truncation() {
    let (mut state, _root) = search_fixture();
    // Three channels of constant 24-byte numbers make each sample about 126 bytes, so a trial of
    // 100,000 samples is about 13.3 MB. Five trials hold 1,500,000 channel samples, within the limit
    // of 1,600,000, and retain about 66 MB, more than the derived 60.3 MB budget (MAX_OUTPUT less
    // 64 KiB). The first four trials are retained whole; the fifth is refused explicitly.
    state.sources=SourceRegistry::new(vec![SourceFile{id:1,path:PathBuf::from("Main.st"),text:"PROGRAM Main VAR Kp : LREAL := 1.0; SP : LREAL := -2.2250738585072014E-308; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; E0 : LREAL := -2.2250738585072014E-308; E1 : LREAL := -2.2250738585072014E-308; END_VAR MV := Kp * (SP-PV); END_PROGRAM".into()}]);
    let revision = handle_request_value(
        json!({"id":8640,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = search_request(revision.clone());
    request["grid"][0]["values"] = json!([1, 2, 3, 4, 5]);
    request["trial_count"] = json!(5);
    request["step_ms"] = json!(1);
    request["duration_ms"] = json!(99_999);
    request["max_total_cycles"] = json!(500_000);
    request["wall_budget_ms"] = json!(300_000);
    request["per_trial_wall_ms"] = json!(60_000);
    request["requirements"] = json!([]);
    request["objective"] = json!({"reference_symbol":"Main.SP","feedback_symbol":"Main.E0","error_unit":"C"});
    request["observations"] = json!([
        {"symbol":"Main.SP","unit":"C"},
        {"symbol":"Main.E0","unit":"C"},
        {"symbol":"Main.E1","unit":"C"}
    ]);
    let start = handle_request_value(
        json!({"id":8641,"type":"control.search.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    // Five trials of 100,000 scans execute before the budget is exceeded: allow for a debug build.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    loop {
        let result=handle_request_value(json!({"id":8642,"type":"control.search.status","params":{"job_id":job,"expected_project_revision":revision}}),&state,None).result.unwrap();
        if !matches!(result["status"].as_str(), Some("queued" | "running")) {
            assert_eq!(
                result["status"], "failed",
                "aggregate output over budget must fail explicitly"
            );
            assert!(result["reason"]
                .as_str()
                .unwrap()
                .contains("retained output"));
            assert_eq!(result["ranking"], json!([]));
            assert!(serde_json::to_vec(&result).unwrap().len() <= 64 * 1024 * 1024);
            let trials = result["trials"].as_array().unwrap();
            let refused = trials.last().expect("the attempted oversized trial");
            assert_eq!(
                refused["status"], "failed",
                "attempted oversized trial must have an explicit failure record"
            );
            assert!(refused["reason"]
                .as_str()
                .unwrap()
                .contains("trial evidence omitted"));
            for retained in &trials[..trials.len() - 1] {
                assert_eq!(
                    retained["run"]["samples"].as_array().unwrap().len(),
                    100_000,
                    "retained trials are complete, never truncated"
                );
            }
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "retained-output test timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(250));
    }
}

#[test]
fn finite_search_thin_progress_omits_runs_and_terminal_fetch_returns_them() {
    let (state, _root) = search_fixture();
    let revision = handle_request_value(
        json!({"id":8650,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let start = handle_request_value(
        json!({"id":8651,"type":"control.search.start","params":search_request(revision.clone())}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(25);
    loop {
        let response = handle_request_value(
            json!({"id":8652,"type":"control.search.status","params":{"job_id":job,"expected_project_revision":revision,"include_results":false}}),
            &state,
            None,
        );
        assert!(
            response.ok,
            "thin progress must be supported: {:?}",
            response.error
        );
        let result = response.result.unwrap();
        assert!(result["trials"]
            .as_array()
            .unwrap()
            .iter()
            .all(|trial| trial.get("run").is_none()));
        if result["status"] == "completed" {
            break;
        }
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let final_result = handle_request_value(
        json!({"id":8653,"type":"control.search.status","params":{"job_id":job,"expected_project_revision":revision,"include_results":true}}),
        &state,
        None,
    );
    assert!(final_result.ok, "{:?}", final_result.error);
    assert_eq!(
        final_result.result.unwrap()["trials"][0]["run"]["evidence"],
        "ST-executed"
    );
}

#[test]
fn finite_search_requires_active_native_simulation_not_a_configuration_file() {
    let (mut state, _root) = search_fixture();
    state.resource = control_test_state("PROGRAM Main END_PROGRAM").resource;
    let response = handle_request_value(
        json!({"id":8660,"type":"control.search.status","params":{}}),
        &state,
        None,
    );
    assert!(response.ok);
    assert_eq!(
        response.result.unwrap()["ready"],
        false,
        "simulation.toml does not establish a trusted native simulation host"
    );
}

#[test]
fn finite_search_rejects_active_simulation_with_a_physical_io_driver() {
    let (mut state, root) = search_fixture();
    let mut runtime = crate::Runtime::new();
    runtime.add_io_driver("unqualified-physical", Box::new(GenericPhysicalDriver));
    let simulator = crate::simulation::SimulationController::new(
        crate::simulation::SimulationConfig::load(root.path().join("simulation.toml")).unwrap(),
    );
    let native = simulator.runtime_snapshot_for(&runtime);
    assert!(native.status.active);
    assert_eq!(native.simulation_only_io, Some(false));
    let (resource, commands) = ResourceControl::stub(StdClock::new());
    std::thread::spawn(move || {
        while let Ok(command) = commands.recv() {
            if let ResourceCommand::SimulationSnapshot { respond_to } = command {
                let _ = respond_to.send(Some(native.clone()));
            }
        }
    });
    state.resource = resource;
    let response = handle_request_value(
        json!({"id":8661,"type":"control.search.status","params":{}}),
        &state,
        None,
    );
    let result = response.result.unwrap();
    assert_eq!(result["ready"], false);
    assert!(result["reason"].as_str().unwrap().contains("physical"));
}

#[test]
fn finite_search_observes_integer_process_signals_without_cast_wrappers() {
    let (mut state, _root) = search_fixture();
    state.sources=SourceRegistry::new(vec![SourceFile{id:1,path:PathBuf::from("Main.st"),text:"PROGRAM Main VAR Kp : LREAL := 1.0; SP : LREAL := 1.0; PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; Counter : DINT; END_VAR Counter := Counter + 1; MV := Kp * (SP - PV); END_PROGRAM".into()}]);
    let revision = handle_request_value(
        json!({"id":8670,"type":"control.search.status","params":{}}),
        &state,
        None,
    )
    .result
    .unwrap()["project_revision"]
        .clone();
    let mut request = search_request(revision.clone());
    request["grid"][0]["values"] = json!([1]);
    request["trial_count"] = json!(1);
    request["observations"]
        .as_array_mut()
        .unwrap()
        .push(json!({"symbol":"Main.Counter","unit":"count"}));
    let start = handle_request_value(
        json!({"id":8671,"type":"control.search.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let result=handle_request_value(json!({"id":8672,"type":"control.search.status","params":{"job_id":job,"expected_project_revision":revision}}),&state,None).result.unwrap();
        if result["status"] == "completed" {
            assert_eq!(
                result["trials"][0]["status"], "qualified",
                "integer observations must not inherit gain write types: {result}"
            );
            assert_eq!(
                channel_value(&result["trials"][0]["run"], 0, "Main.Counter"),
                1.0
            );
            break;
        }
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
