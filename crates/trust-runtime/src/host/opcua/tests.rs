use super::*;
use crate::value::{EnumValue, Value};
use smol_str::SmolStr;
use std::sync::Mutex;

static OPCUA_CLIENT_PKI_ENV_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn maps_scalar_numeric_and_string_types() {
    assert_eq!(
        map_iec_value(&Value::Bool(true)),
        Some(OpcUaValue {
            data_type: OpcUaDataType::Boolean,
            value: OpcUaVariant::Boolean(true),
        })
    );
    assert_eq!(
        map_iec_value(&Value::DInt(42)),
        Some(OpcUaValue {
            data_type: OpcUaDataType::Int32,
            value: OpcUaVariant::Int32(42),
        })
    );
    assert_eq!(
        map_iec_value(&Value::LReal(3.5)),
        Some(OpcUaValue {
            data_type: OpcUaDataType::Double,
            value: OpcUaVariant::Double(3.5),
        })
    );
    assert_eq!(
        map_iec_value(&Value::String(smol_str::SmolStr::new("Pump"))),
        Some(OpcUaValue {
            data_type: OpcUaDataType::String,
            value: OpcUaVariant::String("Pump".to_string()),
        })
    );
}

#[test]
fn maps_enum_values_as_string_variants() {
    let mut registry = trust_hir::types::TypeRegistry::new();
    let quality = registry.register_enum(
        "ADS_QUALITY",
        trust_hir::TypeId::INT,
        vec![
            (smol_str::SmolStr::new("Stale"), 0),
            (smol_str::SmolStr::new("Good"), 1),
        ],
    );
    let value = Value::Enum(Box::new(
        EnumValue::new(&registry, quality, "Good").expect("enum value"),
    ));

    assert_eq!(
        map_iec_value(&value),
        Some(OpcUaValue {
            data_type: OpcUaDataType::String,
            value: OpcUaVariant::String("Good".to_string()),
        })
    );
}

#[test]
fn rejects_non_scalar_or_protocol_specific_types() {
    assert!(map_iec_value(&Value::Null).is_none());
    assert!(map_iec_value(&Value::Reference(None)).is_none());
    assert!(map_iec_value(&Value::Time(crate::value::Duration::from_millis(10))).is_none());
}

#[test]
fn secure_profile_defaults_to_signed_and_encrypted_policy() {
    assert_eq!(
        OpcUaSecurityProfile::default(),
        OpcUaSecurityProfile {
            policy: OpcUaSecurityPolicy::Basic256Sha256,
            mode: OpcUaMessageSecurityMode::SignAndEncrypt,
            allow_anonymous: false,
        }
    );
}

#[test]
fn parses_security_policy_and_mode_aliases() {
    assert_eq!(
        OpcUaSecurityPolicy::parse("basic256_sha256"),
        Some(OpcUaSecurityPolicy::Basic256Sha256)
    );
    assert_eq!(
        OpcUaSecurityPolicy::parse("Aes128-Sha256-RsaOaep"),
        Some(OpcUaSecurityPolicy::Aes128Sha256RsaOaep)
    );
    assert_eq!(
        OpcUaMessageSecurityMode::parse("sign_and_encrypt"),
        Some(OpcUaMessageSecurityMode::SignAndEncrypt)
    );
    assert_eq!(
        OpcUaMessageSecurityMode::parse("none"),
        Some(OpcUaMessageSecurityMode::None)
    );
}

#[test]
fn rejects_invalid_security_profile_combinations() {
    let invalid = OpcUaSecurityProfile {
        policy: OpcUaSecurityPolicy::None,
        mode: OpcUaMessageSecurityMode::Sign,
        allow_anonymous: true,
    };
    assert!(validate_security_profile(&invalid).is_err());
}

