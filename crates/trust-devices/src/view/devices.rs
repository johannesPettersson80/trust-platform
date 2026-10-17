//! The devices and systems at the other end of the connections, the links and the mesh bus
//! (specification 25 §5.7 and §5.13).
//!
//! - A system card shows its name in product words, what it presents (the mirror of truST's role
//!   on the link that reaches it, such as `MQTT broker` or `OPC UA server`), the status of that
//!   link, and the last time seen and the response time that the connection at the link's other
//!   end reports. A device on a fieldbus is a card of its bus, with its own health.
//! - A link is solid only while its status proves it, its arrow follows the direction that the
//!   runtime reports, and a secure link shows a lock.
//! - The mesh connections drop onto one bus. It is a draft unless every peer is proven, and only
//!   a bus of several peers has a label.

use std::collections::{BTreeMap, BTreeSet};

use trust_runtime::fleet_topology::{FleetEndpointChild, FleetLink};

use super::connections::{self, row};
use super::health;
use super::list;
use super::types::{
    Action, ArrowAt, Computer, Connection, Device, DeviceKind, Emphasis, HoverCard, Inspector,
    ItemId, Link, MeshBus, MeshDrop, Op, Pill, Row, Section, Tone, UiOp,
};
use crate::texts::connections::{protocol_name, ROW_LAST_SEEN, ROW_RESPONSE_TIME};
use crate::texts::devices as texts;
use crate::texts::graph;
use crate::topology::{self, Endpoint, Topology};

/// The identity of the mesh bus.
const MESH_BUS: &str = "bus:mesh";

/// The devices, links and mesh buses of one view.
#[derive(Debug, Clone, Default)]
pub(crate) struct Graph {
    /// The devices and systems.
    pub(crate) devices: Vec<Device>,
    /// The links between connections, runtimes and devices.
    pub(crate) links: Vec<Link>,
    /// The mesh buses.
    pub(crate) mesh: Vec<MeshBus>,
}

/// The identity of a device on the bus of the endpoint `endpoint`.
pub(crate) fn subdevice_id(endpoint: &str, child: &str) -> ItemId {
    ItemId(topology::encode(&[
        Some("subdevice"),
        Some(endpoint),
        Some(child),
    ]))
}

/// The devices, links and mesh buses of the topology, drawn beside `computers`.
pub(crate) fn graph(topology: &Topology, computers: &[Computer], now_ms: u64) -> Graph {
    let endpoints: BTreeMap<&str, &Endpoint> = topology
        .runtimes()
        .flat_map(|runtime| runtime.endpoints.iter())
        .map(|endpoint| (endpoint.id.as_str(), endpoint))
        .collect();
    let connections: Vec<&Connection> = computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .flat_map(|runtime| &runtime.connections)
        .collect();
    let names: BTreeMap<&str, &str> = computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .map(|runtime| (runtime.id.as_str(), runtime.name.as_str()))
        .chain(
            connections
                .iter()
                .map(|connection| (connection.id.as_str(), connection.name.as_str())),
        )
        .collect();
    let systems = Systems {
        links: &topology.links,
        endpoints: &endpoints,
        names: &names,
        now_ms,
    };
    let mut devices = systems.cards(topology);
    for connection in &connections {
        if let Some(endpoint) = endpoints.get(connection.id.as_str()) {
            devices.extend(
                endpoint
                    .children
                    .iter()
                    .map(|child| subdevice(child, endpoint, connection)),
            );
        }
    }
    let known: BTreeSet<&str> = names
        .keys()
        .copied()
        .chain(devices.iter().map(|device| device.id.as_str()))
        .collect();
    let links = topology
        .links
        .iter()
        .filter(|link| {
            link.protocol != "mesh"
                && known.contains(link.from.as_str())
                && known.contains(link.to.as_str())
        })
        .map(link)
        .collect();
    let mesh = mesh_bus(&connections, &endpoints, &topology.links, &devices)
        .into_iter()
        .collect();
    Graph {
        devices,
        links,
        mesh,
    }
}

/// The first link that reaches a node.
fn first_link<'a>(links: &'a [FleetLink], id: &str) -> Option<&'a FleetLink> {
    links.iter().find(|link| link.to == id || link.from == id)
}

