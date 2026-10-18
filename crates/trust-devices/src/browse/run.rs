//! Reading one browse at the origin that owns the connection (design §1.6 and §4.3 B8;
//! specification 25 §5.4 and §11.2). This module holds the I/O: it resolves the runtime that
//! owns the named connection, builds the `comm.browse_symbols` request, and reads either that
//! runtime's control endpoint or the open project's files. It returns typed facts; the labels
//! and the decisions are in [`panes::browse`](crate::panes::browse).

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::{json, Map, Value};
use trust_runtime::control::offline_comm_browse_symbols_project_json;
use trust_runtime_client::{ControlClient, ControlEndpoint, ControlError, RESPONSE_LINE_LIMIT};

use crate::actions::write::{self, Writes};
use crate::panes::browse::{self, Capability};
use crate::request::{BrowseRequest, Token};
use crate::session::{KnownRuntime, Session};
use crate::texts::browse as texts;
use crate::view::types::{BrowseKind, ErrorBox, ItemId};

/// The wait for one `comm.browse_symbols` on a runtime (design §1.5: 2,000 ms for each further
/// request).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

/// Why an origin's answer did not decode.
const ANSWER_DID_NOT_DECODE: &str = "The origin's browse answer did not decode.";

/// The typed answer of one `comm.browse_symbols` (specification 11, "Symbol browsing").
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Browsed {
    /// The nodes, in browse order.
    pub(crate) tree: Vec<BrowsedNode>,
    /// The protocol error the origin classified, when it classified one.
    pub(crate) error: Option<BrowsedError>,
}

/// One node of the origin's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowsedNode {
    /// Its origin-side identity.
    pub(crate) id: String,
    /// Its name.
    pub(crate) name: String,
    /// Its path.
    pub(crate) path: String,
    /// Its raw protocol type, such as the OPC UA `NodeId` of its data type.
    pub(crate) type_label: Option<String>,
    /// Its raw protocol identity, which round-trips into `comm.apply` (specification 25 §11.2).
    pub(crate) node_id: Option<String>,
    /// Its apply-ready data type.
    pub(crate) data_type: Option<String>,
    /// Whether the origin reported it writable.
    pub(crate) writable: Option<bool>,
    /// Its children.
    pub(crate) children: Vec<BrowsedNode>,
}

impl BrowsedNode {
    /// Its key in the pane's selection: the raw `node_id`, else the origin-side id, else the
    /// path (specification 25 §11.2; `nodeKey`, `W/opcuaClientModel.ts:84-86`).
    #[must_use]
    pub(crate) fn key(&self) -> &str {
        match self.node_id.as_deref() {
            Some(node_id) if !node_id.is_empty() => node_id,
            _ if !self.id.is_empty() => &self.id,
            _ => &self.path,
        }
    }
}

/// The protocol error of an origin's answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowsedError {
    /// Its stable code, such as `endpoint_unreachable`.
    pub(crate) code: String,
    /// Its human message.
    pub(crate) message: String,
}

impl Browsed {
    /// Reads one `comm.browse_symbols` answer.
    ///
    /// # Errors
    /// The answer is not an object.
    pub(crate) fn of(answer: &Value) -> Result<Self, String> {
        let Some(object) = answer.as_object() else {
            return Err(ANSWER_DID_NOT_DECODE.to_owned());
        };
        Ok(Self {
            tree: nodes(object.get("tree")),
            error: object.get("error").and_then(error_of),
        })
    }
}

/// The nodes of an answer's `tree`.
fn nodes(value: Option<&Value>) -> Vec<BrowsedNode> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(node)
        .collect()
}

/// One node of an answer's `tree` with its fields.
fn node(value: &Value) -> BrowsedNode {
    let text = |key: &str| value.get(key).and_then(Value::as_str).map(str::to_owned);
    BrowsedNode {
        id: text("id").unwrap_or_default(),
        name: text("name").unwrap_or_default(),
        path: text("path").unwrap_or_default(),
        type_label: text("type"),
        node_id: text("node_id"),
        data_type: text("data_type"),
        writable: value.get("writable").and_then(Value::as_bool),
        children: nodes(value.get("children")),
    }
}

