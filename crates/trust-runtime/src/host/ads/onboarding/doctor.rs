use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;
use trust_ads_core::{PointStatus, QualityState};

use crate::ads::diagnostics::{
    AdsConnectionStatusState, DiagnosticTransport, DoctorOverall, DoctorReport, DoctorSkipReason,
    DoctorStep, DoctorStepId, DoctorStepStatus, DoctorVantage, LocalIdentity, NextAction,
    NextActionKind, ProductionEvidence, TargetIdentity,
};

use super::errors::{OnboardingWireError, OnboardingWireErrorKind};
use super::wire::{AdsOnboardingWire, GuardedWriteProbe};

mod active_report;
mod route_evidence;

/// Ordered doctor steps required by the onboarding specification.
pub const REQUIRED_DOCTOR_STEPS: &[DoctorStepId] = &[
    DoctorStepId::UdpIdentify,
    DoctorStepId::LocalIdentity,
    DoctorStepId::Tcp48898,
    DoctorStepId::RoutePresent,
    DoctorStepId::AmsTarget,
    DoctorStepId::ReadState,
    DoctorStepId::SymbolUpload,
    DoctorStepId::HandleResolve,
    DoctorStepId::SumupRead,
    DoctorStepId::WriteGuarded,
    DoctorStepId::Notification,
    DoctorStepId::SymbolVersion,
];

/// Timeout budget for one doctor step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DoctorStepTimeout {
    /// Step that owns this timeout.
    pub step: DoctorStepId,
    /// Timeout in milliseconds.
    pub timeout_ms: u64,
}

const DEFAULT_STEP_TIMEOUTS: &[DoctorStepTimeout] = &[
    DoctorStepTimeout {
        step: DoctorStepId::UdpIdentify,
        timeout_ms: 3_000,
    },
    DoctorStepTimeout {
        step: DoctorStepId::Tcp48898,
        timeout_ms: 1_000,
    },
    DoctorStepTimeout {
        step: DoctorStepId::ReadState,
        timeout_ms: 5_000,
    },
    DoctorStepTimeout {
        step: DoctorStepId::SymbolUpload,
        timeout_ms: 5_000,
    },
    DoctorStepTimeout {
        step: DoctorStepId::HandleResolve,
        timeout_ms: 5_000,
    },
    DoctorStepTimeout {
        step: DoctorStepId::SumupRead,
        timeout_ms: 5_000,
    },
    DoctorStepTimeout {
        step: DoctorStepId::Notification,
        timeout_ms: 5_000,
    },
    DoctorStepTimeout {
        step: DoctorStepId::SymbolVersion,
        timeout_ms: 5_000,
    },
];

/// Default step timeouts owned and enforced by the engine, not by UI code.
pub fn default_step_timeouts() -> Vec<DoctorStepTimeout> {
    DEFAULT_STEP_TIMEOUTS.to_vec()
}

/// Strategy used when the doctor target is already connected by the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActiveDeviceStrategy {
    /// Use live runtime cache/status only and do not open a second AMS connection.
    ReadOnlyViaLiveStatus,
    /// Refuse the full doctor until a caller explicitly pauses the device.
    RequiresPause,
    /// Run the full doctor even when a live device overlaps.
    ///
    /// This is reserved for explicit pause/resume flows; it is never the default.
    FullAfterExplicitPause,
}

/// Live ADS connection snapshot used to avoid duplicate AMS connections.
#[derive(Debug, Clone, PartialEq)]
pub struct ActiveAdsDeviceSnapshot {
    pub connection_name: String,
    pub target: TargetIdentity,
    pub local: Option<LocalIdentity>,
    pub state: AdsConnectionStatusState,
    pub point_statuses: Vec<PointStatus>,
    pub symbol_version: Option<u32>,
}

impl ActiveAdsDeviceSnapshot {
    fn degraded_points(&self) -> usize {
        self.point_statuses
            .iter()
            .filter(|status| status.quality.state != QualityState::Good)
            .count()
    }

    fn last_good_value_ms(&self) -> Option<u64> {
        self.point_statuses
            .iter()
            .filter(|status| status.quality.state == QualityState::Good)
            .filter_map(|status| status.quality.last_update_ms)
            .max()
    }
}

