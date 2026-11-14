use super::*;
use crate::control::endpoint::socket_client::{self, ConnectFailed};
use crate::control::endpoint::{resolve_control_target, ControlTarget};

/// The console's connection to a runtime's socket: the shared control client, with its waits, its
/// response limit and its checks of a response (spec 11, section 6.8.1).
pub(super) struct ControlClient(trust_runtime_client::ControlClient);

impl ControlClient {
    /// Connects to `endpoint`; every request of the connection carries `token`. A failed
    /// connection keeps the operating system's error ([`ConnectFailed`]).
    pub(super) fn connect(endpoint: ControlEndpoint, token: Option<&str>) -> anyhow::Result<Self> {
        Ok(Self(socket_client::connect(&endpoint, token)?))
    }

    /// Sends `payload`, a request with its `type` and its `params`, and returns the whole
    /// response. The reply to `bytecode.reload` is awaited as long as the runtime may take to
    /// answer it, every other reply 750 milliseconds (spec 11, section 6.8.1).
    pub(super) fn request(
        &mut self,
        payload: serde_json::Value,
    ) -> anyhow::Result<serde_json::Value> {
        socket_client::exchange(&mut self.0, &payload)
    }
}

/// What the console connects to: the project folder, an explicit endpoint and the caller's token
/// (spec 11, sections 6.8.0a and 6.8.1). The target is resolved again for every connection
/// attempt, so the console follows a runtime that restarted on another port or with another token.
pub(super) struct ConsoleTarget {
    project: Option<PathBuf>,
    explicit: Option<ControlEndpoint>,
    token: Option<String>,
}

impl ConsoleTarget {
    /// `token` is the explicit `--token` or `TRUST_CTL_TOKEN`, merged by the caller. Without an
    /// endpoint, the console finds its project from `--project` or the working directory. With an
    /// endpoint, the project is the one named by `--project`, if any. It is read only for the
    /// endpoint file and for a token that the caller did not give.
    pub(super) fn new(
        bundle: Option<PathBuf>,
        endpoint: Option<String>,
        token: Option<String>,
    ) -> anyhow::Result<Self> {
        let explicit = endpoint
            .as_deref()
            .map(ControlEndpoint::parse)
            .transpose()?;
        let project = match (bundle, explicit.is_some()) {
            (Some(project), _) => Some(project),
            (None, true) => None,
            (None, false) => Some(detect_bundle_path(None).map_err(anyhow::Error::from)?),
        };
        Ok(Self {
            project,
            explicit,
            token,
        })
    }

    /// The project folder, when the console has one.
    pub(super) fn project(&self) -> Option<&Path> {
        self.project.as_deref()
    }

    /// Resolves the endpoint and the token now, from the current endpoint file.
    pub(super) fn resolve(&self) -> anyhow::Result<ControlTarget> {
        Ok(resolve_control_target(
            self.project.as_deref(),
            self.explicit.clone(),
            self.token.as_deref(),
        )?)
    }

    /// Resolves the target again and connects to it. A runtime that is not running is reported
    /// as such (spec 11, section 6.8.0a).
    pub(super) fn connect(&self) -> anyhow::Result<ControlClient> {
        connect_console(&self.resolve()?)
    }
}

/// Connects the console to a resolved target. Nothing listening at the endpoint file's endpoint
/// or at the platform default means that the runtime is not running (spec 11, section 6.8.0a).
pub(super) fn connect_console(target: &ControlTarget) -> anyhow::Result<ControlClient> {
    ControlClient::connect(target.endpoint.clone(), target.auth_token.as_deref()).map_err(|error| {
        match error.downcast_ref::<ConnectFailed>() {
            Some(failed) => anyhow::anyhow!(target.connect_error(&failed.error)),
            None => error,
        }
    })
}

pub(super) fn load_console_config(root: &Path) -> ConsoleConfig {
    let path = root.join("runtime.toml");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(_) => return ConsoleConfig::default(),
    };
    let value: toml::Value = match toml::from_str(&text) {
        Ok(value) => value,
        Err(_) => return ConsoleConfig::default(),
    };
    let console = match value.get("console") {
        Some(console) => console,
        None => return ConsoleConfig::default(),
    };
    let layout = console
        .get("layout")
        .and_then(|value| value.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| entry.as_str())
                .filter_map(PanelKind::parse)
                .collect::<Vec<_>>()
        })
        .filter(|layout| !layout.is_empty());
    let refresh_ms = console
        .get("refresh_ms")
        .and_then(|value| value.as_integer())
        .and_then(|value| u64::try_from(value).ok())
        .filter(|value| *value > 0);
    ConsoleConfig { layout, refresh_ms }
}

pub(super) fn fetch_data(client: &mut ControlClient) -> anyhow::Result<UiData> {
    let status = client.request(json!({"id": 1, "type": "status"}))?;
    let tasks = client.request(json!({"id": 2, "type": "tasks.stats"}))?;
    let io = client.request(json!({"id": 3, "type": "io.list"}))?;
    let events =
        client.request(json!({"id": 4, "type": "events.tail", "params": { "limit": 20 }}))?;
    let settings = client.request(json!({"id": 5, "type": "config.get"}))?;
    Ok(UiData {
        status: parse_status(&status),
        tasks: parse_tasks(&tasks),
        io: parse_io(&io),
        events: parse_events(&events),
        settings: parse_settings(&settings),
    })
}

#[cfg(test)]
mod tests;
