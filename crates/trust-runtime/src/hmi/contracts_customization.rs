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
