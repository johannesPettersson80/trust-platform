//! HMI schema/value contract helpers.

#![allow(missing_docs)]

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
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
const DEFAULT_PAGE_ID: &str = "overview";
const DEFAULT_TREND_PAGE_ID: &str = "trends";
const DEFAULT_ALARM_PAGE_ID: &str = "alarms";
const DEFAULT_GROUP_NAME: &str = "General";
const DEFAULT_RESPONSIVE_MODE: &str = "auto";
const TREND_HISTORY_LIMIT: usize = 4096;
const ALARM_HISTORY_LIMIT: usize = 1024;

#[derive(Debug, Clone, Serialize)]
pub struct HmiSchemaResult {
    pub version: u32,
    pub mode: &'static str,
    pub read_only: bool,
    pub resource: String,
    pub generated_at_ms: u128,
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
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<String>,
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

#[derive(Debug, Clone, Default)]
pub struct HmiCustomization {
    theme: HmiThemeConfig,
    responsive: HmiResponsiveConfig,
    export: HmiExportConfig,
    pages: Vec<HmiPageConfig>,
    widget_overrides: BTreeMap<String, HmiWidgetOverride>,
    annotation_overrides: BTreeMap<String, HmiWidgetOverride>,
}

#[derive(Debug, Clone, Default)]
struct HmiThemeConfig {
    style: Option<String>,
    accent: Option<String>,
}

#[derive(Debug, Clone)]
struct HmiPageConfig {
    id: String,
    title: String,
    order: i32,
    kind: String,
    duration_ms: Option<u64>,
    signals: Vec<String>,
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
struct HmiWidgetOverride {
    label: Option<String>,
    unit: Option<String>,
    min: Option<f64>,
    max: Option<f64>,
    widget: Option<String>,
    page: Option<String>,
    group: Option<String>,
    order: Option<i32>,
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
        if let Ok(parsed) = load_hmi_toml(root) {
            customization.theme.style = parsed.theme.style;
            customization.theme.accent = parsed.theme.accent;
            customization.responsive.mode = parsed.responsive.mode;
            customization.export.enabled = parsed.export.enabled;
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
                        order,
                        kind,
                        duration_ms: page.duration_s.map(|seconds| seconds.saturating_mul(1_000)),
                        signals,
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
    }

    customization
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
            unit: point.unit,
            min: point.min,
            max: point.max,
        })
        .collect::<Vec<_>>();

    HmiSchemaResult {
        version: HMI_SCHEMA_VERSION,
        mode: if read_only { "read_only" } else { "read_write" },
        read_only,
        resource: resource_name.to_string(),
        generated_at_ms: now_unix_ms(),
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
        } else if alarm.active {
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
                            duration_ms: page.duration_ms,
                            signals: page.signals.clone(),
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
                duration_ms: None,
                signals: Vec::new(),
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
                duration_ms: Some(10 * 60 * 1_000),
                signals: Vec::new(),
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
                duration_ms: None,
                signals: Vec::new(),
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
                    duration_ms: None,
                    signals: Vec::new(),
                },
            );
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
}

fn is_trend_capable_widget(point: &HmiPoint) -> bool {
    is_numeric_data_type(point.data_type.as_str())
        || matches!(point.widget.as_str(), "value" | "slider")
}

fn is_trend_capable_widget_schema(widget: &HmiWidgetSchema) -> bool {
    is_numeric_data_type(widget.data_type.as_str())
        || matches!(widget.widget.as_str(), "value" | "slider")
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
    use crate::harness::TestHarness;
    use serde_json::json;
    use std::path::PathBuf;
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

    fn synthetic_schema(min: Option<f64>, max: Option<f64>) -> HmiSchemaResult {
        HmiSchemaResult {
            version: HMI_SCHEMA_VERSION,
            mode: "read_only",
            read_only: true,
            resource: "RESOURCE".to_string(),
            generated_at_ms: 0,
            theme: resolve_theme(None),
            responsive: resolve_responsive(None),
            export: resolve_export(None),
            pages: vec![HmiPageSchema {
                id: DEFAULT_PAGE_ID.to_string(),
                title: "Overview".to_string(),
                order: 0,
                kind: "dashboard".to_string(),
                duration_ms: None,
                signals: Vec::new(),
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
}
