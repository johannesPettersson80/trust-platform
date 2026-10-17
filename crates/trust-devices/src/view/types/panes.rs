//! The panes beside the graph: the USB tray and the installation of truST, the add dialog, the
//! forms, the search and the browse panes (specification 25 §2, §5.12 to §5.14).

use serde::Serialize;
use serde_json::Value;

use super::{Action, ItemId, Notice, Row, Tone};

/// The boards on this computer's USB (specification 25 §5.12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbTray {
    /// The tray's heading.
    pub heading: String,
    /// The boards.
    pub items: Vec<UsbItem>,
    /// What shows without a board.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_text: Option<String>,
}

/// A board on USB.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct UsbItem {
    /// Its identity.
    pub id: String,
    /// Its name.
    pub title: String,
    /// The line under its name: a chip's flash and RAM, what a probe or adapter is with its serial
    /// ports, or which runtime it is.
    pub detail: String,
    /// Its truST state: its truST version, `No truST yet` or `truST not confirmed`; absent for a
    /// probe or adapter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// `Install truST`, `Update truST`, `Connect` or `Show`.
    pub actions: Vec<Action>,
    /// The installation, while one runs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub install: Option<InstallView>,
}

/// The installation of truST on a chip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallView {
    /// Its steps.
    pub steps: Vec<InstallStep>,
    /// The device settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<Form>,
    /// Its actions.
    pub actions: Vec<Action>,
}

/// A step of the installation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallStep {
    /// Its title.
    pub title: String,
    /// Its state.
    pub state: StepState,
    /// Its facts.
    pub rows: Vec<Row>,
    /// Its progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
}

/// The state of an installation step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StepState {
    /// Done.
    Done,
    /// Running.
    Active,
    /// Not started.
    Waiting,
    /// Failed.
    Failed,
}

/// A step's progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progress {
    /// What is done.
    pub done: u64,
    /// All of it.
    pub total: u64,
    /// In words.
    pub text: String,
}

/// The add dialog (specification 25 §5.14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddDialog {
    /// Its title.
    pub title: String,
    /// The runtime that a new connection belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<RuntimeRef>,
    /// The runtimes to choose from.
    pub runtime_choices: Vec<RuntimeRef>,
    /// `Discover devices and runtimes`.
    pub discover: Action,
    /// Its help.
    pub discover_help: String,
    /// `Found on the network`: the session's last search.
    pub found: Vec<FoundEntry>,
    /// The `Runtimes` group, first.
    pub runtimes: AddGroup,
    /// The other groups, in their canonical order.
    pub groups: Vec<AddGroup>,
    /// Shows or hides the advanced entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub advanced_toggle: Option<AdvancedToggle>,
    /// The quick starts, while the target runtime has no connections.
    pub quick_starts: Vec<QuickStart>,
    /// Why the catalog is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catalog_state: Option<String>,
}

/// A runtime named in a dialog or a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeRef {
    /// Its identity.
    pub id: ItemId,
    /// Its name.
    pub name: String,
}

/// A group of the add dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddGroup {
    /// Its name.
    pub label: String,
    /// Whether it is advanced.
    pub advanced: bool,
    /// Its entries.
    pub entries: Vec<AddEntry>,
}

/// An entry of the add dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddEntry {
    /// Its protocol.
    pub protocol: String,
    /// Its title.
    pub title: String,
    /// Its badge.
    pub badge: String,
    /// Its purpose.
    pub purpose: String,
    /// What choosing it does.
    pub action: Action,
}

/// Shows or hides the advanced entries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdvancedToggle {
    /// The text that shows them.
    pub show_label: String,
    /// The text that hides them.
    pub hide_label: String,
    /// How many are hidden.
    pub count: u32,
}

/// A quick start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct QuickStart {
    /// Its title.
    pub title: String,
    /// What it adds.
    pub description: String,
    /// What choosing it does.
    pub action: Action,
}

/// A find of the last search in the add dialog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FoundEntry {
    /// Its device card.
    pub card: String,
    /// Its title.
    pub title: String,
    /// Its facts.
    pub rows: Vec<Row>,
    /// `Add`, or `Adopt` for a runtime.
    pub action: Action,
}

