//! Compare with the project (specification 11, "Compare with the project"): the devices of a search
//! against the devices that the project's configuration names. The result lists each configured
//! device that the search did not find, each found device that the project does not use, and each
//! found device whose identity differs from the configuration; it also says what the project makes of
//! every found device, which is what the Discover pane marks its cards with (specification 25 §2).
//!
//! A pure function of the search's devices and the project's devices: it reads no file, looks up no
//! host name and opens no connection, and the same inputs give the same result in the same order.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{json, Value};

use super::configured::{
    endpoint_host_and_port, ConfiguredDevice, ConfiguredEthercat, ConfiguredModbus,
    ConfiguredOpcua, Facts,
};
use super::model::{
    EthercatIdentity, FoundDevice, Identity, ProtocolFind, SearchStatus, SourceId, SourceState,
};

/// The schema version of the `comm.compare` reply.
pub const COMPARE_SCHEMA_VERSION: u32 = 1;

/// What the project makes of a found device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Standing {
    /// A configured device matches one of its results: the project uses it.
    InProject,
    /// No configured device matches any of its results.
    NotInProject,
    /// No result matches, but a configured device of one of its protocols could not be judged (a host
    /// name that no result carries, a unit ID that a search cannot probe), so one of them may be it.
    Undecided,
}

/// One fact of a matched device that differs from the configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct Difference {
    /// The protocol whose result differs.
    pub protocol: SourceId,
    /// The facts the project states.
    pub configured: Facts,
    /// The facts the device reports, for the same members.
    pub found: Facts,
    /// The names of the facts that differ, in the order of specification 11 ("Reply").
    pub fields: Vec<&'static str>,
}

/// What the project makes of one found device that has a result of a compared protocol.
#[derive(Debug, Clone, PartialEq)]
pub struct DeviceStanding {
    /// What the project makes of it.
    pub standing: Standing,
    /// The compared protocols found on it, in the order of the source table.
    pub protocols: Vec<SourceId>,
    /// The protocols whose results a configured device matched.
    pub matched: Vec<SourceId>,
    /// How its matched results differ from the configuration.
    pub differences: Vec<Difference>,
    /// Whether the project ignores the device (decision O14): it is never listed as unused.
    pub ignored: bool,
}

/// Why a configured device was not compared (specification 11, "Not compared").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// The search did not run the device's source.
    NotSearched,
    /// The device is named by a host name that no result carries.
    HostName,
    /// The device's Modbus unit ID lies outside 1–247, which a search cannot probe.
    UnitId,
}

impl Reason {
    /// Every reason, in the order of specification 11, "Not compared".
    pub const ALL: [Self; 3] = [Self::NotSearched, Self::HostName, Self::UnitId];

    /// The reason as the reply names it.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::NotSearched => "not_searched",
            Self::HostName => "host_name",
            Self::UnitId => "unit_id",
        }
    }

    /// The reason in words, which the command line and the Discover pane print beside a device that
    /// was not compared (specification 11, "Not compared"; specification 25 §2).
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Self::NotSearched => "its source did not run in this search",
            Self::HostName => "its host name is not looked up",
            Self::UnitId => "a search cannot probe this unit ID",
        }
    }
}

/// A configured device that the comparison could not judge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotCompared {
    /// The device.
    pub device: ConfiguredDevice,
    /// Why it was left out.
    pub reason: Reason,
}

/// The comparison of a search with a project.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Comparison {
    /// What the project makes of each found device that has a compared result, by device ID.
    pub devices: BTreeMap<String, DeviceStanding>,
    /// The configured devices that the search did not find.
    pub missing: Vec<ConfiguredDevice>,
    /// The configured devices that were left out of the comparison.
    pub not_compared: Vec<NotCompared>,
}

