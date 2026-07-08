//! ADS server doctor and production evidence.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use trust_ads_core::{AmsNetId, SymbolDescriptor, SymbolFlag};
use trust_ads_server::{
    ams_net_id_text_to_bytes, AdsErrorCode, AmsHeader, AmsParseError, AmsState, AmsTcpFrame,
    CommandId, ADSIGRP_SUMUP_READ, ADSIGRP_SYM_HNDBYNAME, ADSTRANS_SERVERCYCLE, AMS_HEADER_LEN,
    AMS_TCP_HEADER_LEN,
};

use crate::ads::diagnostics::{
    build_server_production_evidence, AdsConnectionStatus, AdsConnectionStatusState,
    AdsStatusOverall, AdsStatusReport, DiagnosticTransport, DoctorReport, DoctorRole,
    DoctorSkipReason, DoctorStep, DoctorStepId, DoctorStepStatus, DoctorVantage, LocalIdentity,
    NextAction, NextActionKind, ProductionEvidence, ServerProductionEvidenceInput,
};
use crate::ads::onboarding::classify_local_address;
use crate::debug::DebugSnapshot;

use super::{
    build_runtime_symbol_snapshot, AdsServerRuntime, AdsServerRuntimeConfig, AdsServerSourcePin,
};

const SELF_TEST_SOURCE_NET_ID: &str = "127.0.0.1.1.2";
const DENIED_SOURCE_NET_ID: &str = "127.0.0.1.1.200";
const SELF_TEST_SOURCE_PORT: u16 = 0x8001;
const SERVER_DOCTOR_STALE_AFTER_MS: u64 = 30 * 60 * 1000;
const DOCTOR_IO_TIMEOUT: Duration = Duration::from_secs(2);

/// Optional independent-client proof attached to a server doctor run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsServerExternalClientEvidence {
    /// External client kind, for example `twincat`, `pyads`, or `.net`.
    pub kind: String,
    /// Human-readable external client name.
    pub name: String,
    /// Runtime-host clock timestamp in milliseconds.
    pub timestamp_ms: u64,
}

/// Inputs for one ADS server doctor run.
pub struct AdsServerDoctorInput<'a> {
    /// Runtime resource name.
    pub resource_name: &'a str,
    /// Runtime ADS server config.
    pub config: &'a AdsServerRuntimeConfig,
    /// Latest runtime snapshot used for symbol publication evidence.
    pub snapshot: &'a DebugSnapshot,
    /// Running ADS server subsystem, when available.
    pub runtime: Option<&'a AdsServerRuntime>,
    /// Current runtime-host clock in milliseconds.
    pub now_ms: u64,
    /// Optional independent-client evidence. Loopback self-test never sets this.
    pub external_client: Option<AdsServerExternalClientEvidence>,
    /// Canonical deployed config text if available; otherwise a deterministic summary is hashed.
    pub deployed_config_text: Option<&'a str>,
}

/// Runs the ADS server doctor from the runtime host.
///
/// Loopback self-test proves that the listener, symbol table, read path,
/// notification path, and write guard are wired. It deliberately does not mark
/// production-ready without independent external-client evidence.
#[must_use]
pub fn run_ads_server_doctor(input: AdsServerDoctorInput<'_>) -> DoctorReport {
    let local = local_identity(input.resource_name, input.config);
    let symbol_snapshot = build_runtime_symbol_snapshot(input.config, input.snapshot);
    let status =
        build_ads_server_status_report(input.config, symbol_snapshot.as_ref().ok(), input.runtime);
    let mut steps = Vec::new();

    if !input.config.enabled {
        steps.push(
            DoctorStep::skipped(
                DoctorStepId::BindExposure,
                "ADS server enabled",
                DoctorSkipReason::ServerDisabled,
                "runtime.ads_server.enabled is false.",
            )
            .with_next_action(NextAction::new(NextActionKind::ConfigureExpose)),
        );
        return finalize_report(input, local, steps, symbol_snapshot.ok(), status, false);
    }

    steps.push(bind_exposure_step(input.config));
    steps.push(listener_bound_step(input.runtime));

    let discoverable = input
        .runtime
        .is_some_and(|runtime| udp_identify_step(runtime, input.config, &mut steps));

    match &symbol_snapshot {
        Ok(snapshot) if !snapshot.symbols.is_empty() => steps.push(
            DoctorStep::new(
                DoctorStepId::SymbolsExposed,
                "Symbols exposed",
                DoctorStepStatus::Pass,
                format!("{} ADS symbol(s) are exposed.", snapshot.symbols.len()),
            )
            .with_evidence("symbol_count", json!(snapshot.symbols.len())),
        ),
        Ok(_) => steps.push(
            DoctorStep::failed(
                DoctorStepId::SymbolsExposed,
                "Symbols exposed",
                "No runtime variables match runtime.ads_server.expose.",
                failure(NextActionKind::ConfigureExpose),
            )
            .with_evidence("symbol_count", json!(0)),
        ),
        Err(error) => steps.push(DoctorStep::failed(
            DoctorStepId::SymbolsExposed,
            "Symbols exposed",
            format!("Failed to build ADS symbol snapshot: {error}"),
            failure(NextActionKind::ConfigureExpose),
        )),
    }

    steps.push(clients_allowed_step(input.config));

    if let Ok(snapshot) = &symbol_snapshot {
        steps.push(symbol_serve_step(snapshot.symbols.first()));
    } else {
        steps.push(DoctorStep::skipped(
            DoctorStepId::SymbolServe,
            "Symbol service",
            DoctorSkipReason::BlockedByPreviousStep,
            "Symbol service check is blocked because symbol snapshot generation failed.",
        ));
    }

    if let (Some(runtime), Ok(snapshot)) = (input.runtime, &symbol_snapshot) {
        steps.extend(run_loopback_self_test(
            runtime,
            input.config,
            snapshot.symbols.as_slice(),
        ));
        steps.push(allowlist_enforced_step(
            runtime,
            input.config,
            snapshot.symbols.as_slice(),
        ));
    } else {
        steps.extend(blocked_self_test_steps());
        steps.push(DoctorStep::skipped(
            DoctorStepId::AllowlistEnforced,
            "Allowlist enforced",
            DoctorSkipReason::BlockedByPreviousStep,
            "Allowlist probe is blocked because the ADS server listener is not running.",
        ));
    }

    steps.push(parser_limits_step(input.config.max_frame_bytes));
    steps.push(external_client_step(input.external_client.as_ref()));

    finalize_report(
        input,
        local,
        steps,
        symbol_snapshot.ok(),
        status,
        discoverable,
    )
}

