//! HMI schema/value contract helpers.

#![allow(missing_docs)]

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::Path;

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use trust_hir::types::Type;

use crate::debug::dap::value_type_name;
use crate::debug::DebugSnapshot;
use crate::runtime::RuntimeMetadata;
use crate::value::Value;

const HMI_SCHEMA_VERSION: u32 = 1;
const HMI_DESCRIPTOR_VERSION: u32 = 1;
const DEFAULT_PAGE_ID: &str = "overview";
const DEFAULT_TREND_PAGE_ID: &str = "trends";
const DEFAULT_ALARM_PAGE_ID: &str = "alarms";
const DEFAULT_GROUP_NAME: &str = "General";
const DEFAULT_RESPONSIVE_MODE: &str = "auto";
const TREND_HISTORY_LIMIT: usize = 4096;
const ALARM_HISTORY_LIMIT: usize = 1024;
const HMI_DIAG_UNKNOWN_BIND: &str = "HMI_BIND_UNKNOWN_PATH";
const HMI_DIAG_TYPE_MISMATCH: &str = "HMI_BIND_TYPE_MISMATCH";
const HMI_DIAG_UNKNOWN_WIDGET: &str = "HMI_UNKNOWN_WIDGET_KIND";

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirDescriptor {
    pub config: HmiDirConfig,
    pub pages: Vec<HmiDirPage>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirConfig {
    pub version: Option<u32>,
    #[serde(default)]
    pub theme: HmiDirTheme,
    #[serde(default)]
    pub layout: HmiDirLayout,
    #[serde(default)]
    pub write: HmiDirWrite,
    #[serde(default, rename = "alarm")]
    pub alarms: Vec<HmiDirAlarm>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirTheme {
    pub style: Option<String>,
    pub accent: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirLayout {
    pub navigation: Option<String>,
    pub header: Option<bool>,
    pub header_title: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirWrite {
    pub enabled: Option<bool>,
    #[serde(default)]
    pub allow: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirAlarm {
    pub bind: String,
    pub high: Option<f64>,
    pub low: Option<f64>,
    pub deadband: Option<f64>,
    pub inferred: Option<bool>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirPage {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub order: i32,
    pub kind: String,
    pub duration_ms: Option<u64>,
    pub svg: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    pub signals: Vec<String>,
    pub sections: Vec<HmiDirSection>,
    pub bindings: Vec<HmiDirProcessBinding>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirSection {
    pub title: String,
    pub span: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tier: Option<String>,
    pub widgets: Vec<HmiDirWidget>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirWidget {
    pub widget_type: Option<String>,
    pub bind: String,
    pub label: Option<String>,
    pub unit: Option<String>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub span: Option<u32>,
    pub on_color: Option<String>,
    pub off_color: Option<String>,
    pub inferred_interface: Option<bool>,
    pub detail_page: Option<String>,
    pub zones: Vec<HmiZoneSchema>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiDirProcessBinding {
    pub selector: String,
    pub attribute: String,
    pub source: String,
    pub format: Option<String>,
    pub map: BTreeMap<String, String>,
    pub scale: Option<HmiProcessScaleSchema>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HmiBindingDiagnostic {
    pub code: &'static str,
    pub message: String,
    pub bind: String,
    pub widget: Option<String>,
    pub page: String,
    pub section: Option<String>,
}

#[derive(Debug, Clone)]
struct HmiTrendSample {
    ts_ms: u128,
    value: f64,
}

#[derive(Debug, Clone)]
struct HmiAlarmState {
    id: String,
    widget_id: String,
    path: String,
    label: String,
    active: bool,
    acknowledged: bool,
    raised_at_ms: u128,
    last_change_ms: u128,
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
}

#[derive(Debug, Clone)]
enum HmiBinding {
    ProgramVar { program: SmolStr, variable: SmolStr },
    Global { name: SmolStr },
}

#[derive(Debug, Clone)]
pub enum HmiWriteBinding {
    ProgramVar { program: SmolStr, variable: SmolStr },
    Global { name: SmolStr },
}

#[derive(Debug, Clone)]
pub struct HmiWritePoint {
    pub id: String,
    pub path: String,
    pub binding: HmiWriteBinding,
}

#[derive(Debug, Clone)]
struct HmiPoint {
    id: String,
    path: String,
    label: String,
    data_type: String,
    access: &'static str,
    writable: bool,
    widget: String,
    source: String,
    page: String,
    group: String,
    order: i32,
    zones: Vec<HmiZoneSchema>,
    on_color: Option<String>,
    off_color: Option<String>,
    section_title: Option<String>,
    widget_span: Option<u32>,
    alarm_deadband: Option<f64>,
    inferred_interface: bool,
    detail_page: Option<String>,
    unit: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    binding: HmiBinding,
}

#[derive(Debug, Clone, Copy)]
pub struct HmiSourceRef<'a> {
    pub path: &'a Path,
    pub text: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HmiScaffoldMode {
    Init,
    Update,
    Reset,
}

impl HmiScaffoldMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Init => "init",
            Self::Update => "update",
            Self::Reset => "reset",
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HmiScaffoldSummary {
    pub style: String,
    pub files: Vec<HmiScaffoldFileSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct HmiScaffoldFileSummary {
    pub path: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiBindingsCatalog {
    pub programs: Vec<HmiBindingsProgram>,
    pub globals: Vec<HmiBindingsVariable>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiBindingsProgram {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    pub variables: Vec<HmiBindingsVariable>,
}

#[derive(Debug, Clone, Serialize)]
pub struct HmiBindingsVariable {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub data_type: String,
    pub qualifier: String,
    pub writable: bool,
    #[serde(default, skip_serializing_if = "is_false")]
    pub inferred_interface: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enum_values: Vec<String>,
}

impl HmiScaffoldSummary {
    #[must_use]
    pub fn render_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "Generated hmi/ with {} files:", self.files.len());
        for entry in &self.files {
            let _ = writeln!(out, "  {}  - {}", entry.path, entry.detail);
        }
        out.trim_end().to_string()
    }
}

#[derive(Debug, Clone, Default)]
pub struct HmiCustomization {
    theme: HmiThemeConfig,
    responsive: HmiResponsiveConfig,
    export: HmiExportConfig,
    write: HmiWriteConfig,
    pages: Vec<HmiPageConfig>,
    dir_descriptor: Option<HmiDirDescriptor>,
    widget_overrides: BTreeMap<String, HmiWidgetOverride>,
    annotation_overrides: BTreeMap<String, HmiWidgetOverride>,
}

#[derive(Debug, Clone, Default)]
struct HmiThemeConfig {
    style: Option<String>,
    accent: Option<String>,
}

#[derive(Debug, Clone)]
struct ScaffoldPoint {
    program: String,
    raw_name: String,
    path: String,
    label: String,
    data_type: String,
    widget: String,
    writable: bool,
    qualifier: SourceVarKind,
    inferred_interface: bool,
    type_bucket: ScaffoldTypeBucket,
    unit: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    enum_values: Vec<String>,
}

#[derive(Debug, Clone)]
struct ScaffoldSection {
    title: String,
    span: u32,
    tier: Option<String>,
    widgets: Vec<ScaffoldPoint>,
}

#[derive(Debug)]
struct ScaffoldOverviewResult {
    sections: Vec<ScaffoldSection>,
    equipment_groups: Vec<ScaffoldEquipmentGroup>,
}

#[derive(Debug, Clone)]
struct ScaffoldEquipmentGroup {
    #[allow(dead_code)]
    prefix: String,
    title: String,
    detail_page_id: String,
    widgets: Vec<ScaffoldPoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SourceVarKind {
    Input,
    Output,
    InOut,
    Global,
    External,
    Var,
    Temp,
    Unknown,
}

impl SourceVarKind {
    fn is_external(self) -> bool {
        matches!(
            self,
            Self::Input | Self::Output | Self::InOut | Self::Global | Self::External
        )
    }

    fn is_writable(self) -> bool {
        matches!(self, Self::Input | Self::InOut)
    }

    fn qualifier_label(self) -> &'static str {
        match self {
            Self::Input => "VAR_INPUT",
            Self::Output => "VAR_OUTPUT",
            Self::InOut => "VAR_IN_OUT",
            Self::Global => "VAR_GLOBAL",
            Self::External => "VAR_EXTERNAL",
            Self::Var => "VAR",
            Self::Temp => "VAR_TEMP",
            Self::Unknown => "VAR",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ScaffoldTypeBucket {
    Bool,
    Numeric,
    Text,
    Composite,
    Other,
}

#[derive(Debug, Default)]
struct SourceSymbolIndex {
    program_vars: HashMap<String, SourceVarKind>,
    programs_with_entries: HashSet<String>,
    program_files: HashMap<String, String>,
    globals: HashSet<String>,
}

#[derive(Debug, Clone)]
struct HmiPageConfig {
    id: String,
    title: String,
    icon: Option<String>,
    order: i32,
    kind: String,
    duration_ms: Option<u64>,
    svg: Option<String>,
    hidden: bool,
    signals: Vec<String>,
    sections: Vec<HmiSectionConfig>,
    bindings: Vec<HmiProcessBindingSchema>,
}

#[derive(Debug, Clone)]
struct HmiSectionConfig {
    title: String,
    span: u32,
    tier: Option<String>,
    widget_paths: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct HmiResponsiveConfig {
    mode: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct HmiExportConfig {
    enabled: Option<bool>,
}

#[derive(Debug, Clone, Default)]
struct HmiWriteConfig {
    enabled: Option<bool>,
    allow: BTreeSet<String>,
}

#[derive(Debug, Clone, Default)]
struct HmiWidgetOverride {
    label: Option<String>,
    unit: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    widget: Option<String>,
    page: Option<String>,
    group: Option<String>,
    order: Option<i32>,
    zones: Vec<HmiZoneSchema>,
    on_color: Option<String>,
    off_color: Option<String>,
    section_title: Option<String>,
    widget_span: Option<u32>,
    alarm_deadband: Option<f64>,
    inferred_interface: Option<bool>,
    detail_page: Option<String>,
}

impl HmiWidgetOverride {
    fn is_empty(&self) -> bool {
        self.label.is_none()
            && self.unit.is_none()
            && self.min.is_none()
            && self.max.is_none()
            && self.widget.is_none()
            && self.page.is_none()
            && self.group.is_none()
            && self.order.is_none()
            && self.zones.is_empty()
            && self.on_color.is_none()
            && self.off_color.is_none()
            && self.section_title.is_none()
            && self.widget_span.is_none()
            && self.alarm_deadband.is_none()
            && self.inferred_interface.is_none()
            && self.detail_page.is_none()
    }

    fn merge_from(&mut self, other: &Self) {
        if other.label.is_some() {
            self.label = other.label.clone();
        }
        if other.unit.is_some() {
            self.unit = other.unit.clone();
        }
        if other.min.is_some() {
            self.min = other.min;
        }
        if other.max.is_some() {
            self.max = other.max;
        }
        if other.widget.is_some() {
            self.widget = other.widget.clone();
        }
        if other.page.is_some() {
            self.page = other.page.clone();
        }
        if other.group.is_some() {
            self.group = other.group.clone();
        }
        if other.order.is_some() {
            self.order = other.order;
        }
        if !other.zones.is_empty() {
            self.zones = other.zones.clone();
        }
        if other.on_color.is_some() {
            self.on_color = other.on_color.clone();
        }
        if other.off_color.is_some() {
            self.off_color = other.off_color.clone();
        }
        if other.section_title.is_some() {
            self.section_title = other.section_title.clone();
        }
        if other.widget_span.is_some() {
            self.widget_span = other.widget_span;
        }
        if other.alarm_deadband.is_some() {
            self.alarm_deadband = other.alarm_deadband;
        }
        if other.inferred_interface.is_some() {
            self.inferred_interface = other.inferred_interface;
        }
        if other.detail_page.is_some() {
            self.detail_page = other.detail_page.clone();
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct HmiTomlFile {
    #[serde(default)]
    theme: HmiTomlTheme,
    #[serde(default)]
    responsive: HmiTomlResponsive,
    #[serde(default)]
    export: HmiTomlExport,
    #[serde(default)]
    write: HmiTomlWrite,
    #[serde(default)]
    pages: Vec<HmiTomlPage>,
    #[serde(default)]
    widgets: BTreeMap<String, HmiTomlWidgetOverride>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiTomlTheme {
    style: Option<String>,
    accent: Option<String>,
}

#[derive(Debug, Deserialize)]
struct HmiTomlPage {
    id: String,
    title: Option<String>,
    order: Option<i32>,
    kind: Option<String>,
    duration_s: Option<u64>,
    signals: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiTomlWidgetOverride {
    label: Option<String>,
    unit: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    widget: Option<String>,
    page: Option<String>,
    group: Option<String>,
    order: Option<i32>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiTomlResponsive {
    mode: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiTomlExport {
    enabled: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiTomlWrite {
    enabled: Option<bool>,
    #[serde(default)]
    allow: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiDirConfigToml {
    version: Option<u32>,
    #[serde(default)]
    theme: HmiDirTheme,
    #[serde(default)]
    layout: HmiDirLayout,
    #[serde(default)]
    write: HmiDirWrite,
    #[serde(default, rename = "alarm")]
    alarms: Vec<HmiDirAlarm>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiDirPageToml {
    title: Option<String>,
    icon: Option<String>,
    order: Option<i32>,
    kind: Option<String>,
    duration_s: Option<u64>,
    svg: Option<String>,
    #[serde(default)]
    hidden: Option<bool>,
    #[serde(default)]
    signals: Vec<String>,
    #[serde(default, rename = "section")]
    sections: Vec<HmiDirSectionToml>,
    #[serde(default, rename = "bind")]
    bindings: Vec<HmiDirProcessBindingToml>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiDirSectionToml {
    title: Option<String>,
    span: Option<u32>,
    tier: Option<String>,
    #[serde(default, rename = "widget")]
    widgets: Vec<HmiDirWidgetToml>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiDirWidgetToml {
    #[serde(rename = "type")]
    widget_type: Option<String>,
    bind: Option<String>,
    label: Option<String>,
    unit: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    span: Option<u32>,
    on_color: Option<String>,
    off_color: Option<String>,
    inferred_interface: Option<bool>,
    detail_page: Option<String>,
    #[serde(default)]
    zones: Vec<HmiZoneSchema>,
}

#[derive(Debug, Default, Deserialize)]
struct HmiDirProcessBindingToml {
    selector: Option<String>,
    attribute: Option<String>,
    source: Option<String>,
    format: Option<String>,
    #[serde(default)]
    map: BTreeMap<String, String>,
    scale: Option<HmiProcessScaleToml>,
}

#[derive(Debug, Clone, Deserialize)]
struct HmiProcessScaleToml {
    min: f64,
    max: f64,
    output_min: f64,
    output_max: f64,
}

impl HmiCustomization {
    pub fn write_enabled(&self) -> bool {
        self.write.enabled.unwrap_or(false)
    }

    pub fn dir_descriptor(&self) -> Option<&HmiDirDescriptor> {
        self.dir_descriptor.as_ref()
    }

    pub fn write_allowlist(&self) -> &BTreeSet<String> {
        &self.write.allow
    }

    pub fn write_target_allowed(&self, target: &str) -> bool {
        self.write.allow.contains(target)
    }
}

impl From<HmiTomlWidgetOverride> for HmiWidgetOverride {
    fn from(value: HmiTomlWidgetOverride) -> Self {
        Self {
            label: value.label,
            unit: value.unit,
            min: value.min,
            max: value.max,
            widget: value.widget,
            page: value.page,
            group: value.group,
            order: value.order,
            zones: Vec::new(),
            on_color: None,
            off_color: None,
            section_title: None,
            widget_span: None,
            alarm_deadband: None,
            inferred_interface: None,
            detail_page: None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct ThemePalette {
    style: &'static str,
    accent: &'static str,
    background: &'static str,
    surface: &'static str,
    text: &'static str,
}

pub fn load_customization(
    project_root: Option<&Path>,
    sources: &[HmiSourceRef<'_>],
) -> HmiCustomization {
    let mut customization = HmiCustomization {
        annotation_overrides: parse_annotations(sources),
        ..HmiCustomization::default()
    };

    if let Some(root) = project_root {
        if let Some(dir_descriptor) = load_hmi_dir(root) {
            apply_hmi_dir_descriptor(&mut customization, &dir_descriptor);
            customization.dir_descriptor = Some(dir_descriptor);
        } else if let Ok(parsed) = load_hmi_toml(root) {
            apply_legacy_hmi_toml(&mut customization, parsed);
        }
    }

    customization
}

pub fn try_load_customization(
    project_root: Option<&Path>,
    sources: &[HmiSourceRef<'_>],
) -> anyhow::Result<HmiCustomization> {
    let mut customization = HmiCustomization {
        annotation_overrides: parse_annotations(sources),
        ..HmiCustomization::default()
    };

    let Some(root) = project_root else {
        return Ok(customization);
    };

    if root.join("hmi").is_dir() {
        let dir_descriptor = load_hmi_dir_impl(root)?;
        apply_hmi_dir_descriptor(&mut customization, &dir_descriptor);
        customization.dir_descriptor = Some(dir_descriptor);
        return Ok(customization);
    }

    if root.join("hmi.toml").is_file() {
        let parsed = load_hmi_toml(root)?;
        apply_legacy_hmi_toml(&mut customization, parsed);
    }

    Ok(customization)
}

fn apply_legacy_hmi_toml(customization: &mut HmiCustomization, parsed: HmiTomlFile) {
    customization.theme.style = parsed.theme.style;
    customization.theme.accent = parsed.theme.accent;
    customization.responsive.mode = parsed.responsive.mode;
    customization.export.enabled = parsed.export.enabled;
    customization.write.enabled = parsed.write.enabled;
    customization.write.allow = parsed
        .write
        .allow
        .into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect();
    customization.pages = parsed
        .pages
        .into_iter()
        .enumerate()
        .filter_map(|(idx, page)| {
            let id = page.id.trim();
            if id.is_empty() {
                return None;
            }
            let order = page.order.unwrap_or((idx as i32) * 10);
            let title = page
                .title
                .filter(|title| !title.trim().is_empty())
                .unwrap_or_else(|| title_case(id));
            let kind = normalize_page_kind(page.kind.as_deref()).to_string();
            let signals = page
                .signals
                .unwrap_or_default()
                .into_iter()
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect::<Vec<_>>();
            Some(HmiPageConfig {
                id: id.to_string(),
                title,
                icon: None,
                order,
                kind,
                duration_ms: page.duration_s.map(|seconds| seconds.saturating_mul(1_000)),
                svg: None,
                hidden: false,
                signals,
                sections: Vec::new(),
                bindings: Vec::new(),
            })
        })
        .collect();
    customization.widget_overrides = parsed
        .widgets
        .into_iter()
        .filter_map(|(path, override_spec)| {
            let key = path.trim();
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), HmiWidgetOverride::from(override_spec)))
        })
        .collect();
}

pub fn validate_hmi_bindings(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    descriptor: &HmiDirDescriptor,
) -> Vec<HmiBindingDiagnostic> {
    let points = collect_points(resource_name, metadata, snapshot, true);
    let by_path = points
        .iter()
        .map(|point| (point.path.as_str(), point))
        .collect::<HashMap<_, _>>();
    let mut diagnostics = Vec::new();

    for page in &descriptor.pages {
        for section in &page.sections {
            for widget in &section.widgets {
                let bind = widget.bind.trim();
                if bind.is_empty() {
                    continue;
                }
                let widget_kind = widget
                    .widget_type
                    .as_ref()
                    .map(|kind| kind.trim().to_ascii_lowercase())
                    .filter(|kind| !kind.is_empty());
                let Some(point) = by_path.get(bind) else {
                    diagnostics.push(HmiBindingDiagnostic {
                        code: HMI_DIAG_UNKNOWN_BIND,
                        message: format!("unknown binding path '{bind}'"),
                        bind: bind.to_string(),
                        widget: widget_kind.clone(),
                        page: page.id.clone(),
                        section: Some(section.title.clone()),
                    });
                    continue;
                };
                let Some(widget_kind) = widget_kind else {
                    continue;
                };
                if !is_supported_widget_kind(widget_kind.as_str()) {
                    diagnostics.push(HmiBindingDiagnostic {
                        code: HMI_DIAG_UNKNOWN_WIDGET,
                        message: format!("unknown widget kind '{widget_kind}'"),
                        bind: bind.to_string(),
                        widget: Some(widget_kind),
                        page: page.id.clone(),
                        section: Some(section.title.clone()),
                    });
                    continue;
                }
                if !widget_kind_matches_point(widget_kind.as_str(), point) {
                    diagnostics.push(HmiBindingDiagnostic {
                        code: HMI_DIAG_TYPE_MISMATCH,
                        message: format!(
                            "widget '{widget_kind}' is incompatible with '{}' ({})",
                            point.path, point.data_type
                        ),
                        bind: bind.to_string(),
                        widget: Some(widget_kind),
                        page: page.id.clone(),
                        section: Some(section.title.clone()),
                    });
                }
            }
        }
        for binding in &page.bindings {
            let bind = binding.source.trim();
            if bind.is_empty() {
                continue;
            }
            if !by_path.contains_key(bind) {
                diagnostics.push(HmiBindingDiagnostic {
                    code: HMI_DIAG_UNKNOWN_BIND,
                    message: format!("unknown binding path '{bind}'"),
                    bind: bind.to_string(),
                    widget: Some("process.bind".to_string()),
                    page: page.id.clone(),
                    section: None,
                });
            }
        }
    }

    diagnostics.sort_by(|left, right| {
        left.code
            .cmp(right.code)
            .then_with(|| left.page.cmp(&right.page))
            .then_with(|| left.bind.cmp(&right.bind))
            .then_with(|| left.section.cmp(&right.section))
    });
    diagnostics
}

pub fn build_schema(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    read_only: bool,
    customization: Option<&HmiCustomization>,
) -> HmiSchemaResult {
    let mut points = collect_points(resource_name, metadata, snapshot, read_only);

    if let Some(customization) = customization {
        for (idx, point) in points.iter_mut().enumerate() {
            point.order = idx as i32;
            if let Some(annotation) = customization.annotation_overrides.get(point.path.as_str()) {
                apply_widget_override(point, annotation);
            }
            if let Some(file_override) = customization.widget_overrides.get(point.path.as_str()) {
                apply_widget_override(point, file_override);
            }
            normalize_point(point);
        }
    }
    let (pages, page_order) = resolve_pages(&mut points, customization);
    let theme = resolve_theme(customization.map(|value| &value.theme));
    let responsive = resolve_responsive(customization.map(|value| &value.responsive));
    let export = resolve_export(customization.map(|value| &value.export));

    points.sort_by(|left, right| {
        let left_page = page_order
            .get(left.page.as_str())
            .copied()
            .unwrap_or(i32::MAX / 2);
        let right_page = page_order
            .get(right.page.as_str())
            .copied()
            .unwrap_or(i32::MAX / 2);
        left_page
            .cmp(&right_page)
            .then_with(|| left.group.cmp(&right.group))
            .then_with(|| left.order.cmp(&right.order))
            .then_with(|| left.id.cmp(&right.id))
    });

    let widgets = points
        .into_iter()
        .map(|point| HmiWidgetSchema {
            id: point.id,
            path: point.path,
            label: point.label,
            data_type: point.data_type,
            access: point.access,
            writable: point.writable,
            widget: point.widget,
            source: point.source,
            page: point.page,
            group: point.group,
            order: point.order,
            zones: point.zones,
            on_color: point.on_color,
            off_color: point.off_color,
            section_title: point.section_title,
            widget_span: point.widget_span,
            alarm_deadband: point.alarm_deadband,
            inferred_interface: point.inferred_interface,
            detail_page: point.detail_page,
            unit: point.unit,
            min: point.min,
            max: point.max,
        })
        .collect::<Vec<_>>();

    HmiSchemaResult {
        version: HMI_SCHEMA_VERSION,
        schema_revision: 0,
        mode: if read_only { "read_only" } else { "read_write" },
        read_only,
        resource: resource_name.to_string(),
        generated_at_ms: now_unix_ms(),
        descriptor_error: None,
        theme,
        responsive,
        export,
        pages,
        widgets,
    }
}

pub fn build_values(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    read_only: bool,
    ids: Option<&[String]>,
) -> HmiValuesResult {
    let requested = ids.map(|entries| entries.iter().map(String::as_str).collect::<HashSet<_>>());
    let points = collect_points(resource_name, metadata, snapshot, read_only);
    let now_ms = now_unix_ms();
    let mut values = IndexMap::new();

    for point in points {
        if let Some(requested) = requested.as_ref() {
            if !requested.contains(point.id.as_str()) {
                continue;
            }
        }
        let (value, quality) = if let Some(snapshot) = snapshot {
            match resolve_point_value(&point.binding, snapshot) {
                Some(value) => (value_to_json(value), "good"),
                None => (serde_json::Value::Null, "bad"),
            }
        } else {
            (serde_json::Value::Null, "stale")
        };
        values.insert(
            point.id,
            HmiValueRecord {
                v: value,
                q: quality,
                ts_ms: now_ms,
            },
        );
    }

    HmiValuesResult {
        connected: snapshot.is_some(),
        timestamp_ms: now_ms,
        source_time_ns: snapshot.map(|state| state.now.as_nanos()),
        freshness_ms: snapshot.map(|_| 0),
        values,
    }
}

pub fn resolve_write_point(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    target: &str,
) -> Option<HmiWritePoint> {
    let target = target.trim();
    if target.is_empty() {
        return None;
    }
    collect_points(resource_name, metadata, snapshot, true)
        .into_iter()
        .find(|point| point.id == target || point.path == target)
        .map(|point| HmiWritePoint {
            id: point.id,
            path: point.path,
            binding: match point.binding {
                HmiBinding::ProgramVar { program, variable } => {
                    HmiWriteBinding::ProgramVar { program, variable }
                }
                HmiBinding::Global { name } => HmiWriteBinding::Global { name },
            },
        })
}

pub fn resolve_write_value_template(
    point: &HmiWritePoint,
    snapshot: &DebugSnapshot,
) -> Option<Value> {
    match &point.binding {
        HmiWriteBinding::ProgramVar { program, variable } => {
            let Value::Instance(instance_id) = snapshot.storage.get_global(program.as_str())?
            else {
                return None;
            };
            snapshot
                .storage
                .get_instance(*instance_id)
                .and_then(|instance| instance.variables.get(variable.as_str()))
                .cloned()
        }
        HmiWriteBinding::Global { name } => snapshot.storage.get_global(name.as_str()).cloned(),
    }
}

pub fn update_live_state(
    state: &mut HmiLiveState,
    schema: &HmiSchemaResult,
    values: &HmiValuesResult,
) {
    state.last_connected = values.connected;
    state.last_timestamp_ms = values.timestamp_ms;
    let widgets = schema
        .widgets
        .iter()
        .map(|widget| (widget.id.as_str(), widget))
        .collect::<HashMap<_, _>>();

    for (id, value) in &values.values {
        let Some(widget) = widgets.get(id.as_str()) else {
            continue;
        };
        if value.q != "good" {
            continue;
        }
        let Some(numeric) = numeric_value_from_json(&value.v) else {
            continue;
        };
        if is_trend_capable_widget_schema(widget) {
            let samples = state.trend_samples.entry(id.clone()).or_default();
            samples.push_back(HmiTrendSample {
                ts_ms: value.ts_ms,
                value: numeric,
            });
            while samples.len() > TREND_HISTORY_LIMIT {
                let _ = samples.pop_front();
            }
        }
        if widget.min.is_some() || widget.max.is_some() {
            update_alarm_state(state, widget, numeric, value.ts_ms);
        }
    }
}

pub fn build_trends(
    state: &HmiLiveState,
    schema: &HmiSchemaResult,
    ids: Option<&[String]>,
    duration_ms: u64,
    buckets: usize,
) -> HmiTrendResult {
    let now_ms = if state.last_timestamp_ms > 0 {
        state.last_timestamp_ms
    } else {
        now_unix_ms()
    };
    let duration_ms = duration_ms.max(5_000);
    let buckets = buckets.clamp(8, 480);
    let cutoff = now_ms.saturating_sub(u128::from(duration_ms));
    let allowed_ids = ids
        .filter(|entries| !entries.is_empty())
        .map(|entries| entries.iter().map(String::as_str).collect::<HashSet<_>>());

    let series = schema
        .widgets
        .iter()
        .filter(|widget| is_trend_capable_widget_schema(widget))
        .filter(|widget| {
            allowed_ids
                .as_ref()
                .is_none_or(|entries| entries.contains(widget.id.as_str()))
        })
        .filter_map(|widget| {
            let samples = state.trend_samples.get(widget.id.as_str())?;
            let scoped = samples
                .iter()
                .filter(|sample| sample.ts_ms >= cutoff)
                .cloned()
                .collect::<Vec<_>>();
            let points = downsample_trend_samples(&scoped, buckets);
            if points.is_empty() {
                return None;
            }
            Some(HmiTrendSeries {
                id: widget.id.clone(),
                label: widget.label.clone(),
                unit: widget.unit.clone(),
                points,
            })
        })
        .collect::<Vec<_>>();

    HmiTrendResult {
        connected: state.last_connected,
        timestamp_ms: now_ms,
        duration_ms,
        buckets,
        series,
    }
}

pub fn build_alarm_view(state: &HmiLiveState, history_limit: usize) -> HmiAlarmResult {
    let mut active = state
        .alarms
        .values()
        .filter(|alarm| alarm.active)
        .map(to_alarm_record)
        .collect::<Vec<_>>();
    active.sort_by(|left, right| {
        left.acknowledged
            .cmp(&right.acknowledged)
            .then_with(|| right.last_change_ms.cmp(&left.last_change_ms))
            .then_with(|| left.id.cmp(&right.id))
    });

    let history_limit = history_limit.clamp(1, ALARM_HISTORY_LIMIT);
    let history = state
        .history
        .iter()
        .rev()
        .take(history_limit)
        .cloned()
        .collect::<Vec<_>>();

    HmiAlarmResult {
        connected: state.last_connected,
        timestamp_ms: if state.last_timestamp_ms > 0 {
            state.last_timestamp_ms
        } else {
            now_unix_ms()
        },
        active,
        history,
    }
}

pub fn acknowledge_alarm(
    state: &mut HmiLiveState,
    alarm_id: &str,
    timestamp_ms: u128,
) -> Result<(), String> {
    let (id, widget_id, path, label, value) = {
        let alarm = state
            .alarms
            .get_mut(alarm_id)
            .ok_or_else(|| format!("unknown alarm '{alarm_id}'"))?;
        if !alarm.active {
            return Err("alarm is not active".to_string());
        }
        if alarm.acknowledged {
            return Ok(());
        }
        alarm.acknowledged = true;
        alarm.last_change_ms = timestamp_ms;
        (
            alarm.id.clone(),
            alarm.widget_id.clone(),
            alarm.path.clone(),
            alarm.label.clone(),
            alarm.value,
        )
    };
    push_alarm_history(
        state,
        HmiAlarmHistoryRecord {
            id,
            widget_id,
            path,
            label,
            event: "acknowledged",
            timestamp_ms,
            value,
        },
    );
    Ok(())
}

fn update_alarm_state(state: &mut HmiLiveState, widget: &HmiWidgetSchema, value: f64, ts_ms: u128) {
    let violation = alarm_violation(value, widget.min, widget.max);
    let clear_window = alarm_clear_window(value, widget.min, widget.max, widget.alarm_deadband);
    let mut raised = false;
    let mut cleared = false;
    let (id, widget_id, path, label) = {
        let alarm = state
            .alarms
            .entry(widget.id.clone())
            .or_insert_with(|| HmiAlarmState {
                id: widget.id.clone(),
                widget_id: widget.id.clone(),
                path: widget.path.clone(),
                label: widget.label.clone(),
                active: false,
                acknowledged: false,
                raised_at_ms: 0,
                last_change_ms: 0,
                value,
                min: widget.min,
                max: widget.max,
            });
        alarm.value = value;
        alarm.min = widget.min;
        alarm.max = widget.max;
        if violation {
            if !alarm.active {
                alarm.active = true;
                alarm.acknowledged = false;
                alarm.raised_at_ms = ts_ms;
                alarm.last_change_ms = ts_ms;
                raised = true;
            }
        } else if alarm.active && clear_window {
            alarm.active = false;
            alarm.acknowledged = false;
            alarm.last_change_ms = ts_ms;
            cleared = true;
        }
        (
            alarm.id.clone(),
            alarm.widget_id.clone(),
            alarm.path.clone(),
            alarm.label.clone(),
        )
    };
    if raised {
        push_alarm_history(
            state,
            HmiAlarmHistoryRecord {
                id,
                widget_id,
                path,
                label,
                event: "raised",
                timestamp_ms: ts_ms,
                value,
            },
        );
    } else if cleared {
        push_alarm_history(
            state,
            HmiAlarmHistoryRecord {
                id,
                widget_id,
                path,
                label,
                event: "cleared",
                timestamp_ms: ts_ms,
                value,
            },
        );
    }
}

fn alarm_violation(value: f64, min: Option<f64>, max: Option<f64>) -> bool {
    if let Some(min) = min {
        if value < min {
            return true;
        }
    }
    if let Some(max) = max {
        if value > max {
            return true;
        }
    }
    false
}

fn alarm_clear_window(
    value: f64,
    min: Option<f64>,
    max: Option<f64>,
    deadband: Option<f64>,
) -> bool {
    let deadband = deadband.unwrap_or(0.0).max(0.0);
    if let Some(min) = min {
        if value < min + deadband {
            return false;
        }
    }
    if let Some(max) = max {
        if value > max - deadband {
            return false;
        }
    }
    true
}

fn push_alarm_history(state: &mut HmiLiveState, event: HmiAlarmHistoryRecord) {
    state.history.push_back(event);
    while state.history.len() > ALARM_HISTORY_LIMIT {
        let _ = state.history.pop_front();
    }
}

fn downsample_trend_samples(samples: &[HmiTrendSample], buckets: usize) -> Vec<HmiTrendPoint> {
    if samples.is_empty() {
        return Vec::new();
    }
    if samples.len() <= buckets {
        return samples
            .iter()
            .map(|sample| HmiTrendPoint {
                ts_ms: sample.ts_ms,
                value: sample.value,
                min: sample.value,
                max: sample.value,
                samples: 1,
            })
            .collect();
    }

    let chunk_size = samples.len().div_ceil(buckets);
    samples
        .chunks(chunk_size.max(1))
        .map(|chunk| {
            let mut min = f64::INFINITY;
            let mut max = f64::NEG_INFINITY;
            let mut sum = 0.0;
            for sample in chunk {
                min = min.min(sample.value);
                max = max.max(sample.value);
                sum += sample.value;
            }
            HmiTrendPoint {
                ts_ms: chunk.last().map(|sample| sample.ts_ms).unwrap_or_default(),
                value: sum / chunk.len() as f64,
                min,
                max,
                samples: chunk.len(),
            }
        })
        .collect()
}

fn numeric_value_from_json(value: &serde_json::Value) -> Option<f64> {
    match value {
        serde_json::Value::Number(number) => number.as_f64(),
        serde_json::Value::Bool(boolean) => Some(if *boolean { 1.0 } else { 0.0 }),
        _ => None,
    }
}

fn to_alarm_record(state: &HmiAlarmState) -> HmiAlarmRecord {
    HmiAlarmRecord {
        id: state.id.clone(),
        widget_id: state.widget_id.clone(),
        path: state.path.clone(),
        label: state.label.clone(),
        state: if state.acknowledged {
            "acknowledged"
        } else {
            "raised"
        },
        acknowledged: state.acknowledged,
        raised_at_ms: state.raised_at_ms,
        last_change_ms: state.last_change_ms,
        value: state.value,
        min: state.min,
        max: state.max,
    }
}

fn collect_points(
    resource_name: &str,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    read_only: bool,
) -> Vec<HmiPoint> {
    let resource = stable_component(resource_name);
    let writable = !read_only;
    let mut points = Vec::new();

    for (program_name, program) in metadata.programs() {
        for variable in &program.vars {
            let ty = metadata.registry().get(variable.type_id);
            let data_type = metadata
                .registry()
                .type_name(variable.type_id)
                .map(|name| name.to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let widget = ty
                .map(|ty| widget_for_type(ty, writable).to_string())
                .unwrap_or_else(|| "value".to_string());
            let path = format!("{program_name}.{}", variable.name);
            points.push(HmiPoint {
                id: format!(
                    "resource/{resource}/program/{}/field/{}",
                    stable_component(program_name.as_str()),
                    stable_component(variable.name.as_str())
                ),
                path,
                label: variable.name.to_string(),
                data_type,
                access: if writable { "read_write" } else { "read" },
                writable,
                widget,
                source: format!("program:{program_name}"),
                page: DEFAULT_PAGE_ID.to_string(),
                group: DEFAULT_GROUP_NAME.to_string(),
                order: 0,
                zones: Vec::new(),
                on_color: None,
                off_color: None,
                section_title: None,
                widget_span: None,
                alarm_deadband: None,
                inferred_interface: false,
                detail_page: None,
                unit: None,
                min: None,
                max: None,
                binding: HmiBinding::ProgramVar {
                    program: program_name.clone(),
                    variable: variable.name.clone(),
                },
            });
        }
    }

    if let Some(snapshot) = snapshot {
        let programs = metadata
            .programs()
            .keys()
            .map(|name| name.to_ascii_uppercase())
            .collect::<HashSet<_>>();
        for (name, value) in snapshot.storage.globals() {
            if programs.contains(&name.to_ascii_uppercase()) {
                continue;
            }
            if matches!(value, Value::Instance(_)) {
                continue;
            }
            let data_type = value_type_name(value).unwrap_or_else(|| "UNKNOWN".to_string());
            points.push(HmiPoint {
                id: format!(
                    "resource/{resource}/global/{}",
                    stable_component(name.as_str())
                ),
                path: format!("global.{name}"),
                label: name.to_string(),
                data_type,
                access: if writable { "read_write" } else { "read" },
                writable,
                widget: widget_for_value(value, writable).to_string(),
                source: "global".to_string(),
                page: DEFAULT_PAGE_ID.to_string(),
                group: DEFAULT_GROUP_NAME.to_string(),
                order: 0,
                zones: Vec::new(),
                on_color: None,
                off_color: None,
                section_title: None,
                widget_span: None,
                alarm_deadband: None,
                inferred_interface: false,
                detail_page: None,
                unit: None,
                min: None,
                max: None,
                binding: HmiBinding::Global { name: name.clone() },
            });
        }
    }

    points
}

fn resolve_point_value<'a>(binding: &HmiBinding, snapshot: &'a DebugSnapshot) -> Option<&'a Value> {
    match binding {
        HmiBinding::ProgramVar { program, variable } => {
            let Value::Instance(instance_id) = snapshot.storage.get_global(program.as_str())?
            else {
                return None;
            };
            snapshot
                .storage
                .get_instance(*instance_id)
                .and_then(|instance| instance.variables.get(variable.as_str()))
        }
        HmiBinding::Global { name } => snapshot.storage.get_global(name.as_str()),
    }
}

fn widget_for_type(ty: &Type, writable: bool) -> &'static str {
    match ty {
        Type::Bool => {
            if writable {
                "toggle"
            } else {
                "indicator"
            }
        }
        Type::Enum { .. } => {
            if writable {
                "selector"
            } else {
                "readout"
            }
        }
        Type::Array { .. } => "table",
        Type::Struct { .. }
        | Type::Union { .. }
        | Type::FunctionBlock { .. }
        | Type::Class { .. }
        | Type::Interface { .. } => "tree",
        ty if ty.is_string() || ty.is_char() => "text",
        ty if ty.is_numeric() || ty.is_bit_string() || ty.is_time() => {
            if writable {
                "slider"
            } else {
                "value"
            }
        }
        _ => "value",
    }
}

fn widget_for_value(value: &Value, writable: bool) -> &'static str {
    match value {
        Value::Bool(_) => {
            if writable {
                "toggle"
            } else {
                "indicator"
            }
        }
        Value::Enum(_) => {
            if writable {
                "selector"
            } else {
                "readout"
            }
        }
        Value::Array(_) => "table",
        Value::Struct(_) | Value::Instance(_) => "tree",
        Value::String(_) | Value::WString(_) | Value::Char(_) | Value::WChar(_) => "text",
        Value::SInt(_)
        | Value::Int(_)
        | Value::DInt(_)
        | Value::LInt(_)
        | Value::USInt(_)
        | Value::UInt(_)
        | Value::UDInt(_)
        | Value::ULInt(_)
        | Value::Real(_)
        | Value::LReal(_)
        | Value::Byte(_)
        | Value::Word(_)
        | Value::DWord(_)
        | Value::LWord(_)
        | Value::Time(_)
        | Value::LTime(_)
        | Value::Date(_)
        | Value::LDate(_)
        | Value::Tod(_)
        | Value::LTod(_)
        | Value::Dt(_)
        | Value::Ldt(_) => {
            if writable {
                "slider"
            } else {
                "value"
            }
        }
        Value::Reference(_) | Value::Null => "value",
    }
}

fn value_to_json(value: &Value) -> serde_json::Value {
    match value {
        Value::Bool(value) => serde_json::Value::Bool(*value),
        Value::SInt(value) => serde_json::json!(*value),
        Value::Int(value) => serde_json::json!(*value),
        Value::DInt(value) => serde_json::json!(*value),
        Value::LInt(value) => serde_json::json!(*value),
        Value::USInt(value) => serde_json::json!(*value),
        Value::UInt(value) => serde_json::json!(*value),
        Value::UDInt(value) => serde_json::json!(*value),
        Value::ULInt(value) => serde_json::json!(*value),
        Value::Real(value) => serde_json::json!(*value),
        Value::LReal(value) => serde_json::json!(*value),
        Value::Byte(value) => serde_json::json!(*value),
        Value::Word(value) => serde_json::json!(*value),
        Value::DWord(value) => serde_json::json!(*value),
        Value::LWord(value) => serde_json::json!(*value),
        Value::Time(value) | Value::LTime(value) => serde_json::json!(value.as_nanos()),
        Value::Date(value) => serde_json::json!(value.ticks()),
        Value::LDate(value) => serde_json::json!(value.nanos()),
        Value::Tod(value) => serde_json::json!(value.ticks()),
        Value::LTod(value) => serde_json::json!(value.nanos()),
        Value::Dt(value) => serde_json::json!(value.ticks()),
        Value::Ldt(value) => serde_json::json!(value.nanos()),
        Value::String(value) => serde_json::json!(value.as_str()),
        Value::WString(value) => serde_json::json!(value),
        Value::Char(value) => {
            let text = char::from_u32((*value).into()).unwrap_or('?').to_string();
            serde_json::json!(text)
        }
        Value::WChar(value) => {
            let text = char::from_u32((*value).into()).unwrap_or('?').to_string();
            serde_json::json!(text)
        }
        Value::Array(value) => {
            serde_json::Value::Array(value.elements.iter().map(value_to_json).collect())
        }
        Value::Struct(value) => {
            let mut object = serde_json::Map::new();
            for (name, field) in &value.fields {
                object.insert(name.to_string(), value_to_json(field));
            }
            serde_json::Value::Object(object)
        }
        Value::Enum(value) => serde_json::json!({
            "type": value.type_name.as_str(),
            "variant": value.variant_name.as_str(),
            "value": value.numeric_value,
        }),
        Value::Reference(_) => serde_json::Value::Null,
        Value::Instance(value) => serde_json::json!({ "instance": value.0 }),
        Value::Null => serde_json::Value::Null,
    }
}

fn stable_component(value: &str) -> String {
    let text = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    if text.is_empty() {
        "unnamed".to_string()
    } else {
        text
    }
}

fn now_unix_ms() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn resolve_pages(
    points: &mut [HmiPoint],
    customization: Option<&HmiCustomization>,
) -> (Vec<HmiPageSchema>, HashMap<String, i32>) {
    let trend_capable = points.iter().any(is_trend_capable_widget);
    let alarm_capable = points
        .iter()
        .any(|point| point.min.is_some() || point.max.is_some());
    let mut pages = customization
        .map(|config| {
            config
                .pages
                .iter()
                .map(|page| {
                    (
                        page.id.clone(),
                        HmiPageSchema {
                            id: page.id.clone(),
                            title: page.title.clone(),
                            order: page.order,
                            kind: normalize_page_kind(Some(page.kind.as_str())).to_string(),
                            icon: page.icon.clone(),
                            duration_ms: page.duration_ms,
                            svg: page.svg.clone(),
                            hidden: page.hidden,
                            signals: page.signals.clone(),
                            sections: Vec::new(),
                            bindings: page.bindings.clone(),
                        },
                    )
                })
                .collect::<BTreeMap<_, _>>()
        })
        .unwrap_or_default();

    if pages.is_empty() {
        pages.insert(
            DEFAULT_PAGE_ID.to_string(),
            HmiPageSchema {
                id: DEFAULT_PAGE_ID.to_string(),
                title: "Overview".to_string(),
                order: 0,
                kind: "dashboard".to_string(),
                icon: None,
                duration_ms: None,
                svg: None,
                hidden: false,
                signals: Vec::new(),
                sections: Vec::new(),
                bindings: Vec::new(),
            },
        );
    }
    if trend_capable && !pages.contains_key(DEFAULT_TREND_PAGE_ID) {
        pages.insert(
            DEFAULT_TREND_PAGE_ID.to_string(),
            HmiPageSchema {
                id: DEFAULT_TREND_PAGE_ID.to_string(),
                title: "Trends".to_string(),
                order: 50,
                kind: "trend".to_string(),
                icon: None,
                duration_ms: Some(10 * 60 * 1_000),
                svg: None,
                hidden: false,
                signals: Vec::new(),
                sections: Vec::new(),
                bindings: Vec::new(),
            },
        );
    }
    if alarm_capable && !pages.contains_key(DEFAULT_ALARM_PAGE_ID) {
        pages.insert(
            DEFAULT_ALARM_PAGE_ID.to_string(),
            HmiPageSchema {
                id: DEFAULT_ALARM_PAGE_ID.to_string(),
                title: "Alarms".to_string(),
                order: 60,
                kind: "alarm".to_string(),
                icon: None,
                duration_ms: None,
                svg: None,
                hidden: false,
                signals: Vec::new(),
                sections: Vec::new(),
                bindings: Vec::new(),
            },
        );
    }

    for point in points.iter_mut() {
        normalize_point(point);
        if !pages.contains_key(point.page.as_str()) {
            pages.insert(
                point.page.clone(),
                HmiPageSchema {
                    id: point.page.clone(),
                    title: title_case(&point.page),
                    order: 1000,
                    kind: "dashboard".to_string(),
                    icon: None,
                    duration_ms: None,
                    svg: None,
                    hidden: false,
                    signals: Vec::new(),
                    sections: Vec::new(),
                    bindings: Vec::new(),
                },
            );
        }
    }

    if let Some(customization) = customization {
        let id_by_path = points
            .iter()
            .map(|point| (point.path.as_str(), point.id.as_str()))
            .collect::<HashMap<_, _>>();
        let dir_desc = customization.dir_descriptor.as_ref();
        for page in &customization.pages {
            let Some(page_schema) = pages.get_mut(page.id.as_str()) else {
                continue;
            };
            if page.sections.is_empty() {
                continue;
            }
            let dir_page = dir_desc.and_then(|d| d.pages.iter().find(|p| p.id == page.id));
            page_schema.sections = page
                .sections
                .iter()
                .enumerate()
                .map(|(section_idx, section)| {
                    let widget_ids: Vec<String> = section
                        .widget_paths
                        .iter()
                        .filter_map(|path| {
                            id_by_path.get(path.as_str()).map(|id| (*id).to_string())
                        })
                        .collect();
                    let module_meta = if section.tier.as_deref() == Some("module") {
                        dir_page
                            .and_then(|dp| dp.sections.get(section_idx))
                            .map(|ds| {
                                ds.widgets
                                    .iter()
                                    .filter_map(|w| {
                                        let widget_id =
                                            id_by_path.get(w.bind.as_str())?.to_string();
                                        Some(HmiModuleMeta {
                                            id: widget_id,
                                            label: w.label.clone().unwrap_or_default(),
                                            detail_page: w.detail_page.clone(),
                                            unit: w.unit.clone(),
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    } else {
                        Vec::new()
                    };
                    HmiSectionSchema {
                        title: section.title.clone(),
                        span: section.span.clamp(1, 12),
                        tier: section.tier.clone(),
                        widget_ids,
                        module_meta,
                    }
                })
                .collect();
        }
    }

    let mut ordered = pages.into_values().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
    });
    let page_order = ordered
        .iter()
        .map(|page| (page.id.clone(), page.order))
        .collect::<HashMap<_, _>>();
    (ordered, page_order)
}

fn normalize_page_kind(value: Option<&str>) -> &'static str {
    match value
        .map(|raw| raw.trim().to_ascii_lowercase())
        .as_deref()
        .unwrap_or("dashboard")
    {
        "dashboard" => "dashboard",
        "trend" => "trend",
        "alarm" => "alarm",
        "table" => "table",
        "process" => "process",
        _ => "dashboard",
    }
}

fn is_safe_process_selector(selector: &str) -> bool {
    let mut chars = selector.chars();
    if chars.next() != Some('#') {
        return false;
    }
    if selector.len() > 128 {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | ':' | '.'))
}

fn normalize_process_attribute(attribute: &str) -> Option<String> {
    let normalized = attribute.trim().to_ascii_lowercase();
    if matches!(
        normalized.as_str(),
        "text"
            | "fill"
            | "stroke"
            | "opacity"
            | "x"
            | "y"
            | "width"
            | "height"
            | "class"
            | "transform"
            | "data-value"
    ) {
        Some(normalized)
    } else {
        None
    }
}

fn normalize_process_scale(scale: HmiProcessScaleToml) -> Option<HmiProcessScaleSchema> {
    if !scale.min.is_finite()
        || !scale.max.is_finite()
        || !scale.output_min.is_finite()
        || !scale.output_max.is_finite()
    {
        return None;
    }
    if scale.max <= scale.min {
        return None;
    }
    if (scale.output_max - scale.output_min).abs() < f64::EPSILON {
        return None;
    }
    Some(HmiProcessScaleSchema {
        min: scale.min,
        max: scale.max,
        output_min: scale.output_min,
        output_max: scale.output_max,
    })
}

fn resolve_responsive(config: Option<&HmiResponsiveConfig>) -> HmiResponsiveSchema {
    let mode = config
        .and_then(|value| value.mode.as_deref())
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| matches!(value.as_str(), "auto" | "mobile" | "tablet" | "kiosk"))
        .unwrap_or_else(|| DEFAULT_RESPONSIVE_MODE.to_string());
    HmiResponsiveSchema {
        mode,
        mobile_max_px: 680,
        tablet_max_px: 1024,
    }
}

fn resolve_export(config: Option<&HmiExportConfig>) -> HmiExportSchema {
    HmiExportSchema {
        enabled: config.and_then(|value| value.enabled).unwrap_or(true),
        route: "/hmi/export.json".to_string(),
    }
}

fn resolve_theme(theme: Option<&HmiThemeConfig>) -> HmiThemeSchema {
    let requested_style = theme
        .and_then(|config| config.style.as_ref())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_else(|| "classic".to_string());
    let palette = theme_palette(requested_style.as_str())
        .unwrap_or(theme_palette("classic").expect("classic theme"));
    let accent = theme
        .and_then(|config| config.accent.as_ref())
        .filter(|value| is_hex_color(value))
        .cloned()
        .unwrap_or_else(|| palette.accent.to_string());
    HmiThemeSchema {
        style: palette.style.to_string(),
        accent,
        background: palette.background.to_string(),
        surface: palette.surface.to_string(),
        text: palette.text.to_string(),
    }
}

fn theme_palette(style: &str) -> Option<ThemePalette> {
    match style {
        "classic" => Some(ThemePalette {
            style: "classic",
            accent: "#0f766e",
            background: "#f3f5f8",
            surface: "#ffffff",
            text: "#142133",
        }),
        "industrial" => Some(ThemePalette {
            style: "industrial",
            accent: "#c2410c",
            background: "#f5f3ef",
            surface: "#ffffff",
            text: "#221a14",
        }),
        "mint" => Some(ThemePalette {
            style: "mint",
            accent: "#0d9488",
            background: "#ecfdf5",
            surface: "#f8fffc",
            text: "#0b3b35",
        }),
        "control-room" => Some(ThemePalette {
            style: "control-room",
            accent: "#14b8a6",
            background: "#0f1115",
            surface: "#171a21",
            text: "#f2f2f2",
        }),
        _ => None,
    }
}

fn apply_widget_override(point: &mut HmiPoint, override_spec: &HmiWidgetOverride) {
    if let Some(label) = override_spec.label.as_ref() {
        point.label = label.clone();
    }
    if let Some(unit) = override_spec.unit.as_ref() {
        point.unit = Some(unit.clone());
    }
    if let Some(min) = override_spec.min {
        point.min = Some(min);
    }
    if let Some(max) = override_spec.max {
        point.max = Some(max);
    }
    if let Some(widget) = override_spec.widget.as_ref() {
        point.widget = widget.clone();
    }
    if let Some(page) = override_spec.page.as_ref() {
        point.page = page.clone();
    }
    if let Some(group) = override_spec.group.as_ref() {
        point.group = group.clone();
    }
    if let Some(order) = override_spec.order {
        point.order = order;
    }
    if !override_spec.zones.is_empty() {
        point.zones = override_spec.zones.clone();
    }
    if let Some(on_color) = override_spec.on_color.as_ref() {
        point.on_color = Some(on_color.clone());
    }
    if let Some(off_color) = override_spec.off_color.as_ref() {
        point.off_color = Some(off_color.clone());
    }
    if let Some(section_title) = override_spec.section_title.as_ref() {
        point.section_title = Some(section_title.clone());
    }
    if let Some(widget_span) = override_spec.widget_span {
        point.widget_span = Some(widget_span);
    }
    if let Some(alarm_deadband) = override_spec.alarm_deadband {
        point.alarm_deadband = Some(alarm_deadband.max(0.0));
    }
    if let Some(inferred_interface) = override_spec.inferred_interface {
        point.inferred_interface = inferred_interface;
    }
    if let Some(detail_page) = override_spec.detail_page.as_ref() {
        point.detail_page = Some(detail_page.clone());
    }
}

fn normalize_point(point: &mut HmiPoint) {
    if point.page.trim().is_empty() {
        point.page = DEFAULT_PAGE_ID.to_string();
    }
    if point.group.trim().is_empty() {
        point.group = DEFAULT_GROUP_NAME.to_string();
    }
    if point.widget.trim().is_empty() {
        point.widget = "value".to_string();
    }
    point.zones.sort_by(|left, right| {
        left.from
            .total_cmp(&right.from)
            .then_with(|| left.to.total_cmp(&right.to))
    });
    if let Some(section_title) = point.section_title.as_ref() {
        if section_title.trim().is_empty() {
            point.section_title = None;
        }
    }
    if let Some(span) = point.widget_span {
        point.widget_span = Some(span.clamp(1, 12));
    }
    if let Some(deadband) = point.alarm_deadband {
        point.alarm_deadband = Some(deadband.max(0.0));
    }
}

fn is_trend_capable_widget(point: &HmiPoint) -> bool {
    is_numeric_data_type(point.data_type.as_str())
        || matches!(point.widget.as_str(), "value" | "slider")
}

fn is_trend_capable_widget_schema(widget: &HmiWidgetSchema) -> bool {
    is_numeric_data_type(widget.data_type.as_str())
        || matches!(widget.widget.as_str(), "value" | "slider")
}

fn is_supported_widget_kind(kind: &str) -> bool {
    matches!(
        kind,
        "gauge"
            | "sparkline"
            | "bar"
            | "tank"
            | "value"
            | "slider"
            | "indicator"
            | "toggle"
            | "selector"
            | "readout"
            | "text"
            | "table"
            | "tree"
    )
}

fn widget_kind_matches_point(kind: &str, point: &HmiPoint) -> bool {
    let point_kind = point.widget.as_str();
    match point_kind {
        "indicator" | "toggle" => matches!(kind, "indicator" | "toggle"),
        "selector" | "readout" => matches!(kind, "selector" | "readout"),
        "table" => kind == "table",
        "tree" => kind == "tree",
        "text" => kind == "text",
        "value" | "slider" => matches!(
            kind,
            "gauge" | "sparkline" | "bar" | "tank" | "value" | "slider"
        ),
        _ => true,
    }
}

fn is_numeric_data_type(data_type: &str) -> bool {
    matches!(
        data_type.to_ascii_uppercase().as_str(),
        "SINT"
            | "INT"
            | "DINT"
            | "LINT"
            | "USINT"
            | "UINT"
            | "UDINT"
            | "ULINT"
            | "BYTE"
            | "WORD"
            | "DWORD"
            | "LWORD"
            | "REAL"
            | "LREAL"
            | "TIME"
            | "LTIME"
            | "DATE"
            | "LDATE"
            | "TOD"
            | "LTOD"
            | "DT"
            | "LDT"
    )
}

fn load_hmi_toml(root: &Path) -> anyhow::Result<HmiTomlFile> {
    let path = root.join("hmi.toml");
    if !path.is_file() {
        return Ok(HmiTomlFile::default());
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(toml::from_str::<HmiTomlFile>(&text)?)
}

pub fn load_hmi_dir(root: &Path) -> Option<HmiDirDescriptor> {
    load_hmi_dir_impl(root).ok()
}

pub fn write_hmi_dir_descriptor(
    root: &Path,
    descriptor: &HmiDirDescriptor,
) -> anyhow::Result<Vec<String>> {
    let dir = root.join("hmi");
    std::fs::create_dir_all(&dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create hmi descriptor directory '{}': {err}",
            dir.display()
        )
    })?;

    let mut written = Vec::new();
    let mut normalized_pages = descriptor
        .pages
        .iter()
        .filter_map(normalize_descriptor_page)
        .collect::<Vec<_>>();
    normalized_pages.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut normalized_config = descriptor.config.clone();
    if normalized_config.version.is_none() {
        normalized_config.version = Some(HMI_DESCRIPTOR_VERSION);
    }
    let config_text = render_hmi_dir_config_toml(&normalized_config);
    write_scaffold_file(&dir.join("_config.toml"), config_text.as_str())?;
    written.push("_config.toml".to_string());

    for page in &normalized_pages {
        let page_text = render_hmi_dir_page_toml(page);
        let file_name = format!("{}.toml", page.id);
        write_scaffold_file(&dir.join(&file_name), page_text.as_str())?;
        written.push(file_name);
    }

    let keep = written
        .iter()
        .map(|file| file.to_ascii_lowercase())
        .collect::<HashSet<_>>();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|value| value.to_str()) != Some("toml") {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if keep.contains(&name.to_ascii_lowercase()) {
            continue;
        }
        std::fs::remove_file(&path).map_err(|err| {
            anyhow::anyhow!(
                "failed to remove stale hmi descriptor file '{}': {err}",
                path.display()
            )
        })?;
    }

    Ok(written)
}

fn normalize_descriptor_page(page: &HmiDirPage) -> Option<HmiDirPage> {
    let id = page.id.trim();
    if id.is_empty() {
        return None;
    }
    let title = page
        .title
        .trim()
        .strip_prefix('\u{feff}')
        .unwrap_or(page.title.trim());
    let title = if title.is_empty() {
        title_case(id)
    } else {
        title.to_string()
    };

    let mut sections = Vec::new();
    for (section_idx, section) in page.sections.iter().enumerate() {
        let section_title = section
            .title
            .trim()
            .strip_prefix('\u{feff}')
            .unwrap_or(section.title.trim());
        let section_title = if section_title.is_empty() {
            format!("Section {}", section_idx + 1)
        } else {
            section_title.to_string()
        };
        let mut widgets = Vec::new();
        for widget in &section.widgets {
            let bind = widget.bind.trim();
            if bind.is_empty() {
                continue;
            }
            widgets.push(HmiDirWidget {
                widget_type: widget
                    .widget_type
                    .as_ref()
                    .map(|kind| kind.trim().to_ascii_lowercase())
                    .filter(|kind| !kind.is_empty()),
                bind: bind.to_string(),
                label: widget
                    .label
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                unit: widget
                    .unit
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                min: widget.min,
                max: widget.max,
                span: widget.span.map(|span| span.clamp(1, 12)),
                on_color: widget
                    .on_color
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                off_color: widget
                    .off_color
                    .as_ref()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty()),
                inferred_interface: widget.inferred_interface,
                detail_page: widget.detail_page.clone(),
                zones: widget.zones.clone(),
            });
        }
        if widgets.is_empty() {
            continue;
        }
        sections.push(HmiDirSection {
            title: section_title,
            span: section.span.clamp(1, 12),
            tier: section.tier.clone(),
            widgets,
        });
    }

    let mut bindings = Vec::new();
    for binding in &page.bindings {
        let selector = binding.selector.trim();
        let Some(attribute) = normalize_process_attribute(binding.attribute.as_str()) else {
            continue;
        };
        let source = binding.source.trim();
        if !is_safe_process_selector(selector) || source.is_empty() {
            continue;
        }
        bindings.push(HmiDirProcessBinding {
            selector: selector.to_string(),
            attribute,
            source: source.to_string(),
            format: binding
                .format
                .as_ref()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            map: binding
                .map
                .iter()
                .filter_map(|(key, value)| {
                    let key = key.trim();
                    let value = value.trim();
                    if key.is_empty() || value.is_empty() {
                        return None;
                    }
                    Some((key.to_string(), value.to_string()))
                })
                .collect(),
            scale: binding.scale.clone(),
        });
    }
    bindings.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.selector.cmp(&right.selector))
            .then_with(|| left.attribute.cmp(&right.attribute))
    });

    Some(HmiDirPage {
        id: id.to_string(),
        title,
        icon: page
            .icon
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        order: page.order,
        kind: normalize_page_kind(Some(page.kind.as_str())).to_string(),
        duration_ms: page.duration_ms,
        svg: page
            .svg
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        hidden: page.hidden,
        signals: page
            .signals
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect(),
        sections,
        bindings,
    })
}

fn render_hmi_dir_config_toml(config: &HmiDirConfig) -> String {
    let mut out = String::new();
    if let Some(version) = config.version {
        let _ = writeln!(out, "version = {}", version.max(1));
        let _ = writeln!(out);
    }
    if config.theme.style.is_some() || config.theme.accent.is_some() {
        let _ = writeln!(out, "[theme]");
        if let Some(style) = config.theme.style.as_ref() {
            let _ = writeln!(out, "style = \"{}\"", escape_toml_string(style.trim()));
        }
        if let Some(accent) = config.theme.accent.as_ref() {
            let _ = writeln!(out, "accent = \"{}\"", escape_toml_string(accent.trim()));
        }
        let _ = writeln!(out);
    }
    if config.layout.navigation.is_some()
        || config.layout.header.is_some()
        || config.layout.header_title.is_some()
    {
        let _ = writeln!(out, "[layout]");
        if let Some(navigation) = config.layout.navigation.as_ref() {
            let _ = writeln!(
                out,
                "navigation = \"{}\"",
                escape_toml_string(navigation.trim())
            );
        }
        if let Some(header) = config.layout.header {
            let _ = writeln!(out, "header = {header}");
        }
        if let Some(header_title) = config.layout.header_title.as_ref() {
            let _ = writeln!(
                out,
                "header_title = \"{}\"",
                escape_toml_string(header_title.trim())
            );
        }
        let _ = writeln!(out);
    }
    if config.write.enabled.is_some() || !config.write.allow.is_empty() {
        let _ = writeln!(out, "[write]");
        if let Some(enabled) = config.write.enabled {
            let _ = writeln!(out, "enabled = {enabled}");
        }
        let allow = config
            .write
            .allow
            .iter()
            .map(|entry| format!("\"{}\"", escape_toml_string(entry.trim())))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "allow = [{}]", allow);
        let _ = writeln!(out);
    }
    for alarm in &config.alarms {
        let bind = alarm.bind.trim();
        if bind.is_empty() {
            continue;
        }
        let _ = writeln!(out, "[[alarm]]");
        let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(bind));
        if let Some(high) = alarm.high {
            let _ = writeln!(out, "high = {}", format_toml_number(high));
        }
        if let Some(low) = alarm.low {
            let _ = writeln!(out, "low = {}", format_toml_number(low));
        }
        if let Some(deadband) = alarm.deadband {
            let _ = writeln!(out, "deadband = {}", format_toml_number(deadband.max(0.0)));
        }
        if let Some(inferred) = alarm.inferred {
            let _ = writeln!(out, "inferred = {inferred}");
        }
        if let Some(label) = alarm.label.as_ref() {
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(label.trim()));
        }
        let _ = writeln!(out);
    }
    out.trim().to_string()
}

fn render_hmi_dir_page_toml(page: &HmiDirPage) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "title = \"{}\"",
        escape_toml_string(page.title.as_str())
    );
    if let Some(icon) = page.icon.as_ref() {
        let _ = writeln!(out, "icon = \"{}\"", escape_toml_string(icon.as_str()));
    }
    let _ = writeln!(out, "order = {}", page.order);
    let _ = writeln!(out, "kind = \"{}\"", escape_toml_string(page.kind.as_str()));
    if let Some(duration_ms) = page.duration_ms {
        let _ = writeln!(out, "duration_s = {}", duration_ms / 1_000);
    }
    if let Some(svg) = page.svg.as_ref() {
        let _ = writeln!(out, "svg = \"{}\"", escape_toml_string(svg.as_str()));
    }
    if !page.signals.is_empty() {
        let values = page
            .signals
            .iter()
            .map(|entry| format!("\"{}\"", escape_toml_string(entry.as_str())))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "signals = [{}]", values);
    }

    for section in &page.sections {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(
            out,
            "title = \"{}\"",
            escape_toml_string(section.title.as_str())
        );
        let _ = writeln!(out, "span = {}", section.span.clamp(1, 12));
        if let Some(tier) = section.tier.as_ref() {
            let _ = writeln!(out, "tier = \"{}\"", escape_toml_string(tier.as_str()));
        }
        for widget in &section.widgets {
            let bind = widget.bind.trim();
            if bind.is_empty() {
                continue;
            }
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            if let Some(kind) = widget.widget_type.as_ref() {
                let _ = writeln!(out, "type = \"{}\"", escape_toml_string(kind.as_str()));
            }
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(bind));
            if let Some(label) = widget.label.as_ref() {
                let _ = writeln!(out, "label = \"{}\"", escape_toml_string(label.as_str()));
            }
            if let Some(unit) = widget.unit.as_ref() {
                let _ = writeln!(out, "unit = \"{}\"", escape_toml_string(unit.as_str()));
            }
            if let Some(min) = widget.min {
                let _ = writeln!(out, "min = {}", format_toml_number(min));
            }
            if let Some(max) = widget.max {
                let _ = writeln!(out, "max = {}", format_toml_number(max));
            }
            if let Some(span) = widget.span {
                let _ = writeln!(out, "span = {}", span.clamp(1, 12));
            }
            if let Some(on_color) = widget.on_color.as_ref() {
                let _ = writeln!(
                    out,
                    "on_color = \"{}\"",
                    escape_toml_string(on_color.as_str())
                );
            }
            if let Some(off_color) = widget.off_color.as_ref() {
                let _ = writeln!(
                    out,
                    "off_color = \"{}\"",
                    escape_toml_string(off_color.as_str())
                );
            }
            if let Some(inferred_interface) = widget.inferred_interface {
                let _ = writeln!(out, "inferred_interface = {inferred_interface}");
            }
            if let Some(detail_page) = widget.detail_page.as_ref() {
                let _ = writeln!(
                    out,
                    "detail_page = \"{}\"",
                    escape_toml_string(detail_page.as_str())
                );
            }
            for zone in &widget.zones {
                let _ = writeln!(out);
                let _ = writeln!(out, "[[section.widget.zones]]");
                let _ = writeln!(out, "from = {}", format_toml_number(zone.from));
                let _ = writeln!(out, "to = {}", format_toml_number(zone.to));
                let _ = writeln!(
                    out,
                    "color = \"{}\"",
                    escape_toml_string(zone.color.as_str())
                );
            }
        }
    }

    for binding in &page.bindings {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind]]");
        let _ = writeln!(
            out,
            "selector = \"{}\"",
            escape_toml_string(binding.selector.as_str())
        );
        let _ = writeln!(
            out,
            "attribute = \"{}\"",
            escape_toml_string(binding.attribute.as_str())
        );
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(binding.source.as_str())
        );
        if let Some(format) = binding.format.as_ref() {
            let _ = writeln!(out, "format = \"{}\"", escape_toml_string(format.as_str()));
        }
        if !binding.map.is_empty() {
            let values = binding
                .map
                .iter()
                .map(|(key, value)| {
                    format!(
                        "\"{}\" = \"{}\"",
                        escape_toml_string(key.as_str()),
                        escape_toml_string(value.as_str())
                    )
                })
                .collect::<Vec<_>>()
                .join(", ");
            let _ = writeln!(out, "map = {{ {values} }}");
        }
        if let Some(scale) = binding.scale.as_ref() {
            let _ = writeln!(
                out,
                "scale = {{ min = {}, max = {}, output_min = {}, output_max = {} }}",
                format_toml_number(scale.min),
                format_toml_number(scale.max),
                format_toml_number(scale.output_min),
                format_toml_number(scale.output_max)
            );
        }
    }

    out.trim().to_string()
}

fn load_hmi_dir_impl(root: &Path) -> anyhow::Result<HmiDirDescriptor> {
    let dir = root.join("hmi");
    if !dir.is_dir() {
        anyhow::bail!("hmi directory not found");
    }

    let config = load_hmi_dir_config(&dir)?;
    let mut page_paths = BTreeSet::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) != Some("toml") {
            continue;
        }
        if path.file_name().and_then(|name| name.to_str()) == Some("_config.toml") {
            continue;
        }
        page_paths.insert(path);
    }

    let mut pages = Vec::with_capacity(page_paths.len());
    for path in page_paths {
        let Some(stem) = path.file_stem().and_then(|name| name.to_str()) else {
            continue;
        };
        let id = stem.trim();
        if id.is_empty() {
            continue;
        }
        let text = std::fs::read_to_string(&path)?;
        let parsed = toml::from_str::<HmiDirPageToml>(&text)?;
        pages.push((id.to_string(), parsed));
    }

    pages.sort_by(|left, right| left.0.cmp(&right.0));
    let mut parsed_pages = Vec::with_capacity(pages.len());
    for (idx, (id, parsed)) in pages.into_iter().enumerate() {
        parsed_pages.push(map_hmi_dir_page(id, idx, parsed));
    }
    parsed_pages.sort_by(|left, right| {
        left.order
            .cmp(&right.order)
            .then_with(|| left.id.cmp(&right.id))
    });
    promote_process_pages_to_custom_svg_if_available(&dir, &mut parsed_pages);

    Ok(HmiDirDescriptor {
        config,
        pages: parsed_pages,
    })
}

fn promote_process_pages_to_custom_svg_if_available(dir: &Path, pages: &mut [HmiDirPage]) {
    let Some(candidate) = find_custom_process_svg_candidate(dir) else {
        return;
    };
    for page in pages {
        if !page.kind.eq_ignore_ascii_case("process") {
            continue;
        }
        let svg_is_auto = page
            .svg
            .as_ref()
            .map(|value| value.trim().eq_ignore_ascii_case("process.auto.svg"))
            .unwrap_or(true);
        if svg_is_auto {
            page.svg = Some(candidate.clone());
        }
    }
}

fn find_custom_process_svg_candidate(dir: &Path) -> Option<String> {
    let mut svg_files = Vec::new();
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("svg"))
        {
            continue;
        }
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if name.eq_ignore_ascii_case("process.auto.svg") {
            continue;
        }
        svg_files.push(name.to_string());
    }
    svg_files.sort();
    svg_files.into_iter().next()
}

fn load_hmi_dir_config(dir: &Path) -> anyhow::Result<HmiDirConfig> {
    let path = dir.join("_config.toml");
    if !path.is_file() {
        return Ok(HmiDirConfig::default());
    }
    let text = std::fs::read_to_string(path)?;
    let parsed = toml::from_str::<HmiDirConfigToml>(&text)?;
    let mut alarms = parsed
        .alarms
        .into_iter()
        .filter_map(|alarm| {
            let bind = alarm.bind.trim();
            if bind.is_empty() {
                return None;
            }
            let label = alarm
                .label
                .map(|label| label.trim().to_string())
                .filter(|label| !label.is_empty());
            Some(HmiDirAlarm {
                bind: bind.to_string(),
                high: alarm.high,
                low: alarm.low,
                deadband: alarm.deadband.map(|value| value.max(0.0)),
                inferred: alarm.inferred,
                label,
            })
        })
        .collect::<Vec<_>>();
    alarms.sort_by(|left, right| left.bind.cmp(&right.bind));
    Ok(HmiDirConfig {
        version: parsed.version.or(Some(HMI_DESCRIPTOR_VERSION)),
        theme: parsed.theme,
        layout: parsed.layout,
        write: HmiDirWrite {
            enabled: parsed.write.enabled,
            allow: parsed
                .write
                .allow
                .into_iter()
                .map(|entry| entry.trim().to_string())
                .filter(|entry| !entry.is_empty())
                .collect(),
        },
        alarms,
    })
}

fn map_hmi_dir_page(id: String, default_index: usize, page: HmiDirPageToml) -> HmiDirPage {
    let title = page
        .title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| title_case(id.as_str()));
    let icon = page
        .icon
        .map(|icon| icon.trim().to_string())
        .filter(|icon| !icon.is_empty());
    let svg = page
        .svg
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty());
    let mut sections = Vec::with_capacity(page.sections.len());
    for (idx, section) in page.sections.into_iter().enumerate() {
        let title = section
            .title
            .map(|title| title.trim().to_string())
            .filter(|title| !title.is_empty())
            .unwrap_or_else(|| format!("Section {}", idx + 1));
        let span = section.span.unwrap_or(12).clamp(1, 12);
        let mut widgets = Vec::with_capacity(section.widgets.len());
        for widget in section.widgets {
            let bind = widget.bind.unwrap_or_default();
            let bind = bind.trim();
            if bind.is_empty() {
                continue;
            }
            let mut zones = widget.zones;
            zones.sort_by(|left, right| {
                left.from
                    .total_cmp(&right.from)
                    .then_with(|| left.to.total_cmp(&right.to))
            });
            widgets.push(HmiDirWidget {
                widget_type: widget
                    .widget_type
                    .map(|kind| kind.trim().to_ascii_lowercase())
                    .filter(|kind| !kind.is_empty()),
                bind: bind.to_string(),
                label: widget
                    .label
                    .map(|label| label.trim().to_string())
                    .filter(|label| !label.is_empty()),
                unit: widget
                    .unit
                    .map(|unit| unit.trim().to_string())
                    .filter(|unit| !unit.is_empty()),
                min: widget.min,
                max: widget.max,
                span: widget.span.map(|span| span.clamp(1, 12)),
                on_color: widget
                    .on_color
                    .map(|color| color.trim().to_string())
                    .filter(|color| !color.is_empty()),
                off_color: widget
                    .off_color
                    .map(|color| color.trim().to_string())
                    .filter(|color| !color.is_empty()),
                inferred_interface: widget.inferred_interface,
                detail_page: widget.detail_page.clone(),
                zones,
            });
        }
        sections.push(HmiDirSection {
            title,
            span,
            tier: section
                .tier
                .map(|t| t.trim().to_ascii_lowercase())
                .filter(|t| !t.is_empty()),
            widgets,
        });
    }

    let mut bindings = Vec::with_capacity(page.bindings.len());
    for binding in page.bindings {
        let selector = binding.selector.unwrap_or_default();
        let selector = selector.trim();
        if !is_safe_process_selector(selector) {
            continue;
        }
        let attribute = binding.attribute.unwrap_or_default();
        let attribute = attribute.trim();
        let Some(attribute) = normalize_process_attribute(attribute) else {
            continue;
        };
        let source = binding.source.unwrap_or_default();
        let source = source.trim();
        if source.is_empty() {
            continue;
        }
        let format = binding
            .format
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let map = binding
            .map
            .into_iter()
            .filter_map(|(key, value)| {
                let key = key.trim().to_string();
                let value = value.trim().to_string();
                if key.is_empty() || value.is_empty() {
                    return None;
                }
                Some((key, value))
            })
            .collect::<BTreeMap<_, _>>();
        let scale = binding.scale.and_then(normalize_process_scale);
        bindings.push(HmiDirProcessBinding {
            selector: selector.to_string(),
            attribute,
            source: source.to_string(),
            format,
            map,
            scale,
        });
    }
    bindings.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.selector.cmp(&right.selector))
            .then_with(|| left.attribute.cmp(&right.attribute))
    });

    HmiDirPage {
        id,
        title,
        icon,
        order: page.order.unwrap_or((default_index as i32) * 10),
        kind: normalize_page_kind(page.kind.as_deref()).to_string(),
        duration_ms: page.duration_s.map(|seconds| seconds.saturating_mul(1_000)),
        svg,
        hidden: page.hidden.unwrap_or(false),
        signals: page
            .signals
            .into_iter()
            .map(|signal| signal.trim().to_string())
            .filter(|signal| !signal.is_empty())
            .collect(),
        sections,
        bindings,
    }
}

fn apply_hmi_dir_descriptor(customization: &mut HmiCustomization, descriptor: &HmiDirDescriptor) {
    customization.theme.style = descriptor.config.theme.style.clone();
    customization.theme.accent = descriptor.config.theme.accent.clone();
    customization.write.enabled = descriptor.config.write.enabled;
    customization.write.allow = descriptor
        .config
        .write
        .allow
        .iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect();

    customization.pages = descriptor
        .pages
        .iter()
        .map(|page| HmiPageConfig {
            id: page.id.clone(),
            title: page.title.clone(),
            icon: page.icon.clone(),
            order: page.order,
            kind: page.kind.clone(),
            duration_ms: page.duration_ms,
            svg: page.svg.clone(),
            hidden: page.hidden,
            signals: page.signals.clone(),
            sections: page
                .sections
                .iter()
                .map(|section| HmiSectionConfig {
                    title: section.title.clone(),
                    span: section.span,
                    tier: section.tier.clone(),
                    widget_paths: section
                        .widgets
                        .iter()
                        .map(|widget| widget.bind.clone())
                        .collect(),
                })
                .collect(),
            bindings: page
                .bindings
                .iter()
                .map(|binding| HmiProcessBindingSchema {
                    selector: binding.selector.clone(),
                    attribute: binding.attribute.clone(),
                    source: binding.source.clone(),
                    format: binding.format.clone(),
                    map: binding.map.clone(),
                    scale: binding.scale.clone(),
                })
                .collect(),
        })
        .collect();

    let mut overrides = BTreeMap::<String, HmiWidgetOverride>::new();
    for (page_idx, page) in descriptor.pages.iter().enumerate() {
        // Hidden pages (equipment detail pages) must not steal widget
        // page/label/type assignments from visible pages.
        if page.hidden {
            continue;
        }
        for (section_idx, section) in page.sections.iter().enumerate() {
            for (widget_idx, widget) in section.widgets.iter().enumerate() {
                let key = widget.bind.trim();
                if key.is_empty() {
                    continue;
                }
                let entry = overrides.entry(key.to_string()).or_default();
                entry.merge_from(&HmiWidgetOverride {
                    label: widget.label.clone(),
                    unit: widget.unit.clone(),
                    min: widget.min,
                    max: widget.max,
                    widget: widget.widget_type.clone(),
                    page: Some(page.id.clone()),
                    group: Some(section.title.clone()),
                    order: Some(
                        ((page_idx as i32) * 10_000)
                            + ((section_idx as i32) * 100)
                            + widget_idx as i32,
                    ),
                    zones: widget.zones.clone(),
                    on_color: widget.on_color.clone(),
                    off_color: widget.off_color.clone(),
                    section_title: Some(section.title.clone()),
                    widget_span: widget.span,
                    alarm_deadband: None,
                    inferred_interface: widget.inferred_interface,
                    detail_page: widget.detail_page.clone(),
                });
            }
        }
    }

    for alarm in &descriptor.config.alarms {
        let key = alarm.bind.trim();
        if key.is_empty() {
            continue;
        }
        let entry = overrides.entry(key.to_string()).or_default();
        if let Some(low) = alarm.low {
            entry.min = Some(low);
        }
        if let Some(high) = alarm.high {
            entry.max = Some(high);
        }
        if let Some(deadband) = alarm.deadband {
            entry.alarm_deadband = Some(deadband.max(0.0));
        }
        if entry.label.is_none() {
            entry.label = alarm.label.clone();
        }
    }

    customization.widget_overrides = overrides;
}

pub fn collect_hmi_bindings_catalog(
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    sources: &[HmiSourceRef<'_>],
) -> HmiBindingsCatalog {
    let source_index = collect_source_symbol_index(sources);
    let points = collect_scaffold_points(metadata, snapshot, &source_index);
    let mut programs = BTreeMap::<String, HmiBindingsProgram>::new();
    let mut globals = Vec::new();

    for point in points {
        let program_key = point.program.to_ascii_uppercase();
        let variable = HmiBindingsVariable {
            name: point.raw_name.clone(),
            path: point.path.clone(),
            data_type: point.data_type.clone(),
            qualifier: point.qualifier.qualifier_label().to_string(),
            writable: point.writable,
            inferred_interface: point.inferred_interface,
            unit: point.unit.clone(),
            min: point.min,
            max: point.max,
            enum_values: point.enum_values.clone(),
        };

        if point.program.eq_ignore_ascii_case("global") {
            globals.push(variable);
            continue;
        }

        let entry = programs
            .entry(point.program.clone())
            .or_insert_with(|| HmiBindingsProgram {
                name: point.program.clone(),
                file: source_index.program_files.get(&program_key).cloned(),
                variables: Vec::new(),
            });
        if entry.file.is_none() {
            entry.file = source_index.program_files.get(&program_key).cloned();
        }
        entry.variables.push(variable);
    }

    let mut program_entries = programs.into_values().collect::<Vec<_>>();
    for program in &mut program_entries {
        program.variables.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.name.cmp(&right.name))
        });
    }
    program_entries.sort_by(|left, right| left.name.cmp(&right.name));

    globals.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.name.cmp(&right.name))
    });

    HmiBindingsCatalog {
        programs: program_entries,
        globals,
    }
}