/// A form, which `form.open` returns.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Form {
    /// Its identity, from protocol, runtime and prefill.
    pub id: String,
    /// Its protocol.
    pub protocol: String,
    /// Its title.
    pub title: String,
    /// Its breadcrumb.
    pub breadcrumb: String,
    /// The runtime it writes to, in words.
    pub owner_line: String,
    /// The protocol's purpose.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purpose: Option<String>,
    /// The runtime that `Save` writes to; absent for a form that adds a runtime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeRef>,
    /// The runtimes to choose from.
    pub runtime_choices: Vec<RuntimeRef>,
    /// Its sections: the first without a heading, then `Advanced`.
    pub sections: Vec<FormSection>,
    /// Its buttons.
    pub buttons: Vec<Action>,
    /// A note, such as that the endpoint is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The draft tile drawn while the form is open.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ghost: Option<GhostTile>,
}

/// A section of a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormSection {
    /// Its heading.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// Its fields.
    pub fields: Vec<FormField>,
}

/// A field of a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FormField {
    /// Its identity.
    pub id: String,
    /// Its label.
    pub label: String,
    /// Whether it is required.
    pub required: bool,
    /// What it holds.
    pub kind: FieldKind,
    /// Its value: the current one, the default or the prefill, chosen by Rust.
    pub value: Value,
    /// Its choices.
    pub options: Vec<FieldOption>,
    /// Its help.
    pub help: String,
    /// Why its value was refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// When it is shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub visible_when: Option<VisibleWhen>,
    /// Its list editor.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<ListEditor>,
    /// The example shown while it is empty.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

/// What a field holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldKind {
    /// Text.
    Text,
    /// A number.
    Number,
    /// On or off.
    Boolean,
    /// One of its options.
    Choice,
    /// A list.
    List,
    /// An object.
    Object,
    /// A secret, never shown.
    Secret,
}

/// A choice of a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldOption {
    /// Its value.
    pub value: String,
    /// Its label.
    pub label: String,
}

/// A field that is shown only while another field has a value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VisibleWhen {
    /// The other field.
    pub field: String,
    /// The value.
    pub equals: String,
}

/// The editor of a list field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListEditor {
    /// What one item is called.
    pub item_noun: String,
    /// The heading of an item, before its number, such as `Peer address` in `Peer address 1`.
    pub item_title: String,
    /// The button that adds one.
    pub add_label: String,
    /// What shows without items.
    pub empty_text: String,
    /// Whether items can be added.
    pub can_add: bool,
    /// What an item holds.
    pub item_kind: FieldKind,
    /// A new item.
    pub item_template: Value,
    /// The properties of the items that are objects, in their order.
    pub properties: Vec<ListProperty>,
}

/// A property of the object items of a list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListProperty {
    /// Its key in an item.
    pub key: String,
    /// Its label.
    pub label: String,
    /// What it holds.
    pub kind: FieldKind,
    /// Whether an item needs it.
    pub required: bool,
    /// Its choices.
    pub options: Vec<FieldOption>,
    /// The properties that an item can set instead of this one, of which it sets exactly one,
    /// such as `source_cidr` for `source_ip`.
    pub alternatives: Vec<String>,
}

/// The draft tile drawn while a form is open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GhostTile {
    /// Its name.
    pub name: String,
    /// Its detail.
    pub detail: String,
}

/// The search pane (specification 25 §2 and §5.14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DiscoverView {
    /// The running or last search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_id: Option<String>,
    /// Whether it runs.
    pub running: bool,
    /// Its progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_text: Option<String>,
    /// `Scan from`.
    pub origins: Vec<OriginChoice>,
    /// `Only search`.
    pub only: Vec<OnlyChoice>,
    /// `Known address or subnet`.
    pub targets: TargetsInput,
    /// The state of each source.
    pub sources: Vec<SourceRow>,
    /// The fieldbus scans that need a confirmation.
    pub fieldbus: Vec<FieldbusOffer>,
    /// One card per device.
    pub cards: Vec<DeviceCard>,
    /// `Compare with project`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<CompareView>,
    /// What shows without finds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_text: Option<String>,
    /// Its actions.
    pub actions: Vec<Action>,
}

/// An origin of the search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OriginChoice {
    /// Its identity.
    pub id: String,
    /// Its name.
    pub label: String,
    /// Whether it can be chosen.
    pub enabled: bool,
    /// Why it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Whether it is chosen.
    pub selected: bool,
}

/// A source of `Only search`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct OnlyChoice {
    /// The source.
    pub source: String,
    /// Its name.
    pub label: String,
    /// Whether it is ticked.
    pub checked: bool,
    /// Whether it can be ticked.
    pub enabled: bool,
    /// Why it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// The search's address and subnet inputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TargetsInput {
    /// The address input's label.
    pub hosts_label: String,
    /// The subnet input's label.
    pub cidr_label: String,
    /// The address input's placeholder.
    pub hosts_placeholder: String,
    /// The subnet input's placeholder.
    pub cidr_placeholder: String,
}

