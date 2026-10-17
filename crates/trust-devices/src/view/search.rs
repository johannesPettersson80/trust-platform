//! The protocol filter and the search (specification 25 §5.5 and §5.11). Both only change what
//! is drawn: no health, rollup, issue or count of the issues list changes with them.
//!
//! - The filter hides the connections of the protocols it names, the devices on their buses, the
//!   links that reach a hidden item, the mesh bus without a shown connection, and the systems that
//!   only hidden links or a hidden mesh bus reach. It reports what it hides: the connections,
//!   those that need attention, and their errors and warnings.
//! - The search, ignoring case, matches an item whose list cells other than its kind, whose
//!   issues, or, for a connection or a device, whose inspector rows (its settings and signals)
//!   contain it; a connection also matches by its protocol's identifier. What an item holds
//!   matches when the item matches: a runtime when its computer does, a connection when its
//!   runtime does, a device on a bus when its connection does. A link is lit when it matches or
//!   one of its ends matches; an item is lit when it matches or a lit link reaches it; a computer
//!   or a runtime is lit when something it holds is. Every item that is not lit is dimmed.

use std::collections::{BTreeMap, BTreeSet};

use super::devices::Graph;
use super::health;
use super::list;
use super::types::{
    Computer, Connection, Device, DeviceKind, FilterLine, FilterProtocol, FilterReport, Issues,
    ItemId, ListCells, SearchEcho, Section, Severity, Tone,
};
use crate::texts::connections::protocol_name;
use crate::texts::graph::SEARCH_PLACEHOLDER;
use crate::texts::issues as texts;
use crate::topology::Topology;

fn count_u32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// Applies the protocol filter (`apply_filter`): hides what it names and reports what is hidden
/// (`filter_report`).
pub(crate) fn apply_filter(
    topology: &Topology,
    computers: &mut [Computer],
    graph: &mut Graph,
    issues: &Issues,
    hidden_protocols: &[String],
) -> FilterReport {
    let hidden: BTreeSet<&str> = hidden_protocols
        .iter()
        .map(|protocol| protocol.trim())
        .filter(|protocol| !protocol.is_empty())
        .collect();
    let healths: BTreeMap<&str, &str> = topology
        .runtimes()
        .flat_map(|runtime| &runtime.endpoints)
        .map(|endpoint| (endpoint.id.as_str(), endpoint.health.as_str()))
        .collect();
    let mut protocols: BTreeMap<String, usize> = BTreeMap::new();
    let mut hidden_items: BTreeSet<ItemId> = BTreeSet::new();
    let mut connection_ids: BTreeSet<ItemId> = BTreeSet::new();
    let mut attention = 0;
    for connection in computers
        .iter_mut()
        .flat_map(|computer| computer.runtimes.iter_mut())
        .flat_map(|runtime| runtime.connections.iter_mut())
    {
        *protocols.entry(connection.protocol.clone()).or_default() += 1;
        connection_ids.insert(connection.id.clone());
        connection.hidden = hidden.contains(connection.protocol.as_str());
        if connection.hidden {
            hidden_items.insert(connection.id.clone());
            let health = healths
                .get(connection.id.as_str())
                .copied()
                .unwrap_or_default();
            if health::needs_attention(health) {
                attention += 1;
            }
        }
    }
    hide_devices_and_links(graph, &mut hidden_items, &connection_ids);
    let (errors, warnings) = issues
        .groups
        .iter()
        .filter(|group| {
            group
                .item
                .as_ref()
                .is_some_and(|item| hidden_items.contains(item))
        })
        .flat_map(|group| &group.issues)
        .fold((0, 0), |(errors, warnings), issue| match issue.severity {
            Severity::Error => (errors + 1, warnings),
            Severity::Warning => (errors, warnings + 1),
            Severity::Info => (errors, warnings),
        });
    let hidden_connections = connection_ids
        .iter()
        .filter(|id| hidden_items.contains(*id))
        .count();
    FilterReport {
        lines: filter_lines(
            &hidden,
            protocols.is_empty(),
            hidden_connections,
            attention,
            errors,
            warnings,
        ),
        protocols: protocols
            .into_iter()
            .map(|(id, count)| FilterProtocol {
                label: protocol_name(&id),
                count: count_u32(count),
                hidden: hidden.contains(id.as_str()),
                id,
            })
            .collect(),
        hidden_connections: count_u32(hidden_connections),
        hidden_attention: count_u32(attention),
        hidden_errors: count_u32(errors),
        hidden_warnings: count_u32(warnings),
    }
}

