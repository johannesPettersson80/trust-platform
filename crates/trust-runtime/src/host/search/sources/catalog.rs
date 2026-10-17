//! Which requested sources a search runs, what each one probes, and why the others are skipped
//! (specification 11, "Sources").

use std::time::Duration;

use smol_str::SmolStr;

use crate::ads::onboarding::{runtime_address_candidates_from_interfaces, RuntimeAddressCandidate};

use super::super::model::SourceId;
use super::super::request::ValidSearch;
use super::super::runner::{SourcePlan, SourceSink};
use super::modbus_tcp::{self, ModbusTarget};
use super::scope::ipv4_targets_for_cidr;
use super::{ads, ethercat, gpio, mqtt, opcua, trust_runtimes};
use super::{serial, usb};
use super::{SearchHost, SourceContext};

/// A probe of a known address waits this long for an answer.
const KNOWN_ADDRESS_PROBE: Duration = Duration::from_millis(1_000);
/// A probe of a subnet host waits as long as a `comm.discover` probe does by default.
const SUBNET_PROBE: Duration = Duration::from_millis(150);

/// The plan of every requested source, in the order of the source table.
pub(crate) fn plan(search: &ValidSearch, host: &SearchHost) -> Vec<(SourceId, SourcePlan)> {
    let context = SourceContext {
        hosts: search.hosts.clone(),
        cidr: search.cidr.clone(),
        ethercat_adapters: search.ethercat_adapters.clone(),
        modbus_unit_ids: search.modbus_unit_ids.clone(),
        ads_ports: search.ads_ports.clone(),
        runtime: match host {
            SearchHost::ThisHost => None,
            SearchHost::Runtime(facts) => Some(facts.clone()),
        },
    };
    search
        .sources
        .iter()
        .map(|&source| {
            let plan = plan_source(source, search.fieldbus_confirmed, &context);
            (source, plan)
        })
        .collect()
}

fn plan_source(source: SourceId, fieldbus_confirmed: bool, context: &SourceContext) -> SourcePlan {
    match source {
        SourceId::TrustRuntime => match &context.runtime {
            Some(runtime) if !runtime.discovery_enabled => skip(
                "Discovery is switched off on this runtime ([runtime.discovery] enabled = false).",
            ),
            _ => run(context, trust_runtimes::search),
        },
        SourceId::Ads if !cfg!(feature = "ads-wire") => {
            skip("This build of truST does not include ADS.")
        }
        SourceId::Ads => run(context, ads::search),
        SourceId::ModbusTcp => match modbus_targets(context) {
            Ok(targets) => SourcePlan::Run(Box::new(move |sink: &SourceSink| {
                modbus_tcp::search(&targets, sink);
            })),
            Err(reason) => SourcePlan::Skip(reason),
        },
        SourceId::Mqtt if context.hosts.is_empty() => {
            skip("MQTT brokers are searched at known addresses, and this search has none.")
        }
        SourceId::Mqtt => run(context, mqtt::search),
        SourceId::Opcua if !cfg!(feature = "opcua-wire") => {
            skip("This build of truST does not include OPC UA.")
        }
        SourceId::Opcua if context.hosts.is_empty() => {
            skip("OPC UA servers are searched at known addresses, and this search has none.")
        }
        SourceId::Opcua => run(context, opcua::search),
        SourceId::Ethercat if context.runtime.is_none() => {
            skip("EtherCAT is searched inside a runtime, on the computer with the bus adapter.")
        }
        SourceId::Ethercat if !fieldbus_confirmed => {
            skip("An EtherCAT scan runs only when the search confirms it (confirm_fieldbus_scan).")
        }
        SourceId::Ethercat => run(context, ethercat::search),
        SourceId::Gpio if context.runtime.is_none() => {
            skip("GPIO is searched inside a runtime, on the computer with the GPIO lines.")
        }
        SourceId::Gpio => SourcePlan::Run(Box::new(gpio::search)),
        SourceId::Serial => match serial::unavailable() {
            Some(reason) => SourcePlan::Skip(reason),
            None => SourcePlan::Run(Box::new(serial::search)),
        },
        SourceId::Usb => SourcePlan::Run(Box::new(usb::search)),
        SourceId::Bluetooth => skip("Bluetooth search is not available in this truST version yet."),
    }
}

fn run(context: &SourceContext, source: fn(&SourceContext, &SourceSink)) -> SourcePlan {
    let context = context.clone();
    SourcePlan::Run(Box::new(move |sink: &SourceSink| source(&context, sink)))
}

fn skip(reason: &str) -> SourcePlan {
    SourcePlan::Skip(reason.to_string())
}

/// What Modbus TCP probes: the known addresses and the hosts of the subnet, or without either the
/// private `/24` subnets of the origin computer's interfaces, each at every requested unit ID. The
/// reason it has nothing to probe is the error.
fn modbus_targets(context: &SourceContext) -> Result<Vec<ModbusTarget>, String> {
    let mut hosts: Vec<(String, Duration)> = context
        .hosts
        .iter()
        .map(|host| (host.clone(), KNOWN_ADDRESS_PROBE))
        .collect();
    let subnets = match &context.cidr {
        Some(cidr) => vec![cidr.clone()],
        None if hosts.is_empty() => {
            let candidates = runtime_address_candidates_from_interfaces()
                .map_err(|error| format!("The network interfaces could not be read: {error}."))?;
            let allowed = context
                .runtime
                .as_ref()
                .map(|runtime| runtime.discovery_interfaces.as_slice())
                .unwrap_or_default();
            let subnets = default_subnets(&candidates, allowed);
            if subnets.is_empty() {
                return Err(
                    "No private IPv4 network was found to search; give a known address or subnet."
                        .to_string(),
                );
            }
            subnets
        }
        None => Vec::new(),
    };
    for subnet in subnets {
        for address in ipv4_targets_for_cidr(&subnet)? {
            hosts.push((address.to_string(), SUBNET_PROBE));
        }
    }
    let mut targets = Vec::with_capacity(hosts.len() * context.modbus_unit_ids.len());
    for (host, timeout) in hosts {
        for &unit_id in &context.modbus_unit_ids {
            targets.push(ModbusTarget {
                host: host.clone(),
                unit_id,
                timeout,
            });
        }
    }
    Ok(targets)
}

/// The `/24` of each private IPv4 address (RFC 1918) of the interfaces that `allowed` names, or of
/// every interface when it names none; each subnet once, in interface order.
pub(crate) fn default_subnets(
    candidates: &[RuntimeAddressCandidate],
    allowed: &[SmolStr],
) -> Vec<String> {
    let mut subnets = Vec::new();
    for candidate in candidates {
        let interface_allowed = allowed.is_empty()
            || candidate
                .nic
                .as_deref()
                .is_some_and(|nic| allowed.iter().any(|name| name == nic));
        if !interface_allowed {
            continue;
        }
        let Ok(address) = candidate.ip.parse::<std::net::Ipv4Addr>() else {
            continue;
        };
        if !address.is_private() {
            continue;
        }
        let [a, b, c, _] = address.octets();
        let subnet = format!("{a}.{b}.{c}.0/24");
        if !subnets.contains(&subnet) {
            subnets.push(subnet);
        }
    }
    subnets
}