pub fn scaffold_hmi_dir(
    root: &Path,
    metadata: &RuntimeMetadata,
    style: &str,
) -> anyhow::Result<HmiScaffoldSummary> {
    scaffold_hmi_dir_with_sources_mode(
        root,
        metadata,
        None,
        &[],
        style,
        HmiScaffoldMode::Reset,
        true,
    )
}

pub fn scaffold_hmi_dir_with_sources(
    root: &Path,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    sources: &[HmiSourceRef<'_>],
    style: &str,
) -> anyhow::Result<HmiScaffoldSummary> {
    scaffold_hmi_dir_with_sources_mode(
        root,
        metadata,
        snapshot,
        sources,
        style,
        HmiScaffoldMode::Reset,
        true,
    )
}

pub fn scaffold_hmi_dir_with_sources_mode(
    root: &Path,
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    sources: &[HmiSourceRef<'_>],
    style: &str,
    mode: HmiScaffoldMode,
    force: bool,
) -> anyhow::Result<HmiScaffoldSummary> {
    let style = normalize_scaffold_style(style);
    let palette = theme_palette(style.as_str())
        .or_else(|| theme_palette("industrial"))
        .expect("industrial theme");
    let source_index = collect_source_symbol_index(sources);
    let points = collect_scaffold_points(metadata, snapshot, &source_index);
    let overview_points = select_scaffold_overview_points(points.clone());
    let overview_result = build_tiered_overview_sections(overview_points);
    let overview_icon = infer_icon_for_points(&points);
    let overview_text = render_overview_toml(
        overview_icon.as_str(),
        &overview_result.sections,
        &overview_result.equipment_groups,
    );

    let numeric_signals = select_scaffold_trend_signals(&points);

    let mut alarms = points
        .iter()
        .filter_map(|point| match (point.writable, point.min, point.max) {
            (false, Some(min), Some(max)) if point.type_bucket == ScaffoldTypeBucket::Numeric => {
                let span = (max - min).abs();
                let deadband = if span > f64::EPSILON {
                    Some(span * 0.02)
                } else {
                    None
                };
                Some((point.path.clone(), point.label.clone(), min, max, deadband))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    alarms.sort_by(|left, right| left.0.cmp(&right.0));

    let mut program_names = metadata
        .programs()
        .keys()
        .map(|name| infer_label(name.as_str()))
        .collect::<Vec<_>>();
    program_names.sort();
    let header_title = program_names
        .first()
        .map(|name| format!("{name} HMI"))
        .unwrap_or_else(|| "trueST HMI".to_string());
    let config_text = render_config_toml(
        style.as_str(),
        palette.accent,
        header_title.as_str(),
        &alarms,
    );

    let hmi_dir = root.join("hmi");
    let hmi_exists = hmi_dir.is_dir();
    let hmi_has_files = hmi_exists
        && hmi_dir
            .read_dir()
            .ok()
            .is_some_and(|mut it| it.next().is_some());
    if mode == HmiScaffoldMode::Init && hmi_has_files && !force {
        anyhow::bail!(
            "hmi directory already exists at '{}' (run 'trust-runtime hmi update' to merge missing pages, 'trust-runtime hmi reset' to overwrite, or pass --force to init)",
            hmi_dir.display()
        );
    }
    std::fs::create_dir_all(&hmi_dir).map_err(|err| {
        anyhow::anyhow!(
            "failed to create scaffold directory '{}': {err}",
            hmi_dir.display()
        )
    })?;

    let mut files = Vec::new();
    if mode == HmiScaffoldMode::Reset && hmi_has_files {
        let backup_name = backup_existing_hmi_dir(root, &hmi_dir)?;
        files.push(HmiScaffoldFileSummary {
            path: backup_name,
            detail: "backup snapshot created before reset".to_string(),
        });
    }

    let has_writable_points = points.iter().any(|point| point.writable);
    let custom_process_pages_present = mode == HmiScaffoldMode::Update
        && hmi_has_custom_page_kind(&hmi_dir, "process", "process.toml");
    let skip_default_process_page = mode == HmiScaffoldMode::Update
        && !hmi_dir.join("process.toml").is_file()
        && custom_process_pages_present;
    let skip_default_control_page = mode == HmiScaffoldMode::Update
        && !hmi_dir.join("control.toml").is_file()
        && !has_writable_points;

    let process_text = render_process_toml(&points, "process.auto.svg");
    let process_svg_text = render_process_auto_svg();
    let control_text = render_control_toml(&points);
    let trends_text = render_trends_toml(&numeric_signals);
    let alarms_text = render_alarms_toml();

    let mut artifacts = vec![
        (
            "overview.toml",
            overview_text,
            format!(
                "{} sections, {} widgets",
                overview_result.sections.len(),
                overview_result
                    .sections
                    .iter()
                    .map(|section| section.widgets.len())
                    .sum::<usize>()
            ),
        ),
        (
            "trends.toml",
            trends_text,
            format!("{} curated numeric signals", numeric_signals.len()),
        ),
        (
            "alarms.toml",
            alarms_text,
            format!("{} alarm points", alarms.len()),
        ),
        (
            "_config.toml",
            config_text,
            format!("theme {style}, accent {}", palette.accent),
        ),
    ];
    if !skip_default_process_page {
        artifacts.push((
            "process.toml",
            process_text,
            "process page (auto-schematic mode)".to_string(),
        ));
        artifacts.push((
            "process.auto.svg",
            process_svg_text,
            "generated process topology SVG".to_string(),
        ));
    } else {
        files.push(HmiScaffoldFileSummary {
            path: "process.toml".to_string(),
            detail: "skipped (custom process page exists)".to_string(),
        });
        files.push(HmiScaffoldFileSummary {
            path: "process.auto.svg".to_string(),
            detail: "skipped (custom process page exists)".to_string(),
        });
    }
    if !skip_default_control_page {
        artifacts.push((
            "control.toml",
            control_text,
            "control page (commands/setpoints/modes)".to_string(),
        ));
    } else {
        files.push(HmiScaffoldFileSummary {
            path: "control.toml".to_string(),
            detail: "skipped (no writable points discovered)".to_string(),
        });
    }

    let overwrite = !matches!(mode, HmiScaffoldMode::Update);
    artifacts.sort_by(|left, right| left.0.cmp(right.0));
    for (name, text, detail) in artifacts {
        let path = hmi_dir.join(name);
        if !overwrite && path.exists() {
            if name.ends_with(".toml") && name != "_config.toml" {
                if let Ok(Some((merged_text, merge_detail))) =
                    merge_scaffold_update_page(path.as_path(), name, text.as_str())
                {
                    write_scaffold_file(&path, merged_text.as_str())?;
                    files.push(HmiScaffoldFileSummary {
                        path: name.to_string(),
                        detail: merge_detail,
                    });
                    continue;
                }
            }
            files.push(HmiScaffoldFileSummary {
                path: name.to_string(),
                detail: "preserved existing".to_string(),
            });
            continue;
        }
        write_scaffold_file(&path, text.as_str())?;
        files.push(HmiScaffoldFileSummary {
            path: name.to_string(),
            detail,
        });
    }

    // Generate equipment detail pages (hidden, accessible via equipment strip click-through)
    for (idx, group) in overview_result.equipment_groups.iter().enumerate() {
        let filename = format!("{}.toml", group.detail_page_id);
        let path = hmi_dir.join(&filename);
        if !overwrite && path.exists() {
            files.push(HmiScaffoldFileSummary {
                path: filename,
                detail: "preserved existing".to_string(),
            });
            continue;
        }
        let detail_text = render_equipment_detail_toml(group, 100 + idx as i32);
        write_scaffold_file(&path, detail_text.as_str())?;
        files.push(HmiScaffoldFileSummary {
            path: filename,
            detail: format!(
                "equipment detail: {} ({} signals)",
                group.title,
                group.widgets.len()
            ),
        });
    }

    files.push(HmiScaffoldFileSummary {
        path: "mode".to_string(),
        detail: mode.as_str().to_string(),
    });

    Ok(HmiScaffoldSummary { style, files })
}

fn hmi_has_custom_page_kind(hmi_dir: &Path, kind: &str, default_file_name: &str) -> bool {
    let normalized_kind = kind.trim().to_ascii_lowercase();
    let Ok(entries) = std::fs::read_dir(hmi_dir) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if !path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("toml"))
        {
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if file_name.eq_ignore_ascii_case("_config.toml")
            || file_name.eq_ignore_ascii_case(default_file_name)
        {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(parsed) = text.parse::<toml::Value>() else {
            continue;
        };
        let page_kind = parsed
            .get("kind")
            .and_then(toml::Value::as_str)
            .map(|value| value.to_ascii_lowercase())
            .unwrap_or_else(|| "dashboard".to_string());
        if page_kind == normalized_kind {
            return true;
        }
    }
    false
}

fn backup_existing_hmi_dir(root: &Path, hmi_dir: &Path) -> anyhow::Result<String> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let backup_name = format!("hmi.backup.{stamp}");
    let backup_dir = root.join(&backup_name);
    copy_dir_recursive(hmi_dir, &backup_dir)?;
    Ok(backup_name)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> anyhow::Result<()> {
    std::fs::create_dir_all(dst).map_err(|err| {
        anyhow::anyhow!(
            "failed to create backup directory '{}': {err}",
            dst.display()
        )
    })?;
    for entry in std::fs::read_dir(src).map_err(|err| {
        anyhow::anyhow!("failed to read source directory '{}': {err}", src.display())
    })? {
        let entry =
            entry.map_err(|err| anyhow::anyhow!("failed to read source directory entry: {err}"))?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_recursive(path.as_path(), dest_path.as_path())?;
        } else if path.is_file() {
            std::fs::copy(path.as_path(), dest_path.as_path()).map_err(|err| {
                anyhow::anyhow!(
                    "failed to backup '{}' to '{}': {err}",
                    path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn merge_scaffold_update_page(
    path: &Path,
    file_name: &str,
    generated_text: &str,
) -> anyhow::Result<Option<(String, String)>> {
    let page_id = file_name.trim_end_matches(".toml");
    if page_id.is_empty() {
        return Ok(None);
    }
    let existing_text = std::fs::read_to_string(path)?;
    let existing_toml = toml::from_str::<HmiDirPageToml>(&existing_text)?;
    let generated_toml = toml::from_str::<HmiDirPageToml>(generated_text)?;

    let existing_page = map_hmi_dir_page(page_id.to_string(), 0, existing_toml);
    let generated_page = map_hmi_dir_page(page_id.to_string(), 0, generated_toml);
    let (merged, changed) = merge_scaffold_page(existing_page, generated_page);
    if !changed {
        return Ok(None);
    }
    Ok(Some((
        render_hmi_dir_page_toml(&merged),
        "merged missing scaffold signals".to_string(),
    )))
}

fn merge_scaffold_page(existing: HmiDirPage, generated: HmiDirPage) -> (HmiDirPage, bool) {
    let mut merged = existing;
    let mut changed = false;

    if merged.kind == "trend" {
        let mut seen = merged
            .signals
            .iter()
            .map(|signal| signal.to_ascii_lowercase())
            .collect::<HashSet<_>>();
        for signal in generated.signals {
            let key = signal.to_ascii_lowercase();
            if seen.insert(key) {
                merged.signals.push(signal);
                changed = true;
            }
        }
        return (merged, changed);
    }

    if merged.kind == "process" {
        if merged.svg.is_none() && generated.svg.is_some() {
            merged.svg = generated.svg;
            changed = true;
        }
        let mut seen = merged
            .bindings
            .iter()
            .map(|binding| {
                (
                    binding.source.to_ascii_lowercase(),
                    binding.selector.to_ascii_lowercase(),
                    binding.attribute.to_ascii_lowercase(),
                )
            })
            .collect::<HashSet<_>>();
        for binding in generated.bindings {
            let key = (
                binding.source.to_ascii_lowercase(),
                binding.selector.to_ascii_lowercase(),
                binding.attribute.to_ascii_lowercase(),
            );
            if seen.insert(key) {
                merged.bindings.push(binding);
                changed = true;
            }
        }
        return (merged, changed);
    }

    let mut placed = HashSet::new();
    for section in &merged.sections {
        for widget in &section.widgets {
            placed.insert(widget.bind.to_ascii_lowercase());
        }
    }

    for generated_section in generated.sections {
        let mut additions = generated_section
            .widgets
            .into_iter()
            .filter(|widget| placed.insert(widget.bind.to_ascii_lowercase()))
            .collect::<Vec<_>>();
        if additions.is_empty() {
            continue;
        }
        if let Some(existing_section) = merged.sections.iter_mut().find(|section| {
            section
                .title
                .eq_ignore_ascii_case(generated_section.title.as_str())
        }) {
            existing_section.widgets.append(&mut additions);
        } else {
            merged.sections.push(HmiDirSection {
                title: generated_section.title,
                span: generated_section.span,
                tier: generated_section.tier.clone(),
                widgets: additions,
            });
        }
        changed = true;
    }

    (merged, changed)
}

fn render_process_toml(points: &[ScaffoldPoint], svg_name: &str) -> String {
    const TANK_FILL_BOTTOM_Y: i32 = 480;
    const TANK_FILL_TOP_Y: i32 = 200;
    const TANK_FILL_MAX_HEIGHT: i32 = TANK_FILL_BOTTOM_Y - TANK_FILL_TOP_Y;

    let mut out = String::new();
    let _ = writeln!(out, "title = \"Process\"");
    let _ = writeln!(out, "kind = \"process\"");
    let _ = writeln!(out, "icon = \"workflow\"");
    let _ = writeln!(out, "order = 20");
    let _ = writeln!(out, "svg = \"{}\"", escape_toml_string(svg_name));

    let running = select_scaffold_point(points, &["run", "running", "enabled"], None);
    let flow = select_scaffold_point(points, &["flow"], Some(ScaffoldTypeBucket::Numeric));
    let pressure = select_scaffold_point(
        points,
        &["pressure", "bar", "pt"],
        Some(ScaffoldTypeBucket::Numeric),
    );
    let feed_level = select_scaffold_point(
        points,
        &["feed", "source", "inlet", "level"],
        Some(ScaffoldTypeBucket::Numeric),
    );
    let product_level = select_scaffold_point(
        points,
        &["product", "outlet", "tank", "level"],
        Some(ScaffoldTypeBucket::Numeric),
    );

    if let Some(point) = flow.as_ref() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind]]");
        let _ = writeln!(out, "selector = \"#pid-flow-value\"");
        let _ = writeln!(out, "attribute = \"text\"");
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(point.path.as_str())
        );
        let _ = writeln!(out, "format = \"{}\"", escape_toml_string("{} m3/h"));
    }

    if let Some(point) = pressure.as_ref() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind]]");
        let _ = writeln!(out, "selector = \"#pid-pressure-value\"");
        let _ = writeln!(out, "attribute = \"text\"");
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(point.path.as_str())
        );
        let _ = writeln!(out, "format = \"{}\"", escape_toml_string("{} bar"));
    }

    if let Some(point) = feed_level.as_ref() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind]]");
        let _ = writeln!(out, "selector = \"#pid-feed-level-value\"");
        let _ = writeln!(out, "attribute = \"text\"");
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(point.path.as_str())
        );
        let _ = writeln!(out, "format = \"{}\"", escape_toml_string("{} %"));

        let min = point.min.unwrap_or(0.0);
        let max = point.max.unwrap_or(100.0);
        if max > min {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[bind]]");
            let _ = writeln!(out, "selector = \"#pid-feed-level-fill\"");
            let _ = writeln!(out, "attribute = \"y\"");
            let _ = writeln!(
                out,
                "source = \"{}\"",
                escape_toml_string(point.path.as_str())
            );
            let _ = writeln!(
                out,
                "scale = {{ min = {}, max = {}, output_min = {}, output_max = {} }}",
                format_toml_number(min),
                format_toml_number(max),
                TANK_FILL_BOTTOM_Y,
                TANK_FILL_TOP_Y
            );

            let _ = writeln!(out);
            let _ = writeln!(out, "[[bind]]");
            let _ = writeln!(out, "selector = \"#pid-feed-level-fill\"");
            let _ = writeln!(out, "attribute = \"height\"");
            let _ = writeln!(
                out,
                "source = \"{}\"",
                escape_toml_string(point.path.as_str())
            );
            let _ = writeln!(
                out,
                "scale = {{ min = {}, max = {}, output_min = 0, output_max = {} }}",
                format_toml_number(min),
                format_toml_number(max),
                TANK_FILL_MAX_HEIGHT
            );
        }
    }

    if let Some(point) = product_level.as_ref() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind]]");
        let _ = writeln!(out, "selector = \"#pid-product-level-value\"");
        let _ = writeln!(out, "attribute = \"text\"");
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(point.path.as_str())
        );
        let _ = writeln!(out, "format = \"{}\"", escape_toml_string("{} %"));

        let min = point.min.unwrap_or(0.0);
        let max = point.max.unwrap_or(100.0);
        if max > min {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[bind]]");
            let _ = writeln!(out, "selector = \"#pid-product-level-fill\"");
            let _ = writeln!(out, "attribute = \"y\"");
            let _ = writeln!(
                out,
                "source = \"{}\"",
                escape_toml_string(point.path.as_str())
            );
            let _ = writeln!(
                out,
                "scale = {{ min = {}, max = {}, output_min = {}, output_max = {} }}",
                format_toml_number(min),
                format_toml_number(max),
                TANK_FILL_BOTTOM_Y,
                TANK_FILL_TOP_Y
            );

            let _ = writeln!(out);
            let _ = writeln!(out, "[[bind]]");
            let _ = writeln!(out, "selector = \"#pid-product-level-fill\"");
            let _ = writeln!(out, "attribute = \"height\"");
            let _ = writeln!(
                out,
                "source = \"{}\"",
                escape_toml_string(point.path.as_str())
            );
            let _ = writeln!(
                out,
                "scale = {{ min = {}, max = {}, output_min = 0, output_max = {} }}",
                format_toml_number(min),
                format_toml_number(max),
                TANK_FILL_MAX_HEIGHT
            );
        }
    }

    if let Some(point) = running.as_ref() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind]]");
        let _ = writeln!(out, "selector = \"#pid-pump-indicator\"");
        let _ = writeln!(out, "attribute = \"fill\"");
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(point.path.as_str())
        );
        let _ = writeln!(
            out,
            "map = {{ \"true\" = \"#22c55e\", \"false\" = \"#ef4444\" }}"
        );

        let _ = writeln!(out);
        let _ = writeln!(out, "[[bind]]");
        let _ = writeln!(out, "selector = \"#pid-main-line\"");
        let _ = writeln!(out, "attribute = \"stroke\"");
        let _ = writeln!(
            out,
            "source = \"{}\"",
            escape_toml_string(point.path.as_str())
        );
        let _ = writeln!(
            out,
            "map = {{ \"true\" = \"#2563eb\", \"false\" = \"#94a3b8\" }}"
        );
    }

    out
}

