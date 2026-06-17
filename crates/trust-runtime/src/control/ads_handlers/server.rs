use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde_json::json;

use crate::ads::diagnostics::{
    AdsConnectionStatus, AdsConnectionStatusState, AdsStatusOverall, AdsStatusReport, DoctorRole,
    LocalIdentity, LocalNetworkClassification, ADS_DIAGNOSTICS_SCHEMA_VERSION,
};
use crate::ads::onboarding::{build_route_plan, RoutePlanRequest, RoutePlanRole};
use crate::ads::server::AdsServerRuntimeConfig;
#[cfg(feature = "ads-server")]
use crate::ads::server::{build_runtime_symbol_snapshot, AdsServerExternalClientEvidence};
use crate::scheduler::ResourceCommand;

use super::super::{ControlResponse, ControlState};

const ADS_SERVER_CONTROL_TIMEOUT: Duration = Duration::from_millis(250);
const ADS_SERVER_DOCTOR_STEPS: usize = 14;

pub(in crate::control) fn handle_ads_server_status(
    id: u64,
    state: &ControlState,
) -> ControlResponse {
    match build_status_surface(state) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(id, error),
    }
}

pub(in crate::control) fn handle_ads_server_symbols(
    id: u64,
    state: &ControlState,
) -> ControlResponse {
    let config = config_from_state(state);
    let snapshot = match snapshot_from_resource(state) {
        Ok(snapshot) => snapshot,
        Err(error) => return ControlResponse::error(id, error),
    };
    #[cfg(not(feature = "ads-server"))]
    {
        let _ = (config, snapshot);
        return ControlResponse::error(
            id,
            "ADS server symbols require trust-runtime built with feature 'ads-server'".to_string(),
        );
    }
    #[cfg(feature = "ads-server")]
    match build_runtime_symbol_snapshot(&config, &snapshot) {
        Ok(symbols) => match serde_json::to_value(symbols) {
            Ok(value) => ControlResponse::ok(id, value),
            Err(error) => ControlResponse::error(
                id,
                format!("ADS server symbols serialization failed: {error}"),
            ),
        },
        Err(error) => ControlResponse::error(id, format!("ADS server symbols failed: {error}")),
    }
}

pub(in crate::control) fn handle_ads_server_doctor(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match parse_server_doctor_params(id, params) {
        Ok(params) => params,
        Err(response) => return response,
    };
    match run_server_doctor_for_control(state, params) {
        Ok(report) => {
            if let Ok(mut jobs) = state.ads_doctor_jobs.lock() {
                let _ = jobs.record_completed(
                    "ads-server-doctor",
                    ADS_SERVER_DOCTOR_STEPS,
                    report.clone(),
                );
            }
            match serde_json::to_value(report) {
                Ok(value) => ControlResponse::ok(id, value),
                Err(error) => ControlResponse::error(
                    id,
                    format!("ADS server doctor report serialization failed: {error}"),
                ),
            }
        }
        Err(error) => ControlResponse::error(id, error),
    }
}

pub(in crate::control) fn handle_ads_server_doctor_start(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match parse_server_doctor_params(id, params) {
        Ok(params) => params,
        Err(response) => return response,
    };
    let mut jobs = match state.ads_doctor_jobs.lock() {
        Ok(jobs) => jobs,
        Err(_) => return ControlResponse::error(id, "ADS doctor job store poisoned".to_string()),
    };
    let status = jobs.start("ads-server-doctor", ADS_SERVER_DOCTOR_STEPS);
    let job_id = status.job_id.clone();
    drop(jobs);

    let jobs = Arc::clone(&state.ads_doctor_jobs);
    let control_state = state_for_thread(state);
    thread::spawn(move || {
        let result = run_server_doctor_for_parts(&control_state, params);
        if let Ok(mut jobs) = jobs.lock() {
            match result {
                Ok(report) => {
                    let _ = jobs.complete(job_id.as_str(), report);
                }
                Err(error) => {
                    let _ = jobs.fail(job_id.as_str(), error);
                }
            }
        }
    });

    match serde_json::to_value(status) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(
            id,
            format!("ADS server doctor job serialization failed: {error}"),
        ),
    }
}

pub(in crate::control) fn handle_ads_server_doctor_status(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params: DoctorStatusParams = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let jobs = match state.ads_doctor_jobs.lock() {
        Ok(jobs) => jobs,
        Err(_) => return ControlResponse::error(id, "ADS doctor job store poisoned".to_string()),
    };
    match jobs.get(params.job_id.as_str()) {
        Some(status) => match serde_json::to_value(status) {
            Ok(value) => ControlResponse::ok(id, value),
            Err(error) => ControlResponse::error(
                id,
                format!("ADS server doctor job serialization failed: {error}"),
            ),
        },
        None => ControlResponse::error_with_code(
            id,
            format!("unknown ADS server doctor job '{}'", params.job_id),
            "ads_server_doctor_job_not_found",
        ),
    }
}

