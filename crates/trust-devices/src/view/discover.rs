//! The Discover pane (specification 25 §2): the status of one search, turned into what the pane draws. One
//! card per device with every protocol found on it, the source rows, the scan choices with the reason of each
//! scan that cannot run, the headings, notes and help texts, the pane's actions (`Search`, `Cancel`,
//! `Compare with project` and `Configure ADS ports to scan`), the lists of that comparison, the empty-result
//! checks, and the card actions that fit the device's identity. The engine adds this view to the answer of a
//! search status (design §1.4.1); the search itself runs in the engine's search module, and its sources in the
//! runtime.
//!
//! - `runtime_origins`: the runtimes the panel offers as origins of a search, and whether each is
//!   ready: attached, or with `connected`, `running` or `online` health (specification 25 §2).
//! - A runtime card (a device that a truST runtime answered for) gets `Adopt`; a `TwinCAT` card gets `Add to
//!   canvas` or `Manage tags` (`discover_ads`); any other card gets `+ Add` with the connection it would
//!   create on the owning runtime. Every card gets `Ignore`, or `Stop ignoring` while the project ignores it.
//!   Every card also carries the project's view of its device, and the pane the lists of the comparison, when
//!   asked for (`discover_compare`).
//! - The scan choices, the origins and their reasons are `discover_choices`; the facts that the view reads
//!   beside the status are `discover_context`.
//! - A source or protocol that this build does not know is left out, and the view asks to update truST.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Deserializer};
use serde_json::{json, Map, Value};
use trust_runtime::connectors::DiscoveryConfidence;
use trust_runtime::search::SourceId;

use super::discover_ads;
use super::discover_choices;
use super::discover_compare::{self, Standings};
use super::discover_context::{self, Context};
use super::discover_names::{advertised_labels, known_name, known_source, label_of};
use super::health;
use crate::gather::Gathered;
use crate::inputs;
use crate::panes::forms::FormRequest;
use crate::request::HostFacts;
use crate::session::KnownRuntime;
use crate::texts::discover as texts;
use crate::texts::graph;
use crate::topology;
use crate::view::types::{
    Action, CardProtocol, Confirm, DeviceCard, DiscoverView, Emphasis, HostEffect, OnlyChoice, Op,
    Row, ShowIgnored, SourceRow, TargetsInput, Tone,
};

/// The origin of a search that runs on this computer (`origin` in `comm.search.start`).
pub(crate) const THIS_HOST: &str = "this_host";

/// One runtime that the panel offers as the origin of a search (specification 25 §2, `Scan from`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RuntimeOrigin {
    /// Its item ID, which a search names as its origin.
    pub(crate) id: String,
    /// Its name, which its origin choice shows.
    pub(crate) name: String,
    /// Whether it is attached, or has `connected`, `running` or `online` health: only then do its
    /// hardware scans run.
    pub(crate) ready: bool,
}

/// Where the search that the pane shows runs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Scan {
    /// On this computer.
    ThisHost,
    /// Inside the runtime with this item ID.
    Runtime(String),
}

/// The runtimes that the panel offers as origins, in the order its last view showed them, each with
/// whether it is ready. Readiness is read from the panel's last gather (specification 25 §2).
#[must_use]
pub(crate) fn runtime_origins(
    known: &[KnownRuntime],
    gathered: Option<&Gathered>,
    host: &HostFacts,
) -> Vec<RuntimeOrigin> {
    let ready = gathered.map_or_else(BTreeSet::new, |gathered| ready_runtimes(gathered, host));
    known
        .iter()
        .map(|runtime| {
            let id = runtime.id.as_str().to_owned();
            RuntimeOrigin {
                ready: ready.contains(&id),
                name: runtime.name.clone(),
                id,
            }
        })
        .collect()
}

/// The item IDs of the runtimes that are attached, or have `connected`, `running` or `online`
/// health, as the gather's topology reports them.
fn ready_runtimes(gathered: &Gathered, host: &HostFacts) -> BTreeSet<String> {
    let inputs = inputs::read(gathered, host, None);
    let topology = topology::merge(&inputs.topology);
    topology
        .runtimes()
        .filter(|runtime| {
            runtime.attached
                || matches!(runtime.health.as_str(), "connected" | "running" | "online")
        })
        .map(|runtime| runtime.id.clone())
        .collect()
}

