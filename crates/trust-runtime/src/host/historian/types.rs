#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordingMode {
    All,
    Allowlist,
}

#[derive(Debug, Clone)]
pub struct AlertRule {
    pub name: SmolStr,
    pub variable: SmolStr,
    pub above: Option<f64>,
    pub below: Option<f64>,
    pub debounce_samples: u32,
    pub hook: Option<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct HistorianConfig {
    pub enabled: bool,
    pub sample_interval_ms: u64,
    pub mode: RecordingMode,
    pub include: Vec<SmolStr>,
    pub history_path: PathBuf,
    pub max_entries: usize,
    pub prometheus_enabled: bool,
    pub prometheus_path: SmolStr,
    pub alerts: Vec<AlertRule>,
}

impl Default for HistorianConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            sample_interval_ms: 1_000,
            mode: RecordingMode::All,
            include: Vec::new(),
            history_path: PathBuf::from("history/historian.jsonl"),
            max_entries: 20_000,
            prometheus_enabled: true,
            prometheus_path: SmolStr::new("/metrics"),
            alerts: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistorianSample {
    pub timestamp_ms: u128,
    pub source_time_ns: i64,
    pub variable: String,
    pub value: HistorianValue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum HistorianValue {
    Bool(bool),
    Integer(i64),
    Unsigned(u64),
    Float(f64),
    String(String),
}

impl HistorianValue {
    fn as_f64(&self) -> Option<f64> {
        match self {
            Self::Bool(value) => Some(if *value { 1.0 } else { 0.0 }),
            Self::Integer(value) => Some(*value as f64),
            Self::Unsigned(value) => Some(*value as f64),
            Self::Float(value) => Some(*value),
            Self::String(_) => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AlertState {
    Triggered,
    Cleared,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HistorianAlertEvent {
    pub timestamp_ms: u128,
    pub rule: String,
    pub variable: String,
    pub state: AlertState,
    pub value: Option<f64>,
    pub threshold: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct HistorianPrometheusSnapshot {
    pub samples_total: u64,
    pub series_total: usize,
    pub alerts_total: u64,
}

#[derive(Debug, Clone)]
enum HookTarget {
    Log,
    File(PathBuf),
    Webhook(SmolStr),
}

#[derive(Debug, Clone)]
struct CompiledAlertRule {
    name: SmolStr,
    variable: SmolStr,
    above: Option<f64>,
    below: Option<f64>,
    debounce_samples: u32,
    hook: Option<HookTarget>,
}

#[derive(Debug, Clone, Default)]
struct AlertTracker {
    active: bool,
    consecutive: u32,
}

#[derive(Debug, Default)]
struct HistorianInner {
    samples: VecDeque<HistorianSample>,
    tracked_variables: HashSet<String>,
    samples_total: u64,
    last_capture_ms: Option<u128>,
    alert_trackers: HashMap<SmolStr, AlertTracker>,
    alerts: VecDeque<HistorianAlertEvent>,
    alerts_total: u64,
}

#[derive(Debug)]
pub struct HistorianService {
    config: HistorianConfig,
    include_patterns: Vec<Pattern>,
    alert_rules: Vec<CompiledAlertRule>,
    inner: Mutex<HistorianInner>,
}
