//! The request the extension sends: the example of design §1.4.1, its defaults, and a token that
//! never appears in debug output.

use std::path::PathBuf;

use super::{DebugSessionKind, GatherMode, RunMode, ViewRequest};

#[test]
fn view_request_reads_the_host_facts() {
    let request: ViewRequest = serde_json::from_str(
        r#"{
          "session": "dc-7f3a", "seq": 12, "gather": "auto",
          "project": "/home/u/plant/line1",
          "workspace_folders": ["/home/u/plant"],
          "view": { "search": "pump", "hidden_protocols": ["mqtt"], "selected": "trust-fleet-v1:WyJy" },
          "host": {
            "runtime_executable": "/home/u/.vscode/extensions/trust/bin/trust-runtime",
            "settings": {
              "mode": "online", "control_endpoint": "tcp://10.0.0.5:5680", "use_control_endpoint": true,
              "additional_endpoints": ["tcp://10.0.0.6:5680"], "ads_discovery_ports": [301, 501, 851, 852, 853, 854]
            },
            "tokens": { "tcp://10.0.0.5:5680": "s3cr3t-primary" },
            "endpoint_labels": { "tcp://10.0.0.6:5680": "Line 2" },
            "run_target": "simulator",
            "debug_session": { "kind": "launch", "endpoint": "unix:///tmp/trust-debug-ab12cd34.sock",
                               "label": "Simulator", "starting": false,
                               "failure": { "kind": "port_conflict", "message": "The runtime port is already in use." } },
            "simulator": { "endpoint": "unix:///tmp/trust-debug-ab12cd34.sock", "token": "s3cr3t-simulator" },
            "io_state": [ { "name": "x", "address": "%IX0.0", "value": "TRUE" } ]
          }
        }"#,
    )
    .expect("the example request of the design");
    assert_eq!(request.session, "dc-7f3a");
    assert_eq!(request.seq, 12);
    assert_eq!(request.gather, GatherMode::Auto);
    assert_eq!(request.project, Some(PathBuf::from("/home/u/plant/line1")));
    assert_eq!(request.workspace_folders, [PathBuf::from("/home/u/plant")]);
    assert_eq!(request.view.search, "pump");
    assert_eq!(request.view.hidden_protocols, ["mqtt"]);
    assert_eq!(
        request.view.selected.as_deref(),
        Some("trust-fleet-v1:WyJy")
    );

    let host = &request.host;
    assert_eq!(host.settings.mode, RunMode::Online);
    assert_eq!(host.settings.control_endpoint, "tcp://10.0.0.5:5680");
    assert!(host.settings.use_control_endpoint);
    assert_eq!(host.settings.additional_endpoints, ["tcp://10.0.0.6:5680"]);
    assert_eq!(
        host.settings.ads_discovery_ports,
        [301, 501, 851, 852, 853, 854]
    );
    assert_eq!(
        host.token("tcp://10.0.0.5:5680").map(super::Token::secret),
        Some("s3cr3t-primary")
    );
    assert!(host.token("tcp://10.0.0.6:5680").is_none());
    assert_eq!(host.label("tcp://10.0.0.6:5680"), Some("Line 2"));
    assert_eq!(host.run_target.as_deref(), Some("simulator"));
    let session = host.debug_session.as_ref().expect("a debug session");
    assert_eq!(session.kind, DebugSessionKind::Launch);
    assert_eq!(
        session
            .failure
            .as_ref()
            .map(|failure| failure.kind.as_str()),
        Some("port_conflict")
    );
    let simulator = host.simulator.as_ref().expect("the simulator's endpoint");
    assert_eq!(simulator.endpoint, "unix:///tmp/trust-debug-ab12cd34.sock");
    assert_eq!(host.io_state[0].address, "%IX0.0");

    // A token never shows in debug output.
    let printed = format!("{request:?}");
    assert!(!printed.contains("s3cr3t"), "{printed}");
    assert!(printed.contains("<redacted>"));

    // Only the session and the number are required. Every mode other than `online` is the
    // simulator, as before.
    let minimal: ViewRequest = serde_json::from_str(
        r#"{"session":"dc-1","seq":1,"gather":"none","host":{"settings":{"mode":"remote"}}}"#,
    )
    .expect("a minimal request");
    assert_eq!(minimal.gather, GatherMode::Cached);
    assert_eq!(minimal.host.settings.mode, RunMode::Simulate);
    assert!(minimal.host.settings.use_control_endpoint);
    assert!(minimal.project.is_none());
    assert!(minimal.host.debug_session.is_none());
    assert!(serde_json::from_str::<ViewRequest>(r#"{"seq":1}"#).is_err());
}