fn render_process_auto_svg() -> String {
    [
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 1280 720\">",
        "  <defs>",
        "    <style>",
        "      .pid-title { font-family: 'IBM Plex Sans', 'Segoe UI', sans-serif; fill: #1f2937; }",
        "      .pid-label { font-family: 'IBM Plex Sans', 'Segoe UI', sans-serif; fill: #64748b; }",
        "      .pid-value { font-family: 'IBM Plex Mono', 'Consolas', monospace; fill: #2563eb; }",
        "      .pid-shell { fill: #ffffff; stroke: #475569; stroke-width: 2.5; }",
        "      .pid-line { fill: none; stroke: #94a3b8; stroke-width: 6; stroke-linecap: round; }",
        "      .pid-symbol { fill: none; stroke: #475569; stroke-width: 2.4; stroke-linecap: round; stroke-linejoin: round; }",
        "      .pid-solid { fill: #475569; }",
        "      .pid-flow-arrow { fill: #94a3b8; }",
        "    </style>",
        "    <pattern id=\"pid-layout-grid\" width=\"40\" height=\"40\" patternUnits=\"userSpaceOnUse\">",
        "      <path d=\"M40 0H0V40\" fill=\"none\" stroke=\"#cbd5e1\" stroke-width=\"1\"/>",
        "    </pattern>",
        "  </defs>",
        "  <rect x=\"0\" y=\"0\" width=\"1280\" height=\"720\" fill=\"#f8fafc\"/>",
        "  <rect x=\"20\" y=\"20\" width=\"1240\" height=\"680\" rx=\"10\" fill=\"#ffffff\" stroke=\"#e2e8f0\" stroke-width=\"1.5\"/>",
        "  <g id=\"pid-layout-guides\" opacity=\"0\" pointer-events=\"none\">",
        "    <rect x=\"120\" y=\"180\" width=\"1040\" height=\"320\" fill=\"url(#pid-layout-grid)\"/>",
        "    <rect x=\"120\" y=\"180\" width=\"1040\" height=\"320\" fill=\"none\" stroke=\"#cbd5e1\" stroke-width=\"1\"/>",
        "  </g>",
        "  <text class=\"pid-title\" x=\"120\" y=\"96\" font-size=\"30\" font-weight=\"700\">Auto Process View</text>",
        "  <text class=\"pid-label\" x=\"120\" y=\"122\" font-size=\"15\">Deterministic grid layout (40px cell): FIT/PT use identical instrument templates and value offsets.</text>",
        "  <rect x=\"120\" y=\"180\" width=\"200\" height=\"320\" rx=\"12\" class=\"pid-shell\"/>",
        "  <rect id=\"pid-feed-level-fill\" x=\"140\" y=\"480\" width=\"160\" height=\"0\" rx=\"6\" fill=\"#60a5fa\" opacity=\"0.62\"/>",
        "  <text class=\"pid-title\" x=\"145\" y=\"220\" font-size=\"20\" font-weight=\"700\">FEED TANK</text>",
        "  <text id=\"pid-feed-level-value\" class=\"pid-value\" x=\"145\" y=\"248\" font-size=\"18\">-- %</text>",
        "  <line id=\"pid-main-line\" x1=\"320\" y1=\"360\" x2=\"960\" y2=\"360\" class=\"pid-line\"/>",
        "  <polygon class=\"pid-flow-arrow\" points=\"430,352 444,360 430,368\"/>",
        "  <polygon class=\"pid-flow-arrow\" points=\"630,352 644,360 630,368\"/>",
        "  <polygon class=\"pid-flow-arrow\" points=\"840,352 854,360 840,368\"/>",
        "  <g id=\"pid-pump-001\" transform=\"translate(400,280)\">",
        "    <circle cx=\"80\" cy=\"80\" r=\"24\" class=\"pid-symbol\"/>",
        "    <path d=\"M66 98 L94 98 L80 74 Z\" class=\"pid-solid\" transform=\"rotate(90 80 80)\"/>",
        "    <circle id=\"pid-pump-indicator\" cx=\"122\" cy=\"44\" r=\"10\" fill=\"#ef4444\" stroke=\"#ffffff\" stroke-width=\"2\"/>",
        "    <text class=\"pid-title\" x=\"38\" y=\"158\" font-size=\"16\" font-weight=\"700\">PUMP</text>",
        "  </g>",
        "  <g id=\"pid-fit-001\" transform=\"translate(500,240)\">",
        "    <line x1=\"80\" y1=\"62\" x2=\"80\" y2=\"120\" class=\"pid-symbol\"/>",
        "    <circle cx=\"80\" cy=\"40\" r=\"22\" class=\"pid-symbol\"/>",
        "    <line x1=\"62\" y1=\"40\" x2=\"98\" y2=\"40\" class=\"pid-symbol\"/>",
        "    <text class=\"pid-title\" x=\"80\" y=\"-24\" font-size=\"14\" font-weight=\"700\" text-anchor=\"middle\">FIT-001</text>",
        "    <text id=\"pid-flow-value\" class=\"pid-value\" x=\"80\" y=\"-4\" font-size=\"14\" text-anchor=\"middle\">-- m3/h</text>",
        "  </g>",
        "  <g id=\"pid-valve-001\" transform=\"translate(620,280)\">",
        "    <polygon points=\"46,52 80,80 46,108\" class=\"pid-symbol\"/>",
        "    <polygon points=\"114,52 80,80 114,108\" class=\"pid-symbol\"/>",
        "    <text class=\"pid-title\" x=\"28\" y=\"158\" font-size=\"16\" font-weight=\"700\">VALVE</text>",
        "  </g>",
        "  <g id=\"pid-pt-001\" transform=\"translate(740,240)\">",
        "    <line x1=\"80\" y1=\"62\" x2=\"80\" y2=\"120\" class=\"pid-symbol\"/>",
        "    <circle cx=\"80\" cy=\"40\" r=\"22\" class=\"pid-symbol\"/>",
        "    <line x1=\"62\" y1=\"40\" x2=\"98\" y2=\"40\" class=\"pid-symbol\"/>",
        "    <text class=\"pid-title\" x=\"80\" y=\"-24\" font-size=\"14\" font-weight=\"700\" text-anchor=\"middle\">PT-001</text>",
        "    <text id=\"pid-pressure-value\" class=\"pid-value\" x=\"80\" y=\"-4\" font-size=\"14\" text-anchor=\"middle\">-- bar</text>",
        "  </g>",
        "  <rect x=\"960\" y=\"180\" width=\"200\" height=\"320\" rx=\"12\" class=\"pid-shell\"/>",
        "  <rect id=\"pid-product-level-fill\" x=\"980\" y=\"480\" width=\"160\" height=\"0\" rx=\"6\" fill=\"#34d399\" opacity=\"0.72\"/>",
        "  <text class=\"pid-title\" x=\"985\" y=\"220\" font-size=\"20\" font-weight=\"700\">PRODUCT</text>",
        "  <text id=\"pid-product-level-value\" class=\"pid-value\" x=\"985\" y=\"248\" font-size=\"18\">-- %</text>",
        "</svg>",
    ]
    .join("\n")
}