impl Comparison {
    /// The found devices that the project does not use and does not ignore, by device ID.
    pub fn unused(&self) -> impl Iterator<Item = (&str, &DeviceStanding)> + '_ {
        self.devices
            .iter()
            .filter(|(_, standing)| {
                standing.standing == Standing::NotInProject && !standing.ignored
            })
            .map(|(id, standing)| (id.as_str(), standing))
    }

    /// The matched results that differ from the configuration, by device ID and then in the order of
    /// the source table.
    pub fn differs(&self) -> impl Iterator<Item = (&str, &Difference)> + '_ {
        self.devices.iter().flat_map(|(id, standing)| {
            standing
                .differences
                .iter()
                .map(move |difference| (id.as_str(), difference))
        })
    }

    /// The `comm.compare` reply for the search `search_id` (specification 11, "Reply").
    #[must_use]
    pub fn report(&self, search_id: &str) -> Value {
        json!({
            "schema_version": COMPARE_SCHEMA_VERSION,
            "search_id": search_id,
            "missing": self.missing.iter().map(|device| json!({
                "protocol": device.protocol(),
                "configured": device.facts(),
            })).collect::<Vec<_>>(),
            "unused": self.unused().map(|(id, standing)| json!({
                "device_id": id,
                "protocols": standing.protocols,
            })).collect::<Vec<_>>(),
            "differs": self.differs().map(|(id, difference)| json!({
                "device_id": id,
                "protocol": difference.protocol,
                "configured": difference.configured,
                "found": difference.found,
                "fields": difference.fields,
            })).collect::<Vec<_>>(),
            "not_compared": self.not_compared.iter().map(|skipped| json!({
                "protocol": skipped.device.protocol(),
                "reason": skipped.reason.as_str(),
                "configured": skipped.device.facts(),
            })).collect::<Vec<_>>(),
        })
    }
}

/// Compares the devices of a search with the devices of a project.
///
/// `searched` are the sources the search ran (a skipped source did not search), `found` its
/// devices, `configured` the project's devices, and `ignored` the IDs of the devices that the
/// project ignores.
#[must_use]
pub fn compare(
    searched: &[SourceId],
    found: &[FoundDevice],
    configured: &[ConfiguredDevice],
    ignored: &BTreeSet<String>,
) -> Comparison {
    let mut comparison = Comparison {
        devices: standings(found, ignored),
        ..Comparison::default()
    };
    let mut candidates = Vec::new();
    for device in in_order(configured) {
        let reason = if !searched.contains(&device.protocol()) {
            Some(Reason::NotSearched)
        } else if device.unit_id_unsearchable() {
            Some(Reason::UnitId)
        } else {
            None
        };
        match reason {
            Some(reason) => comparison.not_compared.push(NotCompared { device, reason }),
            None => candidates.push(device),
        }
    }
    let (modules, others): (Vec<_>, Vec<_>) = candidates
        .into_iter()
        .partition(|device| device.protocol() == SourceId::Ethercat);
    let mut unmatched = Vec::new();
    for device in others {
        let hits: Vec<(&FoundDevice, &ProtocolFind)> = found
            .iter()
            .flat_map(|candidate| candidate.finds.iter().map(move |find| (candidate, find)))
            .filter(|(candidate, find)| matches(&device, candidate, find))
            .collect();
        if hits.is_empty() {
            unmatched.push(device);
        } else {
            record(&mut comparison, &device, &hits);
        }
    }
    unmatched.extend(pair_ethercat(&mut comparison, &modules, found));
    for device in in_order(&unmatched) {
        if device.named_by_unplaced_host() {
            comparison.not_compared.push(NotCompared {
                device,
                reason: Reason::HostName,
            });
        } else {
            comparison.missing.push(device);
        }
    }
    comparison
        .not_compared
        .sort_by(|left, right| order(&left.device, &right.device));
    decide(&mut comparison);
    comparison
}

/// The sources that ran in `status`: every source it lists that was not skipped.
#[must_use]
pub fn searched(status: &SearchStatus) -> Vec<SourceId> {
    status
        .sources
        .iter()
        .filter(|source| source.state != SourceState::Skipped)
        .map(|source| source.id)
        .collect()
}

/// The devices of a `comm.search.status` answer that `compare` reads: the sources that ran, and the
/// devices with every result this build can read. A source or a result of a protocol this build does
/// not know is left out, as the Discover pane leaves it out (specification 25 §2).
#[must_use]
pub fn read_status(status: &Value) -> (Vec<SourceId>, Vec<FoundDevice>) {
    let searched = list(status, "sources")
        .iter()
        .filter_map(|source| {
            let id = serde_json::from_value::<SourceId>(source.get("id")?.clone()).ok()?;
            (source.get("state")?.as_str()? != "skipped").then_some(id)
        })
        .collect();
    let devices = list(status, "devices")
        .iter()
        .filter_map(read_device)
        .collect();
    (searched, devices)
}

