//! The computers and chips (specification 25 §5.5, §5.11 and §5.12): each is a frame that holds
//! all its runtimes side by side, headed `This computer`, `Computer <IP>`, the configured label
//! of a target that did not answer, or the chip's name and address, with its platform and its
//! reachability. The networks group the frames by subnet (decision O29).
//!
//! A computer's state is the machine's reachability and never a rollup of its runtimes or
//! connections. A container that holds one runtime is the chip on that runtime's card; only a
//! container of several runtimes is drawn as a frame.

use super::address;
use super::chips;
use super::health::{self, runtime_rollup};
use super::lifecycle;
use super::list::{self, ComputerCells};
use super::runtimes::{self, Place};
use super::types::{
    Action, Computer, ComputerKind, Container, Emphasis, HoverCard, Inspector, ItemId, Look,
    Network, Op, Pill, Row, Runtime, Section, UiOp,
};
use crate::inputs::Facts;
use crate::texts::graph as texts;
use crate::topology::{self, Host, HostOrigin, Topology};

/// Every computer of the topology, in its order: the project's own first.
pub(crate) fn computers(topology: &Topology, facts: &Facts, now_ms: u64) -> Vec<Computer> {
    topology
        .hosts
        .iter()
        .map(|host| computer(host, topology.this_computer.as_deref(), facts, now_ms))
        .collect()
}

/// The bands of the computers that share an IPv4 subnet, in the order of their first computer.
/// A computer without such an address belongs to no band (decision O29).
pub(crate) fn networks(topology: &Topology, computers: &[Computer]) -> Vec<Network> {
    let mut networks: Vec<Network> = Vec::new();
    for (host, computer) in topology.hosts.iter().zip(computers) {
        let Some(subnet) = address::subnet(host) else {
            continue;
        };
        let id = format!("network:{subnet}");
        match networks.iter().position(|network| network.id == id) {
            Some(index) => networks[index].computers.push(computer.id.clone()),
            None => networks.push(Network {
                id,
                label: texts::network(&subnet),
                computers: vec![computer.id.clone()],
            }),
        }
    }
    networks
}

/// The identity of a computer's frame: its host ID, which is global, in the scheme of the
/// owner-scoped identities so that it never equals another item's.
pub(crate) fn computer_id(host: &Host) -> ItemId {
    ItemId(topology::encode(&[Some("host"), Some(host.id.as_str())]))
}

/// The chip a host is: the chip that one of its runtimes reports as its platform.
fn chip_model(host: &Host, facts: &Facts) -> Option<String> {
    host.all_runtimes()
        .filter_map(|runtime| runtime.control_endpoint.as_deref())
        .filter_map(|endpoint| facts.statuses.get(endpoint.trim()))
        .filter_map(|status| status.platform.as_deref())
        .find_map(chips::model)
}

fn computer(host: &Host, this_computer: Option<&str>, facts: &Facts, now_ms: u64) -> Computer {
    let id = computer_id(host);
    let chip = chip_model(host, facts);
    let heading = heading(host, this_computer, chip.as_deref());
    let platform = platform(host, chip.as_deref());
    let running = host.all_runtimes().any(|runtime| {
        runtime_rollup(
            &runtime.health,
            runtime
                .endpoints
                .iter()
                .map(|endpoint| endpoint.health.as_str()),
        )
        .running()
    });
    let reachability = health::reachability_pill(health::host_reachable(host, running));
    let (cards, containers) = cards(host, &heading, facts);
    let counts = (
        cards.len(),
        host.all_runtimes()
            .map(|runtime| runtime.endpoints.len())
            .sum::<usize>(),
    );
    let address = addresses(host);
    let kind_word = if chip.is_some() {
        texts::KIND_CHIP
    } else {
        texts::KIND_COMPUTER
    };
    let subnet = address::subnet(host).map(|subnet| texts::network(&subnet));
    let last_seen = host
        .last_seen_ms
        .map(|seen| texts::ago(now_ms.saturating_sub(seen) / 1_000));
    Computer {
        list: list::computer_cells(&ComputerCells {
            chip: chip.is_some(),
            name: &heading,
            state: &reachability.word,
            address: address.as_deref(),
            platform: platform.as_deref(),
            network: subnet.as_deref(),
            last_seen: last_seen.as_deref(),
            host_name: reported(host)
                .then(|| host.hostname.trim())
                .filter(|name| !name.is_empty()),
        }),
        hover: HoverCard {
            title: heading.clone(),
            rows: hover_rows(host, &reachability.word, counts),
        },
        inspector: Inspector {
            breadcrumb: texts::breadcrumb(kind_word),
            kind: kind_word.to_owned(),
            title: heading.clone(),
            health: Some(reachability.tone),
            primary: Vec::new(),
            secondary: vec![focus(&id)],
            sections: vec![Section {
                heading: None,
                rows: inspector_rows(host, address.as_deref(), &reachability, counts),
            }],
            edit: None,
        },
        slots: lifecycle::computer_slots(&id),
        id,
        kind: if chip.is_some() {
            ComputerKind::Chip
        } else {
            ComputerKind::Computer
        },
        heading,
        platform,
        reachability,
        look: Look::Normal,
        containers,
        runtimes: cards,
        dimmed: false,
        hidden: false,
    }
}

