use super::*;

#[test]
fn current_session_samples_require_exact_configured_identity() {
    let configured = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let cases = [
        (
            "unknown variable",
            opcua_point(
                "other_temp",
                "ns=2;i=2",
                OpcUaDataType::Float,
                OpcUaClientPointAccess::Read,
            ),
        ),
        (
            "wrong node",
            opcua_point(
                "line1_temp",
                "ns=2;i=99",
                OpcUaDataType::Float,
                OpcUaClientPointAccess::Read,
            ),
        ),
        (
            "wrong data type",
            opcua_point(
                "line1_temp",
                "ns=2;i=2",
                OpcUaDataType::Double,
                OpcUaClientPointAccess::Read,
            ),
        ),
        (
            "wrong access",
            opcua_point(
                "line1_temp",
                "ns=2;i=2",
                OpcUaDataType::Float,
                OpcUaClientPointAccess::ReadWrite,
            ),
        ),
    ];

    for (case_name, observed) in cases {
        let runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
        let connection = opcua_connection(vec![configured.clone()]);
        let bindings = opcua_bindings(&runtime, std::slice::from_ref(&configured));
        let (bridge, mut worker) = OpcUaClientBridge::with_transport(
            connection,
            MockOpcUaClientTransport::default(),
            bindings,
        )
        .expect("bridge");
        worker.tick(0).expect("connect");
        let sink = worker
            .transport()
            .sink
            .clone()
            .expect("connected event sink");

        assert!(sink.publish_sample(OpcUaClientSample {
            var: observed.var,
            node_id: observed.node_id,
            data_type: observed.data_type,
            access: observed.access,
            value: Some(Value::Real(22.5)),
            state: OpcUaClientConnectionState::Connected,
            last_seen_ms: Some(10),
            detail: "mismatched subscription sample".to_string(),
        }));
        worker.tick(10).expect("drain invalid sample");

        let snapshot = bridge.snapshot();
        assert_eq!(
            snapshot.state,
            OpcUaClientConnectionState::Faulted,
            "{case_name}: invalid identity must be a visible fault"
        );
        assert!(
            snapshot.values.is_empty(),
            "{case_name}: invalid identity entered the accepted-value cache"
        );
        assert_eq!(
            snapshot.point_statuses.len(),
            1,
            "{case_name}: invalid identity created a dynamic point status"
        );
        assert_eq!(snapshot.point_statuses[0].var, configured.var);
        assert_eq!(snapshot.point_statuses[0].node_id, configured.node_id);
    }
}

#[test]
fn older_session_callbacks_cannot_regress_reconnected_worker() {
    let runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let connection = opcua_connection(vec![point.clone()]);
    let bindings = opcua_bindings(&runtime, &[point]);
    let (bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("initial connect");
    let old_sink = worker.transport().sink.clone().expect("initial event sink");
    assert!(old_sink.publish_connection_status(false, 100, "first session lost"));
    worker.tick(100).expect("enter reconnecting");
    worker.tick(2_101).expect("replacement connection");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);

    assert!(old_sink.publish_session_closed(2_200, "delayed close from first session"));
    worker
        .tick(2_200)
        .expect("drain delayed old-session callback");

    assert_eq!(
        bridge.state(),
        OpcUaClientConnectionState::Connected,
        "a callback from the replaced session regressed current authority"
    );
}

#[test]
fn older_active_session_sample_cannot_replace_newer_value() {
    let runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let connection = opcua_connection(vec![point.clone()]);
    let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
    let (bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect");
    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(20.0), 20);
    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(10.0), 10);
    worker.tick(20).expect("drain samples");

    assert_eq!(
        bridge.snapshot().values.get("line1_temp"),
        Some(&Value::Real(20.0)),
        "an older sample replaced the newer accepted value"
    );
}

#[test]
fn write_completion_preserves_newer_pending_generation() {
    for reject_transmitted in [false, true] {
        let mut runtime = runtime_with_opcua_globals(vec![("line1_setpoint", Value::Real(0.0))]);
        let point = opcua_point(
            "line1_setpoint",
            "ns=2;i=3",
            OpcUaDataType::Float,
            OpcUaClientPointAccess::ReadWrite,
        );
        let connection = opcua_connection(vec![point.clone()]);
        let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
        let mut bridge = OpcUaClientBridge::new(bindings).expect("bridge");
        let transport = RequeueingTransport {
            shared: bridge.shared.clone(),
            replacement: Value::Real(44.0),
            reject_transmitted,
            ..RequeueingTransport::default()
        };
        let mut worker = OpcUaClientWorker::new(connection, transport, bridge.shared.clone());
        worker.tick(0).expect("connect");

        write_global(&mut runtime, "line1_setpoint", Value::Real(31.0));
        bridge
            .capture_outputs(runtime.storage_mut(), 10)
            .expect("queue transmitted generation");
        worker
            .tick(11)
            .expect("validation rejection remains point-local");

        assert_eq!(
            bridge.pending_write("line1_setpoint"),
            Some(Value::Real(44.0)),
            "completion for an older generation removed its replacement; reject={reject_transmitted}"
        );
    }
}

