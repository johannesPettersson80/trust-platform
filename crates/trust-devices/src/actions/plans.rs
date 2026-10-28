//! The connection plans of the panel's `act` (design §4.3 B7; inventories I14, I16, I17, I18, I25,
//! I37 and I71): exposing globals through the server that owns them, the `EtherCAT` channel
//! selection, an OPC UA client connection built from the selected nodes, and one ADS connection per
//! responding port.
//!
//! Each plan is one action whose `comm.apply` payload Rust chooses, written through the one write
//! path of `write` to the runtime that [`Session`](crate::session::Session) last showed, as
//! `config`'s `connection.save` does: over the runtime's control endpoint when it answers, else in
//! process into the open project's files. Its result is a notice in the pane that started it
//! (specification 25 §5.8, §5.13 and §5.14).
//!
//! The planners (`expose_params`, `ethercat_selection`, `opcua_connection` and
//! `ads_device_connections`) are pure: they hold the ported TypeScript's rules and touch no socket
//! (design §1.6).

use serde_json::{json, Map, Value};

use super::write;
use crate::request::ActRequest;
use crate::session::{KnownRuntime, Session};
use crate::texts::connections::protocol_name;
use crate::texts::forms as form_texts;
use crate::texts::plans as plan_texts;
use crate::view::types::{ActReply, Refresh, Tone};

/// The derived topology keys that a server's re-applied configuration drops
/// (`NC/exposeConfig.ts:1`; specification 25 §5.8).
const DERIVED_TOPOLOGY_PARAM_KEYS: [&str; 2] = ["clients_count", "clients_summary"];

/// The action's payload did not name the protocol.
const NO_PROTOCOL: &str = "The action did not name the protocol.";

/// What exposing globals through a server writes: the names as the server's own globals, and the
/// connection's parameters with the exposure merged in (`NC/exposeConfig.ts:13-32`).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ExposePlan {
    /// The chosen globals, without the `global.` prefix.
    pub(crate) names: Vec<String>,
    /// The server's parameters with `expose` (and `writable`) merged in, and the derived
    /// topology-only and secret-presence keys dropped (specification 25 §5.8).
    pub(crate) params: Map<String, Value>,
}

/// `connection.expose`: merges the chosen globals into the server that owns them, over the runtime
/// the session resolves (`NC/protocolActions.ts:158-206`; specification 25 §5.8).
#[must_use]
pub(crate) fn expose(session: &Session, request: &ActRequest) -> ActReply {
    let pane = plan_pane(request);
    let Some(protocol) = super::payload_text(&request.op.payload, "protocol") else {
        return super::error(&pane, NO_PROTOCOL);
    };
    let runtime = match known_runtime(session, request) {
        Ok(runtime) => runtime,
        Err(message) => return super::error(&pane, &message),
    };
    let Some(current) = server_params(runtime, request) else {
        return super::error(
            &pane,
            &plan_texts::expose_not_configured(&protocol_name(&protocol)),
        );
    };
    let plan = expose_params(&current, &paths_of(request), writable_of(request));
    match apply(
        runtime,
        request,
        &apply_request(&protocol, "upsert", &plan.params),
    ) {
        Applied::Ok(answer) => super::reply(
            true,
            Some(super::notice(
                Tone::Ok,
                &pane,
                plan_texts::exposed(&protocol, plan.names.len(), restart_required(&answer)),
            )),
            Vec::new(),
            false,
            Refresh::Now,
        ),
        Applied::Failed(reason) => failure(&pane, plan_texts::expose_failed(reason.as_deref())),
    }
}

