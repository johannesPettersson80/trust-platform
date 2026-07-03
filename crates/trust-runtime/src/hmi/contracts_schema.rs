const fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiSchemaResult {
    pub version: u32,
    pub schema_revision: u64,
    pub mode: &'static str,
    pub read_only: bool,
    pub resource: String,
    pub generated_at_ms: u128,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor_error: Option<String>,
    pub theme: HmiThemeSchema,
    pub responsive: HmiResponsiveSchema,
    pub export: HmiExportSchema,
    pub pages: Vec<HmiPageSchema>,
    pub widgets: Vec<HmiWidgetSchema>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiWidgetSchema {
    pub id: String,
    pub path: String,
    pub label: String,
    pub data_type: String,
    pub access: &'static str,
    pub writable: bool,
    pub widget: String,
    pub source: String,
    pub page: String,
    pub group: String,
    pub order: i32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub zones: Vec<HmiZoneSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub off_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub section_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub widget_span: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alarm_deadband: Option<f64>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub inferred_interface: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_page: Option<String>,
    pub unit: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiThemeSchema {
    pub style: String,
    pub accent: String,
    pub background: String,
    pub surface: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiPageSchema {
    pub id: String,
    pub title: String,
    pub order: i32,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub svg: Option<String>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<HmiSectionSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<HmiProcessBindingSchema>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiSectionSchema {
    pub title: String,
    pub span: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub widget_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub module_meta: Vec<HmiModuleMeta>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiModuleMeta {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_page: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiProcessBindingSchema {
    pub selector: String,
    pub attribute: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<HmiProcessScaleSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmiProcessScaleSchema {
    pub min: f64,
    pub max: f64,
    pub output_min: f64,
    pub output_max: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HmiZoneSchema {
    pub from: f64,
    pub to: f64,
    pub color: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiResponsiveSchema {
    pub mode: String,
    pub mobile_max_px: u32,
    pub tablet_max_px: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiExportSchema {
    pub enabled: bool,
    pub route: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiValuesResult {
    pub connected: bool,
    pub timestamp_ms: u128,
    pub source_time_ns: Option<i64>,
    pub freshness_ms: Option<u64>,
    pub values: IndexMap<String, HmiValueRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiValueRecord {
    pub v: serde_json::Value,
    pub q: &'static str,
    pub ts_ms: u128,
}

#[derive(Debug, Default)]
pub struct HmiLiveState {
    trend_samples: BTreeMap<String, VecDeque<HmiTrendSample>>,
    alarms: BTreeMap<String, HmiAlarmState>,
    history: VecDeque<HmiAlarmHistoryRecord>,
    last_connected: bool,
    last_timestamp_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiTrendResult {
    pub connected: bool,
    pub timestamp_ms: u128,
    pub duration_ms: u64,
    pub buckets: usize,
    pub series: Vec<HmiTrendSeries>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiTrendSeries {
    pub id: String,
    pub label: String,
    pub unit: Option<String>,
    pub points: Vec<HmiTrendPoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiTrendPoint {
    pub ts_ms: u128,
    pub value: f64,
    pub min: f64,
    pub max: f64,
    pub samples: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiAlarmResult {
    pub connected: bool,
    pub timestamp_ms: u128,
    pub active: Vec<HmiAlarmRecord>,
    pub history: Vec<HmiAlarmHistoryRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiAlarmRecord {
    pub id: String,
    pub widget_id: String,
    pub path: String,
    pub label: String,
    pub state: &'static str,
    pub acknowledged: bool,
    pub raised_at_ms: u128,
    pub last_change_ms: u128,
    pub value: f64,
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiAlarmHistoryRecord {
    pub id: String,
    pub widget_id: String,
    pub path: String,
    pub label: String,
    pub event: &'static str,
    pub timestamp_ms: u128,
    pub value: f64,
}