/// Inputs for one doctor run.
#[derive(Debug, Clone)]
pub struct DoctorOptions {
    pub ran_from: DoctorVantage,
    pub transport: DiagnosticTransport,
    pub target_ip: String,
    pub expected_target_ams_net_id: Option<String>,
    pub ams_port: u16,
    pub local_identity: Option<LocalIdentity>,
    pub selected_symbol: Option<String>,
    pub writes_enabled: bool,
    pub write_probe: Option<GuardedWriteProbe>,
    pub active_device: Option<ActiveAdsDeviceSnapshot>,
    pub active_device_strategy: ActiveDeviceStrategy,
    pub production_evidence: Option<ProductionEvidence>,
    pub live_deployed_status_available: bool,
}

impl DoctorOptions {
    /// Creates doctor options for the normal production runtime-host vantage.
    pub fn runtime_host(target_ip: impl Into<String>, local_identity: LocalIdentity) -> Self {
        Self {
            ran_from: DoctorVantage::RuntimeHost,
            transport: DiagnosticTransport::Plain,
            target_ip: target_ip.into(),
            expected_target_ams_net_id: None,
            ams_port: 851,
            local_identity: Some(local_identity),
            selected_symbol: None,
            writes_enabled: false,
            write_probe: None,
            active_device: None,
            active_device_strategy: ActiveDeviceStrategy::ReadOnlyViaLiveStatus,
            production_evidence: None,
            live_deployed_status_available: false,
        }
    }

    pub fn with_expected_target_ams_net_id(mut self, ams_net_id: impl Into<String>) -> Self {
        self.expected_target_ams_net_id = Some(ams_net_id.into());
        self
    }

    pub fn with_selected_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.selected_symbol = Some(symbol.into());
        self
    }

    pub fn with_writes_enabled(mut self, writes_enabled: bool) -> Self {
        self.writes_enabled = writes_enabled;
        self
    }

    pub fn with_write_probe(mut self, write_probe: GuardedWriteProbe) -> Self {
        self.writes_enabled = true;
        self.write_probe = Some(write_probe);
        self
    }

    pub fn with_active_device(
        mut self,
        active_device: ActiveAdsDeviceSnapshot,
        strategy: ActiveDeviceStrategy,
    ) -> Self {
        self.active_device = Some(active_device);
        self.active_device_strategy = strategy;
        self
    }

    pub fn with_production_evidence(
        mut self,
        evidence: ProductionEvidence,
        live_deployed_status_available: bool,
    ) -> Self {
        self.production_evidence = Some(evidence);
        self.live_deployed_status_available = live_deployed_status_available;
        self
    }
}

/// Shared cancellation token for doctor jobs.
#[derive(Debug, Clone, Default)]
pub struct DoctorCancellation {
    cancelled: Arc<AtomicBool>,
}

impl DoctorCancellation {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }
}

/// Progress snapshot for a doctor job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorJobProgress {
    pub total_steps: usize,
    pub completed_steps: usize,
    pub current_step: Option<DoctorStepId>,
}

impl DoctorJobProgress {
    pub fn new(total_steps: usize) -> Self {
        Self {
            total_steps,
            completed_steps: 0,
            current_step: None,
        }
    }
}

/// Lifecycle state for a doctor job.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DoctorJobState {
    Queued,
    Running,
    Complete,
    Cancelled,
}

/// Minimal engine-owned doctor job runner.
#[derive(Debug, Clone)]
pub struct DoctorJob {
    state: DoctorJobState,
    progress: DoctorJobProgress,
    cancellation: DoctorCancellation,
    report: Option<DoctorReport>,
}

impl DoctorJob {
    pub fn new() -> Self {
        Self {
            state: DoctorJobState::Queued,
            progress: DoctorJobProgress::new(REQUIRED_DOCTOR_STEPS.len()),
            cancellation: DoctorCancellation::new(),
            report: None,
        }
    }

    pub fn state(&self) -> DoctorJobState {
        self.state
    }

    pub fn progress(&self) -> &DoctorJobProgress {
        &self.progress
    }

    pub fn cancellation(&self) -> DoctorCancellation {
        self.cancellation.clone()
    }

    pub fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub fn report(&self) -> Option<&DoctorReport> {
        self.report.as_ref()
    }