/// `connection.ethercat_channels`: writes the selected channels on the `EtherCAT` bus that the
/// payload names (`NC/protocolActions.ts:248-295`).
#[must_use]
pub(crate) fn ethercat_channels(session: &Session, request: &ActRequest) -> ActReply {
    let pane = plan_pane(request);
    let runtime = match known_runtime(session, request) {
        Ok(runtime) => runtime,
        Err(message) => return super::error(&pane, &message),
    };
    let Some(mut params) = server_params(runtime, request) else {
        return super::error(&pane, plan_texts::ETHERCAT_NOT_CONFIGURED);
    };
    let selected = ethercat_selection(&paths_of(request));
    params.insert("selected_channels".to_owned(), strings(&selected));
    match apply(
        runtime,
        request,
        &apply_request("ethercat", "upsert", &params),
    ) {
        Applied::Ok(answer) => super::reply(
            true,
            Some(super::notice(
                Tone::Ok,
                &pane,
                plan_texts::ethercat_selected(selected.len(), restart_required(&answer)),
            )),
            Vec::new(),
            false,
            Refresh::Now,
        ),
        Applied::Failed(reason) => failure(&pane, plan_texts::ethercat_failed(reason.as_deref())),
    }
}

/// `connection.opcua_add`: builds an OPC UA client connection from the selected nodes and adds it
/// to the runtime (`NC/protocolActions.ts:208-246`; `W/opcuaClientModel.ts:121-155`).
#[must_use]
pub(crate) fn opcua_add(session: &Session, request: &ActRequest) -> ActReply {
    let pane = plan_pane(request);
    let runtime = match known_runtime(session, request) {
        Ok(runtime) => runtime,
        Err(message) => return super::error(&pane, &message),
    };
    let label = super::payload_text(&request.op.payload, "label").unwrap_or_default();
    let target = object_of(&request.op.payload, "target");
    let nodes = array_of(request.values.get("nodes"));
    let Some(connection) = opcua_connection(&target, &label, &nodes, writable_of(request)) else {
        return failure(&pane, plan_texts::opcua_failed(None));
    };
    let points = connection
        .get("points")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let params = plan_params(vec![connection]);
    match apply(
        runtime,
        request,
        &apply_request("opcua_client", "add", &params),
    ) {
        Applied::Ok(answer) => super::reply(
            true,
            Some(super::notice(
                Tone::Ok,
                &pane,
                plan_texts::opcua_added(points, restart_required(&answer)),
            )),
            Vec::new(),
            false,
            Refresh::Now,
        ),
        Applied::Failed(reason) => failure(&pane, plan_texts::opcua_failed(reason.as_deref())),
    }
}

/// `connection.ads_add`: adds one ADS connection per responding port of the discovered device to
/// the runtime (`NC/protocolActions.ts:46-104`).
#[must_use]
pub(crate) fn ads_add(session: &Session, request: &ActRequest) -> ActReply {
    let pane = plan_pane(request);
    let runtime = match known_runtime(session, request) {
        Ok(runtime) => runtime,
        Err(message) => return super::error(&pane, &message),
    };
    let label = super::payload_text(&request.op.payload, "label")
        .unwrap_or_else(|| plan_texts::ADS_DEVICE.to_owned());
    let target = object_of(&request.op.payload, "target");
    let connections = match ads_device_connections(&target, &label) {
        Ok(connections) => connections,
        Err(message) => return failure(&pane, message),
    };
    let count = connections.len();
    let params = plan_params(connections);
    match apply(runtime, request, &apply_request("ads", "add", &params)) {
        Applied::Ok(answer) => super::reply(
            true,
            Some(super::notice(
                Tone::Ok,
                &pane,
                plan_texts::ads_added(count, restart_required(&answer)),
            )),
            Vec::new(),
            false,
            Refresh::Now,
        ),
        Applied::Failed(reason) => failure(&pane, plan_texts::ads_failed(reason.as_deref())),
    }
}

/// The pane that started the plan: the browse pane the payload names, else what the panel would
/// address (`super::pane`). A browse pane is not a form, so its action carries its own pane.
fn plan_pane(request: &ActRequest) -> String {
    super::payload_text(&request.op.payload, "pane").unwrap_or_else(|| super::pane(request))
}

/// The parameters of the server or bus connection that the payload names, as the session last
/// showed them; `None` when the runtime the session showed has no such connection.
fn server_params(runtime: &KnownRuntime, request: &ActRequest) -> Option<Map<String, Value>> {
    let id = super::payload_text(&request.op.payload, "connection")?;
    runtime
        .connection(&id)
        .map(|connection| connection.params.clone())
}