fn finalize_report(
    input: AdsServerDoctorInput<'_>,
    local: LocalIdentity,
    steps: Vec<DoctorStep>,
    symbol_snapshot: Option<trust_ads_core::SymbolSnapshot>,
    status: AdsStatusReport,
    discoverable: bool,
) -> DoctorReport {
    let mut report = DoctorReport::for_role(
        DoctorRole::Server,
        DoctorVantage::RuntimeHost,
        DiagnosticTransport::Plain,
    )
    .with_local(local.clone())
    .with_steps(steps);
    report.writes_enabled = input.config.writes_enabled;

    if let Some(snapshot) = symbol_snapshot {
        if let Ok(evidence) = build_server_evidence(input, &local, &snapshot, &status, discoverable)
        {
            report = report.with_evidence(evidence);
        }
    }
    report.summary = summary_for_report(&report);
    report
}

fn summary_for_report(report: &DoctorReport) -> String {
    let blocking = report
        .steps
        .iter()
        .filter(|step| step.blocks_production_ready && step.status != DoctorStepStatus::Pass)
        .count();
    if blocking == 0 && report.production_ready {
        "ADS server is production-ready with independent client evidence.".to_string()
    } else if blocking == 0
        && report.role == DoctorRole::Server
        && report.evidence.as_ref().is_some_and(|evidence| {
            evidence.external_client_verified
                && evidence
                    .external_client_kind
                    .as_deref()
                    .is_some_and(|kind| !kind.trim().eq_ignore_ascii_case("twincat"))
        })
    {
        "ADS server self-test passed with non-TwinCAT client evidence; real TwinCAT validation is still required.".to_string()
    } else if blocking == 0 {
        "ADS server self-test passed; independent client evidence is still required.".to_string()
    } else {
        format!("ADS server is not production-ready: {blocking} blocking step(s).")
    }
}

fn build_server_evidence(
    input: AdsServerDoctorInput<'_>,
    local: &LocalIdentity,
    snapshot: &trust_ads_core::SymbolSnapshot,
    status: &AdsStatusReport,
    discoverable: bool,
) -> Result<ProductionEvidence, crate::ads::diagnostics::ProductionEvidenceError> {
    let allowed_clients = allowed_clients_evidence(input.config);
    let config_text = input
        .deployed_config_text
        .map(ToString::to_string)
        .unwrap_or_else(|| config_evidence_text(input.config));
    let expires_at_ms = input
        .now_ms
        .checked_add(SERVER_DOCTOR_STALE_AFTER_MS)
        .or(Some(input.now_ms));
    build_server_production_evidence(ServerProductionEvidenceInput {
        doctor_timestamp_ms: input.now_ms,
        runtime_identity: local,
        allowed_clients: &allowed_clients,
        ads_server_config: config_text.as_str(),
        symbol_snapshot: snapshot,
        deployed_ads_server_config: Some(config_text.as_str()),
        runtime_ads_status: Some(status),
        external_client_verified: input.external_client.is_some(),
        external_client_kind: input
            .external_client
            .as_ref()
            .map(|evidence| evidence.kind.as_str()),
        external_client_name: input
            .external_client
            .as_ref()
            .map(|evidence| evidence.name.as_str()),
        external_client_timestamp_ms: input
            .external_client
            .as_ref()
            .map(|evidence| evidence.timestamp_ms),
        discoverable,
        stale_after_ms: SERVER_DOCTOR_STALE_AFTER_MS,
        expires_at_ms,
        runtime_clock_warning: None,
    })
}

fn local_identity(resource_name: &str, config: &AdsServerRuntimeConfig) -> LocalIdentity {
    let chosen_ip = config.listen.as_deref().unwrap_or("127.0.0.1").to_string();
    let ams_net_id = config
        .ams_net_id
        .as_ref()
        .map(|net_id| net_id.0.clone())
        .unwrap_or_else(|| format!("{chosen_ip}.1.1"));
    LocalIdentity {
        host_name: Some(resource_name.to_string()),
        chosen_ip: chosen_ip.clone(),
        ams_net_id,
        nic: None,
        candidates: Vec::new(),
        classification: classify_local_address(&chosen_ip, None),
    }
}