/// The protocol error of an answer's `error`, when it carries one.
fn error_of(value: &Value) -> Option<BrowsedError> {
    let code = value.get("code").and_then(Value::as_str)?;
    Some(BrowsedError {
        code: code.to_owned(),
        message: value
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
    })
}

/// The runtime that owns the target and how the pane names it.
#[derive(Debug, Clone)]
pub(crate) struct Resolved {
    /// The runtime that owns the connection, when the source named one.
    pub(crate) runtime: Option<ItemId>,
    /// The named connection's identity, when the source names one.
    pub(crate) connection: Option<ItemId>,
    /// The connection's protocol.
    pub(crate) protocol: String,
    /// What the pane shows.
    pub(crate) kind: BrowseKind,
    /// The connection's own name.
    pub(crate) name: String,
    /// The connection's configuration, which tells what it already has.
    pub(crate) params: Map<String, Value>,
    /// The identity of this browse in the session's cache.
    pub(crate) key: String,
}

/// What one browse of the origin needs, owned for the background run (design §1.6).
#[derive(Debug, Clone)]
pub(crate) struct Run {
    /// Where the browse is read.
    at: At,
    /// The `comm.browse_symbols` request.
    params: Value,
    /// The token the extension stored for the endpoint, when there is one.
    token: Option<Token>,
}

/// Where one browse is read (design §1.6).
#[derive(Debug, Clone)]
enum At {
    /// The runtime's control endpoint, which answered.
    Endpoint(String),
    /// The open project's files, in process.
    Project(PathBuf),
}

/// Resolves the runtime that owns the target and what one browse of it needs (design §4.3 B8 and
/// §3.3 J44): the named connection comes from the session's own facts, never the connection array.
///
/// # Errors
/// The request names no source, the session has not shown the connection, its protocol offers no
/// browse, or the runtime that owns it answers nowhere and is not the open project's own runtime.
pub(crate) fn resolve(
    session: &Session,
    request: &BrowseRequest,
) -> Result<(Resolved, Run), Box<ErrorBox>> {
    let source = &request.source;
    let (runtime, protocol, params, instance_id) = if let Some(id) = source.connection() {
        let found = session.runtimes().iter().find_map(|runtime| {
            runtime
                .connection(id)
                .map(|connection| (runtime, connection))
        });
        let Some((runtime, connection)) = found else {
            return Err(failure(texts::connection_not_shown(id)));
        };
        (
            Some(runtime),
            connection.protocol.clone(),
            connection.params.clone(),
            connection.instance_id.clone(),
        )
    } else if source.found().is_some() {
        let Some(protocol) = source.protocol() else {
            return Err(failure(texts::NO_SOURCE.to_owned()));
        };
        let runtime = session.runtimes().iter().find(|runtime| runtime.project);
        (runtime, protocol.to_owned(), Map::new(), None)
    } else {
        return Err(failure(texts::NO_SOURCE.to_owned()));
    };
    let Some(capability) = browse::capability(&protocol) else {
        return Err(failure(texts::not_browsable(&protocol)));
    };
    let at = run_at(session, runtime)?;
    let token = match &at {
        At::Endpoint(endpoint) => request.host.token(endpoint).cloned(),
        At::Project(_) => None,
    };
    let wire = wire_params(
        &protocol,
        capability,
        &params,
        instance_id.as_deref(),
        request.trust_server_certificate,
    );
    let key = format!(
        "{}|{}|{}",
        source
            .connection()
            .or_else(|| source.found())
            .unwrap_or_default(),
        capability.wire_kind,
        request.port.unwrap_or(0),
    );
    let resolved = Resolved {
        runtime: runtime.map(|runtime| runtime.id.clone()),
        connection: source.connection().map(|id| ItemId(id.to_owned())),
        protocol,
        kind: capability.kind,
        name: source.connection_name().unwrap_or_default().to_owned(),
        params,
        key,
    };
    Ok((
        resolved,
        Run {
            at,
            params: wire,
            token,
        },
    ))
}

/// Reads one browse at its origin (design §1.5).
///
/// # Errors
/// The runtime refused or did not answer, the project's files could not be read, or the answer did
/// not decode.
pub(crate) fn execute(run: &Run) -> Result<Browsed, String> {
    let answer = match &run.at {
        At::Endpoint(endpoint) => send(endpoint, &run.params, run.token.as_ref())?,
        At::Project(project) => offline(project, &run.params)?,
    };
    Browsed::of(&answer)
}

