//! The view projection (design §2): a pure function from one gather and the extension's facts to
//! the whole panel, `DevicesView`. It reads no file, socket or clock; the time is passed in.
//!
//! - `types`: the contract.
//! - `health`: the health rules, the rollup and the product words of the states.
//! - `computers` and `runtimes`: the frames and the cards, with their inspectors; `lifecycle`:
//!   a runtime's actions; `list`: the list's columns and rows.
//! - `status`, `address` and `chips`: what a `status` answer says, how addresses are shown, and
//!   which platform names a chip.
//!
//! - `connections`: the connection pills and their inspectors; `devices`: the devices and
//!   systems at their other ends, the links and the mesh bus; `search`: the protocol filter and
//!   the search; `issues`: the issues list, the banner and the summary strip.
//!
//! The add dialog, the forms it and the inspectors open, and the USB tray are built in
//! [the `panes` namespace](crate::panes), which answers the pane types of `types`.

pub(crate) mod address;
pub(crate) mod chips;
mod computers;
mod connections;
mod devices;
pub mod health;
mod issues;
pub(crate) mod lifecycle;
mod list;
pub(crate) mod runtimes;
mod search;
pub(crate) mod status;
pub mod types;

use std::path::Path;

use crate::gather::{Gathered, ProjectInputs};
use crate::inputs;
use crate::request::ViewRequest;
use crate::texts::graph as texts;
use crate::topology::{self, Topology};
use types::{
    Action, AddDialog, AddGroup, DevicesView, Emphasis, Op, Pane, ProjectBar, ProjectChoice,
    Toolbar, UiOp, UsbTray, ViewSwitch, SCHEMA_VERSION,
};

/// The whole panel for one gather (design §2.1). `this_computer` is the host ID that this
/// computer's runtimes report, when the caller knows it; `now_ms` is the time of the view, in
/// milliseconds since the Unix epoch.
#[must_use]
pub fn project(
    gathered: &Gathered,
    request: &ViewRequest,
    this_computer: Option<&str>,
    now_ms: u64,
) -> DevicesView {
    let inputs = inputs::read(gathered, &request.host, this_computer);
    let mut topology = topology::merge(&inputs.topology);
    topology.issues.extend(inputs.issues);
    render(&topology, &inputs.facts, gathered, request, now_ms)
}

fn render(
    topology: &Topology,
    facts: &inputs::Facts,
    gathered: &Gathered,
    request: &ViewRequest,
    now_ms: u64,
) -> DevicesView {
    let mut computers = computers::computers(topology, facts, now_ms);
    connections::fill(
        &mut computers,
        topology,
        &connections::Context::new(facts, gathered, &request.host.io_state, now_ms),
    );
    let mut graph = devices::graph(topology, &computers, now_ms);
    let issues = issues::issues(topology, facts, &computers, &graph.devices);
    let filter = search::apply_filter(
        topology,
        &mut computers,
        &mut graph,
        &issues,
        &request.view.hidden_protocols,
    );
    let search = search::apply_search(&mut computers, &mut graph, &issues, &request.view.search);
    let running = topology
        .runtimes()
        .filter(|runtime| runtimes::runs(runtime, facts))
        .count();
    let [add, discover, add_host_slot] = lifecycle::toolbar_actions();
    DevicesView {
        schema_version: SCHEMA_VERSION,
        generated_ms: now_ms,
        toolbar: Toolbar {
            add,
            discover,
            add_host_slot,
            view_switch: ViewSwitch {
                topology: texts::VIEW_TOPOLOGY.to_owned(),
                list: texts::VIEW_LIST.to_owned(),
            },
        },
        project: project_bar(&gathered.project),
        summary: issues::summary(&computers, running, &issues),
        banner: issues::banner(topology, facts),
        issues,
        filter,
        search,
        networks: computers::networks(topology, &computers),
        computers,
        devices: graph.devices,
        links: graph.links,
        mesh: graph.mesh,
        usb: UsbTray {
            heading: String::new(),
            items: Vec::new(),
            empty_text: None,
        },
        add: crate::panes::add::add_dialog(topology, facts, gathered, request),
        list: list::list_view(),
        notices: Vec::new(),
    }
}

fn open(pane: Pane) -> Op {
    Op::Ui {
        ui: UiOp::Open { pane },
    }
}

/// The add dialog before its catalog is read: its title and its search entry.
pub(crate) fn empty_add_dialog() -> AddDialog {
    AddDialog {
        title: texts::ADD_DIALOG_TITLE.to_owned(),
        target: None,
        runtime_choices: Vec::new(),
        discover: Action {
            id: "discover".to_owned(),
            label: texts::DISCOVER_DEVICES_AND_RUNTIMES.to_owned(),
            enabled: true,
            reason: None,
            emphasis: Emphasis::Secondary,
            confirm: None,
            op: open(Pane::Discover),
        },
        discover_help: texts::DISCOVER_HELP.to_owned(),
        found: Vec::new(),
        runtimes: AddGroup {
            label: texts::RUNTIMES_GROUP.to_owned(),
            advanced: false,
            entries: Vec::new(),
        },
        groups: Vec::new(),
        advanced_toggle: None,
        quick_starts: Vec::new(),
        catalog_state: None,
    }
}

/// The project name and the projects of the workspace (specification 25 §5.11).
fn project_bar(project: &ProjectInputs) -> ProjectBar {
    let name = |path: &Path| {
        path.file_name().map_or_else(
            || path.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        )
    };
    let Some(chosen) = &project.chosen else {
        return ProjectBar {
            name: texts::NO_PROJECT.to_owned(),
            root: String::new(),
            choices: Vec::new(),
        };
    };
    ProjectBar {
        name: name(chosen),
        root: chosen.display().to_string(),
        choices: project
            .projects
            .iter()
            .map(|path| ProjectChoice {
                name: name(path),
                root: path.display().to_string(),
                selected: path == chosen,
            })
            .collect(),
    }
}

#[cfg(test)]
pub(crate) mod tests;
