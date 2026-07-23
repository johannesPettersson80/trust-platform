use std::io::{Read, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::mpsc::channel;
use std::sync::Arc;
use std::time::Duration as StdDuration;

use smol_str::SmolStr;
use trust_ads_core::{ads_bytes_from_value, AmsNetId, IecDataType, QualityState, SymbolFlag};
use trust_ads_server::{
    ams_net_id_text_to_bytes, AdsErrorCode, AdsServerAuditEvent, AdsServerAuditKind,
    AdsServerError, AmsHeader, AmsState, AmsTcpFrame, AuditSink, ClientId, CommandId,
    RuntimeWritePort, SymbolSource, ValueIo, ADSIGRP_SYM_VERSION,
};

use crate::ads::diagnostics::{DoctorRole, DoctorStepId, DoctorStepStatus};
use crate::debug::{DebugControl, PendingVarTarget};
use crate::error::RuntimeError;
use crate::memory::VariableStorage;
use crate::scheduler::{ResourceControl, ResourceState, StdClock};
use crate::value::{ArrayValue, Duration, Value};

use super::{
    build_ads_server_status_report, build_runtime_symbol_snapshot, descriptor_for_value,
    run_ads_server_doctor, AdsServerClientConfig, AdsServerClientPolicy, AdsServerDoctorInput,
    AdsServerExternalClientEvidence, AdsServerRuntimeAuditSink, AdsServerRuntimeConfig,
    AdsServerRuntimeWritePort, AdsServerSourcePin, AdsServerSymbolSource, AdsServerValuePublisher,
};

mod policy_write_audit;
mod publication_lifecycle;

fn config(writes_enabled: bool) -> AdsServerRuntimeConfig {
    AdsServerRuntimeConfig {
        enabled: true,
        listen: Some(SmolStr::new("192.168.10.20")),
        ads_port: 851,
        ams_net_id: Some(AmsNetId::new("192.168.10.20.1.1")),
        insecure_transport: true,
        writes_enabled,
        expose: vec![SmolStr::new("global.*")],
        writable: vec![SmolStr::new("global.setpoint")],
        clients: vec![AdsServerClientConfig {
            ams_net_id: AmsNetId::new("5.23.91.12.1.1"),
            source: AdsServerSourcePin::Cidr(SmolStr::new("192.168.10.0/24")),
        }],
        ..AdsServerRuntimeConfig::default()
    }
}

fn snapshot(
    values: impl IntoIterator<Item = (&'static str, Value)>,
) -> crate::debug::DebugSnapshot {
    let mut storage = VariableStorage::new();
    for (name, value) in values {
        storage.set_global(name, value);
    }
    crate::debug::DebugSnapshot {
        storage,
        now: Duration::from_millis(10),
    }
}

fn allowed_client() -> ClientId {
    ClientId::new(AmsNetId::new("5.23.91.12.1.1")).with_source_ip("192.168.10.50")
}

fn resource_control() -> ResourceControl<StdClock> {
    let (resource, _rx) = ResourceControl::stub(StdClock::new());
    resource
}

#[test]
fn runtime_symbols_expose_configured_globals_and_writable_flags() {
    let config = config(true);
    let snapshot = snapshot([
        ("line_ready", Value::Bool(true)),
        ("setpoint", Value::Real(12.5)),
        ("hidden", Value::DInt(4)),
    ]);

    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");

    assert_eq!(symbols.symbols.len(), 3);
    let setpoint = symbols
        .symbols
        .iter()
        .find(|symbol| symbol.name == "global.setpoint")
        .expect("setpoint symbol");
    assert_eq!(setpoint.data_type.iec_type, IecDataType::Real);
    assert!(setpoint.flags.contains(&SymbolFlag::Read));
    assert!(setpoint.flags.contains(&SymbolFlag::Write));

    let ready = symbols
        .symbols
        .iter()
        .find(|symbol| symbol.name == "global.line_ready")
        .expect("ready symbol");
    assert!(ready.flags.contains(&SymbolFlag::Read));
    assert!(!ready.flags.contains(&SymbolFlag::Write));
}

#[test]
fn runtime_symbols_skip_unsupported_values() {
    let config = config(true);
    let snapshot = snapshot([("ok", Value::Bool(true)), ("unsupported", Value::Null)]);

    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");

    assert_eq!(
        symbols
            .symbols
            .iter()
            .map(|symbol| symbol.name.as_str())
            .collect::<Vec<_>>(),
        vec!["global.ok"]
    );
}

#[test]
fn runtime_symbol_source_refresh_bumps_version_and_swaps_snapshot() {
    let config = config(true);
    let first = snapshot([("setpoint", Value::Real(0.0))]);
    let second = snapshot([
        ("setpoint", Value::Real(0.0)),
        ("line_ready", Value::Bool(true)),
    ]);
    let source =
        AdsServerSymbolSource::from_runtime_snapshot(&config, &first).expect("initial symbols");

    assert_eq!(source.version(), 1);
    assert_eq!(source.snapshot().symbols.len(), 1);

    let refreshed = source
        .refresh_from_runtime_snapshot(&config, &second)
        .expect("refresh symbols");

    assert_eq!(refreshed, 2);
    assert_eq!(source.version(), 2);
    assert_eq!(source.snapshot().symbols.len(), 2);
    assert!(source
        .snapshot()
        .symbols
        .iter()
        .any(|symbol| symbol.name == "global.line_ready"));
}

#[test]
fn descriptor_for_array_preserves_bounds_and_scalar_type() {
    let value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1), Value::Int(2), Value::Int(3)],
        vec![(1, 3)],
    )));

    let descriptor = descriptor_for_value(&value, 128).expect("array descriptor");

    assert_eq!(descriptor.iec_type, IecDataType::Int);
    assert_eq!(descriptor.dimensions.len(), 1);
    assert_eq!(descriptor.dimensions[0].lower, 1);
    assert_eq!(descriptor.dimensions[0].upper, 3);
}

