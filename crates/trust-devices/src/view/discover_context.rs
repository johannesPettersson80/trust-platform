//! What the Discover pane knows beside one search status (specification 25 §2, decision O14): the
//! project's remembered choices and ignored finds, the panel's `Show ignored` tick, the ADS connections on
//! the canvas, the configured ADS ports, and what each origin runtime reports: its role, the protocols its
//! `comm.schema` lists with `discover`, and its `EtherCAT` adapter. The engine gathers it; the view reads it.
//! `Context::default()` is a view with no remembered choice, nothing ignored and no runtime facts: the
//! first run ticks ADS, no role limits a scan, and no advertised protocol limits a card.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::Value;
use trust_runtime::search::SourceId;
use trust_runtime::security::AccessRole;

use crate::texts::discover as texts;
use trust_runtime::search::project_choices::Memory;

/// What the view reads about one origin runtime.
#[derive(Debug, Clone, Default)]
pub(crate) struct OriginFacts {
    /// The role of the connection to the runtime, when its status reports one.
    pub(crate) role: Option<AccessRole>,
    /// The adapter of the runtime's `EtherCAT` connection, when it has one.
    pub(crate) ethercat_adapter: Option<String>,
    /// The protocols its `comm.schema` lists with `discover`; `None` while the schema is unknown.
    pub(crate) discover: Option<Discover>,
}

/// The protocols with `discover` in one runtime's `comm.schema` answer.
#[derive(Debug, Clone, Default)]
pub(crate) struct Discover {
    /// Every protocol ID that the runtime lists with `discover`.
    pub(crate) ids: BTreeSet<String>,
    /// Those that are not one of the ten sources, in the order the schema lists them.
    pub(crate) beyond: Vec<Advertised>,
}

/// A protocol that a runtime advertises beyond the ten sources.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Advertised {
    /// Its protocol ID, as the schema names it.
    pub(crate) id: String,
    /// Its advertised label, which the choice shows.
    pub(crate) label: String,
}

/// What the view reads beside a search status.
#[derive(Debug, Clone)]
pub(crate) struct Context {
    /// What the project remembers: the ticks and the ignored finds.
    pub(crate) memory: Memory,
    /// Whether the finds that the project ignores are shown (`Show ignored`).
    pub(crate) show_ignored: bool,
    /// Whether the search was started by this panel session; a find of another session is not current.
    pub(crate) session_current: bool,
    /// The AMS Net IDs of the ADS connections on the canvas.
    pub(crate) ads_on_canvas: BTreeSet<String>,
    /// The ADS ports the project configures (`trust.ads.discoveryPorts`); empty for the default ports.
    pub(crate) ads_ports: Vec<u16>,
    /// The facts of each origin runtime, by its item ID.
    pub(crate) origins: BTreeMap<String, OriginFacts>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            memory: Memory::default(),
            show_ignored: false,
            session_current: true,
            ads_on_canvas: BTreeSet::new(),
            ads_ports: Vec::new(),
            origins: BTreeMap::new(),
        }
    }
}

impl Context {
    /// The facts of the origin runtime `id`, when the view has them.
    #[must_use]
    pub(crate) fn origin(&self, id: &str) -> Option<&OriginFacts> {
        self.origins.get(id)
    }
}

/// The protocols that a `comm.schema` answer lists with `discover` (`discoverableProtocols`), and those of
/// them that are not one of the ten sources.
#[must_use]
pub(crate) fn discover_from_schema(schema: &Value) -> Discover {
    let mut discover = Discover::default();
    let protocols = schema
        .get("protocols")
        .and_then(Value::as_array)
        .into_iter()
        .flatten();
    for protocol in protocols {
        let Some(id) = protocol.get("id").and_then(Value::as_str) else {
            continue;
        };
        let lists_discover = protocol
            .get("actions")
            .and_then(Value::as_array)
            .is_some_and(|actions| {
                actions
                    .iter()
                    .any(|action| action.as_str() == Some("discover"))
            });
        if !lists_discover {
            continue;
        }
        discover.ids.insert(id.to_owned());
        if schema_source(id).is_none() {
            let label = protocol
                .get("label")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|label| !label.is_empty())
                .map_or_else(|| texts::source_name(id), str::to_owned);
            discover.beyond.push(Advertised {
                id: id.to_owned(),
                label,
            });
        }
    }
    discover
}

/// The source that a `comm.schema` protocol ID stands for: `discovery` is truST runtimes and `opcua_client`
/// is OPC UA; the other IDs carry the source's own name. `None` for an ID that is not one of the ten.
#[must_use]
pub(crate) fn schema_source(id: &str) -> Option<SourceId> {
    match id {
        "discovery" => Some(SourceId::TrustRuntime),
        "opcua_client" => Some(SourceId::Opcua),
        other => SourceId::ALL
            .into_iter()
            .find(|source| source.as_str() == other),
    }
}

/// The `comm.schema` protocol ID of a source whose discover action the schema lists. Serial, USB and
/// Bluetooth have none, so no card from them is checked against a schema.
#[must_use]
pub(crate) fn schema_id(source: SourceId) -> Option<&'static str> {
    match source {
        SourceId::TrustRuntime => Some("discovery"),
        SourceId::Opcua => Some("opcua_client"),
        SourceId::Ads => Some("ads"),
        SourceId::ModbusTcp => Some("modbus_tcp"),
        SourceId::Mqtt => Some("mqtt"),
        SourceId::Ethercat => Some("ethercat"),
        SourceId::Gpio => Some("gpio"),
        SourceId::Serial | SourceId::Usb | SourceId::Bluetooth => None,
    }
}