#[test]
fn opcua_client_trust_store_can_be_listed_and_cleared() {
    let _guard = OPCUA_CLIENT_PKI_ENV_LOCK
        .lock()
        .expect("OPC UA client PKI env lock");
    let root = std::env::temp_dir().join(format!(
        "trust-opcua-client-pki-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let trusted = root.join("trusted").join("certs");
    std::fs::create_dir_all(&trusted).expect("create trusted cert dir");
    std::fs::write(trusted.join("server.der"), b"cert").expect("write trusted cert");
    std::env::set_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR", &root);

    let listed =
        list_trusted_opcua_client_server_certificates().expect("list trusted OPC UA certs");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].file_name, "server.der");

    let cleared =
        clear_trusted_opcua_client_server_certificates().expect("clear trusted OPC UA certs");
    assert_eq!(cleared, 1);
    assert!(list_trusted_opcua_client_server_certificates()
        .expect("list after clear")
        .is_empty());

    std::env::remove_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn opcua_client_explicit_trust_promotes_rejected_certificates() {
    let _guard = OPCUA_CLIENT_PKI_ENV_LOCK
        .lock()
        .expect("OPC UA client PKI env lock");
    let root = std::env::temp_dir().join(format!(
        "trust-opcua-client-pki-promote-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    let rejected = root.join("rejected");
    std::fs::create_dir_all(&rejected).expect("create rejected cert dir");
    std::fs::write(rejected.join("server.der"), b"cert").expect("write rejected cert");
    std::env::set_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR", &root);

    let promoted =
        promote_rejected_opcua_client_server_certificates().expect("promote rejected OPC UA cert");

    assert_eq!(promoted, 1);
    assert!(!rejected.join("server.der").exists());
    assert_eq!(
        std::fs::read(root.join("trusted").join("server.der")).expect("trusted cert"),
        b"cert"
    );

    std::env::remove_var("TRUST_RUNTIME_OPCUA_CLIENT_PKI_DIR");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn opcua_client_rejected_security_policy_during_login_prompts_for_auth() {
    let error = RuntimeError::ControlError(
        "OPC UA status: BadSecurityPolicyRejected"
            .to_string()
            .into(),
    );

    assert_eq!(
        classify_opcua_client_browse_error(&error),
        OpcUaClientErrorCode::AuthRequired
    );
}

#[test]
fn persistent_worker_applies_subscription_updates_without_reconnecting_per_scan() {
    let mut runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let connection = opcua_connection(vec![point.clone()]);
    let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
    let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect");
    assert_eq!(worker.transport().connect_count, 1);
    assert_eq!(worker.transport().subscribe_count, 1);

    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(22.5), 10);
    worker.tick(10).expect("drain update");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(0.0)),
        "worker callbacks must not mutate runtime storage mid-scan"
    );

    bridge
        .apply_inputs(runtime.storage_mut(), 11)
        .expect("apply input");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(22.5))
    );

    worker.tick(20).expect("second worker tick");
    bridge
        .apply_inputs(runtime.storage_mut(), 21)
        .expect("second scan");
    assert_eq!(
        worker.transport().connect_count,
        1,
        "scan-cycle reads must reuse the persistent session"
    );
}

