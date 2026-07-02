use trust_runtime::harness::TestHarness;
use trust_runtime::opcua::parse_opcua_client_toml;

#[test]
fn opcua_client_accepts_vs_code_global_var_names() {
    let source = r#"
PROGRAM Main
END_PROGRAM

CONFIGURATION Config
VAR_GLOBAL
    ConveyorRunning : BOOL := FALSE;
END_VAR
RESOURCE CommRes ON PLC
    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM P1 WITH MainTask : Main;
END_RESOURCE
END_CONFIGURATION
"#;
    let mut harness = TestHarness::from_source(source).expect("project should compile");
    let config = parse_opcua_client_toml(
        r#"
[[connections]]
name = "line1"
endpoint_url = "opc.tcp://127.0.0.1:4840/trust"
security_policy = "none"
security_mode = "none"
auth = "anonymous"
trust_server_certificate = true

[[connections.points]]
var = "global.ConveyorRunning"
node_id = "ns=2;i=2"
type = "bool"
access = "read"
"#,
    )
    .expect("OPC UA client config should parse");

    harness
        .runtime_mut()
        .configure_opcua_client(&config)
        .expect("UI-generated global.X point should bind to runtime storage");

    let status = harness.runtime().opcua_client_status_report();
    assert_eq!(status.connections.len(), 1);
    assert_eq!(
        status.connections[0].points[0].var,
        "global.ConveyorRunning"
    );
}