/// Sends one `comm.browse_symbols` to `endpoint`, with the token the extension stored for it.
fn send(endpoint: &str, params: &Value, token: Option<&Token>) -> Result<Value, String> {
    let Some(address) = ControlEndpoint::parse(endpoint) else {
        return Err(format!("{endpoint} is not a control endpoint."));
    };
    let mut client = ControlClient::connect(&address, token.map(Token::secret), REQUEST_TIMEOUT)
        .map_err(|error| error.to_string())?
        .with_response_limit(RESPONSE_LINE_LIMIT);
    client
        .set_timeout(REQUEST_TIMEOUT)
        .map_err(|error| error.to_string())?;
    client
        .request("comm.browse_symbols", Some(params.clone()))
        .map_err(|error| match error {
            ControlError::Refused { error, .. } => error,
            other => other.to_string(),
        })
}

/// Reads one `comm.browse_symbols` from the open project's files, in process.
fn offline(project: &Path, params: &Value) -> Result<Value, String> {
    offline_comm_browse_symbols_project_json(project, params.clone())
}

/// Where a browse of `runtime` is read: its control endpoint when it answered, else the open
/// project's files when it is the project's own runtime (design §4.3 B8; specification 25 §5.4).
fn run_at(session: &Session, runtime: Option<&KnownRuntime>) -> Result<At, Box<ErrorBox>> {
    let Some(runtime) = runtime else {
        return project_at(session);
    };
    match write::writes(runtime) {
        Some(Writes::Endpoint(endpoint)) => Ok(At::Endpoint(endpoint)),
        Some(Writes::Project) => project_at(session),
        None => Err(failure(texts::runtime_did_not_answer(&runtime.name))),
    }
}

/// The open project's files, in process.
fn project_at(session: &Session) -> Result<At, Box<ErrorBox>> {
    session
        .project_folder()
        .map(|path| At::Project(path.to_path_buf()))
        .ok_or_else(|| failure(texts::NO_PROJECT.to_owned()))
}

/// One browse failure that never reached the origin, with its human text and no recovery. It is
/// boxed because the human texts and their optional recovery are much larger than the `At` they
/// ride beside.
fn failure(detail: String) -> Box<ErrorBox> {
    Box::new(ErrorBox {
        title: texts::BROWSE_FAILED.to_owned(),
        detail,
        recovery: None,
    })
}

/// One `comm.browse_symbols` request (`BrowseSymbolsRequest`,
/// `control/comm_handlers/browse_symbols.rs:32-52`): the protocol, the kind, the connection's
/// target when it has one, and its `instance_id`.
fn wire_params(
    protocol: &str,
    capability: Capability,
    params: &Map<String, Value>,
    instance_id: Option<&str>,
    trust: bool,
) -> Value {
    let mut wire = Map::new();
    wire.insert("protocol".to_owned(), Value::String(protocol.to_owned()));
    wire.insert(
        "kind".to_owned(),
        Value::String(capability.wire_kind.to_owned()),
    );
    if capability.local {
        wire.insert("target".to_owned(), json!({ "local": true }));
    } else if capability.kind == BrowseKind::Nodes {
        wire.insert(
            "target".to_owned(),
            Value::Object(opcua_target(params, trust)),
        );
    }
    if let Some(instance_id) = instance_id.filter(|value| !value.trim().is_empty()) {
        wire.insert(
            "instance_id".to_owned(),
            Value::String(instance_id.to_owned()),
        );
    }
    Value::Object(wire)
}

/// The known fields of an OPC UA `comm.browse_symbols` target (`BrowseTarget`,
/// `control/comm_handlers/browse_symbols.rs:54-83`), with the request's trust flag. Only the
/// fields the origin reads are sent, so the connection's unrelated parameters never reach it.
pub(crate) fn opcua_target(params: &Map<String, Value>, trust: bool) -> Map<String, Value> {
    let mut target = Map::new();
    for key in [
        "endpoint_url",
        "security_policy",
        "security_mode",
        "auth",
        "username",
        "password",
        "name",
    ] {
        if let Some(value) = params.get(key) {
            target.insert(key.to_owned(), value.clone());
        }
    }
    target.insert("trust_server_certificate".to_owned(), Value::Bool(trust));
    target
}
