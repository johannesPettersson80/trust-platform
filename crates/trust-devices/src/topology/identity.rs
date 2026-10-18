//! Owner-scoped identities (specification 25 §5.6).
//!
//! A wire ID means something only inside the host, container and runtime that report it: two
//! runtimes may both report `RESOURCE` and `endpoint:RESOURCE:mesh`. Normalization gives every
//! container, runtime, endpoint, configured mesh external and link an identity that encodes its
//! lossless owner tuple: `trust-fleet-v1:` and the unpadded base64url form of a JSON array of
//! strings and nulls, the scheme the panel used before it moved to Rust. The tuple of a runtime
//! that a target reached also holds its instance, so that two runtimes of one host with the same
//! runtime ID stay two. Host and shared-system IDs stay global, and so does an external system
//! unless it is the configured mesh peer of exactly one runtime.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use base64::prelude::{Engine as _, BASE64_URL_SAFE_NO_PAD};
use serde_json::Value;
use trust_runtime::fleet_topology::FleetLink;

use super::{Runtime, Topology};

const PREFIX: &str = "trust-fleet-v1:";

/// Encodes `parts` as an identity. The encoding is injective: [`decode`] gives the parts back.
pub(crate) fn encode(parts: &[Option<&str>]) -> String {
    let json = Value::Array(
        parts
            .iter()
            .map(|part| part.map_or(Value::Null, |text| Value::String(text.to_owned())))
            .collect(),
    )
    .to_string();
    format!("{PREFIX}{}", BASE64_URL_SAFE_NO_PAD.encode(json))
}

/// The parts of the identity `value`, or `None` when it is not one. Only the canonical encoding
/// is read: no padding and no trailing bits.
pub(super) fn decode(value: &str) -> Option<Vec<Option<String>>> {
    let json = BASE64_URL_SAFE_NO_PAD
        .decode(value.strip_prefix(PREFIX)?)
        .ok()?;
    serde_json::from_slice(&json).ok()
}

/// The runtime ID that a runtime's identity holds, as its runtime reported it.
pub(super) fn raw_runtime_id(id: &str) -> String {
    raw_part(id, "runtime", 3)
}

/// What names an endpoint's connection inside its runtime: the raw endpoint ID after
/// `endpoint:<runtime ID>:`, such as `modbus_tcp:1`. Runtimes number the connections of their
/// configuration in configuration order (specification 11, "Offline communication operation and
/// fleet-topology truth"), so one configured connection has the same key under any runtime ID.
/// `None` when the identity is no endpoint's or its raw ID has another form.
pub(super) fn connection_key(id: &str) -> Option<String> {
    let parts = decode(id)?;
    if part(&parts, 0) != Some("endpoint") {
        return None;
    }
    let runtime = part(&parts, 3)?;
    part(&parts, 4)?
        .strip_prefix("endpoint:")?
        .strip_prefix(runtime)?
        .strip_prefix(':')
        .map(str::to_owned)
}

/// The identity of the connection that a `comm.apply` wrote, from the runtime's identity
/// `runtime` and the `instance_id` its answer carries (`{protocol}:{index}`, specification 11
/// §6.8.1a; design §4.3 B7, I41 and I42): the runtime's owner tuple with the endpoint at the
/// written position in its configuration. It needs no second read of the topology. `None` when
/// `runtime` is no runtime's identity or `instance_id` names no connection.
#[must_use]
pub(crate) fn connection_id(runtime: &str, instance_id: &str) -> Option<String> {
    let parts = decode(runtime)?;
    if part(&parts, 0) != Some("runtime") {
        return None;
    }
    let runtime_id = part(&parts, 3)?;
    let raw = endpoint_raw(runtime_id, instance_id)?;
    let owner = Owner {
        host: part(&parts, 1)?.to_owned(),
        container: parts.get(2).cloned().flatten(),
        runtime: runtime_id.to_owned(),
        instance: parts.get(4).cloned().flatten(),
    };
    Some(owner.scoped("endpoint", &raw))
}