    pub fn run<W: AdsOnboardingWire>(
        &mut self,
        wire: &mut W,
        options: DoctorOptions,
    ) -> DoctorReport {
        self.state = DoctorJobState::Running;
        self.progress = DoctorJobProgress::new(REQUIRED_DOCTOR_STEPS.len());
        let report =
            run_doctor_with_progress(wire, options, &self.cancellation, &mut self.progress);
        self.state = if self.cancellation.is_cancelled() {
            DoctorJobState::Cancelled
        } else {
            DoctorJobState::Complete
        };
        self.report = Some(report.clone());
        report
    }
}

impl Default for DoctorJob {
    fn default() -> Self {
        Self::new()
    }
}

/// Run the ADS doctor synchronously.
pub fn run_doctor<W: AdsOnboardingWire>(
    wire: &mut W,
    options: DoctorOptions,
    cancellation: &DoctorCancellation,
) -> DoctorReport {
    let mut progress = DoctorJobProgress::new(REQUIRED_DOCTOR_STEPS.len());
    run_doctor_with_progress(wire, options, cancellation, &mut progress)
}

fn run_doctor_with_progress<W: AdsOnboardingWire>(
    wire: &mut W,
    options: DoctorOptions,
    cancellation: &DoctorCancellation,
    progress: &mut DoctorJobProgress,
) -> DoctorReport {
    if let Some(active_device) = options.active_device.as_ref() {
        match options.active_device_strategy {
            ActiveDeviceStrategy::ReadOnlyViaLiveStatus => {
                progress.completed_steps = REQUIRED_DOCTOR_STEPS.len();
                return active_report::read_only_report(&options, active_device);
            }
            ActiveDeviceStrategy::RequiresPause => {
                progress.completed_steps = REQUIRED_DOCTOR_STEPS.len();
                return active_report::requires_pause_report(&options, active_device);
            }
            ActiveDeviceStrategy::FullAfterExplicitPause => {}
        }
    }

    let mut context = DoctorContext::new(&options);
    let mut steps = Vec::with_capacity(REQUIRED_DOCTOR_STEPS.len());
    let mut blocked = false;

    for step_id in REQUIRED_DOCTOR_STEPS {
        progress.current_step = Some(*step_id);
        let step = if blocked {
            blocked_step(*step_id)
        } else if cancellation.is_cancelled() {
            blocked = true;
            cancelled_step(*step_id)
        } else {
            let started = Instant::now();
            let step = run_step(wire, &options, &mut context, *step_id);
            enforce_step_deadline(*step_id, step, started.elapsed())
        };
        if step.blocks_production_ready && step.status != DoctorStepStatus::Pass {
            blocked = true;
        }
        steps.push(step);
        progress.completed_steps += 1;
    }
    progress.current_step = None;

    build_report(options, context.target, context.local, steps)
}

fn enforce_step_deadline(id: DoctorStepId, step: DoctorStep, elapsed: Duration) -> DoctorStep {
    let Some(timeout) = DEFAULT_STEP_TIMEOUTS
        .iter()
        .find(|timeout| timeout.step == id)
    else {
        return step;
    };
    if step.status != DoctorStepStatus::Pass || elapsed <= Duration::from_millis(timeout.timeout_ms)
    {
        return step;
    }

    // Ads/TcAdsDll calls are synchronous. We configure finite native/socket
    // deadlines before entering them, then reject a late success here. This is
    // an enforceable result deadline, not unsafe thread cancellation.
    failed_step(
        id,
        step_title(id),
        OnboardingWireError::new(
            timeout_error_kind(id),
            format!(
                "ADS doctor step returned after its {} ms deadline (elapsed {} ms); the synchronous transport call cannot be safely pre-empted",
                timeout.timeout_ms,
                elapsed.as_millis()
            ),
        )
        .with_transport_failure(crate::ads::AdsTransportFailureKind::TimedOut),
    )
    .with_evidence("timeout_ms", json!(timeout.timeout_ms))
    .with_evidence("elapsed_ms", json!(elapsed.as_millis()))
}