/// A source's state in the search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceRow {
    /// The source.
    pub id: String,
    /// Its name.
    pub label: String,
    /// `searching`, `{n} found`, `timed out` or `skipped`.
    pub word: String,
    /// Its finds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<u32>,
    /// Why it was skipped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Its tone.
    pub tone: Tone,
}

/// A fieldbus scan that runs only after a confirmation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldbusOffer {
    /// The bus.
    pub bus: String,
    /// Its name.
    pub label: String,
    /// The adapter or port.
    pub target: String,
    /// What the scan does.
    pub text: String,
    /// The confirmation.
    pub action: Action,
}

/// A found device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DeviceCard {
    /// Its identity.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its addresses.
    pub addresses: Vec<String>,
    /// Every protocol found on it.
    pub protocols: Vec<CardProtocol>,
    /// Its identity facts.
    pub identity: Vec<Row>,
    /// When it was last seen.
    pub last_seen: String,
    /// Whether the project uses it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    /// How it differs from the project.
    pub differs: Vec<Row>,
    /// The search's warnings about it.
    pub warnings: Vec<String>,
    /// Its actions.
    pub actions: Vec<Action>,
    /// Why its actions are disabled on a stale result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_reason: Option<String>,
}

/// A protocol found on a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CardProtocol {
    /// The protocol's name.
    pub name: String,
    /// The source's name.
    pub source: String,
    /// The confidence's name.
    pub confidence: String,
    /// Whether the find is simulated.
    pub simulated: bool,
}

/// `Compare with project`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompareView {
    /// Configured devices not found.
    pub missing: Vec<Row>,
    /// Found devices the project does not use.
    pub unused: Vec<Row>,
    /// Devices whose identity differs.
    pub differs: Vec<Row>,
}

/// A browse pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowseView {
    /// Its identity.
    pub id: String,
    /// Its title.
    pub title: String,
    /// Its subtitle.
    pub subtitle: String,
    /// What it browses.
    pub kind: BrowseKind,
    /// The tree.
    pub tree: Vec<TreeNode>,
    /// The ADS ports.
    pub ports: Vec<AdsPortCard>,
    /// The suggested ADS ports.
    pub port_suggestions: Vec<FieldOption>,
    /// Their help.
    pub port_help: String,
    /// What failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBox>,
    /// `Add`.
    pub add: AddButton,
    /// The write toggle.
    pub write_toggle: Toggle,
    /// The result of the last action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<Notice>,
}

/// What a browse pane browses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BrowseKind {
    /// OPC UA nodes.
    Nodes,
    /// Fieldbus channels.
    Channels,
    /// The globals a server exposes.
    Expose,
    /// ADS tags.
    AdsTags,
}

/// A node of a browse tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TreeNode {
    /// Its identity in the selection.
    pub key: String,
    /// Its name.
    pub name: String,
    /// Its path.
    pub path: String,
    /// Its type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_label: Option<String>,
    /// `read/write` or `read-only`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access: Option<String>,
    /// Whether it can be ticked.
    pub selectable: bool,
    /// Whether the project already has it.
    pub added: bool,
    /// Its children.
    pub children: Vec<TreeNode>,
}

/// An ADS port of the tag browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdsPortCard {
    /// The port.
    pub port: u16,
    /// Its title.
    pub title: String,
    /// Its state.
    pub state_text: String,
    /// Its tags.
    pub tags: Vec<TreeNode>,
    /// The route help of a port whose route is missing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<RouteHelp>,
    /// What failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBox>,
    /// Browses the port.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub browse: Option<Action>,
}

/// The help of a missing `TwinCAT` route.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RouteHelp {
    /// The text.
    pub text: String,
    /// The help lines.
    pub help: Vec<String>,
    /// The generated route files.
    pub artifacts: Vec<Action>,
    /// `Create route`.
    pub create_route: Action,
}

/// A failure in a pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ErrorBox {
    /// What failed.
    pub title: String,
    /// Why.
    pub detail: String,
    /// The recovery action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Action>,
}

/// The add button of a browse pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AddButton {
    /// Its text.
    pub label: String,
    /// Whether it can be chosen.
    pub enabled: bool,
    /// Why it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Why it cannot while nothing is ticked.
    pub no_selection_reason: String,
}

/// A toggle of a browse pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Toggle {
    /// Its text.
    pub label: String,
    /// Whether it can be changed.
    pub enabled: bool,
    /// Why it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}