/// The `instance_id` (`{protocol}:{index}`) that `comm.apply` answers for the connection
/// `connection`, so that the same connection can be removed, disabled or enabled again
/// (specification 11 §6.8.1a; design §4.3 B7, R4). The inverse of [`connection_id`] for a
/// connection of a runtime. `None` when `connection` is no connection's identity.
#[must_use]
pub(crate) fn connection_instance_id(connection: &str) -> Option<String> {
    let key = connection_key(connection)?;
    Some(if key.contains(':') {
        key
    } else {
        format!("{key}:0")
    })
}

/// The raw endpoint ID of the connection at the configuration position `instance_id`, as a
/// runtime numbers its connections (specification 11, "Offline communication operation and
/// fleet-topology truth"): `endpoint:<runtime ID>:<protocol>` for the first one and
/// `endpoint:<runtime ID>:<protocol>:<index>` after it.
fn endpoint_raw(runtime_id: &str, instance_id: &str) -> Option<String> {
    let key = instance_id.trim();
    if key.is_empty() {
        return None;
    }
    let (protocol, index) = match key.rsplit_once(':') {
        Some((protocol, index)) if !protocol.is_empty() => (protocol, index.parse::<usize>().ok()?),
        Some(_) => return None,
        None => (key, 0),
    };
    Some(if index == 0 {
        format!("endpoint:{runtime_id}:{protocol}")
    } else {
        format!("endpoint:{runtime_id}:{protocol}:{index}")
    })
}

/// Normalizes a snapshot: every container, runtime, endpoint, configured mesh external and link
/// gets its owner-scoped identity, and every reference to one is rewritten. A reference with more
/// than one possible owner drops the link that uses it, omits the configured mesh external it
/// would bind, and leaves the `used_by` list it is in; the nodes themselves stay. Every other
/// value stays as reported, and normalizing a normalized snapshot changes nothing.
pub(super) fn normalize(topology: &Topology) -> Topology {
    let mut topology = topology.clone();
    let mut runtimes = References::default();
    let mut endpoints = References::default();
    for host in &mut topology.hosts {
        for runtime in &mut host.runtimes {
            scope_runtime(runtime, &host.id, None, &mut runtimes, &mut endpoints);
        }
        for container in &mut host.containers {
            let raw = raw_part(&container.id, "container", 2);
            container.id = encode(&[Some("container"), Some(&host.id), Some(&raw)]);
            for runtime in &mut container.runtimes {
                scope_runtime(runtime, &host.id, Some(&raw), &mut runtimes, &mut endpoints);
            }
        }
    }

    let mut records: Vec<External> = topology
        .external
        .iter()
        .map(|external| {
            let raw = raw_part(&external.id, "mesh-external", 4);
            External {
                current: external.id.clone(),
                normalized: raw.clone(),
                raw,
                omitted: false,
            }
        })
        .collect();
    for (record, external) in records.iter_mut().zip(&topology.external) {
        if external
            .via_protocol
            .iter()
            .any(|protocol| protocol == "mesh")
        {
            bind_mesh_external(record, &topology.links, &endpoints);
        }
    }
    let mut externals = References::default();
    for record in records.iter().filter(|record| !record.omitted) {
        let names = [
            record.raw.as_str(),
            record.current.as_str(),
            record.normalized.as_str(),
        ];
        externals.add(names, &record.normalized, &());
    }
    let external = std::mem::take(&mut topology.external);
    topology.external = external
        .into_iter()
        .zip(&records)
        .filter(|(_, record)| !record.omitted)
        .map(|(mut external, record)| {
            external.id.clone_from(&record.normalized);
            external
        })
        .collect();

    let nodes = Nodes {
        endpoints: &endpoints,
        runtimes: &runtimes,
        externals: &externals,
        shared: topology
            .shared
            .iter()
            .map(|shared| shared.id.clone())
            .collect(),
    };
    let links = std::mem::take(&mut topology.links);
    topology.links = links
        .into_iter()
        .filter_map(|link| scope_link(link, &nodes))
        .collect();

    for shared in &mut topology.shared {
        let mut used_by: Vec<String> = Vec::new();
        for reference in &shared.used_by {
            if let Resolution::Unique((id, _)) = runtimes.resolve(reference)
                && !used_by.iter().any(|known| known == id)
            {
                used_by.push(id.to_owned());
            }
        }
        shared.used_by = used_by;
    }
    topology
}

/// The lossless owner of a runtime's nodes: host, optional container and runtime ID, each raw,
/// and the runtime's instance when a target reached it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Owner {
    host: String,
    container: Option<String>,
    runtime: String,
    instance: Option<String>,
}

