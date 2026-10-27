//! The MQTT brokers and OPC UA servers that announce themselves over mDNS (specification 11,
//! "Sources"): a search browses `_mqtt._tcp.local.` and `_opcua-tcp._tcp.local.` until it ends,
//! and hands every service that resolves to the probe of its protocol. Only a probe that answers
//! is a device; a service that does not answer is a warning that names it.

use std::net::{IpAddr, SocketAddr};

use mdns_sd::ResolvedService;

use crate::error::RuntimeError;
use crate::opcua::OpcUaClientEndpointInfo;

use super::super::model::ProtocolFind;
use super::super::runner::SourceSink;
use super::mqtt::{mqtt_candidate, mqtt_find, mqtt_probe_timeout, probe_mqtt};
use super::opcua::{opcua_endpoint_finds, opcua_probe_timeout};
use super::probe::DiscoveryProbeOutcome;
use super::report::DiscoverCandidate;

/// The mDNS service type that MQTT brokers announce (specification 11, "Sources").
pub(super) const MQTT_SERVICE_TYPE: &str = "_mqtt._tcp.local.";
/// The mDNS service type that OPC UA servers announce (specification 11, "Sources").
pub(super) const OPCUA_SERVICE_TYPE: &str = "_opcua-tcp._tcp.local.";
/// How a find that an announcement gave is named (specification 11, "Sources").
const MDNS_SOURCE: &str = "mdns";
/// The endpoint path of an OPC UA service that names none in its `path` TXT entry.
const OPCUA_ROOT_PATH: &str = "/";

/// Which announced services an mDNS browse looks for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AnnouncedProtocol {
    /// MQTT brokers, announced as `_mqtt._tcp.local.`.
    Mqtt,
    /// OPC UA servers, announced as `_opcua-tcp._tcp.local.`.
    Opcua,
}

impl AnnouncedProtocol {
    /// The mDNS service type that this protocol's devices announce.
    fn service_type(self) -> &'static str {
        match self {
            Self::Mqtt => MQTT_SERVICE_TYPE,
            Self::Opcua => OPCUA_SERVICE_TYPE,
        }
    }
}

/// One service that announced itself over mDNS, as a browse hands it over: its name, its
/// addresses, its port, and the `path` TXT entry an OPC UA endpoint needs. The rules below work
/// on this, so that a test can state an announcement literally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Announcement {
    /// The service's full mDNS name, for a warning that names it.
    pub(super) fullname: String,
    /// The addresses it announces.
    pub(super) addresses: Vec<IpAddr>,
    /// Its port.
    pub(super) port: u16,
    /// Its `path` TXT entry, as the service wrote it.
    pub(super) path: Option<String>,
}

impl Announcement {
    /// The announcement of one service that resolved in a browse.
    fn of(service: &ResolvedService) -> Self {
        Self {
            fullname: service.get_fullname().to_string(),
            addresses: service
                .get_addresses()
                .iter()
                .map(|address| address.to_ip_addr())
                .collect(),
            port: service.get_port(),
            path: service.get_property_val_str("path").map(str::to_string),
        }
    }

    /// The address to probe: the lowest IPv4 address, or the lowest IPv6 address when the service
    /// announces none, so that one service is always probed at one address.
    fn announced_address(&self) -> Option<IpAddr> {
        let mut addresses = self.addresses.clone();
        addresses.sort_unstable();
        addresses
            .iter()
            .copied()
            .find(IpAddr::is_ipv4)
            .or_else(|| addresses.first().copied())
    }

    /// The path of its OPC UA endpoint: the `path` TXT entry, `/` when it names none.
    fn opcua_path(&self) -> &str {
        self.path
            .as_deref()
            .map(str::trim)
            .filter(|path| !path.is_empty())
            .unwrap_or(OPCUA_ROOT_PATH)
    }
}

/// What one announced service gave: the results its probe answered, or the warning that names the
/// service that it could not prove (specification 11, "Sources" and "Confidence").
pub(super) enum Announced {
    /// The service's probe answered; these results, as the search reports them.
    Found(Vec<ProtocolFind>),
    /// It did not answer; this warning names it, and it is not a device.
    Warning(String),
}