/// The Discover view of one search status (specification 25 §2), read with what the panel knows beside it:
/// `ctx` holds the project's remembered ticks and ignored finds, the runtimes' roles and advertised protocols,
/// and the ADS connections on the canvas. `status` is the body that `comm.search.status` answers, or the body
/// the engine builds for a runtime that cannot search; `scan` is the origin the search ran at, `runtimes` the
/// origins the panel offers, and `now_ms` the time of the view in milliseconds since the Unix epoch.
///
/// A source or a protocol that this build does not know is left out of the view, and the view then carries
/// the line that asks to update truST (specification 25 §2). An ADS find that no port answered is left out.
///
/// # Errors
/// The body is not a search status: a source or a device lacks a member that the answer requires.
pub(crate) fn project_in(
    status: &Map<String, Value>,
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    ctx: &Context,
    now_ms: u64,
) -> Result<DiscoverView, serde_json::Error> {
    let answer: Answer = serde_json::from_value(Value::Object(status.clone()))?;
    let advertised = advertised_labels(scan, ctx);
    let mut left_out = false;
    let mut known_sources = Vec::new();
    for source in &answer.sources {
        if known_name(&source.id, &advertised) {
            known_sources.push(source);
        } else {
            left_out = true;
        }
    }
    let mut sources = known_sources
        .iter()
        .map(|source| source_row(source, &advertised))
        .collect::<Result<Vec<_>, _>>()?;
    verify_ads(&answer, &mut sources, ctx);
    let comparison = discover_compare::compared(status, ctx);
    let compare = comparison
        .as_ref()
        .filter(|_| discover_compare::listed(Some(answer.state), ctx))
        .map(|comparison| discover_compare::lists(comparison, &answer.devices));
    let mut devices = Vec::new();
    for device in answer.devices {
        let had_finds = !device.finds.is_empty();
        let (known, unknown): (Vec<FindAnswer>, Vec<FindAnswer>) = device
            .finds
            .into_iter()
            .filter(|find| !discover_ads::unanswered(&find.protocol, &find.params))
            .partition(|find| known_name(&find.protocol, &advertised) && find.confidence.is_some());
        left_out |= !unknown.is_empty();
        if had_finds && known.is_empty() {
            continue;
        }
        if ctx.memory.ignored.contains(&device.id) && !ctx.show_ignored {
            continue;
        }
        devices.push(DeviceAnswer {
            finds: known,
            ..device
        });
    }
    let only = discover_choices::only_choices(
        scan,
        runtimes,
        ctx,
        answer.search_id.as_deref(),
        |source| answer.sources.iter().any(|given| given.id == source),
    );
    let shown = Shown {
        advertised: &advertised,
        standings: comparison.as_ref().map(|comparison| &comparison.devices),
    };
    let cards: Vec<DeviceCard> = devices
        .iter()
        .map(|device| {
            card(
                device,
                scan,
                runtimes,
                now_ms,
                ctx,
                answer.search_id.as_deref(),
                &shown,
            )
        })
        .collect();
    let ran = known_sources
        .iter()
        .any(|source| source.state != Progress::Skipped);
    let running = answer.state == Run::Running;
    let empty_text = (!running && cards.is_empty() && ran).then(|| empty_text(scan, runtimes));
    let unavailable_text = (only.is_empty() && !running && sources.is_empty() && cards.is_empty())
        .then(|| texts::NO_DISCOVERY_YET.to_owned());
    let mut view = frame(
        Some(answer.state),
        answer.search_id,
        scan,
        runtimes,
        ctx,
        only,
    );
    view.compare = compare;
    view.unavailable_text = unavailable_text;
    view.sources = sources;
    view.update_text = left_out.then(|| texts::UPDATE_TRUST.to_owned());
    view.cards = cards;
    view.empty_text = empty_text;
    Ok(view)
}

