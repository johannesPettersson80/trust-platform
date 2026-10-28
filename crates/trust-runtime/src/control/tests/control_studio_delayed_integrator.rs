#[test]
fn isolated_verify_executes_st_controller_against_selected_delayed_integrator() {
    let (mut state, _root) = search_fixture();
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("Main.st"),
        text: "PROGRAM Main VAR PV AT %IL0 : LREAL; MV AT %QL0 : LREAL; SP : LREAL := 1.0; Kp : LREAL := 1.0; END_VAR MV := Kp * (SP - PV); END_PROGRAM".into(),
    }]);
    let ready = handle_request_value(
        json!({"id":8900,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    );
    assert!(ready.ok, "{:?}", ready.error);
    let revision = ready.result.unwrap()["project_revision"].clone();
    let plant = r#"[simulation]
enabled = true
plant_step_ms = 100
[[plants]]
id = "selected-integrator"
model = "integrator_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
initial_input = [0.0]
[plants.parameters]
gain = 1.0
dead_time_s = 0.15
initial_delayed_input = 0.0
"#;
    let mut runs = Vec::new();
    for gain in [0.5, 1.0] {
        let request = json!({
            "expected_project_revision":revision,"execution":"isolated-simulation",
            "step_ms":100,"duration_ms":2000,"seed":7,"max_total_cycles":21,"wall_budget_ms":5000,
            "initial_values":{"Main.SP":1.0,"Main.MV":0.0},"events":[],
            "observations":[{"symbol":"Main.PV","unit":"m"},{"symbol":"Main.MV","unit":"V"}],
            "parameter_overrides":[{"symbol":"Main.Kp","iec_type":"LREAL","unit":"V/m",
                "convention":"proportional gain","minimum":0.0,"maximum":2.0,"value":gain}],
            "simulation_override":{"model_revision":"selected-integrator-model",
                "model_provenance":"declared-provided-validated","simulation_toml":plant}
        });
        let started = handle_request_value(
            json!({"id":8901,"type":"control.simulation.run.start","params":request}),
            &state,
            None,
        );
        assert!(started.ok, "{:?}", started.error);
        let job = started.result.unwrap()["job_id"].clone();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            let reply = handle_request_value(
                json!({"id":8902,"type":"control.simulation.run.status","params":{
                    "job_id":job,"expected_project_revision":revision}}),
                &state,
                None,
            );
            assert!(reply.ok, "{:?}", reply.error);
            let result = reply.result.unwrap();
            if result["status"] == "completed" {
                let run = result["run"].clone();
                assert_eq!(run["evidence"], "ST-executed");
                assert_eq!(
                    run["conditions"]["modelRevision"],
                    "selected-integrator-model"
                );
                assert_eq!(
                    run["conditions"]["modelProvenance"],
                    "declared-provided-validated"
                );
                assert_eq!(run["parameters"]["Main.Kp"], gain);
                let samples = run["samples"].as_array().unwrap();
                assert_eq!(samples.len(), 21);
                assert_eq!(channel_value(&run, 0, "Main.PV"), 0.0);
                assert_eq!(channel_value(&run, 1, "Main.PV"), 0.0);
                assert!(channel_value(&run, 3, "Main.PV").as_f64().unwrap() > 0.0);
                runs.push(run);
                break;
            }
            assert_ne!(result["status"], "failed", "{result}");
            assert!(std::time::Instant::now() < deadline, "{result}");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    assert_eq!(
        runs[0]["conditions"], runs[1]["conditions"],
        "both gains must use identical verification conditions"
    );
    let final_value =
        |run: &serde_json::Value| last_channel_value(run, "Main.PV").as_f64().unwrap();
    assert!(final_value(&runs[1]) > final_value(&runs[0]));
    assert!(final_value(&runs[1]) < 1.1);
}
