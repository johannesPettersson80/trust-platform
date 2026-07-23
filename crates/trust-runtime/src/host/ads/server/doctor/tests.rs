use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::thread;

use smol_str::SmolStr;
use trust_ads_core::{AmsNetId, SymbolDescriptor};
use trust_ads_server::{AmsState, AmsTcpFrame, CommandId, AMS_TCP_HEADER_LEN};

use crate::ads::diagnostics::{AdsStatusOverall, DoctorStepId, DoctorStepStatus};
use crate::debug::{DebugControl, DebugSnapshot};
use crate::memory::VariableStorage;
use crate::scheduler::{ResourceControl, StdClock};
use crate::value::{Duration, Value};

use super::super::{build_runtime_symbol_snapshot, AdsServerClientConfig, AdsServerSourcePin};
use super::*;

fn config() -> AdsServerRuntimeConfig {
    AdsServerRuntimeConfig {
        enabled: true,
        listen: Some(SmolStr::new("127.0.0.1")),
        ads_port: 851,
        ams_net_id: Some(AmsNetId::new("127.0.0.1.1.1")),
        insecure_transport: true,
        expose: vec![SmolStr::new("global.*")],
        clients: vec![AdsServerClientConfig {
            ams_net_id: AmsNetId::new("5.23.91.12.1.1"),
            source: AdsServerSourcePin::Cidr(SmolStr::new("127.0.0.0/8")),
        }],
        ..AdsServerRuntimeConfig::default()
    }
}

fn snapshot(values: impl IntoIterator<Item = (&'static str, Value)>) -> DebugSnapshot {
    let mut storage = VariableStorage::new();
    for (name, value) in values {
        storage.set_global(name, value);
    }
    DebugSnapshot {
        storage,
        now: Duration::from_millis(10),
    }
}

fn resource_control() -> ResourceControl<StdClock> {
    let (resource, _rx) = ResourceControl::stub(StdClock::new());
    resource
}

fn read_request(stream: &mut TcpStream, max_frame_bytes: usize) -> AmsTcpFrame {
    let mut prefix = [0_u8; AMS_TCP_HEADER_LEN];
    stream.read_exact(&mut prefix).expect("read request prefix");
    let ams_len = u32::from_le_bytes([prefix[2], prefix[3], prefix[4], prefix[5]]) as usize;
    let mut bytes = Vec::from(prefix);
    bytes.resize(AMS_TCP_HEADER_LEN + ams_len, 0);
    stream
        .read_exact(&mut bytes[AMS_TCP_HEADER_LEN..])
        .expect("read request body");
    AmsTcpFrame::parse(&bytes, max_frame_bytes).expect("parse request")
}

fn with_mock_client<T>(
    build_responses: impl FnOnce(&AmsTcpFrame) -> Vec<AmsTcpFrame> + Send + 'static,
    operation: impl FnOnce(&mut LoopbackAdsClient) -> T,
) -> T {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock ADS listener");
    let addr = listener.local_addr().expect("mock listener address");
    let max_frame_bytes = config().max_frame_bytes;
    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept mock ADS client");
        let request = read_request(&mut stream, max_frame_bytes);
        for response in build_responses(&request) {
            stream
                .write_all(&response.to_bytes().expect("serialize mock ADS response"))
                .expect("write mock ADS response");
        }
    });

    let mut client = LoopbackAdsClient::connect(
        addr,
        &config(),
        SELF_TEST_SOURCE_NET_ID,
        SELF_TEST_SOURCE_PORT,
    )
    .expect("connect mock ADS client");
    let result = operation(&mut client);
    drop(client);
    server.join().expect("mock ADS server thread");
    result
}

fn response_with_payload(request: &AmsTcpFrame, payload: Vec<u8>) -> AmsTcpFrame {
    AmsTcpFrame {
        header: request
            .header
            .response_for(payload.len(), 0)
            .expect("response header"),
        payload,
    }
}

fn test_symbol() -> SymbolDescriptor {
    let config = config();
    let runtime_snapshot = snapshot([("setpoint", Value::DInt(7))]);
    build_runtime_symbol_snapshot(&config, &runtime_snapshot)
        .expect("symbol snapshot")
        .symbols
        .into_iter()
        .next()
        .expect("test symbol")
}

