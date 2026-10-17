//! The panel and the items of its graph: the toolbar, project bar, summary, issues and filter,
//! the networks, computers, containers, runtimes, connections, devices, links and mesh buses,
//! their inspectors and their rows of the list (specification 25 §5.11 to §5.14).

use std::collections::BTreeMap;

use serde::Serialize;

use super::{
    Action, AddDialog, HoverCard, ItemId, Look, Notice, Pill, Section, Severity, Tone, UsbTray,
};

/// The whole panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DevicesView {
    /// The contract's version, `SCHEMA_VERSION`.
    pub schema_version: u32,
    /// When the view was made, in milliseconds since the Unix epoch.
    pub generated_ms: u64,
    /// The toolbar's actions.
    pub toolbar: Toolbar,
    /// The project name and the project list.
    pub project: ProjectBar,
    /// The summary strip.
    pub summary: Summary,
    /// The banner, while one is shown.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub banner: Option<Banner>,
    /// The issues button and list.
    pub issues: Issues,
    /// The protocol filter.
    pub filter: FilterReport,
    /// The search.
    pub search: SearchEcho,
    /// The bands of computers on one network.
    pub networks: Vec<Network>,
    /// The computers and chips, each with its runtimes.
    pub computers: Vec<Computer>,
    /// The devices and systems at the other end of the connections.
    pub devices: Vec<Device>,
    /// The links.
    pub links: Vec<Link>,
    /// The mesh buses.
    pub mesh: Vec<MeshBus>,
    /// The boards on this computer's USB.
    pub usb: UsbTray,
    /// The add dialog.
    pub add: AddDialog,
    /// The list's columns.
    pub list: ListView,
    /// Results of earlier actions, each for its pane (specification 25 §5.13).
    pub notices: Vec<Notice>,
}

/// The toolbar's actions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Toolbar {
    /// `+ Add`.
    pub add: Action,
    /// `Discover`.
    pub discover: Action,
    /// The Edit slot after the computers: `Add host`.
    pub add_host_slot: Action,
}

/// The project name and the projects to choose from (specification 25 §5.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectBar {
    /// The chosen project's name.
    pub name: String,
    /// Its folder; empty without a project.
    pub root: String,
    /// The projects of the workspace.
    pub choices: Vec<ProjectChoice>,
}

/// A project of the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProjectChoice {
    /// Its name.
    pub name: String,
    /// Its folder.
    pub root: String,
    /// Whether it is the one shown.
    pub selected: bool,
}

/// The summary strip (specification 25 §5.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Summary {
    /// The line, such as `1 computer · 2 runtimes · 3 connections · 2 running`.
    pub text: String,
    /// The computers.
    pub computers: u32,
    /// The runtimes.
    pub runtimes: u32,
    /// The connections.
    pub connections: u32,
    /// The runtimes that run.
    pub running: u32,
    /// The faults.
    pub faults: u32,
    /// The warnings.
    pub warnings: u32,
    /// Opens the issues list.
    pub open_issues: Action,
}

/// The banner (specification 25 §5.10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Banner {
    /// Its tone.
    pub tone: Tone,
    /// Its text.
    pub text: String,
    /// Its actions, such as `Retry` and `Open logs`.
    pub actions: Vec<Action>,
}

/// The issues button and the list of every fault and warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issues {
    /// The button.
    pub button: IssuesButton,
    /// The issues, grouped by item.
    pub groups: Vec<IssueGroup>,
}

/// The issues button.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssuesButton {
    /// Its text.
    pub text: String,
    /// Its tone.
    pub tone: Tone,
    /// The faults.
    pub faults: u32,
    /// The warnings.
    pub warnings: u32,
}

/// The issues of one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IssueGroup {
    /// The item; absent for an issue of no shown item.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub item: Option<ItemId>,
    /// The item's name.
    pub item_name: String,
    /// Its issues.
    pub issues: Vec<Issue>,
}

/// One fault or warning.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// Its identity.
    pub id: String,
    /// How severe it is.
    pub severity: Severity,
    /// The text.
    pub text: String,
    /// `Show`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show: Option<Action>,
    /// The recovery action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recovery: Option<Action>,
}

/// What the protocol filter hides (specification 25 §5.5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilterReport {
    /// The protocols shown in the graph.
    pub protocols: Vec<FilterProtocol>,
    /// The filter panel's lines.
    pub lines: Vec<FilterLine>,
    /// The hidden connections.
    pub hidden_connections: u32,
    /// The hidden connections that need attention.
    pub hidden_attention: u32,
    /// The hidden errors.
    pub hidden_errors: u32,
    /// The hidden warnings.
    pub hidden_warnings: u32,
}