/// The runtime that the payload names, as the session last showed it; a runtime the session does
/// not know is a refusal naming it (design §4.3 B7).
fn known_runtime<'a>(
    session: &'a Session,
    request: &ActRequest,
) -> Result<&'a KnownRuntime, String> {
    let Some(id) = super::payload_text(&request.op.payload, "runtime") else {
        return Err(form_texts::NO_RUNTIME.to_owned());
    };
    session
        .runtime(&id)
        .ok_or_else(|| form_texts::runtime_not_shown(&id))
}

/// What one plan's `comm.apply` did.
enum Applied {
    /// The runtime applied it; its own answer.
    Ok(Value),
    /// It did not apply: the runtime's message, else the plan's own default.
    Failed(Option<String>),
}

/// Sends one plan's `comm.apply` to the runtime, through the one write path of `write`: over its
/// control endpoint when it answers, else in process into the open project's files. A runtime that
/// answers nowhere and is not the open project's own is refused by name.
fn apply(runtime: &KnownRuntime, request: &ActRequest, params: &Value) -> Applied {
    let Some(writes) = write::writes(runtime) else {
        return Applied::Failed(Some(form_texts::runtime_did_not_answer(&runtime.name)));
    };
    match write::apply(request, &writes, params) {
        Err(message) => Applied::Failed(Some(message)),
        Ok(answer) => {
            let fields = field_messages(&answer);
            if !fields.is_empty() {
                Applied::Failed(Some(fields))
            } else if applied(&answer) {
                Applied::Ok(answer)
            } else {
                Applied::Failed(super::payload_text(&answer, "message"))
            }
        }
    }
}

/// Whether the runtime applied the change; a `comm.apply` answer always says so.
fn applied(answer: &Value) -> bool {
    answer.get("applied").and_then(Value::as_bool) == Some(true)
}

/// Whether the runtime must restart to apply the change (design §1.4.1).
fn restart_required(answer: &Value) -> bool {
    answer.get("lifecycle_effect").and_then(Value::as_str) == Some("restart_required")
}

/// The messages of the refused fields of an answer, joined as the panel shows them
/// (`NC/protocolActions.ts:196-197`).
fn field_messages(answer: &Value) -> String {
    answer
        .get("field_errors")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|error| error.get("message").and_then(Value::as_str))
        .collect::<Vec<&str>>()
        .join("; ")
}

/// One `comm.apply` payload, as the runtime and the offline applier read it.
fn apply_request(protocol: &str, action: &str, params: &Map<String, Value>) -> Value {
    json!({ "protocol": protocol, "action": action, "params": params })
}

/// The `params` of an `opcua_client` or `ads` add: the new connections, enabled
/// (`NC/protocolActions.ts:220-226` and `85-91`).
fn plan_params(connections: Vec<Value>) -> Map<String, Value> {
    let mut params = Map::new();
    params.insert("enabled".to_owned(), Value::Bool(true));
    params.insert("connections".to_owned(), Value::Array(connections));
    params
}

/// A plan that failed with a reason that is not a field error, shown in the pane that started it
/// (specification 25 §5.13).
fn failure(pane: &str, text: String) -> ActReply {
    super::reply(
        false,
        Some(super::notice(Tone::Warning, pane, text)),
        Vec::new(),
        false,
        Refresh::None,
    )
}

// ---- the pure planners (design §1.6) ----

/// Merges the chosen globals into `current`, the parameters of the server that owns them
/// (`buildExposeApplyParams`, `NC/exposeConfig.ts:13-32`; specification 25 §5.8): every key that
/// is not a derived topology-only or secret-presence key is kept as it is, `expose` gains each name
/// once in today's order, and `writable` gains them too when the user opted in and the server
/// holds the field. The returned `ExposePlan` carries both the names and the merged parameters.
#[must_use]
pub(crate) fn expose_params(
    current: &Map<String, Value>,
    paths: &[String],
    allow_writes: bool,
) -> ExposePlan {
    let names: Vec<String> = paths.iter().map(|path| global_name(path)).collect();
    let mut params = Map::new();
    for (key, value) in current {
        if key.ends_with("_set") || DERIVED_TOPOLOGY_PARAM_KEYS.contains(&key.as_str()) {
            continue;
        }
        params.insert(key.clone(), value.clone());
    }
    params.insert(
        "expose".to_owned(),
        strings(&merged(current.get("expose"), &names)),
    );
    if allow_writes && current.contains_key("writable") {
        params.insert(
            "writable".to_owned(),
            strings(&merged(current.get("writable"), &names)),
        );
    }
    ExposePlan { names, params }
}