pub(in crate::control) fn handle_ads_server_route_plan(
    id: u64,
    params: Option<serde_json::Value>,
    _state: &ControlState,
) -> ControlResponse {
    let mut params: RoutePlanRequest = match params {
        Some(value) => match serde_json::from_value(value) {
            Ok(parsed) => parsed,
            Err(error) => return ControlResponse::error(id, format!("invalid params: {error}")),
        },
        None => return ControlResponse::error(id, "missing params".into()),
    };
    params.role = RoutePlanRole::Server;
    match serde_json::to_value(build_route_plan(params)) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(
            id,
            format!("ADS server route plan serialization failed: {error}"),
        ),
    }
}

pub(in crate::control) fn refresh_ads_server_runtime_after_online_change(
    state: &ControlState,
) -> Result<(), String> {
    #[cfg(not(feature = "ads-server"))]
    {
        let config = config_from_state(state);
        if config.enabled {
            return Err(
                "ADS server refresh requires trust-runtime built with feature 'ads-server'"
                    .to_string(),
            );
        }
        return Ok(());
    }

    #[cfg(feature = "ads-server")]
    {
        let config = config_from_state(state);
        if !config.enabled {
            return Ok(());
        }

        let snapshot_provider = snapshot_provider_from_state(state);
        let snapshot = snapshot_provider()
            .ok_or_else(|| "ADS server refresh failed: runtime snapshot unavailable".to_string())?;
        {
            let guard = state
                .ads_server_runtime
                .lock()
                .map_err(|_| "ADS server runtime lock poisoned".to_string())?;
            if let Some(server) = guard.as_ref() {
                server
                    .refresh_symbols(&config, &snapshot)
                    .map_err(|error| format!("ADS server symbol refresh failed: {error}"))?;
                return Ok(());
            }
        }

        let restarted = crate::ads::server::start_ads_server_runtime(
            state.resource_name.as_str(),
            &config,
            state.debug.clone(),
            state.resource.clone(),
            snapshot_provider,
            state.audit_tx.clone(),
        )
        .map_err(|error| format!("ADS server refresh failed: {error}"))?;

        let mut guard = state
            .ads_server_runtime
            .lock()
            .map_err(|_| "ADS server runtime lock poisoned".to_string())?;
        *guard = restarted;
        Ok(())
    }
}

#[derive(Clone)]
#[cfg_attr(not(feature = "ads-server"), allow(dead_code))]
struct ServerControlThreadState {
    resource_name: String,
    resource: crate::scheduler::ResourceControl<crate::scheduler::StdClock>,
    ads_server_config: Arc<std::sync::Mutex<Option<AdsServerRuntimeConfig>>>,
    #[cfg(feature = "ads-server")]
    ads_server_runtime: Arc<std::sync::Mutex<Option<crate::ads::server::AdsServerRuntime>>>,
}

fn state_for_thread(state: &ControlState) -> ServerControlThreadState {
    ServerControlThreadState {
        resource_name: state.resource_name.to_string(),
        resource: state.resource.clone(),
        ads_server_config: Arc::clone(&state.ads_server_config),
        #[cfg(feature = "ads-server")]
        ads_server_runtime: Arc::clone(&state.ads_server_runtime),
    }
}

fn run_server_doctor_for_control(
    state: &ControlState,
    params: ServerDoctorControlParams,
) -> Result<crate::ads::diagnostics::DoctorReport, String> {
    let state = state_for_thread(state);
    run_server_doctor_for_parts(&state, params)
}

fn run_server_doctor_for_parts(
    state: &ServerControlThreadState,
    params: ServerDoctorControlParams,
) -> Result<crate::ads::diagnostics::DoctorReport, String> {
    #[cfg(not(feature = "ads-server"))]
    {
        let _ = (state, params);
        return Err(
            "ADS server doctor requires trust-runtime built with feature 'ads-server'".to_string(),
        );
    }

    #[cfg(feature = "ads-server")]
    {
        let config = state
            .ads_server_config
            .lock()
            .map_err(|_| "ADS server config lock poisoned".to_string())?
            .clone()
            .unwrap_or_default();
        let snapshot = snapshot_from_resource_parts(&state.resource)?;
        let runtime = state
            .ads_server_runtime
            .lock()
            .map_err(|_| "ADS server runtime lock poisoned".to_string())?;
        Ok(crate::ads::server::run_ads_server_doctor(
            crate::ads::server::AdsServerDoctorInput {
                resource_name: state.resource_name.as_str(),
                config: &config,
                snapshot: &snapshot,
                runtime: runtime.as_ref(),
                now_ms: now_ms(),
                external_client: params.external_client,
                deployed_config_text: params.deployed_config_text.as_deref(),
            },
        ))
    }
}

