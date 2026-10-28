//! The project's view of the Discover pane (specification 25 §2, the project mark and `Compare with
//! project`; specification 11, "Compare with the project"): what the comparison of the search with the
//! project says of each found device, the mark and rows that a card carries for it, the four lists of
//! `Compare with project`, and that action.
//!
//! The panel has no project of its own to read. It compares the finds of the status with the devices
//! that the connections on the canvas name, those of the runtimes it shows, by the function that
//! `comm.compare` runs (`trust_runtime::search::compare`). One comparison serves the marks and the lists.

use std::collections::BTreeMap;

use serde_json::{json, Map, Value};
use trust_runtime::search::compare::{
    compare, read_status, Comparison, DeviceStanding, Difference, Standing,
};
use trust_runtime::search::configured::{ConfiguredDevice, Facts};
use trust_runtime::search::SourceId;

use super::discover::{device_title, text, DeviceAnswer, Run, Scan};
use super::discover_choices;
use super::discover_context::Context;
use crate::session::KnownRuntime;
use crate::texts::discover as texts;
use crate::view::types::{Action, CompareList, CompareView, Emphasis, Pill, Row, Tone};

/// What the project makes of each compared device of a status, by device ID.
pub(crate) type Standings = BTreeMap<String, DeviceStanding>;

/// The devices that the connections of `known` name, the project of the comparison; `None` while the
/// panel shows no runtime, so that no card is marked. A disabled connection names no device, and nor
/// does a connection whose settings do not name one the way its driver reads them (specification 11,
/// "The project's devices").
#[must_use]
pub(crate) fn project_of(known: &[KnownRuntime]) -> Option<Vec<ConfiguredDevice>> {
    (!known.is_empty()).then(|| {
        known
            .iter()
            .flat_map(|runtime| &runtime.connections)
            .filter(|connection| !connection.disabled)
            .filter_map(|connection| {
                ConfiguredDevice::from_connection(
                    &connection.protocol,
                    &Value::Object(connection.params.clone()),
                )
                .ok()
            })
            .flatten()
            .collect()
    })
}

/// The comparison of the devices of `status` with the project; `None` while the context has no project.
#[must_use]
pub(crate) fn compared(status: &Map<String, Value>, ctx: &Context) -> Option<Comparison> {
    let project = ctx.project.as_ref()?;
    let (searched, found) = read_status(&Value::Object(status.clone()));
    Some(compare(&searched, &found, project, &ctx.memory.ignored))
}

// ---- the marks of the cards ----

/// The mark of a card and its rows, from what the project makes of its device: none for a device the
/// comparison does not cover or cannot judge (specification 25 §2).
#[must_use]
pub(crate) fn mark(standing: Option<&DeviceStanding>) -> (Option<Pill>, Vec<Row>) {
    let Some(standing) = standing else {
        return (None, Vec::new());
    };
    match standing.standing {
        Standing::Undecided => (None, Vec::new()),
        Standing::NotInProject => (Some(pill(texts::NOT_IN_PROJECT, Tone::Neutral)), Vec::new()),
        Standing::InProject if !standing.differences.is_empty() => (
            Some(pill(texts::DIFFERS_FROM_PROJECT, Tone::Warning)),
            standing.differences.iter().flat_map(rows).collect(),
        ),
        Standing::InProject if standing.matched.contains(&SourceId::Ads) => {
            (Some(pill(texts::ON_CANVAS, Tone::Ok)), Vec::new())
        }
        Standing::InProject => (Some(pill(texts::IN_PROJECT, Tone::Ok)), Vec::new()),
    }
}

fn pill(word: &str, tone: Tone) -> Pill {
    Pill {
        word: word.to_owned(),
        tone,
        live: false,
    }
}

fn row(label: String, value: String) -> Row {
    Row {
        label,
        value,
        tone: None,
        wide: false,
        copy: None,
    }
}

/// One row for each fact of `difference` that differs.
fn rows(difference: &Difference) -> Vec<Row> {
    difference
        .fields
        .iter()
        .map(|field| {
            row(
                texts::difference_label(field),
                texts::difference_value(
                    &fact(&difference.configured, field).unwrap_or_default(),
                    fact(&difference.found, field).as_deref(),
                ),
            )
        })
        .collect()
}

/// The text of the fact `key`; `None` when the side did not report it.
fn fact(facts: &Facts, key: &str) -> Option<String> {
    match facts.get(key)? {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        other => Some(other.to_string()),
    }
}

// ---- the action and the lists ----

/// Why `Compare with project` cannot run, in this order: the panel shows no runtime, so there is no project;
/// no search was started; the search still runs; or it was cancelled and has not looked everywhere
/// (specification 25 §2; specification 11, "Compare with the project"). `run` is the state of the search the
/// pane shows, `None` before the first search.
fn blocked(run: Option<Run>, ctx: &Context) -> Option<&'static str> {
    if ctx.project.is_none() {
        return Some(texts::COMPARE_NO_PROJECT);
    }
    match run {
        None => Some(texts::COMPARE_NO_SEARCH),
        Some(Run::Running) => Some(texts::COMPARE_RUNNING),
        Some(Run::Cancelled) => Some(texts::COMPARE_CANCELLED),
        Some(Run::Done) => None,
    }
}

