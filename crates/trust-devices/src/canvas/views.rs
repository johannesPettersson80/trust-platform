//! The canvas's small views, each the panel engine's rule in the shape the canvas draws: the ADS
//! client's summary (specification 25 §5.13), the recovery of a failed browse (§5.4 and §11.2),
//! the names and failure texts of a discovery, and why a discovery result cannot be added (§2).

use serde::Serialize;
use serde_json::Value;

use crate::texts::browse as browse_texts;
use crate::texts::discover as discover_texts;
use crate::view::connections::ads_client::{ads_client_status, devices, settings};
use crate::view::discover_names::runtime_failure_text;
use crate::view::types::Tone;

/// The ADS client's summary: its devices, one sentence about its state with its tone, and its
/// settings.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsClientSummaryView {
    /// The devices of its connections, each with the tags read from each of its ports.
    pub devices: Vec<AdsDeviceView>,
    /// Its state in one sentence.
    pub status: String,
    /// `ok`, `neutral` or `error`.
    pub status_kind: &'static str,
    /// Whether it is enabled.
    pub enabled: bool,
    /// Its configuration file.
    pub config_path: String,
    /// Its update interval in milliseconds.
    pub update_interval_ms: f64,
}

/// An ADS device of the summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsDeviceView {
    /// Its host address.
    pub address: String,
    /// Its AMS Net ID.
    pub ams_net_id: String,
    /// Its ADS ports in order, each with the number of tags read from it.
    pub ports: Vec<AdsPortView>,
}

/// One ADS port of a device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdsPortView {
    /// The port.
    pub port: u16,
    /// The tags read from it.
    pub tag_count: usize,
}

/// The recovery of a failed browse: the protocol error's code, the one action the pane offers
/// (`trust`, `credentials`, `security`, `retry` or `none`), and its title and detail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrowseErrorView {
    /// The protocol error's code.
    pub code: String,
    /// The recovery the pane offers.
    pub action: &'static str,
    /// Its title.
    pub title: String,
    /// What to do.
    pub detail: String,
}

/// The summary of an ADS client with the parameters `params`, the health `health` and `detail`,
/// whose runtime's own health is `runtime_health` (the panel's `ads_client_status`, devices and
/// settings): a configured client of a running runtime is never called stopped.
#[must_use]
pub(super) fn ads_summary(
    params: Option<&Value>,
    health: &str,
    detail: &str,
    runtime_health: &str,
) -> AdsClientSummaryView {
    let runtime_runs = matches!(
        runtime_health.trim().to_ascii_lowercase().as_str(),
        "connected" | "simulate" | "running"
    );
    let (status, tone) = ads_client_status(health, detail, runtime_runs);
    let settings = settings(params);
    AdsClientSummaryView {
        devices: devices(params)
            .into_iter()
            .map(|device| AdsDeviceView {
                address: device.address,
                ams_net_id: device.net_id,
                ports: device
                    .ports
                    .into_iter()
                    .map(|(port, tag_count)| AdsPortView { port, tag_count })
                    .collect(),
            })
            .collect(),
        status,
        status_kind: match tone {
            Tone::Ok => "ok",
            Tone::Error => "error",
            _ => "neutral",
        },
        enabled: settings.enabled,
        config_path: settings.config_path,
        update_interval_ms: settings.interval_ms.parse().unwrap_or_default(),
    }
}