/// A browse path as the global it exposes: `global.TankLevel` becomes `TankLevel`
/// (`globalName`, `NC/exposeConfig.ts:9-11`).
#[must_use]
pub(crate) fn global_name(path: &str) -> String {
    path.strip_prefix("global.").unwrap_or(path).to_owned()
}

/// `current`'s strings, then `names`, each kept once in order, as `new Set([...current, ...names])`
/// (`NC/exposeConfig.ts:27-29`).
fn merged(current: Option<&Value>, names: &[String]) -> Vec<String> {
    let mut values: Vec<String> = Vec::new();
    for value in string_array(current)
        .into_iter()
        .chain(names.iter().cloned())
    {
        if !values.contains(&value) {
            values.push(value);
        }
    }
    values
}

/// The selected `EtherCAT` channels (`NC/protocolActions.ts:266-268`): each path trimmed, an empty
/// one dropped, a duplicate kept once, and the result sorted as today.
#[must_use]
pub(crate) fn ethercat_selection(paths: &[String]) -> Vec<String> {
    let mut selected: Vec<String> = Vec::new();
    for path in paths {
        let trimmed = path.trim();
        if !trimmed.is_empty() && !selected.iter().any(|known| known == trimmed) {
            selected.push(trimmed.to_owned());
        }
    }
    selected.sort();
    selected
}

/// Builds an OPC UA client connection from the selected nodes
/// (`buildOpcuaConnection`, `W/opcuaClientModel.ts:121-155`): the endpoint URL, the security
/// policy and mode, the authentication (with the username and password only for `username`), the
/// certificate trust, and one point per selected node. `None` when the target names no endpoint URL
/// or no selected node can be saved.
#[must_use]
pub(crate) fn opcua_connection(
    target: &Map<String, Value>,
    label: &str,
    nodes: &[Value],
    allow_writes: bool,
) -> Option<Value> {
    let endpoint_url = text_field(target, "endpoint_url")?;
    let points: Vec<Value> = nodes
        .iter()
        .filter_map(|node| opcua_point(node, allow_writes))
        .collect();
    if points.is_empty() {
        return None;
    }
    let auth = text_field(target, "auth").unwrap_or_else(|| "anonymous".to_owned());
    let mut connection = Map::new();
    connection.insert(
        "name".to_owned(),
        Value::String(opcua_connection_name(label, &endpoint_url)),
    );
    connection.insert("endpoint_url".to_owned(), Value::String(endpoint_url));
    connection.insert(
        "security_policy".to_owned(),
        Value::String(text_field(target, "security_policy").unwrap_or_else(|| "none".to_owned())),
    );
    connection.insert(
        "security_mode".to_owned(),
        Value::String(text_field(target, "security_mode").unwrap_or_else(|| "none".to_owned())),
    );
    connection.insert("auth".to_owned(), Value::String(auth.clone()));
    connection.insert(
        "trust_server_certificate".to_owned(),
        Value::Bool(target.get("trust_server_certificate") == Some(&Value::Bool(true))),
    );
    connection.insert("points".to_owned(), Value::Array(points));
    if auth == "username" {
        if let Some(username) = text_field(target, "username") {
            connection.insert("username".to_owned(), Value::String(username));
        }
        if let Some(password) = text_field(target, "password") {
            connection.insert("password".to_owned(), Value::String(password));
        }
    }
    Some(Value::Object(connection))
}

