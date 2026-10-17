//! The issues, the banner and the summary strip (specification 25 §5.10 and §5.11).
//!
//! - Every fault and warning appears once in the issues list, grouped by its item, each with
//!   `Show` and its recovery action where one exists. A connection or a device on a fieldbus whose
//!   health word is `error` is a fault and one whose word is `degraded` a warning; a runtime's
//!   own failure (a failed start, a refused token, a status that cannot be read, a faulted
//!   program, or an error the runtime reports itself) is a fault; and a report that cannot be
//!   read is a fault of the item it names, or of no item. A running runtime whose connection
//!   fails raises nothing of its own: the connection's issue is the one.
//! - The banner keeps the simulator's failed start with `Retry` and `Open logs`, the reports that
//!   cannot be read, and, while the panel draws the simulator for the project, `Start simulator`
//!   or `Stop simulator`.
//! - The summary strip counts the computers, the runtimes, the connections the filter shows, the
//!   runtimes that run, and the faults and warnings of the issues list.

use std::collections::BTreeMap;

use super::runtimes;
use super::types::{
    Action, Banner, Computer, Device, DeviceKind, Emphasis, HostEffect, Issue, IssueGroup, Issues,
    IssuesButton, ItemId, Op, Pane, Runtime, RuntimeIcon, Severity, Summary, Tone, UiOp,
};
use crate::inputs::Facts;
use crate::texts::graph;
use crate::texts::issues as texts;
use crate::topology::{self, RuntimeOrigin, SimulatorState, Topology, TopologyIssue};

/// One issue with the item it belongs to.
struct Raised {
    item: Option<ItemId>,
    name: String,
    issue: Issue,
}

/// The severity that a health word raises: a fault for `error`, a warning for `degraded`.
fn severity_of(health: &str) -> Option<Severity> {
    match health.trim().to_ascii_lowercase().as_str() {
        "error" => Some(Severity::Error),
        "degraded" => Some(Severity::Warning),
        _ => None,
    }
}

/// The word and tone of an issue's severity (specification 25 §5.11): `Fault` for a fault,
/// `Warning` for a warning, `Info` for information.
fn severity_label(severity: Severity) -> (&'static str, Tone) {
    match severity {
        Severity::Error => (graph::FAULT, Tone::Error),
        Severity::Warning => (texts::WARNING, Tone::Warning),
        Severity::Info => (texts::INFO, Tone::Neutral),
    }
}

fn action(id: &str, label: &str, emphasis: Emphasis, op: Op) -> Action {
    Action {
        id: id.to_owned(),
        label: label.to_owned(),
        enabled: true,
        reason: None,
        emphasis,
        confirm: None,
        op,
    }
}

fn host(effect: HostEffect) -> Op {
    Op::Host { effect }
}

fn show(item: &ItemId) -> Action {
    action(
        "show",
        texts::SHOW,
        Emphasis::Secondary,
        Op::Ui {
            ui: UiOp::Focus { item: item.clone() },
        },
    )
}

fn retry() -> Action {
    action(
        "retry",
        texts::RETRY,
        Emphasis::Primary,
        host(HostEffect::StartSimulator),
    )
}

/// An issue of `item`, with `Show`.
fn raised(
    item: Option<&ItemId>,
    name: &str,
    id: String,
    severity: Severity,
    text: String,
    recovery: Option<Action>,
) -> Raised {
    let (word, tone) = severity_label(severity);
    Raised {
        item: item.cloned(),
        name: name.to_owned(),
        issue: Issue {
            id,
            severity,
            severity_word: word.to_owned(),
            severity_tone: tone,
            text,
            show: item.map(show),
            recovery,
        },
    }
}

/// What the runtimes' reports and the project's files raise, by the item they name.
struct Reports<'a> {
    by_item: BTreeMap<&'a str, Vec<(usize, &'a TopologyIssue)>>,
}

impl<'a> Reports<'a> {
    fn new(issues: &'a [TopologyIssue]) -> Self {
        let mut by_item: BTreeMap<&str, Vec<(usize, &TopologyIssue)>> = BTreeMap::new();
        for (index, issue) in issues.iter().enumerate() {
            if let Some(item) = issue.item.as_deref() {
                by_item.entry(item).or_default().push((index, issue));
            }
        }
        Self { by_item }
    }

    /// The report issues of `item`, which it then no longer holds.
    fn take(&mut self, item: &ItemId, name: &str, into: &mut Vec<Raised>) {
        for (index, issue) in self.by_item.remove(item.as_str()).unwrap_or_default() {
            into.push(raised(
                Some(item),
                name,
                format!("report:{index}"),
                Severity::Error,
                issue.text.clone(),
                None,
            ));
        }
    }
}