/// A protocol of the filter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilterProtocol {
    /// The protocol.
    pub id: String,
    /// Its name.
    pub label: String,
    /// Its connections.
    pub count: u32,
    /// Whether the filter hides it.
    pub hidden: bool,
}

/// A line of the filter panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FilterLine {
    /// The text.
    pub text: String,
    /// Its tone.
    pub tone: Tone,
}

/// The search as the panel shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SearchEcho {
    /// The search text.
    pub query: String,
    /// The placeholder.
    pub placeholder: String,
    /// The items that match.
    pub matches: u32,
}

/// A band of the computers on one network (decision O29).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Network {
    /// Its identity.
    pub id: String,
    /// Its name, such as `Network · 192.168.10.0/24`.
    pub label: String,
    /// The computers in it.
    pub computers: Vec<ItemId>,
}

/// A computer or a chip, drawn as a frame holding its runtimes (specification 25 §5.12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Computer {
    /// Its identity.
    pub id: ItemId,
    /// A computer or a chip.
    pub kind: ComputerKind,
    /// `This computer`, `Computer <IP>`, a configured label, or the chip name and address.
    pub heading: String,
    /// The board, operating system and architecture, or the chip.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    /// `Reachable` or `Unreachable`.
    pub reachability: Pill,
    /// How the frame is drawn.
    pub look: Look,
    /// The containers that run its runtimes.
    pub containers: Vec<Container>,
    /// Every runtime on it, side by side.
    pub runtimes: Vec<Runtime>,
    /// Its hover card.
    pub hover: HoverCard,
    /// Its inspector.
    pub inspector: Inspector,
    /// Its Edit slots: `Set up runtime`.
    pub slots: Vec<Action>,
    /// Whether the search dims it.
    pub dimmed: bool,
    /// Whether the filter hides it.
    pub hidden: bool,
    /// Its row of the list.
    pub list: ListCells,
}

/// What a frame holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputerKind {
    /// A computer.
    Computer,
    /// A chip that runs truST.
    Chip,
}

/// A container on a computer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Container {
    /// Its identity.
    pub id: ItemId,
    /// Its name.
    pub name: String,
    /// Its state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    /// Whether it is drawn as a frame: only with more than one runtime; a container of one
    /// runtime is the chip on that runtime's card.
    pub framed: bool,
    /// Its runtimes.
    pub runtimes: Vec<ItemId>,
    /// Its hover card.
    pub hover: HoverCard,
    /// Its inspector.
    pub inspector: Inspector,
}

/// A runtime card (specification 25 §5.12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Runtime {
    /// Its identity.
    pub id: ItemId,
    /// Its name.
    pub name: String,
    /// Its kind icon.
    pub icon: RuntimeIcon,
    /// Its state.
    pub state: Pill,
    /// The current cycle time, such as `4.2 ms`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cycle: Option<String>,
    /// The host and port of its control address.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// Whether it is the selected run target.
    pub run_target: bool,
    /// How the card is drawn.
    pub look: Look,
    /// The container it runs in, when that container holds no other runtime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub container_chip: Option<String>,
    /// The hint of a card without connections.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub empty_hint: Option<EmptyHint>,
    /// Its connections.
    pub connections: Vec<Connection>,
    /// Its hover card.
    pub hover: HoverCard,
    /// Its inspector.
    pub inspector: Inspector,
    /// Its right-click menu.
    pub context_menu: Vec<Action>,
    /// Its Edit slots: `Add connection`.
    pub slots: Vec<Action>,
    /// Whether the search dims it.
    pub dimmed: bool,
    /// Whether the filter hides it.
    pub hidden: bool,
    /// Its row of the list.
    pub list: ListCells,
}

/// A runtime's kind icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeIcon {
    /// The simulator.
    Simulator,
    /// A runtime on a computer.
    Computer,
    /// The truST firmware of a chip.
    Chip,
}

/// The hint of a runtime card without connections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EmptyHint {
    /// `No devices yet`.
    pub title: String,
    /// What to do.
    pub text: String,
}