fn bind_exposure_step(config: &AdsServerRuntimeConfig) -> DoctorStep {
    let Some(listen) = config.listen.as_deref() else {
        return DoctorStep::failed(
            DoctorStepId::BindExposure,
            "Bind exposure",
            "runtime.ads_server.listen is not configured.",
            failure(NextActionKind::OpenFirewall),
        );
    };
    if listen == "0.0.0.0" || listen == "::" {
        return DoctorStep::failed(
            DoctorStepId::BindExposure,
            "Bind exposure",
            "ADS server must bind a concrete runtime-host IP, not a wildcard address.",
            failure(NextActionKind::OpenFirewall),
        )
        .with_evidence("listen", listen.to_string());
    }
    DoctorStep::new(
        DoctorStepId::BindExposure,
        "Bind exposure",
        DoctorStepStatus::Pass,
        format!("ADS server is configured to bind {listen}:48898."),
    )
    .with_evidence("listen", listen.to_string())
}

fn listener_bound_step(runtime: Option<&AdsServerRuntime>) -> DoctorStep {
    match runtime {
        Some(runtime) => DoctorStep::new(
            DoctorStepId::ListenerBound,
            "Listener bound",
            DoctorStepStatus::Pass,
            format!("ADS TCP listener is bound at {}.", runtime.local_addr()),
        )
        .with_evidence("tcp_addr", runtime.local_addr().to_string()),
        None => DoctorStep::failed(
            DoctorStepId::ListenerBound,
            "Listener bound",
            "ADS server runtime is not running.",
            failure(NextActionKind::RerunDoctor),
        ),
    }
}

fn udp_identify_step(
    runtime: &AdsServerRuntime,
    config: &AdsServerRuntimeConfig,
    steps: &mut Vec<DoctorStep>,
) -> bool {
    match query_identify(runtime.identify_addr(), config) {
        Ok(()) => {
            steps.push(
                DoctorStep::new(
                    DoctorStepId::UdpIdentifyAnswer,
                    "UDP identify",
                    DoctorStepStatus::Pass,
                    format!(
                        "ADS identify responder answered at {}.",
                        runtime.identify_addr()
                    ),
                )
                .with_evidence("udp_addr", runtime.identify_addr().to_string()),
            );
            true
        }
        Err(error) => {
            steps.push(DoctorStep::failed(
                DoctorStepId::UdpIdentifyAnswer,
                "UDP identify",
                format!("ADS identify responder did not answer: {error}"),
                failure(NextActionKind::OpenFirewall),
            ));
            false
        }
    }
}

fn query_identify(
    identify_addr: SocketAddr,
    config: &AdsServerRuntimeConfig,
) -> Result<(), String> {
    let socket = UdpSocket::bind(if identify_addr.is_ipv4() {
        "0.0.0.0:0"
    } else {
        "[::]:0"
    })
    .map_err(|error| format!("bind UDP identify probe: {error}"))?;
    socket
        .set_read_timeout(Some(DOCTOR_IO_TIMEOUT))
        .map_err(|error| format!("set UDP timeout: {error}"))?;
    let request = identify_request();
    socket
        .send_to(&request, identify_addr)
        .map_err(|error| format!("send UDP identify probe: {error}"))?;
    let mut response = [0_u8; 512];
    let (len, _) = socket
        .recv_from(&mut response)
        .map_err(|error| format!("receive UDP identify response: {error}"))?;
    let expected = config
        .ams_net_id
        .as_ref()
        .ok_or_else(|| "runtime.ads_server.ams_net_id is missing".to_string())?;
    let expected = ams_net_id_text_to_bytes(expected.0.as_str())
        .map_err(|error| format!("parse server AMS Net ID: {error}"))?;
    if len < 18 || response[12..18] != expected {
        return Err("identify response carried an unexpected AMS Net ID".to_string());
    }
    Ok(())
}