/// Every fault and warning, grouped by item in the order of the canvas; the issues of no item
/// shown come first (specification 25 §5.11).
pub(crate) fn issues(
    topology: &Topology,
    facts: &Facts,
    computers: &[Computer],
    devices: &[Device],
) -> Issues {
    let runtimes: BTreeMap<&str, &topology::Runtime> = topology
        .runtimes()
        .map(|runtime| (runtime.id.as_str(), runtime))
        .collect();
    let mut reports = Reports::new(&topology.issues);
    let mut raised_issues = Vec::new();
    for card in computers.iter().flat_map(|computer| &computer.runtimes) {
        reports.take(&card.id, &card.name, &mut raised_issues);
        if let Some(runtime) = runtimes.get(card.id.as_str()) {
            raised_issues.extend(runtime_fault(runtime, card, facts));
        }
        for connection in &card.connections {
            reports.take(&connection.id, &connection.name, &mut raised_issues);
            let endpoint = runtimes.get(card.id.as_str()).and_then(|runtime| {
                runtime
                    .endpoints
                    .iter()
                    .find(|endpoint| endpoint.id == connection.id.as_str())
            });
            if let Some(endpoint) = endpoint {
                raised_issues.extend(item_issue(
                    &connection.id,
                    &connection.name,
                    "fleet",
                    &endpoint.health,
                    &endpoint.detail,
                ));
                raised_issues.extend(subdevice_issues(endpoint, devices));
            }
        }
    }
    for device in devices
        .iter()
        .filter(|device| device.kind != DeviceKind::SubDevice)
    {
        reports.take(&device.id, &device.name, &mut raised_issues);
    }
    let mut general = general_issues(topology, facts, computers);
    general.extend(
        reports
            .by_item
            .into_values()
            .flatten()
            .map(|(index, issue)| report_issue(index, issue)),
    );
    general.extend(raised_issues);
    grouped(general)
}

/// The issue of a connection or of a device on a fieldbus: a fault for `error`, a warning for
/// `degraded`, with its name and detail.
fn item_issue(item: &ItemId, name: &str, kind: &str, health: &str, detail: &str) -> Option<Raised> {
    let severity = severity_of(health)?;
    let detail = detail.trim();
    let text = if detail.is_empty() {
        graph::health_word(health)
    } else {
        detail.to_owned()
    };
    Some(raised(
        Some(item),
        name,
        format!("{kind}:{}", item.as_str()),
        severity,
        texts::item_issue(name, &text),
        None,
    ))
}

/// The issues of the devices on the bus of `endpoint`.
fn subdevice_issues(endpoint: &topology::Endpoint, devices: &[Device]) -> Vec<Raised> {
    endpoint
        .children
        .iter()
        .filter_map(|child| {
            let id = super::devices::subdevice_id(&endpoint.id, &child.id);
            let device = devices.iter().find(|device| device.id == id)?;
            item_issue(
                &device.id,
                &device.name,
                "device",
                &child.health,
                &child.detail,
            )
        })
        .collect()
}

/// A runtime's own failure (specification 25 §5.10): its card shows the error tone and that tone
/// does not only roll up its connections. Its text is the runtime's own detail when the runtime
/// reports the error, else its state; after a failed start the simulator offers `Retry`, and
/// after a refused token the runtime offers its credential recovery.
fn runtime_fault(runtime: &topology::Runtime, card: &Runtime, facts: &Facts) -> Option<Raised> {
    let own_error = runtime.health.trim().eq_ignore_ascii_case("error");
    if card.state.tone != Tone::Error || (card.state.word == graph::ERROR && !own_error) {
        return None;
    }
    let detail = runtimes::clean_detail(&runtime.detail);
    let text = if own_error && !detail.is_empty() {
        detail
    } else {
        card.state.word.clone()
    };
    let recovery = card
        .inspector
        .primary
        .iter()
        .find(|action| action.id == "set_auth_token" && action.enabled)
        .cloned()
        .or_else(|| (facts.simulator_failed && card.icon == RuntimeIcon::Simulator).then(retry));
    Some(raised(
        Some(&card.id),
        &card.name,
        format!("runtime:{}", card.id.as_str()),
        Severity::Error,
        texts::item_issue(&card.name, &text),
        recovery,
    ))
}

/// The issue of a report that names no item that is shown.
fn report_issue(index: usize, issue: &TopologyIssue) -> Raised {
    raised(
        None,
        texts::GENERAL,
        format!("report:{index}"),
        Severity::Error,
        issue.text.clone(),
        None,
    )
}

/// The issues of no item: the reports that name none, and the simulator's failed start while no
/// card shows the simulator.
fn general_issues(topology: &Topology, facts: &Facts, computers: &[Computer]) -> Vec<Raised> {
    let mut general: Vec<Raised> = topology
        .issues
        .iter()
        .enumerate()
        .filter(|(_, issue)| issue.item.is_none())
        .map(|(index, issue)| report_issue(index, issue))
        .collect();
    let simulator_card = computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .any(|card| card.icon == RuntimeIcon::Simulator);
    if let Some(failure) = facts.simulator_failure.as_deref()
        && !simulator_card
    {
        general.push(raised(
            None,
            texts::GENERAL,
            "runtime:simulator".to_owned(),
            Severity::Error,
            texts::item_issue(texts::SIMULATOR, failure.trim()),
            Some(retry()),
        ));
    }
    general
}