#[test]
fn publisher_reads_snapshot_and_marks_good_quality() {
    let config = config(true);
    let snapshot = snapshot([("setpoint", Value::Real(12.5))]);
    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");
    let symbol = symbols.symbols.first().expect("symbol").clone();
    let publisher = AdsServerValuePublisher::new_with_timed_provider(
        config,
        Arc::new(move || {
            Some(super::publisher::TimedDebugSnapshot::captured_at(
                snapshot.clone(),
                1_000,
            ))
        }),
        Arc::new(|| 1_050),
    );

    let (bytes, quality) = publisher.read(&symbol).expect("read");

    assert_eq!(bytes, 12.5_f32.to_le_bytes());
    assert_eq!(quality.state, QualityState::Good);
    assert_eq!(quality.last_update_ms, Some(1_000));
}

#[test]
fn publisher_marks_old_snapshot_quality_stale() {
    let mut config = config(true);
    config.read_timeout_ms = 50;
    let snapshot = snapshot([("setpoint", Value::Real(12.5))]);
    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");
    let symbol = symbols.symbols.first().expect("symbol").clone();
    let publisher = AdsServerValuePublisher::new_with_timed_provider(
        config,
        Arc::new(move || {
            Some(super::publisher::TimedDebugSnapshot::captured_at(
                snapshot.clone(),
                1_000,
            ))
        }),
        Arc::new(|| 1_101),
    );

    let (_bytes, quality) = publisher.read(&symbol).expect("read");

    assert_eq!(quality.state, QualityState::Stale);
    assert_eq!(quality.last_update_ms, Some(1_000));
    assert!(quality
        .detail
        .as_deref()
        .is_some_and(|detail| detail.contains("101 ms old")));
}

#[test]
fn publisher_rejects_missing_snapshot_without_touching_scan_thread() {
    let config = config(true);
    let snapshot = snapshot([("setpoint", Value::Real(12.5))]);
    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");
    let symbol = symbols.symbols.first().expect("symbol").clone();
    let publisher = AdsServerValuePublisher::new(config, Arc::new(|| None));

    let error = publisher.read(&symbol).expect_err("missing snapshot");

    assert_eq!(error.code(), AdsErrorCode::NotReady);
}

