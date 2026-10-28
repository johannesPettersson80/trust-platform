//! The ADS client's view (specification 25 §5.13 and §5.16): one sentence about its state, its
//! devices with their address, AMS Net ID and ports with the number of tags read from each, and
//! its settings. A setting the runtime does not report reads as the default of the parsed
//! configuration; the runtime reports an ADS client only while `[runtime.ads]` is enabled.

use std::collections::BTreeMap;

use serde_json::Value;
use trust_runtime::config::AdsRuntimeConfig;

use super::fields::{count_of, non_empty, scalar};
use super::row;
use crate::texts::connections as texts;
use crate::texts::graph;
use crate::topology::Endpoint;
use crate::view::types::{Row, Section, Tone};

/// The health words of an ADS client that needs attention.
const NEEDS_ATTENTION: [&str; 4] = ["error", "degraded", "auth_failed", "runtime_unreachable"];

/// An ADS device: one address and AMS Net ID, with the tags read from each of its ports.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdsDevice {
    /// Its host address, or `Unknown`.
    pub(crate) address: String,
    /// Its AMS Net ID, or `Unknown`.
    pub(crate) net_id: String,
    /// The tags read from each of its ADS ports, by port.
    pub(crate) ports: BTreeMap<u16, usize>,
}

/// The client's settings, each read from the configuration's defaults when the runtime does not
/// report it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdsSettings {
    /// Whether the client is enabled.
    pub(crate) enabled: bool,
    /// Its configuration file.
    pub(crate) config_path: String,
    /// Its update interval in milliseconds, as text.
    pub(crate) interval_ms: String,
}

/// The sections of the ADS client's inspector: its state and devices, and its settings.
pub(super) fn sections(endpoint: &Endpoint, runtime_runs: bool) -> Vec<Section> {
    let params = endpoint.params.as_ref();
    let (status, tone) = ads_client_status(&endpoint.health, &endpoint.detail, runtime_runs);
    let devices = devices(params);
    let mut first = vec![Row {
        tone: Some(tone),
        ..row(graph::ROW_STATE, status)
    }];
    match devices.as_slice() {
        [] => first.push(row(texts::ROW_ADS_DEVICES, texts::NO_ADS_DEVICE)),
        [device] => first.extend(device_rows(device)),
        _ => {}
    }
    let mut sections = vec![Section {
        heading: None,
        rows: first,
    }];
    if devices.len() > 1 {
        sections.extend(devices.iter().enumerate().map(|(index, device)| Section {
            heading: Some(texts::ads_device(index + 1)),
            rows: device_rows(device),
        }));
    }
    sections.push(Section {
        heading: Some(texts::SECTION_ADVANCED_SETTINGS.to_owned()),
        rows: settings_rows(params),
    });
    sections
}

/// The ADS client's state in one sentence, with its tone (`ads_client_status`). A configured
/// client of a running runtime is never called stopped.
pub(crate) fn ads_client_status(health: &str, detail: &str, runtime_runs: bool) -> (String, Tone) {
    let health = health.trim().to_ascii_lowercase();
    let detail = detail.trim();
    let (text, tone) = if runtime_runs && health == "configured_policy" {
        (texts::ADS_RUNTIME_RUNNING, Tone::Neutral)
    } else if health == "configured_policy"
        || health == "stopped"
        || detail
            .to_ascii_lowercase()
            .contains("runtime is not running")
    {
        (texts::ADS_RUNTIME_STOPPED, Tone::Neutral)
    } else if health == "connected" {
        (texts::ADS_CONNECTED, Tone::Ok)
    } else if health == "disabled" {
        (texts::ADS_DISABLED, Tone::Neutral)
    } else if NEEDS_ATTENTION.contains(&health.as_str()) {
        (or_detail(detail, texts::ADS_NEEDS_ATTENTION), Tone::Error)
    } else {
        (or_detail(detail, texts::ADS_CONFIGURED), Tone::Neutral)
    };
    (text.to_owned(), tone)
}

fn or_detail<'a>(detail: &'a str, fallback: &'a str) -> &'a str {
    if detail.is_empty() {
        fallback
    } else {
        detail
    }
}

/// The ADS devices of the client's connections, grouped by address and AMS Net ID, each port
/// with the number of tags read from it.
pub(crate) fn devices(params: Option<&Value>) -> Vec<AdsDevice> {
    let mut devices: Vec<AdsDevice> = Vec::new();
    let connections = params
        .and_then(|params| params.get("connections"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|connection| connection.is_object());
    for connection in connections {
        let text = |keys: [&str; 2]| {
            keys.iter()
                .find_map(|key| {
                    connection
                        .get(*key)
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                })
                .unwrap_or(graph::UNKNOWN)
                .to_owned()
        };
        let address = text(["host", "ip"]);
        let net_id = text(["target_net_id", "ams_net_id"]);
        let index = if let Some(index) = devices
            .iter()
            .position(|device| device.address == address && device.net_id == net_id)
        {
            index
        } else {
            devices.push(AdsDevice {
                address,
                net_id,
                ports: BTreeMap::new(),
            });
            devices.len() - 1
        };
        if let Some(port) = count_of(connection.get("ams_port"))
            .and_then(|port| u16::try_from(port).ok())
            .filter(|port| *port >= 1)
        {
            let tags = connection
                .get("points")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);
            *devices[index].ports.entry(port).or_default() += tags;
        }
    }
    devices
}

/// An ADS device's address, AMS Net ID, and the tags read from each of its ports.
fn device_rows(device: &AdsDevice) -> Vec<Row> {
    let mut rows = vec![
        row(graph::ROW_ADDRESS, device.address.as_str()),
        row(texts::ROW_AMS_NET_ID, device.net_id.as_str()),
    ];
    if device.ports.is_empty() {
        rows.push(row(texts::ROW_ADS_PORTS, texts::NO_PORTS));
    }
    rows.extend(
        device
            .ports
            .iter()
            .map(|(port, tags)| row(&texts::ads_port(&port.to_string()), texts::tags(*tags))),
    );
    rows
}

/// The client's settings: whether it is enabled, its configuration file and its update
/// interval, each read from the configuration's defaults when the runtime does not report it.
fn settings_rows(params: Option<&Value>) -> Vec<Row> {
    let AdsSettings {
        enabled,
        config_path,
        interval_ms: interval,
    } = settings(params);
    vec![
        row(
            texts::LABEL_ENABLED,
            if enabled { texts::ON } else { texts::OFF },
        ),
        row(texts::LABEL_CONFIG_FILE, config_path),
        row(texts::ROW_UPDATE_INTERVAL, graph::milliseconds(&interval)),
    ]
}

/// The client's settings (see [`AdsSettings`]).
pub(crate) fn settings(params: Option<&Value>) -> AdsSettings {
    let defaults = AdsRuntimeConfig::default();
    let param = |key: &str| params.and_then(|params| params.get(key));
    let enabled = match param("enabled") {
        Some(Value::Bool(enabled)) => *enabled,
        Some(Value::String(text)) => !text.trim().eq_ignore_ascii_case("false"),
        _ => true,
    };
    let config_path = param("config_path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map_or_else(|| defaults.config_path.display().to_string(), str::to_owned);
    let interval = non_empty(scalar(param("worker_tick_interval_ms")))
        .filter(|interval| {
            interval
                .parse::<f64>()
                .is_ok_and(|milliseconds| milliseconds.is_finite() && milliseconds > 0.0)
        })
        .unwrap_or_else(|| defaults.worker_tick_interval.as_millis().to_string());
    AdsSettings {
        enabled,
        config_path,
        interval_ms: interval,
    }
}
