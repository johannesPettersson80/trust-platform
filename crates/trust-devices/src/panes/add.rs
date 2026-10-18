//! The add dialog (specification 25 §5.9 and §5.14): what `+ Add` offers for the runtime it adds
//! to. Its catalog comes from that runtime when it served one, and else from the tool's own.
//!
//! - The target: the selected runtime, else the simulator, else the first runtime shown. A
//!   runtime's menu and its `Add connection` slot open the dialog for that runtime by selecting
//!   it.
//! - `Runtimes` first: connecting a runtime that already runs, running another runtime on this
//!   computer, one entry per chip of truST's chip files, and the installation over SSH and the
//!   runtime in Docker, which come later and say so.
//! - The catalog's groups in their canonical order, without the empty ones. `discovery` is never
//!   an entry, and a protocol that no group holds is never dropped: it goes last into the advanced
//!   `Other choices`.
//! - The quick starts, while the target has no connections yet.

use std::collections::BTreeSet;

use serde_json::{json, Map, Value};

use super::forms::{Catalog, CatalogProtocol, FormRequest, RuntimeSetup};
use crate::gather::Gathered;
use crate::inputs::Facts;
use crate::request::ViewRequest;
use crate::texts::forms as texts;
use crate::texts::graph;
use crate::topology::{self, Topology};
use crate::view::chips;
use crate::view::lifecycle::Owner;
use crate::view::runtimes;
use crate::view::types::{
    Action, AddDialog, AddEntry, AddGroup, AdvancedToggle, Emphasis, ItemId, Op, QuickStart,
    RuntimeRef,
};

/// A runtime that the dialog can add to.
struct Candidate<'a> {
    /// Its identity and its name as its card shows it.
    reference: RuntimeRef,
    /// The runtime.
    runtime: &'a topology::Runtime,
    /// Whether it is the simulator.
    simulator: bool,
}

/// The add dialog of the view: for the runtime that `request` selects, else for the simulator,
/// else for the first runtime of `topology`.
pub(crate) fn add_dialog(
    topology: &Topology,
    facts: &Facts,
    gathered: &Gathered,
    request: &ViewRequest,
) -> AddDialog {
    let candidates = candidates(topology, facts);
    let target = add_target(&candidates, request.view.selected.as_deref());
    let catalog = Catalog::for_runtime(
        gathered,
        target.and_then(|target| target.runtime.control_endpoint.as_deref()),
    );
    let reference = target.map(|target| &target.reference);
    let mut dialog = crate::view::empty_add_dialog();
    dialog.target = reference.cloned();
    dialog.runtime_choices = candidates
        .iter()
        .map(|candidate| candidate.reference.clone())
        .collect();
    dialog.runtimes.entries = runtime_entries();
    let Some(catalog) = catalog else {
        dialog.catalog_state = Some(texts::CATALOG_UNAVAILABLE.to_owned());
        return dialog;
    };
    dialog.groups = groups(&catalog, reference);
    if dialog.groups.is_empty() {
        dialog.catalog_state = Some(texts::NO_TYPES.to_owned());
    }
    dialog.advanced_toggle = advanced_toggle(&dialog.groups);
    if let Some(target) = target.filter(|target| target.runtime.endpoints.is_empty()) {
        dialog.quick_starts = quick_starts(&catalog, &target.reference);
    }
    dialog
}

/// Every runtime of the topology, in the order of the computers' cards.
fn candidates<'a>(topology: &'a Topology, facts: &Facts) -> Vec<Candidate<'a>> {
    topology
        .hosts
        .iter()
        .flat_map(topology::Host::all_runtimes)
        .map(|runtime| {
            let owner = runtimes::owner(runtime, facts);
            Candidate {
                reference: RuntimeRef {
                    id: ItemId(runtime.id.clone()),
                    name: runtimes::display_name(runtime, owner),
                },
                runtime,
                simulator: owner == Owner::Simulator,
            }
        })
        .collect()
}