#[test]
fn client_policy_requires_ams_net_id_and_source_pin() {
    let policy = AdsServerClientPolicy::new(&config(true));

    assert!(policy.permits_client(&allowed_client()));
    assert!(!policy.permits_client(
        &ClientId::new(AmsNetId::new("5.23.91.12.1.1")).with_source_ip("10.0.0.5")
    ));
    assert!(!policy.permits_client(
        &ClientId::new(AmsNetId::new("1.2.3.4.5.6")).with_source_ip("192.168.10.50")
    ));
}

#[test]
fn write_port_accepts_guarded_write_and_coalesces_same_target() {
    let config = config(true);
    let snapshot = snapshot([("setpoint", Value::Real(0.0))]);
    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");
    let symbol = symbols.symbols.first().expect("symbol").clone();
    let policy = AdsServerClientPolicy::new(&config);
    let debug = DebugControl::new();
    let write_port =
        AdsServerRuntimeWritePort::new(config, policy, debug.clone(), resource_control())
            .expect("write port");

    write_port
        .write(
            &symbol,
            &ads_bytes_from_value(&symbol.data_type, &Value::Real(1.0)).unwrap(),
            &allowed_client(),
        )
        .expect("first write");
    write_port
        .write(
            &symbol,
            &ads_bytes_from_value(&symbol.data_type, &Value::Real(2.5)).unwrap(),
            &allowed_client(),
        )
        .expect("second write");

    let pending = debug.drain_var_writes();
    assert_eq!(pending.len(), 1);
    assert_eq!(
        pending[0].target,
        PendingVarTarget::Global(SmolStr::new("setpoint"))
    );
    assert_eq!(pending[0].value, Value::Real(2.5));
}

#[test]
fn write_port_rejects_raw_non_finite_real_and_lreal_without_mutation() {
    let cases = [
        (
            "REAL NaN",
            Value::Real(0.0),
            f32::NAN.to_le_bytes().to_vec(),
        ),
        (
            "REAL positive infinity",
            Value::Real(0.0),
            f32::INFINITY.to_le_bytes().to_vec(),
        ),
        (
            "REAL negative infinity",
            Value::Real(0.0),
            f32::NEG_INFINITY.to_le_bytes().to_vec(),
        ),
        (
            "LREAL NaN",
            Value::LReal(0.0),
            f64::NAN.to_le_bytes().to_vec(),
        ),
        (
            "LREAL positive infinity",
            Value::LReal(0.0),
            f64::INFINITY.to_le_bytes().to_vec(),
        ),
        (
            "LREAL negative infinity",
            Value::LReal(0.0),
            f64::NEG_INFINITY.to_le_bytes().to_vec(),
        ),
    ];

    for (case, initial, raw_bytes) in cases {
        let config = config(true);
        let symbols = build_runtime_symbol_snapshot(&config, &snapshot([("setpoint", initial)]))
            .expect("symbol snapshot");
        let symbol = symbols.symbols.first().expect("symbol").clone();
        let policy = AdsServerClientPolicy::new(&config);
        let debug = DebugControl::new();
        let write_port =
            AdsServerRuntimeWritePort::new(config, policy, debug.clone(), resource_control())
                .expect("write port");

        let error = write_port
            .write(&symbol, &raw_bytes, &allowed_client())
            .expect_err(case);

        assert_eq!(error.code(), AdsErrorCode::InvalidData, "{case}");
        assert!(
            debug.drain_var_writes().is_empty(),
            "{case} must not queue a PLC mutation"
        );
    }
}

#[test]
fn write_port_rejects_policy_failure_without_mutation() {
    let config = config(true);
    let snapshot = snapshot([("setpoint", Value::Real(0.0))]);
    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");
    let symbol = symbols.symbols.first().expect("symbol").clone();
    let policy = AdsServerClientPolicy::new(&config);
    let debug = DebugControl::new();
    let write_port =
        AdsServerRuntimeWritePort::new(config, policy, debug.clone(), resource_control())
            .expect("write port");

    let error = write_port
        .write(
            &symbol,
            &ads_bytes_from_value(&symbol.data_type, &Value::Real(1.0)).unwrap(),
            &ClientId::new(AmsNetId::new("5.23.91.12.1.1")).with_source_ip("10.0.0.5"),
        )
        .expect_err("source IP rejected");

    assert_eq!(error.code(), AdsErrorCode::AccessDenied);
    assert!(debug.drain_var_writes().is_empty());
}