fn select_scaffold_point<'a>(
    points: &'a [ScaffoldPoint],
    hints: &[&str],
    bucket: Option<ScaffoldTypeBucket>,
) -> Option<&'a ScaffoldPoint> {
    let mut by_score = points
        .iter()
        .filter(|point| bucket.is_none_or(|kind| point.type_bucket == kind))
        .map(|point| {
            let haystack = format!(
                "{} {} {}",
                point.path.to_ascii_lowercase(),
                point.raw_name.to_ascii_lowercase(),
                point.label.to_ascii_lowercase()
            );
            let score = hints
                .iter()
                .filter(|hint| haystack.contains(&hint.to_ascii_lowercase()))
                .count();
            (score, point)
        })
        .collect::<Vec<_>>();
    by_score.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.path.cmp(&right.1.path))
    });
    by_score
        .into_iter()
        .find(|(score, _)| *score > 0)
        .map(|(_, point)| point)
}

fn render_control_toml(points: &[ScaffoldPoint]) -> String {
    let mut commands = Vec::new();
    let mut setpoints = Vec::new();
    let mut modes = Vec::new();
    let mut text_fields = Vec::new();

    for point in points {
        if !point.writable {
            continue;
        }
        match point.type_bucket {
            ScaffoldTypeBucket::Bool => commands.push(point.clone()),
            ScaffoldTypeBucket::Numeric => setpoints.push(point.clone()),
            ScaffoldTypeBucket::Text => text_fields.push(point.clone()),
            _ => modes.push(point.clone()),
        }
    }

    for entries in [&mut commands, &mut setpoints, &mut modes, &mut text_fields] {
        entries.sort_by(|left, right| {
            left.label
                .cmp(&right.label)
                .then_with(|| left.path.cmp(&right.path))
        });
    }

    let mut out = String::new();
    let _ = writeln!(out, "title = \"Control\"");
    let _ = writeln!(out, "icon = \"sliders\"");
    let _ = writeln!(out, "order = 30");
    let _ = writeln!(out, "kind = \"dashboard\"");
    render_control_section(&mut out, "Commands", 4, &commands);
    render_control_section(&mut out, "Setpoints", 8, &setpoints);
    render_control_section(&mut out, "Modes", 6, &modes);
    render_control_section(&mut out, "Text Inputs", 6, &text_fields);
    out
}