fn build_status_surface(state: &ControlState) -> Result<serde_json::Value, String> {
    let config = config_from_state(state);
    let snapshot = snapshot_from_resource(state)?;
    #[cfg(not(feature = "ads-server"))]
    let _ = &snapshot;
    #[cfg(feature = "ads-server")]
    let symbols = build_runtime_symbol_snapshot(&config, &snapshot).ok();
    #[cfg(not(feature = "ads-server"))]
    let symbols: Option<trust_ads_core::SymbolSnapshot> = None;
    let exposed_count = symbols
        .as_ref()
        .map_or(0, |snapshot| snapshot.symbols.len());
    let writable_count = symbols.as_ref().map_or(0, |snapshot| {
        snapshot
            .symbols
            .iter()
            .filter(|symbol| symbol.flags.contains(&trust_ads_core::SymbolFlag::Write))
            .count()
    });
    let status = ads_server_status_from_state(state, &config, symbols.as_ref());
    let server_running = ads_server_runtime_is_running(state);
    let connected_clients = connected_clients_from_state(state);
    let recently_refused_clients = recently_refused_clients_from_state(state);
    let pending_clients = recently_refused_clients.clone();
    let configured_empty = config.expose.is_empty() || config.clients.is_empty();
    let latest_doctor = latest_server_doctor_report(state);
    let latest_evidence = latest_doctor
        .as_ref()
        .and_then(|report| report.evidence.as_ref());
    let production_ready = latest_doctor
        .as_ref()
        .is_some_and(|report| report.production_ready);
    let external_client_verified =
        latest_evidence.is_some_and(|evidence| evidence.external_client_verified);
    let external_client_kind =
        latest_evidence.and_then(|evidence| evidence.external_client_kind.as_deref());
    let external_client_name =
        latest_evidence.and_then(|evidence| evidence.external_client_name.as_deref());
    Ok(json!({
        "schema_version": ADS_DIAGNOSTICS_SCHEMA_VERSION,
        "role": "server",
        "status": status,
        "identity": local_identity_from_config(&config, state.resource_name.as_str()),
        "enabled": config.enabled,
        "listen": config.listen.as_deref(),
        "ams_net_id": config.ams_net_id.as_ref().map(|value| value.0.as_str()),
        "ads_port": config.ads_port,
        "exposed_count": exposed_count,
        "writable_count": writable_count,
        "allowed_client_count": config.clients.len(),
        "connected_clients": connected_clients,
        "recently_refused_clients": recently_refused_clients,
        "pending_clients": pending_clients,
        "discoverable": server_running,
        "production_ready": production_ready,
        "external_client_verified": external_client_verified,
        "external_client_kind": external_client_kind,
        "external_client_name": external_client_name,
        "configured_empty": configured_empty,
        "proof_status": server_proof_status(&status, latest_doctor.as_ref()),
    }))
}

fn latest_server_doctor_report(
    state: &ControlState,
) -> Option<crate::ads::diagnostics::DoctorReport> {
    state
        .ads_doctor_jobs
        .lock()
        .ok()
        .and_then(|jobs| jobs.latest_completed_report(DoctorRole::Server))
}

fn connected_clients_from_state(state: &ControlState) -> Option<usize> {
    #[cfg(feature = "ads-server")]
    {
        state
            .ads_server_runtime
            .lock()
            .ok()
            .and_then(|runtime| runtime.as_ref().map(|runtime| runtime.connected_clients()))
    }
    #[cfg(not(feature = "ads-server"))]
    {
        let _ = state;
        None
    }
}

fn server_proof_status(
    status: &AdsStatusReport,
    latest_doctor: Option<&crate::ads::diagnostics::DoctorReport>,
) -> &'static str {
    if latest_doctor.is_some_and(|report| report.production_ready) {
        return "production_ready";
    }
    if latest_doctor
        .and_then(|report| report.evidence.as_ref())
        .is_some_and(|evidence| evidence.external_client_verified)
    {
        return "external_client_verified";
    }
    if status.overall == AdsStatusOverall::Healthy {
        "self_test_available"
    } else {
        "not_ready"
    }
}

fn ads_server_runtime_is_running(state: &ControlState) -> bool {
    #[cfg(feature = "ads-server")]
    {
        state
            .ads_server_runtime
            .lock()
            .is_ok_and(|runtime| runtime.is_some())
    }
    #[cfg(not(feature = "ads-server"))]
    {
        let _ = state;
        false
    }
}