fn identify_request() -> Vec<u8> {
    let mut request = Vec::with_capacity(24);
    request.extend_from_slice(&0x7114_6603_u32.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request.extend_from_slice(&1_u32.to_le_bytes());
    request.extend_from_slice(&[0_u8; 6]);
    request.extend_from_slice(&0_u16.to_le_bytes());
    request.extend_from_slice(&0_u32.to_le_bytes());
    request
}

fn clients_allowed_step(config: &AdsServerRuntimeConfig) -> DoctorStep {
    if config.clients.is_empty() {
        return DoctorStep::failed(
            DoctorStepId::ClientsAllowed,
            "Clients allowed",
            "No external ADS clients are allowlisted.",
            failure(NextActionKind::AddAllowedClient),
        );
    }
    if let Some(client) = config
        .clients
        .iter()
        .find(|client| matches!(client.source, AdsServerSourcePin::Unpinned))
    {
        return DoctorStep::failed(
            DoctorStepId::ClientsAllowed,
            "Clients allowed",
            format!(
                "Client {} is not source-IP/CIDR pinned; plain ADS production-ready requires pinning.",
                client.ams_net_id.0
            ),
            failure(NextActionKind::AddAllowedClient),
        );
    }
    DoctorStep::new(
        DoctorStepId::ClientsAllowed,
        "Clients allowed",
        DoctorStepStatus::Pass,
        format!(
            "{} external ADS client(s) are allowlisted.",
            config.clients.len()
        ),
    )
    .with_evidence("client_count", json!(config.clients.len()))
}

fn symbol_serve_step(symbol: Option<&SymbolDescriptor>) -> DoctorStep {
    match symbol {
        Some(symbol) => DoctorStep::new(
            DoctorStepId::SymbolServe,
            "Symbol service",
            DoctorStepStatus::Pass,
            format!("Symbol '{}' is available for ADS service.", symbol.name),
        )
        .with_evidence("symbol", symbol.name.clone())
        .with_evidence("byte_size", json!(symbol.byte_size)),
        None => DoctorStep::skipped(
            DoctorStepId::SymbolServe,
            "Symbol service",
            DoctorSkipReason::NoSymbolsExposed,
            "No exposed symbol is available for service checks.",
        )
        .with_next_action(NextAction::new(NextActionKind::ConfigureExpose)),
    }
}

fn run_loopback_self_test(
    runtime: &AdsServerRuntime,
    config: &AdsServerRuntimeConfig,
    symbols: &[SymbolDescriptor],
) -> Vec<DoctorStep> {
    let Some(first_symbol) = symbols.first() else {
        return blocked_self_test_steps();
    };
    let source_ip = runtime.local_addr().ip().to_string();
    let _permit = runtime
        .policy()
        .permit_temporarily(AmsNetId::new(SELF_TEST_SOURCE_NET_ID), source_ip);
    let mut client = match LoopbackAdsClient::connect(
        runtime.local_addr(),
        config,
        SELF_TEST_SOURCE_NET_ID,
        SELF_TEST_SOURCE_PORT,
    ) {
        Ok(client) => client,
        Err(error) => {
            return self_test_connect_failed(error);
        }
    };

    let read_state = match client.read_state() {
        Ok(()) => DoctorStep::new(
            DoctorStepId::SelfReadState,
            "Self read state",
            DoctorStepStatus::Pass,
            "Loopback ADS client read the server state.",
        ),
        Err(error) => DoctorStep::failed(
            DoctorStepId::SelfReadState,
            "Self read state",
            error,
            failure(NextActionKind::RerunDoctor),
        ),
    };
    let handle = client.handle_by_name(&first_symbol.name);
    let handle_step = match handle {
        Ok(_) => DoctorStep::new(
            DoctorStepId::SelfHandleResolve,
            "Self handle resolve",
            DoctorStepStatus::Pass,
            format!("Loopback ADS client resolved '{}'.", first_symbol.name),
        )
        .with_evidence("symbol", first_symbol.name.clone()),
        Err(ref error) => DoctorStep::failed(
            DoctorStepId::SelfHandleResolve,
            "Self handle resolve",
            error.clone(),
            failure(NextActionKind::RerunDoctor),
        ),
    };
    let sumup_step = match client.sumup_read(first_symbol) {
        Ok(()) => DoctorStep::new(
            DoctorStepId::SelfSumupRead,
            "Self sum-up read",
            DoctorStepStatus::Pass,
            format!("Loopback ADS client sum-up read '{}'.", first_symbol.name),
        )
        .with_evidence("symbol", first_symbol.name.clone()),
        Err(error) => DoctorStep::failed(
            DoctorStepId::SelfSumupRead,
            "Self sum-up read",
            error,
            failure(NextActionKind::RerunDoctor),
        ),
    };
    let notification_step = match client.notification(first_symbol) {
        Ok(()) => DoctorStep::new(
            DoctorStepId::SelfNotification,
            "Self notification",
            DoctorStepStatus::Pass,
            format!(
                "Loopback ADS client received a notification for '{}'.",
                first_symbol.name
            ),
        )
        .with_evidence("symbol", first_symbol.name.clone()),
        Err(error) => DoctorStep::failed(
            DoctorStepId::SelfNotification,
            "Self notification",
            error,
            failure(NextActionKind::RerunDoctor),
        ),
    };
    let write_step = self_write_guarded_step(&mut client, symbols, config);

    vec![
        read_state,
        handle_step,
        sumup_step,
        notification_step,
        write_step,
    ]
}

fn self_test_connect_failed(error: String) -> Vec<DoctorStep> {
    [
        DoctorStepId::SelfReadState,
        DoctorStepId::SelfHandleResolve,
        DoctorStepId::SelfSumupRead,
        DoctorStepId::SelfNotification,
        DoctorStepId::SelfWriteGuarded,
    ]
    .into_iter()
    .map(|id| {
        DoctorStep::failed(
            id,
            title(id),
            format!("Loopback ADS client could not connect: {error}"),
            failure(NextActionKind::RerunDoctor),
        )
    })
    .collect()
}

fn blocked_self_test_steps() -> Vec<DoctorStep> {
    [
        DoctorStepId::SelfReadState,
        DoctorStepId::SelfHandleResolve,
        DoctorStepId::SelfSumupRead,
        DoctorStepId::SelfNotification,
        DoctorStepId::SelfWriteGuarded,
    ]
    .into_iter()
    .map(|id| {
        DoctorStep::skipped(
            id,
            title(id),
            DoctorSkipReason::BlockedByPreviousStep,
            "Self-test is blocked because no exposed ADS symbol is available.",
        )
    })
    .collect()
}

fn self_write_guarded_step(
    client: &mut LoopbackAdsClient,
    symbols: &[SymbolDescriptor],
    config: &AdsServerRuntimeConfig,
) -> DoctorStep {
    if let Some(symbol) = symbols
        .iter()
        .find(|symbol| symbol.flags.contains(&SymbolFlag::Write))
    {
        return match client
            .direct_read(symbol)
            .and_then(|bytes| client.write(symbol, &bytes))
        {
            Ok(()) => DoctorStep::new(
                DoctorStepId::SelfWriteGuarded,
                "Self write guard",
                DoctorStepStatus::Pass,
                format!(
                    "Loopback ADS client wrote guarded symbol '{}'.",
                    symbol.name
                ),
            )
            .with_evidence("symbol", symbol.name.clone()),
            Err(error) => DoctorStep::failed(
                DoctorStepId::SelfWriteGuarded,
                "Self write guard",
                error,
                failure(NextActionKind::RerunDoctor),
            ),
        };
    }

    let Some(symbol) = symbols.first() else {
        return DoctorStep::skipped(
            DoctorStepId::SelfWriteGuarded,
            "Self write guard",
            DoctorSkipReason::NoSymbolsExposed,
            "Write guard cannot be tested because no exposed symbol exists.",
        );
    };
    let probe = vec![0_u8; symbol.byte_size as usize];
    match client.write_expect_denied(symbol, &probe) {
        Ok(()) => DoctorStep::new(
            DoctorStepId::SelfWriteGuarded,
            "Self write guard",
            DoctorStepStatus::Pass,
            if config.writes_enabled {
                "No writable symbols are configured; read-only ADS write guard rejected a write."
            } else {
                "runtime.ads_server.writes_enabled=false; ADS write guard rejected a write."
            },
        )
        .with_evidence("symbol", symbol.name.clone()),
        Err(error) => DoctorStep::failed(
            DoctorStepId::SelfWriteGuarded,
            "Self write guard",
            error,
            failure(NextActionKind::RerunDoctor),
        ),
    }
}

fn allowlist_enforced_step(
    runtime: &AdsServerRuntime,
    config: &AdsServerRuntimeConfig,
    symbols: &[SymbolDescriptor],
) -> DoctorStep {
    let Some(symbol) = symbols.first() else {
        return DoctorStep::skipped(
            DoctorStepId::AllowlistEnforced,
            "Allowlist enforced",
            DoctorSkipReason::NoSymbolsExposed,
            "Allowlist probe is blocked because no exposed symbol is available.",
        );
    };
    let mut client = match LoopbackAdsClient::connect(
        runtime.local_addr(),
        config,
        DENIED_SOURCE_NET_ID,
        SELF_TEST_SOURCE_PORT + 1,
    ) {
        Ok(client) => client,
        Err(error) => {
            return DoctorStep::failed(
                DoctorStepId::AllowlistEnforced,
                "Allowlist enforced",
                format!("Denied-client probe could not connect: {error}"),
                failure(NextActionKind::RerunDoctor),
            );
        }
    };
    match client.direct_read_expect_denied(symbol) {
        Ok(()) => DoctorStep::new(
            DoctorStepId::AllowlistEnforced,
            "Allowlist enforced",
            DoctorStepStatus::Pass,
            "A non-allowlisted ADS client was rejected by the Rust policy layer.",
        )
        .with_evidence("denied_ams_net_id", DENIED_SOURCE_NET_ID),
        Err(error) => DoctorStep::failed(
            DoctorStepId::AllowlistEnforced,
            "Allowlist enforced",
            error,
            failure(NextActionKind::AddAllowedClient),
        ),
    }
}

fn parser_limits_step(max_frame_bytes: usize) -> DoctorStep {
    let oversized_len = max_frame_bytes.saturating_add(1);
    let oversized_len = u32::try_from(oversized_len).unwrap_or(u32::MAX);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&[0, 0]);
    bytes.extend_from_slice(&oversized_len.to_le_bytes());
    match AmsTcpFrame::parse(&bytes, max_frame_bytes) {
        Err(AmsParseError::FrameTooLarge { .. }) => DoctorStep::new(
            DoctorStepId::ParserLimits,
            "Parser limits",
            DoctorStepStatus::Pass,
            "AMS/TCP parser rejects oversized peer-declared frames before payload allocation.",
        )
        .with_evidence("max_frame_bytes", json!(max_frame_bytes)),
        other => DoctorStep::failed(
            DoctorStepId::ParserLimits,
            "Parser limits",
            format!("Oversized parser probe returned unexpected result: {other:?}"),
            failure(NextActionKind::RerunDoctor),
        ),
    }
}

