//! The status of the configured run target (specification 25 §5.1): the Simulator is never
//! contacted; an online target is probed at its control endpoint and asked for `status` with its
//! token, and named reachable, unreachable, failing on its credentials or without an endpoint.
//!
//! `target` `{ mode, endpoint?, authToken?, endpointEnabled?, label?, setupUrl?,
//! unavailableReason? }` answers `{ ok: true, target: { mode, endpoint, endpointEnabled,
//! reachable, status, authFailureKind, label, setupUrl, credentialChannel } }`. The token is used,
//! never echoed. `unavailableReason` is trust-runtime's reason why the workspace's own runtime has
//! no endpoint now (specification 11 §6.8.0a): such a target is unreachable, not unset.

use serde::Deserialize;
use serde_json::{json, Value};
use trust_runtime_client::{probe, ProbeTimeouts, TargetProbe};

use super::{decode, endpoint, non_blank, request::auth_json};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Args {
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    auth_token: Option<String>,
    #[serde(default)]
    endpoint_enabled: Option<bool>,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    setup_url: Option<String>,
    #[serde(default)]
    unavailable_reason: Option<String>,
}

/// `{ ok: true, target }` for the settings in `argument`.
pub(super) fn answer(argument: Value) -> Result<Value, String> {
    let args: Args = decode(argument)?;
    let target = status(&args, |endpoint, enabled, token| {
        probe(endpoint, enabled, token, ProbeTimeouts::default())
    });
    Ok(json!({ "ok": true, "target": target }))
}

/// The target of `args`, asking `ask` for an online target's probe.
fn status(args: &Args, ask: impl FnOnce(&str, bool, Option<&str>) -> TargetProbe) -> Value {
    let online = args.mode.as_deref() == Some("online");
    let endpoint = args
        .endpoint
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_owned();
    let enabled = args.endpoint_enabled.unwrap_or(true);
    let active = if enabled { endpoint.as_str() } else { "" };
    let token = non_blank(args.auth_token.as_deref());
    let target = |status: &str, reachable: bool, auth: Option<&str>, label: String| {
        json!({
            "mode": if online { "online" } else { "simulate" },
            "endpoint": non_blank(Some(&endpoint)),
            "endpointEnabled": enabled,
            "reachable": reachable,
            "status": status,
            "authFailureKind": auth,
            "label": label,
            "setupUrl": non_blank(args.setup_url.as_deref()),
            "credentialChannel": endpoint::credential_channel(active),
        })
    };
    let given_label = args.label.clone();
    if !online {
        let label = given_label.unwrap_or_else(|| "Simulator".to_owned());
        return target("simulate", false, None, label);
    }
    let online_label = || {
        given_label.clone().unwrap_or_else(|| {
            if endpoint.is_empty() {
                "Online runtime".to_owned()
            } else {
                endpoint.clone()
            }
        })
    };
    match ask(active, enabled, token.as_deref()) {
        TargetProbe::MissingEndpoint => {
            let status = if non_blank(args.unavailable_reason.as_deref()).is_some() {
                "online_unreachable"
            } else {
                "missing_endpoint"
            };
            let label = given_label
                .clone()
                .unwrap_or_else(|| "Online runtime".to_owned());
            target(status, false, None, label)
        }
        TargetProbe::Unreachable => target("online_unreachable", false, None, online_label()),
        TargetProbe::AuthFailed(auth) => {
            target("auth_failed", true, Some(auth_json(auth)), online_label())
        }
        TargetProbe::Reachable(_) => target("online_reachable", true, None, online_label()),
    }
}

#[cfg(test)]
#[path = "target_tests.rs"]
mod tests;