/// One OPC UA point from a browsed node (`opcuaPointFromNode`, `W/opcuaClientModel.ts:66-79`):
/// its truST variable, the **raw** `node_id`, the apply-ready type, and the access. `None` when the
/// node carries no raw `node_id`, which cannot be saved honestly.
fn opcua_point(node: &Value, allow_writes: bool) -> Option<Value> {
    let node_id = node
        .get("node_id")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())?;
    let display = opcua_display_type(node);
    let mut point = Map::new();
    point.insert("var".to_owned(), Value::String(opcua_var(node)));
    point.insert("node_id".to_owned(), Value::String(node_id.to_owned()));
    point.insert(
        "type".to_owned(),
        Value::String(if display.is_empty() {
            "string".to_owned()
        } else {
            display
        }),
    );
    point.insert(
        "access".to_owned(),
        Value::String(opcua_access(node, allow_writes).to_owned()),
    );
    Some(Value::Object(point))
}

/// The truST variable of a browsed node (`deriveOpcuaVarName`, `W/opcuaClientModel.ts:43-48`):
/// `global.` plus its path (or name, else `node`) with every run of characters that are not
/// `[A-Za-z0-9_]` folded to one `_` and the surrounding `_` trimmed.
fn opcua_var(node: &Value) -> String {
    let raw = text_field_value(node.get("path"))
        .or_else(|| text_field_value(node.get("name")))
        .unwrap_or_else(|| "node".to_owned());
    let core = fold_identifier(&raw).trim_matches('_').to_owned();
    if core.is_empty() {
        "global.node".to_owned()
    } else {
        format!("global.{core}")
    }
}

/// The apply-ready type of a node (`opcuaDisplayType`, `W/opcuaClientModel.ts:52-54`): the
/// backend's resolved `data_type`, else the raw OPC UA type, else empty.
fn opcua_display_type(node: &Value) -> String {
    node.get("data_type")
        .and_then(Value::as_str)
        .or_else(|| node.get("type").and_then(Value::as_str))
        .unwrap_or_default()
        .to_owned()
}

/// Whether a node is written (`opcuaAccess`, `W/opcuaClientModel.ts:57-62`): only when the server
/// reports it writable **and** the user opted in.
fn opcua_access(node: &Value, allow_writes: bool) -> &'static str {
    if allow_writes && node.get("writable").and_then(Value::as_bool) == Some(true) {
        "read_write"
    } else {
        "read"
    }
}

/// The connection name of an OPC UA client (`deriveOpcuaConnectionName`,
/// `W/opcuaClientModel.ts:109-119`): the label, or the endpoint URL when the label is empty or the
/// raw protocol id, slugged to `[a-z0-9-]` and cut to 48 characters.
fn opcua_connection_name(label: &str, endpoint_url: &str) -> String {
    let base = if label.is_empty() || label == "opcua_client" {
        endpoint_url
    } else {
        label
    };
    let base = if base.is_empty() { "opcua" } else { base };
    let without_scheme = base.strip_prefix("opc.tcp://").unwrap_or(base);
    let slug = slug(without_scheme);
    let name: String = slug
        .trim_matches('-')
        .to_lowercase()
        .chars()
        .take(48)
        .collect();
    if name.is_empty() {
        "opcua".to_owned()
    } else {
        name
    }
}

/// The text with every run of characters that are not `[A-Za-z0-9]` replaced by one `-`
/// (the `/[^A-Za-z0-9]+/g` fold of `W/opcuaClientModel.ts:114`).
fn slug(text: &str) -> String {
    let mut slug = String::with_capacity(text.len());
    let mut separator = false;
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            separator = false;
        } else if !separator {
            slug.push('-');
            separator = true;
        }
    }
    slug
}

/// The text with every run of characters that are not `[A-Za-z0-9_]` replaced by one `_`
/// (the `/[^A-Za-z0-9_]+/g` fold of `W/opcuaClientModel.ts:45`).
fn fold_identifier(text: &str) -> String {
    let mut folded = String::with_capacity(text.len());
    let mut separator = false;
    for character in text.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            folded.push(character);
            separator = false;
        } else if !separator {
            folded.push('_');
            separator = true;
        }
    }
    folded
}

