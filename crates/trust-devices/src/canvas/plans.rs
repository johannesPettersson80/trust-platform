//! The connection plans of the canvas, each written into the open project's files with
//! `comm.apply` (specification 25 §5.2, §5.4 and §5.8): globals exposed through the server that
//! owns them, the `EtherCAT` channel selection, and an OPC UA client connection built from the
//! browsed nodes. The planners are the panel engine's own (`actions::plans`).

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::act::{CanvasActRequest, CanvasReply};
use crate::actions::plans::{ethercat_selection, expose_params, opcua_connection};
use crate::texts::connections::protocol_name;
use crate::texts::plans as texts;

/// The arguments of `expose`: the server's protocol, the project's topology that the canvas shows
/// (the `local` answer of `trust-lsp.devicesTopology`), the chosen globals and whether they are
/// writable.
#[derive(Debug, Deserialize)]
pub(super) struct ExposeArgs {
    #[serde(default)]
    protocol: String,
    #[serde(default)]
    topology: Value,
    #[serde(default)]
    paths: Vec<Value>,
    #[serde(default)]
    writable: bool,
}

/// The arguments of `ethercat_channels`: the browsed bus's parameters (empty when the pane browsed
/// none), the project's topology that the canvas shows, and the chosen channels.
#[derive(Debug, Deserialize)]
pub(super) struct EthercatArgs {
    #[serde(default)]
    target: Map<String, Value>,
    #[serde(default)]
    topology: Value,
    #[serde(default)]
    paths: Vec<Value>,
}

/// The arguments of `opcua_add`: the browsed server, its label, the chosen nodes and whether they
/// are writable.
#[derive(Debug, Deserialize)]
pub(super) struct OpcuaArgs {
    #[serde(default)]
    target: Map<String, Value>,
    #[serde(default)]
    label: String,
    #[serde(default)]
    nodes: Vec<Value>,
    #[serde(default)]
    writable: bool,
}

/// `expose`: merges the chosen globals into the configuration of the project's first server of the
/// protocol and writes it.
pub(super) fn expose(request: &CanvasActRequest, args: &ExposeArgs) -> CanvasReply {
    let protocol = args.protocol.trim();
    let paths = strings(&args.paths, false);
    let Some(project) = request.project() else {
        return CanvasReply::nothing();
    };
    if protocol.is_empty() || paths.is_empty() {
        return CanvasReply::nothing();
    }
    let Some(current) = endpoint_params(&args.topology, protocol) else {
        return CanvasReply::warning(texts::expose_not_configured(&protocol_name(protocol)));
    };
    let plan = expose_params(&current, &paths, args.writable);
    match apply_offline(project, protocol, "upsert", &plan.params) {
        Applied::Ok(answer) => CanvasReply::done(texts::exposed(
            protocol,
            plan.names.len(),
            restart_required(&answer),
        )),
        Applied::Failed(reason) => CanvasReply::warning(texts::expose_failed(reason.as_deref())),
    }
}

/// `ethercat_channels`: writes the chosen channels on the browsed `EtherCAT` bus, else on the
/// project's first configured one.
pub(super) fn ethercat_channels(request: &CanvasActRequest, args: &EthercatArgs) -> CanvasReply {
    let paths = strings(&args.paths, true);
    let Some(project) = request.project() else {
        return CanvasReply::nothing();
    };
    if paths.is_empty() {
        return CanvasReply::nothing();
    }
    let current = if args.target.is_empty() {
        endpoint_params(&args.topology, "ethercat").unwrap_or_default()
    } else {
        args.target.clone()
    };
    if current.is_empty() {
        return CanvasReply::warning(texts::ETHERCAT_NOT_CONFIGURED);
    }
    let selected = ethercat_selection(&paths);
    let mut params = current;
    params.insert("selected_channels".to_owned(), json!(selected));
    match apply_offline(project, "ethercat", "upsert", &params) {
        Applied::Ok(answer) => CanvasReply::done(texts::ethercat_selected(
            selected.len(),
            restart_required(&answer),
        )),
        Applied::Failed(reason) => CanvasReply::warning(texts::ethercat_failed(reason.as_deref())),
    }
}

/// `opcua_add`: builds an OPC UA client connection from the chosen nodes and adds it. Nodes that
/// cannot be saved, or a server without an endpoint URL, add nothing.
pub(super) fn opcua_add(request: &CanvasActRequest, args: &OpcuaArgs) -> CanvasReply {
    let Some(project) = request.project() else {
        return CanvasReply::nothing();
    };
    let Some(connection) = opcua_connection(&args.target, &args.label, &args.nodes, args.writable)
    else {
        return CanvasReply::nothing();
    };
    let points = connection
        .get("points")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    match apply_offline(
        project,
        "opcua_client",
        "add",
        &connections(vec![connection]),
    ) {
        Applied::Ok(answer) => {
            CanvasReply::done(texts::opcua_added(points, restart_required(&answer)))
        }
        Applied::Failed(reason) => CanvasReply::warning(texts::opcua_failed(reason.as_deref())),
    }
}

/// What one offline `comm.apply` did.
pub(super) enum Applied {
    /// It was applied; the answer.
    Ok(Value),
    /// It was not: the refused fields' messages, else the answer's message or the failure.
    Failed(Option<String>),
}

/// Writes one `comm.apply` into the open project's files, as `trust-runtime comm apply` does.
pub(super) fn apply_offline(
    project: &std::path::Path,
    protocol: &str,
    action: &str,
    params: &Map<String, Value>,
) -> Applied {
    let request = json!({ "protocol": protocol, "action": action, "params": params });
    match trust_runtime::control::offline_comm_apply_json(project, request) {
        Err(reason) => Applied::Failed(Some(reason)),
        Ok(answer) => {
            let fields: Vec<&str> = answer
                .get("field_errors")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(|error| error.get("message").and_then(Value::as_str))
                .collect();
            if !fields.is_empty() {
                Applied::Failed(Some(fields.join("; ")))
            } else if answer.get("applied").and_then(Value::as_bool) == Some(true) {
                Applied::Ok(answer)
            } else {
                Applied::Failed(
                    answer
                        .get("message")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                )
            }
        }
    }
}

/// The `params` of a client add: the new connections, enabled.
pub(super) fn connections(connections: Vec<Value>) -> Map<String, Value> {
    let mut params = Map::new();
    params.insert("enabled".to_owned(), Value::Bool(true));
    params.insert("connections".to_owned(), Value::Array(connections));
    params
}

/// The parameters of the first connection of `protocol` in `topology` (a `fleet.topology` report)
/// that has parameters; see [`super::comm::endpoints_of`] for the order.
pub(super) fn endpoint_params(topology: &Value, protocol: &str) -> Option<Map<String, Value>> {
    super::comm::endpoints_of(topology, protocol)
        .into_iter()
        .find_map(|endpoint| endpoint.get("params").and_then(Value::as_object).cloned())
}

/// Whether the runtime must restart to apply the change.
fn restart_required(answer: &Value) -> bool {
    answer.get("lifecycle_effect").and_then(Value::as_str) == Some("restart_required")
}

/// The texts among `values`; with `non_empty`, only those that are not blank.
fn strings(values: &[Value], non_empty: bool) -> Vec<String> {
    values
        .iter()
        .filter_map(Value::as_str)
        .filter(|text| !non_empty || !text.trim().is_empty())
        .map(str::to_owned)
        .collect()
}