fn timeout_error_kind(id: DoctorStepId) -> OnboardingWireErrorKind {
    match id {
        DoctorStepId::UdpIdentify => OnboardingWireErrorKind::UdpIdentifyBlocked,
        DoctorStepId::Tcp48898 => OnboardingWireErrorKind::Tcp48898Blocked,
        DoctorStepId::SymbolUpload => OnboardingWireErrorKind::NoSymbols,
        DoctorStepId::Notification => OnboardingWireErrorKind::NotificationFailure,
        DoctorStepId::ReadState
        | DoctorStepId::HandleResolve
        | DoctorStepId::SumupRead
        | DoctorStepId::SymbolVersion => OnboardingWireErrorKind::WrongPlcPort,
        _ => OnboardingWireErrorKind::UnsupportedOperation,
    }
}

struct DoctorContext {
    target: Option<TargetIdentity>,
    local: Option<LocalIdentity>,
    route_probe_result: Option<Result<String, OnboardingWireError>>,
    selected_symbol: Option<String>,
    resolved_handle: Option<u32>,
}

impl DoctorContext {
    fn new(options: &DoctorOptions) -> Self {
        Self {
            target: None,
            local: options.local_identity.clone(),
            route_probe_result: None,
            selected_symbol: None,
            resolved_handle: None,
        }
    }
}