/// The ADS source row once the search has ended without a responding port (old `discoverResults`): when `TwinCAT`
/// identity was found and no configured port answered, the row reads `0 found` and carries the reason. While the
/// search still runs, `TwinCAT` may yet answer, so the row has no text yet.
fn verify_ads(answer: &Answer, sources: &mut [SourceRow], ctx: &Context) {
    let ads_finds: Vec<&FindAnswer> = answer
        .devices
        .iter()
        .flat_map(|device| &device.finds)
        .filter(|find| find.protocol == SourceId::Ads.as_str())
        .collect();
    let answered = ads_finds
        .iter()
        .any(|find| !discover_ads::unanswered(&find.protocol, &find.params));
    let verification = (answer.state != Run::Running)
        .then(|| discover_ads::unverified_text(!ads_finds.is_empty(), answered, &ctx.ads_ports))
        .flatten();
    if let Some(text) = verification
        && let Some(row) = sources
            .iter_mut()
            .find(|row| row.id == SourceId::Ads.as_str())
    {
        // The old pane counted the devices that answered (`verified.length`), so the row reads `0 found`.
        row.count = Some(0);
        row.word = texts::found(0);
        row.tone = Tone::Neutral;
        row.reason = Some(text);
    }
}

/// The view that every Discover view shares (specification 25 §2): the headings, the origins, the
/// `Only search` choices, the `Show ignored` toggle, the empty targets and the pane's actions. The caller adds
/// what its search found.
fn frame(
    run: Option<Run>,
    search_id: Option<String>,
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    ctx: &Context,
    only: Vec<OnlyChoice>,
) -> DiscoverView {
    let choose_text = (!only.is_empty()).then(|| texts::CHOOSE_WHAT_TO_FIND.to_owned());
    let actions = pane_actions(run, search_id.as_deref(), scan, &only, ctx);
    let show_ignored = show_ignored_toggle(scan, search_id.as_deref(), ctx);
    DiscoverView {
        search_id,
        running: run == Some(Run::Running),
        progress_text: None,
        breadcrumb: texts::BREADCRUMB.to_owned(),
        close_label: texts::CLOSE_LABEL.to_owned(),
        heading_find: texts::HEADING_FIND.to_owned(),
        heading_scan_from: texts::HEADING_SCAN_FROM.to_owned(),
        heading_targets: texts::HEADING_TARGETS.to_owned(),
        heading_advanced: texts::HEADING_ADVANCED.to_owned(),
        heading_runtime: texts::HEADING_RUNTIME.to_owned(),
        unavailable_text: None,
        choose_text,
        origins: discover_choices::origin_choices(scan, runtimes),
        only,
        targets: targets(),
        sources: Vec::new(),
        update_text: None,
        fieldbus: Vec::new(),
        show_ignored,
        cards: Vec::new(),
        compare: None,
        empty_text: None,
        error_text: None,
        actions,
    }
}

/// The Discover view before the first search, and after a search is cleared (specification 25 §2): the
/// origins, every scan choice with its reason, the ticks that the project remembers (ADS on the first run),
/// empty targets and the Search action.
#[must_use]
pub(crate) fn idle(scan: &Scan, runtimes: &[RuntimeOrigin], ctx: &Context) -> DiscoverView {
    frame(
        None,
        None,
        scan,
        runtimes,
        ctx,
        discover_choices::only_choices(scan, runtimes, ctx, None, |source| {
            source == SourceId::Ads.as_str()
        }),
    )
}

/// The Discover view of a search that failed at a runtime (specification 25 §2), or of a project file that
/// cannot be read: the failure's text beside the pane's actions, so that Search can run again.
#[must_use]
pub(crate) fn failed(
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    ctx: &Context,
    text: String,
) -> DiscoverView {
    let mut view = idle(scan, runtimes, ctx);
    view.error_text = Some(text);
    view
}

/// The confidence of a find, or `None` for a value that this build does not know. Such a find is left
/// out of the view, as an unknown protocol is, and the view does not fail (specification 25 §2).
fn lenient_confidence<'de, D>(deserializer: D) -> Result<Option<DiscoveryConfidence>, D::Error>
where
    D: Deserializer<'de>,
{
    let name = Option::<String>::deserialize(deserializer)?;
    Ok(name.and_then(|name| DiscoveryConfidence::deserialize(Value::String(name)).ok()))
}

// ---- the answer that `comm.search.status` gives (specification 11, "One search") ----

/// The body of a search status, with the members that the Discover view reads. The names of sources
/// and protocols stay text here, so that a name this build does not know can be left out.
#[derive(Debug, Deserialize)]
struct Answer {
    search_id: Option<String>,
    state: Run,
    sources: Vec<SourceAnswer>,
    devices: Vec<DeviceAnswer>,
}

/// The state of a search.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Run {
    Running,
    Done,
    Cancelled,
}

/// One source of a search.
#[derive(Debug, Deserialize)]
struct SourceAnswer {
    id: String,
    state: Progress,
    count: Option<u32>,
    reason: Option<String>,
}

