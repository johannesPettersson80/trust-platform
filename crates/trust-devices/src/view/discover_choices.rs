//! The origins and the `Only search` choices of the Discover pane (specification 25 §2): one tick per source
//! with the reason when a scan from the origin cannot run, the `EtherCAT` warning, the remembered ticks and the
//! change that a tick sends, and the protocols that a runtime advertises beyond the ten sources.

use serde_json::{json, Map, Value};
use trust_runtime::search::{SourceId, BLUETOOTH_UNAVAILABLE};
use trust_runtime::security::required_role_for_control_request;

use super::discover::{origin_body, RuntimeOrigin, Scan, THIS_HOST};
use super::discover_context::Context;
use crate::texts::discover as texts;
use crate::texts::graph;
use crate::view::types::{Action, Emphasis, OnlyChoice, Op, OriginChoice};

/// The origin choices: this computer, then each runtime the panel offers (specification 25 §2).
pub(crate) fn origin_choices(scan: &Scan, runtimes: &[RuntimeOrigin]) -> Vec<OriginChoice> {
    let this_host = OriginChoice {
        id: THIS_HOST.to_owned(),
        label: graph::THIS_COMPUTER.to_owned(),
        enabled: true,
        reason: None,
        selected: *scan == Scan::ThisHost,
    };
    std::iter::once(this_host)
        .chain(runtimes.iter().map(|runtime| OriginChoice {
            id: runtime.id.clone(),
            label: runtime.name.clone(),
            enabled: true,
            reason: None,
            selected: *scan == Scan::Runtime(runtime.id.clone()),
        }))
        .collect()
}

/// The `Only search` choices (specification 25 §2): one per source, in the order of the source table, each
/// ticked as the project remembers it and disabled with its reason where a scan from `scan` cannot run. Until
/// the project remembers a tick, `ran` says which sources are ticked (the first run ticks ADS; a status ticks
/// the sources its search ran). Then, under `Known address or subnet`, each protocol that the origin runtime
/// advertises beyond the ten sources. A tick that can run carries the change that records it; `search_id`
/// names the search the pane shows, if any.
pub(crate) fn only_choices(
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    ctx: &Context,
    search_id: Option<&str>,
    ran: impl Fn(&str) -> bool,
) -> Vec<OnlyChoice> {
    let mut choices: Vec<OnlyChoice> = SourceId::ALL
        .into_iter()
        .map(|source| {
            let reason = scan_reason(source, scan, runtimes, ctx);
            let enabled = reason.is_none();
            let checked = enabled && remembered_tick(source.as_str(), ctx, &ran);
            let label = texts::source_name(source.as_str());
            OnlyChoice {
                source: source.as_str().to_owned(),
                action: enabled
                    .then(|| select_action(source.as_str(), &label, !checked, scan, search_id)),
                label,
                checked,
                enabled,
                reason,
                note: source_note(source).map(str::to_owned),
                group: choice_group(source).to_owned(),
                warning: (source == SourceId::Ethercat).then(|| texts::FIELDBUS_WARNING.to_owned()),
            }
        })
        .collect();
    choices.extend(advertised_choices(scan, runtimes, ctx, search_id, &ran));
    choices
}

/// Whether the choice `source` is ticked: by the project's remembered list, or until it has one, by `ran`.
fn remembered_tick(source: &str, ctx: &Context, ran: &impl Fn(&str) -> bool) -> bool {
    ctx.memory.protocols.as_ref().map_or_else(
        || ran(source),
        |remembered| remembered.iter().any(|name| name == source),
    )
}

/// The operation of a change that the pane records before it answers: a status request of the search the
/// pane shows, with the origin it shows and the `change` (specification 25 §2). Without a search it asks
/// for the idle view.
pub(crate) fn change_op(scan: &Scan, search_id: Option<&str>, change: Value) -> Op {
    let mut request = Map::new();
    request.insert("verb".to_owned(), json!("status"));
    request.insert("origin".to_owned(), origin_body(scan));
    request.insert("change".to_owned(), change);
    if let Some(search_id) = search_id {
        request.insert("search_id".to_owned(), json!(search_id));
    }
    Op::Search {
        request: Value::Object(request),
    }
}

/// The adapter that an `EtherCAT` scan from the origin uses, as the origin's `EtherCAT` connection names it; `None`
/// when it names none (the confirmation then says so).
pub(crate) fn ethercat_adapter(scan: &Scan, ctx: &Context) -> Option<String> {
    match scan {
        Scan::ThisHost => None,
        Scan::Runtime(id) => ctx
            .origin(id)
            .and_then(|facts| facts.ethercat_adapter.clone()),
    }
}

/// The action of a tick: it records `checked` for the choice `source` and answers the new view.
fn select_action(
    source: &str,
    label: &str,
    checked: bool,
    scan: &Scan,
    search_id: Option<&str>,
) -> Action {
    Action {
        id: "select".to_owned(),
        label: label.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Quiet,
        confirm: None,
        op: change_op(
            scan,
            search_id,
            json!({ "kind": "select", "source": source, "checked": checked }),
        ),
    }
}

