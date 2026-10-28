//! The ADS ports of ADS discovery (specification 11, "Communication discovery scope and symbol
//! browsing"): the checked port list, the port probe, the rule that decides whether a port
//! answered, and the port the find keeps. The probe browses each port with the same symbol browse
//! that `comm.browse_symbols` runs; the ports of one device are probed one after the other.

use serde_json::Value;

#[cfg(feature = "ads-wire")]
use super::report::Deadline;

/// The ADS ports probed when a request names none, in ascending order.
pub(crate) const DEFAULT_PORTS: [u16; 6] = [301, 501, 851, 852, 853, 854];

/// The ports a request names: duplicates merged, sorted; the default when it names none.
///
/// # Errors
/// The value is not a list, or holds an entry that is not an integer from 1 through 65,535; the
/// message names the value.
pub(crate) fn checked_ports(raw: Option<&Value>) -> Result<Vec<u16>, String> {
    let Some(raw) = raw else {
        return Ok(DEFAULT_PORTS.to_vec());
    };
    let values = raw
        .as_array()
        .ok_or_else(|| format!("ads_ports must be a list of ports, not {raw}"))?;
    let mut ports = Vec::with_capacity(values.len());
    for value in values {
        let port = value
            .as_u64()
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port != 0)
            .ok_or_else(|| {
                format!("ads_ports must each be an integer from 1 through 65,535, not {value}")
            })?;
        if !ports.contains(&port) {
            ports.push(port);
        }
    }
    ports.sort_unstable();
    Ok(ports)
}

/// What one ADS port browse answers: the same shape that `comm.browse_symbols` returns.
#[cfg(feature = "ads-wire")]
pub(crate) trait AdsPortProbe: Send + Sync {
    /// Browses `port` of `target`; `None` when the browse could not run.
    fn probe(&self, target: &crate::ads::diagnostics::TargetIdentity, port: u16) -> Option<Value>;
}

/// The real probe: the live ADS symbol browse over the wire.
#[cfg(feature = "ads-wire")]
pub(crate) struct LiveAdsPortProbe;

#[cfg(feature = "ads-wire")]
impl AdsPortProbe for LiveAdsPortProbe {
    fn probe(&self, target: &crate::ads::diagnostics::TargetIdentity, port: u16) -> Option<Value> {
        use crate::ads::diagnostics::CredentialChannelClassification;

        let mut target = target.clone();
        target.ams_port = port;
        crate::ads::onboarding::browse::browse_ads_live(
            target,
            None,
            Vec::new(),
            None,
            CredentialChannelClassification::TrustedSameHost,
        )
        .ok()
    }
}

/// Probes the `ports` of one device, one after the other, and builds the find's parameters and
/// warnings (specification 11, "Communication discovery scope and symbol browsing").
#[cfg(feature = "ads-wire")]
pub(crate) fn probe_candidate(
    target: &crate::ads::diagnostics::TargetIdentity,
    ports: &[u16],
    deadline: Deadline,
    probe: &dyn AdsPortProbe,
) -> (Value, Vec<String>) {
    let (responding, browse_results) = probe_ports(target, ports, deadline, probe);
    let current = (1..=65535)
        .contains(&target.ams_port)
        .then_some(target.ams_port);
    let selected = select_port(current, &responding);
    let params = serde_json::json!({
        "ams_net_id": target.ams_net_id,
        "host": target.ip,
        "name": target.name,
        "ams_port": selected,
        "tc_version": target.tc_version,
        "os_version": target.os_version,
        "fingerprint": target.fingerprint,
        "responding_ads_ports": responding,
        "ads_port_browse_results": browse_results,
    });
    let warnings = if responding.is_empty() {
        vec![no_ads_port_warning(target.ams_net_id.as_str())]
    } else {
        Vec::new()
    };
    (params, warnings)
}

/// The ports that answered, in the order probed, with their browse results.
#[cfg(feature = "ads-wire")]
fn probe_ports(
    target: &crate::ads::diagnostics::TargetIdentity,
    ports: &[u16],
    deadline: Deadline,
    probe: &dyn AdsPortProbe,
) -> (Vec<u16>, Vec<Value>) {
    let mut responding = Vec::new();
    let mut browse_results = Vec::new();
    for &port in ports {
        // A probe starts no new port once the search deadline has passed.
        if deadline.passed() {
            break;
        }
        if let Some(value) = probe.probe(target, port)
            && responded(&value)
        {
            responding.push(port);
            browse_results.push(browse_result(port, &value));
        }
    }
    (responding, browse_results)
}

/// Whether a port answered: its symbol tree is not empty, its route status is `ok`, or its error
/// code is `symbol_upload_unsupported` or `empty_symbol_table`.
#[cfg(feature = "ads-wire")]
fn responded(value: &Value) -> bool {
    let tree = value
        .get("tree")
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    if tree > 0 || value.pointer("/route/status").and_then(Value::as_str) == Some("ok") {
        return true;
    }
    matches!(
        value.pointer("/error/code").and_then(Value::as_str),
        Some("symbol_upload_unsupported" | "empty_symbol_table")
    )
}

/// One answering port's browse result, with the field names the panel reads.
#[cfg(feature = "ads-wire")]
fn browse_result(port: u16, value: &Value) -> Value {
    let mut result = serde_json::Map::new();
    result.insert("port".to_string(), serde_json::json!(port));
    result.insert(
        "tree".to_string(),
        value
            .get("tree")
            .filter(|node| node.is_array())
            .cloned()
            .unwrap_or_else(|| Value::Array(Vec::new())),
    );
    result.insert(
        "routeMissing".to_string(),
        serde_json::json!(
            value.pointer("/route/status").and_then(Value::as_str) == Some("missing")
        ),
    );
    if let Some(route_plan) = value
        .pointer("/route/route_plan")
        .filter(|plan| !plan.is_null())
    {
        result.insert("routePlan".to_string(), route_plan.clone());
    }
    if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
        result.insert("error".to_string(), error.clone());
    }
    Value::Object(result)
}

/// The port the find keeps: its own port when that answered, otherwise the first answering port,
/// otherwise its own port, otherwise 851.
#[cfg(feature = "ads-wire")]
fn select_port(current: Option<u16>, responding: &[u16]) -> u16 {
    if let Some(current) = current
        && responding.contains(&current)
    {
        return current;
    }
    responding.first().copied().or(current).unwrap_or(851)
}

/// The warning an ADS find gets when none of its ports answered.
#[cfg(feature = "ads-wire")]
fn no_ads_port_warning(ams_net_id: &str) -> String {
    format!(
        "No ADS port answered on {ams_net_id}; the TwinCAT target is kept without a responding port."
    )
}

#[cfg(all(test, feature = "ads-wire"))]
#[path = "ads_ports_tests.rs"]
mod tests;