fn run_notification_payload(
    symbol: &SymbolDescriptor,
    notification_payload: Vec<u8>,
) -> Result<(), String> {
    with_mock_client(
        move |request| {
            let handle = 41_u32;
            let mut add_payload = Vec::new();
            add_payload.extend_from_slice(&0_u32.to_le_bytes());
            add_payload.extend_from_slice(&handle.to_le_bytes());
            let add_response = response_with_payload(request, add_payload);

            let mut notification = response_with_payload(request, notification_payload);
            notification.header.command_id = CommandId::DeviceNotification;
            notification.header.state = AmsState::Request;
            notification.header.invoke_id = 0;
            notification.header.data_length =
                u32::try_from(notification.payload.len()).expect("payload length fits u32");

            vec![add_response, notification]
        },
        |client| client.notification(symbol),
    )
}

#[test]
fn status_uses_live_runtime_symbol_count_not_supplied_snapshot() {
    let config = config();
    let mut runtime = super::super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        DebugControl::new(),
        resource_control(),
        Arc::new(|| None),
        None,
        0,
    )
    .expect("start empty ADS server")
    .expect("server enabled");
    let supplied = snapshot([("setpoint", Value::DInt(7))]);
    let supplied_symbols =
        build_runtime_symbol_snapshot(&config, &supplied).expect("supplied symbol snapshot");

    assert_eq!(runtime.symbol_count(), 0);
    let report = build_ads_server_status_report(&config, Some(&supplied_symbols), Some(&runtime));

    assert_eq!(report.overall, AdsStatusOverall::NotReady);
    assert_eq!(report.connections[0].point_count, 0);
    runtime.shutdown();
}

