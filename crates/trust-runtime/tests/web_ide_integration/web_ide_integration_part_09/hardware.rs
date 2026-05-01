use super::super::*;

#[test]
fn unified_shell_hardware_module_exposes_runtime_cloud_link_transport_projection() {
    let project = make_project("hardware-cloud-links");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let mut response = ureq::get(&format!("{base}/ide/modules/ide-hardware.js"))
        .call()
        .expect("fetch ide-hardware.js");
    let body = response
        .body_mut()
        .read_to_string()
        .expect("read ide-hardware.js");

    assert!(
        body.contains("runtime.cloud.links"),
        "hardware module must parse runtime.cloud.links section"
    );
    assert!(
        body.contains("const HW_RUNTIME_LINK_TRANSPORTS"),
        "hardware module must define runtime link transport selector options"
    );
    assert!(
        body.contains("const HW_RUNTIME_LINK_TRANSPORT_NOTES"),
        "hardware module must define runtime link transport descriptions for picker UX"
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
            body.contains(&format!("id: \"{transport}\"")),
            "hardware module must expose runtime link transport '{transport}'"
        );
    }
    assert!(
        body.contains("Cloud Link Transports"),
        "hardware module must expose cloud link transport card label"
    );
    assert!(
        body.contains("hwParseCloudLinkTransportSection"),
        "hardware module must parse cloud link transport rules from runtime.toml"
    );
    assert!(
        body.contains("hwParseCloudWanAllowWriteSection"),
        "hardware module must parse runtime.cloud.wan allow_write rules from runtime.toml"
    );
    assert!(
        body.contains("Cloud WAN Access"),
        "hardware module must expose cloud WAN access card label"
    );
    assert!(
        body.contains("ide-runtime-config-updated"),
        "hardware module must react to runtime.toml updates from settings"
    );
    assert!(
        body.contains("client_id"),
        "hardware module must expose MQTT client_id parameter"
    );
    assert!(
        body.contains("allow_insecure_remote"),
        "hardware module must expose MQTT allow_insecure_remote parameter"
    );
    assert!(
        body.contains("inputs_json"),
        "hardware module must expose GPIO inputs JSON parameter"
    );
    assert!(
        body.contains("modules_json"),
        "hardware module must expose EtherCAT modules JSON parameter"
    );
    assert!(
        body.contains("mock_fail_read"),
        "hardware module must expose EtherCAT mock_fail_read parameter"
    );
    assert!(
        body.contains("/api/io/mqtt-test"),
        "hardware module must expose MQTT connection test endpoint call"
    );
    assert!(
        body.contains("Connection test is currently available for Modbus TCP and MQTT."),
        "hardware module should clearly communicate supported test drivers"
    );
    assert!(
        body.contains("runtime_cloud.links.transports_json"),
        "hardware module must deep-link cloud links nodes to settings transport rules"
    );
    assert!(
        body.contains("type: \"opcua\"")
            && body.contains("label: \"OPC UA\"")
            && body.contains("driver: \"opcua\""),
        "hardware palette OPC UA entry must use the OPC UA driver binding"
    );
    assert!(
        body.contains("if (name === \"simulated\") return \"io.simulated.inputs\""),
        "hardware module must deep-link simulated modules to simulated I/O settings"
    );
    assert!(
        body.contains("if (name === \"loopback\") return \"io.simulated.inputs\""),
        "hardware module must deep-link loopback modules to simulated I/O settings"
    );
    assert!(
        body.contains("data-hw-driver-settings"),
        "hardware driver cards must expose configure action deep-links into Settings"
    );
    assert!(
        body.contains("btn.dataset.hwDriverSettings"),
        "hardware driver card configure action must route by settings key"
    );
    assert!(
        body.contains("data-hw-driver-settings-category"),
        "hardware driver card configure action must include settings category routing metadata"
    );
    assert!(
        body.contains("function hwSettingsActionsForDriver"),
        "hardware module must provide per-driver settings action expansion"
    );
    assert!(
        body.contains("function hwSettingsCategoryForKey"),
        "hardware module must map settings keys to target categories"
    );
    assert!(
        body.contains("const HW_RUNTIME_SELECTION_EVENT = \"ide-runtime-selection-changed\""),
        "hardware module must share runtime selection event contract with settings tab"
    );
    assert!(
        body.contains("function hwBroadcastActiveRuntimeSelection"),
        "hardware module must broadcast runtime scope changes for settings synchronization"
    );
    assert!(
        body.contains("hwTransportModal"),
        "hardware module must integrate runtime link transport modal flow"
    );
    assert!(
        body.contains("data-hw-transport-option"),
        "hardware module must render clickable runtime link transport options"
    );
    assert!(
        body.contains("hwLinkFlowHint"),
        "hardware module must drive runtime link creation hint banner state"
    );
    assert!(
        body.contains("Runtime Control"),
        "hardware module must project runtime.control section as a hardware/runtime card"
    );
    assert!(
        body.contains("Deploy Security"),
        "hardware module must project runtime.deploy section as a hardware/runtime card"
    );
    assert!(
        body.contains("Observability"),
        "hardware module must project runtime.observability section as a hardware/runtime card"
    );
    assert!(
        body.contains("\"io.mqtt.topic_in\""),
        "hardware module must deep-link MQTT topic settings from driver cards"
    );
    assert!(
        body.contains("\"io.modbus.unit_id\""),
        "hardware module must deep-link Modbus unit ID settings from driver cards"
    );
    assert!(
        body.contains("\"tls.mode\""),
        "hardware module must deep-link TLS settings from runtime cards"
    );
    assert!(
        body.contains("\"control.debug_enabled\""),
        "hardware module must deep-link debug settings from runtime control card"
    );
    assert!(
        body.contains("el.hwCtxRuntimeCommSettingsBtn.classList.add(\"ide-hidden\")"),
        "endpoint context menu must hide duplicate protocol settings action"
    );
    assert!(
        body.contains("if (meta.type !== \"runtime\") return;"),
        "communication-settings context action must be runtime-only"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_exposes_mqtt_connectivity_probe_api() {
    let project = make_project("mqtt-probe-api");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let (missing_status, missing_json) = request_json(
        "POST",
        &format!("{base}/api/io/mqtt-test"),
        Some(json!({ "broker": "" })),
        &[],
    );
    assert_eq!(missing_status, 400, "empty broker must be rejected");
    assert!(
        !missing_json
            .get("ok")
            .and_then(|value| value.as_bool())
            .unwrap_or(false),
        "empty broker must return ok=false"
    );

    let (probe_status, probe_json) = request_json(
        "POST",
        &format!("{base}/api/io/mqtt-test"),
        Some(json!({ "broker": "not-a-valid-host-@@@:1883", "timeout_ms": 30 })),
        &[],
    );
    assert_eq!(
        probe_status, 200,
        "MQTT probe endpoint must return a structured probe result"
    );
    assert!(
        !probe_json
            .get("ok")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        "invalid host must return ok=false"
    );
    assert!(
        probe_json
            .get("error")
            .and_then(|value| value.as_str())
            .is_some(),
        "failed probe must include an error string"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_serves_composed_ide_modules_required_for_bootstrap() {
    let project = make_project("composed-ide-modules");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let checks: [(&str, &[&str]); 5] = [
        (
            "ide-editor-language.js",
            &[
                "function configureMonacoLanguageSupport()",
                "function buildFallbackHover",
            ],
        ),
        (
            "ide-editor-pane.js",
            &["function createEditor(", "function syncSecondaryEditor()"],
        ),
        (
            "ide-workspace-tree.js",
            &[
                "async function refreshProjectSelection()",
                "async function bootstrapFiles()",
                "async function doOpenProject",
            ],
        ),
        (
            "ide-observability.js",
            &[
                "async function loadPresenceModel()",
                "function refreshMultiTabCollision()",
            ],
        ),
        (
            "ide-commands.js",
            &[
                "async function workspaceSearchFlow()",
                "function ideConfirm(",
            ],
        ),
    ];

    for (module, required_symbols) in checks {
        let mut response = ureq::get(&format!("{base}/ide/modules/{module}"))
            .call()
            .unwrap_or_else(|err| panic!("fetch {module} failed: {err}"));
        let body = response
            .body_mut()
            .read_to_string()
            .unwrap_or_else(|_| panic!("read {module} failed"));
        assert!(body.len() > 500, "{module} must have non-trivial content");
        for symbol in required_symbols {
            assert!(
                body.contains(symbol),
                "{module} must contain symbol `{symbol}` for runtime bootstrap"
            );
        }
    }

    let _ = std::fs::remove_dir_all(project);
}
