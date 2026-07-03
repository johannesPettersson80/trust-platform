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