#[test]
fn opcua_event_queue_rejects_saturation_and_recovers_after_drain() {
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let connection = opcua_connection(vec![point]);
    let runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let bindings = opcua_bindings(&runtime, &connection.points);
    let (_bridge, mut worker) = OpcUaClientBridge::with_transport(
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
    let accepted = (0..1024)
        .take_while(|index| sink.publish_connection_status(true, *index, "saturation probe"))
        .count();

    assert!(accepted > 0, "bounded queue must accept initial events");
    assert!(accepted < 1024, "bounded queue must reject saturation");
    assert!(
        !sink.publish_connection_status(true, 1024, "queue still saturated"),
        "saturated queue must fail immediately"
    );

    worker.tick(2_000).expect("drain queued events");
    assert!(
        sink.publish_connection_status(true, 2_001, "accepted after drain"),
        "draining must restore bounded handoff capacity"
    );
}

#[cfg(feature = "opcua-wire")]
#[test]
fn opcua_client_rejects_non_finite_subscription_values_before_storage() {
    let cases = [
        (
            "float_nan",
            OpcUaDataType::Float,
            ::opcua::client::prelude::Variant::Float(f32::NAN),
            Value::Real(7.25),
        ),
        (
            "float_positive_infinity",
            OpcUaDataType::Float,
            ::opcua::client::prelude::Variant::Float(f32::INFINITY),
            Value::Real(7.25),
        ),
        (
            "float_negative_infinity",
            OpcUaDataType::Float,
            ::opcua::client::prelude::Variant::Float(f32::NEG_INFINITY),
            Value::Real(7.25),
        ),
        (
            "double_nan",
            OpcUaDataType::Double,
            ::opcua::client::prelude::Variant::Double(f64::NAN),
            Value::LReal(7.25),
        ),
        (
            "double_positive_infinity",
            OpcUaDataType::Double,
            ::opcua::client::prelude::Variant::Double(f64::INFINITY),
            Value::LReal(7.25),
        ),
        (
            "double_negative_infinity",
            OpcUaDataType::Double,
            ::opcua::client::prelude::Variant::Double(f64::NEG_INFINITY),
            Value::LReal(7.25),
        ),
    ];

    for (case_name, data_type, variant, initial_value) in cases {
        let mut runtime = runtime_with_opcua_globals(vec![("line1_temp", initial_value.clone())]);
        let point = opcua_point(
            "line1_temp",
            "ns=2;i=2",
            data_type,
            OpcUaClientPointAccess::Read,
        );
        let connection = opcua_connection(vec![point.clone()]);
        let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
        let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
            connection,
            MockOpcUaClientTransport::default(),
            bindings,
        )
        .expect("bridge");

        worker.tick(0).expect("connect");
        worker.transport_mut().emit_wire_sample(&point, variant);
        worker.tick(10).expect("drain rejected update");
        bridge
            .apply_inputs(runtime.storage_mut(), 11)
            .expect("faulted point is skipped");

        assert_eq!(
            runtime.storage().get_global("line1_temp"),
            Some(&initial_value),
            "{case_name}: rejected input must leave PLC storage unchanged"
        );
        let snapshot = bridge.snapshot();
        assert!(
            !snapshot.values.contains_key("line1_temp"),
            "{case_name}: rejected input must not enter the accepted-value cache"
        );
        let status = snapshot
            .point_statuses
            .into_iter()
            .find(|status| status.var == "line1_temp")
            .expect("point status");
        assert_eq!(
            status.state,
            OpcUaClientConnectionState::Faulted,
            "{case_name}"
        );
        assert_eq!(status.value, None, "{case_name}");
        assert!(
            status.detail.contains("non-finite"),
            "{case_name}: unexpected detail: {}",
            status.detail
        );
    }
}

