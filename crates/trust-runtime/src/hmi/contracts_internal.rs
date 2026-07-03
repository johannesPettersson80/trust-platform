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
