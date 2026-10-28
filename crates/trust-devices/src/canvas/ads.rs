//! The ADS device of the canvas: the finds of a search that its Discover cards show
//! (specification 25 §2), a discovered device added to the open project with one ADS connection
//! per responding port, each with this computer's AMS Net ID for that port (specification 11,
//! "Communication discovery scope and symbol browsing"; specification 25 §5.4), and the ADS port
//! typed into the browse pane checked before it is browsed. The rules are the panel engine's own
//! (`view::discover_ads` and `actions::plans::ads_device_connections`).

use serde::Deserialize;
use serde_json::{json, Map, Value};

use super::act::{CanvasActRequest, CanvasReply};
use super::plans::{apply_offline, connections, Applied};
use super::views;
use crate::actions::plans::ads_device_connections;
use crate::texts::plans as texts;
use crate::view::discover_ads;

/// The ADS port of an empty port field: the first PLC runtime's (specification 11).
pub(super) const DEFAULT_ADS_PORT: u16 = 851;

/// Why a typed ADS port is not a whole number.
const PORT_NOT_A_NUMBER: &str = "Enter a whole-number ADS port from 1 to 65535.";
/// Why a typed ADS port is outside the ports ADS has.
const PORT_OUT_OF_RANGE: &str = "ADS port must be between 1 and 65535.";

/// The arguments of `ads_add_device`: the discovered device and its label.
#[derive(Debug, Deserialize)]
pub(super) struct AddDeviceArgs {
    #[serde(default)]
    target: Map<String, Value>,
    #[serde(default)]
    label: String,
}

/// The arguments of `ads_port`: the text typed into the port field.
#[derive(Debug, Deserialize)]
pub(super) struct PortArgs {
    #[serde(default)]
    text: String,
}

/// The arguments of `ads_finds`: the ADS finds of one search (`comm.discover` candidates) and the
/// ADS ports it was told to probe (`trust.ads.discoveryPorts`).
#[derive(Debug, Deserialize)]
pub(super) struct FindsArgs {
    #[serde(default)]
    candidates: Vec<Value>,
    #[serde(default)]
    ads_ports: Vec<Value>,
}

/// `ads_finds`: the finds that the Discover cards show, as `{ "candidates": [...] }`, and, when the
/// search identified `TwinCAT` systems and none of them answered on a port, the source row's text
/// as `error` (specification 25 §2). A find that no port answered is left out of the cards. Each
/// port's browse that failed carries the recovery its tag browser shows, as `error_view`.
pub(super) fn finds(args: &FindsArgs) -> CanvasReply {
    let shown: Vec<Value> = args
        .candidates
        .iter()
        .filter(|candidate| {
            !discover_ads::unanswered("ads", candidate.get("params").unwrap_or(&Value::Null))
        })
        .cloned()
        .map(with_error_views)
        .collect();
    let configured: Vec<u16> = args
        .ads_ports
        .iter()
        .filter_map(|port| port.as_u64().and_then(|port| u16::try_from(port).ok()))
        .filter(|port| *port >= 1)
        .collect();
    let answered = !shown.is_empty();
    let mut result = json!({ "candidates": shown });
    if let Some(error) =
        discover_ads::unverified_text(!args.candidates.is_empty(), answered, &configured)
    {
        result["error"] = Value::String(error);
    }
    CanvasReply {
        result: Some(result),
        ..CanvasReply::nothing()
    }
}

/// `candidate` with the recovery of each port's failed browse (`params.ads_port_browse_results[]`
/// `.error`) as that result's `error_view`.
fn with_error_views(mut candidate: Value) -> Value {
    let results = candidate
        .get_mut("params")
        .and_then(|params| params.get_mut("ads_port_browse_results"))
        .and_then(Value::as_array_mut);
    for result in results.into_iter().flatten() {
        let Some(error) = result.get("error").filter(|error| error.is_object()) else {
            continue;
        };
        let view = views::browse_error(
            "ads",
            error.get("code").and_then(Value::as_str),
            error.get("message").and_then(Value::as_str),
        );
        result["error_view"] = serde_json::to_value(view).unwrap_or_default();
    }
    candidate
}

/// `ads_add_device`: adds the device's connections and reads the data again; a device that cannot
/// be added, or a refused write, warns with the reason.
pub(super) fn add_device(request: &CanvasActRequest, args: &AddDeviceArgs) -> CanvasReply {
    let Some(project) = request.project() else {
        return CanvasReply::warning(texts::ADS_NO_ADDRESS);
    };
    let label = match args.label.trim() {
        "" => texts::ADS_DEVICE,
        label => label,
    };
    let connections_of_ports = match ads_device_connections(&args.target, label) {
        Ok(connections) => connections,
        Err(reason) => return CanvasReply::warning(reason),
    };
    match apply_offline(project, "ads", "add", &connections(connections_of_ports)) {
        Applied::Ok(_) => CanvasReply {
            applied: true,
            refresh: true,
            ..CanvasReply::nothing()
        },
        Applied::Failed(reason) => CanvasReply::warning(texts::ads_failed(reason.as_deref())),
    }
}

/// `ads_port`: the port that the typed text names, as `{ "port": n }`, or why it names none, as
/// `{ "error": text }`. An empty field is port 851; any other text must be a whole number from 1
/// to 65535.
pub(super) fn port(args: &PortArgs) -> CanvasReply {
    let result = match typed_port(&args.text) {
        Ok(port) => json!({ "port": port }),
        Err(reason) => json!({ "error": reason }),
    };
    CanvasReply {
        result: Some(result),
        ..CanvasReply::nothing()
    }
}

/// The ADS port of a typed text (see [`port`]).
pub(super) fn typed_port(text: &str) -> Result<u16, &'static str> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(DEFAULT_ADS_PORT);
    }
    if !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(PORT_NOT_A_NUMBER);
    }
    text.parse::<u64>()
        .ok()
        .and_then(|port| u16::try_from(port).ok())
        .filter(|port| *port >= 1)
        .ok_or(PORT_OUT_OF_RANGE)
}