/// Whether the lists show: the user asked for them, and nothing blocks the comparison.
#[must_use]
pub(super) fn listed(run: Option<Run>, ctx: &Context) -> bool {
    ctx.compare && blocked(run, ctx).is_none()
}

/// The pane's `Compare with project` action: it shows the lists, or hides them while they show, and it is
/// disabled with its reason when the comparison cannot run. It is a `search` operation that records the
/// change and answers the new view, as `Show ignored` does.
#[must_use]
pub(super) fn action(
    run: Option<Run>,
    ctx: &Context,
    scan: &Scan,
    search_id: Option<&str>,
) -> Action {
    let reason = blocked(run, ctx);
    let showing = listed(run, ctx);
    Action {
        id: "compare".to_owned(),
        label: if showing {
            texts::HIDE_COMPARISON
        } else {
            texts::COMPARE
        }
        .to_owned(),
        enabled: reason.is_none(),
        reason: reason.map(str::to_owned),
        emphasis: if reason.is_none() {
            Emphasis::Secondary
        } else {
            Emphasis::Quiet
        },
        confirm: None,
        op: discover_choices::change_op(
            scan,
            search_id,
            json!({ "kind": "compare", "shown": !showing }),
        ),
    }
}

/// The four lists of `comparison` (specification 25 §2): the configured devices that were not found, the found
/// devices the project does not use, the found devices whose identity differs, and the configured devices that
/// were not compared. `devices` are the devices of the status, which name the found devices.
#[must_use]
pub(super) fn lists(comparison: &Comparison, devices: &[DeviceAnswer]) -> CompareView {
    let found: BTreeMap<&str, &DeviceAnswer> = devices
        .iter()
        .map(|device| (device.id.as_str(), device))
        .collect();
    let title = |id: &str| {
        found
            .get(id)
            .map_or_else(|| id.to_owned(), |device| device_title(device))
    };
    let missing = comparison
        .missing
        .iter()
        .map(|device| row(protocol_name(device), configured_text(device)))
        .collect();
    let unused = comparison
        .unused()
        .map(|(id, standing)| {
            let title = title(id);
            let protocols = standing
                .protocols
                .iter()
                .map(|protocol| texts::source_name(protocol.as_str()))
                .collect::<Vec<_>>()
                .join(", ");
            let place = found.get(id).and_then(|device| place_of(device, &title));
            row(title, texts::unused_device(&protocols, place.as_deref()))
        })
        .collect();
    let differs = comparison
        .differs()
        .flat_map(|(id, difference)| {
            let title = title(id);
            difference.fields.iter().map(move |field| {
                row(
                    title.clone(),
                    texts::listed_difference(
                        field,
                        &fact(&difference.configured, field).unwrap_or_default(),
                        fact(&difference.found, field).as_deref(),
                    ),
                )
            })
        })
        .collect();
    let not_compared = comparison
        .not_compared
        .iter()
        .map(|skipped| {
            row(
                protocol_name(&skipped.device),
                texts::not_compared(&configured_text(&skipped.device), skipped.reason.words()),
            )
        })
        .collect();
    CompareView {
        heading: texts::COMPARED.to_owned(),
        missing: list(texts::HEADING_MISSING, texts::EMPTY_MISSING, missing),
        unused: list(texts::HEADING_UNUSED, texts::EMPTY_UNUSED, unused),
        differs: list(texts::HEADING_DIFFERS, texts::EMPTY_DIFFERS, differs),
        not_compared: list(
            texts::HEADING_NOT_COMPARED,
            texts::EMPTY_NOT_COMPARED,
            not_compared,
        ),
    }
}

fn list(heading: &str, empty_text: &str, rows: Vec<Row>) -> CompareList {
    CompareList {
        heading: heading.to_owned(),
        rows,
        empty_text: empty_text.to_owned(),
    }
}

/// The name of the protocol of a configured device, such as `Modbus TCP`.
fn protocol_name(device: &ConfiguredDevice) -> String {
    texts::source_name(device.protocol().as_str())
}

/// A configured device in words: what names it, such as `10.0.0.9:502, unit 1`.
fn configured_text(device: &ConfiguredDevice) -> String {
    match device {
        ConfiguredDevice::ModbusTcp(modbus) => {
            texts::modbus_device(&modbus.address.to_string(), modbus.unit_id)
        }
        ConfiguredDevice::Ethercat(module) => {
            texts::ethercat_module(&module.model, module.position, &module.adapter)
        }
        ConfiguredDevice::Mqtt(broker) => broker.host.with_port(broker.port),
        ConfiguredDevice::Opcua(server) => server.host.with_port(server.port),
        ConfiguredDevice::Ads(system) => texts::ads_system(&system.ams_net_id, &system.host),
    }
}

/// Where a found device stands, when its title does not say: the position of an `EtherCAT` `SubDevice` on its
/// adapter (several of one model differ by nothing else), else its address.
fn place_of(device: &DeviceAnswer, title: &str) -> Option<String> {
    let ethercat = device
        .finds
        .iter()
        .find(|find| find.protocol == SourceId::Ethercat.as_str());
    if let Some(find) = ethercat {
        let adapter = text(&find.identity, "adapter")?;
        let position = u16::try_from(find.identity.get("slot")?.as_u64()?).ok()?;
        return Some(texts::ethercat_place(position, &adapter));
    }
    device
        .addresses
        .first()
        .filter(|address| address.as_str() != title)
        .cloned()
}