fn external_client_step(evidence: Option<&AdsServerExternalClientEvidence>) -> DoctorStep {
    match evidence {
        Some(evidence) => DoctorStep::new(
            DoctorStepId::ExternalClientVerified,
            "External client verified",
            DoctorStepStatus::Pass,
            format!(
                "{} '{}' verified the ADS server externally.",
                evidence.kind, evidence.name
            ),
        )
        .with_evidence("kind", evidence.kind.clone())
        .with_evidence("name", evidence.name.clone())
        .with_evidence("timestamp_ms", json!(evidence.timestamp_ms)),
        None => DoctorStep::skipped(
            DoctorStepId::ExternalClientVerified,
            "External client verified",
            DoctorSkipReason::ExternalClientPending,
            "Loopback self-test passed only inside truST; an independent ADS client must still verify the server.",
        )
        .with_next_action(NextAction::new(NextActionKind::WaitForClient)),
    }
}

/// Builds the ADS server status report used by control/web surfaces.
#[must_use]
pub fn build_ads_server_status_report(
    config: &AdsServerRuntimeConfig,
    snapshot: Option<&trust_ads_core::SymbolSnapshot>,
    runtime: Option<&AdsServerRuntime>,
) -> AdsStatusReport {
    let point_count = snapshot.map_or(0, |snapshot| snapshot.symbols.len());
    let state = if !config.enabled {
        AdsStatusOverall::Disabled
    } else if runtime.is_some() && point_count > 0 && !config.clients.is_empty() {
        AdsStatusOverall::Healthy
    } else if runtime.is_some() {
        AdsStatusOverall::NotReady
    } else {
        AdsStatusOverall::Faulted
    };
    AdsStatusReport {
        schema_version: crate::ads::diagnostics::ADS_DIAGNOSTICS_SCHEMA_VERSION,
        role: DoctorRole::Server,
        overall: state,
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: vec![AdsConnectionStatus {
            name: "ads-server".to_string(),
            target: None,
            state: match state {
                AdsStatusOverall::Healthy => AdsConnectionStatusState::Connected,
                AdsStatusOverall::NotReady => AdsConnectionStatusState::NotReady,
                AdsStatusOverall::Disabled => AdsConnectionStatusState::Disabled,
                AdsStatusOverall::Faulted => AdsConnectionStatusState::Faulted,
                AdsStatusOverall::Degraded => AdsConnectionStatusState::Stale,
                AdsStatusOverall::Unknown => AdsConnectionStatusState::Unknown,
            },
            point_count,
            degraded_points: 0,
            last_good_value_ms: None,
            symbol_version: None,
            summary: format!("ADS server exposes {point_count} symbol(s)."),
        }],
        summary: match state {
            AdsStatusOverall::Healthy => "ADS server is listening and serving symbols.".to_string(),
            AdsStatusOverall::NotReady => {
                "ADS server is listening but not ready to serve symbols.".to_string()
            }
            AdsStatusOverall::Disabled => "ADS server is disabled.".to_string(),
            _ => "ADS server is not ready.".to_string(),
        },
    }
}

