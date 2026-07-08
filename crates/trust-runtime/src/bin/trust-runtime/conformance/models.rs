const V1_PROFILE_NAME: &str = "trust-conformance-v1";
const V2_PROFILE_NAME: &str = "trust-conformance-v2";

const V1_CATEGORIES: [&str; 6] = [
    "timers",
    "edges",
    "scan_cycle",
    "init_reset",
    "arithmetic",
    "memory_map",
];

const CATEGORIES: [&str; 16] = [
    "timers",
    "edges",
    "scan_cycle",
    "init_reset",
    "arithmetic",
    "memory_map",
    "strings",
    "arrays",
    "structs",
    "enums",
    "nested_values",
    "oop_dispatch",
    "references",
    "retain_matrix",
    "scheduler",
    "comms_determinism",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SummaryContract {
    version: u32,
    profile: &'static str,
}

impl SummaryContract {
    const fn v1() -> Self {
        Self {
            version: 1,
            profile: V1_PROFILE_NAME,
        }
    }

    const fn v2() -> Self {
        Self {
            version: 2,
            profile: V2_PROFILE_NAME,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum CaseKind {
    #[default]
    Runtime,
    CompileError,
    ConnectorStatusTrace,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct RestartDirective {
    before_cycle: u32,
    mode: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct ConnectorStatusTraceStep {
    source: String,
    state: String,
    degraded_points: usize,
    error_policy: Option<String>,
    expected_state: Option<String>,
    expected_health: Option<String>,
    detail: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
struct CaseManifest {
    id: String,
    category: String,
    description: Option<String>,
    kind: CaseKind,
    cycles: u32,
    sources: Vec<String>,
    watch_globals: Vec<String>,
    watch_direct: Vec<String>,
    advance_ms: Vec<i64>,
    input_series: BTreeMap<String, Vec<String>>,
    direct_input_series: BTreeMap<String, Vec<String>>,
    restarts: Vec<RestartDirective>,
    connector_status_steps: Vec<ConnectorStatusTraceStep>,
}

#[derive(Debug, Clone)]
struct CaseDefinition {
    id: String,
    category: String,
    dir: PathBuf,
    manifest: CaseManifest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaseStatus {
    Passed,
    Failed,
    Error,
    Skipped,
}

impl CaseStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Error => "error",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct SummaryReason {
    code: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct SummaryCaseResult {
    case_id: String,
    category: String,
    status: String,
    expected_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    actual_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    duration_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cycles: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<SummaryReason>,
}

#[derive(Debug, Clone, Serialize)]
struct SummaryTotals {
    total: usize,
    passed: usize,
    failed: usize,
    errors: usize,
    skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
struct RuntimeSummaryMeta {
    name: String,
    version: String,
    target: String,
}

#[derive(Debug, Clone, Serialize)]
struct SummaryOutput {
    version: u32,
    profile: String,
    generated_at_utc: String,
    ordering: String,
    runtime: RuntimeSummaryMeta,
    summary: SummaryTotals,
    results: Vec<SummaryCaseResult>,
}

#[derive(Debug, Clone)]
struct CaseArtifact {
    payload: serde_json::Value,
    cycles: Option<u64>,
}