#[test]
fn write_port_rejects_read_only_gate_without_mutation() {
    let config = config(false);
    let snapshot = snapshot([("setpoint", Value::Real(0.0))]);
    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");
    let symbol = symbols.symbols.first().expect("symbol").clone();
    let policy = AdsServerClientPolicy::new(&config);
    let debug = DebugControl::new();
    let write_port =
        AdsServerRuntimeWritePort::new(config, policy, debug.clone(), resource_control())
            .expect("write port");

    let error = write_port
        .write(
            &symbol,
            &ads_bytes_from_value(&symbol.data_type, &Value::Real(1.0)).unwrap(),
            &allowed_client(),
        )
        .expect_err("writes disabled");

    assert_eq!(error.code(), AdsErrorCode::AccessDenied);
    assert!(debug.drain_var_writes().is_empty());
}

#[test]
fn write_port_rejects_faulted_runtime_without_mutation() {
    let config = config(true);
    let snapshot = snapshot([("setpoint", Value::Real(0.0))]);
    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).expect("symbol snapshot");
    let symbol = symbols.symbols.first().expect("symbol").clone();
    let policy = AdsServerClientPolicy::new(&config);
    let debug = DebugControl::new();
    let (resource, _rx) = ResourceControl::stub_with_state(
        StdClock::new(),
        ResourceState::Faulted,
        Some(RuntimeError::ResourceFaulted),
    );
    let write_port = AdsServerRuntimeWritePort::new(config, policy, debug.clone(), resource)
        .expect("write port");

    let error = write_port
        .write(
            &symbol,
            &ads_bytes_from_value(&symbol.data_type, &Value::Real(1.0)).unwrap(),
            &allowed_client(),
        )
        .expect_err("faulted runtime rejected");

    assert_eq!(error.code(), AdsErrorCode::NotReady);
    assert!(debug.drain_var_writes().is_empty());
}

#[test]
fn audit_sink_records_accepted_and_rejected_ads_write_details() {
    let (tx, rx) = channel();
    let sink = AdsServerRuntimeAuditSink::new(Some(tx));
    let client = allowed_client();

    sink.record(
        &AdsServerAuditEvent::new(
            AdsServerAuditKind::WriteAccepted,
            client.clone(),
            Some("global.setpoint".to_string()),
            Ok(()),
            42,
        )
        .with_value_type("REAL"),
    );
    sink.record(
        &AdsServerAuditEvent::new(
            AdsServerAuditKind::WriteRejected,
            client,
            Some("global.setpoint".to_string()),
            Err(AdsServerError::device(
                AdsErrorCode::AccessDenied,
                "not writable",
            )),
            43,
        )
        .with_value_type("REAL"),
    );

    let accepted = rx.recv().expect("accepted audit");
    assert_eq!(accepted.request_type.as_str(), "ads.server.write");
    assert!(accepted.ok);
    let details = accepted.details.expect("accepted details");
    assert_eq!(details["symbol"], "global.setpoint");
    assert_eq!(details["value_type"], "REAL");
    assert_eq!(details["client_ams_net_id"], "5.23.91.12.1.1");

    let rejected = rx.recv().expect("rejected audit");
    assert_eq!(rejected.request_type.as_str(), "ads.server.write");
    assert!(!rejected.ok);
    assert!(rejected
        .error
        .as_ref()
        .expect("rejection error")
        .contains("not writable"));
    assert_eq!(
        rejected.details.expect("rejected details")["ads_error"]["code"],
        0x0723
    );
}