/// The cards of every runtime on the host, side by side: those outside containers, then those in
/// each container, and the containers.
fn cards(host: &Host, heading: &str, facts: &Facts) -> (Vec<Runtime>, Vec<Container>) {
    let bare = Place {
        computer: heading,
        container: None,
        container_framed: false,
    };
    let mut cards: Vec<Runtime> = host
        .runtimes
        .iter()
        .map(|runtime| runtimes::runtime(runtime, bare, facts))
        .collect();
    let mut containers = Vec::with_capacity(host.containers.len());
    for container in &host.containers {
        let framed = container.runtimes.len() > 1;
        let place = Place {
            computer: heading,
            container: Some(container.name.as_str()),
            container_framed: framed,
        };
        cards.extend(
            container
                .runtimes
                .iter()
                .map(|runtime| runtimes::runtime(runtime, place, facts)),
        );
        containers.push(container_view(container, framed));
    }
    (cards, containers)
}

/// Whether a report gives the host, so that it has a host name of its own.
fn reported(host: &Host) -> bool {
    host.origin == HostOrigin::Reported
}

/// The heading of a computer (specification 25 §5.5 and §5.12).
///
/// - A computer drawn for a configured target that did not answer keeps its configured label;
///   it is never `This computer`.
/// - This computer is `This computer`: the host drawn for it, or the reported host that stands
///   for it.
/// - A chip is headed by the chip's name and address.
/// - Another computer is `Computer <IP>` by its first address that is not a loopback address;
///   without one, `Computer <host name>`. A reported host without any machine facts keeps its
///   name, and one known only by loopback names is this computer.
pub(crate) fn heading(host: &Host, this_computer: Option<&str>, chip: Option<&str>) -> String {
    let hostname = host.hostname.trim();
    match host.origin {
        HostOrigin::Unanswered => return hostname.to_owned(),
        HostOrigin::ThisComputer => return texts::THIS_COMPUTER.to_owned(),
        HostOrigin::Reported => {}
    }
    if let Some(model) = chip {
        return texts::chip_heading(model, address::remote_address(host).as_deref());
    }
    if this_computer == Some(host.id.as_str()) {
        return texts::THIS_COMPUTER.to_owned();
    }
    let machine_facts =
        !host.arch.trim().is_empty() || !host.os.trim().is_empty() || !host.ips.is_empty();
    if !machine_facts && !hostname.is_empty() {
        return hostname.to_owned();
    }
    if host.ips.iter().any(|ip| !address::is_loopback(ip))
        && let Some(remote) = address::remote_address(host)
    {
        return texts::computer_at(&remote);
    }
    if !hostname.is_empty() && !address::is_loopback(hostname) {
        return texts::computer_at(hostname);
    }
    texts::THIS_COMPUTER.to_owned()
}

/// The platform line: the chip, or the board, operating system and architecture.
fn platform(host: &Host, chip: Option<&str>) -> Option<String> {
    if let Some(model) = chip {
        return Some(texts::chip_platform(model));
    }
    let line = texts::joined(&[host.board.as_deref().unwrap_or_default(), &system(host)]);
    (!line.is_empty()).then_some(line)
}

/// The operating system and architecture, such as `Linux aarch64`.
fn system(host: &Host) -> String {
    format!("{} {}", texts::operating_system(&host.os), host.arch.trim())
        .trim()
        .to_owned()
}

/// The addresses a computer reports, else the address of one of its runtimes.
fn addresses(host: &Host) -> Option<String> {
    let reported: Vec<&str> = host
        .ips
        .iter()
        .map(|ip| ip.trim())
        .filter(|ip| !ip.is_empty())
        .collect();
    if reported.is_empty() {
        address::remote_address(host)
    } else {
        Some(reported.join(", "))
    }
}

fn row(label: &str, value: impl Into<String>) -> Row {
    Row {
        label: label.to_owned(),
        value: value.into(),
        tone: None,
        wide: false,
        copy: None,
    }
}

