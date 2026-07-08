use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use trust_ads_core::SymbolSnapshot;

use super::{DiagnosticMap, FailureClassification, ADS_DIAGNOSTICS_SCHEMA_VERSION};

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
    NotReady,
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
    NotReady,
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
