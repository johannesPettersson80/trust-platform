const ADS_GENERATED_ST: &str =
    include_str!("../../../../../examples/communication/ads_line1/src/generated/ads_generated.st");
const ADS_MAIN_ST: &str =
    include_str!("../../../../../examples/communication/ads_line1/src/main.st");
const ADS_CONFIG_ST: &str =
    include_str!("../../../../../examples/communication/ads_line1/src/config.st");

#[test]
fn generated_ads_globals_are_first_class_product_surface_variables() {
    let source = ads_example_source();
    let state = hmi_test_state(source.as_str());
    let temp_id = "resource/RESOURCE/global/line1_temp";
    let quality_id = "resource/RESOURCE/global/line1_temp_quality";

    let schema = hmi_schema_result(&state);
    let temp_widget = hmi_widget_by_path(&schema, "global.line1_temp");
    assert_eq!(temp_widget.get("id").and_then(serde_json::Value::as_str), Some(temp_id));
    assert_eq!(
        temp_widget
            .get("data_type")
            .and_then(serde_json::Value::as_str),
        Some("REAL")
    );
    assert_eq!(
        temp_widget
            .get("source")
            .and_then(serde_json::Value::as_str),
        Some("global")
    );

    let quality_widget = hmi_widget_by_path(&schema, "global.line1_temp_quality");
    assert_eq!(
        quality_widget
            .get("id")
            .and_then(serde_json::Value::as_str),
        Some(quality_id)
    );
    assert_eq!(
        quality_widget
            .get("data_type")
            .and_then(serde_json::Value::as_str),
        Some("ADS_QUALITY")
    );

    let hmi_values = handle_request_value(
        json!({
            "id": 710,
            "type": "hmi.values.get",
            "params": { "ids": [temp_id, quality_id] }
        }),
        &state,
        None,
    );
    assert!(
        hmi_values.ok,
        "hmi.values.get should succeed: {:?}",
        hmi_values.error
    );
    let values = hmi_values
        .result
        .as_ref()
        .and_then(|value| value.get("values"))
        .and_then(serde_json::Value::as_object)
        .expect("values object");
    assert_eq!(
        values
            .get(temp_id)
            .and_then(|record| record.get("q"))
            .and_then(serde_json::Value::as_str),
        Some("good")
    );
    assert_eq!(
        values
            .get(quality_id)
            .and_then(|record| record.get("v"))
            .and_then(|value| value.get("variant"))
            .and_then(serde_json::Value::as_str),
        Some("Stale")
    );

    let snapshot = state.debug.snapshot().expect("debug snapshot");
    let metadata = state.metadata.lock().expect("metadata").clone();
    let generated_path = Path::new("src/generated/ads_generated.st");
    let main_path = Path::new("src/main.st");
    let config_path = Path::new("src/config.st");
    let sources = [
        crate::hmi::HmiSourceRef {
            path: generated_path,
            text: ADS_GENERATED_ST,
        },
        crate::hmi::HmiSourceRef {
            path: main_path,
            text: ADS_MAIN_ST,
        },
        crate::hmi::HmiSourceRef {
            path: config_path,
            text: ADS_CONFIG_ST,
        },
    ];
    let catalog = crate::hmi::collect_hmi_bindings_catalog(&metadata, Some(&snapshot), &sources);
    assert!(
        catalog
            .globals
            .iter()
            .any(|entry| entry.name == "line1_temp" && entry.path == "global.line1_temp"),
        "line1_temp should appear in HMI bindings catalog"
    );
    assert!(
        catalog.globals.iter().any(|entry| {
            entry.name == "line1_temp_quality"
                && entry.path == "global.line1_temp_quality"
                && entry.data_type == "ADS_QUALITY"
        }),
        "line1_temp_quality should appear in HMI bindings catalog"
    );

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
                && entry
                    .get("type")
                    .and_then(serde_json::Value::as_str)
                    == Some("REAL")
        }),
        "line1_temp should appear in debug Globals"
    );
    assert!(
        globals.iter().any(|entry| {
            entry.get("name").and_then(serde_json::Value::as_str)
                == Some("line1_temp_quality")
                && entry
                    .get("value")
                    .and_then(serde_json::Value::as_str)
                    == Some("ADS_QUALITY::Stale")
        }),
        "line1_temp_quality should appear in debug Globals"
    );

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
    assert!(recorded >= 2, "historian should record ADS value and quality globals");
    assert!(historian.query(Some("line1_temp"), None, 10).iter().any(|sample| {
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

fn hmi_widget_by_path<'a>(schema: &'a serde_json::Value, path: &str) -> &'a serde_json::Value {
    schema
        .get("widgets")
        .and_then(serde_json::Value::as_array)
        .and_then(|widgets| {
            widgets.iter().find(|widget| {
                widget.get("path").and_then(serde_json::Value::as_str) == Some(path)
            })
        })
        .unwrap_or_else(|| panic!("missing HMI widget for {path}"))
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
