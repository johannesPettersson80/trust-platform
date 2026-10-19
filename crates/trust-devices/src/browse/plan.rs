//! `browse.add` (design §4.3 B8 and §3.3 J49; specification 25 §5.4 and §11.2): build the plan of
//! the pane's kind from the ticked leaves and write it through the panel's one write path. The
//! selection is read from the session's own browse answer, so nothing is trusted to the extension
//! beyond the ticked keys.

use serde_json::{json, Map, Value};

use super::run::{self, BrowsedNode};
use crate::actions::{self, plans};
use crate::request::{ActOp, ActRequest, BrowseRequest};
use crate::session::Session;
use crate::texts::browse as texts;
use crate::view::types::{ActReply, BrowseKind, ItemId};

/// `browse.add`: adds the leaves the pane ticked with the plan of its kind (`opcua_add`,
/// `ethercat_channels`, `connection.expose`), written through the one write path of
/// [`actions::write`](crate::actions::write). A pane that cannot add says why and writes nothing.
#[must_use]
pub(crate) fn add(session: &Session, request: &ActRequest) -> ActReply {
    let pane = actions::payload_text(&request.op.payload, "pane")
        .unwrap_or_else(|| actions::pane(request));
    let Some(body) = request.op.payload.get("request") else {
        return actions::error(&pane, texts::ADD_DID_NOT_DECODE);
    };
    let pane_request: BrowseRequest = match serde_json::from_value(body.clone()) {
        Ok(pane_request) => pane_request,
        Err(error) => return actions::error(&pane, &texts::add_did_not_decode(&error.to_string())),
    };
    let (resolved, _run) = match run::resolve(session, &pane_request) {
        Ok(resolved) => resolved,
        Err(unresolved) => {
            return actions::error(
                &pane,
                &format!("{}: {}", unresolved.error.title, unresolved.error.detail),
            );
        }
    };
    let Some(answer) = session.browse().answer(&resolved.key) else {
        return actions::error(&pane, texts::WAIT_FOR_RESULTS);
    };
    let tree = match answer {
        Ok(tree) => tree,
        Err(message) => return actions::error(&pane, &message),
    };
    if tree.error.is_some() {
        return actions::error(&pane, texts::RESOLVE_ERROR);
    }
    let keys = keys_of(request);
    let leaves = selected_leaves(&tree.tree, &keys);
    if leaves.is_empty() {
        return actions::error(&pane, texts::NO_SELECTION);
    }
    let Some(runtime) = resolved.runtime.as_ref() else {
        return actions::error(&pane, texts::NO_SOURCE);
    };
    let plan = plan_request(
        &pane,
        &resolved,
        &pane_request,
        request,
        &leaves,
        writable_of(request),
        runtime,
    );
    let mut reply = match resolved.kind {
        BrowseKind::Nodes => plans::opcua_add(session, &plan),
        BrowseKind::Channels => plans::ethercat_channels(session, &plan),
        BrowseKind::Expose => plans::expose(session, &plan),
        BrowseKind::AdsTags => {
            return actions::error(&pane, &texts::not_browsable(&resolved.protocol))
        }
    };
    reply.close_pane = reply.ok;
    reply
}

/// The `comm.apply` plan that one `browse.add` builds for its kind (`NC/protocolActions.ts` and
/// `W/opcuaClientModel.ts:121-155`), as the payload and values `plans` reads.
fn plan_request(
    pane: &str,
    resolved: &run::Resolved,
    pane_request: &BrowseRequest,
    source: &ActRequest,
    leaves: &[&BrowsedNode],
    writable: bool,
    runtime: &ItemId,
) -> ActRequest {
    let (action, payload, values) = match resolved.kind {
        BrowseKind::Nodes => (
            "connection.opcua_add",
            json!({
                "pane": pane,
                "runtime": runtime.0,
                "label": resolved.name,
                "target": run::opcua_target(&resolved.params, pane_request.trust_server_certificate),
            }),
            values_of([
                ("nodes", nodes_value(leaves)),
                ("writable", Value::Bool(writable)),
            ]),
        ),
        BrowseKind::Channels => (
            "connection.ethercat_channels",
            json!({
                "pane": pane,
                "runtime": runtime.0,
                "connection": connection_id(resolved),
            }),
            values_of([("paths", paths_value(leaves))]),
        ),
        BrowseKind::Expose => (
            "connection.expose",
            json!({
                "pane": pane,
                "runtime": runtime.0,
                "protocol": resolved.protocol,
                "connection": connection_id(resolved),
            }),
            values_of([
                ("paths", paths_value(leaves)),
                ("writable", Value::Bool(writable)),
            ]),
        ),
        BrowseKind::AdsTags => ("connection.opcua_add", Value::Null, Map::new()),
    };
    ActRequest {
        session: source.session.clone(),
        op: ActOp {
            action: action.to_owned(),
            payload,
        },
        values,
        project: source.project.clone(),
        host: source.host.clone(),
    }
}

/// The connection's item ID, as the plan addresses it.
fn connection_id(resolved: &run::Resolved) -> Value {
    resolved
        .connection
        .as_ref()
        .map_or(Value::Null, |id| Value::String(id.0.clone()))
}

/// One node per ticked leaf, with exactly the fields `opcua_connection` reads.
fn nodes_value(leaves: &[&BrowsedNode]) -> Value {
    Value::Array(
        leaves
            .iter()
            .map(|node| {
                json!({
                    "id": node.id,
                    "node_id": node.node_id,
                    "name": node.name,
                    "path": node.path,
                    "type": node.type_label,
                    "data_type": node.data_type,
                    "writable": node.writable,
                })
            })
            .collect(),
    )
}

/// The path of every ticked leaf, in browse order.
fn paths_value(leaves: &[&BrowsedNode]) -> Value {
    Value::Array(
        leaves
            .iter()
            .map(|node| Value::String(node.path.clone()))
            .collect(),
    )
}

/// The leaves whose key the pane ticked, in browse order (`selectedLeaves`,
/// `W/opcuaClientModel.ts:90-106`).
fn selected_leaves<'a>(nodes: &'a [BrowsedNode], keys: &[String]) -> Vec<&'a BrowsedNode> {
    let mut leaves = Vec::new();
    walk(nodes, keys, &mut leaves);
    leaves
}

fn walk<'a>(nodes: &'a [BrowsedNode], keys: &[String], leaves: &mut Vec<&'a BrowsedNode>) {
    for node in nodes {
        if node.children.is_empty() {
            if keys.iter().any(|key| key.as_str() == node.key()) {
                leaves.push(node);
            }
        } else {
            walk(&node.children, keys, leaves);
        }
    }
}

/// The keys the pane ticked.
fn keys_of(request: &ActRequest) -> Vec<String> {
    request
        .values
        .get("keys")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect()
}

/// Whether the user opted into writing the ticked leaves.
fn writable_of(request: &ActRequest) -> bool {
    request
        .values
        .get("writable")
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

/// `pairs` as a values map.
fn values_of(pairs: impl IntoIterator<Item = (&'static str, Value)>) -> Map<String, Value> {
    pairs
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value))
        .collect()
}
