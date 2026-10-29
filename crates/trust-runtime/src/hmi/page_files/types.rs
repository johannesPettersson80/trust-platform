//! The page model of format 1 (specification 39, §5.1 and §15).
//!
//! The model also travels to the editor as JSON (§17.3). The JSON uses the file's own keys
//! (`object`, `pipe`, `type`, `top_bar`), so the editor and the file share one vocabulary.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// One page file, `hmi/pages/<name>.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiPage {
    /// Derived viewports for shared panels and page content; authored object coordinates stay local.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub regions: Vec<super::regions::HmiRegion>,
    /// Title shown in the navigation and the top bar.
    pub title: String,
    /// Place in the navigation: pages are listed by order, then by name (§4.6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub order: Option<u32>,
    /// One of the shipped navigation icons.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// The navigation group the page is listed under.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Left out of the navigation; page buttons still reach it.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// Explicitly unfinished authoring work; bundle validation refuses deployment.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub incomplete: bool,
    /// The P&ID whose process objects the page shares, `hmi/pid/<name>.toml` (§18 phase 4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<String>,
    /// Objects in z-order, the first drawn first.
    #[serde(rename = "object", default)]
    pub objects: Vec<HmiObject>,
    /// Pipes, each a connection between two ports.
    #[serde(rename = "pipe", default)]
    pub pipes: Vec<HmiPipe>,
}

/// A popup (§4.9): a page with its own design size that opens over the page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HmiPopup {
    /// Its title, objects and pipes.
    #[serde(flatten)]
    pub page: HmiPage,
    /// Design size in pixels.
    pub size: [u32; 2],
    /// Whether the operator may pin it open beside the page.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub pinnable: bool,
}

/// An object placed on a page (§5.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiObject {
    /// Stable id, unique on its page; never changes when the object moves or is renamed.
    pub id: String,
    /// Displayed equipment tag, separate from the id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tag: Option<String>,
    /// Component type, for example `tank` or `value`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Symbol variant, for example `horizontal`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// The function block type of the equipment the object shows (`Plant.FB_Pump`), recorded so a
    /// design whose PLC code is not created yet knows what to generate (§21.1 item 2).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    /// The panel the object opens when clicked, or for a placed panel (type `faceplate`) the panel
    /// it shows (§8.7 item 3, §8.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faceplate: Option<String>,
    /// The instance the object shows and its panel is bound to (§8.7 item 3, §8.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// Position of the top-left corner in pixels.
    pub at: [i64; 2],
    /// Width and height in pixels.
    pub size: [u32; 2],
    /// Rotation in degrees.
    #[serde(default)]
    pub rotation: u16,
    /// Label text.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Content text, for text and button components.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Picture of an image object: a project path under `hmi/assets/`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Engineering unit shown with the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// Decimals shown for numeric values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decimals: Option<u8>,
    /// Lower end of the display range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    /// Upper end of the display range.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    /// Display-only fallback alarm thresholds (§5.9), when no bound equipment supplies them.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limits: Option<super::limits::HmiLimits>,
    /// For a trend: the time it shows, in whole seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub span: Option<u32>,
    /// For a control: the lowest role that may use it, which can only raise the policy's (§12.8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<HmiRole>,
    /// Declared capability request, resolved and authorized by the host before execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<crate::hmi::commands::capability::MethodCommand>,
    /// For a control: a question asked before the command is sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<String>,
    /// For a button: what it does to its BOOL target.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<HmiButtonAction>,
    /// For a selector or mode selector: the choices it offers (§5.8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
    /// For a pulse button: how long the pulse lasts, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pulse: Option<f64>,
    /// For a control: the value its feedback reports when the command took effect.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expect: Option<HmiScalar>,
    /// For a control: how long the command may take to be confirmed, in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<f64>,
    /// For an instrument: what it measures, an equipment's id or `pipe:<n>` (§6, item 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attach: Option<String>,
    /// For a loop controller: the instrument whose measurement it reads, by id; drawn as a signal
    /// line from that instrument to the controller (§6, item 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub measures: Option<String>,
    /// For a loop controller: the equipment it acts on, by id; drawn as a signal line from the
    /// controller to it (§6, item 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acts_on: Option<String>,
    /// The invisible group it belongs to, by name (§3.3 item 8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Locked in the editor: not selected or moved on the canvas (§3.3 item 8).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
    /// For a panel or group: how it rearranges its objects on a smaller screen (§4, item 5).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arrange: Option<HmiArrange>,
    /// For an arranged panel or group: the space between its objects, in pixels.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gap: Option<u32>,
    /// Bindings by property name, sorted by name as they are stored.
    #[serde(default)]
    pub bind: BTreeMap<String, HmiBinding>,
    /// What a click does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub click: Option<HmiClick>,
}

