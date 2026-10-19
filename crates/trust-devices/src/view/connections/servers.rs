//! What a server's summary says (specification 25 §5.4 and §5.8): where it listens, how many
//! clients are connected to it and whether only the self-test or an independent client proved
//! it, the globals it exposes, and the clients it allows, as the runtime humanized them.

use serde_json::Value;

use super::fields::{array_of, count_of, non_empty, scalar};
use super::row;
use crate::texts::connections as texts;
use crate::view::types::Row;

/// The scheme of an OPC UA endpoint.
const OPC_TCP: &str = "opc.tcp://";

/// The server rows of a connection: the endpoint of an OPC UA server, and the endpoint,
/// connected clients and verification of an ADS server.
pub(super) fn rows(protocol: &str, params: Option<&Value>, live: Option<&Value>) -> Vec<Row> {
    match protocol {
        "opcua" => opcua_server_endpoint(params)
            .map(|endpoint| row(texts::ROW_SERVER_ENDPOINT, endpoint))
            .into_iter()
            .collect(),
        "ads_server" => ads_server_endpoint(params)
            .map(|endpoint| row(texts::ROW_SERVER_ENDPOINT, endpoint))
            .into_iter()
            .chain([
                row(texts::ROW_CONNECTED_CLIENTS, connected_clients(live)),
                row(texts::ROW_VERIFICATION, ads_server_verification(live)),
            ])
            .collect(),
        _ => Vec::new(),
    }
}

fn param(params: Option<&Value>, key: &str) -> String {
    scalar(params.and_then(|params| params.get(key)))
}

/// The URL that an OPC UA server listens at: its listen address with the `opc.tcp://` scheme,
/// and its endpoint path.
pub(super) fn opcua_server_endpoint(params: Option<&Value>) -> Option<String> {
    let listen = param(params, "listen");
    let path = param(params, "endpoint_path");
    if listen.is_empty() {
        return non_empty(path);
    }
    let base = if listen.contains("://") {
        listen
    } else {
        format!("{OPC_TCP}{listen}")
    };
    if path.is_empty() || path == "/" {
        return Some(base);
    }
    let suffix = if path.starts_with('/') {
        path
    } else {
        format!("/{path}")
    };
    if base.ends_with(&suffix) {
        Some(base)
    } else {
        Some(format!("{}{suffix}", base.trim_end_matches('/')))
    }
}

/// Where an ADS server listens: its listen address, AMS Net ID and ADS port.
pub(super) fn ads_server_endpoint(params: Option<&Value>) -> Option<String> {
    let listen = param(params, "listen");
    let net_id = param(params, "ams_net_id");
    let port = param(params, "ads_port");
    let parts: Vec<String> = [
        non_empty(listen),
        non_empty(net_id).map(|net_id| texts::ams_net_id(&net_id)),
        non_empty(port).map(|port| texts::ads_port(&port)),
    ]
    .into_iter()
    .flatten()
    .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The object of a server's live evidence, also when it arrives as JSON text.
fn live_value(live: Option<&Value>) -> Option<Value> {
    let value = live?.as_object()?.get("value")?;
    match value {
        Value::Object(_) => Some(value.clone()),
        Value::String(text) if text.trim_start().starts_with('{') => {
            serde_json::from_str::<Value>(text)
                .ok()
                .filter(Value::is_object)
        }
        _ => None,
    }
}

/// How many clients are connected, from the server's live evidence.
fn connected_clients(live: Option<&Value>) -> String {
    live_value(live)
        .and_then(|value| count_of(value.get("connected_clients")))
        .map_or_else(
            || texts::NO_LIVE_CLIENT_EVIDENCE.to_owned(),
            texts::clients_connected,
        )
}

/// Whether only the self-test proved the server or an external client did, and which one
/// (`ads_server_verification`).
pub(super) fn ads_server_verification(live: Option<&Value>) -> String {
    let Some(value) = live_value(live) else {
        return texts::NO_EXTERNAL_CLIENT_VERIFIED.to_owned();
    };
    let text = |key: &str| scalar(value.get(key));
    let proof = text("proof_status");
    let verified = value
        .get("external_client_verified")
        .and_then(Value::as_bool)
        == Some(true)
        || proof == "external_client_verified"
        || proof == "production_ready";
    if !verified {
        let shown = if proof == "self_test_available" || value.get("connected_clients").is_some() {
            texts::SELF_TEST_ONLY
        } else {
            texts::NO_EXTERNAL_CLIENT_VERIFIED
        };
        return shown.to_owned();
    }
    let client = [text("external_client_kind"), text("external_client_name")]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    match (proof == "production_ready", client.is_empty()) {
        (true, true) => texts::PRODUCTION_VERIFIED.to_owned(),
        (true, false) => texts::production_verified_by(&client),
        (false, true) => texts::VERIFIED.to_owned(),
        (false, false) => texts::verified_by(&client),
    }
}

/// The globals a server exposes (`exposed_globals`), such as `2 globals: global.TankLevel,
/// global.PumpRunning`.
pub(super) fn exposed_globals(value: Option<&Value>) -> String {
    let names: Vec<String> = array_of(value)
        .iter()
        .map(|name| scalar(Some(name)))
        .filter(|name| !name.is_empty())
        .collect();
    if names.is_empty() {
        texts::NONE.to_owned()
    } else {
        texts::globals(&names)
    }
}

/// The clients that the ADS server allows (`ads_server_clients`): the runtime's humanized
/// summary when it reports one, else each client's AMS Net ID with the source it is pinned to.
pub(super) fn ads_server_clients(value: Option<&Value>, params: Option<&Value>) -> String {
    let summary: Vec<String> = array_of(params.and_then(|params| params.get("clients_summary")))
        .iter()
        .map(|client| scalar(Some(client)))
        .filter(|client| !client.is_empty())
        .collect();
    if !summary.is_empty() {
        return summary.join("; ");
    }
    let clients = array_of(value);
    if clients.is_empty() {
        return texts::NONE.to_owned();
    }
    clients
        .iter()
        .enumerate()
        .map(|(index, client)| allowed_client(index, client))
        .filter(|client| !client.is_empty())
        .collect::<Vec<_>>()
        .join("; ")
}

fn allowed_client(index: usize, client: &Value) -> String {
    let fallback = || texts::numbered(texts::CLIENT_NOUN, index + 1);
    if !client.is_object() {
        return non_empty(scalar(Some(client))).unwrap_or_else(fallback);
    }
    let text = |key: &str| non_empty(scalar(client.get(key)));
    let net_id = text("ams_net_id")
        .or_else(|| text("net_id"))
        .unwrap_or_else(fallback);
    match text("source_ip").or_else(|| text("source_cidr")) {
        Some(source) => texts::client_from(&net_id, &source),
        None if client.get("unpinned").and_then(Value::as_bool) == Some(true) => {
            texts::unpinned_client(&net_id)
        }
        None => net_id,
    }
}
