//! The view-model contract between the panel engine and the webview (design §2.1). Rust returns
//! one `DevicesView` per request: every item with its product words, its states, the actions it
//! allows with the reasons for the disabled ones, and its inspector. The webview draws it and
//! sends back the `op` of the action the user chose.
//!
//! The JSON field names are the Rust field names, enums are `snake_case`, and an absent `Option`
//! is omitted. `Op`, `UiOp`, `Pane` and `HostEffect` are tagged by `kind`. `SCHEMA_VERSION`
//! starts at 1: a field may be added, while renaming or removing one raises the version.
//!
//! This module holds the vocabulary that every item uses: identities, tones, pills, rows,
//! actions and what they do. `graph` holds the panel and the items of its graph, and `panes`
//! the panes beside it.

mod graph;
mod panes;

use std::fmt;

use serde::Serialize;
use serde_json::Value;

/// The version of this contract.
pub const SCHEMA_VERSION: u32 = 1;

/// The stable, injective, owner-scoped identity of an item (specification 25 §5.6): the scheme
/// `trust-fleet-v1:` and the unpadded base64url form of a JSON array, or a fixed identity of a
/// drawn item such as `runtime:local`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

impl ItemId {
    /// The identity as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A secret that a host effect hands to VS Code; its debug output never shows it.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Secret(pub String);

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

/// The colour role of a status. Every tone comes with a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    /// Running, connected or reachable.
    Ok,
    /// Needs attention.
    Warning,
    /// Failed.
    Error,
    /// Neither good nor bad, such as stopped or not proven yet.
    Neutral,
    /// Drawn quietly, such as a draft.
    Muted,
}

/// How an item is drawn: solid, dashed while not proven, or as a draft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Look {
    /// Solid.
    Normal,
    /// Dashed and never green: not proven yet (specification 25 §5.3 and §5.5).
    Pending,
    /// Marked `DRAFT`.
    Draft,
}

/// A status word with its tone; `live` draws a pulsing dot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Pill {
    /// The word.
    pub word: String,
    /// Its colour role.
    pub tone: Tone,
    /// Whether the dot pulses.
    pub live: bool,
}

/// One line of an inspector section or a hover card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Row {
    /// What the line is about.
    pub label: String,
    /// Its value.
    pub value: String,
    /// The value's tone, when it carries one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tone: Option<Tone>,
    /// Whether the value takes the whole width, such as a list of connections.
    pub wide: bool,
    /// The text that a copy button copies, when the line has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copy: Option<String>,
}

/// A group of rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Section {
    /// The group's heading; the first section has none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// The rows.
    pub rows: Vec<Row>,
}

/// What shows while the pointer rests on an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HoverCard {
    /// The item's name.
    pub title: String,
    /// Its facts.
    pub rows: Vec<Row>,
}

/// An action that an item offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Action {
    /// Stable within the item, such as `connect` or `test`.
    pub id: String,
    /// The button's text.
    pub label: String,
    /// Whether it can be chosen.
    pub enabled: bool,
    /// Why it cannot; always present when `enabled` is false (specification 25 §5.1 and §5.4).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// How strongly it is drawn.
    pub emphasis: Emphasis,
    /// The confirmation of a two-step action.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirm: Option<Confirm>,
    /// What choosing it does.
    pub op: Op,
}

/// How strongly an action is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Emphasis {
    /// Filled: an enabled Start or Connect, or the recovery after a failure.
    Primary,
    /// Outlined.
    Secondary,
    /// Outlined in the error role, such as Remove.
    Danger,
    /// Neutral, such as a disabled action.
    Quiet,
}

/// The second step of a two-step action.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[allow(
    clippy::struct_field_names,
    reason = "the contract names the confirming button `confirm`"
)]
pub struct Confirm {
    /// What happens.
    pub text: String,
    /// The confirming button.
    pub confirm: String,
    /// The cancelling button.
    pub cancel: String,
}

/// What an action does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Op {
    /// A Rust action, sent unchanged to `trust-lsp.devicesAct`.
    Rust {
        /// The action's name, such as `runtime.connect`.
        action: String,
        /// Its arguments, chosen by Rust.
        payload: Value,
    },
    /// A VS Code call that decides nothing.
    Host {
        /// The call.
        effect: HostEffect,
    },
    /// A change of the webview's own view.
    Ui {
        /// The change.
        ui: UiOp,
    },
}

/// A change of the webview's own view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiOp {
    /// Zooms to the item.
    Focus {
        /// The item.
        item: ItemId,
    },
    /// Opens a right-hand pane.
    Open {
        /// The pane.
        pane: Pane,
    },
    /// Closes the inspector.
    CloseInspector,
}