/// One ADS connection per responding port of the discovered device (`NC/protocolActions.ts:71-84`),
/// each with today's fields. `Err` is the warning the panel shows today: the device names no
/// address, AMS Net ID or responding port, or discovery returned no AMS Net ID for this computer.
///
/// # Errors
///
/// The device cannot be added because it names no host, no target AMS Net ID or no responding
/// port, or because discovery returned no AMS Net ID for this computer.
pub(crate) fn ads_device_connections(
    target: &Map<String, Value>,
    label: &str,
) -> Result<Vec<Value>, String> {
    let Some(host) = string_field(target, &["host", "ip"]) else {
        return Err(plan_texts::ADS_NO_ADDRESS.to_owned());
    };
    let Some(target_net_id) = string_field(target, &["target_net_id", "ams_net_id"]) else {
        return Err(plan_texts::ADS_NO_ADDRESS.to_owned());
    };
    let ports = ads_responding_ports(target);
    if ports.is_empty() {
        return Err(plan_texts::ADS_NO_ADDRESS.to_owned());
    }
    let Some(first_local) = local_ads_net_id(target, None) else {
        return Err(plan_texts::ADS_NO_LOCAL_NET_ID.to_owned());
    };
    let mut connections = Vec::with_capacity(ports.len());
    for port in ports {
        let mut port_target = target.clone();
        port_target.insert("name".to_owned(), Value::String(label.to_owned()));
        port_target.insert("ams_port".to_owned(), Value::Number(u64::from(port).into()));
        let mut connection = Map::new();
        connection.insert(
            "name".to_owned(),
            Value::String(ads_connection_name(&port_target, label)),
        );
        connection.insert(
            "target_net_id".to_owned(),
            Value::String(target_net_id.clone()),
        );
        connection.insert("host".to_owned(), Value::String(host.clone()));
        connection.insert("ams_port".to_owned(), Value::Number(u64::from(port).into()));
        connection.insert(
            "local_net_id".to_owned(),
            Value::String(
                local_ads_net_id(target, Some(port)).unwrap_or_else(|| first_local.clone()),
            ),
        );
        connection.insert("transport".to_owned(), Value::String("plain".to_owned()));
        connection.insert("insecure_transport".to_owned(), Value::Bool(true));
        connection.insert("auto_add_route".to_owned(), Value::Bool(false));
        connection.insert("points".to_owned(), Value::Array(Vec::new()));
        connections.push(Value::Object(connection));
    }
    Ok(connections)
}

/// The responding ADS ports of a device (`respondingAdsPorts` with `adsDiscoveryPorts`,
/// `NC/adsDiscoveryPorts.ts:38-48`): the whole numbers from 1 to 65535, each once, sorted.
#[must_use]
pub(crate) fn ads_responding_ports(target: &Map<String, Value>) -> Vec<u16> {
    let mut ports: Vec<u16> = target
        .get("responding_ads_ports")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(ads_port)
        .collect();
    ports.sort_unstable();
    ports.dedup();
    ports
}

/// The ADS port of `value`: a whole number from 1 to 65535 (`isAdsPort`,
/// `NC/adsDiscoveryPorts.ts:147-149`).
fn ads_port(value: &Value) -> Option<u16> {
    value
        .as_u64()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port >= 1)
}

/// The connection name of an ADS device (`adsConnectionNameForTarget`,
/// `NC/adsDiscoveryPorts.ts:133-145`): the device's name, suffixed with `_port_<n>` while more than
/// one port answers, with any earlier `_port_<n>` suffix removed first.
#[must_use]
pub(crate) fn ads_connection_name(target: &Map<String, Value>, fallback: &str) -> String {
    let base = target
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback);
    let port = target.get("ams_port").and_then(ads_port).unwrap_or(851);
    let unsuffixed = strip_port_suffix(base);
    if ads_responding_ports(target).len() > 1 {
        format!("{unsuffixed}_port_{port}")
    } else {
        base.to_owned()
    }
}