fn render_control_section(out: &mut String, title: &str, span: u32, widgets: &[ScaffoldPoint]) {
    if widgets.is_empty() {
        return;
    }
    let _ = writeln!(out);
    let _ = writeln!(out, "[[section]]");
    let _ = writeln!(out, "title = \"{}\"", escape_toml_string(title));
    let _ = writeln!(out, "span = {}", span.clamp(1, 12));

    for point in widgets {
        let widget_type = match point.type_bucket {
            ScaffoldTypeBucket::Bool => "toggle",
            ScaffoldTypeBucket::Numeric => "slider",
            _ => point.widget.as_str(),
        };
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section.widget]]");
        let _ = writeln!(out, "type = \"{}\"", escape_toml_string(widget_type));
        let _ = writeln!(
            out,
            "bind = \"{}\"",
            escape_toml_string(point.path.as_str())
        );
        if point.inferred_interface {
            let _ = writeln!(out, "inferred_interface = true");
        }
        let _ = writeln!(
            out,
            "label = \"{}\"",
            escape_toml_string(point.label.as_str())
        );
        let _ = writeln!(
            out,
            "span = {}",
            if point.type_bucket == ScaffoldTypeBucket::Numeric {
                6
            } else {
                4
            }
        );
        if let Some(unit) = point.unit.as_ref() {
            let _ = writeln!(out, "unit = \"{}\"", escape_toml_string(unit));
        }
        if let Some(min) = point.min {
            let _ = writeln!(out, "min = {}", format_toml_number(min));
        }
        if let Some(max) = point.max {
            let _ = writeln!(out, "max = {}", format_toml_number(max));
        }
    }
}

fn write_scaffold_file(path: &Path, text: &str) -> anyhow::Result<()> {
    std::fs::write(path, text)
        .map_err(|err| anyhow::anyhow!("failed to write scaffold file '{}': {err}", path.display()))
}

fn render_overview_toml(
    icon: &str,
    sections: &[ScaffoldSection],
    equipment_groups: &[ScaffoldEquipmentGroup],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "title = \"Overview\"");
    let _ = writeln!(out, "icon = \"{}\"", escape_toml_string(icon));
    let _ = writeln!(out, "order = 0");

    for section in sections {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(
            out,
            "title = \"{}\"",
            escape_toml_string(section.title.as_str())
        );
        let _ = writeln!(out, "span = {}", section.span);
        if let Some(tier) = section.tier.as_ref() {
            let _ = writeln!(out, "tier = \"{}\"", escape_toml_string(tier));
        }

        let is_module = section.tier.as_deref() == Some("module");

        for point in &section.widgets {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            let _ = writeln!(out, "type = \"{}\"", escape_toml_string(&point.widget));
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(&point.path));
            if point.inferred_interface {
                let _ = writeln!(out, "inferred_interface = true");
            }
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(&point.label));
            // For module widgets, link to their equipment detail page.
            if is_module {
                if let Some(group) = equipment_groups
                    .iter()
                    .find(|g| g.widgets.iter().any(|w| w.path == point.path))
                {
                    let _ = writeln!(
                        out,
                        "detail_page = \"{}\"",
                        escape_toml_string(&group.detail_page_id)
                    );
                }
            }
            let _ = writeln!(
                out,
                "span = {}",
                overview_widget_span(point, section.tier.as_deref())
            );
            if let Some(unit) = point.unit.as_ref() {
                let _ = writeln!(out, "unit = \"{}\"", escape_toml_string(unit));
            }
            if let Some(min) = point.min {
                let _ = writeln!(out, "min = {}", format_toml_number(min));
            }
            if let Some(max) = point.max {
                let _ = writeln!(out, "max = {}", format_toml_number(max));
            }
        }
    }

    out
}

fn render_equipment_detail_toml(group: &ScaffoldEquipmentGroup, order: i32) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "title = \"{}\"", escape_toml_string(&group.title));
    let _ = writeln!(out, "icon = \"settings\"");
    let _ = writeln!(out, "order = {order}");
    let _ = writeln!(out, "kind = \"dashboard\"");
    let _ = writeln!(out, "hidden = true");

    // Status section: boolean signals
    let bools: Vec<_> = group
        .widgets
        .iter()
        .filter(|w| w.type_bucket == ScaffoldTypeBucket::Bool)
        .collect();
    if !bools.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(out, "title = \"Status\"");
        let _ = writeln!(out, "span = 12");
        for point in &bools {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            let _ = writeln!(out, "type = \"indicator\"");
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(&point.path));
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(&point.label));
            let _ = writeln!(out, "span = 6");
        }
    }

    // Values section: numeric signals
    let numerics: Vec<_> = group
        .widgets
        .iter()
        .filter(|w| w.type_bucket == ScaffoldTypeBucket::Numeric)
        .collect();
    if !numerics.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(out, "title = \"Values\"");
        let _ = writeln!(out, "span = 12");
        for point in &numerics {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            let _ = writeln!(out, "type = \"gauge\"");
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(&point.path));
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(&point.label));
            let _ = writeln!(out, "span = 6");
            if let Some(unit) = point.unit.as_ref() {
                let _ = writeln!(out, "unit = \"{}\"", escape_toml_string(unit));
            }
            if let Some(min) = point.min {
                let _ = writeln!(out, "min = {}", format_toml_number(min));
            }
            if let Some(max) = point.max {
                let _ = writeln!(out, "max = {}", format_toml_number(max));
            }
        }
    }

    // Text/enum section: text signals
    let strings: Vec<_> = group
        .widgets
        .iter()
        .filter(|w| w.type_bucket == ScaffoldTypeBucket::Text)
        .collect();
    if !strings.is_empty() {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[section]]");
        let _ = writeln!(out, "title = \"Text\"");
        let _ = writeln!(out, "span = 12");
        for point in &strings {
            let _ = writeln!(out);
            let _ = writeln!(out, "[[section.widget]]");
            let _ = writeln!(out, "type = \"text\"");
            let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(&point.path));
            let _ = writeln!(out, "label = \"{}\"", escape_toml_string(&point.label));
            let _ = writeln!(out, "span = 6");
        }
    }

    out
}

fn render_trends_toml(signals: &[String]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "title = \"Trends\"");
    let _ = writeln!(out, "kind = \"trend\"");
    let _ = writeln!(out, "icon = \"line-chart\"");
    let _ = writeln!(out, "order = 50");
    let _ = writeln!(out, "duration_s = 600");
    if signals.is_empty() {
        let _ = writeln!(out, "signals = []");
    } else {
        let formatted = signals
            .iter()
            .take(8)
            .map(|signal| format!("\"{}\"", escape_toml_string(signal)))
            .collect::<Vec<_>>()
            .join(", ");
        let _ = writeln!(out, "signals = [{formatted}]");
    }
    out
}

fn render_alarms_toml() -> String {
    let mut out = String::new();
    let _ = writeln!(out, "title = \"Alarms\"");
    let _ = writeln!(out, "kind = \"alarm\"");
    let _ = writeln!(out, "icon = \"bell\"");
    let _ = writeln!(out, "order = 60");
    out
}

fn render_config_toml(
    style: &str,
    accent: &str,
    header_title: &str,
    alarms: &[(String, String, f64, f64, Option<f64>)],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "version = {HMI_DESCRIPTOR_VERSION}");
    let _ = writeln!(out);
    let _ = writeln!(out, "[theme]");
    let _ = writeln!(out, "style = \"{}\"", escape_toml_string(style));
    let _ = writeln!(out, "accent = \"{}\"", escape_toml_string(accent));
    let _ = writeln!(out);
    let _ = writeln!(out, "[layout]");
    let _ = writeln!(out, "navigation = \"sidebar-left\"");
    let _ = writeln!(out, "header = true");
    let _ = writeln!(
        out,
        "header_title = \"{}\"",
        escape_toml_string(header_title)
    );
    let _ = writeln!(out);
    let _ = writeln!(out, "[write]");
    let _ = writeln!(out, "enabled = false");
    let _ = writeln!(out, "allow = []");

    for (bind, label, low, high, deadband) in alarms {
        let _ = writeln!(out);
        let _ = writeln!(out, "[[alarm]]");
        let _ = writeln!(out, "bind = \"{}\"", escape_toml_string(bind));
        let _ = writeln!(out, "high = {}", format_toml_number(*high));
        let _ = writeln!(out, "low = {}", format_toml_number(*low));
        if let Some(deadband) = deadband {
            let _ = writeln!(out, "deadband = {}", format_toml_number(*deadband));
        }
        let _ = writeln!(out, "inferred = true");
        let _ = writeln!(out, "label = \"{}\"", escape_toml_string(label));
    }

    out
}