#[test]
fn lifecycle_starts_tcp_listener_from_runtime_integration() {
    let mut config = config(false);
    config.listen = Some(SmolStr::new("127.0.0.1"));
    config.ams_net_id = Some(AmsNetId::new("127.0.0.1.1.1"));
    config.clients = vec![AdsServerClientConfig {
        ams_net_id: AmsNetId::new("5.23.91.12.1.1"),
        source: AdsServerSourcePin::Cidr(SmolStr::new("127.0.0.0/8")),
    }];
    let snapshot = snapshot([("setpoint", Value::Real(0.0))]);
    let debug = DebugControl::new();
    let snapshot_provider = Arc::new(move || Some(snapshot.clone()));

    let mut server = super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        debug,
        resource_control(),
        snapshot_provider,
        None,
        0,
    )
    .expect("start ADS server")
    .expect("server enabled");

    let response = send_frame(
        server.local_addr(),
        &request_frame(CommandId::ReadDeviceInfo, Vec::new()),
    );

    assert_eq!(response.header.command_id, CommandId::ReadDeviceInfo);
    assert_eq!(response.header.error_code, 0);
    assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
    assert_eq!(response.payload[8..16], padded_name("RESOURCE"));

    let mut denied = request_frame(CommandId::ReadDeviceInfo, Vec::new());
    denied.header.source_net_id =
        ams_net_id_text_to_bytes("1.2.3.4.5.6").expect("denied source net id");
    let denied_response = send_frame(server.local_addr(), &denied);
    assert_eq!(
        u32::from_le_bytes(denied_response.payload[0..4].try_into().expect("result")),
        AdsErrorCode::AccessDenied.value()
    );
    let refused = server.policy().recently_refused_clients();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].ams_net_id, "1.2.3.4.5.6");
    assert_eq!(refused[0].source_ip.as_deref(), Some("127.0.0.1"));
    assert_eq!(refused[0].reason, "ams_net_id_not_allowlisted");

    assert!(
        server.identify_bind_addr().ip().is_unspecified(),
        "ADS UDP identify must bind wildcard so subnet broadcast search is delivered"
    );
    assert_eq!(server.identify_addr().ip().to_string(), "127.0.0.1");
    let identify = query_identify(server.identify_addr());
    assert_eq!(&identify[12..18], &[127, 0, 0, 1, 1, 1]);
    server.shutdown();
}

#[test]
fn lifecycle_refreshes_symbols_without_rebinding_ads_socket() {
    let mut config = config(false);
    config.listen = Some(SmolStr::new("127.0.0.1"));
    config.ams_net_id = Some(AmsNetId::new("127.0.0.1.1.1"));
    config.clients = vec![AdsServerClientConfig {
        ams_net_id: AmsNetId::new("5.23.91.12.1.1"),
        source: AdsServerSourcePin::Cidr(SmolStr::new("127.0.0.0/8")),
    }];
    let first = snapshot([("setpoint", Value::Real(0.0))]);
    let second = snapshot([
        ("setpoint", Value::Real(0.0)),
        ("line_ready", Value::Bool(true)),
    ]);
    let debug = DebugControl::new();
    let snapshot_provider = Arc::new({
        let first = first.clone();
        move || Some(first.clone())
    });

    let mut server = super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        debug,
        resource_control(),
        snapshot_provider,
        None,
        0,
    )
    .expect("start ADS server")
    .expect("server enabled");
    let addr = server.local_addr();

    assert_eq!(server.symbol_count(), 1);
    assert_eq!(server.symbol_version(), 1);

    let version = server
        .refresh_symbols(&config, &second)
        .expect("refresh symbols");

    assert_eq!(version, 2);
    assert_eq!(server.local_addr(), addr);
    assert_eq!(server.symbol_count(), 2);
    assert_eq!(server.symbol_version(), 2);

    let mut payload = Vec::new();
    payload.extend_from_slice(&ADSIGRP_SYM_VERSION.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&4_u32.to_le_bytes());
    let response = send_frame(addr, &request_frame(CommandId::Read, payload));
    assert_eq!(response.header.command_id, CommandId::Read);
    assert_eq!(response.header.error_code, 0);
    assert_eq!(response.payload[0..4], 0_u32.to_le_bytes());
    assert_eq!(response.payload[4..8], 4_u32.to_le_bytes());
    assert_eq!(response.payload[8..12], 2_u32.to_le_bytes());

    server.shutdown();
}