/// `base` without a trailing `_port_<digits>` (`NC/adsDiscoveryPorts.ts:141`).
fn strip_port_suffix(base: &str) -> &str {
    let Some(index) = base.rfind("_port_") else {
        return base;
    };
    let suffix = &base[index + "_port_".len()..];
    if !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit()) {
        &base[..index]
    } else {
        base
    }
}

/// This computer's AMS Net ID for `port`, as discovery reported it (`localAdsNetId`,
/// `NC/protocolActions.ts:714-733`): the configured `local_net_id`, else the route plan of the
/// port's browse evidence, else any port's. `port` is `None` while the caller asks for the first.
#[must_use]
pub(crate) fn local_ads_net_id(target: &Map<String, Value>, port: Option<u16>) -> Option<String> {
    if let Some(configured) = string_field(target, &["local_net_id", "local_ams_net_id"]) {
        return Some(configured);
    }
    let evidence = ads_port_browse_evidence(target);
    let matching = evidence
        .iter()
        .filter(|item| port.is_none_or(|wanted| item.port == wanted))
        .chain(evidence.iter());
    for item in matching {
        if let Some(value) = item
            .route_plan
            .as_ref()
            .and_then(|plan| plan.get("local"))
            .and_then(|local| local.get("ams_net_id"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_owned());
        }
    }
    None
}

/// One port's ADS browse evidence: the port and the route plan it reported.
#[derive(Debug, Clone)]
struct AdsPortEvidence {
    /// The ADS port.
    port: u16,
    /// The route plan it reported, when one was a record.
    route_plan: Option<Value>,
}

/// The browse evidence of each responding ADS port (`adsPortBrowseEvidence`,
/// `NC/adsDiscoveryPorts.ts:50-77`), in port order.
fn ads_port_browse_evidence(target: &Map<String, Value>) -> Vec<AdsPortEvidence> {
    let mut evidence: Vec<AdsPortEvidence> = target
        .get("ads_port_browse_results")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let item = item.as_object()?;
            let port = item.get("port").and_then(ads_port)?;
            if !item.get("tree").is_some_and(Value::is_array) {
                return None;
            }
            let route_plan = item
                .get("routePlan")
                .filter(|plan| plan.is_object())
                .cloned();
            Some(AdsPortEvidence { port, route_plan })
        })
        .collect();
    evidence.sort_by_key(|item| item.port);
    evidence
}

/// The trimmed, non-empty string at the first of `keys` that holds one (`stringField`,
/// `NC/protocolActions.ts:735-746`).
fn string_field(value: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    for key in keys {
        if let Some(text) = value.get(*key).and_then(Value::as_str) {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_owned());
            }
        }
    }
    None
}

/// The non-empty string at `key` of `value`, as `str()` reads it (`NC/protocolActions.ts:205-207`
/// and the `|| "none"` defaults of `W/opcuaClientModel.ts:142-144`).
fn text_field(value: &Map<String, Value>, key: &str) -> Option<String> {
    text_field_value(value.get(key))
}

/// The non-empty string of `value`, else `None`.
fn text_field_value(value: Option<&Value>) -> Option<String> {
    value
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}

/// The object at `key` of `value`, else an empty one.
fn object_of(value: &Value, key: &str) -> Map<String, Value> {
    value
        .get(key)
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default()
}

/// The array of `value`, else an empty one.
fn array_of(value: Option<&Value>) -> Vec<Value> {
    value.and_then(Value::as_array).cloned().unwrap_or_default()
}

/// The paths that the user selected, in the order the browse pane sent them.
fn paths_of(request: &ActRequest) -> Vec<String> {
    string_array(request.values.get("paths"))
}

/// Whether the user opted into exposing the globals writable.
fn writable_of(request: &ActRequest) -> bool {
    request
        .values
        .get("writable")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// The strings of `value`, ignoring everything else (`stringArray`, `NC/exposeConfig.ts:3-7`).
fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// `values` as a JSON array of strings.
fn strings(values: &[String]) -> Value {
    Value::Array(values.iter().cloned().map(Value::String).collect())
}