fn format_toml_number(value: f64) -> String {
    if (value.fract()).abs() < f64::EPSILON {
        format!("{value:.0}")
    } else {
        let mut text = format!("{value:.3}");
        while text.ends_with('0') {
            let _ = text.pop();
        }
        if text.ends_with('.') {
            text.push('0');
        }
        text
    }
}

fn normalize_scaffold_style(style: &str) -> String {
    let candidate = style.trim().to_ascii_lowercase();
    if theme_palette(candidate.as_str()).is_some() {
        candidate
    } else {
        "control-room".to_string()
    }
}

fn collect_scaffold_points(
    metadata: &RuntimeMetadata,
    snapshot: Option<&DebugSnapshot>,
    source_index: &SourceSymbolIndex,
) -> Vec<ScaffoldPoint> {
    let mut points = Vec::new();

    for (program_name, program) in metadata.programs() {
        let program_key = program_name.to_ascii_uppercase();
        let program_has_entries = source_index.programs_with_entries.contains(&program_key);
        let mut program_points = Vec::new();
        let mut external_points_added = false;
        for variable in &program.vars {
            if variable.constant {
                continue;
            }
            let key = normalize_symbol_key(program_name.as_str(), variable.name.as_str());
            let qualifier = source_index
                .program_vars
                .get(key.as_str())
                .copied()
                .unwrap_or(if program_has_entries {
                    SourceVarKind::Unknown
                } else {
                    SourceVarKind::Output
                });
            if program_has_entries && !qualifier.is_external() {
                continue;
            }
            external_points_added = true;

            let writable = qualifier.is_writable();
            let ty = metadata.registry().get(variable.type_id);
            let data_type = metadata
                .registry()
                .type_name(variable.type_id)
                .map(|name| name.to_string())
                .unwrap_or_else(|| "UNKNOWN".to_string());
            let type_bucket = ty
                .map(scaffold_type_bucket_for_type)
                .unwrap_or(ScaffoldTypeBucket::Other);
            let widget = ty
                .map(|ty| widget_for_scaffold_type(ty, writable, qualifier).to_string())
                .unwrap_or_else(|| "value".to_string());
            let path = format!("{program_name}.{}", variable.name);
            let (unit, min, max) =
                infer_unit_and_range(path.as_str(), data_type.as_str(), type_bucket);

            program_points.push(ScaffoldPoint {
                program: program_name.to_string(),
                raw_name: variable.name.to_string(),
                path,
                label: infer_label(variable.name.as_str()),
                data_type: data_type.clone(),
                widget,
                writable,
                qualifier,
                inferred_interface: !program_has_entries,
                type_bucket,
                unit,
                min,
                max,
                enum_values: ty.map(enum_values_for_type).unwrap_or_default(),
            });
        }

        if program_has_entries && !external_points_added {
            for variable in &program.vars {
                if variable.constant {
                    continue;
                }
                let ty = metadata.registry().get(variable.type_id);
                let data_type = metadata
                    .registry()
                    .type_name(variable.type_id)
                    .map(|name| name.to_string())
                    .unwrap_or_else(|| "UNKNOWN".to_string());
                let type_bucket = ty
                    .map(scaffold_type_bucket_for_type)
                    .unwrap_or(ScaffoldTypeBucket::Other);
                let widget = ty
                    .map(|ty| {
                        widget_for_scaffold_type(ty, false, SourceVarKind::Output).to_string()
                    })
                    .unwrap_or_else(|| "value".to_string());
                let path = format!("{program_name}.{}", variable.name);
                let (unit, min, max) =
                    infer_unit_and_range(path.as_str(), data_type.as_str(), type_bucket);

                program_points.push(ScaffoldPoint {
                    program: program_name.to_string(),
                    raw_name: variable.name.to_string(),
                    path,
                    label: infer_label(variable.name.as_str()),
                    data_type: data_type.clone(),
                    widget,
                    writable: false,
                    qualifier: SourceVarKind::Unknown,
                    inferred_interface: true,
                    type_bucket,
                    unit,
                    min,
                    max,
                    enum_values: ty.map(enum_values_for_type).unwrap_or_default(),
                });
            }
        }

        points.extend(program_points);
    }

    if let Some(snapshot) = snapshot {
        let program_names = metadata
            .programs()
            .keys()
            .map(|name| name.to_ascii_uppercase())
            .collect::<HashSet<_>>();
        let has_global_filter = !source_index.globals.is_empty();
        for (name, value) in snapshot.storage.globals() {
            if program_names.contains(&name.to_ascii_uppercase()) {
                continue;
            }
            if matches!(value, Value::Instance(_)) {
                continue;
            }
            if has_global_filter && !source_index.globals.contains(&name.to_ascii_uppercase()) {
                continue;
            }

            let data_type = value_type_name(value).unwrap_or_else(|| "UNKNOWN".to_string());
            let type_bucket = scaffold_type_bucket_for_value(value, data_type.as_str());
            let path = format!("global.{name}");
            let (unit, min, max) =
                infer_unit_and_range(path.as_str(), data_type.as_str(), type_bucket);

            points.push(ScaffoldPoint {
                program: "global".to_string(),
                raw_name: name.to_string(),
                path,
                label: infer_label(name.as_str()),
                data_type,
                widget: widget_for_scaffold_value(value).to_string(),
                writable: false,
                qualifier: SourceVarKind::Global,
                inferred_interface: !has_global_filter,
                type_bucket,
                unit,
                min,
                max,
                enum_values: Vec::new(),
            });
        }
    }

    points.sort_by(|left, right| {
        left.program
            .cmp(&right.program)
            .then_with(|| left.path.cmp(&right.path))
    });
    points
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ScaffoldOverviewCategory {
    SafetyAlarm,
    CommandMode,
    Kpi,
    Deviation,
    Inventory,
    Diagnostic,
}

impl ScaffoldOverviewCategory {
    const fn weight(self) -> i32 {
        match self {
            Self::SafetyAlarm => 100,
            Self::CommandMode => 80,
            Self::Kpi => 60,
            Self::Deviation => 45,
            Self::Inventory => 35,
            Self::Diagnostic => 20,
        }
    }

    const fn slot_cap(self) -> usize {
        match self {
            Self::SafetyAlarm => 2,
            Self::CommandMode => 2,
            Self::Kpi | Self::Deviation => 4,
            Self::Inventory => 2,
            Self::Diagnostic => 2,
        }
    }
}

fn classify_overview_category(point: &ScaffoldPoint) -> ScaffoldOverviewCategory {
    let name = format!(
        "{} {} {}",
        point.path.to_ascii_lowercase(),
        point.raw_name.to_ascii_lowercase(),
        point.label.to_ascii_lowercase()
    );
    if contains_any(
        name.as_str(),
        &[
            "alarm",
            "fault",
            "trip",
            "interlock",
            "estop",
            "emergency",
            "safety",
        ],
    ) {
        return ScaffoldOverviewCategory::SafetyAlarm;
    }
    if contains_any(name.as_str(), &["deviation", "delta", "error", "diff"]) {
        return ScaffoldOverviewCategory::Deviation;
    }
    if contains_any(
        name.as_str(),
        &[
            "inventory",
            "tank",
            "feed",
            "source",
            "product",
            "stock",
            "level",
        ],
    ) {
        return ScaffoldOverviewCategory::Inventory;
    }
    if point.writable
        || contains_any(
            name.as_str(),
            &[
                "mode", "cmd", "command", "start", "stop", "reset", "enable", "bypass",
            ],
        )
    {
        return ScaffoldOverviewCategory::CommandMode;
    }
    if point.type_bucket == ScaffoldTypeBucket::Numeric
        && contains_any(
            name.as_str(),
            &[
                "flow",
                "pressure",
                "temp",
                "temperature",
                "speed",
                "rpm",
                "current",
                "voltage",
                "power",
                "rate",
                "level",
            ],
        )
    {
        return ScaffoldOverviewCategory::Kpi;
    }
    ScaffoldOverviewCategory::Diagnostic
}

fn select_scaffold_overview_points(points: Vec<ScaffoldPoint>) -> Vec<ScaffoldPoint> {
    let budget = 10_usize;
    if points.len() <= budget {
        return points;
    }

    let mut scored = points
        .into_iter()
        .enumerate()
        .map(|(index, point)| {
            let category = classify_overview_category(&point);
            (category.weight(), category, index, point)
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.2.cmp(&right.2))
            .then_with(|| left.3.path.cmp(&right.3.path))
    });

    let mut selected = Vec::new();
    let mut overflow = Vec::new();
    let mut category_counts = HashMap::<ScaffoldOverviewCategory, usize>::new();

    for item in scored {
        let count = category_counts.get(&item.1).copied().unwrap_or_default();
        if count < item.1.slot_cap() && selected.len() < budget {
            category_counts.insert(item.1, count + 1);
            selected.push(item);
        } else {
            overflow.push(item);
        }
    }
    for item in overflow {
        if selected.len() >= budget {
            break;
        }
        selected.push(item);
    }

    selected
        .into_iter()
        .map(|(_, _, _, point)| point)
        .collect::<Vec<_>>()
}

fn select_scaffold_trend_signals(points: &[ScaffoldPoint]) -> Vec<String> {
    let mut scored = points
        .iter()
        .filter(|point| point.type_bucket == ScaffoldTypeBucket::Numeric)
        .map(|point| {
            let name = format!(
                "{} {} {}",
                point.path.to_ascii_lowercase(),
                point.raw_name.to_ascii_lowercase(),
                point.label.to_ascii_lowercase()
            );
            let mut score = 0_i32;
            if contains_any(
                name.as_str(),
                &[
                    "flow",
                    "pressure",
                    "temp",
                    "temperature",
                    "level",
                    "speed",
                    "rpm",
                    "deviation",
                    "error",
                ],
            ) {
                score += 50;
            }
            if contains_any(name.as_str(), &["setpoint", "sp"]) {
                score += 18;
            }
            if contains_any(
                name.as_str(),
                &[
                    "cmd", "command", "mode", "counter", "tick", "scan", "uptime", "config",
                    "limit",
                ],
            ) {
                score -= 28;
            }
            if point.writable {
                score -= 16;
            }
            if point.unit.is_some() {
                score += 5;
            }
            (score, point.path.clone())
        })
        .collect::<Vec<_>>();
    scored.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));

    let target = scored.len().clamp(0, 8);
    let mut selected = scored
        .into_iter()
        .take(target)
        .map(|(_, path)| path)
        .collect::<Vec<_>>();
    selected.sort();
    selected.dedup();
    selected
}

fn contains_any(haystack: &str, hints: &[&str]) -> bool {
    hints
        .iter()
        .any(|hint| haystack.contains(&hint.to_ascii_lowercase()))
}

fn overview_widget_span(point: &ScaffoldPoint, tier: Option<&str>) -> u32 {
    if tier == Some("module") {
        return 3;
    }
    match classify_overview_category(point) {
        ScaffoldOverviewCategory::SafetyAlarm => 2,
        ScaffoldOverviewCategory::CommandMode => 2,
        ScaffoldOverviewCategory::Kpi => 4,
        ScaffoldOverviewCategory::Deviation => 3,
        ScaffoldOverviewCategory::Inventory => 4,
        ScaffoldOverviewCategory::Diagnostic => 3,
    }
}

/// Extract an equipment-instance prefix from a variable name.
///
/// Looks for patterns like `pump1_speed`, `tank_001_level`, `valve2_state` –
/// i.e. a word followed by digits, then an underscore separator.  Returns
/// `None` when no recognisable equipment prefix is found.
fn infer_instance_prefix(raw_name: &str) -> Option<String> {
    let name = raw_name.to_ascii_lowercase();
    let bytes = name.as_bytes();
    // Phase 1: consume leading alphabetic chars
    let mut i = 0;
    while i < bytes.len() && bytes[i].is_ascii_lowercase() {
        i += 1;
    }
    if i == 0 {
        return None;
    }
    // Allow an optional underscore before digits (e.g. "tank_001_")
    let alpha_end = i;
    if i < bytes.len() && bytes[i] == b'_' {
        i += 1;
    }
    // Phase 2: consume digits
    let digit_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    if i == digit_start {
        return None; // no digits found
    }
    // Phase 3: must be followed by '_' or a capital letter (camelCase)
    if i >= bytes.len() {
        return None; // digits at end of name, no suffix
    }
    if bytes[i] == b'_' && i + 1 < bytes.len() {
        return Some(name[..i].to_string());
    }
    // camelCase: original name must have uppercase after digit run
    let orig_bytes = raw_name.as_bytes();
    if i < orig_bytes.len() && orig_bytes[i].is_ascii_uppercase() {
        return Some(name[..alpha_end].to_string() + &name[alpha_end..i]);
    }
    None
}

fn build_tiered_overview_sections(points: Vec<ScaffoldPoint>) -> ScaffoldOverviewResult {
    if points.is_empty() {
        return ScaffoldOverviewResult {
            sections: Vec::new(),
            equipment_groups: Vec::new(),
        };
    }

    let mut hero: Vec<ScaffoldPoint> = Vec::new();
    let mut status: Vec<ScaffoldPoint> = Vec::new();
    let mut module_groups: IndexMap<String, Vec<ScaffoldPoint>> = IndexMap::new();
    let mut detail: Vec<ScaffoldPoint> = Vec::new();

    for point in points {
        match classify_overview_category(&point) {
            ScaffoldOverviewCategory::Kpi | ScaffoldOverviewCategory::Inventory => {
                if point.type_bucket == ScaffoldTypeBucket::Numeric && hero.len() < 3 {
                    hero.push(point);
                } else {
                    detail.push(point);
                }
            }
            ScaffoldOverviewCategory::SafetyAlarm | ScaffoldOverviewCategory::CommandMode => {
                status.push(point);
            }
            ScaffoldOverviewCategory::Deviation | ScaffoldOverviewCategory::Diagnostic => {
                detail.push(point);
            }
        }
    }

    // Detect equipment instance groups from the detail bucket.
    // Points whose raw_name shares an instance prefix (e.g. "pump1_speed",
    // "pump1_pressure") get promoted to module blocks when ≥2 variables share
    // the same prefix.
    let mut remaining_detail = Vec::new();
    for point in detail {
        if let Some(prefix) = infer_instance_prefix(&point.raw_name) {
            module_groups.entry(prefix).or_default().push(point);
        } else {
            remaining_detail.push(point);
        }
    }

    // Only keep groups with 2+ variables as module blocks; demote singletons
    // back to detail.
    let mut equipment_strip_widgets: Vec<ScaffoldPoint> = Vec::new();
    let mut equipment_detail_groups: Vec<ScaffoldEquipmentGroup> = Vec::new();
    for (prefix, group) in module_groups {
        if group.len() >= 2 {
            let title = infer_label(&prefix);
            let detail_page_id = format!("equipment-{}", prefix.replace('_', "-"));
            // Pick a representative widget for the equipment strip:
            // prefer a boolean (running/on-off), else first numeric.
            let rep_idx = group
                .iter()
                .position(|p| p.type_bucket == ScaffoldTypeBucket::Bool)
                .unwrap_or(0);
            let mut rep = group[rep_idx].clone();
            rep.widget = "module".to_string();
            rep.label = title.clone();
            equipment_strip_widgets.push(rep);
            equipment_detail_groups.push(ScaffoldEquipmentGroup {
                prefix: prefix.clone(),
                title,
                detail_page_id,
                widgets: group,
            });
        } else {
            remaining_detail.extend(group);
        }
    }

    let mut sections = Vec::new();

    // Equipment strip comes FIRST (module tier)
    if !equipment_strip_widgets.is_empty() {
        sections.push(ScaffoldSection {
            title: "Equipment".to_string(),
            span: 12,
            tier: Some("module".to_string()),
            widgets: equipment_strip_widgets,
        });
    }

    if !hero.is_empty() {
        sections.push(ScaffoldSection {
            title: "Key Metrics".to_string(),
            span: 12,
            tier: Some("hero".to_string()),
            widgets: hero,
        });
    }

    if !status.is_empty() {
        sections.push(ScaffoldSection {
            title: "Status".to_string(),
            span: 12,
            tier: Some("status".to_string()),
            widgets: status,
        });
    }

    if !remaining_detail.is_empty() {
        sections.push(ScaffoldSection {
            title: "Details".to_string(),
            span: 12,
            tier: Some("detail".to_string()),
            widgets: remaining_detail,
        });
    }

    ScaffoldOverviewResult {
        sections,
        equipment_groups: equipment_detail_groups,
    }
}

fn widget_for_scaffold_type(ty: &Type, writable: bool, qualifier: SourceVarKind) -> &'static str {
    if !writable
        && matches!(qualifier, SourceVarKind::Output | SourceVarKind::Global)
        && matches!(ty, Type::Real | Type::LReal)
    {
        return "gauge";
    }
    widget_for_type(ty, writable)
}

fn widget_for_scaffold_value(value: &Value) -> &'static str {
    match value {
        Value::Real(_) | Value::LReal(_) => "gauge",
        _ => widget_for_value(value, false),
    }
}

fn enum_values_for_type(ty: &Type) -> Vec<String> {
    match ty {
        Type::Enum { values, .. } => values.iter().map(|(name, _)| name.to_string()).collect(),
        _ => Vec::new(),
    }
}

fn scaffold_type_bucket_for_type(ty: &Type) -> ScaffoldTypeBucket {
    match ty {
        Type::Bool => ScaffoldTypeBucket::Bool,
        ty if ty.is_numeric() || ty.is_bit_string() || ty.is_time() => ScaffoldTypeBucket::Numeric,
        ty if ty.is_string() || ty.is_char() => ScaffoldTypeBucket::Text,
        Type::Array { .. }
        | Type::Struct { .. }
        | Type::Union { .. }
        | Type::FunctionBlock { .. }
        | Type::Class { .. }
        | Type::Interface { .. } => ScaffoldTypeBucket::Composite,
        _ => ScaffoldTypeBucket::Other,
    }
}

fn scaffold_type_bucket_for_value(value: &Value, data_type: &str) -> ScaffoldTypeBucket {
    match value {
        Value::Bool(_) => ScaffoldTypeBucket::Bool,
        Value::SInt(_)
        | Value::Int(_)
        | Value::DInt(_)
        | Value::LInt(_)
        | Value::USInt(_)
        | Value::UInt(_)
        | Value::UDInt(_)
        | Value::ULInt(_)
        | Value::Byte(_)
        | Value::Word(_)
        | Value::DWord(_)
        | Value::LWord(_)
        | Value::Real(_)
        | Value::LReal(_)
        | Value::Time(_)
        | Value::LTime(_)
        | Value::Date(_)
        | Value::LDate(_)
        | Value::Tod(_)
        | Value::LTod(_)
        | Value::Dt(_)
        | Value::Ldt(_) => ScaffoldTypeBucket::Numeric,
        Value::String(_) | Value::WString(_) | Value::Char(_) | Value::WChar(_) => {
            ScaffoldTypeBucket::Text
        }
        Value::Array(_) | Value::Struct(_) => ScaffoldTypeBucket::Composite,
        _ if is_numeric_data_type(data_type) => ScaffoldTypeBucket::Numeric,
        _ => ScaffoldTypeBucket::Other,
    }
}

fn infer_unit_and_range(
    path: &str,
    data_type: &str,
    type_bucket: ScaffoldTypeBucket,
) -> (Option<String>, Option<f64>, Option<f64>) {
    if type_bucket != ScaffoldTypeBucket::Numeric && !is_numeric_data_type(data_type) {
        return (None, None, None);
    }

    let name = path.to_ascii_lowercase();
    if name.contains("rpm") || name.contains("speed") {
        return (Some("rpm".to_string()), Some(0.0), Some(3600.0));
    }
    if name.contains("pressure") || name.contains("bar") {
        return (Some("bar".to_string()), Some(0.0), Some(16.0));
    }
    if name.contains("temp") || name.contains("temperature") {
        return (Some("C".to_string()), Some(0.0), Some(120.0));
    }
    if name.contains("level") || name.contains("percent") || name.contains('%') {
        return (Some("%".to_string()), Some(0.0), Some(100.0));
    }
    if name.contains("flow") {
        return (Some("l/min".to_string()), Some(0.0), Some(500.0));
    }

    (None, Some(0.0), Some(100.0))
}

fn infer_icon_for_points(points: &[ScaffoldPoint]) -> String {
    for point in points {
        let name = point.path.to_ascii_lowercase();
        if name.contains("pump") || name.contains("motor") {
            return "activity".to_string();
        }
        if name.contains("valve") {
            return "sliders".to_string();
        }
        if name.contains("tank") || name.contains("level") {
            return "droplets".to_string();
        }
        if name.contains("temp") {
            return "thermometer".to_string();
        }
        if name.contains("pressure") {
            return "gauge".to_string();
        }
    }
    "activity".to_string()
}

fn infer_label(raw: &str) -> String {
    let mut normalized = String::new();
    let mut prev_was_lower = false;
    for ch in raw.chars() {
        if ch == '_' || ch == '-' || ch == '.' {
            normalized.push(' ');
            prev_was_lower = false;
            continue;
        }
        if ch.is_ascii_uppercase() && prev_was_lower {
            normalized.push(' ');
        }
        normalized.push(ch);
        prev_was_lower = ch.is_ascii_lowercase() || ch.is_ascii_digit();
    }

    normalized
        .split_whitespace()
        .map(expand_label_token)
        .collect::<Vec<_>>()
        .join(" ")
}

fn expand_label_token(token: &str) -> String {
    let lower = token.to_ascii_lowercase();
    match lower.as_str() {
        "sp" => "Setpoint".to_string(),
        "pv" => "Process Value".to_string(),
        "temp" | "tmp" => "Temperature".to_string(),
        "cmd" => "Command".to_string(),
        "rpm" => "RPM".to_string(),
        "pid" => "PID".to_string(),
        _ => {
            if lower.len() <= 3 && lower.chars().all(|ch| ch.is_ascii_alphanumeric()) {
                token.to_ascii_uppercase()
            } else {
                title_case(token)
            }
        }
    }
}

fn escape_toml_string(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', " ")
}

fn collect_source_symbol_index(sources: &[HmiSourceRef<'_>]) -> SourceSymbolIndex {
    let mut index = SourceSymbolIndex::default();
    for source in sources {
        collect_source_symbols_in_file(source.path, source.text, &mut index);
    }
    index
}

fn collect_source_symbols_in_file(path: &Path, source: &str, out: &mut SourceSymbolIndex) {
    let path_text = path.to_string_lossy().to_string();
    let mut current_program: Option<String> = None;
    let mut current_kind: Option<SourceVarKind> = None;

    for raw_line in source.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(program_name) = parse_program_header(line) {
            out.program_files
                .entry(program_name.to_ascii_uppercase())
                .or_insert_with(|| path_text.clone());
            current_program = Some(program_name);
            current_kind = None;
            continue;
        }
        let upper = line.to_ascii_uppercase();
        if upper.starts_with("END_PROGRAM") {
            current_program = None;
            current_kind = None;
            continue;
        }
        if upper.starts_with("END_VAR") {
            current_kind = None;
            continue;
        }
        if let Some(kind) = parse_source_var_block_kind(line) {
            current_kind = Some(kind);
            continue;
        }
        let Some(kind) = current_kind else {
            continue;
        };
        let names = parse_var_names(line);
        if names.is_empty() {
            continue;
        }
        for name in names {
            match (kind, current_program.as_ref()) {
                (SourceVarKind::Global | SourceVarKind::External, _) => {
                    out.globals.insert(name.to_ascii_uppercase());
                }
                (_, Some(program_name)) => {
                    out.programs_with_entries
                        .insert(program_name.to_ascii_uppercase());
                    out.program_vars.insert(
                        normalize_symbol_key(program_name.as_str(), name.as_str()),
                        kind,
                    );
                }
                _ => {}
            }
        }
    }
}

