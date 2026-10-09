const ADS_GENERATED_ST: &str =
    include_str!("../../../../../examples/communication/ads_line1/src/generated/ads_generated.st");
const ADS_MAIN_ST: &str =
    include_str!("../../../../../examples/communication/ads_line1/src/main.st");
const ADS_CONFIG_ST: &str =
    include_str!("../../../../../examples/communication/ads_line1/src/config.st");

#[test]
fn generated_ads_globals_are_first_class_product_surface_variables() {
    let source = ads_example_source();
    let state = control_test_state(source.as_str());

    let eval = handle_request_value(
        json!({
            "id": 711,
            "type": "eval",
            "params": { "expr": "line1_temp" }
        }),
        &state,
        None,
    );
    assert!(eval.ok, "control eval should succeed: {:?}", eval.error);
    assert!(eval
        .result
        .as_ref()
        .and_then(|value| value.get("value"))
        .and_then(serde_json::Value::as_str)
        .is_some_and(|value| value.contains("Real")));

    let globals = debug_global_variables(&state);
    assert!(
        globals.iter().any(|entry| {
            entry.get("name").and_then(serde_json::Value::as_str) == Some("line1_temp")
                && entry.get("type").and_then(serde_json::Value::as_str) == Some("REAL")
        }),
        "line1_temp should appear in debug Globals"
    );
    assert!(
        globals.iter().any(|entry| {
            entry.get("name").and_then(serde_json::Value::as_str) == Some("line1_temp_quality")
                && entry.get("value").and_then(serde_json::Value::as_str)
                    == Some("ADS_QUALITY::Stale")
        }),
        "line1_temp_quality should appear in debug Globals"
    );

    let snapshot = state.debug.snapshot().expect("debug snapshot");
    let history_path = temp_history_path("ads-product-surfaces");
    let historian = HistorianService::new(
        HistorianConfig {
            enabled: true,
            sample_interval_ms: 1,
            mode: RecordingMode::Allowlist,
            include: vec![SmolStr::new("line1_*")],
            history_path: history_path.clone(),
            max_entries: 128,
            prometheus_enabled: false,
            prometheus_path: SmolStr::new("/metrics"),
            alerts: Vec::new(),
        },
        None,
    )
    .expect("historian");
    let recorded = historian
        .capture_snapshot_at(&snapshot, 1_000)
        .expect("historian capture");
    assert!(
        recorded >= 2,
        "historian should record ADS value and quality globals"
    );
    assert!(historian
        .query(Some("line1_temp"), None, 10)
        .iter()
        .any(|sample| {
            matches!(
                &sample.value,
                crate::historian::HistorianValue::Float(value) if *value == 0.0
            )
        }));
    assert!(
        historian
            .query(Some("line1_temp_quality"), None, 10)
            .iter()
            .any(|sample| matches!(
                &sample.value,
                crate::historian::HistorianValue::String(value) if value == "Stale"
            )),
        "historian should record ADS_QUALITY enum variant"
    );
    let _ = std::fs::remove_file(history_path);

    let opc_temp = crate::opcua::map_iec_value(
        snapshot
            .storage
            .get_global("line1_temp")
            .expect("line1_temp global"),
    )
    .expect("line1_temp maps to OPC UA");
    assert_eq!(opc_temp.data_type, crate::opcua::OpcUaDataType::Float);
    let opc_quality = crate::opcua::map_iec_value(
        snapshot
            .storage
            .get_global("line1_temp_quality")
            .expect("line1_temp_quality global"),
    )
    .expect("line1_temp_quality maps to OPC UA");
    assert_eq!(opc_quality.data_type, crate::opcua::OpcUaDataType::String);
    assert_eq!(
        opc_quality.value,
        crate::opcua::OpcUaVariant::String("Stale".to_string())
    );
}

fn ads_example_source() -> String {
    format!("{ADS_GENERATED_ST}\n{ADS_MAIN_ST}\n{ADS_CONFIG_ST}")
}

fn debug_global_variables(state: &ControlState) -> Vec<serde_json::Value> {
    let globals_ref = state
        .debug_variables
        .lock()
        .expect("debug variable handles")
        .alloc(crate::debug::dap::VariableHandle::Globals);
    let variables = handle_request_value(
        json!({
            "id": 713,
            "type": "debug.variables",
            "params": { "variables_reference": globals_ref }
        }),
        state,
        None,
    );
    assert!(
        variables.ok,
        "debug.variables should succeed: {:?}",
        variables.error
    );
    variables
        .result
        .as_ref()
        .and_then(|value| value.get("variables"))
        .and_then(serde_json::Value::as_array)
        .cloned()
        .expect("debug variables")
}