/// The runtime that the dialog adds to: the selected one, else the simulator, else the first.
fn add_target<'c, 'a>(
    candidates: &'c [Candidate<'a>],
    selected: Option<&str>,
) -> Option<&'c Candidate<'a>> {
    selected
        .and_then(|selected| {
            candidates
                .iter()
                .find(|candidate| candidate.reference.id.as_str() == selected)
        })
        .or_else(|| candidates.iter().find(|candidate| candidate.simulator))
        .or_else(|| candidates.first())
}

fn enabled(id: &str, label: &str, op: Op) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Secondary,
        confirm: None,
        op,
    }
}

/// The action that opens the form of `request`.
fn open_form(request: &FormRequest) -> Op {
    Op::Rust {
        action: "form.open".to_owned(),
        payload: request.payload(),
    }
}

fn entry(id: &str, title: &str, badge: &str, purpose: &str, action: Action) -> AddEntry {
    AddEntry {
        protocol: id.to_owned(),
        title: title.to_owned(),
        badge: badge.to_owned(),
        purpose: purpose.to_owned(),
        action,
    }
}

/// The entries of `Runtimes` (specification 25 §5.2 and §5.14).
fn runtime_entries() -> Vec<AddEntry> {
    let computer = |id: &str, title: &str, purpose: &str, setup: RuntimeSetup| {
        let op = open_form(&FormRequest {
            setup: Some(setup),
            ..FormRequest::default()
        });
        entry(
            id,
            title,
            graph::KIND_COMPUTER,
            purpose,
            enabled(id, title, op),
        )
    };
    let later = |id: &str, title: &str, purpose: &str, request: &str| {
        let op = Op::Rust {
            action: request.to_owned(),
            payload: json!({}),
        };
        let action = Action {
            enabled: false,
            reason: Some(texts::LATER_RELEASE.to_owned()),
            emphasis: Emphasis::Quiet,
            ..enabled(id, title, op)
        };
        entry(id, title, graph::KIND_COMPUTER, purpose, action)
    };
    let mut entries = vec![
        computer(
            "runtime.connect",
            texts::CONNECT_EXISTING_RUNTIME,
            texts::CONNECT_EXISTING_DETAIL,
            RuntimeSetup::Connect,
        ),
        computer(
            "runtime.local",
            texts::RUN_LOCAL_RUNTIME,
            texts::RUN_LOCAL_DETAIL,
            RuntimeSetup::Local,
        ),
    ];
    entries.extend(chips::all().iter().map(|chip| {
        let id = format!("chip.{}", chip.identity.name);
        let op = Op::Rust {
            action: "runtime.install_chip".to_owned(),
            payload: json!({ "chip": chip.identity.name }),
        };
        entry(
            &id,
            &chip.identity.model,
            graph::KIND_CHIP,
            &chip_purpose(&chip.interfaces),
            enabled(&id, &chip.identity.model, op),
        )
    }));
    entries.push(later(
        "runtime.ssh",
        texts::INSTALL_OVER_SSH,
        texts::INSTALL_OVER_SSH_DETAIL,
        "runtime.install_ssh",
    ));
    entries.push(later(
        "runtime.docker",
        texts::RUN_IN_DOCKER,
        texts::RUN_IN_DOCKER_DETAIL,
        "runtime.run_docker",
    ));
    entries
}

/// What a chip's entry offers: the truST firmware and the links it is reached over.
fn chip_purpose(interfaces: &trust_chips::Interfaces) -> String {
    let links: Vec<&str> = [
        (&interfaces.ethernet, texts::LINK_ETHERNET),
        (&interfaces.wifi, texts::LINK_WIFI),
        (&interfaces.bluetooth, texts::LINK_BLUETOOTH),
        (&interfaces.usb, texts::LINK_USB),
    ]
    .into_iter()
    .filter(|(controllers, _)| !controllers.is_empty())
    .map(|(_, link)| link)
    .collect();
    texts::chip_purpose(&links)
}

