use trust_runtime::harness::TestHarness;
use trust_runtime::opcua::parse_opcua_client_toml;

#[cfg(feature = "opcua-wire")]
#[test]
fn opcua_client_subscription_api_surface_is_available_for_phase3_worker() {
    use opcua::client::prelude::{
        DataChangeCallback, MonitoredItemCreateRequest, MonitoredItemService, NodeId,
        RepublishRequest, RequestHeader, Service, Session, SubscriptionService, SupportedMessage,
        TimestampsToReturn,
    };
    use opcua::sync::RwLock;
    use opcua::types::StatusCode;
    use std::sync::Arc;

    fn requires_subscription_service<T: SubscriptionService>() {}
    fn requires_monitored_item_service<T: MonitoredItemService>() {}
    fn requires_service<T: Service>() {}
    fn send_republish_request(
        session: &Session,
        request: RepublishRequest,
    ) -> Result<SupportedMessage, StatusCode> {
        <Session as Service>::send_request(session, request)
    }

    requires_subscription_service::<Session>();
    requires_monitored_item_service::<Session>();
    requires_service::<Session>();

    let _item: MonitoredItemCreateRequest = NodeId::new(2, "Demo").into();
    let _timestamps = TimestampsToReturn::Both;
    let _republish = RepublishRequest {
        request_header: RequestHeader::default(),
        subscription_id: 1,
        retransmit_sequence_number: 1,
    };
    let _send_republish: fn(&Session, RepublishRequest) -> Result<SupportedMessage, StatusCode> =
        send_republish_request;
    let _callback = DataChangeCallback::new(|items| {
        for item in items {
            let _node_id = item.item_to_monitor().node_id.clone();
            let _last_value = item.last_value();
        }
    });
    let _run: fn(Arc<RwLock<Session>>) = Session::run;
    let _reconnect: fn(&mut Session) -> Result<(), StatusCode> = Session::reconnect_and_activate;
}

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
