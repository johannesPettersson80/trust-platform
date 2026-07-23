//! ADS server doctor and production evidence.

mod loopback_client;
#[cfg(test)]
mod tests;

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use trust_ads_core::{AmsNetId, SymbolDescriptor, SymbolFlag};
use trust_ads_server::{ams_net_id_text_to_bytes, AmsParseError, AmsTcpFrame};

use crate::ads::diagnostics::{
    build_server_production_evidence, AdsConnectionStatus, AdsConnectionStatusState,
    AdsStatusOverall, AdsStatusReport, DiagnosticTransport, DoctorReport, DoctorRole,
    DoctorSkipReason, DoctorStep, DoctorStepId, DoctorStepStatus, DoctorVantage, LocalIdentity,
    NextAction, NextActionKind, ProductionEvidence, ServerProductionEvidenceInput,
};
use crate::ads::onboarding::classify_local_address;
use crate::debug::DebugSnapshot;

use super::{AdsServerRuntime, AdsServerRuntimeConfig, AdsServerSourcePin};
#[cfg(test)]
use loopback_client::expect_add_notification;
use loopback_client::LoopbackAdsClient;

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
    /// Latest caller-observed runtime snapshot.
    ///
    /// This observation cannot establish live symbol-service or production
    /// evidence; the Doctor reads those only from the running ADS server.
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
    let symbol_snapshot = input.runtime.map(AdsServerRuntime::symbol_snapshot);
    let status =
        build_ads_server_status_report(input.config, symbol_snapshot.as_deref(), input.runtime);
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
        return finalize_report(input, local, steps, symbol_snapshot, status, false);
    }

    steps.push(bind_exposure_step(input.config));
    steps.push(listener_bound_step(input.runtime));

    let discoverable = input
        .runtime
        .is_some_and(|runtime| udp_identify_step(runtime, input.config, &mut steps));

    match symbol_snapshot.as_deref() {
        Some(snapshot) if !snapshot.symbols.is_empty() => steps.push(
            DoctorStep::new(
                DoctorStepId::SymbolsExposed,
                "Symbols exposed",
                DoctorStepStatus::Pass,
                format!("{} ADS symbol(s) are exposed.", snapshot.symbols.len()),
            )
            .with_evidence("symbol_count", json!(snapshot.symbols.len())),
        ),
        Some(_) => steps.push(
            DoctorStep::failed(
                DoctorStepId::SymbolsExposed,
                "Symbols exposed",
                "The live ADS server symbol table is empty.",
                failure(NextActionKind::ConfigureExpose),
            )
            .with_evidence("symbol_count", json!(0)),
        ),
        None => steps.push(DoctorStep::failed(
            DoctorStepId::SymbolsExposed,
            "Symbols exposed",
            "ADS server runtime is not running; no live symbol table is available.",
            failure(NextActionKind::RerunDoctor),
        )),
    }

    steps.push(clients_allowed_step(input.config));

    if let Some(snapshot) = symbol_snapshot.as_deref() {
        steps.push(symbol_serve_step(snapshot.symbols.first()));
    } else {
        steps.push(DoctorStep::skipped(
            DoctorStepId::SymbolServe,
            "Symbol service",
            DoctorSkipReason::BlockedByPreviousStep,
            "Symbol service check is blocked because no live ADS server symbol table is available.",
        ));
    }

    if let (Some(runtime), Some(snapshot)) = (input.runtime, symbol_snapshot.as_deref()) {
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

    finalize_report(input, local, steps, symbol_snapshot, status, discoverable)
}

fn finalize_report(
    input: AdsServerDoctorInput<'_>,
    local: LocalIdentity,
    steps: Vec<DoctorStep>,
    symbol_snapshot: Option<std::sync::Arc<trust_ads_core::SymbolSnapshot>>,
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
        if let Ok(evidence) =
            build_server_evidence(input, &local, snapshot.as_ref(), &status, discoverable)
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
    let external_client = input
        .external_client
        .as_ref()
        .filter(|evidence| external_client_evidence_is_valid(evidence));
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
        external_client_verified: external_client.is_some(),
        external_client_kind: external_client.map(|evidence| evidence.kind.as_str()),
        external_client_name: external_client.map(|evidence| evidence.name.as_str()),
        external_client_timestamp_ms: external_client.map(|evidence| evidence.timestamp_ms),
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
        Some(evidence) if external_client_evidence_is_valid(evidence) => DoctorStep::new(
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
        Some(_) => DoctorStep::failed(
            DoctorStepId::ExternalClientVerified,
            "External client verified",
            "External client proof kind and name must both contain non-whitespace text.",
            failure(NextActionKind::WaitForClient),
        ),
        None => DoctorStep::skipped(
            DoctorStepId::ExternalClientVerified,
            "External client verified",
            DoctorSkipReason::ExternalClientPending,
            "Loopback self-test passed only inside truST; an independent ADS client must still verify the server.",
        )
        .with_next_action(NextAction::new(NextActionKind::WaitForClient)),
    }
}

fn external_client_evidence_is_valid(evidence: &AdsServerExternalClientEvidence) -> bool {
    !evidence.kind.trim().is_empty() && !evidence.name.trim().is_empty()
}

/// Builds the ADS server status report used by control/web surfaces.
#[must_use]
pub fn build_ads_server_status_report(
    config: &AdsServerRuntimeConfig,
    snapshot: Option<&trust_ads_core::SymbolSnapshot>,
    runtime: Option<&AdsServerRuntime>,
) -> AdsStatusReport {
    let point_count = runtime.map_or_else(
        || snapshot.map_or(0, |snapshot| snapshot.symbols.len()),
        AdsServerRuntime::symbol_count,
    );
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