#[test]
fn persistent_worker_batches_writes_without_reconnecting_per_write() {
    let mut runtime = runtime_with_opcua_globals(vec![("line1_setpoint", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_setpoint",
        "ns=2;i=3",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::ReadWrite,
    );
    let connection = opcua_connection(vec![point.clone()]);
    let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
    let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect");
    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(10.0), 10);
    worker.tick(10).expect("drain baseline");
    bridge
        .apply_inputs(runtime.storage_mut(), 11)
        .expect("apply baseline");

    write_global(&mut runtime, "line1_setpoint", Value::Real(31.0));
    bridge
        .capture_outputs(runtime.storage_mut(), 12)
        .expect("queue first write");
    assert_eq!(
        bridge.pending_write("line1_setpoint"),
        Some(Value::Real(31.0))
    );
    worker.tick(12).expect("publish first write");
    assert_eq!(bridge.pending_write("line1_setpoint"), None);
    assert_eq!(worker.transport().write_batches.len(), 1);
    assert_eq!(
        worker.transport().write_batches[0],
        vec![("line1_setpoint".into(), Value::Real(31.0))]
    );

    write_global(&mut runtime, "line1_setpoint", Value::Real(44.0));
    bridge
        .capture_outputs(runtime.storage_mut(), 13)
        .expect("queue second write");
    worker.tick(13).expect("publish second write");

    assert_eq!(worker.transport().connect_count, 1);
    assert_eq!(worker.transport().write_batches.len(), 2);
}

#[test]
fn persistent_worker_rejected_write_marks_point_without_reconnecting() {
    let mut runtime = runtime_with_opcua_globals(vec![("line1_setpoint", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_setpoint",
        "ns=2;i=3",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::ReadWrite,
    );
    let connection = opcua_connection(vec![point.clone()]);
    let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
    let transport = MockOpcUaClientTransport {
        write_error: Some(OpcUaClientBridgeError::validation(
            "OPC UA node 'ns=2;i=3' write returned BadUserAccessDenied",
        )),
        ..Default::default()
    };
    let (mut bridge, mut worker) =
        OpcUaClientBridge::with_transport(connection, transport, bindings).expect("bridge");

    worker.tick(0).expect("connect");
    write_global(&mut runtime, "line1_setpoint", Value::Real(31.0));
    bridge
        .capture_outputs(runtime.storage_mut(), 12)
        .expect("queue write");
    assert_eq!(
        bridge.pending_write("line1_setpoint"),
        Some(Value::Real(31.0))
    );

    worker
        .tick(12)
        .expect("write rejection handled as point quality");

    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    assert_eq!(bridge.pending_write("line1_setpoint"), None);
    assert_eq!(worker.transport().connect_count, 1);
    assert_eq!(worker.transport().disconnect_count, 0);
    let status = bridge
        .snapshot()
        .point_statuses
        .into_iter()
        .find(|status| status.var == "line1_setpoint")
        .expect("point status");
    assert_eq!(status.state, OpcUaClientConnectionState::Faulted);
    assert!(
        status.detail.contains("BadUserAccessDenied"),
        "unexpected status detail: {}",
        status.detail
    );
}

#[test]
fn persistent_worker_marks_stale_then_recovers_on_subscription_update() {
    let mut runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let mut connection = opcua_connection(vec![point.clone()]);
    connection.timeout_ms = 50;
    let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
    let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("connect");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);

    worker.tick(60).expect("stale timeout");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Stale);

    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(12.0), 61);
    worker.tick(61).expect("recover from subscription update");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    bridge
        .apply_inputs(runtime.storage_mut(), 62)
        .expect("apply recovered value");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(12.0))
    );
}

#[test]
fn persistent_worker_reconnects_after_session_loss_without_scan_thread_io() {
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

    worker.tick(0).expect("connect");
    worker
        .transport_mut()
        .emit_connection_status(false, 100, "session lost");
    worker.tick(100).expect("drain disconnect");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Reconnecting);
    assert_eq!(worker.transport().connect_count, 1);

    worker.tick(1_000).expect("backoff waits");
    assert_eq!(worker.transport().connect_count, 1);

    worker.tick(2_101).expect("reconnect after backoff");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    assert_eq!(worker.transport().connect_count, 2);
}

#[test]
fn persistent_worker_recreates_subscription_after_server_restart() {
    let mut runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let connection = opcua_connection(vec![point.clone()]);
    let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
    let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::default(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("initial connect");
    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(18.0), 10);
    worker.tick(10).expect("initial subscription update");
    bridge
        .apply_inputs(runtime.storage_mut(), 11)
        .expect("apply initial value");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(18.0))
    );

    worker
        .transport_mut()
        .emit_connection_status(false, 100, "server restart");
    worker.tick(100).expect("detect restart");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Reconnecting);
    assert_eq!(worker.transport().subscribe_count, 1);

    worker.tick(2_101).expect("reconnect after restart");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    assert_eq!(worker.transport().connect_count, 2);
    assert_eq!(
        worker.transport().subscribe_count,
        2,
        "read subscriptions must be recreated after reconnect"
    );

    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(19.0), 2_110);
    worker
        .tick(2_110)
        .expect("post-restart subscription update");
    bridge
        .apply_inputs(runtime.storage_mut(), 2_111)
        .expect("apply post-restart value");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(19.0))
    );
}