/// The note under a source's name, as the old pane placed it (`DiscoverPane.tsx` lines 33-49). Serial,
/// USB and Bluetooth had no row in the old pane, so they have no note.
fn source_note(source: SourceId) -> Option<&'static str> {
    match source {
        SourceId::TrustRuntime => Some(texts::NOTE_MDNS),
        SourceId::Ads => Some(texts::NOTE_NETWORK_BROADCAST),
        SourceId::ModbusTcp => Some(texts::NOTE_LOCAL_NETWORK_SCAN),
        SourceId::Mqtt | SourceId::Opcua => Some(texts::NOTE_AT_HOST),
        SourceId::Ethercat => Some(texts::NOTE_BUS_ENUMERATION),
        SourceId::Gpio => Some(texts::NOTE_LOCAL_PINS),
        SourceId::Serial | SourceId::Usb | SourceId::Bluetooth => None,
    }
}

/// The heading group a source's choice sits under (`DiscoverPane.tsx` lines 26-50): the sources that need a
/// known address, and `EtherCAT` and GPIO, which run only inside a runtime.
fn choice_group(source: SourceId) -> &'static str {
    match source {
        SourceId::Mqtt | SourceId::Opcua => texts::GROUP_KNOWN_ADDRESS,
        SourceId::Ethercat | SourceId::Gpio => texts::GROUP_RUNTIME_HARDWARE,
        SourceId::TrustRuntime
        | SourceId::Ads
        | SourceId::ModbusTcp
        | SourceId::Serial
        | SourceId::Usb
        | SourceId::Bluetooth => texts::GROUP_FIND,
    }
}

/// Why a scan of `source` from `scan` cannot run, or `None` when it can (specification 25 §2, lines 82-88).
fn scan_reason(
    source: SourceId,
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    ctx: &Context,
) -> Option<String> {
    if source == SourceId::Bluetooth {
        return Some(BLUETOOTH_UNAVAILABLE.to_owned());
    }
    let needs_runtime = matches!(source, SourceId::Ethercat | SourceId::Gpio);
    match (origin_reason(needs_runtime, scan, runtimes), scan) {
        (Some(reason), _) => Some(reason),
        (None, Scan::Runtime(origin)) => role_reason(source, origin, ctx),
        (None, Scan::ThisHost) => None,
    }
}

/// Why a scan from `scan` cannot run because of its origin: `EtherCAT` and GPIO run only from a runtime, and
/// every scan from a runtime that is not ready is disabled until it is started or connected.
fn origin_reason(needs_runtime: bool, scan: &Scan, runtimes: &[RuntimeOrigin]) -> Option<String> {
    match scan {
        Scan::ThisHost => needs_runtime.then(|| texts::CHOOSE_RUNTIME_FOR_HARDWARE.to_owned()),
        Scan::Runtime(id) => match runtimes.iter().find(|runtime| runtime.id == *id) {
            Some(runtime) if runtime.ready => None,
            _ if needs_runtime => Some(texts::START_RUNTIME_FOR_HARDWARE.to_owned()),
            Some(runtime) => Some(texts::start_or_connect(&runtime.name)),
            None => Some(texts::START_A_RUNTIME.to_owned()),
        },
    }
}

/// Why `EtherCAT` cannot run from an origin whose connection has a role that a confirmed scan does not allow
/// (decision O12). `None` while the role is unknown or allowed.
fn role_reason(source: SourceId, origin: &str, ctx: &Context) -> Option<String> {
    if source != SourceId::Ethercat {
        return None;
    }
    let role = ctx.origin(origin)?.role?;
    let required = required_role_for_control_request(
        "comm.search.start",
        Some(&json!({ "confirm_fieldbus_scan": true })),
    );
    (!role.allows(required))
        .then(|| graph::role_needed("Scanning EtherCAT", required.as_str(), role.as_str()))
}

/// The protocols that the origin runtime advertises beyond the ten sources: one choice each, under `Known address
/// or subnet`, named by its advertised label and noted `discoverable` (`DiscoverPane.tsx` line 94). A scan of one
/// runs through the origin runtime's `comm.discover` (specification 25 §2), so it is enabled wherever a scan from
/// that runtime can run.
fn advertised_choices(
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    ctx: &Context,
    search_id: Option<&str>,
    ran: &impl Fn(&str) -> bool,
) -> Vec<OnlyChoice> {
    let Scan::Runtime(id) = scan else {
        return Vec::new();
    };
    let Some(discover) = ctx.origin(id).and_then(|facts| facts.discover.as_ref()) else {
        return Vec::new();
    };
    discover
        .beyond
        .iter()
        .map(|advertised| {
            let reason = origin_reason(false, scan, runtimes);
            let enabled = reason.is_none();
            let checked = enabled && remembered_tick(&advertised.id, ctx, ran);
            OnlyChoice {
                source: advertised.id.clone(),
                label: advertised.label.clone(),
                checked,
                enabled,
                reason,
                note: Some(texts::NOTE_DISCOVERABLE.to_owned()),
                group: texts::GROUP_KNOWN_ADDRESS.to_owned(),
                warning: None,
                action: enabled.then(|| {
                    select_action(&advertised.id, &advertised.label, !checked, scan, search_id)
                }),
            }
        })
        .collect()
}