/// The node at the other end of a link from `id`.
fn far_end<'a>(link: &'a FleetLink, id: &str) -> &'a str {
    if link.to == id {
        &link.from
    } else {
        &link.to
    }
}

/// The role of the system at the other end of a link: the mirror of truST's own role on it. An
/// MQTT link always reaches a broker.
fn counterpart_role(link: &FleetLink) -> &str {
    if link.protocol == "mqtt" {
        return texts::COUNTERPART_BROKER;
    }
    match link.role.as_deref().unwrap_or_default() {
        "server" => texts::COUNTERPART_CLIENT,
        "client" => texts::COUNTERPART_SERVER,
        "master" => texts::COUNTERPART_SLAVES,
        "advertise" | "" => texts::COUNTERPART_PEER,
        role => role,
    }
}

/// What the system `id` presents (`presents`): a client link names the remote server
/// (`ADS server`, `OPC UA server`), a server link the remote client (`OPC UA client`,
/// `ADS client`), and every other link its protocol's product name and the counterpart's role.
pub(crate) fn presents(id: &str, links: &[FleetLink]) -> String {
    let Some(link) = first_link(links, id) else {
        return texts::PRESENTS_EXTERNAL_SYSTEM.to_owned();
    };
    let role = counterpart_role(link);
    let named = match (link.protocol.as_str(), role) {
        ("opcua_client", texts::COUNTERPART_SERVER) => texts::OPCUA_SERVER,
        ("opcua", texts::COUNTERPART_CLIENT) => texts::OPCUA_CLIENT,
        ("ads", texts::COUNTERPART_SERVER) => texts::ADS_SERVER,
        ("ads_server", texts::COUNTERPART_CLIENT) => texts::ADS_CLIENT,
        _ => return texts::presents(&protocol_name(&link.protocol), role),
    };
    named.to_owned()
}

/// The name a system is shown with (`device_label`, specification 25 §5.7): a name that is only
/// its protocol's identifier, or what a client or server link presents, reads as the product
/// name, and a name that starts with one, such as `modbus_tcp 127.0.0.1:502`, starts with the
/// product name instead.
pub(crate) fn device_label(id: &str, name: &str, links: &[FleetLink]) -> String {
    let name = name.trim();
    let Some(link) = first_link(links, id) else {
        return if name.is_empty() {
            texts::EXTERNAL_SYSTEM.to_owned()
        } else {
            name.to_owned()
        };
    };
    let protocol = link.protocol.as_str();
    let directional = matches!(protocol, "ads" | "ads_server" | "opcua" | "opcua_client");
    let rendered = if directional {
        presents(id, links)
    } else {
        protocol_name(protocol)
    };
    let lower = name.to_ascii_lowercase();
    let mut prefixes = Vec::new();
    if directional {
        prefixes.push(rendered.to_ascii_lowercase());
    }
    prefixes.push(protocol.to_ascii_lowercase());
    prefixes.push(protocol.replace('_', " ").to_ascii_lowercase());
    for prefix in prefixes {
        if lower == prefix {
            return rendered;
        }
        if lower.starts_with(&format!("{prefix} ")) {
            return format!("{rendered}{}", name.get(prefix.len()..).unwrap_or_default());
        }
    }
    name.to_owned()
}

/// Where a link's arrow points (`link_arrow`): at the counterpart for `outbound` traffic, at the
/// runtime's end for `inbound` traffic, and nowhere for another direction, such as
/// `publish_subscribe`, `observed` or `policy`.
pub(crate) fn link_arrow(direction: &str) -> ArrowAt {
    match direction.trim().to_ascii_lowercase().as_str() {
        "outbound" => ArrowAt::To,
        "inbound" => ArrowAt::From,
        _ => ArrowAt::None,
    }
}

/// The status pill of a link: its product word and tone, pulsing only while it is connected.
fn status_pill(status: &str) -> Pill {
    Pill {
        word: graph::health_word(status),
        tone: health::tone_of(status),
        live: status.trim().eq_ignore_ascii_case("connected"),
    }
}