#[test]
fn persistent_worker_uses_recovery_hook_to_reestablish_subscriptions() {
    let mut runtime = runtime_with_opcua_globals(vec![("line1_temp", Value::Real(0.0))]);
    let point = opcua_point(
        "line1_temp",
        "ns=2;i=2",
        OpcUaDataType::Float,
        OpcUaClientPointAccess::Read,
    );
    let connection = opcua_connection(vec![point.clone()]);
    let bindings = opcua_bindings(&runtime, std::slice::from_ref(&point));
    let (mut bridge, mut worker) = OpcUaClientBridge::with_transport(
        connection,
        MockOpcUaClientTransport::recovering(),
        bindings,
    )
    .expect("bridge");

    worker.tick(0).expect("initial connect");
    worker
        .transport_mut()
        .emit_connection_status(false, 100, "server restart");
    worker.tick(100).expect("detect restart");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Reconnecting);

    worker.tick(2_101).expect("recover subscriptions");
    assert_eq!(bridge.state(), OpcUaClientConnectionState::Connected);
    assert_eq!(
        worker.transport().connect_count,
        1,
        "recovery hook should avoid a new session when transport can recover"
    );
    assert_eq!(worker.transport().subscribe_count, 1);
    assert_eq!(worker.transport().recover_count, 1);
    assert!(
        bridge
            .snapshot()
            .detail
            .contains("transferred subscriptions with send_initial_values=true"),
        "{}",
        bridge.snapshot().detail
    );

    worker
        .transport_mut()
        .emit_sample(&point, Value::Real(21.0), 2_110);
    worker
        .tick(2_110)
        .expect("post-recovery subscription update");
    bridge
        .apply_inputs(runtime.storage_mut(), 2_111)
        .expect("apply post-recovery value");
    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(21.0))
    );
}

#[test]
fn connected_detail_reports_timeout_negotiation_or_documented_gap() {
    let negotiated = session_detail(2, 100, Some(250), None);
    assert!(
        negotiated.contains("requested stale timeout 100 ms"),
        "{negotiated}"
    );
    assert!(
        negotiated.contains("revised session timeout 250 ms"),
        "{negotiated}"
    );

    let hidden = session_detail(2, 100, None, None);
    assert!(
        hidden.contains("revised session timeout is not exposed by opcua 0.12"),
        "{hidden}"
    );
    assert!(
        hidden.contains("using configured stale timeout 100 ms"),
        "{hidden}"
    );
}

#[derive(Default)]
struct MockOpcUaClientTransport {
    connect_count: usize,
    subscribe_count: usize,
    recover_count: usize,
    recover_succeeds: bool,
    disconnect_count: usize,
    write_batches: Vec<Vec<(SmolStr, Value)>>,
    write_error: Option<OpcUaClientBridgeError>,
    sink: Option<OpcUaClientEventSink>,
}

impl MockOpcUaClientTransport {
    fn recovering() -> Self {
        Self {
            recover_succeeds: true,
            ..Self::default()
        }
    }

    fn emit_sample(&mut self, point: &OpcUaClientPointConfig, value: Value, now_ms: u64) {
        let sink = self.sink.as_ref().expect("event sink");
        assert!(sink.publish_sample(OpcUaClientSample {
            var: point.var.clone(),
            node_id: point.node_id.clone(),
            data_type: point.data_type,
            access: point.access,
            value: Some(value),
            state: OpcUaClientConnectionState::Connected,
            last_seen_ms: Some(now_ms),
            detail: "mock subscription update".to_string(),
        }));
    }

    #[cfg(feature = "opcua-wire")]
    fn emit_wire_sample(
        &mut self,
        point: &OpcUaClientPointConfig,
        value: ::opcua::client::prelude::Variant,
    ) {
        let sink = self.sink.as_ref().expect("event sink");
        let sample = sample_from_data_value(
            point,
            &::opcua::client::prelude::DataValue {
                value: Some(value),
                status: Some(::opcua::client::prelude::StatusCode::Good),
                ..Default::default()
            },
        );
        assert!(sink.publish_sample(sample));
    }

    fn emit_connection_status(&mut self, connected: bool, now_ms: u64, detail: &str) {
        let sink = self.sink.as_ref().expect("event sink");
        assert!(sink.publish_connection_status(connected, now_ms, detail));
    }
}