impl Owner {
    fn runtime_id(&self) -> String {
        self.identity("runtime", None)
    }

    /// The identity of an endpoint or a configured mesh external that this runtime owns.
    fn scoped(&self, kind: &str, raw: &str) -> String {
        self.identity(kind, Some(raw))
    }

    /// `kind`, the owner tuple and `raw`; the instance comes last, so that the raw parts keep
    /// their places.
    fn identity(&self, kind: &str, raw: Option<&str>) -> String {
        let mut parts = vec![
            Some(kind),
            Some(self.host.as_str()),
            self.container.as_deref(),
            Some(self.runtime.as_str()),
        ];
        if let Some(raw) = raw {
            parts.push(Some(raw));
        }
        if let Some(instance) = &self.instance {
            parts.push(Some(instance.as_str()));
        }
        encode(&parts)
    }
}

/// An external system and the identity it gets.
struct External {
    raw: String,
    current: String,
    normalized: String,
    omitted: bool,
}

/// Every name under which a node may be referenced, with each identity that the name may mean.
struct References<T> {
    by_name: HashMap<String, BTreeMap<String, T>>,
}

impl<T> Default for References<T> {
    fn default() -> Self {
        Self {
            by_name: HashMap::new(),
        }
    }
}

enum Resolution<T> {
    Missing,
    Ambiguous,
    Unique(T),
}

impl<T: Clone> References<T> {
    fn add(&mut self, names: [&str; 3], identity: &str, value: &T) {
        for name in names {
            self.by_name
                .entry(name.to_owned())
                .or_default()
                .insert(identity.to_owned(), value.clone());
        }
    }

    /// What `reference` means: looked up as written, else by the raw ID inside it.
    fn resolve(&self, reference: &str) -> Resolution<(&str, &T)> {
        let Some(candidates) = self
            .by_name
            .get(reference)
            .or_else(|| self.by_name.get(raw_reference(reference).as_str()))
        else {
            return Resolution::Missing;
        };
        let mut candidates = candidates.iter();
        match (candidates.next(), candidates.next()) {
            (Some((identity, value)), None) => Resolution::Unique((identity.as_str(), value)),
            (Some(_), Some(_)) => Resolution::Ambiguous,
            (None, _) => Resolution::Missing,
        }
    }
}

/// The nodes that a link may name, in the order they are tried.
struct Nodes<'a> {
    endpoints: &'a References<Owner>,
    runtimes: &'a References<Owner>,
    externals: &'a References<()>,
    shared: BTreeSet<String>,
}

impl Nodes<'_> {
    /// The identity of the node `reference` names, or `None` when it names none or more than
    /// one possible owner.
    fn resolve(&self, reference: &str) -> Option<String> {
        match self.endpoints.resolve(reference) {
            Resolution::Unique((id, _)) => return Some(id.to_owned()),
            Resolution::Ambiguous => return None,
            Resolution::Missing => {}
        }
        match self.runtimes.resolve(reference) {
            Resolution::Unique((id, _)) => return Some(id.to_owned()),
            Resolution::Ambiguous => return None,
            Resolution::Missing => {}
        }
        match self.externals.resolve(reference) {
            Resolution::Unique((id, ())) => return Some(id.to_owned()),
            Resolution::Ambiguous => return None,
            Resolution::Missing => {}
        }
        let raw = raw_reference(reference);
        [reference, raw.as_str()]
            .into_iter()
            .find(|id| self.shared.contains(*id))
            .map(str::to_owned)
    }
}

fn scope_runtime(
    runtime: &mut Runtime,
    host: &str,
    container: Option<&str>,
    runtimes: &mut References<Owner>,
    endpoints: &mut References<Owner>,
) {
    let owner = Owner {
        host: host.to_owned(),
        container: container.map(str::to_owned),
        runtime: raw_runtime_id(&runtime.id),
        instance: runtime.instance.clone(),
    };
    let id = owner.runtime_id();
    runtimes.add([owner.runtime.as_str(), &runtime.id, &id], &id, &owner);
    runtime.id = id;
    for endpoint in &mut runtime.endpoints {
        let raw = raw_part(&endpoint.id, "endpoint", 4);
        let scoped = owner.scoped("endpoint", &raw);
        endpoints.add([raw.as_str(), &endpoint.id, &scoped], &scoped, &owner);
        endpoint.id = scoped;
    }
}