/// A point-to-point link (`link`): solid only while its status proves it, in its status's tone,
/// with its protocol, its arrow, its lock, and its status and detail on hover.
pub(crate) fn link(link: &FleetLink) -> Link {
    let status = status_pill(&link.status);
    Link {
        id: ItemId(
            link.id
                .clone()
                .unwrap_or_else(|| format!("link:{}:{}:{}", link.protocol, link.from, link.to)),
        ),
        from: ItemId(link.from.clone()),
        to: ItemId(link.to.clone()),
        protocol_label: protocol_name(&link.protocol),
        proven: health::link_proven(&link.status),
        tone: status.tone,
        arrow: link_arrow(&link.direction),
        secure: link.secure,
        hover_text: Some(texts::status_and_detail(
            &status.word,
            link.detail.as_deref().unwrap_or_default(),
        )),
        status: Some(status),
        dimmed: false,
        hidden: false,
    }
}

/// What the card of a shared or external system is drawn from.
struct System<'a> {
    /// Its identity.
    id: &'a str,
    /// Its name as reported.
    name: &'a str,
    /// A shared or an external system.
    kind: DeviceKind,
    /// Its address, for a shared system.
    address: &'a str,
    /// The names of the runtimes that use it, for a shared system.
    used_by: String,
    /// The product names of the protocols that reach it, for an external system.
    protocols: String,
}

/// What the connection at a system's far end reports: when it was last seen and its response
/// time.
struct Seen {
    last_seen: Option<String>,
    response_time: Option<String>,
}

/// What the cards of the shared and external systems read.
struct Systems<'a> {
    links: &'a [FleetLink],
    endpoints: &'a BTreeMap<&'a str, &'a Endpoint>,
    names: &'a BTreeMap<&'a str, &'a str>,
    now_ms: u64,
}

impl Systems<'_> {
    /// The cards of the shared systems, then of the external systems.
    fn cards(&self, topology: &Topology) -> Vec<Device> {
        let shared = topology.shared.iter().map(|shared| {
            let used_by: Vec<&str> = shared
                .used_by
                .iter()
                .filter_map(|runtime| self.names.get(runtime.as_str()).copied())
                .collect();
            self.card(&System {
                id: &shared.id,
                name: Some(shared.name.trim())
                    .filter(|name| !name.is_empty())
                    .unwrap_or(shared.address.trim()),
                kind: DeviceKind::Shared,
                address: shared.address.trim(),
                used_by: used_by.join(", "),
                protocols: String::new(),
            })
        });
        let external = topology.external.iter().map(|external| {
            let protocols: Vec<String> = external
                .via_protocol
                .iter()
                .map(|protocol| protocol_name(protocol.as_str()))
                .collect();
            self.card(&System {
                id: &external.id,
                name: external.name.trim(),
                kind: DeviceKind::External,
                address: "",
                used_by: String::new(),
                protocols: protocols.join(", "),
            })
        });
        shared.chain(external).collect()
    }

    /// The card of a shared or external system.
    fn card(&self, system: &System<'_>) -> Device {
        let id = ItemId(system.id.to_owned());
        let name = device_label(system.id, system.name, self.links);
        let presents = presents(system.id, self.links);
        let link = first_link(self.links, system.id);
        let status = link.map(|link| status_pill(&link.status));
        let far = link.map(|link| far_end(link, system.id));
        let live = far
            .and_then(|far| self.endpoints.get(far))
            .and_then(|endpoint| endpoint.live.as_ref());
        let seen = Seen {
            last_seen: connections::last_seen(live, self.now_ms),
            response_time: connections::response_time(live),
        };
        let belongs_to = Some(system.used_by.as_str())
            .filter(|names| !names.is_empty())
            .or_else(|| far.and_then(|far| self.names.get(far).copied()));
        let kind = if system.kind == DeviceKind::Shared {
            texts::SHARED_SYSTEM
        } else {
            texts::EXTERNAL_SYSTEM
        };
        let state = status.as_ref().map(|status| status.word.as_str());
        Device {
            list: list::cells(
                "device",
                [
                    (list::KIND, Some(graph::KIND_DEVICE)),
                    (list::NAME, Some(name.as_str())),
                    (list::STATE, state),
                    (list::ADDRESS, Some(system.address)),
                    (list::TYPE, Some(presents.as_str())),
                    (list::BELONGS_TO, belongs_to),
                    (list::DETAIL, link.and_then(|link| link.detail.as_deref())),
                    (list::LAST_SEEN, seen.last_seen.as_deref()),
                    (list::RESPONSE_TIME, seen.response_time.as_deref()),
                ],
            ),
            hover: HoverCard {
                title: name.clone(),
                rows: system_hover(&presents, status.as_ref(), &seen),
            },
            inspector: Inspector {
                breadcrumb: graph::breadcrumb(kind),
                kind: kind.to_owned(),
                title: name.clone(),
                health: status.clone(),
                primary: Vec::new(),
                secondary: vec![focus(&id)],
                sections: vec![Section {
                    heading: None,
                    rows: system_rows(system, &presents, status.as_ref(), &seen),
                }],
                edit: None,
            },
            kind: system.kind,
            name,
            presents,
            status,
            last_seen: seen.last_seen,
            response_time: seen.response_time,
            owner: None,
            dimmed: false,
            hidden: false,
            id,
        }
    }
}

