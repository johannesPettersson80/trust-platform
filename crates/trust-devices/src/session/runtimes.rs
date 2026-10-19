//! What the last view showed of each runtime (design §4.3 B7): the panel's session keeps one
//! entry per runtime, so that an action resolves the runtime it writes to, and the connection and
//! instance ID it names, from the panel's own facts and never from a fact the extension sent
//! (specification 25 §5.14).

use serde_json::{Map, Value};

use crate::gather::Gathered;
use crate::panes::forms::Catalog;
use crate::topology;
use crate::view::types::{ItemId, Mark, Runtime};

/// One runtime, as the last view showed it.
#[derive(Debug, Clone, PartialEq)]
pub struct KnownRuntime {
    /// Its identity.
    pub id: ItemId,
    /// The name its card shows.
    pub name: String,
    /// The control endpoint the panel reaches it through, which its actions write to; absent when
    /// it has none.
    pub control_endpoint: Option<String>,
    /// Whether it answered `status` this round.
    pub answers: bool,
    /// Whether it is the open project's own runtime, whose files can be written in process.
    pub project: bool,
    /// The catalog its runtime serves, else the tool's own (specification 25 §5.14); `None` when
    /// neither is available.
    pub catalog: Option<Catalog>,
    /// Its connections, in the order its card showed them.
    pub connections: Vec<KnownConnection>,
}

impl KnownRuntime {
    /// What the last view showed of `runtime` (design §4.3 B7). `gathered` is the gather that the
    /// view was projected from, which tells the catalog the runtime serves.
    #[must_use]
    pub fn of(gathered: &Gathered, runtime: &Runtime) -> Self {
        Self {
            id: runtime.id.clone(),
            name: runtime.name.clone(),
            control_endpoint: runtime.control_endpoint.clone(),
            answers: runtime.answers,
            project: runtime.project,
            catalog: Catalog::for_runtime(gathered, runtime.control_endpoint.as_deref()),
            connections: runtime
                .connections
                .iter()
                .map(|connection| KnownConnection {
                    id: connection.id.clone(),
                    instance_id: topology::connection_instance_id(connection.id.as_str()),
                    protocol: connection.protocol.clone(),
                    params: connection
                        .params
                        .as_ref()
                        .and_then(Value::as_object)
                        .cloned()
                        .unwrap_or_default(),
                    disabled: connection.marks.contains(&Mark::Disabled),
                })
                .collect(),
        }
    }

    /// Whether a connection can be saved to it: over its control endpoint when it answers, or,
    /// for the open project's own runtime, in process into the project's files
    /// (specification 25 §5.14).
    #[must_use]
    pub fn accepts_connections(&self) -> bool {
        self.project || (self.answers && self.control_endpoint.is_some())
    }

    /// The connection that the last view showed under the item ID `id`; `None` when it showed no
    /// such connection.
    #[must_use]
    pub fn connection(&self, id: &str) -> Option<&KnownConnection> {
        self.connections
            .iter()
            .find(|connection| connection.id.as_str() == id)
    }
}

/// One connection of a runtime, as the last view showed it.
#[derive(Debug, Clone, PartialEq)]
pub struct KnownConnection {
    /// Its item ID.
    pub id: ItemId,
    /// The `instance_id` that a `comm.apply` addresses it with (specification 11 §6.8.1a);
    /// absent when the item ID names no connection of a runtime.
    pub instance_id: Option<String>,
    /// Its protocol.
    pub protocol: String,
    /// Its configuration without secrets.
    pub params: Map<String, Value>,
    /// Whether it is disabled.
    pub disabled: bool,
}