/// Hides the devices on the buses of hidden connections, the links that reach a hidden item, the
/// mesh bus without a shown connection, and the systems that only hidden links or a hidden mesh
/// bus reach; adds what it hides to `hidden_items`.
fn hide_devices_and_links(
    graph: &mut Graph,
    hidden_items: &mut BTreeSet<ItemId>,
    connection_ids: &BTreeSet<ItemId>,
) {
    for device in &mut graph.devices {
        if device
            .owner
            .as_ref()
            .is_some_and(|owner| hidden_items.contains(owner))
        {
            device.hidden = true;
            hidden_items.insert(device.id.clone());
        }
    }
    for link in &mut graph.links {
        link.hidden = hidden_items.contains(&link.from) || hidden_items.contains(&link.to);
    }
    let mut behind_hidden_bus: BTreeSet<ItemId> = BTreeSet::new();
    graph.mesh.retain(|bus| {
        let shown = bus.drops.iter().any(|member| {
            connection_ids.contains(&member.item) && !hidden_items.contains(&member.item)
        });
        if !shown {
            behind_hidden_bus.extend(bus.drops.iter().map(|member| member.item.clone()));
        }
        shown
    });
    for device in &mut graph.devices {
        if device.kind == DeviceKind::SubDevice {
            continue;
        }
        let reaching: Vec<bool> = graph
            .links
            .iter()
            .filter(|link| link.from == device.id || link.to == device.id)
            .map(|link| link.hidden)
            .collect();
        let reached = !reaching.is_empty() || behind_hidden_bus.contains(&device.id);
        if reached && reaching.iter().all(|hidden| *hidden) {
            device.hidden = true;
            hidden_items.insert(device.id.clone());
        }
    }
}

/// The filter panel's lines (specification 25 §5.5): what is hidden and whether any of it needs
/// attention, or that every protocol is shown.
fn filter_lines(
    hidden: &BTreeSet<&str>,
    no_connections: bool,
    connections: usize,
    attention: usize,
    errors: usize,
    warnings: usize,
) -> Vec<FilterLine> {
    let line = |text: String, tone: Tone| FilterLine { text, tone };
    let mut lines = Vec::new();
    if hidden.is_empty() {
        lines.push(line(texts::ALL_PROTOCOLS_VISIBLE.to_owned(), Tone::Neutral));
    } else {
        lines.push(line(texts::connections_hidden(connections), Tone::Neutral));
        let problems = errors + warnings;
        let needing = problems.max(attention);
        lines.push(if needing > 0 {
            line(texts::hidden_need_attention(needing), Tone::Warning)
        } else {
            line(texts::NO_HIDDEN_PROBLEMS.to_owned(), Tone::Muted)
        });
        if problems > 0 {
            let tone = if errors > 0 {
                Tone::Error
            } else {
                Tone::Warning
            };
            lines.push(line(
                texts::hidden_errors_and_warnings(errors, warnings),
                tone,
            ));
        }
    }
    if no_connections {
        lines.push(line(texts::NO_CONNECTIONS.to_owned(), Tone::Muted));
    }
    lines
}

/// The issue texts of each item.
fn issue_texts(issues: &Issues) -> BTreeMap<&ItemId, Vec<&str>> {
    let mut texts: BTreeMap<&ItemId, Vec<&str>> = BTreeMap::new();
    for group in &issues.groups {
        if let Some(item) = &group.item {
            texts
                .entry(item)
                .or_default()
                .extend(group.issues.iter().map(|issue| issue.text.as_str()));
        }
    }
    texts
}

/// What the search looks in.
struct Needle<'a> {
    text: String,
    issues: BTreeMap<&'a ItemId, Vec<&'a str>>,
}