#[test]
fn doctor_rejects_supplied_symbols_when_live_runtime_table_is_empty() {
    let config = config();
    let mut runtime = super::super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        DebugControl::new(),
        resource_control(),
        Arc::new(|| None),
        None,
        0,
    )
    .expect("start empty ADS server")
    .expect("server enabled");
    let supplied = snapshot([("setpoint", Value::DInt(7))]);
    let empty = snapshot(std::iter::empty::<(&'static str, Value)>());
    assert_eq!(runtime.symbol_count(), 0);

    let run = |caller_snapshot: &DebugSnapshot| {
        run_ads_server_doctor(AdsServerDoctorInput {
            resource_name: "RESOURCE",
            config: &config,
            snapshot: caller_snapshot,
            runtime: Some(&runtime),
            now_ms: 100,
            external_client: Some(AdsServerExternalClientEvidence {
                kind: "twincat".to_string(),
                name: "engineering-station".to_string(),
                timestamp_ms: 99,
            }),
            deployed_config_text: Some("ads-server-test-config"),
        })
    };
    let with_supplied_symbols = run(&supplied);
    let without_supplied_symbols = run(&empty);

    for report in [&with_supplied_symbols, &without_supplied_symbols] {
        assert!(!report.production_ready);
        for id in [DoctorStepId::SymbolsExposed, DoctorStepId::SymbolServe] {
            let step = report
                .steps
                .iter()
                .find(|step| step.id == id)
                .expect("live-symbol Doctor step");
            assert_ne!(
                step.status,
                DoctorStepStatus::Pass,
                "{id:?} must not pass from a caller-supplied snapshot"
            );
        }
    }
    assert!(
        with_supplied_symbols.evidence.is_some(),
        "non-ready Doctor diagnostics retain their live-state attestation"
    );
    assert_eq!(
        with_supplied_symbols.evidence.as_ref(),
        without_supplied_symbols.evidence.as_ref(),
        "caller snapshot content must not affect evidence bound to the empty live server"
    );
    runtime.shutdown();
}

#[test]
fn loopback_connection_failure_fails_every_self_test_step() {
    let steps = self_test_connect_failed("connection refused".to_string());
    let expected = [
        DoctorStepId::SelfReadState,
        DoctorStepId::SelfHandleResolve,
        DoctorStepId::SelfSumupRead,
        DoctorStepId::SelfNotification,
        DoctorStepId::SelfWriteGuarded,
    ];

    assert_eq!(steps.len(), expected.len());
    for (step, expected_id) in steps.iter().zip(expected) {
        assert_eq!(step.id, expected_id);
        assert_eq!(step.status, DoctorStepStatus::Fail);
        assert!(step.detail.contains("connection refused"));
    }
}

#[test]
fn blank_external_client_evidence_is_not_verified_or_passed() {
    let config = config();
    let runtime_snapshot = snapshot([("setpoint", Value::DInt(7))]);
    let mut runtime = super::super::lifecycle::start_ads_server_runtime_on_port(
        "RESOURCE",
        &config,
        DebugControl::new(),
        resource_control(),
        Arc::new({
            let runtime_snapshot = runtime_snapshot.clone();
            move || Some(runtime_snapshot.clone())
        }),
        None,
        0,
    )
    .expect("start ADS server")
    .expect("server enabled");
    runtime
        .refresh_symbols(&config, &runtime_snapshot)
        .expect("publish live ADS symbols before external-proof check");
    assert_eq!(runtime.symbol_count(), 1);

    for (kind, name) in [("   ", "lab-client"), ("pyads", "\t")] {
        let report = run_ads_server_doctor(AdsServerDoctorInput {
            resource_name: "RESOURCE",
            config: &config,
            snapshot: &runtime_snapshot,
            runtime: Some(&runtime),
            now_ms: 100,
            external_client: Some(AdsServerExternalClientEvidence {
                kind: kind.to_string(),
                name: name.to_string(),
                timestamp_ms: 99,
            }),
            deployed_config_text: Some("ads-server-test-config"),
        });

        let step = report
            .steps
            .iter()
            .find(|step| step.id == DoctorStepId::ExternalClientVerified)
            .expect("external client step");
        assert_ne!(step.status, DoctorStepStatus::Pass);
        assert!(
            !report
                .evidence
                .expect("server evidence")
                .external_client_verified
        );
    }
    runtime.shutdown();
}

#[test]
fn zero_symbol_handle_is_rejected() {
    let result = with_mock_client(
        |request| {
            let mut payload = Vec::new();
            payload.extend_from_slice(&0_u32.to_le_bytes());
            payload.extend_from_slice(&4_u32.to_le_bytes());
            payload.extend_from_slice(&0_u32.to_le_bytes());
            vec![response_with_payload(request, payload)]
        },
        |client| client.handle_by_name("global.setpoint"),
    );

    assert!(result.is_err(), "a zero symbol handle must fail closed");
}

#[test]
fn zero_notification_handle_is_rejected() {
    let mut payload = Vec::new();
    payload.extend_from_slice(&0_u32.to_le_bytes());
    payload.extend_from_slice(&0_u32.to_le_bytes());

    assert!(
        expect_add_notification(&payload).is_err(),
        "a zero notification handle must fail closed"
    );
}

#[test]
fn notification_requires_complete_requested_payload() {
    let symbol = test_symbol();
    assert_eq!(symbol.byte_size, 4);

    let handle = 41_u32;
    let mut notification_payload = vec![0_u8; 4];
    notification_payload.extend_from_slice(&1_u32.to_le_bytes());
    notification_payload.extend_from_slice(&0_u64.to_le_bytes());
    notification_payload.extend_from_slice(&1_u32.to_le_bytes());
    notification_payload.extend_from_slice(&handle.to_le_bytes());
    notification_payload.extend_from_slice(&2_u32.to_le_bytes());
    notification_payload.extend_from_slice(&[0xAA, 0xBB]);
    let stream_len =
        u32::try_from(notification_payload.len() - 4).expect("notification length fits u32");
    notification_payload[..4].copy_from_slice(&stream_len.to_le_bytes());
    let result = run_notification_payload(&symbol, notification_payload);

    assert!(
        result.is_err(),
        "a matching handle with only a payload prefix must fail closed"
    );
}

#[test]
fn notification_rejects_malformed_declared_tail_after_valid_match() {
    let symbol = test_symbol();
    let mut payload = vec![0_u8; 4];
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.extend_from_slice(&0_u64.to_le_bytes());
    payload.extend_from_slice(&2_u32.to_le_bytes());
    payload.extend_from_slice(&41_u32.to_le_bytes());
    payload.extend_from_slice(&4_u32.to_le_bytes());
    payload.extend_from_slice(&[1, 2, 3, 4]);
    payload.extend_from_slice(&99_u32.to_le_bytes());
    payload.extend_from_slice(&4_u32.to_le_bytes());
    payload.extend_from_slice(&[0xAA, 0xBB]);
    let stream_len = u32::try_from(payload.len() - 4).expect("notification length fits u32");
    payload[..4].copy_from_slice(&stream_len.to_le_bytes());

    assert!(
        run_notification_payload(&symbol, payload).is_err(),
        "a valid matching sample must not hide a truncated later declared sample"
    );
}

#[test]
fn notification_rejects_bytes_outside_declared_stream() {
    let symbol = test_symbol();
    let mut payload = vec![0_u8; 4];
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.extend_from_slice(&0_u64.to_le_bytes());
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.extend_from_slice(&41_u32.to_le_bytes());
    payload.extend_from_slice(&4_u32.to_le_bytes());
    payload.extend_from_slice(&[1, 2, 3, 4]);
    let stream_len = u32::try_from(payload.len() - 4).expect("notification length fits u32");
    payload[..4].copy_from_slice(&stream_len.to_le_bytes());
    payload.push(0xCC);

    assert!(
        run_notification_payload(&symbol, payload).is_err(),
        "bytes outside the declared notification stream must fail closed"
    );
}

#[test]
fn notification_rejects_duplicate_matching_handle_samples() {
    let symbol = test_symbol();
    let mut payload = vec![0_u8; 4];
    payload.extend_from_slice(&1_u32.to_le_bytes());
    payload.extend_from_slice(&0_u64.to_le_bytes());
    payload.extend_from_slice(&2_u32.to_le_bytes());
    for bytes in [[1, 2, 3, 4], [5, 6, 7, 8]] {
        payload.extend_from_slice(&41_u32.to_le_bytes());
        payload.extend_from_slice(&4_u32.to_le_bytes());
        payload.extend_from_slice(&bytes);
    }
    let stream_len = u32::try_from(payload.len() - 4).expect("notification length fits u32");
    payload[..4].copy_from_slice(&stream_len.to_le_bytes());

    assert!(
        run_notification_payload(&symbol, payload).is_err(),
        "duplicate samples for the expected notification handle are ambiguous"
    );
}

#[derive(Debug, Clone, Copy)]
enum ResponseDefect {
    RequestState,
    TargetNetId,
    TargetPort,
    SourceNetId,
    SourcePort,
    Command,
    Invoke,
}

#[test]
fn loopback_response_requires_direction_and_complete_request_identity() {
    for defect in [
        ResponseDefect::RequestState,
        ResponseDefect::TargetNetId,
        ResponseDefect::TargetPort,
        ResponseDefect::SourceNetId,
        ResponseDefect::SourcePort,
        ResponseDefect::Command,
        ResponseDefect::Invoke,
    ] {
        let result = with_mock_client(
            move |request| {
                let mut response = response_with_payload(request, Vec::new());
                match defect {
                    ResponseDefect::RequestState => response.header.state = AmsState::Request,
                    ResponseDefect::TargetNetId => response.header.target_net_id = [9; 6],
                    ResponseDefect::TargetPort => response.header.target_port = 999,
                    ResponseDefect::SourceNetId => response.header.source_net_id = [8; 6],
                    ResponseDefect::SourcePort => response.header.source_port = 998,
                    ResponseDefect::Command => response.header.command_id = CommandId::Read,
                    ResponseDefect::Invoke => response.header.invoke_id += 1,
                }
                vec![response]
            },
            |client| client.request(CommandId::ReadState, Vec::new()),
        );
        assert!(
            result.is_err(),
            "{defect:?} must not be accepted as the requested response"
        );
    }
}

#[test]
fn loopback_response_accepts_exact_correlated_response() {
    let result = with_mock_client(
        |request| vec![response_with_payload(request, Vec::new())],
        |client| client.request(CommandId::ReadState, Vec::new()),
    );

    assert!(result.is_ok());
}