fn list<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// One device with the results that can be read; `None` when the device itself cannot be read.
fn read_device(value: &Value) -> Option<FoundDevice> {
    let finds: Vec<ProtocolFind> = list(value, "finds")
        .iter()
        .filter_map(|find| serde_json::from_value(find.clone()).ok())
        .collect();
    let mut shell = value.as_object()?.clone();
    shell.insert("finds".to_string(), Value::Array(Vec::new()));
    let mut device: FoundDevice = serde_json::from_value(Value::Object(shell)).ok()?;
    device.finds = finds;
    Some(device)
}

/// The devices in the order of the source table, then by their own order.
fn in_order(devices: &[ConfiguredDevice]) -> Vec<ConfiguredDevice> {
    let mut ordered = devices.to_vec();
    ordered.sort_by(order);
    ordered
}

fn order(left: &ConfiguredDevice, right: &ConfiguredDevice) -> std::cmp::Ordering {
    left.protocol()
        .cmp(&right.protocol())
        .then_with(|| left.cmp(right))
}

/// One provisional standing for each device that has a result of a compared protocol.
fn standings(
    found: &[FoundDevice],
    ignored: &BTreeSet<String>,
) -> BTreeMap<String, DeviceStanding> {
    found
        .iter()
        .filter_map(|device| {
            let protocols: BTreeSet<SourceId> = device
                .finds
                .iter()
                .map(|find| find.protocol)
                .filter(|protocol| ConfiguredDevice::is_compared(*protocol))
                .collect();
            (!protocols.is_empty()).then(|| {
                (
                    device.id.clone(),
                    DeviceStanding {
                        standing: Standing::NotInProject,
                        protocols: protocols.into_iter().collect(),
                        matched: Vec::new(),
                        differences: Vec::new(),
                        ignored: ignored.contains(&device.id),
                    },
                )
            })
        })
        .collect()
}

/// Fixes each standing once every configured device is placed (specification 11, "Host names").
fn decide(comparison: &mut Comparison) {
    let undecided: BTreeSet<SourceId> = comparison
        .not_compared
        .iter()
        .filter(|skipped| skipped.reason != Reason::NotSearched)
        .map(|skipped| skipped.device.protocol())
        .collect();
    for standing in comparison.devices.values_mut() {
        standing.matched.sort();
        standing.matched.dedup();
        standing
            .differences
            .sort_by_key(|difference| difference.protocol);
        standing.standing = if !standing.matched.is_empty() {
            Standing::InProject
        } else if standing
            .protocols
            .iter()
            .any(|protocol| undecided.contains(protocol))
        {
            Standing::Undecided
        } else {
            Standing::NotInProject
        };
    }
}

/// Records that `device` matched `hits`, and how they differ from it.
fn record(
    comparison: &mut Comparison,
    device: &ConfiguredDevice,
    hits: &[(&FoundDevice, &ProtocolFind)],
) {
    let mut groups: Vec<(&FoundDevice, Vec<&ProtocolFind>)> = Vec::new();
    for &(found, find) in hits {
        match groups.iter_mut().find(|(known, _)| known.id == found.id) {
            Some((_, finds)) => finds.push(find),
            None => groups.push((found, vec![find])),
        }
    }
    for (found, finds) in groups {
        let Some(standing) = comparison.devices.get_mut(&found.id) else {
            continue;
        };
        standing.matched.push(device.protocol());
        if let Some(difference) = difference(device, &finds) {
            standing.differences.push(difference);
        }
    }
}

