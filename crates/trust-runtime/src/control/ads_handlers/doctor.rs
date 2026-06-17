use std::collections::BTreeMap;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ads::diagnostics::{
    DiagnosticTransport, DoctorReport, DoctorRole, DoctorVantage, LocalIdentity,
    ProductionEvidence, TargetIdentity,
};
use crate::ads::onboarding::{
    derive_runtime_identity_from_source, resolve_os_source_ip,
    runtime_address_candidates_from_interfaces, ActiveAdsDeviceSnapshot, ActiveDeviceStrategy,
    DoctorCancellation, DoctorJobProgress, DoctorOptions, IdentityRequest,
};
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};

use super::super::{ControlResponse, ControlState};

const ACTIVE_DEVICE_TIMEOUT: Duration = Duration::from_millis(250);

#[derive(Debug, Default)]
pub struct AdsDoctorJobStore {
    next_id: u64,
    jobs: BTreeMap<String, AdsDoctorJobStatus>,
}

impl AdsDoctorJobStore {
    pub(super) fn start(&mut self, prefix: &str, total_steps: usize) -> AdsDoctorJobStatus {
        self.next_id += 1;
        let status = AdsDoctorJobStatus {
            job_id: format!("{prefix}-{}", self.next_id),
            state: AdsDoctorControlJobState::Running,
            progress: DoctorJobProgress::new(total_steps),
            report: None,
            error: None,
        };
        self.jobs.insert(status.job_id.clone(), status.clone());
        status
    }

    pub(super) fn complete(
        &mut self,
        job_id: &str,
        report: DoctorReport,
    ) -> Option<AdsDoctorJobStatus> {
        let status = self.jobs.get_mut(job_id)?;
        status.state = AdsDoctorControlJobState::Complete;
        status.progress.completed_steps = status.progress.total_steps;
        status.progress.current_step = None;
        status.report = Some(report);
        status.error = None;
        Some(status.clone())
    }

    pub(super) fn record_completed(
        &mut self,
        prefix: &str,
        total_steps: usize,
        report: DoctorReport,
    ) -> AdsDoctorJobStatus {
        let status = self.start(prefix, total_steps);
        self.complete(status.job_id.as_str(), report)
            .unwrap_or(status)
    }

    pub(super) fn fail(&mut self, job_id: &str, error: String) -> Option<AdsDoctorJobStatus> {
        let status = self.jobs.get_mut(job_id)?;
        status.state = AdsDoctorControlJobState::Failed;
        status.progress.current_step = None;
        status.error = Some(error);
        Some(status.clone())
    }

    pub(super) fn get(&self, job_id: &str) -> Option<AdsDoctorJobStatus> {
        self.jobs.get(job_id).cloned()
    }

    pub(super) fn latest_completed_report(&self, role: DoctorRole) -> Option<DoctorReport> {
        self.jobs
            .iter()
            .filter_map(|(job_id, status)| {
                if status.state != AdsDoctorControlJobState::Complete {
                    return None;
                }
                let report = status.report.as_ref()?;
                if report.role != role {
                    return None;
                }
                Some((job_sequence(job_id), report))
            })
            .max_by_key(|(sequence, _)| *sequence)
            .map(|(_, report)| report.clone())
    }
}

