use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use trust_ads_core::SymbolSnapshot;

/// Version of the ADS onboarding diagnostics JSON contract.
pub const ADS_DIAGNOSTICS_SCHEMA_VERSION: u32 = 2;

/// Ordered JSON object used for deterministic evidence and action payloads.
pub type DiagnosticMap = BTreeMap<String, JsonValue>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorRole {
    Client,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DoctorVantage {
    RuntimeHost,
    SetupWebRuntimeHost,
    VscodeCli,
    VscodeAuthoringOnly,
    CliLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticTransport {
    Plain,
    Secure,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorOverall {
    Pass,
    Partial,
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStepStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorStepId {
    // ADS client onboarding steps.
    UdpIdentify,
    LocalIdentity,
    #[serde(rename = "tcp_48898")]
    Tcp48898,
    RoutePresent,
    AmsTarget,
    ReadState,
    SymbolUpload,
    HandleResolve,
    SumupRead,
    WriteGuarded,
    Notification,
    SymbolVersion,
    // ADS server onboarding/doctor steps.
    BindExposure,
    ListenerBound,
    UdpIdentifyAnswer,
    SymbolsExposed,
    ClientsAllowed,
    SymbolServe,
    SelfReadState,
    SelfHandleResolve,
    SelfSumupRead,
    SelfNotification,
    SelfWriteGuarded,
    ParserLimits,
    AllowlistEnforced,
    ExternalClientVerified,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoctorSkipReason {
    BlockedByPreviousStep,
    ActiveDevice,
    WritesDisabled,
    NotSupportedByTarget,
    NotRequested,
    Cancelled,
    ServerDisabled,
    NoSymbolsExposed,
    NoClientsAllowed,
    ExternalClientPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NextActionKind {
    None,
    PickTarget,
    FixLocalIp,
    AddRoute,
    OpenSetup,
    DownloadPowershell,
    CopyXml,
    OpenRuntimePane,
    EnableWrite,
    UseSecure,
    Deploy,
    RerunDoctor,
    ConfigureExpose,
    AddAllowedClient,
    OpenFirewall,
    WaitForClient,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NextAction {
    pub kind: NextActionKind,
    #[serde(default, skip_serializing_if = "DiagnosticMap::is_empty")]
    pub params: DiagnosticMap,
}

impl NextAction {
    pub fn new(kind: NextActionKind) -> Self {
        Self {
            kind,
            params: DiagnosticMap::new(),
        }
    }

    pub fn with_param(mut self, key: impl Into<String>, value: impl Into<JsonValue>) -> Self {
        self.params.insert(key.into(), value.into());
        self
    }
}

impl Default for NextAction {
    fn default() -> Self {
        Self::new(NextActionKind::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdsErrorInfo {
    pub code: u32,
    pub name: String,
}

impl AdsErrorInfo {
    pub fn new(code: u32, name: impl Into<String>) -> Self {
        Self {
            code,
            name: name.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctorStep {
    pub id: DoctorStepId,
    pub title: String,
    pub status: DoctorStepStatus,
    pub skip_reason: Option<DoctorSkipReason>,
    pub detail: String,
    #[serde(default, skip_serializing_if = "DiagnosticMap::is_empty")]
    pub evidence: DiagnosticMap,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ads_error: Option<AdsErrorInfo>,
    pub remediation: String,
    pub next_action: NextAction,
    pub blocks_production_ready: bool,
}

impl DoctorStep {
    pub fn new(
        id: DoctorStepId,
        title: impl Into<String>,
        status: DoctorStepStatus,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            id,
            title: title.into(),
            status,
            skip_reason: None,
            detail: detail.into(),
            evidence: DiagnosticMap::new(),
            ads_error: None,
            remediation: String::new(),
            next_action: NextAction::default(),
            blocks_production_ready: !matches!(
                status,
                DoctorStepStatus::Pass | DoctorStepStatus::Warn
            ),
        }
    }

    pub fn failed(
        id: DoctorStepId,
        title: impl Into<String>,
        detail: impl Into<String>,
        classification: FailureClassification,
    ) -> Self {
        let mut step = Self::new(id, title, DoctorStepStatus::Fail, detail);
        step.ads_error = classification.ads_error;
        step.remediation = classification.remediation;
        step.next_action = classification.next_action;
        step.blocks_production_ready = classification.blocks_production_ready;
        step
    }

    pub fn skipped(
        id: DoctorStepId,
        title: impl Into<String>,
        reason: DoctorSkipReason,
        detail: impl Into<String>,
    ) -> Self {
        let mut step = Self::new(id, title, DoctorStepStatus::Skip, detail);
        step.skip_reason = Some(reason);
        step.blocks_production_ready = !matches!(
            reason,
            DoctorSkipReason::WritesDisabled
                | DoctorSkipReason::NotRequested
                | DoctorSkipReason::ActiveDevice
        );
        step
    }

    pub fn with_remediation(mut self, remediation: impl Into<String>) -> Self {
        self.remediation = remediation.into();
        self
    }

    pub fn with_next_action(mut self, next_action: NextAction) -> Self {
        self.next_action = next_action;
        self
    }

    pub fn with_ads_error(mut self, ads_error: AdsErrorInfo) -> Self {
        self.ads_error = Some(ads_error);
        self
    }

    pub fn with_evidence(mut self, key: impl Into<String>, value: impl Into<JsonValue>) -> Self {
        self.evidence.insert(key.into(), value.into());
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TargetIdentity {
    pub name: Option<String>,
    pub ip: String,
    pub ams_net_id: String,
    pub ams_port: u16,
    pub tc_version: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalNetworkClassification {
    Lan,
    Vpn,
    Tailscale,
    Loopback,
    Public,
    NatSuspect,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalAddressCandidate {
    pub ip: String,
    pub ams_net_id: String,
    pub nic: Option<String>,
    pub classification: LocalNetworkClassification,
    pub selected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalIdentity {
    pub host_name: Option<String>,
    pub chosen_ip: String,
    pub ams_net_id: String,
    pub nic: Option<String>,
    #[serde(default)]
    pub candidates: Vec<LocalAddressCandidate>,
    pub classification: LocalNetworkClassification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionEvidence {
    pub doctor_timestamp_ms: u64,
    pub doctor_schema_version: u32,
    pub runtime_identity_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_identity_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allowed_clients_hash: Option<String>,
    pub ads_config_hash: String,
    pub symbol_snapshot_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generated_st_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_ads_config_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_ads_status_hash: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub external_client_verified: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_client_kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_client_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub external_client_timestamp_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub discoverable: bool,
    pub freshness: EvidenceFreshness,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFreshness {
    pub stale_after_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_clock_warning: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub schema_version: u32,
    pub role: DoctorRole,
    pub ran_from: DoctorVantage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local: Option<LocalIdentity>,
    pub transport: DiagnosticTransport,
    pub writes_enabled: bool,
    #[serde(default)]
    pub steps: Vec<DoctorStep>,
    pub overall: DoctorOverall,
    pub production_ready: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evidence: Option<ProductionEvidence>,
    pub summary: String,
}

impl DoctorReport {
    pub fn new(ran_from: DoctorVantage, transport: DiagnosticTransport) -> Self {
        Self::for_role(DoctorRole::Client, ran_from, transport)
    }

    pub fn for_role(
        role: DoctorRole,
        ran_from: DoctorVantage,
        transport: DiagnosticTransport,
    ) -> Self {
        Self {
            schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
            role,
            ran_from,
            target: None,
            local: None,
            transport,
            writes_enabled: false,
            steps: Vec::new(),
            overall: DoctorOverall::Fail,
            production_ready: false,
            evidence: None,
            summary: String::new(),
        }
    }

    pub fn with_target(mut self, target: TargetIdentity) -> Self {
        self.target = Some(target);
        self
    }

    pub fn with_local(mut self, local: LocalIdentity) -> Self {
        self.local = Some(local);
        self
    }

    pub fn with_steps(mut self, steps: Vec<DoctorStep>) -> Self {
        self.steps = steps;
        self.recompute_overall();
        self
    }

    pub fn with_summary(mut self, summary: impl Into<String>) -> Self {
        self.summary = summary.into();
        self
    }

    pub fn with_evidence(mut self, evidence: ProductionEvidence) -> Self {
        self.evidence = Some(evidence);
        self.production_ready = self.overall == DoctorOverall::Pass
            && self
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence_satisfies_role(self.role, evidence));
        self
    }

    pub fn recompute_overall(&mut self) {
        let has_blocking_failure = self
            .steps
            .iter()
            .any(|step| step.blocks_production_ready && step.status != DoctorStepStatus::Pass);
        let has_warning_or_skip = self
            .steps
            .iter()
            .any(|step| matches!(step.status, DoctorStepStatus::Warn | DoctorStepStatus::Skip));
        self.overall = if has_blocking_failure {
            DoctorOverall::Fail
        } else if has_warning_or_skip {
            DoctorOverall::Partial
        } else {
            DoctorOverall::Pass
        };
        self.production_ready = self.overall == DoctorOverall::Pass
            && self
                .evidence
                .as_ref()
                .is_some_and(|evidence| evidence_satisfies_role(self.role, evidence));
    }
}

fn evidence_satisfies_role(role: DoctorRole, evidence: &ProductionEvidence) -> bool {
    match role {
        DoctorRole::Client => true,
        DoctorRole::Server => {
            evidence.external_client_verified
                && evidence
                    .external_client_kind
                    .as_deref()
                    .is_some_and(external_client_kind_is_twincat)
        }
    }
}

fn external_client_kind_is_twincat(kind: &str) -> bool {
    kind.trim().eq_ignore_ascii_case("twincat")
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoutePlan {
    pub route_name: String,
    pub target: TargetIdentity,
    pub local: LocalIdentity,
    pub channel: CredentialChannelClassification,
    pub automatic_route: RouteActionAvailability,
    #[serde(default)]
    pub artifacts: Vec<RouteArtifact>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteActionAvailability {
    Available,
    DisabledUntrustedChannel,
    DisabledNatOrPublic,
    DisabledUnsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RouteArtifact {
    pub kind: RouteArtifactKind,
    pub label: String,
    pub filename: Option<String>,
    pub content_type: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RouteArtifactKind {
    Powershell,
    StaticRoutesXml,
    ManualSteps,
    RemovalPowershell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialChannelClassification {
    TrustedSameHost,
    TrustedHttpsAdmin,
    LocalCliDirectAddRoute,
    UntrustedRemotePlainTcp,
    UntrustedPlainHttpNetwork,
}

impl CredentialChannelClassification {
    pub fn permits_credentials(self) -> bool {
        matches!(
            self,
            Self::TrustedSameHost | Self::TrustedHttpsAdmin | Self::LocalCliDirectAddRoute
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdsStatusReport {
    pub schema_version: u32,
    pub role: DoctorRole,
    pub overall: AdsStatusOverall,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_identity_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployed_ads_config_hash: Option<String>,
    #[serde(default)]
    pub connections: Vec<AdsConnectionStatus>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdsStatusOverall {
    Healthy,
    Degraded,
    Faulted,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdsConnectionStatus {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<TargetIdentity>,
    pub state: AdsConnectionStatusState,
    pub point_count: usize,
    pub degraded_points: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_good_value_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_version: Option<u32>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdsConnectionStatusState {
    Connected,
    Reconnecting,
    Faulted,
    Stale,
    Disabled,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionReadinessState {
    Ready,
    NeedsRecheck,
    NotReady,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProductionReadinessReason {
    MissingEvidence,
    MissingRuntimeStatus,
    EvidenceExpired,
    DeployedAdsConfigMissing,
    DeployedAdsConfigMismatch,
    RuntimeAdsStatusChanged,
    RuntimeAdsFaulted,
    RuntimeAdsDegraded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductionReadinessReport {
    pub state: ProductionReadinessState,
    #[serde(default)]
    pub reasons: Vec<ProductionReadinessReason>,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvidenceHashInputSpec {
    pub field: &'static str,
    pub input: &'static str,
}

pub const EVIDENCE_HASH_INPUTS: &[EvidenceHashInputSpec] = &[
    EvidenceHashInputSpec {
        field: "runtime_identity_hash",
        input: "canonical JSON of runtime hostname, selected source IP, selected local AMS Net ID, runtime endpoint or setup identity, and schema version",
    },
    EvidenceHashInputSpec {
        field: "target_identity_hash",
        input: "canonical JSON of PLC IP, target AMS Net ID, AMS port, target name if known, and TwinCAT version if known",
    },
    EvidenceHashInputSpec {
        field: "allowed_clients_hash",
        input: "server-role canonical JSON of allowed client AMS Net IDs, source IP/CIDR pins, and unsafe/lab trust flags",
    },
    EvidenceHashInputSpec {
        field: "ads_config_hash",
        input: "byte hash of canonical ads.toml as written by the wizard",
    },
    EvidenceHashInputSpec {
        field: "symbol_snapshot_hash",
        input: "byte hash of canonical symbol snapshot JSON",
    },
    EvidenceHashInputSpec {
        field: "generated_st_hash",
        input: "byte hash of src/generated/ads_generated.st when generated",
    },
    EvidenceHashInputSpec {
        field: "deployed_ads_config_hash",
        input: "byte hash of the ADS config loaded by the runtime",
    },
    EvidenceHashInputSpec {
        field: "runtime_ads_status_hash",
        input: "canonical JSON hash of the live ADS status report used for production-ready evidence",
    },
];

#[derive(Debug, Clone, Copy)]
pub struct ProductionEvidenceInput<'a> {
    pub doctor_timestamp_ms: u64,
    pub runtime_identity: &'a LocalIdentity,
    pub target_identity: &'a TargetIdentity,
    pub ads_toml: &'a str,
    pub symbol_snapshots: &'a [SymbolSnapshot],
    pub generated_st: Option<&'a str>,
    pub deployed_ads_toml: Option<&'a str>,
    pub runtime_ads_status: Option<&'a AdsStatusReport>,
    pub stale_after_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub runtime_clock_warning: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub struct ServerProductionEvidenceInput<'a, T: Serialize + ?Sized> {
    pub doctor_timestamp_ms: u64,
    pub runtime_identity: &'a LocalIdentity,
    pub allowed_clients: &'a T,
    pub ads_server_config: &'a str,
    pub symbol_snapshot: &'a SymbolSnapshot,
    pub deployed_ads_server_config: Option<&'a str>,
    pub runtime_ads_status: Option<&'a AdsStatusReport>,
    pub external_client_verified: bool,
    pub external_client_kind: Option<&'a str>,
    pub external_client_name: Option<&'a str>,
    pub external_client_timestamp_ms: Option<u64>,
    pub discoverable: bool,
    pub stale_after_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub runtime_clock_warning: Option<&'a str>,
}

#[derive(Debug)]
pub enum ProductionEvidenceError {
    Serialize(serde_json::Error),
}

impl std::fmt::Display for ProductionEvidenceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Serialize(error) => write!(f, "failed to serialize ADS evidence input: {error}"),
        }
    }
}

impl std::error::Error for ProductionEvidenceError {}

pub fn build_production_evidence(
    input: ProductionEvidenceInput<'_>,
) -> Result<ProductionEvidence, ProductionEvidenceError> {
    Ok(ProductionEvidence {
        doctor_timestamp_ms: input.doctor_timestamp_ms,
        doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        runtime_identity_hash: hash_json(input.runtime_identity)?,
        target_identity_hash: Some(hash_json(input.target_identity)?),
        allowed_clients_hash: None,
        ads_config_hash: sha256_evidence_hash(input.ads_toml.as_bytes()),
        symbol_snapshot_hash: hash_symbol_snapshots(input.symbol_snapshots)?,
        generated_st_hash: input
            .generated_st
            .map(|source| sha256_evidence_hash(source.as_bytes())),
        deployed_ads_config_hash: input
            .deployed_ads_toml
            .map(|source| sha256_evidence_hash(source.as_bytes())),
        runtime_ads_status_hash: input.runtime_ads_status.map(hash_json).transpose()?,
        external_client_verified: false,
        external_client_kind: None,
        external_client_name: None,
        external_client_timestamp_ms: None,
        discoverable: false,
        freshness: EvidenceFreshness {
            stale_after_ms: input.stale_after_ms,
            expires_at_ms: input.expires_at_ms,
            runtime_clock_warning: input.runtime_clock_warning.map(ToString::to_string),
        },
    })
}

pub fn build_server_production_evidence<T: Serialize + ?Sized>(
    input: ServerProductionEvidenceInput<'_, T>,
) -> Result<ProductionEvidence, ProductionEvidenceError> {
    Ok(ProductionEvidence {
        doctor_timestamp_ms: input.doctor_timestamp_ms,
        doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
        runtime_identity_hash: hash_json(input.runtime_identity)?,
        target_identity_hash: None,
        allowed_clients_hash: Some(hash_json(input.allowed_clients)?),
        ads_config_hash: sha256_evidence_hash(input.ads_server_config.as_bytes()),
        symbol_snapshot_hash: hash_symbol_snapshots(std::slice::from_ref(input.symbol_snapshot))?,
        generated_st_hash: None,
        deployed_ads_config_hash: input
            .deployed_ads_server_config
            .map(|source| sha256_evidence_hash(source.as_bytes())),
        runtime_ads_status_hash: input.runtime_ads_status.map(hash_json).transpose()?,
        external_client_verified: input.external_client_verified,
        external_client_kind: input.external_client_kind.map(ToString::to_string),
        external_client_name: input.external_client_name.map(ToString::to_string),
        external_client_timestamp_ms: input.external_client_timestamp_ms,
        discoverable: input.discoverable,
        freshness: EvidenceFreshness {
            stale_after_ms: input.stale_after_ms,
            expires_at_ms: input.expires_at_ms,
            runtime_clock_warning: input.runtime_clock_warning.map(ToString::to_string),
        },
    })
}

pub fn evaluate_production_readiness(
    evidence: Option<&ProductionEvidence>,
    runtime_status: Option<&AdsStatusReport>,
    now_ms: u64,
) -> Result<ProductionReadinessReport, ProductionEvidenceError> {
    let Some(evidence) = evidence else {
        return Ok(production_readiness_report(
            ProductionReadinessState::NotReady,
            vec![ProductionReadinessReason::MissingEvidence],
        ));
    };
    let Some(status) = runtime_status else {
        return Ok(production_readiness_report(
            ProductionReadinessState::NeedsRecheck,
            vec![ProductionReadinessReason::MissingRuntimeStatus],
        ));
    };

    let mut reasons = Vec::new();
    if evidence
        .freshness
        .expires_at_ms
        .is_some_and(|expires_at| now_ms > expires_at)
    {
        reasons.push(ProductionReadinessReason::EvidenceExpired);
    }
    match (
        evidence.deployed_ads_config_hash.as_deref(),
        status.deployed_ads_config_hash.as_deref(),
    ) {
        (None, _) | (_, None) => reasons.push(ProductionReadinessReason::DeployedAdsConfigMissing),
        (Some(expected), Some(actual)) if expected != actual => {
            reasons.push(ProductionReadinessReason::DeployedAdsConfigMismatch);
        }
        _ => {}
    }
    if let Some(expected_status_hash) = evidence.runtime_ads_status_hash.as_deref() {
        let actual_status_hash = hash_json(status)?;
        if actual_status_hash != expected_status_hash {
            reasons.push(ProductionReadinessReason::RuntimeAdsStatusChanged);
        }
    }
    match status.overall {
        AdsStatusOverall::Faulted => reasons.push(ProductionReadinessReason::RuntimeAdsFaulted),
        AdsStatusOverall::Degraded | AdsStatusOverall::Disabled | AdsStatusOverall::Unknown => {
            reasons.push(ProductionReadinessReason::RuntimeAdsDegraded);
        }
        AdsStatusOverall::Healthy => {}
    }

    let state = if reasons.is_empty() {
        ProductionReadinessState::Ready
    } else {
        ProductionReadinessState::NeedsRecheck
    };
    Ok(production_readiness_report(state, reasons))
}

fn production_readiness_report(
    state: ProductionReadinessState,
    reasons: Vec<ProductionReadinessReason>,
) -> ProductionReadinessReport {
    let summary = match state {
        ProductionReadinessState::Ready => {
            "ADS production-ready evidence matches the deployed runtime.".to_string()
        }
        ProductionReadinessState::NeedsRecheck => {
            format!(
                "ADS production-ready evidence needs recheck: {} reason(s).",
                reasons.len()
            )
        }
        ProductionReadinessState::NotReady => {
            "ADS production-ready evidence is not available.".to_string()
        }
    };
    ProductionReadinessReport {
        state,
        reasons,
        summary,
    }
}

pub fn sha256_evidence_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn hash_json<T: Serialize + ?Sized>(value: &T) -> Result<String, ProductionEvidenceError> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_evidence_hash(&bytes))
        .map_err(ProductionEvidenceError::Serialize)
}

fn hash_symbol_snapshots(snapshots: &[SymbolSnapshot]) -> Result<String, ProductionEvidenceError> {
    let mut sorted = snapshots.to_vec();
    sorted.sort_by(|left, right| left.route_name.cmp(&right.route_name));
    let mut hasher = Sha256::new();
    for snapshot in &sorted {
        let json = snapshot
            .to_deterministic_json()
            .map_err(ProductionEvidenceError::Serialize)?;
        hasher.update(json.as_bytes());
    }
    Ok(format!("sha256:{:x}", hasher.finalize()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OnboardingFailureKind {
    UdpIdentifyBlocked,
    #[serde(rename = "tcp_48898_blocked")]
    Tcp48898Blocked,
    RouteMissing,
    WrongAmsNetId,
    WrongPlcPort,
    SecureRequired,
    NoSymbols,
    CredentialsRejected,
    NatOrPublic,
    Fingerprint1861,
    NotificationFailure,
    UnsupportedOperation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureClassification {
    pub kind: OnboardingFailureKind,
    pub explanation: String,
    pub remediation: String,
    pub next_action: NextAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ads_error: Option<AdsErrorInfo>,
    pub blocks_production_ready: bool,
}

pub fn classify_onboarding_failure(kind: OnboardingFailureKind) -> FailureClassification {
    match kind {
        OnboardingFailureKind::UdpIdentifyBlocked => classification(
            kind,
            "The PLC did not answer ADS discovery.",
            "Enter the PLC IP manually or check UDP 48899 and firewall rules.",
            NextActionKind::PickTarget,
        ),
        OnboardingFailureKind::Tcp48898Blocked => classification(
            kind,
            "TwinCAT router TCP port 48898 is not reachable.",
            "Check the network path and firewall on the TwinCAT target.",
            NextActionKind::FixLocalIp,
        ),
        OnboardingFailureKind::RouteMissing => classification(
            kind,
            "The PLC does not trust this truST runtime host yet.",
            "Add a static ADS route on the PLC for this truST runtime host.",
            NextActionKind::AddRoute,
        ),
        OnboardingFailureKind::WrongAmsNetId => classification(
            kind,
            "The IP is reachable, but the ADS identity does not match the configured target AMS Net ID.",
            "Use the detected AMS Net ID or correct the target configuration.",
            NextActionKind::PickTarget,
        ),
        OnboardingFailureKind::WrongPlcPort => classification(
            kind,
            "TwinCAT router works, but the selected PLC runtime port did not respond.",
            "Try PLC port 851, 852, or select the correct TwinCAT runtime.",
            NextActionKind::PickTarget,
        ),
        OnboardingFailureKind::SecureRequired => classification(
            kind,
            "The target requires Secure ADS, which truST does not support yet.",
            "Use OPC UA for secure integration or change the target ADS policy.",
            NextActionKind::UseSecure,
        ),
        OnboardingFailureKind::NoSymbols => classification(
            kind,
            "Symbol upload worked, but no compatible TwinCAT symbols were found.",
            "Check the PLC project and symbol export settings.",
            NextActionKind::None,
        ),
        OnboardingFailureKind::CredentialsRejected => classification(
            kind,
            "TwinCAT refused the route credentials.",
            "Retry the credentials or use the generated PowerShell route artifact.",
            NextActionKind::AddRoute,
        ),
        OnboardingFailureKind::NatOrPublic => classification(
            kind,
            "The runtime host appears to be behind NAT or public routing.",
            "Use a private OT network or VPN path where the PLC can route back to the runtime host.",
            NextActionKind::FixLocalIp,
        ),
        OnboardingFailureKind::Fingerprint1861 => classification(
            kind,
            "TwinCAT's GUI fingerprint handshake failed for this non-Windows client path.",
            "Use truST route setup or generated route artifacts instead of TwinCAT Broadcast Search.",
            NextActionKind::AddRoute,
        )
        .with_ads_error(AdsErrorInfo::new(1861, "ADSERR_DEVICE_INVALIDCONTEXT")),
        OnboardingFailureKind::NotificationFailure => classification(
            kind,
            "ADS notification subscription did not deliver samples.",
            "Use cyclic polling for this point or check TwinCAT notification settings.",
            NextActionKind::None,
        ),
        OnboardingFailureKind::UnsupportedOperation => classification(
            kind,
            "This ADS onboarding operation is not implemented in the current build.",
            "Use the available setup step or update truST to a build that supports this operation.",
            NextActionKind::None,
        ),
    }
}

pub fn classify_ads_error_code(code: u32) -> Option<FailureClassification> {
    match code {
        1861 => Some(classify_onboarding_failure(
            OnboardingFailureKind::Fingerprint1861,
        )),
        _ => None,
    }
}

fn classification(
    kind: OnboardingFailureKind,
    explanation: &str,
    remediation: &str,
    next_action: NextActionKind,
) -> FailureClassification {
    FailureClassification {
        kind,
        explanation: explanation.to_string(),
        remediation: remediation.to_string(),
        next_action: NextAction::new(next_action),
        ads_error: None,
        blocks_production_ready: true,
    }
}

impl FailureClassification {
    fn with_ads_error(mut self, ads_error: AdsErrorInfo) -> Self {
        self.ads_error = Some(ads_error);
        self
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::json;
    use trust_ads_core::{AdsDataTypeDescriptor, IecDataType, SymbolDescriptor, SymbolFlag};

    use super::*;

    #[test]
    fn serializes_documented_missing_route_shape_deterministically() {
        let report = missing_route_report();
        let json = serde_json::to_string_pretty(&report).expect("serialize report");

        assert_eq!(
            json,
            r#"{
  "schema_version": 2,
  "role": "client",
  "ran_from": "runtime-host",
  "target": {
    "name": "CX-1234",
    "ip": "192.168.10.5",
    "ams_net_id": "5.23.91.12.1.1",
    "ams_port": 851,
    "tc_version": "3.1.4024"
  },
  "local": {
    "host_name": "line-controller-1",
    "chosen_ip": "192.168.10.20",
    "ams_net_id": "192.168.10.20.1.1",
    "nic": "eth0",
    "candidates": [],
    "classification": "lan"
  },
  "transport": "plain",
  "writes_enabled": false,
  "steps": [
    {
      "id": "route_present",
      "title": "Route back to truST",
      "status": "fail",
      "skip_reason": null,
      "detail": "The PLC does not have a route back to 192.168.10.20.1.1.",
      "evidence": {
        "local_ams_net_id": "192.168.10.20.1.1",
        "local_ip": "192.168.10.20",
        "target_ip": "192.168.10.5"
      },
      "remediation": "Add a static ADS route on the PLC for this truST runtime host.",
      "next_action": {
        "kind": "add_route"
      },
      "blocks_production_ready": true
    }
  ],
  "overall": "fail",
  "production_ready": false,
  "summary": "1 problem: PLC has no route back to truST."
}"#
        );
    }

    #[test]
    fn committed_golden_fixtures_match_rust_schema() {
        for (name, report) in fixture_reports() {
            let path = fixture_path(name);
            let expected = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("read fixture {}: {error}", path.display()));
            let actual = format!(
                "{}\n",
                serde_json::to_string_pretty(&report).expect("serialize fixture")
            );

            assert_eq!(actual, expected, "fixture drifted: {}", path.display());
        }
    }

    #[test]
    fn schema_reserves_server_role_without_server_modules() {
        let value = serde_json::to_value(DoctorRole::Server).expect("serialize role");
        assert_eq!(value, json!("server"));
    }

    #[test]
    fn skip_reasons_match_contract_names() {
        let reasons = [
            DoctorSkipReason::BlockedByPreviousStep,
            DoctorSkipReason::ActiveDevice,
            DoctorSkipReason::WritesDisabled,
            DoctorSkipReason::NotSupportedByTarget,
            DoctorSkipReason::NotRequested,
            DoctorSkipReason::Cancelled,
            DoctorSkipReason::ServerDisabled,
            DoctorSkipReason::NoSymbolsExposed,
            DoctorSkipReason::NoClientsAllowed,
            DoctorSkipReason::ExternalClientPending,
        ];
        let names: Vec<String> = reasons
            .into_iter()
            .map(|reason| {
                serde_json::to_value(reason)
                    .expect("serialize")
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();

        assert_eq!(
            names,
            vec![
                "blocked_by_previous_step",
                "active_device",
                "writes_disabled",
                "not_supported_by_target",
                "not_requested",
                "cancelled",
                "server_disabled",
                "no_symbols_exposed",
                "no_clients_allowed",
                "external_client_pending",
            ]
        );
    }

    #[test]
    fn next_action_kinds_match_contract_names() {
        let actions = [
            NextActionKind::None,
            NextActionKind::PickTarget,
            NextActionKind::FixLocalIp,
            NextActionKind::AddRoute,
            NextActionKind::OpenSetup,
            NextActionKind::DownloadPowershell,
            NextActionKind::CopyXml,
            NextActionKind::OpenRuntimePane,
            NextActionKind::EnableWrite,
            NextActionKind::UseSecure,
            NextActionKind::Deploy,
            NextActionKind::RerunDoctor,
            NextActionKind::ConfigureExpose,
            NextActionKind::AddAllowedClient,
            NextActionKind::OpenFirewall,
            NextActionKind::WaitForClient,
        ];
        let names: Vec<String> = actions
            .into_iter()
            .map(|action| {
                serde_json::to_value(action)
                    .expect("serialize")
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();

        assert_eq!(
            names,
            vec![
                "none",
                "pick_target",
                "fix_local_ip",
                "add_route",
                "open_setup",
                "download_powershell",
                "copy_xml",
                "open_runtime_pane",
                "enable_write",
                "use_secure",
                "deploy",
                "rerun_doctor",
                "configure_expose",
                "add_allowed_client",
                "open_firewall",
                "wait_for_client",
            ]
        );
    }

    #[test]
    fn server_step_ids_match_contract_names() {
        let steps = [
            DoctorStepId::LocalIdentity,
            DoctorStepId::BindExposure,
            DoctorStepId::ListenerBound,
            DoctorStepId::UdpIdentifyAnswer,
            DoctorStepId::SymbolsExposed,
            DoctorStepId::ClientsAllowed,
            DoctorStepId::SymbolServe,
            DoctorStepId::SelfReadState,
            DoctorStepId::SelfHandleResolve,
            DoctorStepId::SelfSumupRead,
            DoctorStepId::SelfNotification,
            DoctorStepId::SelfWriteGuarded,
            DoctorStepId::ParserLimits,
            DoctorStepId::AllowlistEnforced,
            DoctorStepId::ExternalClientVerified,
        ];
        let names: Vec<String> = steps
            .into_iter()
            .map(|step| {
                serde_json::to_value(step)
                    .expect("serialize")
                    .as_str()
                    .unwrap()
                    .to_string()
            })
            .collect();

        assert_eq!(
            names,
            vec![
                "local_identity",
                "bind_exposure",
                "listener_bound",
                "udp_identify_answer",
                "symbols_exposed",
                "clients_allowed",
                "symbol_serve",
                "self_read_state",
                "self_handle_resolve",
                "self_sumup_read",
                "self_notification",
                "self_write_guarded",
                "parser_limits",
                "allowlist_enforced",
                "external_client_verified",
            ]
        );
    }

    #[test]
    fn error_classifier_maps_common_failures_to_remediation_and_actions() {
        let route = classify_onboarding_failure(OnboardingFailureKind::RouteMissing);
        assert_eq!(route.next_action.kind, NextActionKind::AddRoute);
        assert!(route.remediation.contains("static ADS route"));
        assert!(route.blocks_production_ready);

        let secure = classify_onboarding_failure(OnboardingFailureKind::SecureRequired);
        assert_eq!(secure.next_action.kind, NextActionKind::UseSecure);
        assert!(secure.explanation.contains("Secure ADS"));

        let fingerprint = classify_ads_error_code(1861).expect("known ADS code");
        assert_eq!(fingerprint.kind, OnboardingFailureKind::Fingerprint1861);
        assert_eq!(
            fingerprint.ads_error,
            Some(AdsErrorInfo::new(1861, "ADSERR_DEVICE_INVALIDCONTEXT"))
        );
    }

    #[test]
    fn credential_channel_classification_enforces_secret_boundary() {
        assert!(CredentialChannelClassification::TrustedSameHost.permits_credentials());
        assert!(CredentialChannelClassification::TrustedHttpsAdmin.permits_credentials());
        assert!(CredentialChannelClassification::LocalCliDirectAddRoute.permits_credentials());
        assert!(!CredentialChannelClassification::UntrustedRemotePlainTcp.permits_credentials());
        assert!(!CredentialChannelClassification::UntrustedPlainHttpNetwork.permits_credentials());
    }

    #[test]
    fn reports_contain_no_secret_fields_by_construction() {
        let route = RoutePlan {
            route_name: "trust-runtime-line-controller-1".to_string(),
            target: target_identity(),
            local: local_identity(),
            channel: CredentialChannelClassification::UntrustedRemotePlainTcp,
            automatic_route: RouteActionAvailability::DisabledUntrustedChannel,
            artifacts: vec![RouteArtifact {
                kind: RouteArtifactKind::ManualSteps,
                label: "Manual route steps".to_string(),
                filename: None,
                content_type: "text/plain".to_string(),
                content: "Add route for 192.168.10.20.1.1".to_string(),
            }],
        };
        let report_json = serde_json::to_string(&missing_route_report()).expect("report JSON");
        let route_json = serde_json::to_string(&route).expect("route JSON");
        let fixture_json = fixture_reports()
            .into_iter()
            .map(|(_, report)| serde_json::to_string(&report).expect("fixture JSON"))
            .collect::<Vec<_>>()
            .join("\n");
        let combined = format!("{report_json}\n{route_json}\n{fixture_json}").to_ascii_lowercase();

        for forbidden in ["password", "secret", "username", "token"] {
            assert!(
                !combined.contains(forbidden),
                "schema leaked forbidden field marker {forbidden}"
            );
        }

        for value in [
            serde_json::to_value(missing_route_report()).expect("report value"),
            serde_json::to_value(route).expect("route value"),
        ] {
            assert_no_forbidden_keys(&value);
        }

        for (_, report) in fixture_reports() {
            let value = serde_json::to_value(report).expect("fixture value");
            assert_no_forbidden_keys(&value);
        }
    }

    #[test]
    fn evidence_hash_inputs_are_stable_and_complete() {
        let fields: Vec<&str> = EVIDENCE_HASH_INPUTS
            .iter()
            .map(|input| input.field)
            .collect();

        assert_eq!(
            fields,
            vec![
                "runtime_identity_hash",
                "target_identity_hash",
                "allowed_clients_hash",
                "ads_config_hash",
                "symbol_snapshot_hash",
                "generated_st_hash",
                "deployed_ads_config_hash",
                "runtime_ads_status_hash",
            ]
        );
        assert!(EVIDENCE_HASH_INPUTS
            .iter()
            .all(|input| !input.input.is_empty()));
    }

    #[test]
    fn production_evidence_builder_hashes_declared_inputs() {
        let status = AdsStatusReport {
            schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
            role: DoctorRole::Client,
            overall: AdsStatusOverall::Healthy,
            runtime_identity_hash: None,
            deployed_ads_config_hash: Some(sha256_evidence_hash(b"ads toml")),
            connections: vec![AdsConnectionStatus {
                name: "line1".to_string(),
                target: Some(target_identity()),
                state: AdsConnectionStatusState::Connected,
                point_count: 1,
                degraded_points: 0,
                last_good_value_ms: Some(1781234567000),
                symbol_version: Some(7),
                summary: "Connected.".to_string(),
            }],
            summary: "ADS connections healthy.".to_string(),
        };
        let snapshots = vec![SymbolSnapshot::new(
            "line1",
            vec![SymbolDescriptor::new(
                "MAIN.Temperature",
                AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                0x4020,
                0,
                4,
            )
            .with_flag(SymbolFlag::Read)],
        )];

        let evidence = build_production_evidence(ProductionEvidenceInput {
            doctor_timestamp_ms: 1781234567000,
            runtime_identity: &local_identity(),
            target_identity: &target_identity(),
            ads_toml: "ads toml",
            symbol_snapshots: &snapshots,
            generated_st: Some("generated st"),
            deployed_ads_toml: Some("ads toml"),
            runtime_ads_status: Some(&status),
            stale_after_ms: 86_400_000,
            expires_at_ms: Some(1781320967000),
            runtime_clock_warning: None,
        })
        .expect("evidence");

        assert_eq!(
            evidence.doctor_schema_version,
            ADS_DIAGNOSTICS_SCHEMA_VERSION
        );
        assert_eq!(evidence.ads_config_hash, sha256_evidence_hash(b"ads toml"));
        assert_eq!(
            evidence.deployed_ads_config_hash.as_deref(),
            Some(evidence.ads_config_hash.as_str())
        );
        assert_eq!(
            evidence.generated_st_hash.as_deref(),
            Some(sha256_evidence_hash(b"generated st").as_str())
        );
        assert!(evidence.runtime_identity_hash.starts_with("sha256:"));
        assert!(evidence
            .target_identity_hash
            .as_deref()
            .expect("target hash")
            .starts_with("sha256:"));
        assert!(evidence.allowed_clients_hash.is_none());
        assert!(evidence.symbol_snapshot_hash.starts_with("sha256:"));
        assert!(evidence.runtime_ads_status_hash.is_some());
        assert_eq!(evidence.freshness.expires_at_ms, Some(1781320967000));
    }

    #[test]
    fn production_readiness_requires_matching_deployed_status() {
        let status = healthy_ads_status();
        let evidence = production_evidence_for_status(&status, Some(1781320967000));

        let report = evaluate_production_readiness(Some(&evidence), Some(&status), 1781234567000)
            .expect("readiness");

        assert_eq!(report.state, ProductionReadinessState::Ready);
        assert!(report.reasons.is_empty());
    }

    #[test]
    fn production_readiness_needs_recheck_on_mismatch_fault_or_expiry() {
        let status = healthy_ads_status();
        let mut evidence = production_evidence_for_status(&status, Some(1781234567001));
        let mut mismatched_status = status.clone();
        mismatched_status.deployed_ads_config_hash = Some("sha256:different".to_string());

        let mismatch =
            evaluate_production_readiness(Some(&evidence), Some(&mismatched_status), 1781234567000)
                .expect("mismatch readiness");
        assert_eq!(mismatch.state, ProductionReadinessState::NeedsRecheck);
        assert!(mismatch
            .reasons
            .contains(&ProductionReadinessReason::DeployedAdsConfigMismatch));
        assert!(mismatch
            .reasons
            .contains(&ProductionReadinessReason::RuntimeAdsStatusChanged));

        let mut faulted_status = status.clone();
        faulted_status.overall = AdsStatusOverall::Faulted;
        faulted_status.connections[0].state = AdsConnectionStatusState::Faulted;
        let faulted =
            evaluate_production_readiness(Some(&evidence), Some(&faulted_status), 1781234567000)
                .expect("faulted readiness");
        assert!(faulted
            .reasons
            .contains(&ProductionReadinessReason::RuntimeAdsFaulted));

        evidence.freshness.expires_at_ms = Some(1781234567000);
        let expired = evaluate_production_readiness(Some(&evidence), Some(&status), 1781234567001)
            .expect("expired readiness");
        assert!(expired
            .reasons
            .contains(&ProductionReadinessReason::EvidenceExpired));
    }

    #[test]
    fn production_readiness_is_not_ready_without_evidence() {
        let report = evaluate_production_readiness(None, Some(&healthy_ads_status()), 0)
            .expect("missing evidence readiness");

        assert_eq!(report.state, ProductionReadinessState::NotReady);
        assert_eq!(
            report.reasons,
            vec![ProductionReadinessReason::MissingEvidence]
        );
    }

    #[test]
    fn production_ready_requires_pass_and_evidence() {
        let mut report = DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
            .with_steps(vec![DoctorStep::new(
                DoctorStepId::Tcp48898,
                "TwinCAT router reachable",
                DoctorStepStatus::Pass,
                "Router reachable.",
            )]);

        assert_eq!(report.overall, DoctorOverall::Pass);
        assert!(!report.production_ready);

        report = report.with_evidence(ProductionEvidence {
            doctor_timestamp_ms: 1781234567000,
            doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
            runtime_identity_hash: "sha256:runtime".to_string(),
            target_identity_hash: Some("sha256:target".to_string()),
            allowed_clients_hash: None,
            ads_config_hash: "sha256:ads".to_string(),
            symbol_snapshot_hash: "sha256:symbols".to_string(),
            generated_st_hash: Some("sha256:st".to_string()),
            deployed_ads_config_hash: Some("sha256:deployed".to_string()),
            runtime_ads_status_hash: Some("sha256:status".to_string()),
            external_client_verified: false,
            external_client_kind: None,
            external_client_name: None,
            external_client_timestamp_ms: None,
            discoverable: false,
            freshness: EvidenceFreshness {
                stale_after_ms: 86_400_000,
                expires_at_ms: None,
                runtime_clock_warning: None,
            },
        });

        assert!(report.production_ready);
    }

    #[test]
    fn server_production_ready_requires_independent_client_evidence() {
        let base_step = DoctorStep::new(
            DoctorStepId::SelfSumupRead,
            "Loopback read",
            DoctorStepStatus::Pass,
            "Loopback self-client read succeeded.",
        );
        let snapshot = SymbolSnapshot::new(
            "server",
            vec![SymbolDescriptor::new(
                "global.setpoint",
                AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                0x4020,
                0,
                4,
            )
            .with_flag(SymbolFlag::Read)],
        );
        let allowed_clients = json!([
            {
                "ams_net_id": "5.23.91.12.1.1",
                "source_cidr": "192.168.10.0/24"
            }
        ]);
        let self_test_only = build_server_production_evidence(ServerProductionEvidenceInput {
            doctor_timestamp_ms: 1781234567000,
            runtime_identity: &local_identity(),
            allowed_clients: &allowed_clients,
            ads_server_config: "ads server config",
            symbol_snapshot: &snapshot,
            deployed_ads_server_config: Some("ads server config"),
            runtime_ads_status: Some(&healthy_ads_server_status()),
            external_client_verified: false,
            external_client_kind: None,
            external_client_name: None,
            external_client_timestamp_ms: None,
            discoverable: true,
            stale_after_ms: 86_400_000,
            expires_at_ms: None,
            runtime_clock_warning: None,
        })
        .expect("self-test evidence");
        let mut report = DoctorReport::for_role(
            DoctorRole::Server,
            DoctorVantage::RuntimeHost,
            DiagnosticTransport::Plain,
        )
        .with_steps(vec![base_step.clone()])
        .with_evidence(self_test_only);

        assert_eq!(report.overall, DoctorOverall::Pass);
        assert!(!report.production_ready);
        let evidence = report.evidence.as_ref().expect("evidence");
        assert!(evidence.allowed_clients_hash.is_some());
        assert!(evidence.target_identity_hash.is_none());
        assert!(evidence.discoverable);

        let pyads_external = build_server_production_evidence(ServerProductionEvidenceInput {
            doctor_timestamp_ms: 1781234567000,
            runtime_identity: &local_identity(),
            allowed_clients: &allowed_clients,
            ads_server_config: "ads server config",
            symbol_snapshot: &snapshot,
            deployed_ads_server_config: Some("ads server config"),
            runtime_ads_status: Some(&healthy_ads_server_status()),
            external_client_verified: true,
            external_client_kind: Some("pyads"),
            external_client_name: Some("ci-pyads"),
            external_client_timestamp_ms: Some(1781234567999),
            discoverable: true,
            stale_after_ms: 86_400_000,
            expires_at_ms: None,
            runtime_clock_warning: None,
        })
        .expect("pyads external evidence");
        report = DoctorReport::for_role(
            DoctorRole::Server,
            DoctorVantage::RuntimeHost,
            DiagnosticTransport::Plain,
        )
        .with_steps(vec![base_step])
        .with_evidence(pyads_external);

        assert!(!report.production_ready);
        let evidence = report.evidence.as_ref().expect("evidence");
        assert!(evidence.external_client_verified);
        assert_eq!(evidence.external_client_kind.as_deref(), Some("pyads"));

        let twincat_external = build_server_production_evidence(ServerProductionEvidenceInput {
            doctor_timestamp_ms: 1781234567000,
            runtime_identity: &local_identity(),
            allowed_clients: &allowed_clients,
            ads_server_config: "ads server config",
            symbol_snapshot: &snapshot,
            deployed_ads_server_config: Some("ads server config"),
            runtime_ads_status: Some(&healthy_ads_server_status()),
            external_client_verified: true,
            external_client_kind: Some("twincat"),
            external_client_name: Some("engineering-station"),
            external_client_timestamp_ms: Some(1781234567999),
            discoverable: true,
            stale_after_ms: 86_400_000,
            expires_at_ms: None,
            runtime_clock_warning: None,
        })
        .expect("TwinCAT external evidence");
        report = DoctorReport::for_role(
            DoctorRole::Server,
            DoctorVantage::RuntimeHost,
            DiagnosticTransport::Plain,
        )
        .with_steps(vec![DoctorStep::new(
            DoctorStepId::SelfSumupRead,
            "Loopback read",
            DoctorStepStatus::Pass,
            "Loopback self-client read succeeded.",
        )])
        .with_evidence(twincat_external);

        assert!(report.production_ready);
        let evidence = report.evidence.as_ref().expect("evidence");
        assert!(evidence.external_client_verified);
        assert_eq!(evidence.external_client_kind.as_deref(), Some("twincat"));
    }

    #[test]
    fn v1_client_evidence_json_still_deserializes_with_server_defaults() {
        let json = r#"{
          "doctor_timestamp_ms": 1781234567000,
          "doctor_schema_version": 2,
          "runtime_identity_hash": "sha256:runtime",
          "target_identity_hash": "sha256:target",
          "ads_config_hash": "sha256:ads",
          "symbol_snapshot_hash": "sha256:symbols",
          "freshness": { "stale_after_ms": 300000 }
        }"#;

        let evidence: ProductionEvidence = serde_json::from_str(json).expect("v1 evidence");

        assert_eq!(
            evidence.target_identity_hash.as_deref(),
            Some("sha256:target")
        );
        assert!(evidence.allowed_clients_hash.is_none());
        assert!(!evidence.external_client_verified);
        assert!(!evidence.discoverable);
    }

    fn fixture_reports() -> Vec<(&'static str, DoctorReport)> {
        vec![
            ("pass", pass_report()),
            ("missing_route", missing_route_report()),
            ("untrusted_channel", untrusted_channel_report()),
            ("active_device", active_device_report()),
            ("authoring_only", authoring_only_report()),
            ("secure_required", secure_required_report()),
        ]
    }

    fn fixture_path(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/ads_onboarding")
            .join(format!("{name}.json"))
    }

    fn pass_report() -> DoctorReport {
        DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
            .with_target(target_identity())
            .with_local(local_identity())
            .with_steps(vec![
                pass_step(
                    DoctorStepId::UdpIdentify,
                    "Find PLC on network",
                    "TwinCAT target answered directed ADS identify.",
                ),
                pass_step(
                    DoctorStepId::RoutePresent,
                    "Route back to truST",
                    "PLC accepts the route for 192.168.10.20.1.1.",
                ),
                pass_step(
                    DoctorStepId::SymbolUpload,
                    "Read TwinCAT symbols",
                    "Symbol table upload returned compatible symbols.",
                ),
                pass_step(
                    DoctorStepId::SumupRead,
                    "Read selected values",
                    "SUMUP read returned values for selected symbols.",
                ),
            ])
            .with_evidence(production_evidence())
            .with_summary("ADS connection is production ready from runtime host line-controller-1.")
    }

    fn untrusted_channel_report() -> DoctorReport {
        let step = DoctorStep::new(
            DoctorStepId::RoutePresent,
            "Route back to truST",
            DoctorStepStatus::Fail,
            "TwinCAT credentials cannot be forwarded over the selected remote plain TCP control endpoint.",
        )
        .with_remediation(
            "Run route setup from this computer directly to the PLC, open trusted setup web, or use the generated artifacts.",
        )
        .with_next_action(NextAction::new(NextActionKind::DownloadPowershell));

        DoctorReport::new(DoctorVantage::VscodeCli, DiagnosticTransport::Plain)
            .with_target(target_identity())
            .with_local(local_identity())
            .with_steps(vec![step])
            .with_summary("Route-add needs a trusted setup channel; automatic credential forwarding is disabled.")
    }

    fn active_device_report() -> DoctorReport {
        let step = DoctorStep::skipped(
            DoctorStepId::Notification,
            "Notification probe",
            DoctorSkipReason::ActiveDevice,
            "The ADS connection is already active; full notification probing requires an explicit pause.",
        )
        .with_remediation("Use live ADS status or pause the device before running the full doctor.")
        .with_next_action(NextAction::new(NextActionKind::RerunDoctor));

        DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
            .with_target(target_identity())
            .with_local(local_identity())
            .with_steps(vec![step])
            .with_summary("Active ADS device detected; full doctor was not allowed to open a duplicate AMS connection.")
    }

    fn authoring_only_report() -> DoctorReport {
        let step = DoctorStep::skipped(
            DoctorStepId::RoutePresent,
            "Route back to truST",
            DoctorSkipReason::NotRequested,
            "Authoring-only import does not run the runtime-host route check.",
        )
        .with_remediation("Run the ADS Doctor from the selected runtime host before deployment.")
        .with_next_action(NextAction::new(NextActionKind::OpenRuntimePane));

        DoctorReport::new(
            DoctorVantage::VscodeAuthoringOnly,
            DiagnosticTransport::Plain,
        )
        .with_target(target_identity())
        .with_steps(vec![step])
        .with_summary(
            "Symbols were imported for authoring only; production readiness is not proven.",
        )
    }

    fn secure_required_report() -> DoctorReport {
        let step = DoctorStep::failed(
            DoctorStepId::AmsTarget,
            "ADS target policy",
            "The target requires Secure ADS.",
            classify_onboarding_failure(OnboardingFailureKind::SecureRequired),
        );

        DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
            .with_target(target_identity())
            .with_local(local_identity())
            .with_steps(vec![step])
            .with_summary("Target requires Secure ADS, which this client does not support yet.")
    }

    fn missing_route_report() -> DoctorReport {
        let step = DoctorStep::failed(
            DoctorStepId::RoutePresent,
            "Route back to truST",
            "The PLC does not have a route back to 192.168.10.20.1.1.",
            classify_onboarding_failure(OnboardingFailureKind::RouteMissing),
        )
        .with_evidence("target_ip", json!("192.168.10.5"))
        .with_evidence("local_ip", json!("192.168.10.20"))
        .with_evidence("local_ams_net_id", json!("192.168.10.20.1.1"));

        DoctorReport::new(DoctorVantage::RuntimeHost, DiagnosticTransport::Plain)
            .with_target(target_identity())
            .with_local(local_identity())
            .with_steps(vec![step])
            .with_summary("1 problem: PLC has no route back to truST.")
    }

    fn pass_step(id: DoctorStepId, title: &str, detail: &str) -> DoctorStep {
        DoctorStep::new(id, title, DoctorStepStatus::Pass, detail)
    }

    fn production_evidence() -> ProductionEvidence {
        ProductionEvidence {
            doctor_timestamp_ms: 1781234567000,
            doctor_schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
            runtime_identity_hash: "sha256:runtime".to_string(),
            target_identity_hash: Some("sha256:target".to_string()),
            allowed_clients_hash: None,
            ads_config_hash: "sha256:ads".to_string(),
            symbol_snapshot_hash: "sha256:symbols".to_string(),
            generated_st_hash: Some("sha256:generated-st".to_string()),
            deployed_ads_config_hash: Some("sha256:deployed-ads".to_string()),
            runtime_ads_status_hash: Some("sha256:ads-status".to_string()),
            external_client_verified: false,
            external_client_kind: None,
            external_client_name: None,
            external_client_timestamp_ms: None,
            discoverable: false,
            freshness: EvidenceFreshness {
                stale_after_ms: 86_400_000,
                expires_at_ms: Some(1781320967000),
                runtime_clock_warning: None,
            },
        }
    }

    fn healthy_ads_status() -> AdsStatusReport {
        AdsStatusReport {
            schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
            role: DoctorRole::Client,
            overall: AdsStatusOverall::Healthy,
            runtime_identity_hash: None,
            deployed_ads_config_hash: Some(sha256_evidence_hash(b"ads toml")),
            connections: vec![AdsConnectionStatus {
                name: "line1".to_string(),
                target: Some(target_identity()),
                state: AdsConnectionStatusState::Connected,
                point_count: 1,
                degraded_points: 0,
                last_good_value_ms: Some(1781234567000),
                symbol_version: Some(7),
                summary: "Connected.".to_string(),
            }],
            summary: "ADS connections healthy.".to_string(),
        }
    }

    fn healthy_ads_server_status() -> AdsStatusReport {
        AdsStatusReport {
            schema_version: ADS_DIAGNOSTICS_SCHEMA_VERSION,
            role: DoctorRole::Server,
            overall: AdsStatusOverall::Healthy,
            runtime_identity_hash: Some("sha256:runtime".to_string()),
            deployed_ads_config_hash: Some(sha256_evidence_hash(b"ads server config")),
            connections: Vec::new(),
            summary: "ADS server healthy.".to_string(),
        }
    }

    fn production_evidence_for_status(
        status: &AdsStatusReport,
        expires_at_ms: Option<u64>,
    ) -> ProductionEvidence {
        let snapshots = vec![SymbolSnapshot::new(
            "line1",
            vec![SymbolDescriptor::new(
                "MAIN.Temperature",
                AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
                0x4020,
                0,
                4,
            )
            .with_flag(SymbolFlag::Read)],
        )];
        build_production_evidence(ProductionEvidenceInput {
            doctor_timestamp_ms: 1781234567000,
            runtime_identity: &local_identity(),
            target_identity: &target_identity(),
            ads_toml: "ads toml",
            symbol_snapshots: &snapshots,
            generated_st: Some("generated st"),
            deployed_ads_toml: Some("ads toml"),
            runtime_ads_status: Some(status),
            stale_after_ms: 86_400_000,
            expires_at_ms,
            runtime_clock_warning: None,
        })
        .expect("production evidence")
    }

    fn target_identity() -> TargetIdentity {
        TargetIdentity {
            name: Some("CX-1234".to_string()),
            ip: "192.168.10.5".to_string(),
            ams_net_id: "5.23.91.12.1.1".to_string(),
            ams_port: 851,
            tc_version: Some("3.1.4024".to_string()),
        }
    }

    fn local_identity() -> LocalIdentity {
        LocalIdentity {
            host_name: Some("line-controller-1".to_string()),
            chosen_ip: "192.168.10.20".to_string(),
            ams_net_id: "192.168.10.20.1.1".to_string(),
            nic: Some("eth0".to_string()),
            candidates: Vec::new(),
            classification: LocalNetworkClassification::Lan,
        }
    }

    fn assert_no_forbidden_keys(value: &serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for (key, nested) in map {
                    let normalized = key.to_ascii_lowercase();
                    for forbidden in ["password", "secret", "credential", "username", "token"] {
                        assert!(
                            !normalized.contains(forbidden),
                            "schema leaked forbidden key marker {forbidden}: {key}"
                        );
                    }
                    assert_no_forbidden_keys(nested);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    assert_no_forbidden_keys(item);
                }
            }
            _ => {}
        }
    }
}