/// Whether the result `find` of `found` is the configured `device` (specification 11, "Matching");
/// EtherCAT modules are paired apart, by `pair_ethercat`.
fn matches(device: &ConfiguredDevice, found: &FoundDevice, find: &ProtocolFind) -> bool {
    match (device, &find.identity) {
        (ConfiguredDevice::ModbusTcp(configured), Identity::Modbus(identity)) => {
            address_param(find, "address") == Some(configured.address)
                && identity.unit_id == configured.unit_id
        }
        (ConfiguredDevice::Mqtt(configured), Identity::Mqtt(_)) => address_param(find, "broker")
            .is_some_and(|broker| {
                broker.port() == configured.port && configured.host.is(&broker.ip().to_string())
            }),
        (ConfiguredDevice::Opcua(configured), Identity::Opcua(identity)) => {
            found
                .addresses
                .iter()
                .any(|address| configured.host.is(address))
                && endpoint_host_and_port(&identity.endpoint_url)
                    .is_ok_and(|(_, port)| port == configured.port)
        }
        (ConfiguredDevice::Ads(configured), Identity::Ads(identity)) => {
            identity.ams_net_id.trim() == configured.ams_net_id
        }
        _ => false,
    }
}

/// The address a result was probed at: the IP address and port in its parameter `key`.
fn address_param(find: &ProtocolFind, key: &str) -> Option<std::net::SocketAddr> {
    find.params.get(key)?.as_str()?.trim().parse().ok()
}

/// How the matched results `finds` differ from the configured `device`
/// (specification 11, "Differences"); EtherCAT is judged by `pair_ethercat`.
fn difference(device: &ConfiguredDevice, finds: &[&ProtocolFind]) -> Option<Difference> {
    match device {
        ConfiguredDevice::ModbusTcp(configured) => modbus_difference(configured, finds),
        ConfiguredDevice::Opcua(configured) => opcua_difference(configured, finds),
        ConfiguredDevice::Ethercat(_) | ConfiguredDevice::Mqtt(_) | ConfiguredDevice::Ads(_) => {
            None
        }
    }
}

/// A Modbus device differs when the driver states an identity that the device reports another value
/// for, or none.
fn modbus_difference(configured: &ConfiguredModbus, finds: &[&ProtocolFind]) -> Option<Difference> {
    let Some(Identity::Modbus(identity)) = finds.first().map(|find| &find.identity) else {
        return None;
    };
    let mut found = Facts::new();
    found.insert("address".to_string(), json!(configured.address.to_string()));
    found.insert("unit_id".to_string(), json!(configured.unit_id));
    let mut fields = Vec::new();
    for (key, stated, reported) in [
        (
            "vendor_name",
            &configured.vendor_name,
            &identity.vendor_name,
        ),
        (
            "product_code",
            &configured.product_code,
            &identity.product_code,
        ),
        ("revision", &configured.revision, &identity.revision),
    ] {
        let Some(stated) = stated else {
            continue;
        };
        let reported = reported.as_deref().map(str::trim);
        found.insert(key.to_string(), json!(reported));
        if reported != Some(stated.as_str()) {
            fields.push(key);
        }
    }
    (!fields.is_empty()).then(|| Difference {
        protocol: SourceId::ModbusTcp,
        configured: ConfiguredDevice::ModbusTcp(configured.clone()).facts(),
        found,
        fields,
    })
}

/// An OPC UA connection differs when none of the server's endpoints on its port announces its
/// security policy together with its mode.
fn opcua_difference(configured: &ConfiguredOpcua, finds: &[&ProtocolFind]) -> Option<Difference> {
    let announced: Vec<String> = finds
        .iter()
        .filter_map(|find| match &find.identity {
            Identity::Opcua(identity) => Some(format!(
                "{} / {}",
                identity.security_policy.as_deref()?,
                identity.security_mode.as_deref()?
            )),
            _ => None,
        })
        .collect();
    if announced.contains(&configured.security()) {
        return None;
    }
    let mut found = Facts::new();
    found.insert(
        "address".to_string(),
        json!(configured.host.with_port(configured.port)),
    );
    found.insert(
        "security".to_string(),
        json!(Some(announced.join(", ")).filter(|pairs| !pairs.is_empty())),
    );
    Some(Difference {
        protocol: SourceId::Opcua,
        configured: ConfiguredDevice::Opcua(configured.clone()).facts(),
        found,
        fields: vec!["security"],
    })
}