/// The progress of one source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Progress {
    Searching,
    Found,
    TimedOut,
    Cancelled,
    Skipped,
}

/// One device: every result that shares an address, an AMS Net ID, a USB serial number or a
/// runtime ID.
#[derive(Debug, Deserialize)]
pub(super) struct DeviceAnswer {
    pub(super) id: String,
    #[serde(default)]
    pub(super) addresses: Vec<String>,
    #[serde(default)]
    ams_net_ids: Vec<String>,
    #[serde(default)]
    usb_serials: Vec<String>,
    #[serde(default)]
    runtime_ids: Vec<String>,
    last_seen_ms: u64,
    pub(super) finds: Vec<FindAnswer>,
}

/// One protocol result on a device.
#[derive(Debug, Deserialize)]
pub(super) struct FindAnswer {
    pub(super) protocol: String,
    source: String,
    /// The confidence, or `None` for a value that this build does not know (see `lenient_confidence`).
    #[serde(default, deserialize_with = "lenient_confidence")]
    confidence: Option<DiscoveryConfidence>,
    #[serde(default)]
    simulated: bool,
    #[serde(default)]
    pub(super) identity: Value,
    #[serde(default)]
    params: Value,
    #[serde(default)]
    warnings: Vec<String>,
}

// ---- the source rows ----

/// The row of one source: its word, its count and, when it did not run, its reason (specification 25 §2,
/// lines 55-58). A source that ran must report its count.
fn source_row(
    source: &SourceAnswer,
    advertised: &BTreeMap<String, String>,
) -> Result<SourceRow, serde_json::Error> {
    let name = source.id.as_str();
    let (word, tone, count, reason) = match (source.state, source.count) {
        (Progress::Skipped, _) => (
            texts::SKIPPED.to_owned(),
            Tone::Muted,
            None,
            Some(
                source
                    .reason
                    .clone()
                    .unwrap_or_else(|| texts::SKIPPED_WITHOUT_REASON.to_owned()),
            ),
        ),
        (_, None) => {
            return Err(serde::de::Error::custom(format!(
                "the source {name} ran but reports no count"
            )));
        }
        (Progress::Searching, Some(count)) => (
            texts::SEARCHING.to_owned(),
            Tone::Neutral,
            Some(count),
            None,
        ),
        (Progress::Found, Some(count)) => {
            let tone = if count > 0 { Tone::Ok } else { Tone::Neutral };
            (texts::found(count), tone, Some(count), None)
        }
        (Progress::TimedOut, Some(count)) => (
            texts::TIMED_OUT.to_owned(),
            Tone::Warning,
            Some(count),
            None,
        ),
        (Progress::Cancelled, Some(count)) => (
            texts::CANCELLED.to_owned(),
            Tone::Neutral,
            Some(count),
            None,
        ),
    };
    Ok(SourceRow {
        id: name.to_owned(),
        label: label_of(name, advertised),
        word,
        count,
        reason,
        tone,
    })
}

/// The inputs of `Known address or subnet`.
fn targets() -> TargetsInput {
    TargetsInput {
        hosts_label: texts::TARGET_HOSTS_LABEL.to_owned(),
        cidr_label: texts::TARGET_CIDR_LABEL.to_owned(),
        hosts_placeholder: texts::TARGET_HOSTS_PLACEHOLDER.to_owned(),
        cidr_placeholder: texts::TARGET_CIDR_PLACEHOLDER.to_owned(),
    }
}

