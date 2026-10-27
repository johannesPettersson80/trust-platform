//! The panes beside the graph: the USB tray and the installation of truST, the add dialog, the
//! forms, the search and the browse panes (specification 25 §2, §5.12 to §5.14).

use serde::Serialize;
use serde_json::Value;

use super::{Action, ItemId, Notice, Pill, Row, Tone};

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
    /// The heading of the finds: `Found on the network` (Rust's text, specification 25 §5.14).
    pub found_heading: String,
    /// The finds of the session's last search.
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
    /// The accessible name of the close button.
    pub close_label: String,
}

/// A runtime named in a dialog or a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeRef {
    /// Its identity.
    pub id: ItemId,
    /// Its name.
    pub name: String,
}

/// A runtime a form can be reopened on, with what choosing it does (specification 25 §5.14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RuntimeChoice {
    /// Its identity.
    pub id: ItemId,
    /// Its name.
    pub name: String,
    /// What choosing it does: `form.open` of the same form on that runtime, with the values typed
    /// so far.
    pub action: Action,
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
    /// The runtimes to choose from, which a new connection can be saved to.
    pub runtime_choices: Vec<RuntimeChoice>,
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
    /// The mark a required field carries, from Rust's texts; absent for an optional field.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_mark: Option<String>,
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
    /// Whether the form forgets this field's value after a Save that Rust answers, such as a
    /// username or password typed for one request. Other values stay, so a refused Save keeps them.
    pub forget_after_post: bool,
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

/// The search pane (specification 25 §2 and §5.14). Every heading, help text and action of the pane
/// is a field here, so the pane draws nothing of its own.
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
    /// The breadcrumb: `Devices & Connections`.
    pub breadcrumb: String,
    /// The accessible name of the close button.
    pub close_label: String,
    /// The heading of the scan choices: `What do you want to find?`.
    pub heading_find: String,
    /// The heading of the origin choices: `Scan from`.
    pub heading_scan_from: String,
    /// The heading of the known address and subnet inputs: `Known address or subnet`.
    pub heading_targets: String,
    /// The heading of the advanced settings: `Advanced scan settings`.
    pub heading_advanced: String,
    /// The heading of the runtime's hardware scans: `Runtime hardware scans ⚠`.
    pub heading_runtime: String,
    /// The help that shows when no discovery is available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unavailable_text: Option<String>,
    /// The help that shows above the scan choices.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choose_text: Option<String>,
    /// `Scan from`.
    pub origins: Vec<OriginChoice>,
    /// `Only search`.
    pub only: Vec<OnlyChoice>,
    /// `Known address or subnet`.
    pub targets: TargetsInput,
    /// The state of each source.
    pub sources: Vec<SourceRow>,
    /// The line that asks to update truST, when the status named a source or protocol that this
    /// build does not know; that part is left out of the view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub update_text: Option<String>,
    /// The fieldbus scans that need a confirmation.
    pub fieldbus: Vec<FieldbusOffer>,
    /// `Show ignored`: the toggle of the finds that the project ignores (decision O14).
    pub show_ignored: ShowIgnored,
    /// One card per device.
    pub cards: Vec<DeviceCard>,
    /// The lists of `Compare with project`, while the user has them shown for the finished search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compare: Option<CompareView>,
    /// What shows without finds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_text: Option<String>,
    /// The text of a search that failed at a runtime, as the old pane gave it (specification 25 §2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_text: Option<String>,
    /// Its actions: `Search`, `Cancel` while a scan runs, `Compare with project`, and `Configure ADS ports to scan`.
    pub actions: Vec<Action>,
}

/// The `Show ignored` toggle of the Discover pane (decision O14): whether the finds that the project
/// ignores are shown, and the change of the toggle, which the search answers with the new view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ShowIgnored {
    /// Its text: `Show ignored`.
    pub label: String,
    /// Whether it is ticked.
    pub checked: bool,
    /// The change of the toggle, a `search` operation.
    pub action: Action,
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
    /// The note under its name, as the old pane gave it: `mDNS`, `at host`, and so on.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// The group the choice is drawn in: `find`, `known_address` or `runtime_hardware`.
    pub group: String,
    /// The title of the warning that the choice shows beside its name, such as the `EtherCAT` title
    /// (`DiscoverPane.tsx` line 204).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    /// The change of the tick, a `search` operation that records it and answers the new view; absent
    /// while the choice is disabled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
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
    /// What the project makes of it (specification 25 §2, the project mark): `In the project`, `Not in
    /// the project` or `Differs from the project`, and `On canvas` for a `TwinCAT` device that is in the
    /// project; absent for a device the comparison does not cover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<Pill>,
    /// How it differs from the project: one row for each fact that differs.
    pub differs: Vec<Row>,
    /// The search's warnings about it.
    pub warnings: Vec<String>,
    /// Its actions.
    pub actions: Vec<Action>,
    /// Why its actions are disabled on a stale result.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stale_reason: Option<String>,
    /// The count of a `TwinCAT` device's responding ADS ports, such as `2 responding ADS ports`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub responding_ports: Option<String>,
    /// One button per responding ADS port, which browses that port's tags of the device.
    pub ports: Vec<Action>,
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
    /// The visible mark of a simulated find: it is never presented as hardware (specification 11,
    /// "Confidence").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mark: Option<String>,
}

