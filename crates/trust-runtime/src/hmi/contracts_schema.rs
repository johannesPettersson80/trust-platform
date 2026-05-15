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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scene_view: Option<HmiSceneViewPayload>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub hidden: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub signals: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sections: Vec<HmiSectionSchema>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bindings: Vec<HmiProcessBindingSchema>,
    #[serde(default, rename = "bind3d", skip_serializing_if = "Vec::is_empty")]
    pub bindings3d: Vec<HmiSceneBindingSchema>,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiSceneViewPayload {
    #[serde(default, rename = "asset")]
    pub assets: Vec<HmiSceneAssetSchema>,
    #[serde(default, rename = "node")]
    pub nodes: Vec<HmiSceneNodeSchema>,
    #[serde(default, rename = "camera")]
    pub cameras: Vec<HmiSceneCameraSchema>,
    #[serde(default, rename = "light")]
    pub lights: Vec<HmiSceneLightSchema>,
    #[serde(default, rename = "bind3d")]
    pub bindings3d: Vec<HmiSceneBindingSchema>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiSceneAssetSchema {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiSceneNodeSchema {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub primitive: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transform: Option<HmiSceneTransformSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub material: Option<HmiSceneMaterialSchema>,
    #[serde(default, rename = "interaction", skip_serializing_if = "Vec::is_empty")]
    pub interactions: Vec<HmiSceneInteractionSchema>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiSceneTransformSchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotation: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<[f64; 3]>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiSceneMaterialSchema {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub emissive: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opacity: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiSceneCameraSchema {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fov_degrees: Option<f64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HmiSceneLightSchema {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<[f64; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub intensity: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmiSceneBindingSchema {
    pub node: String,
    #[serde(deserialize_with = "deserialize_scene_binding_property")]
    pub property: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub format: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<HmiProcessScaleSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmiSceneInteractionSchema {
    #[serde(deserialize_with = "deserialize_scene_interaction_event")]
    pub event: String,
    #[serde(deserialize_with = "deserialize_scene_interaction_action")]
    pub action: String,
    pub id: String,
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(deserialize_with = "deserialize_scene_interaction_required_role")]
    pub required_role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<HmiSceneInteractionConfirmationSchema>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmiSceneInteractionConfirmationSchema {
    pub title: String,
    pub message: String,
}

fn deserialize_scene_binding_property<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    normalize_scene_binding_property(value.as_str()).map_err(serde::de::Error::custom)
}

fn normalize_scene_binding_property(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    let allowed = matches!(
        normalized.as_str(),
        "visible"
            | "transform.position"
            | "transform.position.x"
            | "transform.position.y"
            | "transform.position.z"
            | "transform.rotation.x"
            | "transform.rotation.y"
            | "transform.rotation.z"
            | "transform.scale"
            | "transform.scale.x"
            | "transform.scale.y"
            | "transform.scale.z"
            | "material.base_color"
            | "material.emissive"
            | "material.opacity"
            | "text.value"
    );
    if allowed {
        Ok(normalized)
    } else {
        Err(format!("unsupported bind3d property '{}'", value.trim()))
    }
}

fn deserialize_scene_interaction_event<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    normalize_scene_interaction_event(value.as_str()).map_err(serde::de::Error::custom)
}

fn normalize_scene_interaction_event(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if matches!(normalized.as_str(), "click" | "touch" | "toggle") {
        Ok(normalized)
    } else {
        Err(format!("unsupported scene3d interaction event '{}'", value.trim()))
    }
}

fn deserialize_scene_interaction_action<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    normalize_scene_interaction_action(value.as_str()).map_err(serde::de::Error::custom)
}

fn normalize_scene_interaction_action(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "hmi.write" {
        Ok(normalized)
    } else {
        Err(format!(
            "unsupported scene3d interaction action '{}'",
            value.trim()
        ))
    }
}

fn deserialize_scene_interaction_required_role<'de, D>(
    deserializer: D,
) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    normalize_scene_interaction_required_role(value.as_str()).map_err(serde::de::Error::custom)
}

fn normalize_scene_interaction_required_role(value: &str) -> Result<String, String> {
    let normalized = value.trim().to_ascii_lowercase();
    if normalized == "engineer" {
        Ok("Engineer".to_string())
    } else {
        Err("scene3d hmi.write interaction requires required_role \"Engineer\"".to_string())
    }
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