#[derive(Serialize)]
struct AllowedClientsEvidence<'a> {
    clients: Vec<AllowedClientEvidence<'a>>,
    allow_unpinned_clients: bool,
    unsafe_allow_public_bind: bool,
}

#[derive(Serialize)]
struct AllowedClientEvidence<'a> {
    ams_net_id: &'a str,
    source_kind: &'static str,
    source: &'a str,
}

fn allowed_clients_evidence(config: &AdsServerRuntimeConfig) -> AllowedClientsEvidence<'_> {
    AllowedClientsEvidence {
        clients: config
            .clients
            .iter()
            .map(|client| {
                let (source_kind, source) = match &client.source {
                    AdsServerSourcePin::Ip(value) => ("ip", value.as_str()),
                    AdsServerSourcePin::Cidr(value) => ("cidr", value.as_str()),
                    AdsServerSourcePin::Unpinned => ("unpinned", ""),
                };
                AllowedClientEvidence {
                    ams_net_id: client.ams_net_id.0.as_str(),
                    source_kind,
                    source,
                }
            })
            .collect(),
        allow_unpinned_clients: config.allow_unpinned_clients,
        unsafe_allow_public_bind: config.unsafe_allow_public_bind,
    }
}

fn config_evidence_text(config: &AdsServerRuntimeConfig) -> String {
    let mut lines = Vec::new();
    lines.push(format!("enabled={}", config.enabled));
    lines.push(format!(
        "listen={}",
        config.listen.as_deref().unwrap_or_default()
    ));
    lines.push(format!("ads_port={}", config.ads_port));
    lines.push(format!(
        "ams_net_id={}",
        config
            .ams_net_id
            .as_ref()
            .map(|value| value.0.as_str())
            .unwrap_or_default()
    ));
    lines.push(format!("writes_enabled={}", config.writes_enabled));
    lines.push(format!("expose={:?}", config.expose));
    lines.push(format!("writable={:?}", config.writable));
    lines.push(format!("clients={}", config.clients.len()));
    lines.join("\n")
}

fn failure(next_action: NextActionKind) -> crate::ads::diagnostics::FailureClassification {
    crate::ads::diagnostics::FailureClassification {
        kind: crate::ads::diagnostics::OnboardingFailureKind::UnsupportedOperation,
        explanation: "ADS server doctor step failed.".to_string(),
        remediation:
            "Fix the reported ADS server configuration or runtime state, then rerun the doctor."
                .to_string(),
        next_action: NextAction::new(next_action),
        ads_error: None,
        blocks_production_ready: true,
    }
}

fn title(id: DoctorStepId) -> &'static str {
    match id {
        DoctorStepId::SelfReadState => "Self read state",
        DoctorStepId::SelfHandleResolve => "Self handle resolve",
        DoctorStepId::SelfSumupRead => "Self sum-up read",
        DoctorStepId::SelfNotification => "Self notification",
        DoctorStepId::SelfWriteGuarded => "Self write guard",
        _ => "ADS server doctor step",
    }
}

struct LoopbackAdsClient {
    stream: TcpStream,
    target_net_id: [u8; 6],
    target_port: u16,
    source_net_id: [u8; 6],
    source_port: u16,
    max_frame_bytes: usize,
    next_invoke: u32,
}