/// `Compare with project` (specification 25 §2): what the comparison of the finished search with the project
/// says, in four lists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompareView {
    /// The heading of the lists: `Compared with the project`.
    pub heading: String,
    /// The configured devices that the search did not find.
    pub missing: CompareList,
    /// The found devices that the project does not use.
    pub unused: CompareList,
    /// The found devices whose identity differs from the project: one row for each fact that differs.
    pub differs: CompareList,
    /// The configured devices that the comparison could not judge, each with its reason.
    pub not_compared: CompareList,
}

/// One list of the comparison: its heading, its rows, and what shows when it holds none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CompareList {
    /// Its heading, such as `Configured devices not found`.
    pub heading: String,
    /// Its rows.
    pub rows: Vec<Row>,
    /// What shows when `rows` is empty, such as `Every configured device was found.`.
    pub empty_text: String,
}

/// A browse pane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowseView {
    /// Its identity.
    pub id: String,
    /// Its breadcrumb: `Devices & Connections`.
    pub breadcrumb: String,
    /// Its title.
    pub title: String,
    /// Its subtitle: the connection's name.
    pub subtitle: String,
    /// What it browses.
    pub kind: BrowseKind,
    /// Whether the browse still runs: true from the first answer until the origin has answered
    /// (design §1.5, line 381).
    pub loading: bool,
    /// The accessible name of its close button.
    pub close_label: String,
    /// The word of the mark on a leaf that the project already has.
    pub added_label: String,
    /// The tooltip of a leaf's access word: the template `{access} on the device`, which the
    /// webview fills with the access word.
    pub access_tooltip: String,
    /// The accessible name of a leaf's Added mark: the template `Already added: {path}`, which the
    /// webview fills with the leaf's path.
    pub added_aria_label: String,
    /// The search box, while the tree or the ports are there to search.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<SearchBox>,
    /// What the pane shows instead of its tree: the loading, empty or error text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_text: Option<String>,
    /// The tree of an OPC UA, `EtherCAT` or exposed-globals browse.
    pub tree: Vec<TreeNode>,
    /// The ADS ports, in port order.
    pub ports: Vec<AdsPortCard>,
    /// The suggested ADS ports.
    pub port_suggestions: Vec<FieldOption>,
    /// Their help.
    pub port_help: String,
    /// The typed ADS port: its toggle, its field and its browse action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_input: Option<PortInput>,
    /// The ADS port summary line, such as `2 ADS ports`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_summary: Option<String>,
    /// The connection's details, shown under a disclosure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Details>,
    /// What failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorBox>,
    /// `Add`; an ADS pane adds its tags with their own ticks and has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub add: Option<AddButton>,
    /// The write toggle.
    pub write_toggle: Toggle,
    /// The label of the button that closes the pane in its footer, when the pane has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub done_label: Option<String>,
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
    /// The accessible name of its tick box, such as `Select MAIN.Counter`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub select_label: Option<String>,
    /// Whether the project already has it.
    pub added: bool,
    /// What ticking or unticking it does: an ADS tag's `ads.tags_set`, `ticked` while it is not in
    /// the project and `unticked` once it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<Action>,
    /// Its children.
    pub children: Vec<TreeNode>,
}

/// An ADS port of the tag browser.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AdsPortCard {
    /// The port.
    pub port: u16,
    /// Its title, such as `ADS port 851`.
    pub title: String,
    /// Its state, such as `2 tags` or `Loading…`.
    pub state_text: String,
    /// The text of its body when no tree is drawn: loading, not loaded, or no tags.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_text: Option<String>,
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
    /// Its text with the number of ticked symbols: `{count}` stands for the number.
    pub counted_label: String,
    /// Whether it can be chosen.
    pub enabled: bool,
    /// Why it cannot.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Why it cannot while nothing is ticked.
    pub no_selection_reason: String,
    /// What choosing it does: `browse.add` with the pane's own request; the webview adds the ticked
    /// keys and the write toggle as its values (specification 25 §5.4 and §11.2).
    pub action: Action,
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

/// The search box of a browse pane (specification 25 §5.4; the old panels' search, word for word).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchBox {
    /// The text the empty box shows.
    pub placeholder: String,
    /// The text shown when nothing matches.
    pub no_match: String,
}

/// The typed ADS port of a browse pane: its toggle, its field and the action that browses the typed
/// port (specification 25 §5.4 and its lines 880-883).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PortInput {
    /// The toggle's text while the field is hidden.
    pub show_label: String,
    /// The toggle's text while the field is shown.
    pub hide_label: String,
    /// The field's label.
    pub label: String,
    /// The text the empty field shows.
    pub placeholder: String,
    /// `Browse this port`: a `browse.port` action that takes the typed text as its `port` value.
    pub browse: Action,
}

/// The details of a connection, under a disclosure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Details {
    /// The disclosure's text.
    pub summary: String,
    /// The facts.
    pub rows: Vec<Row>,
}