impl OpcUaClientTransport for MockOpcUaClientTransport {
    fn connect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        sink: OpcUaClientEventSink,
    ) -> Result<OpcUaClientSessionInfo, OpcUaClientBridgeError> {
        self.connect_count += 1;
        self.sink = Some(sink);
        Ok(OpcUaClientSessionInfo {
            requested_timeout_ms: connection.timeout_ms,
            revised_timeout_ms: Some(connection.timeout_ms),
            recovery_detail: None,
        })
    }

    fn subscribe_read_points(
        &mut self,
        points: &[OpcUaClientPointConfig],
        _sink: OpcUaClientEventSink,
    ) -> Result<(), OpcUaClientBridgeError> {
        if !points.is_empty() {
            self.subscribe_count += 1;
        }
        Ok(())
    }

    fn write_values(
        &mut self,
        values: &[(OpcUaClientPointConfig, Value)],
    ) -> Result<(), OpcUaClientBridgeError> {
        if let Some(error) = self.write_error.clone() {
            return Err(error);
        }
        self.write_batches.push(
            values
                .iter()
                .map(|(point, value)| (point.var.clone(), value.clone()))
                .collect(),
        );
        Ok(())
    }

    fn recover_after_disconnect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        _read_points: &[OpcUaClientPointConfig],
        sink: OpcUaClientEventSink,
    ) -> Result<Option<OpcUaClientSessionInfo>, OpcUaClientBridgeError> {
        self.recover_count += 1;
        if !self.recover_succeeds {
            return Ok(None);
        }
        self.sink = Some(sink);
        Ok(Some(OpcUaClientSessionInfo {
            requested_timeout_ms: connection.timeout_ms,
            revised_timeout_ms: Some(connection.timeout_ms),
            recovery_detail: Some(
                "mock reconnect transferred subscriptions with send_initial_values=true and replayed available notifications",
            ),
        }))
    }

    fn disconnect(&mut self) -> Result<(), OpcUaClientBridgeError> {
        self.disconnect_count += 1;
        Ok(())
    }
}

fn runtime_with_opcua_globals(globals: Vec<(&str, Value)>) -> crate::Runtime {
    let mut runtime = crate::Runtime::new();
    for (name, value) in globals {
        runtime.storage_mut().set_global(SmolStr::new(name), value);
    }
    runtime
}

fn opcua_connection(points: Vec<OpcUaClientPointConfig>) -> OpcUaClientConnectionConfig {
    OpcUaClientConnectionConfig {
        name: SmolStr::new("line1"),
        endpoint_url: "opc.tcp://127.0.0.1:4840/trust".to_string(),
        security: OpcUaSecurityProfile {
            policy: OpcUaSecurityPolicy::None,
            mode: OpcUaMessageSecurityMode::None,
            allow_anonymous: true,
        },
        auth: OpcUaClientAuthConfig::Anonymous,
        trust_server_certificate: true,
        poll_interval_ms: 10,
        timeout_ms: 100,
        points,
    }
}

fn opcua_point(
    var: &str,
    node_id: &str,
    data_type: OpcUaDataType,
    access: OpcUaClientPointAccess,
) -> OpcUaClientPointConfig {
    OpcUaClientPointConfig {
        var: SmolStr::new(var),
        node_id: node_id.to_string(),
        data_type,
        access,
    }
}

fn opcua_bindings(
    runtime: &crate::Runtime,
    points: &[OpcUaClientPointConfig],
) -> Vec<OpcUaClientBinding> {
    points
        .iter()
        .map(|point| OpcUaClientBinding {
            point: point.clone(),
            reference: runtime
                .storage()
                .ref_for_global(point.var.as_str())
                .expect("global ref"),
        })
        .collect()
}

fn write_global(runtime: &mut crate::Runtime, name: &str, value: Value) {
    let reference = runtime.storage().ref_for_global(name).expect("global ref");
    assert!(runtime.storage_mut().write_by_ref(reference, value));
}