/// Binds a configured mesh external to the one runtime whose endpoint a mesh link pairs it with.
/// With no such runtime it stays global; with more than one, or an endpoint of more than one
/// possible owner, it is omitted.
fn bind_mesh_external(record: &mut External, links: &[FleetLink], endpoints: &References<Owner>) {
    let mut owners = BTreeSet::new();
    let mut ambiguous = false;
    for link in links.iter().filter(|link| link.protocol == "mesh") {
        for (end, other) in [(&link.from, &link.to), (&link.to, &link.from)] {
            if references_external(end, record) {
                match endpoints.resolve(other) {
                    Resolution::Unique((_, owner)) => {
                        owners.insert(owner);
                    }
                    Resolution::Ambiguous => ambiguous = true,
                    Resolution::Missing => {}
                }
            }
        }
    }
    if ambiguous || owners.len() > 1 {
        record.omitted = true;
    } else if let Some(owner) = owners.first() {
        record.normalized = owner.scoped("mesh-external", &record.raw);
    }
}

fn references_external(reference: &str, record: &External) -> bool {
    if reference == record.current {
        return true;
    }
    if decode(&record.current).is_some_and(|parts| part(&parts, 0) == Some("mesh-external")) {
        return false;
    }
    raw_reference(reference) == record.raw
}

/// A link whose two ends each name one node, with an identity made of its raw wire identity and
/// both resolved ends; `None` when an end names no node or more than one possible owner.
fn scope_link(mut link: FleetLink, nodes: &Nodes<'_>) -> Option<FleetLink> {
    let wire = WireLink::of(&link);
    let from = nodes.resolve(&link.from)?;
    let to = nodes.resolve(&link.to)?;
    let reach = if link.same_host {
        "same-host"
    } else {
        "remote"
    };
    link.id = Some(encode(&[
        Some("link"),
        wire.id.as_deref(),
        Some(&wire.from),
        Some(&wire.to),
        Some(&wire.protocol),
        wire.role.as_deref(),
        Some(&wire.direction),
        Some(reach),
        Some(&from),
        Some(&to),
    ]));
    link.from = from;
    link.to = to;
    Some(link)
}

/// A link's identity as its runtime reported it.
struct WireLink {
    id: Option<String>,
    from: String,
    to: String,
    protocol: String,
    role: Option<String>,
    direction: String,
}

impl WireLink {
    fn of(link: &FleetLink) -> Self {
        if let Some(parts) = link.id.as_deref().and_then(decode)
            && part(&parts, 0) == Some("link")
            && parts.len() > 6
            && [2, 3, 4, 6]
                .iter()
                .all(|&index| part(&parts, index).is_some())
        {
            let text = |index| part(&parts, index).unwrap_or_default().to_owned();
            return Self {
                id: parts[1].clone(),
                from: text(2),
                to: text(3),
                protocol: text(4),
                role: parts[5].clone(),
                direction: text(6),
            };
        }
        Self {
            id: link.id.clone(),
            from: raw_reference(&link.from),
            to: raw_reference(&link.to),
            protocol: link.protocol.clone(),
            role: link.role.clone(),
            direction: link.direction.clone(),
        }
    }
}

fn part(parts: &[Option<String>], index: usize) -> Option<&str> {
    parts.get(index)?.as_deref()
}

/// The raw ID at `index` when `value` is an identity of `kind`, else `value` itself.
fn raw_part(value: &str, kind: &str, index: usize) -> String {
    decode(value)
        .filter(|parts| part(parts, 0) == Some(kind))
        .and_then(|parts| part(&parts, index).map(str::to_owned))
        .unwrap_or_else(|| value.to_owned())
}

/// The raw ID inside a runtime, endpoint or mesh-external identity, else `value` itself.
fn raw_reference(value: &str) -> String {
    let Some(parts) = decode(value) else {
        return value.to_owned();
    };
    let index = match part(&parts, 0) {
        Some("runtime") => 3,
        Some("endpoint" | "mesh-external") => 4,
        _ => return value.to_owned(),
    };
    part(&parts, index).map_or_else(|| value.to_owned(), str::to_owned)
}