/// Pairs the configured EtherCAT `modules` with the SubDevices of `found`, one adapter at a time
/// (specification 11, "Matching"), and records the pairs. Answers the modules left over.
fn pair_ethercat(
    comparison: &mut Comparison,
    modules: &[ConfiguredDevice],
    found: &[FoundDevice],
) -> Vec<ConfiguredDevice> {
    let configured: Vec<&ConfiguredEthercat> = modules
        .iter()
        .filter_map(|device| match device {
            ConfiguredDevice::Ethercat(module) => Some(module),
            _ => None,
        })
        .collect();
    let subdevices: Vec<(&FoundDevice, &EthercatIdentity)> = found
        .iter()
        .flat_map(|device| {
            device
                .finds
                .iter()
                .map(move |find| (device, &find.identity))
        })
        .filter_map(|(device, identity)| match identity {
            Identity::Ethercat(subdevice) => Some((device, subdevice)),
            _ => None,
        })
        .collect();
    let on_bus = |module: &ConfiguredEthercat, index: usize| {
        module
            .adapter
            .eq_ignore_ascii_case(&subdevices[index].1.adapter)
    };
    let same_model = |module: &ConfiguredEthercat, index: usize| {
        module
            .model
            .eq_ignore_ascii_case(subdevices[index].1.model.trim())
    };
    let position = |index: usize| subdevices[index].1.slot;

    let mut module_for: Vec<Option<usize>> = vec![None; configured.len()];
    let mut taken = vec![false; subdevices.len()];
    // First step: the same model at the same position.
    for (index, module) in configured.iter().copied().enumerate() {
        let hit = (0..subdevices.len()).find(|&candidate| {
            !taken[candidate]
                && on_bus(module, candidate)
                && same_model(module, candidate)
                && position(candidate) == module.position
        });
        if let Some(candidate) = hit {
            taken[candidate] = true;
            module_for[index] = Some(candidate);
        }
    }
    // Second step: the same model, nearest in position, the lower position on a tie.
    for (index, module) in configured.iter().copied().enumerate() {
        if module_for[index].is_some() {
            continue;
        }
        let hit = (0..subdevices.len())
            .filter(|&candidate| {
                !taken[candidate] && on_bus(module, candidate) && same_model(module, candidate)
            })
            .min_by_key(|&candidate| {
                (
                    module.position.abs_diff(position(candidate)),
                    position(candidate),
                )
            });
        if let Some(candidate) = hit {
            taken[candidate] = true;
            module_for[index] = Some(candidate);
        }
    }
    // Third step: the SubDevice at the module's position, whatever its model.
    for (index, module) in configured.iter().copied().enumerate() {
        if module_for[index].is_some() {
            continue;
        }
        let hit = (0..subdevices.len()).find(|&candidate| {
            !taken[candidate] && on_bus(module, candidate) && position(candidate) == module.position
        });
        if let Some(candidate) = hit {
            taken[candidate] = true;
            module_for[index] = Some(candidate);
        }
    }

    let mut left_over = Vec::new();
    for (index, module) in configured.iter().copied().enumerate() {
        let Some(candidate) = module_for[index] else {
            left_over.push(ConfiguredDevice::Ethercat(module.clone()));
            continue;
        };
        let (device, subdevice) = subdevices[candidate];
        let Some(standing) = comparison.devices.get_mut(&device.id) else {
            continue;
        };
        standing.matched.push(SourceId::Ethercat);
        let mut fields = Vec::new();
        if subdevice.slot != module.position {
            fields.push("position");
        }
        if !same_model(module, candidate) {
            fields.push("model");
        }
        if !fields.is_empty() {
            let mut reported = Facts::new();
            reported.insert("adapter".to_string(), json!(subdevice.adapter));
            reported.insert("position".to_string(), json!(subdevice.slot));
            reported.insert("model".to_string(), json!(subdevice.model));
            standing.differences.push(Difference {
                protocol: SourceId::Ethercat,
                configured: ConfiguredDevice::Ethercat(module.clone()).facts(),
                found: reported,
                fields,
            });
        }
    }
    left_over
}

#[cfg(test)]
#[path = "compare_tests.rs"]
mod tests;