impl LoopbackAdsClient {
    fn connect(
        addr: SocketAddr,
        config: &AdsServerRuntimeConfig,
        source_net_id: &str,
        source_port: u16,
    ) -> Result<Self, String> {
        let target = config
            .ams_net_id
            .as_ref()
            .ok_or_else(|| "runtime.ads_server.ams_net_id is missing".to_string())?;
        let target_net_id = ams_net_id_text_to_bytes(target.0.as_str())
            .map_err(|error| format!("parse target AMS Net ID: {error}"))?;
        let source_net_id = ams_net_id_text_to_bytes(source_net_id)
            .map_err(|error| format!("parse self-test source AMS Net ID: {error}"))?;
        let stream = TcpStream::connect_timeout(&addr, DOCTOR_IO_TIMEOUT)
            .map_err(|error| format!("connect {addr}: {error}"))?;
        stream
            .set_read_timeout(Some(DOCTOR_IO_TIMEOUT))
            .map_err(|error| format!("set read timeout: {error}"))?;
        stream
            .set_write_timeout(Some(DOCTOR_IO_TIMEOUT))
            .map_err(|error| format!("set write timeout: {error}"))?;
        Ok(Self {
            stream,
            target_net_id,
            target_port: config.ads_port,
            source_net_id,
            source_port,
            max_frame_bytes: config.max_frame_bytes,
            next_invoke: 1,
        })
    }

    fn read_state(&mut self) -> Result<(), String> {
        let response = self.request(CommandId::ReadState, Vec::new())?;
        expect_payload_result(&response.payload, AdsErrorCode::NoError)?;
        if response.payload.len() != 8 {
            return Err(format!(
                "ReadState response had {} bytes, expected 8.",
                response.payload.len()
            ));
        }
        Ok(())
    }

    fn handle_by_name(&mut self, symbol: &str) -> Result<u32, String> {
        let mut write_data = symbol.as_bytes().to_vec();
        write_data.push(0);
        let mut payload = Vec::new();
        payload.extend_from_slice(&ADSIGRP_SYM_HNDBYNAME.to_le_bytes());
        payload.extend_from_slice(&0_u32.to_le_bytes());
        payload.extend_from_slice(&4_u32.to_le_bytes());
        payload.extend_from_slice(&(write_data.len() as u32).to_le_bytes());
        payload.extend(write_data);
        let response = self.request(CommandId::ReadWrite, payload)?;
        let data = expect_read_payload(&response.payload, 4)?;
        Ok(u32::from_le_bytes(data.try_into().map_err(|_| {
            "handle response was not four bytes".to_string()
        })?))
    }

    fn direct_read(&mut self, symbol: &SymbolDescriptor) -> Result<Vec<u8>, String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&symbol.byte_size.to_le_bytes());
        let response = self.request(CommandId::Read, payload)?;
        expect_read_payload(&response.payload, symbol.byte_size as usize)
    }

    fn direct_read_expect_denied(&mut self, symbol: &SymbolDescriptor) -> Result<(), String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&symbol.byte_size.to_le_bytes());
        let response = self.request(CommandId::Read, payload)?;
        expect_payload_result(&response.payload, AdsErrorCode::AccessDenied)
    }

    fn sumup_read(&mut self, symbol: &SymbolDescriptor) -> Result<(), String> {
        let mut item = Vec::new();
        item.extend_from_slice(&symbol.index_group.to_le_bytes());
        item.extend_from_slice(&symbol.index_offset.to_le_bytes());
        item.extend_from_slice(&symbol.byte_size.to_le_bytes());
        let mut payload = Vec::new();
        payload.extend_from_slice(&ADSIGRP_SUMUP_READ.to_le_bytes());
        payload.extend_from_slice(&1_u32.to_le_bytes());
        payload.extend_from_slice(&(4 + symbol.byte_size).to_le_bytes());
        payload.extend_from_slice(&(item.len() as u32).to_le_bytes());
        payload.extend(item);
        let response = self.request(CommandId::ReadWrite, payload)?;
        let data = expect_read_payload(&response.payload, 4 + symbol.byte_size as usize)?;
        expect_payload_result(&data[..4], AdsErrorCode::NoError)?;
        if data.len() != 4 + symbol.byte_size as usize {
            return Err("sum-up read returned an unexpected byte count".to_string());
        }
        Ok(())
    }

    fn notification(&mut self, symbol: &SymbolDescriptor) -> Result<(), String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&symbol.byte_size.to_le_bytes());
        payload.extend_from_slice(&ADSTRANS_SERVERCYCLE.to_le_bytes());
        payload.extend_from_slice(&50_u32.to_le_bytes());
        payload.extend_from_slice(&50_u32.to_le_bytes());
        payload.extend_from_slice(&[0_u8; 16]);
        let response = self.request(CommandId::AddDeviceNotification, payload)?;
        let handle = expect_add_notification(&response.payload)?;
        let notification = self.read_frame()?;
        if notification.header.command_id != CommandId::DeviceNotification {
            return Err(format!(
                "expected DeviceNotification, got {:?}",
                notification.header.command_id
            ));
        }
        if !device_notification_has_handle(&notification.payload, handle) {
            return Err("DeviceNotification did not contain the registered handle".to_string());
        }
        Ok(())
    }

    fn write(&mut self, symbol: &SymbolDescriptor, bytes: &[u8]) -> Result<(), String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        payload.extend_from_slice(bytes);
        let response = self.request(CommandId::Write, payload)?;
        expect_payload_result(&response.payload, AdsErrorCode::NoError)
    }

    fn write_expect_denied(
        &mut self,
        symbol: &SymbolDescriptor,
        bytes: &[u8],
    ) -> Result<(), String> {
        let mut payload = Vec::new();
        payload.extend_from_slice(&symbol.index_group.to_le_bytes());
        payload.extend_from_slice(&symbol.index_offset.to_le_bytes());
        payload.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        payload.extend_from_slice(bytes);
        let response = self.request(CommandId::Write, payload)?;
        let result = read_u32(&response.payload, 0)?;
        if result == AdsErrorCode::InvalidAccess.value()
            || result == AdsErrorCode::AccessDenied.value()
        {
            Ok(())
        } else {
            Err(format!(
                "write guard returned 0x{result:04X}, expected access denial"
            ))
        }
    }

    fn request(&mut self, command_id: CommandId, payload: Vec<u8>) -> Result<AmsTcpFrame, String> {
        let frame = self.request_frame(command_id, payload)?;
        let invoke_id = frame.header.invoke_id;
        self.stream
            .write_all(&frame.to_bytes().map_err(|error| error.to_string())?)
            .map_err(|error| format!("write ADS request: {error}"))?;

        for _ in 0..8 {
            let response = self.read_frame()?;
            if response.header.command_id == CommandId::DeviceNotification {
                continue;
            }
            if response.header.command_id != command_id {
                return Err(format!(
                    "response command mismatch: got {:?}, expected {:?}",
                    response.header.command_id, command_id
                ));
            }
            if response.header.invoke_id != invoke_id {
                return Err(format!(
                    "response invoke id mismatch: got {}, expected {}",
                    response.header.invoke_id, invoke_id
                ));
            }
            if response.header.error_code != 0 {
                return Err(format!(
                    "AMS header error 0x{:04X}",
                    response.header.error_code
                ));
            }
            return Ok(response);
        }
        Err(format!(
            "timed out waiting for {:?} response after notification frames",
            command_id
        ))
    }

    fn request_frame(
        &mut self,
        command_id: CommandId,
        payload: Vec<u8>,
    ) -> Result<AmsTcpFrame, String> {
        let data_length = u32::try_from(payload.len())
            .map_err(|_| "ADS request payload length does not fit u32".to_string())?;
        let frame = AmsTcpFrame {
            header: AmsHeader {
                target_net_id: self.target_net_id,
                target_port: self.target_port,
                source_net_id: self.source_net_id,
                source_port: self.source_port,
                command_id,
                state: AmsState::Request,
                data_length,
                error_code: 0,
                invoke_id: self.next_invoke,
            },
            payload,
        };
        self.next_invoke = self.next_invoke.wrapping_add(1).max(1);
        Ok(frame)
    }

    fn read_frame(&mut self) -> Result<AmsTcpFrame, String> {
        let mut prefix = [0_u8; AMS_TCP_HEADER_LEN];
        self.stream
            .read_exact(&mut prefix)
            .map_err(|error| format!("read AMS/TCP prefix: {error}"))?;
        let ams_len = u32::from_le_bytes([prefix[2], prefix[3], prefix[4], prefix[5]]) as usize;
        if ams_len < AMS_HEADER_LEN || ams_len > self.max_frame_bytes {
            return Err(format!("invalid AMS/TCP length {ams_len}"));
        }
        let mut bytes = Vec::from(prefix);
        bytes.resize(AMS_TCP_HEADER_LEN + ams_len, 0);
        self.stream
            .read_exact(&mut bytes[AMS_TCP_HEADER_LEN..])
            .map_err(|error| format!("read AMS/TCP payload: {error}"))?;
        AmsTcpFrame::parse(&bytes, self.max_frame_bytes).map_err(|error| error.to_string())
    }
}