impl HmiObject {
    /// What a click on the object does: its own `click`, else, for equipment that names its panel
    /// (`faceplate`, with its `instance`), opening that panel (specification 39 §8.7 item 3,
    /// §21.1). A placed panel opens nothing.
    #[must_use]
    pub fn effective_click(&self) -> Option<HmiClick> {
        if let Some(click) = &self.click {
            return Some(click.clone());
        }
        if self.kind == super::PLACED_FACEPLATE {
            return None;
        }
        self.faceplate.as_ref().map(|name| HmiClick::Faceplate {
            name: name.clone(),
            instance: self.instance.clone(),
        })
    }
}

/// The modes a mode selector offers when its options do not narrow them (§5.8).
pub const MODE_NAMES: [&str; 3] = ["Manual", "Auto", "Track"];

/// The lowest role a control asks for (§12.3, §12.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiRole {
    /// Operators and Engineers.
    Operator,
    /// Engineers only.
    Engineer,
}

/// How a panel or group rearranges its objects on a smaller screen (§4, item 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiArrange {
    /// Left to right, onto further rows.
    Row,
    /// One under the other.
    Column,
    /// In equal cells, as many columns as fit.
    Grid,
}

/// What a button does to its BOOL target (§5.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiButtonAction {
    /// Writes TRUE.
    Set,
    /// Writes FALSE.
    Reset,
    /// Writes the opposite of the current value.
    Toggle,
}

/// A single value written in a page file: a boolean, a number or text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HmiScalar {
    /// TRUE or FALSE.
    Bool(bool),
    /// A number.
    Number(f64),
    /// Text.
    Text(String),
}

/// The link from an object property to a program variable (§7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiBinding {
    /// Explicit BOOL-valid or quality-enumeration source, read with the value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,
    /// Runtime instance path, for example `FeedArea.P101.Running`.
    pub path: String,
    /// Engineering unit of the bound value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    /// A trend pen's fixed scale, overriding the object's range (§5.9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<[f64; 2]>,
    /// Whether the binding reads, writes or both.
    #[serde(default)]
    pub direction: HmiBindingDirection,
}

/// Direction of a binding (§7.4).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiBindingDirection {
    /// The object only shows the value.
    #[default]
    Read,
    /// The object only writes the value.
    Write,
    /// The object shows and writes the value.
    ReadWrite,
}

/// What a click on an object does (§5.4). Written as the file writes it: `{ faceplate, instance }`,
/// `{ popup }` or `{ page }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "ClickFields", try_from = "ClickFields")]
pub enum HmiClick {
    /// Opens the named faceplate, bound to the instance the click names, else to the instance
    /// whose members the object's bindings name (§8.7, item 4).
    Faceplate {
        /// The faceplate's name.
        name: String,
        /// The instance it is bound to, when the click names one.
        instance: Option<String>,
    },
    /// Opens the named popup.
    Popup(String),
    /// Goes to the named page.
    Page(String),
}

/// A click's fields as the file and the JSON model write them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClickFields {
    /// The faceplate it opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub faceplate: Option<String>,
    /// The instance the faceplate is bound to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance: Option<String>,
    /// The popup it opens.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub popup: Option<String>,
    /// The page it goes to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page: Option<String>,
}

impl From<HmiClick> for ClickFields {
    fn from(click: HmiClick) -> Self {
        match click {
            HmiClick::Faceplate { name, instance } => Self {
                faceplate: Some(name),
                instance,
                ..Self::default()
            },
            HmiClick::Popup(name) => Self {
                popup: Some(name),
                ..Self::default()
            },
            HmiClick::Page(name) => Self {
                page: Some(name),
                ..Self::default()
            },
        }
    }
}

impl TryFrom<ClickFields> for HmiClick {
    type Error = String;

    fn try_from(fields: ClickFields) -> Result<Self, Self::Error> {
        match (fields.faceplate, fields.instance, fields.popup, fields.page) {
            (Some(name), instance, None, None) => Ok(Self::Faceplate { name, instance }),
            (None, None, Some(name), None) => Ok(Self::Popup(name)),
            (None, None, None, Some(name)) => Ok(Self::Page(name)),
            (None, Some(_), _, _) => Err("`instance` goes with `faceplate`".to_string()),
            _ => Err("click needs exactly one of `faceplate`, `popup` or `page`".to_string()),
        }
    }
}