/// The pane's own actions (specification 25 §2): `Search` starts one search with the origin, the ticked
/// sources and the typed targets; `Cancel` shows while a scan runs; `Compare with project` lists the
/// comparison of a finished search; `Configure ADS ports to scan` opens the ADS port settings. A disabled `Search` carries its reason: a scan that runs, or no ticked source. A
/// `Search` that scans `EtherCAT` asks first, and only that confirmed search carries `confirm_fieldbus_scan`
/// (specification 25 §2). The search actions are `search` operations: the host sends each request to
/// `trust-lsp.devicesSearch`, not to `trust-lsp.devicesAct`, with the body specification 25 §2 names.
fn pane_actions(
    run: Option<Run>,
    search_id: Option<&str>,
    scan: &Scan,
    only: &[OnlyChoice],
    ctx: &Context,
) -> Vec<Action> {
    let running = run == Some(Run::Running);
    let ticked: Vec<&str> = only
        .iter()
        .filter(|choice| choice.checked && choice.enabled)
        .map(|choice| choice.source.as_str())
        .collect();
    let reason = if running {
        Some(texts::SEARCH_RUNNING)
    } else if ticked.is_empty() {
        Some(texts::SELECT_A_SCAN_TYPE)
    } else {
        None
    };
    let enabled = reason.is_none();
    let label = if running {
        texts::SCANNING
    } else {
        texts::SEARCH
    };
    let fieldbus = enabled && ticked.contains(&SourceId::Ethercat.as_str());
    let confirm = fieldbus.then(|| Confirm {
        text: texts::ethercat_confirmation(
            discover_choices::ethercat_adapter(scan, ctx).as_deref(),
        ),
        confirm: texts::SEARCH.to_owned(),
        cancel: texts::CANCEL.to_owned(),
    });
    let mut params = json!({ "only": ticked });
    if fieldbus {
        params["confirm_fieldbus_scan"] = json!(true);
    }
    let mut actions = vec![Action {
        id: "search".to_owned(),
        label: label.to_owned(),
        enabled,
        reason: reason.map(str::to_owned),
        emphasis: if enabled {
            Emphasis::Primary
        } else {
            Emphasis::Quiet
        },
        confirm,
        op: Op::Search {
            request: json!({
                "verb": "start",
                "origin": origin_body(scan),
                "params": params,
            }),
        },
    }];
    if let Some(search_id) = search_id.filter(|_| running) {
        actions.push(Action {
            id: "cancel".to_owned(),
            label: texts::CANCEL.to_owned(),
            enabled: true,
            reason: None,
            emphasis: Emphasis::Secondary,
            confirm: None,
            op: Op::Search {
                request: json!({
                    "verb": "cancel",
                    "origin": origin_body(scan),
                    "search_id": search_id,
                }),
            },
        });
    }
    actions.push(discover_compare::action(run, ctx, scan, search_id));
    actions.push(Action {
        id: "configure_ads_ports".to_owned(),
        label: texts::CONFIGURE_ADS_PORTS.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Secondary,
        confirm: None,
        op: Op::Host {
            effect: HostEffect::OpenSettings {
                query: texts::ADS_PORTS_SETTING.to_owned(),
            },
        },
    });
    actions
}

/// The `Show ignored` toggle (decision O14): ticked while the finds that the project ignores are shown. Its
/// change records the panel's own choice and answers the view with the toggle flipped.
fn show_ignored_toggle(scan: &Scan, search_id: Option<&str>, ctx: &Context) -> ShowIgnored {
    ShowIgnored {
        label: texts::SHOW_IGNORED.to_owned(),
        checked: ctx.show_ignored,
        action: Action {
            id: "show_ignored".to_owned(),
            label: texts::SHOW_IGNORED.to_owned(),
            enabled: true,
            reason: None,
            emphasis: Emphasis::Quiet,
            confirm: None,
            op: discover_choices::change_op(
                scan,
                search_id,
                json!({ "kind": "show_ignored", "shown": !ctx.show_ignored }),
            ),
        },
    }
}

/// The origin a search request names, as `trust-lsp.devicesSearch` reads an origin: `this_host`, or
/// `{"runtime": <item ID>}` (specification 25 §2).
pub(crate) fn origin_body(scan: &Scan) -> Value {
    match scan {
        Scan::ThisHost => json!(THIS_HOST),
        Scan::Runtime(id) => json!({ "runtime": id }),
    }
}

/// The empty-result text (specification 25 §2, lines 75-77): the checks, and what the hardware
/// scans need from a runtime.
fn empty_text(scan: &Scan, runtimes: &[RuntimeOrigin]) -> String {
    let selected_ready = match scan {
        Scan::Runtime(id) => runtimes
            .iter()
            .any(|runtime| runtime.id == *id && runtime.ready),
        Scan::ThisHost => false,
    };
    let hardware = if selected_ready {
        texts::HARDWARE_FROM_SELECTED
    } else if runtimes.iter().any(|runtime| runtime.ready) {
        texts::CHOOSE_RUNNING_RUNTIME
    } else {
        texts::START_RUNTIME_FIRST
    };
    format!("{} {hardware}", texts::EMPTY_CHECKS)
}

// ---- the cards ----