fn job_sequence(job_id: &str) -> u64 {
    job_id
        .rsplit_once('-')
        .and_then(|(_, suffix)| suffix.parse::<u64>().ok())
        .unwrap_or(0)
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct AdsDoctorJobStatus {
    pub(super) job_id: String,
    state: AdsDoctorControlJobState,
    progress: DoctorJobProgress,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    report: Option<DoctorReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AdsDoctorControlJobState {
    Running,
    Complete,
    Failed,
}

pub(in crate::control) fn handle_ads_doctor(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match parse_doctor_params(id, params) {
        Ok(params) => params,
        Err(response) => return response,
    };
    match run_doctor_for_control(params, &state.resource) {
        Ok(report) => serialize_value(id, &report, "ADS doctor report"),
        Err(error) => ControlResponse::error(id, error),
    }
}

pub(in crate::control) fn handle_ads_doctor_start(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match parse_doctor_params(id, params) {
        Ok(params) => params,
        Err(response) => return response,
    };
    let mut jobs = match state.ads_doctor_jobs.lock() {
        Ok(jobs) => jobs,
        Err(_) => return ControlResponse::error(id, "ADS doctor job store poisoned".to_string()),
    };
    let status = jobs.start(
        "ads-doctor",
        crate::ads::onboarding::REQUIRED_DOCTOR_STEPS.len(),
    );
    let job_id = status.job_id.clone();
    drop(jobs);

    let jobs = Arc::clone(&state.ads_doctor_jobs);
    let resource = state.resource.clone();
    thread::spawn(move || {
        let result = run_doctor_for_control(params, &resource);
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

    serialize_value(id, &status, "ADS doctor job status")
}

pub(in crate::control) fn handle_ads_doctor_status(
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
        Some(status) => serialize_value(id, &status, "ADS doctor job status"),
        None => ControlResponse::error_with_code(
            id,
            format!("unknown ADS doctor job '{}'", params.job_id),
            "ads_doctor_job_not_found",
        ),
    }
}

fn run_doctor_for_control(
    params: DoctorControlParams,
    resource: &ResourceControl<StdClock>,
) -> Result<DoctorReport, String> {
    let options = build_doctor_options(params, resource)?;

    #[cfg(feature = "ads-wire")]
    {
        let mut wire = crate::ads::onboarding::AdsRsOnboardingWire::default();
        Ok(crate::ads::onboarding::run_doctor(
            &mut wire,
            options,
            &DoctorCancellation::new(),
        ))
    }

    #[cfg(not(feature = "ads-wire"))]
    {
        if options.active_device.is_none() {
            return Err("ADS doctor needs an ads-wire build for live network probes".to_string());
        }
        let mut wire = crate::ads::onboarding::MockAdsOnboardingWire::default();
        Ok(crate::ads::onboarding::run_doctor(
            &mut wire,
            options,
            &DoctorCancellation::new(),
        ))
    }
}

fn build_doctor_options(
    params: DoctorControlParams,
    resource: &ResourceControl<StdClock>,
) -> Result<DoctorOptions, String> {
    let local_identity = match params.local_identity.clone() {
        Some(identity) => identity,
        None => derive_local_identity(&params)?,
    };
    let target_for_active = active_target_identity(&params);
    let active_device = query_active_device(resource, &target_for_active, Some(&local_identity))?;

    let mut options = DoctorOptions {
        ran_from: params.ran_from.unwrap_or(DoctorVantage::RuntimeHost),
        transport: params.transport.unwrap_or(DiagnosticTransport::Plain),
        target_ip: params.target_ip,
        expected_target_ams_net_id: params.expected_target_ams_net_id,
        ams_port: params.ams_port.unwrap_or(851),
        local_identity: Some(local_identity),
        selected_symbol: params.selected_symbol,
        writes_enabled: params.writes_enabled,
        write_probe: None,
        active_device,
        active_device_strategy: params
            .active_device_strategy
            .unwrap_or(ActiveDeviceStrategy::ReadOnlyViaLiveStatus),
        production_evidence: params.production_evidence,
        live_deployed_status_available: params.live_deployed_status_available,
    };
    if let Some(target) = params.target_identity {
        let ams_net_id = target.ams_net_id.trim();
        if !ams_net_id.is_empty() {
            options.expected_target_ams_net_id = Some(ams_net_id.to_string());
        }
    }
    Ok(options)
}

fn derive_local_identity(params: &DoctorControlParams) -> Result<LocalIdentity, String> {
    let request = IdentityRequest {
        target_ip: params.target_ip.clone(),
        local_net_id_override: params.local_net_id_override.clone(),
    };
    let chosen_ip = resolve_os_source_ip(params.target_ip.as_str()).map_err(|error| {
        format!(
            "failed to derive runtime host ADS identity toward '{}': {error}",
            params.target_ip
        )
    })?;
    let candidates = runtime_address_candidates_from_interfaces().unwrap_or_default();
    let nic = candidates
        .iter()
        .find(|candidate| candidate.ip == chosen_ip)
        .and_then(|candidate| candidate.nic.clone());
    derive_runtime_identity_from_source(&request, chosen_ip, None, nic, candidates)
        .map_err(|error| error.to_string())
}

fn active_target_identity(params: &DoctorControlParams) -> TargetIdentity {
    params
        .target_identity
        .clone()
        .unwrap_or_else(|| TargetIdentity {
            name: None,
            ip: params.target_ip.clone(),
            ams_net_id: params
                .expected_target_ams_net_id
                .clone()
                .unwrap_or_default(),
            ams_port: params.ams_port.unwrap_or(851),
            tc_version: None,
        })
}

fn query_active_device(
    resource: &ResourceControl<StdClock>,
    target: &TargetIdentity,
    local: Option<&LocalIdentity>,
) -> Result<Option<ActiveAdsDeviceSnapshot>, String> {
    let (tx, rx) = mpsc::channel();
    resource
        .send_command(ResourceCommand::ActiveAdsDevice {
            target: target.clone(),
            local: local.cloned(),
            respond_to: tx,
        })
        .map_err(|error| format!("ADS active-device query failed: {error}"))?;
    rx.recv_timeout(ACTIVE_DEVICE_TIMEOUT)
        .map_err(|error| format!("ADS active-device query timed out: {error}"))
}

fn parse_doctor_params(
    id: u64,
    params: Option<serde_json::Value>,
) -> Result<DoctorControlParams, ControlResponse> {
    match params {
        Some(value) => serde_json::from_value(value)
            .map_err(|error| ControlResponse::error(id, format!("invalid params: {error}"))),
        None => Err(ControlResponse::error(id, "missing params".into())),
    }
}

fn serialize_value<T: Serialize>(id: u64, value: &T, label: &str) -> ControlResponse {
    match serde_json::to_value(value) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(id, format!("{label} serialization failed: {error}")),
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct DoctorControlParams {
    target_ip: String,
    #[serde(default)]
    target_identity: Option<TargetIdentity>,
    #[serde(default)]
    expected_target_ams_net_id: Option<String>,
    #[serde(default)]
    ams_port: Option<u16>,
    #[serde(default)]
    local_identity: Option<LocalIdentity>,
    #[serde(default)]
    local_net_id_override: Option<String>,
    #[serde(default)]
    selected_symbol: Option<String>,
    #[serde(default)]
    writes_enabled: bool,
    #[serde(default)]
    active_device_strategy: Option<ActiveDeviceStrategy>,
    #[serde(default)]
    production_evidence: Option<ProductionEvidence>,
    #[serde(default)]
    live_deployed_status_available: bool,
    #[serde(default)]
    ran_from: Option<DoctorVantage>,
    #[serde(default)]
    transport: Option<DiagnosticTransport>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct DoctorStatusParams {
    job_id: String,
}