/// A pipe between two ports, written `object-id.port` (§6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiPipe {
    /// Port the pipe starts at, `<object id>.<port>`.
    pub from: String,
    /// Port the pipe ends at.
    pub to: String,
    /// Process line or signal line (§6, item 7).
    #[serde(default, skip_serializing_if = "HmiPipeStyle::is_process")]
    pub style: HmiPipeStyle,
    /// The bends the user placed, in pixels; the rest of the route is computed (§6, item 3).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waypoints: Vec<[i64; 2]>,
    /// The flow it shows, by property name: only `flow` (§6, item 6).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub bind: BTreeMap<String, HmiBinding>,
}

/// A pipe's line style (§6, item 7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiPipeStyle {
    /// A process line, which carries the medium.
    #[default]
    Process,
    /// A signal line, from an instrument to its controller or actuator.
    Signal,
}

impl HmiPipeStyle {
    fn is_process(&self) -> bool {
        *self == HmiPipeStyle::Process
    }
}

/// The screen layout file, `hmi/screen.toml` (§4.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiScreen {
    /// Page opened at /hmi when no explicit page was requested.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start_page: Option<String>,
    /// Design size in pixels.
    pub size: [u32; 2],
    /// Runtime theme.
    pub theme: HmiTheme,
    /// Symbol style of the project.
    pub symbols: HmiSymbolStyle,
    /// The project's languages as BCP 47 tags, the first being that of its texts (§14.5).
    #[serde(default = "english")]
    pub languages: Vec<String>,
    /// The top bar, when the screen has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub top_bar: Option<HmiTopBar>,
    /// Page navigation.
    pub navigation: HmiNavigation,
    /// The project's own colours (§14.1); tokens it does not set keep the defaults.
    #[serde(default, skip_serializing_if = "HmiColors::is_empty")]
    pub colors: HmiColors,
}

fn english() -> Vec<String> {
    vec!["en".to_string()]
}

/// A project's texts in one language, `hmi/texts/<tag>.toml` (§14.5): each text as written to the
/// text shown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HmiTexts {
    /// The language, a BCP 47 tag.
    pub language: String,
    /// The texts by the text as written.
    pub texts: BTreeMap<String, String>,
}

/// Colour tokens a project sets, per theme, as hex colours by token name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiColors {
    /// Overrides for the dark theme.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub dark: BTreeMap<String, String>,
    /// Overrides for the light theme.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub light: BTreeMap<String, String>,
}

impl HmiColors {
    /// Whether the project sets no colours.
    pub fn is_empty(&self) -> bool {
        self.dark.is_empty() && self.light.is_empty()
    }
}

/// Runtime theme (§4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiTheme {
    /// Light theme.
    Light,
    /// Dark theme.
    Dark,
}

/// Symbol style of a project (§4.1, §14.2): the two-dimensional symbols based on P&ID symbols.
/// A three-dimensional style may follow (§20, decision 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiSymbolStyle {
    /// Two-dimensional symbols with named parts and a moving part.
    Flat,
}

/// The top bar shared by all pages (§4.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiTopBar {
    /// Title text.
    pub title: String,
    /// Logo image, a project-relative path.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<String>,
    /// Shows the clock.
    #[serde(default)]
    pub clock: bool,
    /// Shows the alarm counter and the newest alarm.
    #[serde(default)]
    pub alarms: bool,
    /// Kept as the file says it. It changes nothing on the display: the session sits in every top bar
    /// (specification 39, §4.8 item 8; the owner, 29 October 2026).
    #[serde(default)]
    pub user: bool,
    /// Shows the current page title.
    #[serde(default)]
    pub page_title: bool,
}

/// Page navigation (§4.1, §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HmiNavigation {
    /// How pages are offered.
    pub style: HmiNavigationStyle,
    /// Side of the side bar.
    pub position: HmiNavigationPosition,
}

/// How pages are offered (§4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiNavigationStyle {
    /// A side bar.
    Side,
    /// Tabs on top.
    Tabs,
    /// No navigation.
    None,
}

/// Side of the side bar (§4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HmiNavigationPosition {
    /// Left side.
    Left,
    /// Right side.
    Right,
}