#[test]
fn lifecycle_starts_not_ready_without_snapshot_and_refreshes_when_snapshot_appears() {
    let mut config = config(false);
    config.listen = Some(SmolStr::new("127.0.0.1"));
    config.ams_net_id = Some(AmsNetId::new("127.0.0.1.1.1"));
    config.clients = vec![AdsServerClientConfig {
        ams_net_id: AmsNetId::new("5.23.91.12.1.1"),
        source: AdsServerSourcePin::Cidr(SmolStr::new("127.0.0.0/8")),
    }];

    let mut server = super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        DebugControl::new(),
        resource_control(),
        Arc::new(|| None),
        None,
        0,
    )
    .expect("missing snapshot should degrade, not fail")
    .expect("server enabled");

    assert_eq!(server.symbol_count(), 0);
    let not_ready = build_ads_server_status_report(&config, None, Some(&server));
    assert_eq!(
        not_ready.overall,
        crate::ads::diagnostics::AdsStatusOverall::NotReady
    );
    assert_eq!(
        not_ready.connections[0].state,
        crate::ads::diagnostics::AdsConnectionStatusState::NotReady
    );
    assert!(not_ready.summary.contains("not ready"));

    let next = snapshot([("setpoint", Value::Real(0.0))]);
    server
        .refresh_symbols(&config, &next)
        .expect("refresh symbols after snapshot appears");
    let symbols = build_runtime_symbol_snapshot(&config, &next).expect("symbol snapshot");
    let ready = build_ads_server_status_report(&config, Some(&symbols), Some(&server));

    assert_eq!(server.symbol_count(), 1);
    assert_eq!(
        ready.overall,
        crate::ads::diagnostics::AdsStatusOverall::Healthy
    );
    assert_eq!(
        ready.connections[0].state,
        crate::ads::diagnostics::AdsConnectionStatusState::Connected
    );
    server.shutdown();
}

#[test]
fn temporary_self_test_policy_is_removed() {
    let policy = AdsServerClientPolicy::new(&config(false));
    let client = ClientId::new(AmsNetId::new("127.0.0.1.1.2")).with_source_ip("127.0.0.1");

    {
        let _permit = policy.permit_temporarily(AmsNetId::new("127.0.0.1.1.2"), "127.0.0.1");
        assert!(policy.permits_client(&client));
    }

    assert!(!policy.permits_client(&client));
}

#[test]
fn client_policy_records_refused_attempts_for_wait_for_client_flow() {
    let policy = AdsServerClientPolicy::new(&config(false));
    let client = ClientId::new(AmsNetId::new("5.23.91.12.1.1")).with_source_ip("10.0.0.5");

    assert!(!policy.permits_client(&client));
    assert!(!policy.permits_client(&client));

    let refused = policy.recently_refused_clients();
    assert_eq!(refused.len(), 1);
    assert_eq!(refused[0].ams_net_id, "5.23.91.12.1.1");
    assert_eq!(refused[0].source_ip.as_deref(), Some("10.0.0.5"));
    assert_eq!(refused[0].reason, "source_ip_not_allowed");
    assert_eq!(refused[0].count, 2);
    assert_eq!(refused[0].suggested_client.ams_net_id, "5.23.91.12.1.1");
    assert_eq!(
        refused[0].suggested_client.source_ip.as_deref(),
        Some("10.0.0.5")
    );
}