/// The recovery of a browse of `protocol` that failed with the protocol error `code` and
/// `message` (`classifyBrowseError`): an ADS failure by its code, an OPC UA failure by its code
/// with the recovery each needs, and any other failure with its own message.
#[must_use]
pub(super) fn browse_error(
    protocol: &str,
    code: Option<&str>,
    message: Option<&str>,
) -> BrowseErrorView {
    let view = |code: &str, action, title: &str, detail: &str| BrowseErrorView {
        code: code.to_owned(),
        action,
        title: title.to_owned(),
        detail: detail.to_owned(),
    };
    let message = message.filter(|message| !message.is_empty());
    if protocol == "ads" {
        let code = code.unwrap_or("symbol_upload_failed");
        return match code {
            "ads_port_unavailable" => view(
                code,
                "retry",
                browse_texts::ADS_PORT_UNAVAILABLE_TITLE,
                browse_texts::ADS_PORT_UNAVAILABLE_DETAIL,
            ),
            "symbol_upload_unsupported" => view(
                code,
                "none",
                browse_texts::SYMBOL_UPLOAD_UNSUPPORTED_TITLE,
                browse_texts::SYMBOL_UPLOAD_UNSUPPORTED_DETAIL,
            ),
            "empty_symbol_table" => view(
                code,
                "none",
                browse_texts::NO_COMPATIBLE_SYMBOLS_TITLE,
                browse_texts::NO_COMPATIBLE_SYMBOLS_DETAIL,
            ),
            _ => view(
                code,
                "retry",
                browse_texts::ADS_BROWSE_FAILED_TITLE,
                message.unwrap_or(browse_texts::ADS_BROWSE_FAILED_DETAIL),
            ),
        };
    }
    let code = code.unwrap_or_default();
    match code {
        "cert_untrusted" => view(
            code,
            "trust",
            browse_texts::CERT_UNTRUSTED_TITLE,
            browse_texts::CERT_UNTRUSTED_DETAIL,
        ),
        "auth_required" => view(
            code,
            "credentials",
            browse_texts::AUTH_REQUIRED_TITLE,
            browse_texts::AUTH_REQUIRED_DETAIL,
        ),
        "unsupported_security_profile" => view(
            code,
            "security",
            browse_texts::UNSUPPORTED_SECURITY_TITLE,
            browse_texts::UNSUPPORTED_SECURITY_DETAIL,
        ),
        "endpoint_unreachable" => view(
            code,
            "retry",
            browse_texts::UNREACHABLE_TITLE,
            browse_texts::UNREACHABLE_DETAIL,
        ),
        "browse_denied" => view(
            code,
            "none",
            browse_texts::BROWSE_DENIED_TITLE,
            browse_texts::BROWSE_DENIED_DETAIL,
        ),
        _ => {
            let title = browse_texts::browse_failed("opcua_client");
            BrowseErrorView {
                code: if code.is_empty() { "unknown" } else { code }.to_owned(),
                action: "none",
                detail: message.map_or_else(|| format!("{title}."), str::to_owned),
                title,
            }
        }
    }
}

/// The name that the canvas's discovery gives `protocol` in its progress and failure texts.
#[must_use]
pub(super) fn discovery_name(protocol: &str) -> String {
    match protocol {
        "ads" => "ADS".to_owned(),
        "discovery" => discover_texts::RUNTIME_DISCOVERY_NAME.to_owned(),
        "modbus_tcp" => "Modbus".to_owned(),
        "opcua_client" => "OPC UA server".to_owned(),
        "mqtt" => "MQTT broker".to_owned(),
        "ethercat" => "EtherCAT".to_owned(),
        "gpio" => "GPIO".to_owned(),
        other => other.replace('_', " "),
    }
}

/// The label of a discovery's progress row: the protocol's name, with `@ <host>` for a known
/// address, else with the subnet.
#[must_use]
pub(super) fn discovery_label(protocol: &str, host: Option<&str>, cidr: Option<&str>) -> String {
    fn given(value: Option<&str>) -> Option<&str> {
        value.map(str::trim).filter(|value| !value.is_empty())
    }
    let name = discovery_name(protocol);
    match (given(host), given(cidr)) {
        (Some(host), _) => format!("{name} @ {host}"),
        (None, Some(cidr)) => format!("{name} {cidr}"),
        (None, None) => name,
    }
}

/// The text of a discovery that failed at the selected runtime with `message` (the panel's
/// `runtime_failure_text`).
#[must_use]
pub(super) fn discovery_failure(protocol: &str, message: &str) -> String {
    runtime_failure_text(&discovery_name(protocol), message)
}

/// Why a discovery result cannot be added, in this order (specification 25 §2): the runtime that
/// found it is no longer drawn, or its protocol no longer offers discovery.
#[must_use]
pub(super) fn disabled_reason(
    origin_drawn: bool,
    protocol_discovers: bool,
) -> Option<&'static str> {
    if !origin_drawn {
        Some(discover_texts::STALE_RUNTIME)
    } else if !protocol_discovers {
        Some(discover_texts::PROTOCOL_UNAVAILABLE)
    } else {
        None
    }
}
