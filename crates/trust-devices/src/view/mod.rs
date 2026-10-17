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
//! The connections, devices, links, mesh buses, search, filter, issues and summary have their
//! own modules; until they exist, their parts of the view are empty.

mod address;
mod chips;
mod computers;
pub mod health;
mod lifecycle;
mod list;
mod runtimes;
pub(crate) mod status;
pub mod types;

use std::path::Path;

use crate::gather::{Gathered, ProjectInputs};
use crate::inputs;
use crate::request::ViewRequest;
use crate::texts::graph as texts;
use crate::topology::{self, Topology};
use types::{
    Action, AddDialog, AddGroup, DevicesView, Emphasis, FilterReport, Issues, IssuesButton, Op,
    Pane, ProjectBar, ProjectChoice, SearchEcho, Summary, Tone, Toolbar, UiOp, UsbTray,
    SCHEMA_VERSION,
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
    let computers = computers::computers(topology, facts, now_ms);
    let [add, discover, add_host_slot] = lifecycle::toolbar_actions();
    DevicesView {
        schema_version: SCHEMA_VERSION,
        generated_ms: now_ms,
        toolbar: Toolbar {
            add,
            discover,
            add_host_slot,
        },
        project: project_bar(&gathered.project),
        summary: empty_summary(),
        banner: None,
        issues: Issues {
            button: IssuesButton {
                text: String::new(),
                tone: Tone::Neutral,
                faults: 0,
                warnings: 0,
            },
            groups: Vec::new(),
        },
        filter: FilterReport {
            protocols: Vec::new(),
            lines: Vec::new(),
            hidden_connections: 0,
            hidden_attention: 0,
            hidden_errors: 0,
            hidden_warnings: 0,
        },
        search: SearchEcho {
            query: request.view.search.clone(),
            placeholder: texts::SEARCH_PLACEHOLDER.to_owned(),
            matches: 0,
        },
        networks: computers::networks(topology, &computers),
        computers,
        devices: Vec::new(),
        links: Vec::new(),
        mesh: Vec::new(),
        usb: UsbTray {
            heading: String::new(),
            items: Vec::new(),
            empty_text: None,
        },
        add: empty_add_dialog(),
        list: list::list_view(),
        notices: Vec::new(),
    }
}

fn open(pane: Pane) -> Op {
    Op::Ui {
        ui: UiOp::Open { pane },
    }
}

/// The summary strip before its counts are made.
fn empty_summary() -> Summary {
    Summary {
        text: String::new(),
        computers: 0,
        runtimes: 0,
        connections: 0,
        running: 0,
        faults: 0,
        warnings: 0,
        open_issues: Action {
            id: "open_issues".to_owned(),
            label: texts::OPEN_ISSUES.to_owned(),
            enabled: true,
            reason: None,
            emphasis: Emphasis::Quiet,
            confirm: None,
            op: open(Pane::Issues),
        },
    }
}

/// The add dialog before its catalog is read: its title and its search entry.
fn empty_add_dialog() -> AddDialog {
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
mod tests;
