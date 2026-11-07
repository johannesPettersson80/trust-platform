//! The freshness of an OPC UA client connection's subscription (specification 11, "OPC UA client
//! worker, cache, and scalar-output integrity"): a subscription reports a value only when it
//! changes, and its heartbeat shows that it is alive.
use super::*;

/// The state of the point `var`.
fn point_state(bridge: &OpcUaClientBridge, var: &str) -> OpcUaClientConnectionState {
    bridge
        .snapshot()
        .point_statuses
        .into_iter()
        .find(|status| status.var == var)
        .expect("the point's status")
        .state
}

/// Publishes a notification of `point` whose status is not good, as the wire transport reports
/// one: the point is stale, without a value or a time.
fn emit_bad_status(transport: &MockOpcUaClientTransport, point: &OpcUaClientPointConfig) {
    let sink = transport.sink.as_ref().expect("event sink");
    assert!(sink.publish_sample(OpcUaClientSample {
        var: point.var.clone(),
        node_id: point.node_id.clone(),
        data_type: point.data_type,
        access: point.access,
        value: None,
        state: OpcUaClientConnectionState::Stale,
        last_seen_ms: None,
        detail: format!(
            "OPC UA node '{}' subscription returned BadSensorFailure",
            point.node_id
        ),
    }));
}

/// The subscription's heartbeat keeps a steady subscription live. The server reports `line1_temp`
/// 12 once, at 10 ms, and then only its heartbeat, at 40, 80 and 120 ms; with a timeout of 50 ms
/// and a publishing interval of 10 ms, the connection and the point are still live at 125 ms, and
/// the first scan, at 126 ms, applies 12. Catches a client that calls a steady subscription stale
/// after its timeout, and a value that arrived but is never applied because the first scan came
/// after the timeout.
#[test]
fn a_heartbeat_keeps_a_steady_subscription_live() {
    let mut storage = storage_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let mut connection = opcua_connection(vec![point.clone()]);
    connection.timeout_ms = 50;
    let bindings = opcua_bindings(&storage, std::slice::from_ref(&point));
    let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect");
    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(12.0), 10);
    worker.tick(10).expect("the value");
    for at in [40, 80, 120] {
        worker.transport_mut().emit_heartbeat(at);
        worker.tick(at).expect("a heartbeat");
    }
    worker.tick(125).expect("still live");

    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    assert_eq!(
        point_state(&bridge, "line1_temp"),
        OpcUaClientConnectionState::Connected
    );
    bridge
        .apply_inputs_compiled(&mut storage, 126)
        .expect("the first scan");
    assert_eq!(storage.0.get("line1_temp"), Some(&Value::Real(12.0)));
}

/// Silence, no notification for the stale bound, makes the connection and its live point stale,
/// and the point's value is not applied; the next heartbeat makes both live again with the last
/// value, which the next scan applies. The server reports `line1_temp` 12 at 10 ms and then nothing
/// until a heartbeat at 70 ms; with a timeout of 50 ms the connection is stale at 61 ms. Catches
/// silence that leaves a connection live, a stale value applied as live, and a point that stays
/// stale after its subscription's notifications resume, until its value changes.
#[test]
fn silence_makes_a_connection_stale_until_its_next_notification() {
    let mut storage = storage_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let mut connection = opcua_connection(vec![point.clone()]);
    connection.timeout_ms = 50;
    let bindings = opcua_bindings(&storage, std::slice::from_ref(&point));
    let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect");
    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(12.0), 10);
    worker.tick(10).expect("the value");
    worker.tick(61).expect("silence");

    assert_eq!(bridge.state(), OpcUaClientConnectionState::Stale);
    assert_eq!(
        point_state(&bridge, "line1_temp"),
        OpcUaClientConnectionState::Stale
    );
    bridge
        .apply_inputs_compiled(&mut storage, 62)
        .expect("a scan while stale");
    assert_eq!(storage.0.get("line1_temp"), Some(&Value::Real(0.0)));

    worker.transport_mut().emit_heartbeat(70);
    worker.tick(70).expect("the heartbeat");

    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    assert_eq!(
        point_state(&bridge, "line1_temp"),
        OpcUaClientConnectionState::Connected
    );
    bridge
        .apply_inputs_compiled(&mut storage, 71)
        .expect("a scan after the heartbeat");
    assert_eq!(storage.0.get("line1_temp"), Some(&Value::Real(12.0)));
}

/// With specification 23: a point whose server reports a status that is not good is stale alone.
/// The connection stays live and the other point's value is applied, and neither silence nor the
/// next heartbeat makes that point live. `line1_temp` reports 12 and `line1_level` 3 at 10 ms;
/// `line1_level` then reports `BadSensorFailure` at 20 ms; silence makes the connection stale at
/// 61 ms and the heartbeat at 70 ms live again. Catches one bad point that makes its whole
/// connection stale or faulted, and a heartbeat that hides a bad point.
#[test]
fn a_point_its_server_reports_bad_is_stale_alone() {
    let mut storage = storage_with_opcua_globals(vec![
        ("line1_temp", Value::Real(0.0)),
        ("line1_level", Value::Real(0.0)),
    ]);
    let temp = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let level = opcua_point(
        "line1_level",
        "ns=2;i=3",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let mut connection = opcua_connection(vec![temp.clone(), level.clone()]);
    connection.timeout_ms = 50;
    let bindings = opcua_bindings(&storage, &[temp.clone(), level.clone()]);
    let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect");
    worker
        .transport_mut()
        .emit_sample(&temp, Value::Real(12.0), 10);
    worker
        .transport_mut()
        .emit_sample(&level, Value::Real(3.0), 10);
    worker.tick(10).expect("the values");
    emit_bad_status(worker.transport(), &level);
    worker.tick(20).expect("the bad status");

    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    assert_eq!(
        point_state(&bridge, "line1_level"),
        OpcUaClientConnectionState::Stale
    );
    bridge
        .apply_inputs_compiled(&mut storage, 21)
        .expect("a scan");
    assert_eq!(storage.0.get("line1_temp"), Some(&Value::Real(12.0)));
    assert_eq!(storage.0.get("line1_level"), Some(&Value::Real(0.0)));

    worker.tick(61).expect("silence");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Stale);
    worker.transport_mut().emit_heartbeat(70);
    worker.tick(70).expect("the heartbeat");

    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    assert_eq!(
        point_state(&bridge, "line1_temp"),
        OpcUaClientConnectionState::Connected
    );
    assert_eq!(
        point_state(&bridge, "line1_level"),
        OpcUaClientConnectionState::Stale
    );
}

/// The stale bound is the timeout, and at least three publishing intervals, so that one late
/// heartbeat is not silence. With a publishing interval of 40 ms and a timeout of 50 ms the bound
/// is 120 ms: after the value at 10 ms the connection is live at 130 ms, after 120 ms of silence,
/// and stale at 131 ms. Catches a bound of one publishing interval or of the timeout alone.
#[test]
fn the_stale_bound_is_at_least_three_publishing_intervals() {
    let storage = storage_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let mut connection = opcua_connection(vec![point.clone()]);
    connection.timeout_ms = 50;
    connection.poll_interval_ms = 40;
    let bindings = opcua_bindings(&storage, std::slice::from_ref(&point));
    let (bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect");
    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(12.0), 10);
    worker.tick(10).expect("the value");
    worker.tick(130).expect("120 ms of silence");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    worker.tick(131).expect("121 ms of silence");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Stale);
}