fn run_step<W: AdsOnboardingWire>(
    wire: &mut W,
    options: &DoctorOptions,
    context: &mut DoctorContext,
    step_id: DoctorStepId,
) -> DoctorStep {
    match step_id {
        DoctorStepId::UdpIdentify => {
            if let Some(ams_net_id) = options.expected_target_ams_net_id.as_deref() {
                let target = TargetIdentity {
                    name: None,
                    ip: options.target_ip.clone(),
                    ams_net_id: ams_net_id.to_string(),
                    ams_port: options.ams_port,
                    tc_version: None,
                };
                let step = pass_step(
                    step_id,
                    "Find PLC on network",
                    format!("Using manually supplied target {}.", target_label(&target)),
                )
                .with_evidence("target_ip", target.ip.clone())
                .with_evidence("target_ams_net_id", target.ams_net_id.clone())
                .with_evidence("target_source", "manual");
                context.target = Some(target);
                step
            } else {
                match wire.udp_identify(options.target_ip.as_str()) {
                    Ok(mut target) => {
                        target.ams_port = options.ams_port;
                        let step = pass_step(
                            step_id,
                            "Find PLC on network",
                            format!("Found {}", target_label(&target)),
                        )
                        .with_evidence("target_ip", target.ip.clone())
                        .with_evidence("target_ams_net_id", target.ams_net_id.clone())
                        .with_evidence("target_source", "udp_identify");
                        context.target = Some(target);
                        step
                    }
                    Err(error) => failed_step(step_id, "Find PLC on network", error),
                }
            }
        }
        DoctorStepId::LocalIdentity => {
            let Some(local) = context.local.clone() else {
                return DoctorStep::failed(
                    step_id,
                    "truST local identity",
                    "Runtime host source IP and local AMS Net ID were not resolved.",
                    OnboardingWireErrorKind::NatOrPublic.classification(),
                );
            };
            pass_step(
                step_id,
                "truST local identity",
                format!("Using {} ({})", local.ams_net_id, local.chosen_ip),
            )
            .with_evidence("local_ip", local.chosen_ip)
            .with_evidence("local_ams_net_id", local.ams_net_id)
        }
        DoctorStepId::Tcp48898 => {
            let Some(target) = context.target.as_ref() else {
                return internal_missing_step(step_id, "target identity");
            };
            route_evidence::transport_step(wire, target)
        }
        DoctorStepId::RoutePresent => {
            let (Some(target), Some(local)) = (context.target.as_ref(), context.local.as_ref())
            else {
                return internal_missing_step(step_id, "target or local identity");
            };
            let evidence = route_evidence::route_round_trip_step(wire, target, local);
            context.route_probe_result = evidence.read_state;
            evidence.step
        }
        DoctorStepId::AmsTarget => {
            let Some(target) = context.target.as_ref() else {
                return internal_missing_step(step_id, "target identity");
            };
            if let Some(expected) = options.expected_target_ams_net_id.as_deref() {
                if target.ams_net_id != expected {
                    return failed_step(
                        step_id,
                        "Target AMS identity",
                        OnboardingWireError::new(
                            OnboardingWireErrorKind::WrongAmsNetId,
                            format!(
                                "detected target AMS Net ID '{}' but expected '{expected}'",
                                target.ams_net_id
                            ),
                        ),
                    );
                }
            }
            match wire.verify_ams_target(target) {
                Ok(()) => pass_step(
                    step_id,
                    "Target AMS identity",
                    format!("Target AMS Net ID is {}", target.ams_net_id),
                )
                .with_evidence("target_ams_net_id", target.ams_net_id.clone()),
                Err(error) => failed_step(step_id, "Target AMS identity", error),
            }
        }
        DoctorStepId::ReadState => {
            let Some(target) = context.target.as_ref() else {
                return internal_missing_step(step_id, "target identity");
            };
            let state = context
                .route_probe_result
                .clone()
                .unwrap_or_else(|| wire.read_state(target));
            match state {
                Ok(state) => pass_step(
                    step_id,
                    "PLC runtime state",
                    format!("PLC runtime responded with state '{state}'."),
                )
                .with_evidence("state", state),
                Err(error) => failed_step(step_id, "PLC runtime state", error),
            }
        }
        DoctorStepId::SymbolUpload => {
            let Some(target) = context.target.as_ref() else {
                return internal_missing_step(step_id, "target identity");
            };
            match wire.upload_symbols(target) {
                Ok(symbols) if symbols.is_empty() => failed_step(
                    step_id,
                    "TwinCAT symbols",
                    OnboardingWireError::new(
                        OnboardingWireErrorKind::NoSymbols,
                        "symbol upload returned no symbols",
                    ),
                ),
                Ok(symbols) => {
                    let selected = options
                        .selected_symbol
                        .clone()
                        .unwrap_or_else(|| symbols[0].name.clone());
                    context.selected_symbol = Some(selected.clone());
                    pass_step(
                        step_id,
                        "TwinCAT symbols",
                        format!("Uploaded {} symbol(s).", symbols.len()),
                    )
                    .with_evidence("symbol_count", json!(symbols.len()))
                    .with_evidence("selected_symbol", selected)
                }
                Err(error) => failed_step(step_id, "TwinCAT symbols", error),
            }
        }
        DoctorStepId::HandleResolve => {
            let (Some(target), Some(symbol)) =
                (context.target.as_ref(), context.selected_symbol.as_deref())
            else {
                return internal_missing_step(step_id, "target identity or selected symbol");
            };
            match wire.resolve_handle(target, symbol) {
                Ok(handle) => {
                    context.resolved_handle = Some(handle);
                    pass_step(
                        step_id,
                        "Resolve symbol handle",
                        format!("Resolved handle for '{symbol}'."),
                    )
                    .with_evidence("symbol", symbol.to_string())
                }
                Err(error) => failed_step(step_id, "Resolve symbol handle", error),
            }
        }
        DoctorStepId::SumupRead => {
            let (Some(target), Some(handle)) =
                (context.target.as_ref(), context.resolved_handle.as_ref())
            else {
                return internal_missing_step(step_id, "target identity or resolved handle");
            };
            match wire.sumup_read(target, &[*handle]) {
                Ok(values) => pass_step(
                    step_id,
                    "Read values",
                    format!("Read {} value(s) from ADS.", values.len()),
                )
                .with_evidence("value_count", json!(values.len())),
                Err(error) => failed_step(step_id, "Batch read", error),
            }
        }
        DoctorStepId::WriteGuarded => {
            if !options.writes_enabled {
                return DoctorStep::skipped(
                    step_id,
                    "Guarded write",
                    DoctorSkipReason::WritesDisabled,
                    "Writes are disabled for this doctor run.",
                )
                .with_remediation("Enable writes explicitly before running a write probe.")
                .with_next_action(NextAction::new(NextActionKind::EnableWrite));
            }
            let Some(target) = context.target.as_ref() else {
                return internal_missing_step(step_id, "target identity");
            };
            let Some(probe) = options.write_probe.as_ref() else {
                return failed_step(
                    step_id,
                    "Guarded write",
                    OnboardingWireError::new(
                        OnboardingWireErrorKind::UnsupportedOperation,
                        "write probe requires explicit symbol, type, and value",
                    ),
                );
            };
            match wire.guarded_write_probe(target, probe) {
                Ok(()) => pass_step(
                    step_id,
                    "Guarded write",
                    format!("Guarded ADS write probe succeeded for '{}'.", probe.symbol),
                )
                .with_evidence("symbol", probe.symbol.clone()),
                Err(error) => failed_step(step_id, "Guarded write", error),
            }
        }
        DoctorStepId::Notification => {
            let (Some(target), Some(symbol)) =
                (context.target.as_ref(), context.selected_symbol.as_deref())
            else {
                return internal_missing_step(step_id, "target identity or selected symbol");
            };
            match wire.read_update_sample(target, symbol) {
                Ok(sample) => pass_step(
                    step_id,
                    "Read update sample",
                    format!(
                        "Received an ADS read update for '{}' after subscription.",
                        sample.point_name
                    ),
                )
                .with_evidence("symbol", sample.point_name)
                .with_evidence("subscription_id", json!(sample.subscription_id))
                .with_evidence("quality", "good")
                .with_evidence("sample_method", "subscribed_read_update")
                .with_evidence("read_proven", true),
                Err(error) => failed_step(step_id, "Read update sample", error),
            }
        }
        DoctorStepId::SymbolVersion => {
            let Some(target) = context.target.as_ref() else {
                return internal_missing_step(step_id, "target identity");
            };
            match wire.symbol_version(target) {
                Ok(version) => pass_step(
                    step_id,
                    "Symbol version",
                    format!("Symbol version is {version}."),
                )
                .with_evidence("symbol_version", json!(version)),
                Err(error) => failed_step(step_id, "Symbol version", error),
            }
        }
        DoctorStepId::BindExposure
        | DoctorStepId::ListenerBound
        | DoctorStepId::UdpIdentifyAnswer
        | DoctorStepId::SymbolsExposed
        | DoctorStepId::ClientsAllowed
        | DoctorStepId::SymbolServe
        | DoctorStepId::SelfReadState
        | DoctorStepId::SelfHandleResolve
        | DoctorStepId::SelfSumupRead
        | DoctorStepId::SelfNotification
        | DoctorStepId::SelfWriteGuarded
        | DoctorStepId::ParserLimits
        | DoctorStepId::AllowlistEnforced
        | DoctorStepId::ExternalClientVerified => failed_step(
            step_id,
            step_title(step_id),
            OnboardingWireError::new(
                OnboardingWireErrorKind::UnsupportedOperation,
                "server-only doctor step cannot run in ADS client doctor",
            ),
        ),
    }
}

