//! `add_host`: a runtime address typed into `Connect existing runtime`, or the control endpoint of a
//! runtime the canvas created, added to the fleet (specification 25 §5.2): `host:port` becomes
//! `tcp://host:port`, a `tcp://` or `unix://`
//! endpoint is kept, and any other text is refused with the address help. An endpoint already in
//! the fleet is not added twice. Without a token the canvas focuses the runtime drawn for the new
//! endpoint, which says what it needs. The canvas stores the token and the label and writes the
//! fleet setting the reply names.

use serde::Deserialize;
use serde_json::json;

use super::act::{CanvasNotice, CanvasReply};
use crate::actions::runtimes::normalize_endpoint;
use crate::texts::forms::RUNTIME_ADDRESS_HELP;
use crate::topology::configured_runtime_id;

/// The arguments of `add_host`: the typed address, the fleet's endpoints and whether a token was
/// typed.
#[derive(Debug, Deserialize)]
pub(super) struct AddHostArgs {
    #[serde(default)]
    endpoint: String,
    #[serde(default)]
    fleet_endpoints: Vec<String>,
    #[serde(default)]
    token: bool,
}

/// `add_host`: the normalized endpoint, whether the fleet already holds it, the fleet's endpoints
/// with it, and the runtime to focus when no token was typed. An empty address does nothing.
pub(super) fn add_host(args: &AddHostArgs) -> CanvasReply {
    if args.endpoint.trim().is_empty() {
        return CanvasReply::nothing();
    }
    let Some(endpoint) = normalize_endpoint(&args.endpoint) else {
        return CanvasReply::warning(RUNTIME_ADDRESS_HELP);
    };
    let already = args.fleet_endpoints.iter().any(|known| known == &endpoint);
    let mut fleet_endpoints = args.fleet_endpoints.clone();
    if !already {
        fleet_endpoints.push(endpoint.clone());
    }
    let focus = (!args.token).then(|| configured_runtime_id(&endpoint));
    CanvasReply {
        applied: !already,
        notice: already.then(|| CanvasNotice::info(format!("{endpoint} is already in the fleet."))),
        refresh: true,
        result: Some(json!({
            "endpoint": endpoint,
            "already": already,
            "fleet_endpoints": fleet_endpoints,
            "focus": focus,
        })),
        ..CanvasReply::nothing()
    }
}