/// A right-hand pane. One pane is open at a time (specification 25 §5.11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Pane {
    /// `+ Add`, for a runtime or a computer, or for neither.
    Add {
        /// The runtime that the new connection belongs to.
        #[serde(skip_serializing_if = "Option::is_none")]
        runtime: Option<ItemId>,
        /// The computer that a new runtime goes on.
        #[serde(skip_serializing_if = "Option::is_none")]
        host: Option<ItemId>,
    },
    /// A form, which `form.open` returns.
    Form {
        /// The form's identity.
        form: String,
    },
    /// The search.
    Discover,
    /// The protocol filter.
    Filter,
    /// The issues list.
    Issues,
    /// A browse pane; `request` is the `trust-lsp.devicesBrowse` body.
    Browse {
        /// The request.
        request: Value,
    },
    /// The installation of truST on a chip found on USB.
    Install {
        /// The USB item.
        usb: String,
    },
}

/// The closed list of VS Code calls; the extension runs each exactly as given.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HostEffect {
    /// Starts the simulator's debug session.
    StartSimulator,
    /// Ends the debug or attach session.
    StopSession,
    /// Attaches to a runtime.
    Attach {
        /// Its control endpoint.
        endpoint: String,
        /// Its friendly name.
        label: String,
    },
    /// Starts debugging: the simulator without an endpoint, a runtime at its endpoint.
    Debug {
        /// The runtime's control endpoint.
        #[serde(skip_serializing_if = "Option::is_none")]
        endpoint: Option<String>,
    },
    /// Selects the run target.
    SetRunTarget {
        /// The run target's identity: `simulator`, a managed runtime's name or an endpoint.
        id: String,
    },
    /// Asks for an endpoint's token and stores it in the secret store.
    PromptToken {
        /// The endpoint.
        endpoint: String,
    },
    /// Stores a token in the secret store.
    StoreSecret {
        /// The endpoint.
        endpoint: String,
        /// The token.
        token: Secret,
    },
    /// Stores an endpoint's friendly name.
    StoreLabel {
        /// The endpoint.
        endpoint: String,
        /// The name.
        label: String,
    },
    /// Writes a setting.
    WriteSetting {
        /// The setting's key.
        key: String,
        /// Its value.
        value: Value,
        /// Where it is written.
        target: SettingTarget,
    },
    /// Opens Live Values.
    OpenLiveValues,
    /// Opens the runtime settings.
    OpenRuntimeSettings,
    /// Opens the simulator's debug log.
    OpenDebugLog,
    /// Shows lines in an output channel.
    ShowOutput {
        /// The channel.
        channel: String,
        /// The lines.
        lines: Vec<String>,
    },
    /// Opens a file.
    OpenDocument {
        /// The file.
        path: String,
    },
    /// Copies text.
    Copy {
        /// The text.
        text: String,
    },
    /// Opens VS Code's settings at a query.
    OpenSettings {
        /// The query.
        query: String,
    },
    /// Raises a notification; only for failures (specification 25 §5.13).
    Notify {
        /// How severe it is.
        severity: Severity,
        /// The text.
        text: String,
    },
    /// Selects an item.
    Select {
        /// The item.
        item: ItemId,
    },
}

/// Where a setting is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettingTarget {
    /// The workspace.
    Workspace,
    /// The user's settings.
    Global,
}

/// How severe an issue or a notification is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// A fault.
    Error,
    /// Needs attention.
    Warning,
    /// For information.
    Info,
}

/// The result of an earlier action, shown in the pane that started it (specification 25 §5.13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notice {
    /// The pane.
    pub pane: String,
    /// The result's tone.
    pub tone: Tone,
    /// The result.
    pub text: String,
    /// More about it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The fields that were refused.
    pub field_errors: Vec<FieldError>,
    /// What can be done next.
    pub actions: Vec<Action>,
}

/// A refused field of a form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FieldError {
    /// The field.
    pub field: String,
    /// Why.
    pub message: String,
}

pub use graph::{
    ArrowAt, Banner, Column, Computer, ComputerKind, Connection, Container, Device, DeviceKind,
    DevicesView, EmptyHint, FilterLine, FilterProtocol, FilterReport, Inspector, Issue, IssueGroup,
    Issues, IssuesButton, Link, ListCells, ListView, Mark, MeshBus, MeshDrop, Network, ProjectBar,
    ProjectChoice, Runtime, RuntimeIcon, SearchEcho, Summary, Toolbar, ViewSwitch,
};
pub use panes::ListProperty;
pub use panes::{
    AddButton, AddDialog, AddEntry, AddGroup, AdsPortCard, AdvancedToggle, BrowseKind, BrowseView,
    CardProtocol, CompareView, DeviceCard, DiscoverView, ErrorBox, FieldKind, FieldOption,
    FieldbusOffer, Form, FormField, FormSection, FoundEntry, GhostTile, InstallStep, InstallView,
    ListEditor, OnlyChoice, OriginChoice, Progress, QuickStart, RouteHelp, RuntimeRef, SourceRow,
    StepState, TargetsInput, Toggle, TreeNode, UsbItem, UsbTray, VisibleWhen,
};