fn recently_refused_clients_from_state(state: &ControlState) -> serde_json::Value {
    #[cfg(feature = "ads-server")]
    {
        if let Ok(runtime) = state.ads_server_runtime.lock() {
            if let Some(runtime) = runtime.as_ref() {
                return serde_json::to_value(runtime.policy().recently_refused_clients())
                    .unwrap_or_else(|_| json!([]));
            }
        }
    }
    #[cfg(not(feature = "ads-server"))]
    let _ = state;
    json!([])
}

fn ads_server_status_from_state(
    state: &ControlState,
    config: &AdsServerRuntimeConfig,
    symbols: Option<&trust_ads_core::SymbolSnapshot>,
) -> AdsStatusReport {
    #[cfg(not(feature = "ads-server"))]
    let _ = state;

    #[cfg(feature = "ads-server")]
    {
        if let Ok(runtime) = state.ads_server_runtime.lock() {
            return crate::ads::server::build_ads_server_status_report(
                config,
                symbols,
                runtime.as_ref(),
            );
        }
    }
    let point_count = symbols.map_or(0, |snapshot| snapshot.symbols.len());
    AdsStatusReport {
        schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        role: DoctorRole::Server,
        overall: if config.enabled {
            AdsStatusOverall::Unknown
        } else {
            AdsStatusOverall::Disabled
        },
        runtime_identity_hash: None,
        deployed_ads_config_hash: None,
        connections: vec![AdsConnectionStatus {
            name: "ads-server".to_string(),
            target: None,
            state: if config.enabled {
                AdsConnectionStatusState::Unknown
            } else {
                AdsConnectionStatusState::Disabled
            },
            point_count,
            degraded_points: 0,
            last_good_value_ms: None,
            symbol_version: None,
            summary: format!("ADS server exposes {point_count} symbol(s)."),
        }],
        summary: "ADS server runtime status is unavailable.".to_string(),
    }
}

fn config_from_state(state: &ControlState) -> AdsServerRuntimeConfig {
    state
        .ads_server_config
        .lock()
        .ok()
        .and_then(|config| config.clone())
        .unwrap_or_default()
}

fn snapshot_from_resource(state: &ControlState) -> Result<crate::debug::DebugSnapshot, String> {
    snapshot_from_resource_parts(&state.resource)
}

#[cfg(feature = "ads-server")]
fn snapshot_provider_from_state(
    state: &ControlState,
) -> Arc<dyn Fn() -> Option<crate::debug::DebugSnapshot> + Send + Sync> {
    let snapshot_control = state.resource.clone();
    Arc::new(move || snapshot_from_resource_parts(&snapshot_control).ok())
}

fn snapshot_from_resource_parts(
    resource: &crate::scheduler::ResourceControl<crate::scheduler::StdClock>,
) -> Result<crate::debug::DebugSnapshot, String> {
    let (tx, rx) = mpsc::channel();
    resource
        .send_command(ResourceCommand::Snapshot { respond_to: tx })
        .map_err(|error| format!("ADS server snapshot request failed: {error}"))?;
    rx.recv_timeout(ADS_SERVER_CONTROL_TIMEOUT)
        .map_err(|error| format!("ADS server snapshot request timed out: {error}"))
}

fn local_identity_from_config(
    config: &AdsServerRuntimeConfig,
    resource_name: &str,
) -> LocalIdentity {
    let chosen_ip = config.listen.as_deref().unwrap_or("127.0.0.1").to_string();
    LocalIdentity {
        host_name: Some(resource_name.to_string()),
        ams_net_id: config
            .ams_net_id
            .as_ref()
            .map(|value| value.0.clone())
            .unwrap_or_else(|| format!("{chosen_ip}.1.1")),
        nic: None,
        candidates: Vec::new(),
        classification: classify_server_identity(&chosen_ip),
        chosen_ip,
    }
}

fn classify_server_identity(ip: &str) -> LocalNetworkClassification {
    crate::ads::onboarding::classify_local_address(ip, None)
}

fn parse_server_doctor_params(
    id: u64,
    params: Option<serde_json::Value>,
) -> Result<ServerDoctorControlParams, ControlResponse> {
    match params {
        Some(value) => serde_json::from_value(value)
            .map_err(|error| ControlResponse::error(id, format!("invalid params: {error}"))),
        None => Ok(ServerDoctorControlParams::default()),
    }
}

#[cfg_attr(not(feature = "ads-server"), allow(dead_code))]
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(u128::from(u64::MAX)) as u64
}

#[derive(Debug, Default, Deserialize)]
#[cfg_attr(not(feature = "ads-server"), allow(dead_code))]
struct ServerDoctorControlParams {
    #[serde(default)]
    #[cfg(feature = "ads-server")]
    external_client: Option<AdsServerExternalClientEvidence>,
    #[serde(default)]
    #[cfg(not(feature = "ads-server"))]
    external_client: Option<serde_json::Value>,
    #[serde(default)]
    deployed_config_text: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DoctorStatusParams {
    job_id: String,
}