#[test]
fn server_doctor_self_test_does_not_grant_production_ready() {
    let mut config = config(true);
    config.listen = Some(SmolStr::new("127.0.0.1"));
    config.ams_net_id = Some(AmsNetId::new("127.0.0.1.1.1"));
    config.clients = vec![AdsServerClientConfig {
        ams_net_id: AmsNetId::new("5.23.91.12.1.1"),
        source: AdsServerSourcePin::Cidr(SmolStr::new("127.0.0.0/8")),
    }];
    let snapshot = snapshot([("setpoint", Value::Real(3.5))]);
    let debug = DebugControl::new();
    let snapshot_provider = Arc::new({
        let snapshot = snapshot.clone();
        move || Some(snapshot.clone())
    });
    let mut server = super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        debug,
        resource_control(),
        snapshot_provider,
        None,
        0,
    )
    .expect("start ADS server")
    .expect("server enabled");

    let report = run_ads_server_doctor(AdsServerDoctorInput {
        resource_name: "RESOURCE",
        config: &config,
        snapshot: &snapshot,
        runtime: Some(&server),
        now_ms: 100,
        external_client: None,
        deployed_config_text: Some("ads-server-test-config"),
    });

    assert_eq!(report.role, DoctorRole::Server);
    assert!(!report.production_ready);
    assert_eq!(
        report
            .steps
            .iter()
            .find(|step| step.id == DoctorStepId::SelfSumupRead)
            .expect("self sum-up step")
            .status,
        DoctorStepStatus::Pass
    );
    assert_eq!(
        report
            .steps
            .iter()
            .find(|step| step.id == DoctorStepId::ExternalClientVerified)
            .expect("external client step")
            .status,
        DoctorStepStatus::Skip
    );
    let evidence = report.evidence.expect("server evidence");
    assert!(evidence.discoverable);
    assert!(!evidence.external_client_verified);
    server.shutdown();
}

#[test]
fn server_doctor_requires_twincat_external_client_for_production_ready() {
    let mut config = config(true);
    config.listen = Some(SmolStr::new("127.0.0.1"));
    config.ams_net_id = Some(AmsNetId::new("127.0.0.1.1.1"));
    config.clients = vec![AdsServerClientConfig {
        ams_net_id: AmsNetId::new("5.23.91.12.1.1"),
        source: AdsServerSourcePin::Cidr(SmolStr::new("127.0.0.0/8")),
    }];
    let snapshot = snapshot([("setpoint", Value::Real(3.5))]);
    let debug = DebugControl::new();
    let snapshot_provider = Arc::new({
        let snapshot = snapshot.clone();
        move || Some(snapshot.clone())
    });
    let mut server = super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        debug,
        resource_control(),
        snapshot_provider,
        None,
        0,
    )
    .expect("start ADS server")
    .expect("server enabled");
    server
        .refresh_symbols(&config, &snapshot)
        .expect("publish live ADS symbols before Doctor proof");
    assert_eq!(
        server.symbol_count(),
        1,
        "pyads and TwinCAT evidence must be evaluated against a live served symbol"
    );

    let report = run_ads_server_doctor(AdsServerDoctorInput {
        resource_name: "RESOURCE",
        config: &config,
        snapshot: &snapshot,
        runtime: Some(&server),
        now_ms: 100,
        external_client: Some(AdsServerExternalClientEvidence {
            kind: "pyads".to_string(),
            name: "lab-client".to_string(),
            timestamp_ms: 99,
        }),
        deployed_config_text: Some("ads-server-test-config"),
    });

    assert!(!report.production_ready);
    let evidence = report.evidence.expect("server evidence");
    assert!(evidence.external_client_verified);
    assert_eq!(evidence.external_client_kind.as_deref(), Some("pyads"));
    assert!(evidence.allowed_clients_hash.is_some());
    assert!(evidence.target_identity_hash.is_none());

    let report = run_ads_server_doctor(AdsServerDoctorInput {
        resource_name: "RESOURCE",
        config: &config,
        snapshot: &snapshot,
        runtime: Some(&server),
        now_ms: 101,
        external_client: Some(AdsServerExternalClientEvidence {
            kind: "twincat".to_string(),
            name: "engineering-station".to_string(),
            timestamp_ms: 100,
        }),
        deployed_config_text: Some("ads-server-test-config"),
    });

    assert!(report.production_ready);
    let evidence = report.evidence.expect("server evidence");
    assert!(evidence.external_client_verified);
    assert_eq!(evidence.external_client_kind.as_deref(), Some("twincat"));
    server.shutdown();
}