fn expect_payload_result(payload: &[u8], expected: AdsErrorCode) -> Result<(), String> {
    let result = read_u32(payload, 0)?;
    if result == expected.value() {
        Ok(())
    } else {
        Err(format!(
            "ADS result 0x{result:04X}, expected 0x{:04X}",
            expected.value()
        ))
    }
}

fn expect_read_payload(payload: &[u8], expected_len: usize) -> Result<Vec<u8>, String> {
    expect_payload_result(payload, AdsErrorCode::NoError)?;
    let len = read_u32(payload, 4)? as usize;
    if len != expected_len {
        return Err(format!("ADS read length {len}, expected {expected_len}"));
    }
    let data = payload
        .get(8..8 + len)
        .ok_or_else(|| "ADS read response was truncated".to_string())?;
    Ok(data.to_vec())
}

fn expect_add_notification(payload: &[u8]) -> Result<u32, String> {
    expect_payload_result(payload, AdsErrorCode::NoError)?;
    read_u32(payload, 4)
}

fn device_notification_has_handle(payload: &[u8], expected_handle: u32) -> bool {
    let Ok(stream_len) = read_u32(payload, 0) else {
        return false;
    };
    if stream_len as usize + 4 > payload.len() {
        return false;
    }
    let Ok(stamp_count) = read_u32(payload, 4) else {
        return false;
    };
    let mut offset = 8_usize;
    for _ in 0..stamp_count {
        if payload.get(offset..offset + 12).is_none() {
            return false;
        }
        offset += 8;
        let Ok(sample_count) = read_u32(payload, offset) else {
            return false;
        };
        offset += 4;
        for _ in 0..sample_count {
            let Ok(handle) = read_u32(payload, offset) else {
                return false;
            };
            let Ok(sample_len) = read_u32(payload, offset + 4) else {
                return false;
            };
            offset += 8;
            if handle == expected_handle {
                return true;
            }
            let Some(next) = offset.checked_add(sample_len as usize) else {
                return false;
            };
            if next > payload.len() {
                return false;
            }
            offset = next;
        }
    }
    false
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let slice = bytes
        .get(offset..offset + 4)
        .ok_or_else(|| format!("expected u32 at byte {offset}"))?;
    Ok(u32::from_le_bytes(slice.try_into().map_err(|_| {
        format!("expected four bytes at byte {offset}")
    })?))
}