/// What the cards of one view read beside their device: the names of the protocols that runtimes advertise,
/// and what the project makes of each device.
struct Shown<'a> {
    advertised: &'a BTreeMap<String, String>,
    /// What the project makes of each device; `None` while the panel shows no project.
    standings: Option<&'a Standings>,
}

/// The card of one device (specification 25 §2, lines 64-72): every protocol found on it, one address for a
/// runtime, the `TwinCAT` ports, the project's view of the device, and the actions that fit its identity. A
/// card that the session's rules disable says why (`session_reason`); `Ignore` is never disabled.
fn card(
    device: &DeviceAnswer,
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    now_ms: u64,
    ctx: &Context,
    search_id: Option<&str>,
    shown: &Shown<'_>,
) -> DeviceCard {
    let advertised = shown.advertised;
    let standing = shown
        .standings
        .and_then(|standings| standings.get(&device.id));
    let runtime = device
        .finds
        .iter()
        .find(|find| find.protocol == SourceId::TrustRuntime.as_str());
    let ads = device
        .finds
        .iter()
        .find(|find| find.protocol == SourceId::Ads.as_str());
    let endpoint = runtime.and_then(control_endpoint);
    let title = device_title(device);
    let owner = owner(scan, runtimes);
    let reason = session_reason(device, scan, runtimes, ctx);
    let ports = ads.map_or_else(Vec::new, |find| {
        discover_ads::responding_ports(&find.params)
    });
    let on_canvas = standing.is_some_and(|standing| standing.matched.contains(&SourceId::Ads));
    let (project, differs) = discover_compare::mark(standing);
    let mut actions = match (runtime, ads) {
        (Some(_), _) => runtime_actions(device, endpoint.as_deref(), &title),
        (None, Some(find)) => vec![discover_ads::primary_action(
            &device.id,
            &find.params,
            &title,
            on_canvas,
            owner.map(|runtime| runtime.id.as_str()),
        )],
        (None, None) => device_actions(device, owner),
    };
    if let Some(reason) = &reason {
        disable(&mut actions, reason);
    }
    let ignored = ctx.memory.ignored.contains(&device.id);
    actions.push(ignore_action(scan, search_id, &device.id, ignored));
    DeviceCard {
        id: device.id.clone(),
        title,
        addresses: match &endpoint {
            Some(endpoint) => vec![host_and_port(endpoint)],
            None => device.addresses.clone(),
        },
        protocols: device
            .finds
            .iter()
            .filter_map(|find| card_protocol(find, advertised))
            .collect(),
        identity: identity_rows(device),
        last_seen: graph::ago(seconds_since(now_ms, device.last_seen_ms)),
        project,
        differs,
        warnings: device
            .finds
            .iter()
            .flat_map(|find| find.warnings.iter().cloned())
            .collect(),
        actions,
        stale_reason: reason,
        responding_ports: ads.map(|_| texts::responding_ads_ports(ports.len())),
        ports: ads.map_or_else(Vec::new, |find| {
            discover_ads::port_actions(&device.id, &find.params, &ports)
        }),
    }
}

/// Why a card's actions are disabled (specification 25 §2, old lines 334-340 and 385-389), in this order: a
/// search that this panel session did not start, an origin runtime that is no longer shown, and a protocol
/// that the origin runtime does not advertise.
fn session_reason(
    device: &DeviceAnswer,
    scan: &Scan,
    runtimes: &[RuntimeOrigin],
    ctx: &Context,
) -> Option<String> {
    if !ctx.session_current {
        return Some(texts::CONTEXT_CHANGED.to_owned());
    }
    if let Some(reason) = stale_reason(scan, runtimes) {
        return Some(reason);
    }
    device
        .finds
        .iter()
        .any(|find| protocol_unavailable(scan, &find.protocol, ctx))
        .then(|| texts::PROTOCOL_UNAVAILABLE.to_owned())
}

/// Whether the origin runtime does not advertise `protocol` with `discover`, when its `comm.schema` is known
/// and the protocol has a schema ID (`discoverySession.ts`).
fn protocol_unavailable(scan: &Scan, protocol: &str, ctx: &Context) -> bool {
    let (Scan::Runtime(id), Some(source)) = (scan, known_source(protocol)) else {
        return false;
    };
    let Some(schema) = discover_context::schema_id(source) else {
        return false;
    };
    ctx.origin(id)
        .and_then(|facts| facts.discover.as_ref())
        .is_some_and(|discover| !discover.ids.contains(schema))
}

