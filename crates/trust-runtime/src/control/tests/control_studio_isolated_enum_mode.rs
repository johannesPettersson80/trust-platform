// Protocol annex E8.3 (specification 35 A6.1): a test run writes and records a loop's enumeration mode as Control Studio reads a
// mode, 1 in Manual and 0 in Auto, through the run's `mode_bindings`.

/// Catches an enumeration mode written by its raw number (0 is `Off`, the type's first member, not Auto), an event that cannot
/// switch it to Manual, and a recorded mode channel that holds the member's number instead of the reading of 1 or 0.
#[test]
fn isolated_run_writes_and_records_an_enumeration_mode_by_its_manual_and_auto_members() {
    let (mut state, root) = search_fixture();
    // In Manual the program outputs 1, in Auto 2, and in Off 0.
    let source = "TYPE E_Mode : (Off, Manual, Auto); END_TYPE PROGRAM Main VAR H1 AT %IL0 : LREAL; H2 AT %IL8 : LREAL; Pump AT %QL0 : LREAL := 0.010; Mode : E_Mode := E_Mode#Off; Out : LREAL; END_VAR IF Mode = E_Mode#Manual THEN Out := 1.0; ELSIF Mode = E_Mode#Auto THEN Out := 2.0; ELSE Out := 0.0; END_IF; END_PROGRAM";
    state.sources = SourceRegistry::new(vec![SourceFile {
        id: 1,
        path: PathBuf::from("Main.st"),
        text: source.into(),
    }]);
    std::fs::write(root.path().join("simulation.toml"),"[simulation]\nenabled=true\nplant_step_ms=100\n[[plants]]\nid=\"tanks\"\nmodel=\"gravity_tank_cascade\"\ncommand_sources=[\"%QL0\"]\nfeedback_targets=[\"%IL0\",\"%IL8\"]\ninitial_state=[0.27,0.25]\ninitial_input=[0.010]\n[plants.parameters]\narea_m2=0.5\noutflow_coefficient_m2_sqrt_m_per_s=0.020\n").unwrap();
    let readiness = handle_request_value(
        json!({"id":8900,"type":"control.simulation.run.status","params":{}}),
        &state,
        None,
    );
    assert!(readiness.ok, "{:?}", readiness.error);
    let revision = readiness.result.unwrap()["project_revision"].clone();
    std::thread::sleep(std::time::Duration::from_millis(40));
    // The run starts in Auto (0) and switches to Manual (1) at 500 ms.
    let request = json!({"expected_project_revision":revision,"execution":"isolated-simulation","step_ms":100,"duration_ms":1000,
        "seed":7,"max_total_cycles":11,"wall_budget_ms":5000,"initial_values":{"Main.Mode":0},
        "events":[{"at_ms":500,"symbol":"Main.Mode","value":1}],
        "observations":[{"symbol":"Main.Mode","unit":"1"},{"symbol":"Main.Out","unit":"1"}],"parameter_overrides":[],
        "mode_bindings":[{"symbol":"Main.Mode","manual_value":"Manual","auto_value":"Auto"}]});
    let start = handle_request_value(
        json!({"id":8901,"type":"control.simulation.run.start","params":request}),
        &state,
        None,
    );
    assert!(start.ok, "{:?}", start.error);
    let job = start.result.unwrap()["job_id"].clone();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        let response = handle_request_value(
            json!({"id":8902,"type":"control.simulation.run.status","params":{"job_id":job,"expected_project_revision":revision}}),
            &state,
            None,
        );
        assert!(response.ok, "{:?}", response.error);
        let result = response.result.unwrap();
        if result["status"] == "completed" {
            let run = &result["run"];
            // Worked by hand: at 300 ms the run is in Auto (reading 0, output 2); at its end, after the event, in Manual (1, 1).
            assert_eq!(channel_value(run, 3, "Main.Mode"), 0.0, "{run}");
            assert_eq!(channel_value(run, 3, "Main.Out"), 2.0, "{run}");
            assert_eq!(last_channel_value(run, "Main.Mode"), 1.0, "{run}");
            assert_eq!(last_channel_value(run, "Main.Out"), 1.0, "{run}");
            break;
        }
        assert_ne!(result["status"], "failed", "{result}");
        assert!(std::time::Instant::now() < deadline, "{result}");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}