/// The facts on hover; `counts` are the runtimes and the connections.
fn hover_rows(host: &Host, state: &str, (runtimes, connections): (usize, usize)) -> Vec<Row> {
    let mut rows = Vec::new();
    if reported(host) && !host.hostname.trim().is_empty() {
        rows.push(row(texts::ROW_HOST_NAME, host.hostname.trim()));
    }
    let system = system(host);
    if !system.is_empty() {
        rows.push(row(texts::ROW_SYSTEM, system));
    }
    rows.push(row(texts::ROW_STATE, state));
    rows.push(row(texts::ROW_RUNTIMES, runtimes.to_string()));
    rows.push(row(texts::ROW_CONNECTIONS, connections.to_string()));
    rows
}

/// The computer's facts, each when it is reported (specification 25 §5.12); `counts` are the
/// runtimes and the connections.
fn inspector_rows(
    host: &Host,
    address: Option<&str>,
    reachability: &Pill,
    (runtimes, connections): (usize, usize),
) -> Vec<Row> {
    let mut rows = Vec::new();
    if let Some(address) = address {
        rows.push(row(texts::ROW_ADDRESS, address));
    }
    rows.push(Row {
        tone: Some(reachability.tone),
        ..row(texts::ROW_STATE, reachability.word.as_str())
    });
    rows.push(row(texts::ROW_RUNTIMES, runtimes.to_string()));
    rows.push(row(texts::ROW_CONNECTIONS, connections.to_string()));
    if reported(host) && !host.hostname.trim().is_empty() {
        rows.push(row(texts::ROW_HOST_NAME, host.hostname.trim()));
    }
    if let Some(board) = host
        .board
        .as_deref()
        .filter(|board| !board.trim().is_empty())
    {
        rows.push(row(texts::ROW_BOARD, board.trim()));
    }
    let system = system(host);
    if !system.is_empty() {
        rows.push(row(texts::ROW_SYSTEM, system));
    }
    if let Some(celsius) = host.temp_c.filter(|celsius| celsius.is_finite()) {
        rows.push(row(texts::ROW_TEMPERATURE, texts::temperature(celsius)));
    }
    if let Some(uptime) = host.uptime_s {
        rows.push(row(texts::ROW_UP_FOR, texts::duration(uptime)));
    }
    if let Some(load) = host.load.filter(|load| load.is_finite()) {
        rows.push(row(texts::ROW_LOAD, texts::load(load)));
    }
    rows
}

fn focus(id: &ItemId) -> Action {
    Action {
        id: "focus".to_owned(),
        label: texts::FOCUS.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Secondary,
        confirm: None,
        op: Op::Ui {
            ui: UiOp::Focus { item: id.clone() },
        },
    }
}

/// A container: a frame only with more than one runtime.
fn container_view(container: &topology::Container, framed: bool) -> Container {
    let id = ItemId(container.id.clone());
    let status = Some(container.status.trim().to_owned()).filter(|status| !status.is_empty());
    let mut rows = Vec::new();
    if !container.image.trim().is_empty() {
        rows.push(row(texts::ROW_IMAGE, container.image.trim()));
    }
    if let Some(status) = &status {
        rows.push(row(texts::ROW_STATE, status.clone()));
    }
    let hover = HoverCard {
        title: container.name.clone(),
        rows: rows.clone(),
    };
    let optional = [
        (texts::ROW_RESTART_POLICY, container.restart_policy.clone()),
        (texts::ROW_CPU_LIMIT, container.cpu_limit.clone()),
        (texts::ROW_MEMORY_LIMIT, container.mem_limit.clone()),
        (
            texts::ROW_MOUNTS,
            container.mounts.as_ref().map(|mounts| mounts.join(", ")),
        ),
    ];
    for (label, value) in optional {
        if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
            rows.push(row(label, value));
        }
    }
    rows.push(row(
        texts::ROW_RUNTIMES,
        container.runtimes.len().to_string(),
    ));
    Container {
        inspector: Inspector {
            breadcrumb: texts::breadcrumb(texts::KIND_CONTAINER),
            kind: texts::KIND_CONTAINER.to_owned(),
            title: container.name.clone(),
            health: None,
            primary: Vec::new(),
            secondary: vec![focus(&id)],
            sections: vec![Section {
                heading: None,
                rows,
            }],
            edit: None,
        },
        id,
        name: container.name.clone(),
        status,
        framed,
        runtimes: container
            .runtimes
            .iter()
            .map(|runtime| ItemId(runtime.id.clone()))
            .collect(),
        hover,
    }
}