/// The catalog's groups in their canonical order, each without the protocols the catalog lacks
/// and left out when empty, then `Other choices` with every protocol that no group holds,
/// except `discovery` (specification 25 §5.9).
fn groups(catalog: &Catalog, target: Option<&RuntimeRef>) -> Vec<AddGroup> {
    let mut placed = BTreeSet::from(["discovery"]);
    let mut groups = Vec::new();
    for group in &catalog.groups {
        let entries: Vec<AddEntry> = group
            .protocols
            .iter()
            .filter_map(|id| catalog.protocol(id))
            .map(|protocol| {
                placed.insert(protocol.id.as_str());
                protocol_entry(protocol, target)
            })
            .collect();
        if !entries.is_empty() {
            groups.push(AddGroup {
                label: group.label.clone(),
                advanced: group.advanced,
                entries,
            });
        }
    }
    let other: Vec<AddEntry> = catalog
        .protocols
        .iter()
        .filter(|protocol| !placed.contains(protocol.id.as_str()))
        .map(|protocol| protocol_entry(protocol, target))
        .collect();
    if !other.is_empty() {
        groups.push(AddGroup {
            label: texts::OTHER_CHOICES.to_owned(),
            advanced: true,
            entries: other,
        });
    }
    groups
}

/// A protocol's entry, in the words of its picker entry; a protocol without one shows its form's
/// title and purpose and a badge made from its ID.
fn protocol_entry(protocol: &CatalogProtocol, target: Option<&RuntimeRef>) -> AddEntry {
    let (title, badge, purpose) = match &protocol.picker {
        Some(picker) => (
            picker.title.clone(),
            picker.badge.clone(),
            picker.purpose.clone(),
        ),
        None => (
            protocol.title.clone(),
            texts::fallback_badge(&protocol.id),
            protocol.purpose.clone(),
        ),
    };
    let op = open_form(&FormRequest {
        protocol: Some(protocol.id.clone()),
        runtime: target.map(|target| target.id.0.clone()),
        ..FormRequest::default()
    });
    AddEntry {
        protocol: protocol.id.clone(),
        action: enabled(&protocol.id, &title, op),
        title,
        badge,
        purpose,
    }
}

/// The toggle of the advanced groups, with the number of their entries; none without them.
fn advanced_toggle(groups: &[AddGroup]) -> Option<AdvancedToggle> {
    let count: usize = groups
        .iter()
        .filter(|group| group.advanced)
        .map(|group| group.entries.len())
        .sum();
    (count > 0).then(|| AdvancedToggle {
        show_label: texts::SHOW_ADVANCED.to_owned(),
        hide_label: texts::HIDE_ADVANCED.to_owned(),
        count: u32::try_from(count).unwrap_or(u32::MAX),
    })
}

/// The quick starts of a runtime without connections (specification 25 §5.14), each opening its
/// protocol's form with the values its description names, for the protocols the catalog holds.
fn quick_starts(catalog: &Catalog, target: &RuntimeRef) -> Vec<QuickStart> {
    let starts = [
        (
            "simulated",
            texts::QUICK_SIMULATED,
            texts::QUICK_SIMULATED_DETAIL,
            json!({ "mode": "counter" }),
        ),
        (
            "loopback",
            texts::QUICK_LOOPBACK,
            texts::QUICK_LOOPBACK_DETAIL,
            json!({}),
        ),
        (
            "modbus_tcp",
            texts::QUICK_MODBUS,
            texts::QUICK_MODBUS_DETAIL,
            json!({ "address": "192.168.1.50:502", "unit_id": 1 }),
        ),
        (
            "mqtt",
            texts::QUICK_MQTT,
            texts::QUICK_MQTT_DETAIL,
            json!({}),
        ),
    ];
    starts
        .into_iter()
        .filter(|(protocol, ..)| catalog.protocol(protocol).is_some())
        .map(|(protocol, title, description, prefill)| {
            let prefill = match prefill {
                Value::Object(values) if !values.is_empty() => Some(values),
                _ => None::<Map<String, Value>>,
            };
            let op = open_form(&FormRequest {
                protocol: Some(protocol.to_owned()),
                runtime: Some(target.id.0.clone()),
                prefill,
                ..FormRequest::default()
            });
            QuickStart {
                title: title.to_owned(),
                description: description.to_owned(),
                action: enabled(&format!("quick_start.{protocol}"), title, op),
            }
        })
        .collect()
}