fn build_report(
    options: DoctorOptions,
    target: Option<TargetIdentity>,
    local: Option<LocalIdentity>,
    steps: Vec<DoctorStep>,
) -> DoctorReport {
    let mut report = DoctorReport::new(options.ran_from, options.transport).with_steps(steps);
    report.writes_enabled = options.writes_enabled;
    if let Some(target) = target {
        report = report.with_target(target);
    }
    if let Some(local) = local {
        report = report.with_local(local);
    }
    if report.overall == DoctorOverall::Pass
        && production_vantage(options.ran_from)
        && options.live_deployed_status_available
    {
        if let Some(evidence) = options.production_evidence {
            report = report.with_evidence(evidence);
        }
    }
    let summary = summary_for(&report);
    report.with_summary(summary)
}

fn production_vantage(vantage: DoctorVantage) -> bool {
    matches!(
        vantage,
        DoctorVantage::RuntimeHost | DoctorVantage::SetupWebRuntimeHost
    )
}

fn summary_for(report: &DoctorReport) -> String {
    match report.overall {
        DoctorOverall::Pass if report.production_ready => {
            "ADS doctor passed; production-ready evidence is attached.".to_string()
        }
        DoctorOverall::Pass => {
            "ADS doctor passed; deploy and live runtime status are still required for production-ready evidence.".to_string()
        }
        DoctorOverall::Partial => {
            "ADS doctor completed with non-blocking warnings or skips.".to_string()
        }
        DoctorOverall::Fail => {
            let problems = report
                .steps
                .iter()
                .filter(|step| {
                    step.blocks_production_ready && step.status != DoctorStepStatus::Pass
                })
                .count();
            format!("{problems} blocking ADS doctor problem(s).")
        }
    }
}