/// The rows of a status, the last time seen and the response time.
fn seen_rows(status: Option<&Pill>, seen: &Seen) -> Vec<Row> {
    let mut rows: Vec<Row> = status
        .map(|status| Row {
            tone: Some(status.tone),
            ..row(graph::ROW_STATE, status.word.as_str())
        })
        .into_iter()
        .collect();
    rows.extend(
        seen.last_seen
            .as_deref()
            .map(|seen| row(ROW_LAST_SEEN, seen)),
    );
    rows.extend(
        seen.response_time
            .as_deref()
            .map(|time| row(ROW_RESPONSE_TIME, time)),
    );
    rows
}

/// A system's facts on hover.
fn system_hover(presents: &str, status: Option<&Pill>, seen: &Seen) -> Vec<Row> {
    let mut rows = vec![row(texts::ROW_PRESENTS, presents)];
    rows.extend(seen_rows(status, seen));
    rows.push(row(texts::ROW_SCOPE, texts::SCOPE_EXTERNAL));
    rows
}

/// A system's facts in its inspector: what it presents, its address, the protocols that reach
/// it and the runtimes that use it, its status, and its scope.
fn system_rows(
    system: &System<'_>,
    presents: &str,
    status: Option<&Pill>,
    seen: &Seen,
) -> Vec<Row> {
    let mut rows = vec![row(texts::ROW_PRESENTS, presents)];
    for (label, value) in [
        (graph::ROW_ADDRESS, system.address),
        (texts::ROW_PROTOCOLS, system.protocols.as_str()),
        (texts::ROW_USED_BY, system.used_by.as_str()),
    ] {
        if !value.is_empty() {
            rows.push(row(label, value));
        }
    }
    rows.extend(seen_rows(status, seen));
    rows.push(row(texts::ROW_SCOPE, texts::SCOPE_CONFIGURED));
    rows
}

/// A device on the bus of `connection` (specification 25 §5.13): its own name, model, slot,
/// channels and health.
fn subdevice(child: &FleetEndpointChild, endpoint: &Endpoint, connection: &Connection) -> Device {
    let id = subdevice_id(&endpoint.id, &child.id);
    let model = child
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty());
    let name = Some(child.name.trim())
        .filter(|name| !name.is_empty())
        .or(model)
        .unwrap_or(child.id.trim())
        .to_owned();
    let presents = model.map_or_else(
        || texts::bus_device(&connection.protocol_name),
        str::to_owned,
    );
    let health = child.health.trim();
    let status = Pill {
        word: graph::health_word(health),
        tone: health::tone_of(health),
        live: health.eq_ignore_ascii_case("connected"),
    };
    let detail = child.detail.trim();
    let facts = [
        (texts::ROW_MODEL, model.map(str::to_owned)),
        (texts::ROW_SLOT, child.slot.map(|slot| slot.to_string())),
        (
            texts::ROW_CHANNELS,
            child.channels.map(|channels| channels.to_string()),
        ),
    ];
    let mut rows: Vec<Row> = facts
        .into_iter()
        .filter_map(|(label, value)| value.map(|value| row(label, value)))
        .collect();
    rows.push(Row {
        tone: Some(status.tone),
        ..row(graph::ROW_STATE, status.word.as_str())
    });
    if !detail.is_empty() {
        rows.push(row(graph::ROW_DETAIL, detail));
    }
    let hover = rows.clone();
    rows.push(row(texts::ROW_BUS, connection.name.as_str()));
    Device {
        list: list::cells(
            "device",
            [
                (list::KIND, Some(graph::KIND_DEVICE)),
                (list::NAME, Some(name.as_str())),
                (list::STATE, Some(status.word.as_str())),
                (list::TYPE, Some(presents.as_str())),
                (list::BELONGS_TO, Some(connection.name.as_str())),
                (list::DETAIL, Some(detail)),
                (list::IDENTITY, model),
            ],
        ),
        hover: HoverCard {
            title: name.clone(),
            rows: hover,
        },
        inspector: Inspector {
            breadcrumb: graph::breadcrumb(graph::KIND_DEVICE),
            kind: graph::KIND_DEVICE.to_owned(),
            title: name.clone(),
            health: Some(status.clone()),
            primary: Vec::new(),
            secondary: vec![focus(&id)],
            sections: vec![Section {
                heading: None,
                rows,
            }],
            edit: None,
        },
        kind: DeviceKind::SubDevice,
        name,
        presents,
        status: Some(status),
        last_seen: None,
        response_time: None,
        owner: Some(connection.id.clone()),
        dimmed: false,
        hidden: false,
        id,
    }
}

