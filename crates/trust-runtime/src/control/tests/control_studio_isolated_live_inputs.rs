// An isolated run starts from the live plant (Specification 35, the isolated run): the copy's
// first scan sees every bound input as the running PLC had it, so a sensor block fed by an input
// the run's plant does not write reads a real signal instead of zero.

fn live_input(name: &str, address: &str, value: f64) -> crate::io::IoSnapshotEntry {
    crate::io::IoSnapshotEntry {
        name: Some(SmolStr::new(name)),
        address: crate::io::IoAddress::parse(address).expect("an input address"),
        value_type: Some(trust_hir::TypeId::LREAL),
        value: crate::io::IoSnapshotValue::Value(Value::LReal(value)),
        source: None,
    }
}

#[test]
fn an_isolated_run_starts_from_the_live_process_inputs() {
    let (mut state, root) = search_fixture();
    let source = "FUNCTION_BLOCK Scale VAR_INPUT Raw : LREAL; END_VAR VAR_OUTPUT Value : LREAL; END_VAR Value := Raw * 2.0; END_FUNCTION_BLOCK PROGRAM Main VAR Level AT %IL0 : LREAL; Signal AT %IL16 : LREAL; Pump AT %QL0 : LREAL; Sensor : Scale; Measured : LREAL; END_VAR Sensor(Raw := Signal); Measured := Sensor.Value; END_PROGRAM";
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("Main.st"),
        text: source.into(),
    }]);
    std::fs::write(
        root.path().join("simulation.toml"),
        "[simulation]\nenabled=true\nplant_step_ms=100\n[[plants]]\nid=\"level\"\nmodel=\"first_order\"\ncommand_sources=[\"%QL0\"]\nfeedback_targets=[\"%IL0\"]\ninitial_state=[0.5]\n[plants.parameters]\ngain=1.0\ntime_constant_s=1.0\n",
    )
    .unwrap();
    *state.io_snapshot.lock().unwrap() = Some(crate::io::IoSnapshot {
        scan: Some(11),
        inputs: vec![
            live_input("Level", "%IL0", 0.5),
            live_input("Signal", "%IL16", 4.5),
        ],
        ..crate::io::IoSnapshot::default()
    });
    let readiness = handle_request_value(
        json!({"id":8820,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    );
    assert!(readiness.ok, "{:?}", readiness.error);
    let revision = readiness.result.unwrap()["project_revision"].clone();
    let start = handle_request_value(
        json!({"id":8821,"type":"control.simulation.run.start","params":{"expected_project_revision":revision,"execution":"isolated-simulation","step_ms":100,"duration_ms":500,"seed":7,"max_total_cycles":6,"wall_budget_ms":5000,"initial_values":{},"events":[],"observations":[{"symbol":"Main.Measured","unit":"V"}],"parameter_overrides":[]}}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let response = handle_request_value(
            json!({"id":8822,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),
            &state,
            None,
        );
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.unwrap();
        if result["status"] == "completed" {
            assert_eq!(
                channel_value(&result["run"], 0, "Main.Measured"),
                9.0,
                "the first scan reads the live input through the block"
            );
            assert_eq!(
                last_channel_value(&result["run"], "Main.Measured"),
                9.0,
                "an input the run's plant does not write keeps its live value"
            );
            break;
        }
        assert_ne!(result["status"], "failed", "{result}");
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