/// The issues grouped by item, each group where its first issue is.
fn grouped(raised_issues: Vec<Raised>) -> Issues {
    let faults = raised_issues
        .iter()
        .filter(|raised| raised.issue.severity == Severity::Error)
        .count();
    let warnings = raised_issues
        .iter()
        .filter(|raised| raised.issue.severity == Severity::Warning)
        .count();
    let mut groups: Vec<IssueGroup> = Vec::new();
    for Raised { item, name, issue } in raised_issues {
        match groups.iter_mut().find(|group| group.item == item) {
            Some(group) => group.issues.push(issue),
            None => groups.push(IssueGroup {
                item,
                item_name: name,
                issues: vec![issue],
            }),
        }
    }
    let count = faults + warnings;
    Issues {
        button: IssuesButton {
            text: if count == 0 {
                texts::NO_ISSUES.to_owned()
            } else {
                texts::issue_count(count)
            },
            tone: if faults > 0 {
                Tone::Error
            } else if warnings > 0 {
                Tone::Warning
            } else {
                Tone::Neutral
            },
            faults: count_u32(faults),
            warnings: count_u32(warnings),
        },
        groups,
    }
}

fn count_u32(count: usize) -> u32 {
    u32::try_from(count).unwrap_or(u32::MAX)
}

/// The summary strip (specification 25 §5.11): the computers, the runtimes, the connections that
/// the filter shows, the runtimes that run, and the faults and warnings of the issues list.
pub(crate) fn summary(computers: &[Computer], running: usize, issues: &Issues) -> Summary {
    let runtimes: Vec<&Runtime> = computers
        .iter()
        .flat_map(|computer| &computer.runtimes)
        .collect();
    let connections = runtimes
        .iter()
        .flat_map(|runtime| &runtime.connections)
        .filter(|connection| !connection.hidden)
        .count();
    let faults = issues.button.faults;
    let warnings = issues.button.warnings;
    let widen = |count: u32| usize::try_from(count).unwrap_or(usize::MAX);
    Summary {
        text: texts::summary(
            computers.len(),
            runtimes.len(),
            connections,
            running,
            widen(faults),
            widen(warnings),
        ),
        computers: count_u32(computers.len()),
        runtimes: count_u32(runtimes.len()),
        connections: count_u32(connections),
        running: count_u32(running),
        faults,
        warnings,
        open_issues: action(
            "open_issues",
            graph::OPEN_ISSUES,
            Emphasis::Quiet,
            Op::Ui {
                ui: UiOp::Open { pane: Pane::Issues },
            },
        ),
    }
}

/// The banner (specification 25 §5.10 and §5.16): the simulator's failed start with `Retry` and
/// `Open logs`; else the reports that cannot be read; else, while the panel draws the simulator
/// for the project, whether it is stopped or runs, with `Start simulator` or `Stop simulator`.
pub(crate) fn banner(topology: &Topology, facts: &Facts) -> Option<Banner> {
    if let Some(failure) = facts.simulator_failure.as_deref() {
        return Some(Banner {
            tone: Tone::Error,
            text: failure.trim().to_owned(),
            actions: vec![
                retry(),
                action(
                    "open_logs",
                    texts::OPEN_LOGS,
                    Emphasis::Secondary,
                    host(HostEffect::OpenDebugLog),
                ),
            ],
        });
    }
    let unread = |project: bool| {
        topology
            .issues
            .iter()
            .filter(|issue| issue.item.is_none() && issue.project == project)
            .map(|issue| issue.text.as_str())
            .collect::<Vec<_>>()
            .join("; ")
    };
    let peers = unread(false);
    let problems: Vec<String> = [
        unread(true),
        if peers.is_empty() {
            peers
        } else {
            texts::peer_topology_degraded(&peers)
        },
    ]
    .into_iter()
    .filter(|text| !text.is_empty())
    .collect();
    if !problems.is_empty() {
        return Some(Banner {
            tone: Tone::Error,
            text: problems.join("; "),
            actions: Vec::new(),
        });
    }
    let drawn = topology
        .runtimes()
        .any(|runtime| runtime.origin == RuntimeOrigin::SimulatorFallback);
    match facts.simulator {
        SimulatorState::NotStarted if drawn => Some(Banner {
            tone: Tone::Neutral,
            text: texts::SIMULATOR_IS_STOPPED.to_owned(),
            actions: vec![action(
                "start_simulator",
                texts::START_SIMULATOR,
                Emphasis::Primary,
                host(HostEffect::StartSimulator),
            )],
        }),
        SimulatorState::Running if drawn => Some(Banner {
            tone: Tone::Neutral,
            text: texts::SIMULATOR_RUNNING.to_owned(),
            actions: vec![action(
                "stop_simulator",
                texts::STOP_SIMULATOR,
                Emphasis::Secondary,
                host(HostEffect::StopSession),
            )],
        }),
        _ => None,
    }
}
