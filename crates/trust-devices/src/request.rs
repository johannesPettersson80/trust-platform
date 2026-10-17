//! What the extension sends with a Devices & Connections request (design §1.4.1): the panel's
//! session and view, and the facts that only VS Code knows, such as its settings, the stored
//! tokens and the debug session. The extension only collects these facts; the rules that use
//! them are in this crate.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;

use serde::Deserialize;

/// One `trust-lsp.devicesView` request.
#[derive(Debug, Clone, Deserialize)]
pub struct ViewRequest {
    /// The panel's session.
    pub session: String,
    /// The request's number; each request of a session has a larger one.
    pub seq: u64,
    /// Which inputs the request reads again.
    #[serde(default)]
    pub gather: GatherMode,
    /// The project chosen in the project list; absent until the user chooses one.
    #[serde(default)]
    pub project: Option<PathBuf>,
    /// The workspace folders, in the order VS Code lists them.
    #[serde(default)]
    pub workspace_folders: Vec<PathBuf>,
    /// What the panel shows.
    #[serde(default)]
    pub view: ViewOptions,
    /// The facts that only VS Code knows.
    #[serde(default)]
    pub host: HostFacts,
}

/// Which inputs a request reads again.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GatherMode {
    /// `auto`: every input older than its period.
    #[default]
    Auto,
    /// `now`: every input, as after an action.
    Now,
    /// `none`: no input; the answer comes from what the session holds, as while typing a search.
    #[serde(rename = "none")]
    Cached,
}

/// What the panel shows.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct ViewOptions {
    /// The search text.
    pub search: String,
    /// The protocols that the filter hides.
    pub hidden_protocols: Vec<String>,
    /// The selected item's id.
    pub selected: Option<String>,
}

/// The facts that only VS Code knows. The extension reads them and sends them unchanged.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct HostFacts {
    /// The extension's `trust-runtime` executable.
    pub runtime_executable: Option<PathBuf>,
    /// The `trust.runtime` settings.
    pub settings: HostSettings,
    /// The stored token of each endpoint (specification 25 §6.4).
    pub tokens: BTreeMap<String, Token>,
    /// The name that the user gave each added endpoint.
    pub endpoint_labels: BTreeMap<String, String>,
    /// The selected run target's id.
    pub run_target: Option<String>,
    /// The Structured Text debug session, while one exists.
    pub debug_session: Option<DebugSessionFacts>,
    /// The simulator's control endpoint and token, when the extension set them for the debug
    /// session.
    pub simulator: Option<SimulatorFacts>,
    /// The simulator's current I/O values.
    pub io_state: Vec<IoValue>,
}

impl HostFacts {
    /// The stored token of `endpoint`.
    #[must_use]
    pub fn token(&self, endpoint: &str) -> Option<&Token> {
        self.tokens.get(endpoint)
    }

    /// The name that the user gave `endpoint`.
    #[must_use]
    pub fn label(&self, endpoint: &str) -> Option<&str> {
        self.endpoint_labels.get(endpoint).map(String::as_str)
    }
}

/// The `trust.runtime` settings that choose the runtimes the panel reaches.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct HostSettings {
    /// `trust.runtime.mode`.
    pub mode: RunMode,
    /// `trust.runtime.controlEndpoint`.
    pub control_endpoint: String,
    /// `trust.runtime.controlEndpointEnabled`.
    pub use_control_endpoint: bool,
    /// `trust.runtime.fleetEndpoints`: the runtimes added with `Connect existing runtime`.
    pub additional_endpoints: Vec<String>,
    /// `trust.ads.discoveryPorts`.
    pub ads_discovery_ports: Vec<u16>,
}

impl Default for HostSettings {
    fn default() -> Self {
        Self {
            mode: RunMode::Simulate,
            control_endpoint: String::new(),
            use_control_endpoint: true,
            additional_endpoints: Vec::new(),
            ads_discovery_ports: Vec::new(),
        }
    }
}

/// Whether the project runs the simulator or the runtime of its control endpoint.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(from = "String")]
pub enum RunMode {
    /// The simulator. Every mode other than `online` means the simulator.
    #[default]
    Simulate,
    /// The runtime at `trust.runtime.controlEndpoint`.
    Online,
}

impl From<String> for RunMode {
    fn from(mode: String) -> Self {
        if mode == "online" {
            Self::Online
        } else {
            Self::Simulate
        }
    }
}

/// The Structured Text debug session.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DebugSessionFacts {
    /// Whether the session launched the simulator or attached to a runtime.
    pub kind: DebugSessionKind,
    /// The control endpoint that the session talks to.
    #[serde(default)]
    pub endpoint: Option<String>,
    /// The session's name.
    #[serde(default)]
    pub label: Option<String>,
    /// Whether the session is still starting.
    #[serde(default)]
    pub starting: bool,
    /// Why the last start failed.
    #[serde(default)]
    pub failure: Option<DebugFailure>,
}

/// How a debug session reached its runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DebugSessionKind {
    /// It launched the simulator.
    Launch,
    /// It attached to a running runtime.
    Attach,
}

/// Why a debug session did not start (specification 25 §5.10).
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct DebugFailure {
    /// `missing_binary`, `port_conflict`, `workspace_permission`, `stale_runtime` or
    /// `failed_spawn`.
    pub kind: String,
    /// The text for the user.
    pub message: String,
}

/// The simulator's control endpoint and token, which the extension sets for the debug session.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SimulatorFacts {
    /// The control endpoint.
    pub endpoint: String,
    /// The token.
    #[serde(default)]
    pub token: Option<Token>,
}

/// One I/O value of the simulator.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct IoValue {
    /// The variable's name.
    pub name: String,
    /// Its I/O address, such as `%IX0.0`.
    pub address: String,
    /// Its value as text.
    pub value: String,
}

/// A control token. Its debug output never shows the secret.
#[derive(Clone, PartialEq, Eq, Deserialize)]
#[serde(transparent)]
pub struct Token(String);

impl Token {
    /// A token with the text `secret`.
    #[must_use]
    pub fn new(secret: impl Into<String>) -> Self {
        Self(secret.into())
    }

    /// The secret, for the `auth` of a request.
    #[must_use]
    pub fn secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("<redacted>")
    }
}

/// One `trust-lsp.devicesAct` request (design §1.4.1): the action a view offered, the values the
/// user typed, and the facts that only VS Code knows.
#[derive(Debug, Clone, Deserialize)]
pub struct ActRequest {
    /// The panel's session.
    pub session: String,
    /// The action that the view offered.
    pub op: ActOp,
    /// The form values that the user typed, by field.
    #[serde(default)]
    pub values: serde_json::Map<String, serde_json::Value>,
    /// The chosen project, which is also the fleet root of the runtimes this computer manages
    /// (specification 25 §5.11).
    #[serde(default)]
    pub project: Option<PathBuf>,
    /// The facts that only VS Code knows.
    #[serde(default)]
    pub host: HostFacts,
}

/// The action of an [`ActRequest`]: the name and the payload that the view chose.
#[derive(Debug, Clone, Deserialize)]
pub struct ActOp {
    /// The action's name, such as `runtime.connect`.
    pub action: String,
    /// Its arguments, chosen by Rust.
    #[serde(default)]
    pub payload: serde_json::Value,
}

#[cfg(test)]
mod tests;