/// A connection pill on its runtime (specification 25 §5.13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the flags are independent facts of the contract's JSON"
)]
pub struct Connection {
    /// Its identity.
    pub id: ItemId,
    /// Its protocol.
    pub protocol: String,
    /// The protocol's product name.
    pub protocol_name: String,
    /// The connection's own name.
    pub name: String,
    /// The status dot.
    pub dot: Tone,
    /// Whether the dot pulses.
    pub live: bool,
    /// The role word, such as `CLIENT`.
    pub role_word: String,
    /// `DRAFT` and `Disabled`.
    pub marks: Vec<Mark>,
    /// How the pill is drawn.
    pub look: Look,
    /// Whether a link leaves it.
    pub wire_port: bool,
    /// The devices on its bus.
    pub subdevices: Vec<ItemId>,
    /// Its hover card.
    pub hover: HoverCard,
    /// Its inspector.
    pub inspector: Inspector,
    /// Whether the search dims it.
    pub dimmed: bool,
    /// Whether the filter hides it.
    pub hidden: bool,
    /// Its row of the list.
    pub list: ListCells,
}

/// A mark on a connection pill.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mark {
    /// Not applied yet.
    Draft,
    /// Turned off.
    Disabled,
}

/// A device or system at the other end of a connection (specification 25 §5.13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Device {
    /// Its identity.
    pub id: ItemId,
    /// What it is.
    pub kind: DeviceKind,
    /// Its name.
    pub name: String,
    /// What it presents, such as `MQTT broker`.
    pub presents: String,
    /// Its status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Pill>,
    /// When it was last seen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_seen: Option<String>,
    /// Its response time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_time: Option<String>,
    /// The connection or runtime it belongs to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<ItemId>,
    /// Its hover card.
    pub hover: HoverCard,
    /// Its inspector.
    pub inspector: Inspector,
    /// Whether the search dims it.
    pub dimmed: bool,
    /// Whether the filter hides it.
    pub hidden: bool,
    /// Its row of the list.
    pub list: ListCells,
}

/// What a device card shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeviceKind {
    /// A system outside the fleet.
    External,
    /// A system that several runtimes use.
    Shared,
    /// A device on a fieldbus.
    SubDevice,
}

/// A link (specification 25 §5.7 and §5.13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "the flags are independent facts of the contract's JSON"
)]
pub struct Link {
    /// Its identity.
    pub id: ItemId,
    /// One end.
    pub from: ItemId,
    /// The other end.
    pub to: ItemId,
    /// Its protocol's name.
    pub protocol_label: String,
    /// Its status.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<Pill>,
    /// Solid when proven, dashed otherwise.
    pub proven: bool,
    /// Its tone.
    pub tone: Tone,
    /// Where the arrow points.
    pub arrow: ArrowAt,
    /// Whether it shows a lock.
    pub secure: bool,
    /// Its status and detail on hover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_text: Option<String>,
    /// Whether the search dims it.
    pub dimmed: bool,
    /// Whether the filter hides it.
    pub hidden: bool,
}

/// Where a link's arrow points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrowAt {
    /// At `from`.
    From,
    /// At `to`.
    To,
    /// Nowhere.
    None,
}

/// A mesh bus (specification 25 §5.7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MeshBus {
    /// Its identity.
    pub id: ItemId,
    /// Its label; only with more than one peer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// Whether it is a draft.
    pub draft: bool,
    /// Its drops.
    pub drops: Vec<MeshDrop>,
}

/// A drop of a mesh bus.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MeshDrop {
    /// The item it reaches.
    pub item: ItemId,
    /// Whether it is proven.
    pub proven: bool,
    /// Its tone.
    pub tone: Tone,
    /// Its status and detail on hover.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hover_text: Option<String>,
}

/// The inspector of an item: every fact and every action of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Inspector {
    /// `Devices & Connections / <kind>`.
    pub breadcrumb: String,
    /// The item's kind.
    pub kind: String,
    /// The item's name.
    pub title: String,
    /// The tone of its state.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub health: Option<Tone>,
    /// The larger buttons.
    pub primary: Vec<Action>,
    /// The smaller buttons.
    pub secondary: Vec<Action>,
    /// The facts.
    pub sections: Vec<Section>,
    /// The form of `Edit settings`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit: Option<String>,
}

/// The list's columns (specification 25 §5.14).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListView {
    /// The columns, in their order.
    pub columns: Vec<Column>,
}

/// A column of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Column {
    /// The key of its cells.
    pub id: String,
    /// Its heading.
    pub label: String,
}

/// An item's row of the list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ListCells {
    /// The item's kind: `computer`, `chip`, `runtime`, `connection` or `device`.
    pub kind: String,
    /// The text of each column that the item fills, by column key.
    pub cells: BTreeMap<String, String>,
}