/// The card's `Ignore`, or `Stop ignoring` on a find that the project ignores (decision O14). It records the
/// change and answers the new view.
fn ignore_action(scan: &Scan, search_id: Option<&str>, device_id: &str, ignored: bool) -> Action {
    let (id, label) = if ignored {
        ("stop_ignoring", texts::STOP_IGNORING)
    } else {
        ("ignore", texts::IGNORE)
    };
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Secondary,
        confirm: None,
        op: discover_choices::change_op(
            scan,
            search_id,
            json!({ "kind": "ignore", "device": device_id, "ignored": !ignored }),
        ),
    }
}

/// A protocol line of a card: its name, how it was found, its confidence, and the mark of a simulated
/// find. A find whose confidence this build does not know has no line.
fn card_protocol(find: &FindAnswer, advertised: &BTreeMap<String, String>) -> Option<CardProtocol> {
    let confidence = find.confidence?;
    Some(CardProtocol {
        name: label_of(&find.protocol, advertised),
        source: texts::discovery_source(&find.source),
        confidence: health::confidence_word(confidence).to_owned(),
        simulated: find.simulated,
        mark: find.simulated.then(|| texts::SIMULATED_MARK.to_owned()),
    })
}

/// The control endpoint that a runtime's result names, as `tcp://host:port`.
fn control_endpoint(find: &FindAnswer) -> Option<String> {
    text(&find.identity, "control").or_else(|| text(&find.params, "control_endpoint"))
}

/// The endpoint as a card shows it: its `tcp://` prefix is left out (specification 25 §2, lines
/// 69-70).
fn host_and_port(endpoint: &str) -> String {
    endpoint
        .strip_prefix("tcp://")
        .unwrap_or(endpoint)
        .to_owned()
}

/// The name that the device's protocols read about it, else its AMS Net ID, its address or its ID.
pub(super) fn device_title(device: &DeviceAnswer) -> String {
    const NAMES: [&str; 8] = [
        "name",
        "computer_name",
        "application_name",
        "product",
        "model",
        "port",
        "chip_label",
        "chip",
    ];
    device
        .finds
        .iter()
        .find_map(|find| NAMES.iter().find_map(|key| text(&find.identity, key)))
        .or_else(|| device.ams_net_ids.first().cloned())
        .or_else(|| device.addresses.first().cloned())
        .unwrap_or_else(|| device.id.clone())
}

/// The identity rows of a device: its AMS Net ID, its host when it is not a runtime (`DiscoverPane.tsx` line
/// 379), its USB serial number and runtime ID, and a runtime's truST version and platform. A fact that the
/// device does not have has no row.
fn identity_rows(device: &DeviceAnswer) -> Vec<Row> {
    let runtime = device
        .finds
        .iter()
        .find(|find| find.protocol == SourceId::TrustRuntime.as_str());
    let host = match runtime {
        Some(_) => String::new(),
        None => device
            .finds
            .iter()
            .find_map(|find| text(&find.params, "host"))
            .unwrap_or_default(),
    };
    let mut facts = vec![
        (texts::ROW_AMS_NET_ID, device.ams_net_ids.join(" · ")),
        (texts::ROW_HOST, host),
        (texts::ROW_USB_SERIAL, device.usb_serials.join(" · ")),
        (texts::ROW_RUNTIME_ID, device.runtime_ids.join(" · ")),
    ];
    if let Some(runtime) = runtime {
        facts.push((
            graph::ROW_VERSION,
            text(&runtime.identity, "version").unwrap_or_default(),
        ));
        facts.push((
            texts::ROW_PLATFORM,
            text(&runtime.identity, "platform").unwrap_or_default(),
        ));
    }
    facts
        .into_iter()
        .filter(|(_, value)| !value.is_empty())
        .map(|(label, value)| Row {
            label: label.to_owned(),
            value,
            tone: None,
            wide: false,
            copy: None,
        })
        .collect()
}