fn focus(id: &ItemId) -> Action {
    Action {
        id: "focus".to_owned(),
        label: graph::FOCUS.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Secondary,
        confirm: None,
        op: Op::Ui {
            ui: UiOp::Focus { item: id.clone() },
        },
    }
}

/// A drop's tone: muted while the bus is a draft, else the tone of a degraded or failed status.
fn drop_tone(draft: bool, status: &str) -> Tone {
    if draft {
        return Tone::Muted;
    }
    match status.trim().to_ascii_lowercase().as_str() {
        "degraded" => Tone::Warning,
        "error" => Tone::Error,
        _ => Tone::Neutral,
    }
}

/// The mesh bus (`mesh_bus`, specification 25 §5.7): every mesh connection drops onto it, and so
/// does every mesh peer that a mesh link reaches. It is a draft unless every peer is proven (the
/// peers' links, or the connections while no link reaches a peer), and only a bus of several
/// peers has its label.
fn mesh_bus(
    connections: &[&Connection],
    endpoints: &BTreeMap<&str, &Endpoint>,
    links: &[FleetLink],
    devices: &[Device],
) -> Option<MeshBus> {
    let members: Vec<(&Connection, &Endpoint)> = connections
        .iter()
        .filter(|connection| connection.protocol == "mesh")
        .filter_map(|connection| Some((*connection, *endpoints.get(connection.id.as_str())?)))
        .collect();
    if members.is_empty() {
        return None;
    }
    let peers: Vec<(&ItemId, &FleetLink)> = links
        .iter()
        .filter(|link| link.protocol == "mesh")
        .filter_map(|link| {
            devices
                .iter()
                .find(|device| device.id.as_str() == link.to || device.id.as_str() == link.from)
                .map(|device| (&device.id, link))
        })
        .collect();
    let draft = if peers.is_empty() {
        members
            .iter()
            .any(|(_, endpoint)| !health::link_proven(&endpoint.health))
    } else {
        peers
            .iter()
            .any(|(_, link)| !health::link_proven(&link.status))
    };
    let count = if peers.is_empty() {
        members.len()
    } else {
        peers.len()
    };
    let mut drops: Vec<MeshDrop> = members
        .iter()
        .map(|(connection, endpoint)| MeshDrop {
            item: connection.id.clone(),
            proven: !draft,
            tone: drop_tone(
                draft,
                if peers.is_empty() {
                    endpoint.health.as_str()
                } else {
                    ""
                },
            ),
            hover_text: peers.is_empty().then(|| {
                texts::status_and_detail(&graph::health_word(&endpoint.health), &endpoint.detail)
            }),
        })
        .collect();
    drops.extend(peers.iter().map(|(device, link)| MeshDrop {
        item: (*device).clone(),
        proven: !draft && health::link_proven(&link.status),
        tone: drop_tone(draft, &link.status),
        hover_text: Some(texts::status_and_detail(
            &graph::health_word(&link.status),
            link.detail.as_deref().unwrap_or_default(),
        )),
    }));
    Some(MeshBus {
        id: ItemId(MESH_BUS.to_owned()),
        label: (count > 1).then(|| texts::MESH_FABRIC.to_owned()),
        draft,
        drops,
    })
}