fn parse_source_var_block_kind(line: &str) -> Option<SourceVarKind> {
    let upper = line.trim().to_ascii_uppercase();
    if upper.starts_with("VAR_IN_OUT") {
        return Some(SourceVarKind::InOut);
    }
    if upper.starts_with("VAR_INPUT") {
        return Some(SourceVarKind::Input);
    }
    if upper.starts_with("VAR_OUTPUT") {
        return Some(SourceVarKind::Output);
    }
    if upper.starts_with("VAR_GLOBAL") {
        return Some(SourceVarKind::Global);
    }
    if upper.starts_with("VAR_EXTERNAL") {
        return Some(SourceVarKind::External);
    }
    if upper.starts_with("VAR_TEMP") {
        return Some(SourceVarKind::Temp);
    }
    if upper.starts_with("VAR") {
        return Some(SourceVarKind::Var);
    }
    None
}

fn parse_var_names(line: &str) -> Vec<String> {
    let mut text = line;
    if let Some(index) = text.find("//") {
        text = &text[..index];
    }
    if let Some(index) = text.find("(*") {
        text = &text[..index];
    }
    if !text.contains(':') {
        return Vec::new();
    }
    let Some(left) = text.split(':').next() else {
        return Vec::new();
    };
    left.split(',')
        .map(str::trim)
        .filter(|candidate| is_identifier(candidate))
        .map(ToString::to_string)
        .collect::<Vec<_>>()
}

fn normalize_symbol_key(program: &str, variable: &str) -> String {
    format!(
        "{}.{}",
        program.trim().to_ascii_uppercase(),
        variable.trim().to_ascii_uppercase()
    )
}

fn parse_annotations(sources: &[HmiSourceRef<'_>]) -> BTreeMap<String, HmiWidgetOverride> {
    let mut overrides = BTreeMap::new();
    for source in sources {
        parse_annotations_in_source(source.text, &mut overrides);
    }
    overrides
}

fn parse_annotations_in_source(source: &str, out: &mut BTreeMap<String, HmiWidgetOverride>) {
    let mut scope = AnnotationScope::None;
    let mut in_var_block = false;
    let mut global_var_block = false;
    let mut pending: Option<HmiWidgetOverride> = None;

    for raw_line in source.lines() {
        let line = raw_line.trim();
        let upper = line.to_ascii_uppercase();

        if let Some(program_name) = parse_program_header(line) {
            scope = AnnotationScope::Program(program_name);
            in_var_block = false;
            global_var_block = false;
            pending = None;
            continue;
        }
        if upper.starts_with("END_PROGRAM") {
            scope = AnnotationScope::None;
            in_var_block = false;
            global_var_block = false;
            pending = None;
            continue;
        }
        if upper.starts_with("VAR_GLOBAL") {
            in_var_block = true;
            global_var_block = true;
        } else if upper.starts_with("VAR") {
            in_var_block = true;
            global_var_block = false;
        } else if upper.starts_with("END_VAR") {
            in_var_block = false;
            global_var_block = false;
            pending = None;
            continue;
        }

        let inline = parse_hmi_annotation_from_line(line);
        let var_name = parse_var_name(line);

        if let Some(var_name) = var_name {
            let mut merged = pending.take().unwrap_or_default();
            if let Some(inline) = inline {
                merged.merge_from(&inline);
            }
            if merged.is_empty() {
                continue;
            }
            let key = match (&scope, global_var_block) {
                (_, true) => format!("global.{var_name}"),
                (AnnotationScope::Program(program_name), false) => {
                    format!("{program_name}.{var_name}")
                }
                _ => format!("global.{var_name}"),
            };
            out.insert(key, merged);
            continue;
        }

        if inline.is_some() && in_var_block {
            pending = inline;
        }
    }
}

fn parse_program_header(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    let keyword = parts.next()?;
    if !keyword.eq_ignore_ascii_case("PROGRAM") {
        return None;
    }
    let name = parts.next()?.trim_end_matches(';').trim();
    if name.is_empty() || !is_identifier(name) {
        return None;
    }
    Some(name.to_string())
}

fn parse_var_name(line: &str) -> Option<String> {
    let mut text = line;
    if let Some(index) = text.find("//") {
        text = &text[..index];
    }
    if let Some(index) = text.find("(*") {
        text = &text[..index];
    }
    let left = text.split(':').next()?.trim();
    if left.is_empty() {
        return None;
    }
    let candidate = left
        .split(|ch: char| ch.is_whitespace() || ch == ',')
        .find(|token| !token.is_empty())?;
    if !is_identifier(candidate) {
        return None;
    }
    Some(candidate.to_string())
}

fn parse_hmi_annotation_from_line(line: &str) -> Option<HmiWidgetOverride> {
    let lower = line.to_ascii_lowercase();
    let marker = lower.find("@hmi(")?;
    let start = marker + "@hmi(".len();
    let tail = &line[start..];
    let mut depth = 1usize;
    let mut end_index = None;
    for (idx, ch) in tail.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    end_index = Some(idx);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end_index?;
    let payload = &tail[..end];
    parse_hmi_annotation_payload(payload)
}

fn parse_hmi_annotation_payload(payload: &str) -> Option<HmiWidgetOverride> {
    let mut override_spec = HmiWidgetOverride::default();
    for part in split_csv(payload) {
        let trimmed = part.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (key, raw_value) = trimmed.split_once('=')?;
        let key = key.trim().to_ascii_lowercase();
        let raw_value = raw_value.trim();
        match key.as_str() {
            "label" => override_spec.label = parse_annotation_string(raw_value),
            "unit" => override_spec.unit = parse_annotation_string(raw_value),
            "widget" => override_spec.widget = parse_annotation_string(raw_value),
            "page" => override_spec.page = parse_annotation_string(raw_value),
            "group" => override_spec.group = parse_annotation_string(raw_value),
            "min" => override_spec.min = raw_value.parse::<f64>().ok(),
            "max" => override_spec.max = raw_value.parse::<f64>().ok(),
            "order" => override_spec.order = raw_value.parse::<i32>().ok(),
            _ => {}
        }
    }
    if override_spec.is_empty() {
        None
    } else {
        Some(override_spec)
    }
}

fn split_csv(text: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut in_quotes: Option<char> = None;
    for ch in text.chars() {
        match ch {
            '"' | '\'' => {
                if in_quotes == Some(ch) {
                    in_quotes = None;
                } else if in_quotes.is_none() {
                    in_quotes = Some(ch);
                }
                current.push(ch);
            }
            ',' if in_quotes.is_none() => {
                parts.push(current.trim().to_string());
                current.clear();
            }
            _ => current.push(ch),
        }
    }
    if !current.trim().is_empty() {
        parts.push(current.trim().to_string());
    }
    parts
}

fn parse_annotation_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    if (trimmed.starts_with('"') && trimmed.ends_with('"'))
        || (trimmed.starts_with('\'') && trimmed.ends_with('\''))
    {
        return Some(trimmed[1..trimmed.len().saturating_sub(1)].to_string());
    }
    Some(trimmed.to_string())
}