#[test]
fn server_doctor_blocks_empty_expose_and_empty_clients() {
    let mut config = config(false);
    config.listen = Some(SmolStr::new("127.0.0.1"));
    config.ams_net_id = Some(AmsNetId::new("127.0.0.1.1.1"));
    config.expose.clear();
    config.writable.clear();
    config.clients.clear();
    let snapshot = snapshot([("setpoint", Value::Real(3.5))]);
    let debug = DebugControl::new();
    let snapshot_provider = Arc::new({
        let snapshot = snapshot.clone();
        move || Some(snapshot.clone())
    });
    let mut server = super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        debug,
        resource_control(),
        snapshot_provider,
        None,
        0,
    )
    .expect("start ADS server")
    .expect("server enabled");

    let report = run_ads_server_doctor(AdsServerDoctorInput {
        resource_name: "RESOURCE",
        config: &config,
        snapshot: &snapshot,
        runtime: Some(&server),
        now_ms: 100,
        external_client: Some(AdsServerExternalClientEvidence {
            kind: "pyads".to_string(),
            name: "lab-client".to_string(),
            timestamp_ms: 99,
        }),
        deployed_config_text: Some("ads-server-test-config"),
    });

    assert!(!report.production_ready);
    assert_eq!(
        report
            .steps
            .iter()
            .find(|step| step.id == DoctorStepId::SymbolsExposed)
            .expect("symbols step")
            .status,
        DoctorStepStatus::Fail
    );
    assert_eq!(
        report
            .steps
            .iter()
            .find(|step| step.id == DoctorStepId::ClientsAllowed)
            .expect("clients step")
            .status,
        DoctorStepStatus::Fail
    );
    server.shutdown();
}

fn request_frame(command_id: CommandId, payload: Vec<u8>) -> AmsTcpFrame {
    AmsTcpFrame {
        header: AmsHeader {
            target_net_id: ams_net_id_text_to_bytes("127.0.0.1.1.1").expect("target net id"),
            target_port: 851,
            source_net_id: ams_net_id_text_to_bytes("5.23.91.12.1.1").expect("source net id"),
            source_port: 0x8001,
            command_id,
            state: AmsState::Request,
            data_length: u32::try_from(payload.len()).expect("test payload fits u32"),
            error_code: 0,
            invoke_id: 99,
        },
        payload,
    }
}

fn send_frame(addr: SocketAddr, frame: &AmsTcpFrame) -> AmsTcpFrame {
    let mut stream = std::net::TcpStream::connect(addr).expect("connect ADS server");
    stream
        .write_all(&frame.to_bytes().expect("request bytes"))
        .expect("write request");
    let mut prefix = [0_u8; trust_ads_server::AMS_TCP_HEADER_LEN];
    stream.read_exact(&mut prefix).expect("read prefix");
    let ams_len = u32::from_le_bytes([prefix[2], prefix[3], prefix[4], prefix[5]]) as usize;
    let mut bytes = Vec::from(prefix);
    bytes.resize(trust_ads_server::AMS_TCP_HEADER_LEN + ams_len, 0);
    stream
        .read_exact(&mut bytes[trust_ads_server::AMS_TCP_HEADER_LEN..])
        .expect("read response payload");
    AmsTcpFrame::parse(&bytes, 4096).expect("parse ADS response")
}

fn padded_name(name: &str) -> [u8; 8] {
    let mut out = [0_u8; 8];
    out[..name.len()].copy_from_slice(name.as_bytes());
    out
}

fn query_identify(addr: SocketAddr) -> Vec<u8> {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("UDP bind");
    socket
        .set_read_timeout(Some(StdDuration::from_secs(1)))
        .expect("UDP timeout");
    let mut request = Vec::new();
    request.extend_from_slice(&0x7114_6603_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&1_u32.to_le_bytes());
    request.extend_from_slice(&[0_u8; 6]);
    request.extend_from_slice(&0_u16.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    socket.send_to(&request, addr).expect("send identify");
    let mut response = [0_u8; 512];
    let (len, _) = socket.recv_from(&mut response).expect("receive identify");
    response[..len].to_vec()
}
