use super::super::*;

#[test]
fn unified_shell_settings_module_exposes_realtime_link_configuration_fields() {
    let project = make_project("settings-realtime-fields");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let mut response = ureq::get(&format!("{base}/ide/modules/ide-settings.js"))
        .call()
        .expect("fetch ide-settings.js");
    let body = response
        .body_mut()
        .read_to_string()
        .expect("read ide-settings.js");

    assert!(
        body.contains("runtime_cloud.links.transports_json"),
        "settings module must expose runtime-cloud link transport JSON field"
    );
    assert!(
        body.contains("const SETTINGS_RUNTIME_LINK_TRANSPORTS"),
        "settings module must declare explicit runtime-cloud link transport allowlist"
    );
    for transport in [
        "realtime",
        "zenoh",
        "mesh",
        "mqtt",
        "modbus-tcp",
        "opcua",
        "discovery",
        "web",
    ] {
        assert!(
            body.contains(&format!("\"{transport}\"")),
            "settings module must include transport '{transport}' in allowlist/config text"
        );
    }
    assert!(
        body.contains("runtime_cloud.wan.allow_write_json"),
        "settings module must expose runtime-cloud WAN allow-write JSON field"
    );
    assert!(
        body.contains("function settingsTomlEncodeBindingValue"),
        "settings module must encode complex TOML binding values"
    );
    assert!(
        body.contains("function settingsTomlDecodeBindingRaw"),
        "settings module must decode complex TOML binding values"
    );
    assert!(
        body.contains("const SETTINGS_ONLINE_KEY_MAP"),
        "settings module must declare online key mapping for config.set"
    );
    assert!(
        body.contains("\"mesh.connect_json\": \"mesh.connect\""),
        "settings module must map mesh.connect_json to mesh.connect for online writes"
    );
    assert!(
        body.contains("\"mesh.subscribe_json\": \"mesh.subscribe\""),
        "settings module must map mesh.subscribe_json to mesh.subscribe for online writes"
    );
    assert!(
        body.contains("\"runtime_cloud.wan.allow_write_json\": \"runtime_cloud.wan.allow_write\""),
        "settings module must map runtime_cloud.wan.allow_write_json to backend key for online writes"
    );
    assert!(
        body.contains(
            "\"runtime_cloud.links.transports_json\": \"runtime_cloud.links.transports\""
        ),
        "settings module must map runtime_cloud.links.transports_json to backend key for online writes"
    );
    assert!(
        body.contains("\"discovery.interfaces_json\""),
        "settings module must include discovery.interfaces_json in online-capable keys"
    );
    assert!(
        body.contains("\"runtime_cloud.links.transports_json\""),
        "settings module must include runtime_cloud.links.transports_json in online-capable keys"
    );
    assert!(
        body.contains("\"opcua.username\""),
        "settings module must expose OPC UA username setting"
    );
    assert!(
        body.contains("\"opcua.password\""),
        "settings module must expose OPC UA password setting"
    );
    assert!(
        body.contains("\"observability.include_json\""),
        "settings module must expose observability include patterns JSON setting"
    );
    assert!(
        body.contains("\"observability.alerts_json\""),
        "settings module must expose observability alert rules JSON setting"
    );
    assert!(
        body.contains("\"io.mqtt.client_id\""),
        "settings module must expose MQTT client_id setting"
    );
    assert!(
        body.contains("\"io.mqtt.username\""),
        "settings module must expose MQTT username setting"
    );
    assert!(
        body.contains("\"io.mqtt.password\""),
        "settings module must expose MQTT password setting"
    );
    assert!(
        body.contains("\"io.mqtt.tls\""),
        "settings module must expose MQTT tls setting"
    );
    assert!(
        body.contains("\"io.gpio.backend\""),
        "settings module must expose GPIO backend setting"
    );
    assert!(
        body.contains("\"io.gpio.inputs_json\""),
        "settings module must expose GPIO inputs JSON setting"
    );
    assert!(
        body.contains("\"io.gpio.outputs_json\""),
        "settings module must expose GPIO outputs JSON setting"
    );
    assert!(
        body.contains("\"io.ethercat.adapter\""),
        "settings module must expose EtherCAT adapter setting"
    );
    assert!(
        body.contains("\"io.ethercat.modules_json\""),
        "settings module must expose EtherCAT modules JSON setting"
    );
    assert!(
        body.contains("\"io.ethercat.mock_fail_read\""),
        "settings module must expose EtherCAT mock_fail_read setting"
    );
    assert!(
        body.contains("\"io.ethercat.mock_fail_write\""),
        "settings module must expose EtherCAT mock_fail_write setting"
    );
    assert!(
        body.contains("\"io.simulated.inputs\""),
        "settings module must expose simulated input count setting"
    );
    assert!(
        body.contains("\"io.simulated.outputs\""),
        "settings module must expose simulated output count setting"
    );
    assert!(
        body.contains("\"io.simulated.scan_ms\""),
        "settings module must expose simulated scan interval setting"
    );
    assert!(
        body.contains("\"io.safe_state_json\""),
        "settings module must expose I/O safe state JSON setting"
    );
    assert!(
        body.contains("\"simulation.enabled\""),
        "settings module must expose simulation enabled setting"
    );
    assert!(
        body.contains("\"simulation.seed\""),
        "settings module must expose simulation seed setting"
    );
    assert!(
        body.contains("\"simulation.time_scale\""),
        "settings module must expose simulation time-scale setting"
    );
    assert!(
        body.contains("SETTINGS_SIMULATION_BINDINGS"),
        "settings module must declare simulation.toml bindings"
    );
    assert!(
        body.contains("settingsLoadSimulationConfigSnapshot"),
        "settings module must load simulation.toml in standalone mode"
    );
    assert!(
        body.contains("settingsPersistSimulationValue"),
        "settings module must persist simulation.toml setting edits"
    );
    assert!(
        body.contains("/api/ide/fs/create"),
        "settings module must create simulation.toml through fs/create when the file does not exist"
    );
    assert!(
        body.contains("\"resource.tasks_json\""),
        "settings module must expose resource task override JSON setting"
    );
    assert!(
        body.contains("json-array"),
        "settings module must support array JSON coercion for IO driver params"
    );
    assert!(
        body.contains("\"resource.tasks\": \"resource.tasks_json\""),
        "settings module must map resource.tasks online snapshot key to JSON-backed field key"
    );
    assert!(
        body.contains("\"observability.include\": \"observability.include_json\""),
        "settings module must map observability.include to JSON-backed field key"
    );
    assert!(
        body.contains("\"observability.alerts\": \"observability.alerts_json\""),
        "settings module must map observability.alerts to JSON-backed field key"
    );
    assert!(
        body.contains("observability-alert-rules-json"),
        "settings module must encode/decode observability alert rules from runtime.toml"
    );
    assert!(
        body.contains("resource-task-rules-json"),
        "settings module must encode/decode resource tasks from runtime.toml"
    );
    assert!(
        body.contains("function settingsParseSafeStateJsonOrThrow"),
        "settings module must validate safe_state JSON edits before save"
    );
    assert!(
        body.contains("function settingsEnqueueSave"),
        "settings module must serialize setting saves to avoid lost concurrent writes"
    );
    assert!(
        body.contains("function settingsNotifyRuntimeConfigUpdated"),
        "settings module must notify other tabs when runtime.toml values change"
    );
    assert!(
        body.contains("ide-runtime-config-updated"),
        "settings module must dispatch ide-runtime-config-updated events"
    );
    assert!(
        body.contains("Runtime State (Read-only)"),
        "settings module advanced panel must expose runtime read-only state"
    );
    assert!(
        body.contains("settingsImportBtn"),
        "settings advanced panel must expose import action"
    );
    assert!(
        body.contains("id: \"all\""),
        "settings module must expose an All Settings category"
    );
    assert!(
        body.contains("settings-category-search"),
        "settings module must expose settings filter/search control"
    );
    assert!(
        body.contains("settingsGroupsForAllCategory"),
        "settings module must support rendering all categories in one view"
    );
    assert!(
        body.contains("settingsLoadRuntimeTargets"),
        "settings module must load standalone runtime targets for per-runtime editing"
    );
    assert!(
        body.contains("/api/config-ui/runtime/lifecycle"),
        "settings module must query config-ui runtime lifecycle in standalone mode"
    );
    assert!(
        body.contains("settingsRenderRuntimeScopeBar"),
        "settings module must render runtime scope selector in standalone mode"
    );
    assert!(
        body.contains("const SETTINGS_RUNTIME_SELECTION_EVENT"),
        "settings module must share runtime selection event contract with hardware tab"
    );
    assert!(
        body.contains("/api/config-ui/io/config?runtime_id="),
        "settings module must read io.toml via config-ui scoped runtime endpoint"
    );
    assert!(
        body.contains("/api/config-ui/io/config"),
        "settings module must persist io.toml via config-ui scoped runtime endpoint"
    );
    assert!(
        body.contains("settings-filter-summary"),
        "settings module must expose active-filter summary to avoid hidden fields confusion"
    );
    assert!(
        body.contains("data-settings-clear-filter"),
        "settings module must provide one-click filter clear action"
    );

    let online_key_block = body
        .find("const SETTINGS_ONLINE_KEYS = new Set([")
        .and_then(|start| {
            body[start..]
                .find("]);")
                .map(|end| &body[start..start + end])
        })
        .expect("settings module must define SETTINGS_ONLINE_KEYS");
    assert!(
        !online_key_block.contains("\"opcua.username\""),
        "online key set must not include unsupported opcua.username runtime-control write"
    );
    assert!(
        !online_key_block.contains("\"opcua.password\""),
        "online key set must not include unsupported opcua.password runtime-control write"
    );
    assert!(
        !online_key_block.contains("\"observability.include_json\""),
        "online key set must not include unsupported observability.include_json runtime-control write"
    );
    assert!(
        !online_key_block.contains("\"observability.alerts_json\""),
        "online key set must not include unsupported observability.alerts_json runtime-control write"
    );

    let _ = std::fs::remove_dir_all(project);
}