/// The actions of a runtime card: `Adopt`, which keeps the discovered label and needs the endpoint
/// to reach the runtime (specification 25 §2, lines 89-90).
fn runtime_actions(device: &DeviceAnswer, endpoint: Option<&str>, title: &str) -> Vec<Action> {
    let Some(endpoint) = endpoint else {
        return vec![disabled(
            "adopt",
            texts::ADOPT,
            graph::CONNECT_WITHOUT_ENDPOINT,
            "runtime.adopt",
        )];
    };
    vec![Action {
        id: "adopt".to_owned(),
        label: texts::ADOPT.to_owned(),
        enabled: true,
        reason: None,
        emphasis: Emphasis::Primary,
        confirm: None,
        op: Op::Rust {
            action: "runtime.adopt".to_owned(),
            payload: json!({ "runtime": device.id, "endpoint": endpoint, "label": title }),
        },
    }]
}

/// The actions of a device card: `+ Add` with the connection it would create on the owning runtime,
/// from the first of its protocols that a connection of truST uses.
fn device_actions(device: &DeviceAnswer, owner: Option<&RuntimeOrigin>) -> Vec<Action> {
    let connection = device
        .finds
        .iter()
        .find_map(|find| connection_protocol(&find.protocol).map(|protocol| (protocol, find)));
    let action = match (connection, owner) {
        (Some((protocol, find)), Some(owner)) => {
            let request = FormRequest {
                protocol: Some(protocol.to_owned()),
                runtime: Some(owner.id.clone()),
                prefill: find
                    .params
                    .as_object()
                    .cloned()
                    .filter(|map| !map.is_empty()),
                ..FormRequest::default()
            };
            Action {
                id: "add".to_owned(),
                label: graph::ADD.to_owned(),
                enabled: true,
                reason: None,
                emphasis: Emphasis::Secondary,
                confirm: None,
                op: Op::Rust {
                    action: "form.open".to_owned(),
                    payload: request.payload(),
                },
            }
        }
        (Some(_), None) => disabled("add", graph::ADD, texts::ADD_NEEDS_RUNTIME, "form.open"),
        (None, _) => disabled("add", graph::ADD, texts::ADD_NOT_OFFERED, "form.open"),
    };
    vec![action]
}

/// The connection of truST that a device of `protocol` would be added as; `None` when no connection
/// uses it, or the name is not a source that this build knows.
fn connection_protocol(name: &str) -> Option<&'static str> {
    match known_source(name)? {
        SourceId::Ads => Some("ads"),
        SourceId::ModbusTcp => Some("modbus_tcp"),
        SourceId::Mqtt => Some("mqtt"),
        SourceId::Opcua => Some("opcua_client"),
        SourceId::Ethercat => Some("ethercat"),
        SourceId::Gpio => Some("gpio"),
        SourceId::TrustRuntime | SourceId::Serial | SourceId::Usb | SourceId::Bluetooth => None,
    }
}

/// The runtime that a device's connection is added to: the origin of the search, else the first
/// runtime the panel offers.
fn owner<'a>(scan: &Scan, runtimes: &'a [RuntimeOrigin]) -> Option<&'a RuntimeOrigin> {
    match scan {
        Scan::ThisHost => runtimes.first(),
        Scan::Runtime(id) => runtimes.iter().find(|runtime| runtime.id == *id),
    }
}

/// Why the results of a runtime that the panel no longer shows cannot be added (specification 25 §2):
/// the runtime that found them is gone.
fn stale_reason(scan: &Scan, runtimes: &[RuntimeOrigin]) -> Option<String> {
    match scan {
        Scan::ThisHost => None,
        Scan::Runtime(id) => (!runtimes.iter().any(|runtime| runtime.id == *id))
            .then(|| texts::STALE_RUNTIME.to_owned()),
    }
}

/// Disables every action with `reason`; a disabled action is neutral, not a primary one.
fn disable(actions: &mut [Action], reason: &str) {
    for action in actions {
        action.enabled = false;
        action.reason = Some(reason.to_owned());
        action.emphasis = Emphasis::Quiet;
    }
}

/// A disabled action with its reason. It is never sent, so its operation names the action only.
fn disabled(id: &str, label: &str, reason: &str, action: &str) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: false,
        reason: Some(reason.to_owned()),
        emphasis: Emphasis::Quiet,
        confirm: None,
        op: Op::Rust {
            action: action.to_owned(),
            payload: json!({}),
        },
    }
}

/// The whole seconds from `last_seen_ms` to `now_ms`, never negative.
fn seconds_since(now_ms: u64, last_seen_ms: u64) -> u64 {
    now_ms.saturating_sub(last_seen_ms) / 1000
}

/// The trimmed, non-empty text at `key` of `value`.
pub(super) fn text(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned)
}
