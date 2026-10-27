//! The `TwinCAT` cards of the Discover pane (specification 25 §2; old `DiscoverPane.tsx` lines 326-373 and
//! 400-408): the responding ports of an ADS find, the find that no port answered (left out), the text of an
//! identity that no port answered, the ports' buttons, and the card's action, `Add to canvas` or
//! `Manage tags` once the canvas holds the device (the project mark of `discover_compare` says so).

use serde_json::{json, Value};

use crate::actions::plans::ads_responding_ports;
use crate::texts::discover as texts;
use crate::view::types::{Action, Emphasis, Op, Pane, UiOp};

/// The ports that an unconfigured search asks, which the old pane's default list gave
/// (`adsDiscoveryPorts.ts`, `DEFAULT_ADS_DISCOVERY_PORTS`).
pub(crate) const DEFAULT_PORTS: [u16; 6] = [301, 501, 851, 852, 853, 854];

/// The responding ports of an ADS find's parameters (`respondingAdsPorts`).
#[must_use]
pub(crate) fn responding_ports(params: &Value) -> Vec<u16> {
    params
        .as_object()
        .map_or_else(Vec::new, ads_responding_ports)
}

/// Whether a find is left out of the cards: an ADS find that no port answered (`DiscoverPane.tsx` lines
/// 131-134).
#[must_use]
pub(crate) fn unanswered(protocol: &str, params: &Value) -> bool {
    protocol == "ads" && responding_ports(params).is_empty()
}

/// The configured ports, sorted and without repeats, or the default ports when none is configured
/// (`adsDiscoveryPorts`).
#[must_use]
pub(crate) fn configured_ports(configured: &[u16]) -> Vec<u16> {
    let mut ports = configured.to_vec();
    ports.sort_unstable();
    ports.dedup();
    if ports.is_empty() {
        DEFAULT_PORTS.to_vec()
    } else {
        ports
    }
}

/// The source row's text when `TwinCAT` identity was found and no port answered (`discoveryController.ts`
/// lines 237-241). `identity_found` says that an ADS find came back; `answered` that one has a responding
/// port.
#[must_use]
pub(crate) fn unverified_text(
    identity_found: bool,
    answered: bool,
    configured: &[u16],
) -> Option<String> {
    (identity_found && !answered).then(|| texts::ads_unverified(&configured_ports(configured)))
}

/// The button of each responding port, which browses that port's tags of the found device (`DiscoverPane.tsx`
/// lines 358-373 showed the ports as chips; here they are buttons).
#[must_use]
pub(crate) fn port_actions(device_id: &str, params: &Value, ports: &[u16]) -> Vec<Action> {
    ports
        .iter()
        .map(|port| Action {
            id: format!("ads_port_{port}"),
            label: port.to_string(),
            enabled: true,
            reason: None,
            emphasis: Emphasis::Secondary,
            confirm: None,
            op: browse_op(device_id, params, Some(*port)),
        })
        .collect()
}

/// The card's own action: `Manage tags` once the canvas holds the device, which browses its tags (old line
/// 406); else `Add to canvas`, which adds one ADS connection per responding port to the owning runtime (old
/// line 407; `connection.ads_add`). A successful add is followed by the tag browser of the device, as the old
/// pane opened it after the add (`useDiscoverPane.ts` lines 98-109): the follow-up is the browse that
/// `Manage tags` opens. Without an owning runtime it is disabled with its reason.
#[must_use]
pub(crate) fn primary_action(
    device_id: &str,
    params: &Value,
    title: &str,
    on_canvas: bool,
    owner: Option<&str>,
) -> Action {
    if on_canvas {
        return Action {
            id: "manage_tags".to_owned(),
            label: texts::MANAGE_TAGS.to_owned(),
            enabled: true,
            reason: None,
            emphasis: Emphasis::Primary,
            confirm: None,
            op: browse_op(device_id, params, None),
        };
    }
    let Some(runtime) = owner else {
        return Action {
            id: "add_to_canvas".to_owned(),
            label: texts::ADD_TO_CANVAS.to_owned(),
            enabled: false,
            reason: Some(texts::ADD_NEEDS_RUNTIME.to_owned()),
            emphasis: Emphasis::Quiet,
            confirm: None,
            op: Op::Rust {
                action: "connection.ads_add".to_owned(),
                payload: json!({}),
            },
        };
    };
    Action {
        id: "add_to_canvas".to_owned(),
        label: texts::ADD_TO_CANVAS.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Primary,
        confirm: None,
        op: Op::Followed {
            first: Box::new(Op::Rust {
                action: "connection.ads_add".to_owned(),
                payload: json!({ "runtime": runtime, "label": title, "target": params }),
            }),
            then: Box::new(browse_op(device_id, params, None)),
        },
    }
}

/// The browse of a found ADS device's tags, which runs against the device's own target (`BrowseSource`,
/// specification 25 §5.4); `port` names the one port to browse.
fn browse_op(device_id: &str, params: &Value, port: Option<u16>) -> Op {
    let mut request = json!({
        "source": { "found": device_id, "protocol": "ads", "target": params },
        "kind": "symbols",
        "trust_server_certificate": false,
    });
    if let (Some(port), Some(object)) = (port, request.as_object_mut()) {
        object.insert("port".to_owned(), json!(port));
    }
    Op::Ui {
        ui: UiOp::Open {
            pane: Pane::Browse { request },
        },
    }
}