/// Browses the protocol's announced service type until the search ends, and reports every service
/// that answers its protocol's probe (specification 11, "Sources").
pub(super) fn search(protocol: AnnouncedProtocol, sink: &SourceSink) {
    let browsed = crate::discovery::browse_services(
        protocol.service_type(),
        &|| sink.ended(),
        &mut |service| {
            if !sink.ended() {
                report(protocol, service, sink);
            }
        },
    );
    if let Err(error) = browsed {
        sink.warn(error);
    }
}

/// Reports one announced service: the answer of its protocol's probe is a device, and anything
/// else is a warning that names it.
fn report(protocol: AnnouncedProtocol, service: &ResolvedService, sink: &SourceSink) {
    let announcement = Announcement::of(service);
    let announced = match protocol {
        AnnouncedProtocol::Mqtt => announced_mqtt(&announcement, |target| {
            probe_mqtt(target, mqtt_probe_timeout(sink))
        }),
        AnnouncedProtocol::Opcua => announced_opcua(&announcement, |url| {
            crate::opcua::discover_opcua_client_endpoints(url, opcua_probe_timeout(sink))
        }),
    };
    match announced {
        Announced::Found(finds) => {
            for find in finds {
                sink.found(find);
            }
        }
        Announced::Warning(warning) => sink.warn(warning),
    }
}

/// The MQTT broker that an announced `_mqtt._tcp` service answers `ask` with, or the warning that
/// names a service its MQTT probe could not confirm (specification 11, "Sources").
pub(super) fn announced_mqtt<F>(announcement: &Announcement, ask: F) -> Announced
where
    F: Fn(SocketAddr) -> Option<DiscoveryProbeOutcome>,
{
    let Some(target) = mqtt_probe_target(announcement) else {
        return Announced::Warning(format!(
            "The announced MQTT service {} names no address.",
            announcement.fullname
        ));
    };
    if let Some(outcome) = ask(target) {
        return Announced::Found(vec![mqtt_find(DiscoverCandidate {
            source: MDNS_SOURCE,
            ..mqtt_candidate(target, outcome)
        })]);
    }
    Announced::Warning(format!(
        "The announced MQTT broker {target} did not answer its MQTT probe; it is not a device."
    ))
}

/// The endpoints that an announced `_opcua-tcp._tcp` service answers `ask` with, or the warning
/// that names a service whose GetEndpoints call did not answer (specification 11, "Sources").
pub(super) fn announced_opcua<F>(announcement: &Announcement, ask: F) -> Announced
where
    F: Fn(&str) -> Result<Vec<OpcUaClientEndpointInfo>, RuntimeError>,
{
    let Some(endpoint_url) = opcua_endpoint_url(announcement) else {
        return Announced::Warning(format!(
            "The announced OPC UA service {} names no address.",
            announcement.fullname
        ));
    };
    let answer = ask(endpoint_url.as_str());
    match opcua_endpoint_finds(endpoint_url.as_str(), answer, MDNS_SOURCE) {
        Ok(finds) => Announced::Found(finds),
        Err(warning) => Announced::Warning(warning),
    }
}

/// The MQTT probe target of an announced `_mqtt._tcp` service: its announced address and port.
pub(super) fn mqtt_probe_target(announcement: &Announcement) -> Option<SocketAddr> {
    announcement
        .announced_address()
        .map(|address| SocketAddr::new(address, announcement.port))
}

/// The GetEndpoints endpoint URL of an announced `_opcua-tcp._tcp` service:
/// `opc.tcp://<address>:<port><path>`, the `path` TXT entry, `/` without one.
pub(super) fn opcua_endpoint_url(announcement: &Announcement) -> Option<String> {
    announcement
        .announced_address()
        .map(|address| opcua_url_of(address, announcement))
}

/// The endpoint URL of an announced OPC UA service at `address`.
fn opcua_url_of(address: IpAddr, announcement: &Announcement) -> String {
    let port = announcement.port;
    let authority = match address {
        IpAddr::V4(address) => format!("{address}:{port}"),
        IpAddr::V6(address) => format!("[{address}]:{port}"),
    };
    format!("opc.tcp://{authority}{}", announcement.opcua_path())
}

#[cfg(test)]
#[path = "mdns_services_tests.rs"]
mod tests;