impl Needle<'_> {
    fn found(&self, text: &str) -> bool {
        text.to_lowercase().contains(&self.text)
    }

    /// Whether the item's list cells other than its kind or its issues hold the search.
    fn in_item(&self, id: &ItemId, cells: &ListCells) -> bool {
        cells
            .cells
            .iter()
            .any(|(column, value)| column != list::KIND && self.found(value))
            || self
                .issues
                .get(id)
                .is_some_and(|texts| texts.iter().any(|text| self.found(text)))
    }

    /// Whether inspector rows hold the search.
    fn in_rows(&self, sections: &[Section]) -> bool {
        sections
            .iter()
            .flat_map(|section| &section.rows)
            .any(|row| self.found(&row.value))
    }

    fn in_connection(&self, connection: &Connection) -> bool {
        self.in_item(&connection.id, &connection.list)
            || self.found(&connection.protocol)
            || self.in_rows(&connection.inspector.sections)
    }

    fn in_device(&self, device: &Device) -> bool {
        self.in_item(&device.id, &device.list) || self.in_rows(&device.inspector.sections)
    }
}

/// The items that match, each with what holds it.
fn matched(needle: &Needle<'_>, computers: &[Computer], graph: &Graph) -> BTreeSet<ItemId> {
    let mut matched = BTreeSet::new();
    for computer in computers {
        let computer_matches = needle.in_item(&computer.id, &computer.list);
        if computer_matches {
            matched.insert(computer.id.clone());
        }
        for runtime in &computer.runtimes {
            let runtime_matches = computer_matches || needle.in_item(&runtime.id, &runtime.list);
            if runtime_matches {
                matched.insert(runtime.id.clone());
            }
            for connection in &runtime.connections {
                if runtime_matches || needle.in_connection(connection) {
                    matched.insert(connection.id.clone());
                }
            }
        }
    }
    for device in &graph.devices {
        let held = device
            .owner
            .as_ref()
            .is_some_and(|owner| matched.contains(owner));
        if held || needle.in_device(device) {
            matched.insert(device.id.clone());
        }
    }
    matched
}

/// Applies the search (`apply_search`): dims every item that it does not light, and counts the
/// lit items that the filter shows.
pub(crate) fn apply_search(
    computers: &mut [Computer],
    graph: &mut Graph,
    issues: &Issues,
    query: &str,
) -> SearchEcho {
    let mut echo = SearchEcho {
        query: query.to_owned(),
        placeholder: SEARCH_PLACEHOLDER.to_owned(),
        matches: 0,
    };
    let text = query.trim().to_lowercase();
    if text.is_empty() {
        return echo;
    }
    let needle = Needle {
        text,
        issues: issue_texts(issues),
    };
    let mut lit = matched(&needle, computers, graph);
    let lit_links: BTreeSet<ItemId> = graph
        .links
        .iter()
        .filter(|link| {
            lit.contains(&link.from)
                || lit.contains(&link.to)
                || needle.found(&link.protocol_label)
                || link
                    .hover_text
                    .as_deref()
                    .is_some_and(|text| needle.found(text))
        })
        .map(|link| link.id.clone())
        .collect();
    for link in graph
        .links
        .iter()
        .filter(|link| lit_links.contains(&link.id))
    {
        lit.insert(link.from.clone());
        lit.insert(link.to.clone());
    }
    let mut count = 0;
    for link in &mut graph.links {
        link.dimmed = !lit_links.contains(&link.id);
        count += usize::from(!link.dimmed && !link.hidden);
    }
    for device in &mut graph.devices {
        device.dimmed = !lit.contains(&device.id);
        count += usize::from(!device.dimmed && !device.hidden);
    }
    for computer in computers.iter_mut() {
        let mut computer_lit = lit.contains(&computer.id);
        for runtime in &mut computer.runtimes {
            let mut runtime_lit = lit.contains(&runtime.id);
            for connection in &mut runtime.connections {
                let connection_lit = lit.contains(&connection.id)
                    || connection
                        .subdevices
                        .iter()
                        .any(|device| lit.contains(device));
                connection.dimmed = !connection_lit;
                count += usize::from(connection_lit && !connection.hidden);
                runtime_lit |= connection_lit;
            }
            runtime.dimmed = !runtime_lit;
            count += usize::from(runtime_lit);
            computer_lit |= runtime_lit;
        }
        computer.dimmed = !computer_lit;
        count += usize::from(computer_lit);
    }
    echo.matches = count_u32(count);
    echo
}