fn pass_step(id: DoctorStepId, title: impl Into<String>, detail: impl Into<String>) -> DoctorStep {
    DoctorStep::new(id, title, DoctorStepStatus::Pass, detail)
}

fn failed_step(
    id: DoctorStepId,
    title: impl Into<String>,
    error: OnboardingWireError,
) -> DoctorStep {
    let classification = error.classification();
    DoctorStep::failed(id, title, error.detail, classification)
}

fn blocked_step(id: DoctorStepId) -> DoctorStep {
    DoctorStep::skipped(
        id,
        step_title(id),
        DoctorSkipReason::BlockedByPreviousStep,
        "Skipped because an earlier blocking ADS doctor step failed.",
    )
}

fn cancelled_step(id: DoctorStepId) -> DoctorStep {
    DoctorStep::skipped(
        id,
        step_title(id),
        DoctorSkipReason::Cancelled,
        "ADS doctor was cancelled before this step ran.",
    )
}

fn internal_missing_step(id: DoctorStepId, missing: &str) -> DoctorStep {
    failed_step(
        id,
        step_title(id),
        OnboardingWireError::new(
            OnboardingWireErrorKind::UnsupportedOperation,
            format!("doctor step was reached without {missing}"),
        ),
    )
}

fn step_title(id: DoctorStepId) -> &'static str {
    match id {
        DoctorStepId::UdpIdentify => "Find PLC on network",
        DoctorStepId::LocalIdentity => "truST local identity",
        DoctorStepId::Tcp48898 => "ADS transport reachable",
        DoctorStepId::RoutePresent => "Route back to truST",
        DoctorStepId::AmsTarget => "Target AMS identity",
        DoctorStepId::ReadState => "PLC runtime state",
        DoctorStepId::SymbolUpload => "ADS symbols",
        DoctorStepId::HandleResolve => "Resolve symbol handle",
        DoctorStepId::SumupRead => "Read values",
        DoctorStepId::WriteGuarded => "Guarded write",
        DoctorStepId::Notification => "Read update sample",
        DoctorStepId::SymbolVersion => "Symbol version",
        DoctorStepId::BindExposure => "Bind interface",
        DoctorStepId::ListenerBound => "ADS listener",
        DoctorStepId::UdpIdentifyAnswer => "Broadcast discovery",
        DoctorStepId::SymbolsExposed => "Exposed symbols",
        DoctorStepId::ClientsAllowed => "Allowed clients",
        DoctorStepId::SymbolServe => "Serve symbol",
        DoctorStepId::SelfReadState => "Self-test read state",
        DoctorStepId::SelfHandleResolve => "Self-test resolve handle",
        DoctorStepId::SelfSumupRead => "Self-test sum-up read",
        DoctorStepId::SelfNotification => "Self-test notification",
        DoctorStepId::SelfWriteGuarded => "Self-test guarded write",
        DoctorStepId::ParserLimits => "Parser limits",
        DoctorStepId::AllowlistEnforced => "Allowlist enforcement",
        DoctorStepId::ExternalClientVerified => "External client verified",
    }
}

fn target_label(target: &TargetIdentity) -> String {
    target.name.as_ref().map_or_else(
        || target.ip.clone(),
        |name| format!("{name} @ {}", target.ip),
    )
}

#[cfg(test)]
mod deadline_tests {
    use super::*;

    #[test]
    fn late_success_is_rejected_with_explicit_non_preemptible_deadline_evidence() {
        let step = pass_step(DoctorStepId::ReadState, "PLC runtime state", "late success");

        let step =
            enforce_step_deadline(DoctorStepId::ReadState, step, Duration::from_millis(5_001));

        assert_eq!(step.status, DoctorStepStatus::Fail);
        assert!(step.detail.contains("cannot be safely pre-empted"));
        assert_eq!(step.evidence.get("timeout_ms"), Some(&json!(5_000)));
        assert_eq!(step.evidence.get("elapsed_ms"), Some(&json!(5_001)));
    }

    #[test]
    fn success_inside_deadline_remains_a_pass() {
        let step = pass_step(
            DoctorStepId::Notification,
            "Read update sample",
            "read proven",
        );

        let step = enforce_step_deadline(
            DoctorStepId::Notification,
            step,
            Duration::from_millis(4_999),
        );

        assert_eq!(step.status, DoctorStepStatus::Pass);
    }
}