#[test]
fn non_finite_outputs_cancel_pending_and_allow_later_finite_value() {
    let cases = [
        (
            "REAL NaN",
            OpcUaDataType::Float,
            Value::Real(1.0),
            Value::Real(f32::NAN),
            Value::Real(2.0),
        ),
        (
            "REAL positive infinity",
            OpcUaDataType::Float,
            Value::Real(1.0),
            Value::Real(f32::INFINITY),
            Value::Real(2.0),
        ),
        (
            "LREAL negative infinity",
            OpcUaDataType::Double,
            Value::LReal(1.0),
            Value::LReal(f64::NEG_INFINITY),
            Value::LReal(2.0),
        ),
    ];

    for (case_name, data_type, first, invalid, recovery) in cases {
        let mut runtime = runtime_with_opcua_globals(vec![("line1_setpoint", first.clone())]);
        let point = opcua_point(
            "line1_setpoint",
            "ns=2;i=3",
            data_type,
            OpcUaClientPointAccess::ReadWrite,
        );
        let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
        let mut bridge = OpcUaClientBridge::new(bindings).expect("bridge");

        bridge
            .capture_outputs(runtime.storage_mut(), 1)
            .expect("queue finite baseline");
        assert_eq!(bridge.pending_write("line1_setpoint"), Some(first.clone()));

        write_global(&mut runtime, "line1_setpoint", invalid);
        bridge
            .capture_outputs(runtime.storage_mut(), 2)
            .expect("reject non-finite output");
        assert_eq!(
            bridge.pending_write("line1_setpoint"),
            None,
            "{case_name}: non-finite output remained queued"
        );
        let status = bridge
            .snapshot()
            .point_statuses
            .into_iter()
            .find(|status| status.var == "line1_setpoint")
            .expect("point status");
        assert_eq!(
            status.state,
            OpcUaClientConnectionState::Faulted,
            "{case_name}"
        );
        assert!(
            status.detail.contains("non-finite"),
            "{case_name}: unexpected detail: {}",
            status.detail
        );

        write_global(&mut runtime, "line1_setpoint", recovery.clone());
        bridge
            .capture_outputs(runtime.storage_mut(), 3)
            .expect("queue finite recovery");
        assert_eq!(
            bridge.pending_write("line1_setpoint"),
            Some(recovery),
            "{case_name}: finite recovery was suppressed"
        );
    }
}

#[test]
fn worker_shutdown_interrupts_idle_wait_and_revokes_good_authority() {
    let runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let connection = opcua_connection(vec![point.clone()]);
    let bindings = opcua_bindings(&runtime, &[point]);
    let (bridge, worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");
    let thread = worker
        .spawn(std::time::Duration::from_secs(2))
        .expect("spawn worker");

    let connect_deadline = std::time::Instant::now() + std::time::Duration::from_millis(500);
    while bridge.state() != OpcUaClientConnectionState::Connected
        && std::time::Instant::now() < connect_deadline
    {
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert_eq!(
        bridge.state(),
        OpcUaClientConnectionState::Connected,
        "worker did not connect before shutdown probe"
    );

    let started = std::time::Instant::now();
    thread.shutdown().expect("shutdown worker");
    let elapsed = started.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "idle shutdown waited for the tick interval: {elapsed:?}"
    );
    assert_ne!(
        bridge.state(),
        OpcUaClientConnectionState::Connected,
        "shutdown left stale Good connection authority visible"
    );
}

#[cfg(feature = "opcua-wire")]
#[test]
fn wire_write_status_cardinality_must_match_request() {
    let values = vec![
        (
            opcua_point(
                "line1_a",
                "ns=2;i=3",
                OpcUaDataType::Float,
                OpcUaClientPointAccess::ReadWrite,
            ),
            Value::Real(1.0),
        ),
        (
            opcua_point(
                "line1_b",
                "ns=2;i=4",
                OpcUaDataType::Float,
                OpcUaClientPointAccess::ReadWrite,
            ),
            Value::Real(2.0),
        ),
    ];
    let good = ::opcua::client::prelude::StatusCode::Good;

    for (case_name, statuses) in [
        ("missing result", vec![good]),
        ("extra result", vec![good, good, good]),
    ] {
        let error = validate_opcua_write_statuses(values.as_slice(), statuses.as_slice())
            .expect_err(case_name);
        assert!(
            error.message().contains("returned")
                && error.message().contains("status")
                && error.message().contains("requested"),
            "{case_name}: unexpected error: {error}"
        );
    }
}

#[derive(Clone)]
struct RequeueingTransport {
    shared: OpcUaSharedClientCache,
    replacement: Value,
    reject_transmitted: bool,
    sink: Option<OpcUaClientEventSink>,
}

impl Default for RequeueingTransport {
    fn default() -> Self {
        let point = opcua_point(
            "unused",
            "ns=0;i=0",
            OpcUaDataType::Float,
            OpcUaClientPointAccess::ReadWrite,
        );
        Self {
            shared: OpcUaSharedClientCache::new(&[point]),
            replacement: Value::Real(0.0),
            reject_transmitted: false,
            sink: None,
        }
    }
}

impl OpcUaClientTransport for RequeueingTransport {
    fn connect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        sink: OpcUaClientEventSink,
    ) -> Result<OpcUaClientSessionInfo, OpcUaClientBridgeError> {
        self.sink = Some(sink);
        Ok(OpcUaClientSessionInfo {
            requested_timeout_ms: connection.timeout_ms,
            revised_timeout_ms: Some(connection.timeout_ms),
            recovery_detail: None,
        })
    }

    fn subscribe_read_points(
        &mut self,
        _points: &[OpcUaClientPointConfig],
        _sink: OpcUaClientEventSink,
    ) -> Result<(), OpcUaClientBridgeError> {
        Ok(())
    }

    fn write_values(
        &mut self,
        values: &[(OpcUaClientPointConfig, Value)],
    ) -> Result<(), OpcUaClientBridgeError> {
        let (point, _) = values.first().expect("transmitted write");
        let _ = self.shared.queue_write(point, self.replacement.clone());
        if self.reject_transmitted {
            Err(OpcUaClientBridgeError::validation(
                "OPC UA transmitted generation rejected",
            ))
        } else {
            Ok(())
        }
    }

    fn disconnect(&mut self) -> Result<(), OpcUaClientBridgeError> {
        Ok(())
    }
}
