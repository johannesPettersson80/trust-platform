use super::super::*;

#[test]
fn unified_shell_ide_client_supports_wrapped_and_direct_api_payloads() {
    let project = make_project("ide-client-api-payloads");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let mut ide_js_response = ureq::get(&format!("{base}/ide/ide.js"))
        .call()
        .expect("fetch ide.js");
    let ide_js = ide_js_response
        .body_mut()
        .read_to_string()
        .expect("read ide.js");

    assert!(
        ide_js.contains("Object.prototype.hasOwnProperty.call(payload, \"result\")"),
        "ide.js API client must detect wrapped payloads"
    );
    assert!(
        ide_js.contains("return payload;"),
        "ide.js API client must support direct payloads without a result envelope"
    );

    let mode_payload = ureq::get(&format!("{base}/api/ui/mode"))
        .call()
        .expect("fetch /api/ui/mode")
        .body_mut()
        .read_to_string()
        .expect("read /api/ui/mode payload");
    let mode_value: serde_json::Value =
        serde_json::from_str(&mode_payload).expect("parse /api/ui/mode payload");
    assert!(
        mode_value
            .get("mode")
            .and_then(|value| value.as_str())
            .is_some(),
        "/api/ui/mode must expose mode as a top-level string field"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_html_contract_contains_tab_panels_and_status_bar() {
    let project = make_project("shell-contract");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let shell = ureq::get(&format!("{base}/ide"))
        .call()
        .expect("fetch ide shell")
        .body_mut()
        .read_to_string()
        .expect("read ide shell");

    // Tab navigation
    assert!(
        shell.contains("id=\"ideTabNav\""),
        "shell must have tab navigation"
    );

    // Tab panels
    for tab in ["code", "hardware", "settings", "logs"] {
        assert!(
            shell.contains(&format!("data-tab=\"{tab}\"")),
            "shell must have {tab} tab panel"
        );
    }

    // Hardware tab elements
    assert!(
        shell.contains("id=\"hwCanvas\""),
        "shell must have hardware canvas"
    );
    assert!(
        shell.contains("id=\"hwAddressTable\""),
        "shell must have hardware address table"
    );
    assert!(
        !shell.contains("id=\"hwSummary\""),
        "shell must not expose removed hardware summary cards container"
    );
    assert!(
        shell.contains("id=\"hwDriverCards\""),
        "shell must have hardware driver cards container"
    );
    assert!(
        shell.contains("id=\"hwPropertyPanel\""),
        "shell must have hardware property panel"
    );
    assert!(
        shell.contains("id=\"hwLinkFlowHint\""),
        "shell must have runtime link creation guidance banner"
    );
    assert!(
        !shell.contains("id=\"hwCanvasToolbar\""),
        "shell must not expose removed hardware canvas toolbar controls"
    );
    assert!(
        !shell.contains("id=\"hwFabricFilterSelect\""),
        "shell must not expose the removed hardware communication filter control"
    );
    assert!(
        !shell.contains("id=\"hwRuntimeLinkStudio\""),
        "shell must not expose the removed runtime link studio panel"
    );
    assert!(
        !shell.contains("id=\"hwTransportPills\""),
        "shell must not expose removed runtime transport summary pills"
    );
    assert!(
        shell.contains("id=\"hwNodeContextMenu\""),
        "shell must expose hardware runtime node context menu"
    );
    assert!(
        shell.contains("id=\"hwEdgeContextMenu\""),
        "shell must expose hardware edge context menu"
    );
    assert!(
        shell.contains("id=\"hwCtxCreateLinkFromEdgeBtn\""),
        "shell must expose context action to add runtime links from existing links"
    );
    assert!(
        shell.contains("id=\"hwCtxOpenLinkSettingsBtn\""),
        "shell must expose context action to jump from link to settings"
    );
    assert!(
        !shell.contains("id=\"hwLegendToggleBtn\""),
        "shell must not expose removed legend toggle control"
    );
    assert!(
        !shell.contains("id=\"hwLegend\""),
        "shell must not expose removed hardware communication legend"
    );
    assert!(
        !shell.contains("id=\"hwToggleInspectorBtn\""),
        "shell must not expose removed inspector toggle control"
    );
    assert!(
        !shell.contains("id=\"hwToggleDriversBtn\""),
        "shell must not expose removed drivers toggle control"
    );
    assert!(
        !shell.contains("id=\"hwCenterCanvasBtn\""),
        "shell must not expose removed canvas center control"
    );
    assert!(
        !shell.contains("id=\"hwFullscreenBtn\""),
        "shell must not expose removed canvas fullscreen control"
    );
    assert!(
        shell.contains("id=\"hwDriversPanel\""),
        "shell must expose collapsible hardware driver panel"
    );
    assert!(
        shell.contains("id=\"hardwarePalette\""),
        "shell must have hardware palette"
    );
    assert!(
        shell.contains("id=\"hwTransportModal\""),
        "shell must have runtime link transport picker modal"
    );
    assert!(
        shell.contains("id=\"hwTransportOptions\""),
        "shell must have runtime link transport picker options container"
    );
    for removed_copy in [
        "Modules",
        "I/O Points",
        "Active Drivers",
        "Address Health",
        "Fabric",
        "Address Map",
        "Fit",
        "Center",
        "Inspector",
        "Fullscreen",
        "Reload",
        "Active links",
        "Legend",
    ] {
        assert!(
            !shell.contains(removed_copy),
            "shell must not render removed hardware chrome text `{removed_copy}`"
        );
    }

    // Connection dialog
    assert!(
        shell.contains("id=\"connectionDialog\""),
        "shell must have connection dialog"
    );

    // Debug toolbar and panels
    assert!(
        shell.contains("id=\"debugToolbar\""),
        "shell must have debug toolbar"
    );
    assert!(
        shell.contains("id=\"debugVariablesPanel\""),
        "shell must have debug variables panel"
    );
    assert!(
        shell.contains("id=\"debugCallStackPanel\""),
        "shell must have debug call stack panel"
    );
    assert!(
        shell.contains("id=\"debugWatchPanel\""),
        "shell must have debug watch panel"
    );

    // Settings workspace
    assert!(
        shell.contains("id=\"settingsCategories\""),
        "shell must have settings categories sidebar"
    );
    assert!(
        shell.contains("id=\"settingsFormPanel\""),
        "shell must have settings form panel"
    );

    // Logs workspace
    assert!(
        shell.contains("id=\"logsFilterBar\""),
        "shell must have logs filter bar"
    );
    assert!(
        shell.contains("id=\"logsTablePanel\""),
        "shell must have logs table panel"
    );

    // Status bar
    assert!(
        shell.contains("id=\"syncBadge\""),
        "shell must have sync badge in status bar"
    );
    assert!(
        shell.contains("id=\"statusLatency\""),
        "shell must have latency label in status bar"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn unified_shell_removes_legacy_fleet_routes() {
    let project = make_project("fleet-compat");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    for path in [
        "/fleet",
        "/app.js",
        "/runtime-cloud-utils.js",
        "/styles.css",
        "/modules/fleet.js",
    ] {
        let response = ureq::get(&format!("{base}{path}"))
            .config()
            .http_status_as_error(false)
            .build()
            .call()
            .unwrap_or_else(|err| panic!("fetch {path} failed: {err}"));
        assert_eq!(response.status().as_u16(), 404, "{path} must return 404");
    }

    let _ = std::fs::remove_dir_all(project);
}