fn is_identifier(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_') {
        return false;
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn title_case(value: &str) -> String {
    value
        .split(|ch: char| ch == '_' || ch == '-' || ch.is_whitespace())
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            let Some(first) = chars.next() else {
                return String::new();
            };
            let mut title = String::new();
            title.push(first.to_ascii_uppercase());
            title.push_str(&chars.as_str().to_ascii_lowercase());
            title
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_hex_color(value: &str) -> bool {
    let bytes = value.as_bytes();
    if !(bytes.len() == 7 || bytes.len() == 4) {
        return false;
    }
    if bytes.first().copied() != Some(b'#') {
        return false;
    }
    bytes[1..].iter().all(|byte| byte.is_ascii_hexdigit())
}

#[derive(Debug, Clone)]
enum AnnotationScope {
    Program(String),
    None,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::harness::{CompileSession, SourceFile as HarnessSourceFile, TestHarness};
    use serde_json::json;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(prefix: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
        std::fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn metadata_for_source(source: &str) -> RuntimeMetadata {
        let harness = TestHarness::from_source(source).expect("build harness");
        harness.runtime().metadata_snapshot()
    }

    fn scaffold_from_sources(
        root: &Path,
        style: &str,
        sources: &[(&str, &str)],
    ) -> HmiScaffoldSummary {
        let compile_sources = sources
            .iter()
            .map(|(path, text)| HarnessSourceFile::with_path(*path, *text))
            .collect::<Vec<_>>();
        let runtime = CompileSession::from_sources(compile_sources)
            .build_runtime()
            .expect("build runtime");
        let metadata = runtime.metadata_snapshot();
        let snapshot = crate::debug::DebugSnapshot {
            storage: runtime.storage().clone(),
            now: runtime.current_time(),
        };
        let loaded = sources
            .iter()
            .map(|(path, text)| (PathBuf::from(path), (*text).to_string()))
            .collect::<Vec<_>>();
        let refs = loaded
            .iter()
            .map(|(path, text)| HmiSourceRef {
                path: path.as_path(),
                text: text.as_str(),
            })
            .collect::<Vec<_>>();
        scaffold_hmi_dir_with_sources(root, &metadata, Some(&snapshot), &refs, style)
            .expect("scaffold hmi")
    }

    fn scaffold_from_sources_with_mode(
        root: &Path,
        style: &str,
        sources: &[(&str, &str)],
        mode: HmiScaffoldMode,
        force: bool,
    ) -> HmiScaffoldSummary {
        let compile_sources = sources
            .iter()
            .map(|(path, text)| HarnessSourceFile::with_path(*path, *text))
            .collect::<Vec<_>>();
        let runtime = CompileSession::from_sources(compile_sources)
            .build_runtime()
            .expect("build runtime");
        let metadata = runtime.metadata_snapshot();
        let snapshot = crate::debug::DebugSnapshot {
            storage: runtime.storage().clone(),
            now: runtime.current_time(),
        };
        let loaded = sources
            .iter()
            .map(|(path, text)| (PathBuf::from(path), (*text).to_string()))
            .collect::<Vec<_>>();
        let refs = loaded
            .iter()
            .map(|(path, text)| HmiSourceRef {
                path: path.as_path(),
                text: text.as_str(),
            })
            .collect::<Vec<_>>();
        scaffold_hmi_dir_with_sources_mode(
            root,
            &metadata,
            Some(&snapshot),
            &refs,
            style,
            mode,
            force,
        )
        .expect("scaffold hmi")
    }

    #[test]
    fn scaffold_includes_external_symbols_and_excludes_internals() {
        let root = temp_dir("trust-runtime-hmi-scaffold-scope");
        let source = r#"
PROGRAM Main
VAR_INPUT
    speed_sp : REAL := 1200.0;
END_VAR
VAR_OUTPUT
    speed_pv : REAL := 1200.0;
END_VAR
VAR
    internal_counter : DINT := 0;
END_VAR
END_PROGRAM
"#;
        let _summary = scaffold_from_sources(&root, "industrial", &[("sources/main.st", source)]);
        let overview =
            std::fs::read_to_string(root.join("hmi/overview.toml")).expect("read overview");
        assert!(overview.contains("bind = \"Main.speed_sp\""));
        assert!(overview.contains("bind = \"Main.speed_pv\""));
        assert!(!overview.contains("internal_counter"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_local_only_program_uses_inferred_interface_fallback() {
        let root = temp_dir("trust-runtime-hmi-scaffold-local-fallback");
        let source = r#"
PROGRAM Main
VAR
    speed_pv : REAL := 1200.0;
    running : BOOL := FALSE;
END_VAR
END_PROGRAM
"#;
        let _summary = scaffold_from_sources(&root, "industrial", &[("sources/main.st", source)]);
        let overview =
            std::fs::read_to_string(root.join("hmi/overview.toml")).expect("read overview");
        assert!(overview.contains("bind = \"Main.speed_pv\""));
        assert!(overview.contains("bind = \"Main.running\""));
        assert!(overview.contains("inferred_interface = true"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_widget_mapping_respects_type_and_writability() {
        let root = temp_dir("trust-runtime-hmi-scaffold-widget-map");
        let source = r#"
PROGRAM Main
VAR_INPUT
    run_cmd : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    running : BOOL := FALSE;
    pressure_bar : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _summary = scaffold_from_sources(&root, "industrial", &[("sources/main.st", source)]);
        let overview =
            std::fs::read_to_string(root.join("hmi/overview.toml")).expect("read overview");
        assert!(overview.contains("type = \"toggle\"\nbind = \"Main.run_cmd\""));
        assert!(overview.contains("type = \"indicator\"\nbind = \"Main.running\""));
        assert!(overview.contains("type = \"gauge\"\nbind = \"Main.pressure_bar\""));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_output_is_deterministic_for_same_input() {
        let source = r#"
PROGRAM Main
VAR_INPUT
    speed_sp : REAL := 1200.0;
END_VAR
VAR_OUTPUT
    speed_pv : REAL := 1000.0;
    running : BOOL := FALSE;
END_VAR
END_PROGRAM
"#;
        let root_a = temp_dir("trust-runtime-hmi-scaffold-deterministic-a");
        let root_b = temp_dir("trust-runtime-hmi-scaffold-deterministic-b");
        let summary_a = scaffold_from_sources(&root_a, "classic", &[("sources/main.st", source)]);
        let summary_b = scaffold_from_sources(&root_b, "classic", &[("sources/main.st", source)]);
        assert_eq!(summary_a, summary_b);

        let overview_a =
            std::fs::read_to_string(root_a.join("hmi/overview.toml")).expect("read overview a");
        let overview_b =
            std::fs::read_to_string(root_b.join("hmi/overview.toml")).expect("read overview b");
        assert_eq!(overview_a, overview_b);

        let config_a =
            std::fs::read_to_string(root_a.join("hmi/_config.toml")).expect("read config a");
        let config_b =
            std::fs::read_to_string(root_b.join("hmi/_config.toml")).expect("read config b");
        assert_eq!(config_a, config_b);

        std::fs::remove_dir_all(root_a).ok();
        std::fs::remove_dir_all(root_b).ok();
    }

    #[test]
    fn scaffold_overview_enforces_budget_and_config_version() {
        let root = temp_dir("trust-runtime-hmi-scaffold-overview-budget");
        let source = r#"
PROGRAM Main
VAR_INPUT
    start_cmd : BOOL := FALSE;
    stop_cmd : BOOL := FALSE;
    flow_setpoint : REAL := 50.0;
    pressure_setpoint : REAL := 4.0;
END_VAR
VAR_OUTPUT
    alarm_active : BOOL := FALSE;
    flow_main : REAL := 0.0;
    pressure_bar : REAL := 0.0;
    tank_feed_level : REAL := 0.0;
    tank_product_level : REAL := 0.0;
    flow_deviation : REAL := 0.0;
    scan_tick : DINT := 0;
    energy_kwh : REAL := 0.0;
    motor_speed_rpm : REAL := 0.0;
    ambient_temperature : REAL := 0.0;
    line_current : REAL := 0.0;
    valve_position_pct : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _summary = scaffold_from_sources(&root, "industrial", &[("sources/main.st", source)]);
        let overview =
            std::fs::read_to_string(root.join("hmi/overview.toml")).expect("read overview");
        let count = overview.matches("[[section.widget]]").count();
        assert!(
            count <= 10,
            "overview widget count exceeded budget: {count} > 10"
        );
        assert!(overview.contains("bind = \"Main.alarm_active\""));
        let config =
            std::fs::read_to_string(root.join("hmi/_config.toml")).expect("read _config.toml");
        assert!(config.contains("version = 1"));
        assert!(config.contains("inferred = true"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_groups_repeated_instance_prefixes_into_separate_sections() {
        let root = temp_dir("trust-runtime-hmi-scaffold-instance-grouping");
        let source = r#"
PROGRAM Main
VAR_OUTPUT
    pump1_speed : REAL := 0.0;
    pump1_pressure : REAL := 0.0;
    pump2_speed : REAL := 0.0;
    pump2_pressure : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _summary = scaffold_from_sources(&root, "industrial", &[("sources/main.st", source)]);
        let overview =
            std::fs::read_to_string(root.join("hmi/overview.toml")).expect("read overview");
        // Tiered layout puts numeric KPIs into a "Key Metrics" hero section
        assert!(overview.contains("title = \"Key Metrics\""));
        assert!(overview.contains("tier = \"hero\""));
        assert!(overview.contains("bind = \"Main.pump1_speed\""));
        assert!(overview.contains("bind = \"Main.pump2_speed\""));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_generates_control_and_process_pages() {
        let root = temp_dir("trust-runtime-hmi-scaffold-required-pages");
        let source = r#"
PROGRAM Main
VAR_INPUT
    start_cmd : BOOL := FALSE;
    flow_setpoint_m3h : REAL := 40.0;
END_VAR
VAR_OUTPUT
    running : BOOL := FALSE;
    flow_m3h : REAL := 0.0;
    pressure_bar : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let summary = scaffold_from_sources(&root, "industrial", &[("sources/main.st", source)]);
        let file_names = summary
            .files
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<Vec<_>>();
        assert!(file_names.contains(&"process.toml"));
        assert!(file_names.contains(&"process.auto.svg"));
        assert!(file_names.contains(&"control.toml"));
        assert!(root.join("hmi/process.toml").is_file());
        assert!(root.join("hmi/process.auto.svg").is_file());
        assert!(root.join("hmi/control.toml").is_file());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_process_auto_svg_uses_grid_aligned_instrument_templates() {
        let root = temp_dir("trust-runtime-hmi-scaffold-process-grid");
        let source = r#"
PROGRAM Main
VAR_OUTPUT
    running : BOOL := FALSE;
    flow_m3h : REAL := 0.0;
    pressure_bar : REAL := 0.0;
    feed_level_pct : REAL := 0.0;
    product_level_pct : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _summary = scaffold_from_sources(&root, "classic", &[("sources/main.st", source)]);
        let svg =
            std::fs::read_to_string(root.join("hmi/process.auto.svg")).expect("read process svg");
        assert!(svg.contains("id=\"pid-layout-guides\""));
        assert!(svg.contains("<g id=\"pid-fit-001\" transform=\"translate(500,240)\">"));
        assert!(svg.contains("<g id=\"pid-pt-001\" transform=\"translate(740,240)\">"));
        assert!(svg.contains("<text id=\"pid-flow-value\" class=\"pid-value\" x=\"80\" y=\"-4\""));
        assert!(
            svg.contains("<text id=\"pid-pressure-value\" class=\"pid-value\" x=\"80\" y=\"-4\"")
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_process_toml_binds_level_fill_y_and_height() {
        let root = temp_dir("trust-runtime-hmi-scaffold-process-level-scale");
        let source = r#"
PROGRAM Main
VAR_OUTPUT
    feed_level_pct : REAL := 0.0;
    product_level_pct : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _summary = scaffold_from_sources(&root, "classic", &[("sources/main.st", source)]);
        let process =
            std::fs::read_to_string(root.join("hmi/process.toml")).expect("read process page");
        assert!(process.contains(
            "selector = \"#pid-feed-level-fill\"\nattribute = \"y\"\nsource = \"Main.feed_level_pct\"\nscale = { min = 0, max = 100, output_min = 480, output_max = 200 }"
        ));
        assert!(process.contains(
            "selector = \"#pid-feed-level-fill\"\nattribute = \"height\"\nsource = \"Main.feed_level_pct\"\nscale = { min = 0, max = 100, output_min = 0, output_max = 280 }"
        ));
        assert!(process.contains(
            "selector = \"#pid-product-level-fill\"\nattribute = \"y\"\nsource = \"Main.product_level_pct\"\nscale = { min = 0, max = 100, output_min = 480, output_max = 200 }"
        ));
        assert!(process.contains(
            "selector = \"#pid-product-level-fill\"\nattribute = \"height\"\nsource = \"Main.product_level_pct\"\nscale = { min = 0, max = 100, output_min = 0, output_max = 280 }"
        ));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_update_preserves_existing_page_and_fills_missing_files() {
        let root = temp_dir("trust-runtime-hmi-scaffold-update");
        let source = r#"
PROGRAM Main
VAR_INPUT
    start_cmd : BOOL := FALSE;
END_VAR
VAR_OUTPUT
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _initial = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Reset,
            false,
        );
        std::fs::write(
            root.join("hmi/overview.toml"),
            "title = \"Overview\"\n[[section]]\ntitle = \"Custom\"\nspan = 12\n",
        )
        .expect("overwrite overview");
        std::fs::remove_file(root.join("hmi/control.toml")).expect("remove control page");
        std::fs::remove_file(root.join("hmi/process.toml")).expect("remove process page");

        let summary = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Update,
            false,
        );
        let preserved_overview =
            std::fs::read_to_string(root.join("hmi/overview.toml")).expect("read overview");
        assert!(preserved_overview.contains("title = \"Custom\""));
        assert!(root.join("hmi/control.toml").is_file());
        assert!(root.join("hmi/process.toml").is_file());
        assert!(summary
            .files
            .iter()
            .any(|entry| entry.path == "overview.toml"
                && (entry.detail == "preserved existing"
                    || entry.detail == "merged missing scaffold signals")));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_update_skips_default_process_when_custom_process_page_exists() {
        let root = temp_dir("trust-runtime-hmi-scaffold-update-custom-process");
        let source = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _initial = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Reset,
            false,
        );
        std::fs::write(
            root.join("hmi/plant.toml"),
            "title = \"Plant\"\nkind = \"process\"\nsvg = \"plant.svg\"\norder = 20\n",
        )
        .expect("write custom process page");
        std::fs::remove_file(root.join("hmi/process.toml")).expect("remove default process page");
        std::fs::remove_file(root.join("hmi/process.auto.svg"))
            .expect("remove default process svg");

        let summary = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Update,
            false,
        );

        assert!(
            !root.join("hmi/process.toml").is_file(),
            "update should not recreate default process.toml when custom process page exists"
        );
        assert!(
            !root.join("hmi/process.auto.svg").is_file(),
            "update should not recreate default process.auto.svg when custom process page exists"
        );
        assert!(summary.files.iter().any(|entry| {
            entry.path == "process.toml" && entry.detail == "skipped (custom process page exists)"
        }));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_update_skips_default_control_when_no_writable_points() {
        let root = temp_dir("trust-runtime-hmi-scaffold-update-skip-control");
        let source = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _initial = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Reset,
            false,
        );
        std::fs::remove_file(root.join("hmi/control.toml")).expect("remove control page");

        let summary = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Update,
            false,
        );

        assert!(
            !root.join("hmi/control.toml").is_file(),
            "update should not recreate control.toml when no writable points exist"
        );
        assert!(summary.files.iter().any(|entry| {
            entry.path == "control.toml"
                && entry.detail == "skipped (no writable points discovered)"
        }));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_update_merges_missing_signals_without_overwriting_custom_widgets() {
        let root = temp_dir("trust-runtime-hmi-scaffold-update-merge-signals");
        let source_a = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let source_b = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL := 0.0;
    pressure_bar : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _initial = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source_a)],
            HmiScaffoldMode::Reset,
            false,
        );
        std::fs::write(
            root.join("hmi/overview.toml"),
            r#"
title = "Overview"
order = 0
kind = "dashboard"

[[section]]
title = "Custom"
span = 12

[[section.widget]]
type = "gauge"
bind = "Main.speed"
label = "Speed Custom"
"#,
        )
        .expect("overwrite overview");

        let summary = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source_b)],
            HmiScaffoldMode::Update,
            false,
        );
        let overview =
            std::fs::read_to_string(root.join("hmi/overview.toml")).expect("read overview");
        assert!(overview.contains("label = \"Speed Custom\""));
        assert!(overview.contains("bind = \"Main.pressure_bar\""));
        assert!(summary.files.iter().any(|entry| {
            entry.path == "overview.toml" && entry.detail == "merged missing scaffold signals"
        }));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_init_fails_when_hmi_dir_exists_without_force() {
        let root = temp_dir("trust-runtime-hmi-scaffold-init-guard");
        let source = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _initial = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Reset,
            false,
        );
        let compile_sources = [HarnessSourceFile::with_path("sources/main.st", source)];
        let runtime = CompileSession::from_sources(compile_sources.to_vec())
            .build_runtime()
            .expect("build runtime");
        let metadata = runtime.metadata_snapshot();
        let snapshot = crate::debug::DebugSnapshot {
            storage: runtime.storage().clone(),
            now: runtime.current_time(),
        };
        let refs = [HmiSourceRef {
            path: Path::new("sources/main.st"),
            text: source,
        }];
        let err = scaffold_hmi_dir_with_sources_mode(
            &root,
            &metadata,
            Some(&snapshot),
            &refs,
            "industrial",
            HmiScaffoldMode::Init,
            false,
        )
        .expect_err("init should fail when hmi exists without force");
        assert!(err.to_string().contains("hmi directory already exists"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn scaffold_reset_creates_backup_snapshot() {
        let root = temp_dir("trust-runtime-hmi-scaffold-reset-backup");
        let source = r#"
PROGRAM Main
VAR_OUTPUT
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let _initial = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Reset,
            false,
        );
        std::fs::write(root.join("hmi/custom.txt"), "keep me").expect("write custom file");
        let summary = scaffold_from_sources_with_mode(
            &root,
            "industrial",
            &[("sources/main.st", source)],
            HmiScaffoldMode::Reset,
            false,
        );
        let backup_entry = summary
            .files
            .iter()
            .find(|entry| entry.detail.contains("backup snapshot"))
            .expect("backup entry present");
        let backup_path = root.join(&backup_entry.path);
        assert!(backup_path.is_dir());
        assert!(backup_path.join("custom.txt").is_file());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn widget_mapping_covers_required_type_buckets() {
        assert_eq!(widget_for_type(&Type::Bool, false), "indicator");
        assert_eq!(widget_for_type(&Type::Real, false), "value");
        assert_eq!(widget_for_type(&Type::Real, true), "slider");
        assert_eq!(
            widget_for_type(
                &Type::Enum {
                    name: SmolStr::new("MODE"),
                    base: trust_hir::TypeId::INT,
                    values: vec![(SmolStr::new("AUTO"), 1)],
                },
                false,
            ),
            "readout"
        );
        assert_eq!(
            widget_for_type(&Type::String { max_len: None }, false),
            "text"
        );
        assert_eq!(
            widget_for_type(
                &Type::Array {
                    element: trust_hir::TypeId::INT,
                    dimensions: vec![(1, 4)],
                },
                false,
            ),
            "table"
        );
        assert_eq!(
            widget_for_type(
                &Type::Struct {
                    name: SmolStr::new("POINT"),
                    fields: Vec::new(),
                },
                false,
            ),
            "tree"
        );
    }

    #[test]
    fn annotation_parser_handles_valid_invalid_and_missing_fields() {
        let valid = parse_hmi_annotation_payload(
            r#"label="Motor Speed", unit="rpm", min=0, max=100, widget="gauge", page="ops", group="Drive", order=2"#,
        )
        .expect("valid annotation");
        assert_eq!(valid.label.as_deref(), Some("Motor Speed"));
        assert_eq!(valid.unit.as_deref(), Some("rpm"));
        assert_eq!(valid.widget.as_deref(), Some("gauge"));
        assert_eq!(valid.page.as_deref(), Some("ops"));
        assert_eq!(valid.group.as_deref(), Some("Drive"));
        assert_eq!(valid.order, Some(2));
        assert_eq!(valid.min, Some(0.0));
        assert_eq!(valid.max, Some(100.0));

        let invalid = parse_hmi_annotation_payload(r#"label"#);
        assert!(invalid.is_none(), "invalid annotation should be rejected");

        let missing = parse_hmi_annotation_payload(" ");
        assert!(missing.is_none(), "empty annotation should be ignored");
    }

    #[test]
    fn schema_merge_applies_defaults_annotations_and_file_overrides() {
        let root = temp_dir("trust-runtime-hmi-merge");
        write_file(
            &root.join("hmi.toml"),
            r##"
[theme]
style = "industrial"
accent = "#ff5500"

[[pages]]
id = "ops"
title = "Operations"
order = 1

[widgets."Main.speed"]
label = "Speed (Override)"
widget = "slider"
page = "ops"
group = "Drive"
min = 5
max = 95
"##,
        );

        let source = r#"
PROGRAM Main
VAR
    // @hmi(label="Speed (Annotation)", unit="rpm", min=0, max=100, widget="gauge")
    speed : REAL := 42.5;
END_VAR
END_PROGRAM
"#;
        let metadata = metadata_for_source(source);
        let source_path = root.join("sources/main.st");
        let source_refs = [HmiSourceRef {
            path: &source_path,
            text: source,
        }];
        let customization = load_customization(Some(&root), &source_refs);
        let schema = build_schema("RESOURCE", &metadata, None, true, Some(&customization));

        let speed = schema
            .widgets
            .iter()
            .find(|widget| widget.path == "Main.speed")
            .expect("speed widget");
        assert_eq!(speed.label, "Speed (Override)");
        assert_eq!(speed.widget, "slider");
        assert_eq!(speed.unit.as_deref(), Some("rpm"));
        assert_eq!(speed.page, "ops");
        assert_eq!(speed.group, "Drive");
        assert_eq!(speed.min, Some(5.0));
        assert_eq!(speed.max, Some(95.0));

        assert_eq!(schema.theme.style, "industrial");
        assert_eq!(schema.theme.accent, "#ff5500");
        assert!(schema.pages.iter().any(|page| page.id == "ops"));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_dir_loader_discovers_and_sorts_pages() {
        let root = temp_dir("trust-runtime-hmi-dir-load");
        write_file(
            &root.join("hmi/_config.toml"),
            r##"
[theme]
style = "mint"
accent = "#14b8a6"

[write]
enabled = true
allow = ["Main.speed"]
"##,
        );
        write_file(
            &root.join("hmi/beta.toml"),
            r#"
title = "Beta"
kind = "dashboard"

[[section]]
title = "B"
span = 6

[[section.widget]]
type = "value"
bind = "Main.speed"
"#,
        );
        write_file(
            &root.join("hmi/alpha.toml"),
            r#"
title = "Alpha"
order = 1
kind = "dashboard"

[[section]]
title = "A"
span = 6

[[section.widget]]
type = "indicator"
bind = "Main.run"
"#,
        );

        let descriptor = load_hmi_dir(&root).expect("load hmi dir");
        assert_eq!(descriptor.pages.len(), 2);
        assert_eq!(descriptor.pages[0].id, "alpha");
        assert_eq!(descriptor.pages[1].id, "beta");
        assert_eq!(descriptor.config.theme.style.as_deref(), Some("mint"));
        assert_eq!(descriptor.config.write.enabled, Some(true));
        assert_eq!(
            descriptor.config.write.allow,
            vec!["Main.speed".to_string()]
        );
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_dir_loader_returns_none_for_invalid_toml() {
        let root = temp_dir("trust-runtime-hmi-dir-invalid");
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"
[[section]]
title = "Bad"
span = "wide"
"#,
        );
        assert!(load_hmi_dir(&root).is_none());
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_dir_loader_promotes_process_auto_svg_to_custom_asset() {
        let root = temp_dir("trust-runtime-hmi-dir-process-promotion");
        write_file(
            &root.join("hmi/process.toml"),
            r#"
title = "Process"
kind = "process"
svg = "process.auto.svg"
"#,
        );
        write_file(
            &root.join("hmi/process.auto.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
        );
        write_file(
            &root.join("hmi/plant.svg"),
            "<svg xmlns=\"http://www.w3.org/2000/svg\"></svg>",
        );
        let descriptor = load_hmi_dir(&root).expect("load descriptor");
        let process = descriptor
            .pages
            .iter()
            .find(|page| page.id == "process")
            .expect("process page");
        assert_eq!(process.svg.as_deref(), Some("plant.svg"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn load_customization_prefers_hmi_dir_over_legacy_toml() {
        let root = temp_dir("trust-runtime-hmi-dir-priority");
        write_file(
            &root.join("hmi.toml"),
            r##"
[theme]
style = "industrial"
accent = "#ff5500"

[widgets."Main.speed"]
label = "Legacy Speed"
"##,
        );
        write_file(
            &root.join("hmi/_config.toml"),
            r##"
[theme]
style = "mint"
accent = "#14b8a6"
"##,
        );
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Process"
span = 12

[[section.widget]]
type = "gauge"
bind = "Main.speed"
label = "Dir Speed"
"#,
        );

        let source = r#"
PROGRAM Main
VAR
    speed : REAL := 42.0;
END_VAR
END_PROGRAM
"#;
        let metadata = metadata_for_source(source);
        let source_path = root.join("src/main.st");
        let source_refs = [HmiSourceRef {
            path: &source_path,
            text: source,
        }];
        let customization = load_customization(Some(&root), &source_refs);
        let schema = build_schema("RESOURCE", &metadata, None, true, Some(&customization));
        let speed = schema
            .widgets
            .iter()
            .find(|widget| widget.path == "Main.speed")
            .expect("speed widget");
        assert_eq!(schema.theme.style, "mint");
        assert_eq!(speed.label, "Dir Speed");
        assert_eq!(speed.widget, "gauge");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn load_customization_uses_legacy_toml_when_hmi_dir_missing() {
        let root = temp_dir("trust-runtime-hmi-legacy-fallback");
        write_file(
            &root.join("hmi.toml"),
            r##"
[theme]
style = "industrial"
accent = "#ff5500"

[widgets."Main.speed"]
label = "Legacy Speed"
widget = "slider"
page = "ops"
group = "Legacy"
"##,
        );

        let source = r#"
PROGRAM Main
VAR
    speed : REAL := 10.0;
END_VAR
END_PROGRAM
"#;
        let metadata = metadata_for_source(source);
        let source_path = root.join("src/main.st");
        let source_refs = [HmiSourceRef {
            path: &source_path,
            text: source,
        }];
        let customization = load_customization(Some(&root), &source_refs);
        assert!(customization.dir_descriptor().is_none());

        let schema = build_schema("RESOURCE", &metadata, None, true, Some(&customization));
        let speed = schema
            .widgets
            .iter()
            .find(|widget| widget.path == "Main.speed")
            .expect("speed widget");
        assert_eq!(schema.theme.style, "industrial");
        assert_eq!(speed.label, "Legacy Speed");
        assert_eq!(speed.widget, "slider");
        assert_eq!(speed.page, "ops");
        assert_eq!(speed.group, "Legacy");

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_dir_schema_snapshot_includes_rich_metadata() {
        let root = temp_dir("trust-runtime-hmi-schema-snapshot");
        write_file(
            &root.join("hmi/_config.toml"),
            r##"
[theme]
style = "mint"
accent = "#14b8a6"
"##,
        );
        write_file(
            &root.join("hmi/overview.toml"),
            r##"
title = "Overview"
icon = "activity"
order = 1
kind = "dashboard"

[[section]]
title = "Drive"
span = 8

[[section.widget]]
type = "gauge"
bind = "Main.speed"
label = "Speed"
unit = "rpm"
span = 6
on_color = "#22c55e"
off_color = "#1f2937"

[[section.widget.zones]]
from = 50
to = 100
color = "#ef4444"

[[section.widget.zones]]
from = 0
to = 50
color = "#22c55e"
"##,
        );

        let source = r#"
PROGRAM Main
VAR
    speed : REAL := 25.0;
END_VAR
END_PROGRAM
"#;
        let metadata = metadata_for_source(source);
        let source_path = root.join("src/main.st");
        let source_refs = [HmiSourceRef {
            path: &source_path,
            text: source,
        }];
        let customization = load_customization(Some(&root), &source_refs);
        let schema = build_schema("RESOURCE", &metadata, None, true, Some(&customization));
        let widget_id = "resource/RESOURCE/program/Main/field/speed";

        let overview_page = schema
            .pages
            .iter()
            .find(|page| page.id == "overview")
            .expect("overview page");
        assert_eq!(
            serde_json::to_value(overview_page).expect("serialize overview page"),
            json!({
                "id": "overview",
                "title": "Overview",
                "order": 1,
                "kind": "dashboard",
                "icon": "activity",
                "duration_ms": null,
                "sections": [
                    {
                        "title": "Drive",
                        "span": 8,
                        "widget_ids": [widget_id]
                    }
                ]
            })
        );

        let speed = schema
            .widgets
            .iter()
            .find(|widget| widget.path == "Main.speed")
            .expect("speed widget");
        assert_eq!(
            serde_json::to_value(speed).expect("serialize speed widget"),
            json!({
                "id": widget_id,
                "path": "Main.speed",
                "label": "Speed",
                "data_type": "REAL",
                "access": "read",
                "writable": false,
                "widget": "gauge",
                "source": "program:Main",
                "page": "overview",
                "group": "Drive",
                "order": 0,
                "zones": [
                    { "from": 0.0, "to": 50.0, "color": "#22c55e" },
                    { "from": 50.0, "to": 100.0, "color": "#ef4444" }
                ],
                "on_color": "#22c55e",
                "off_color": "#1f2937",
                "section_title": "Drive",
                "widget_span": 6,
                "unit": "rpm",
                "min": null,
                "max": null
            })
        );

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn hmi_dir_alarm_thresholds_map_to_widget_limits() {
        let root = temp_dir("trust-runtime-hmi-dir-alarms");
        write_file(
            &root.join("hmi/_config.toml"),
            r#"
[[alarm]]
bind = "Main.speed"
high = 120.0
low = 10.0
label = "Speed Alarm"
"#,
        );
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Process"
span = 12

[[section.widget]]
type = "value"
bind = "Main.speed"
"#,
        );
        let source = r#"
PROGRAM Main
VAR
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let metadata = metadata_for_source(source);
        let source_path = root.join("src/main.st");
        let source_refs = [HmiSourceRef {
            path: &source_path,
            text: source,
        }];
        let customization = load_customization(Some(&root), &source_refs);
        let schema = build_schema("RESOURCE", &metadata, None, true, Some(&customization));
        let speed = schema
            .widgets
            .iter()
            .find(|widget| widget.path == "Main.speed")
            .expect("speed widget");
        assert_eq!(speed.min, Some(10.0));
        assert_eq!(speed.max, Some(120.0));
        assert_eq!(speed.label, "Speed Alarm");
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn validate_hmi_bindings_reports_unknown_paths_widgets_and_mismatches() {
        let root = temp_dir("trust-runtime-hmi-dir-validate");
        write_file(
            &root.join("hmi/overview.toml"),
            r#"
title = "Overview"

[[section]]
title = "Main"
span = 12

[[section.widget]]
type = "gauge"
bind = "Main.run"

[[section.widget]]
type = "rocket"
bind = "Main.speed"

[[section.widget]]
type = "value"
bind = "Main.unknown"
"#,
        );
        let descriptor = load_hmi_dir(&root).expect("descriptor");
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := FALSE;
    speed : REAL := 0.0;
END_VAR
END_PROGRAM
"#;
        let metadata = metadata_for_source(source);
        let diagnostics = validate_hmi_bindings("RESOURCE", &metadata, None, &descriptor);
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == HMI_DIAG_TYPE_MISMATCH));
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == HMI_DIAG_UNKNOWN_WIDGET));
        assert!(diagnostics
            .iter()
            .any(|diag| diag.code == HMI_DIAG_UNKNOWN_BIND));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn layout_overrides_keep_widget_ids_stable() {
        let root = temp_dir("trust-runtime-hmi-layout-stable");
        write_file(
            &root.join("hmi.toml"),
            r#"
[[pages]]
id = "controls"

[widgets."Main.run"]
page = "controls"
group = "Commands"
order = 10
"#,
        );

        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let metadata = metadata_for_source(source);
        let source_path = root.join("sources/main.st");
        let source_refs = [HmiSourceRef {
            path: &source_path,
            text: source,
        }];
        let customization = load_customization(Some(&root), &source_refs);

        let baseline = build_schema("RESOURCE", &metadata, None, true, None);
        let customized = build_schema("RESOURCE", &metadata, None, true, Some(&customization));

        let baseline_map = baseline
            .widgets
            .iter()
            .map(|widget| (widget.path.clone(), widget.id.clone()))
            .collect::<BTreeMap<_, _>>();
        let customized_map = customized
            .widgets
            .iter()
            .map(|widget| (widget.path.clone(), widget.id.clone()))
            .collect::<BTreeMap<_, _>>();

        assert_eq!(baseline_map, customized_map);
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn theme_snapshot_uses_default_fallbacks() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let metadata = metadata_for_source(source);
        let schema = build_schema("RESOURCE", &metadata, None, true, None);
        let theme = serde_json::to_value(&schema.theme).expect("serialize theme");
        assert_eq!(
            theme,
            json!({
                "style": "classic",
                "accent": "#0f766e",
                "background": "#f3f5f8",
                "surface": "#ffffff",
                "text": "#142133"
            })
        );
    }

    #[test]
    fn write_customization_parses_enabled_and_allowlist() {
        let root = temp_dir("trust-runtime-hmi-write-config");
        write_file(
            &root.join("hmi.toml"),
            r#"
[write]
enabled = true
allow = [" resource/RESOURCE/program/Main/field/run ", "", "Main.run"]
"#,
        );
        let source_refs: [HmiSourceRef<'_>; 0] = [];
        let customization = load_customization(Some(&root), &source_refs);
        assert!(customization.write_enabled());
        assert_eq!(customization.write_allowlist().len(), 2);
        assert!(customization.write_target_allowed("resource/RESOURCE/program/Main/field/run"));
        assert!(customization.write_target_allowed("Main.run"));
        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn resolve_write_point_supports_id_and_path_matches() {
        let source = r#"
PROGRAM Main
VAR
    run : BOOL := TRUE;
END_VAR
END_PROGRAM
"#;
        let harness = TestHarness::from_source(source).expect("build harness");
        let metadata = harness.runtime().metadata_snapshot();
        let snapshot = crate::debug::DebugSnapshot {
            storage: harness.runtime().storage().clone(),
            now: harness.runtime().current_time(),
        };

        let by_id = resolve_write_point(
            "RESOURCE",
            &metadata,
            Some(&snapshot),
            "resource/RESOURCE/program/Main/field/run",
        )
        .expect("resolve id");
        assert_eq!(by_id.path, "Main.run");
        assert_eq!(
            resolve_write_value_template(&by_id, &snapshot),
            Some(Value::Bool(true))
        );

        let by_path = resolve_write_point("RESOURCE", &metadata, Some(&snapshot), "Main.run")
            .expect("resolve path");
        assert_eq!(by_path.id, "resource/RESOURCE/program/Main/field/run");
    }

    fn synthetic_schema(min: Option<f64>, max: Option<f64>) -> HmiSchemaResult {
        synthetic_schema_with_deadband(min, max, None)
    }

    fn synthetic_schema_with_deadband(
        min: Option<f64>,
        max: Option<f64>,
        deadband: Option<f64>,
    ) -> HmiSchemaResult {
        HmiSchemaResult {
            version: HMI_SCHEMA_VERSION,
            schema_revision: 0,
            mode: "read_only",
            read_only: true,
            resource: "RESOURCE".to_string(),
            generated_at_ms: 0,
            descriptor_error: None,
            theme: resolve_theme(None),
            responsive: resolve_responsive(None),
            export: resolve_export(None),
            pages: vec![HmiPageSchema {
                id: DEFAULT_PAGE_ID.to_string(),
                title: "Overview".to_string(),
                order: 0,
                kind: "dashboard".to_string(),
                icon: None,
                duration_ms: None,
                svg: None,
                hidden: false,
                signals: Vec::new(),
                sections: Vec::new(),
                bindings: Vec::new(),
            }],
            widgets: vec![HmiWidgetSchema {
                id: "resource/RESOURCE/program/Main/field/speed".to_string(),
                path: "Main.speed".to_string(),
                label: "Speed".to_string(),
                data_type: "REAL".to_string(),
                access: "read",
                writable: false,
                widget: "value".to_string(),
                source: "program:Main".to_string(),
                page: DEFAULT_PAGE_ID.to_string(),
                group: DEFAULT_GROUP_NAME.to_string(),
                order: 0,
                zones: Vec::new(),
                on_color: None,
                off_color: None,
                section_title: None,
                widget_span: None,
                alarm_deadband: deadband,
                inferred_interface: false,
                detail_page: None,
                unit: Some("rpm".to_string()),
                min,
                max,
            }],
        }
    }

    fn synthetic_values(value: f64, ts_ms: u128) -> HmiValuesResult {
        let mut values = IndexMap::new();
        values.insert(
            "resource/RESOURCE/program/Main/field/speed".to_string(),
            HmiValueRecord {
                v: json!(value),
                q: "good",
                ts_ms,
            },
        );
        HmiValuesResult {
            connected: true,
            timestamp_ms: ts_ms,
            source_time_ns: None,
            freshness_ms: Some(0),
            values,
        }
    }

    #[test]
    fn trend_downsample_preserves_bounds_and_window() {
        let schema = synthetic_schema(None, None);
        let mut live = HmiLiveState::default();
        for idx in 0..60 {
            update_live_state(
                &mut live,
                &schema,
                &synthetic_values(idx as f64, idx * 1_000),
            );
        }

        let trend = build_trends(&live, &schema, None, 60_000, 12);
        assert_eq!(trend.series.len(), 1);
        let points = &trend.series[0].points;
        assert!(points.len() <= 12);
        assert!(points.iter().all(|point| point.min <= point.value));
        assert!(points.iter().all(|point| point.max >= point.value));
        assert!(points.iter().all(|point| point.samples >= 1));

        let short_window = build_trends(&live, &schema, None, 10_000, 12);
        assert_eq!(short_window.series.len(), 1);
        let last_ts = short_window.series[0]
            .points
            .last()
            .map(|point| point.ts_ms)
            .unwrap_or_default();
        assert!(last_ts >= 50_000);
    }

    #[test]
    fn alarm_state_machine_covers_raise_ack_clear_history() {
        let schema = synthetic_schema(Some(0.0), Some(100.0));
        let mut live = HmiLiveState::default();

        update_live_state(&mut live, &schema, &synthetic_values(80.0, 1_000));
        let baseline = build_alarm_view(&live, 10);
        assert!(baseline.active.is_empty());

        update_live_state(&mut live, &schema, &synthetic_values(120.0, 2_000));
        let raised = build_alarm_view(&live, 10);
        assert_eq!(raised.active.len(), 1);
        assert_eq!(raised.active[0].state, "raised");
        assert_eq!(
            raised.history.first().map(|event| event.event),
            Some("raised")
        );

        let alarm_id = raised.active[0].id.clone();
        acknowledge_alarm(&mut live, alarm_id.as_str(), 2_500).expect("acknowledge alarm");
        let acknowledged = build_alarm_view(&live, 10);
        assert_eq!(acknowledged.active[0].state, "acknowledged");
        assert_eq!(
            acknowledged.history.first().map(|event| event.event),
            Some("acknowledged")
        );

        update_live_state(&mut live, &schema, &synthetic_values(95.0, 3_000));
        let cleared = build_alarm_view(&live, 10);
        assert!(cleared.active.is_empty());
        let history_events = cleared
            .history
            .iter()
            .map(|event| event.event)
            .collect::<Vec<_>>();
        assert!(history_events.contains(&"raised"));
        assert!(history_events.contains(&"acknowledged"));
        assert!(history_events.contains(&"cleared"));
    }

    #[test]
    fn alarm_deadband_requires_reentry_window_before_clear() {
        let schema = synthetic_schema_with_deadband(None, Some(100.0), Some(2.0));
        let mut live = HmiLiveState::default();

        update_live_state(&mut live, &schema, &synthetic_values(101.0, 1_000));
        let raised = build_alarm_view(&live, 10);
        assert_eq!(raised.active.len(), 1);

        // Still active because value is inside threshold but not outside deadband clear window.
        update_live_state(&mut live, &schema, &synthetic_values(99.0, 2_000));
        let still_active = build_alarm_view(&live, 10);
        assert_eq!(still_active.active.len(), 1);

        // Clears once value re-enters clear window (<= max-deadband).
        update_live_state(&mut live, &schema, &synthetic_values(97.5, 3_000));
        let cleared = build_alarm_view(&live, 10);
        assert!(cleared.active.is_empty());
    }
}
