//! The engine's methods (specification 35, §0.1a). Each answers `{"result": …}` or
//! `{"failure": …}`; a failure carries its cause and next action (A11). Each stage adds its
//! methods: `controlStudio/identify` (V1), `controlStudio/tune` and `controlStudio/preview` (V2), `controlStudio/validate`
//! (V3), `controlStudio/apply`, `controlStudio/restore` and `controlStudio/keep` (V4),
//! `controlStudio/setupDiscover` and `controlStudio/setupSave` (V5).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use trust_control::chart_export::{self, ChartExportRequest};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::messages::{
    OpenRequest, OpenResult, OperationRequest, OperationState, PageRequest, ParameterWorkRequest,
    PreviewRequest,
};
use trust_control::plant_monitor::PlantMonitorRequest;
use trust_control::setup::messages::{
    ProjectLoopsRequest, SetupDiscoverRequest, SetupSaveRequest, SetupStepRequest,
};
use trust_control::simulator_start::{self, SimulatorStartOutcome, SimulatorStartRequest};

use super::analysis_parameters_action;
use super::analyze_action;
use super::apply_action;
use super::busy_loops::LoopClaims;
use super::export_action;
use super::identify_action;
use super::inputs_action;
use super::keep_action;
use super::monitor_action;
use super::open_action;
use super::operation::{Operations, UpdateSink};
use super::plant_monitor_action;
use super::preview_action;
use super::project_loops_action;
use super::run_action;
use super::runtime_client::runtime_recording::PendingReleases;
use super::settings::{self, Environment, Platform};
use super::setup_action;
use super::setup_step_action;
use super::tune_action;
use super::validate_action;
use super::{loops, storage};

/// Opens a project loop's study, or lists the project's loops.
pub const OPEN: &str = "controlStudio/open";
/// The latest update of an operation, for a panel that reattaches.
pub const STATUS: &str = "controlStudio/status";
/// Cancels an operation at its next step.
pub const CANCEL: &str = "controlStudio/cancel";
/// Identifies the plant (B3): a live campaign, a refit of the study's recording, the journaled campaign followed to its end,
/// or a new working study (A8.4).
pub const IDENTIFY: &str = "controlStudio/identify";
/// Records the page the engineer last viewed (A8.1): `controlStudio/open` returns it, so reopening shows it (A8.6). It changes
/// nothing else and starts nothing.
pub const PAGE: &str = "controlStudio/page";
/// Designs the candidate for the loop's controller from the study's model (B4): an operation, answered at once with its
/// identifier, whose final update carries the candidate, its predicted response and its margins. It never reaches the runtime
/// and never writes to the controller (B4.1).
pub const TUNE: &str = "controlStudio/tune";
/// Answers the predicted response, margins and outcome of controller settings entered on the study's model, as `tune` with the
/// manual method would finish, and stores nothing (B4.5). It answers at once and starts no operation.
pub const PREVIEW: &str = "controlStudio/preview";
/// Answers the analysis tools of the Tune page (B9.9, A7, B10.2).
pub const ANALYZE: &str = "controlStudio/analyze";
/// The parameter writes that a loop's declared analysis maps (C12, annex E7): the review of the values, and the confirmed Apply or
/// Restore of the reviewed record as an operation. Each one is refused for a loop that the analysis does not serve.
pub const PARAMETERS: &str = "controlStudio/parameters";
/// Validates the candidate (B5): an operation, answered at once with its identifier. It runs the independent model check when
/// the model is provisional (B5.2), then the actual-ST validation of the current and the candidate settings in isolated
/// simulations (B5.3). Its final update carries the Validate result. It never writes to the controller.
pub const VALIDATE: &str = "controlStudio/validate";
/// Applies the validated candidate (B6.1 to B6.3): without a review it answers the review at once and writes nothing; with the
/// review's identity it starts the Apply operation, whose result follows through `controlStudio/update`.
pub const APPLY: &str = "controlStudio/apply";
/// Restores the settings from before Apply (B6.4), by the same review and confirmation as `apply`.
pub const RESTORE: &str = "controlStudio/restore";
/// Keeps the running values as the project's initial values (B6.5), by the same review and confirmation as `apply`.
pub const KEEP: &str = "controlStudio/keep";
/// Loop setup (B1.2, B1.3): the compiled project's setup form and its configured loops, checked against the compiled catalog.
pub const SETUP_DISCOVER: &str = "controlStudio/setupDiscover";
/// Loop setup (B1.3, §0.1b): writes a confirmed draft as loop files, all or none, never over an existing one.
pub const SETUP_SAVE: &str = "controlStudio/setupSave";
/// The step-by-step loop setup (B1.3, C12): the question the engine asks next, the words of a refused answer, or the finished draft and the
/// sentence the engineer confirms before the save. The engine keeps the session's answers; the page keeps the session it returns.
pub const SETUP_STEP: &str = "controlStudio/setupStep";
/// The loop files of the workspace folders (A8.3, B1.2, B1.3): each file's decision and open request, the folder that declares a loop,
/// and the loops of an equipment. The extension forwards the answer; it decides none of them.
pub const PROJECT_LOOPS: &str = "controlStudio/projectLoops";
/// Records the study's inputs (A8.1, A8.4; B4.2, B4.3): the acquisition settings, the requirements, the tuning method, its response
/// and structure, the pins and the manual settings. It answers at once with the stages and what the change invalidated.
pub const INPUTS: &str = "controlStudio/inputs";
/// The automatic progression (B8): runs, from the earliest stage whose result is missing or no longer current, the stages up to the one
/// it names (Identify, Tune or Validate), as one operation. It never applies.
pub const RUN: &str = "controlStudio/run";
/// The loop's results for the engineer to save (B10.5): the workflow result as JSON, or the signal table of a recording as CSV. The
/// engine writes the text; the extension asks where to save it. It writes nothing and starts nothing.
pub const EXPORT: &str = "controlStudio/export";
/// The loop's sample for the page (B2, A3.4) and, when it changed, the target's label (B9.2): one coherent scan of the open loop, or
/// the reason it is not current. The engine decides availability and staleness; it reads the runtime and writes nothing.
pub const MONITOR: &str = "controlStudio/monitor";
/// The plant summary of a plant handoff (B1.2, specification 47, §20.4): the runtime's `plant.status`, decided by the engine. It reads
/// the runtime and writes nothing.
pub const PLANT_MONITOR: &str = "controlStudio/plantMonitor";
/// Decides a Start simulator request (A2.8): the extension starts the local simulator, joins a start in flight, or shows the reply
/// the engine gives. The engine starts nothing itself.
pub const SIMULATOR_START: &str = "controlStudio/simulatorStart";
/// Reports the outcome of the simulator start the extension made (A2.8) and answers the reply the page shows.
pub const SIMULATOR_START_RESULT: &str = "controlStudio/simulatorStartResult";
/// The chart image export (B10.5): the file name, the save filter and the bytes of the page's chart. The extension shows the save
/// dialog and writes the bytes; the engine writes nothing.
pub const CHART_EXPORT: &str = "controlStudio/chartExport";
/// The methods this module answers.
pub const METHODS: [&str; 25] = [
    OPEN,
    STATUS,
    CANCEL,
    IDENTIFY,
    PAGE,
    INPUTS,
    TUNE,
    PREVIEW,
    VALIDATE,
    RUN,
    APPLY,
    RESTORE,
    KEEP,
    SETUP_DISCOVER,
    SETUP_SAVE,
    SETUP_STEP,
    PROJECT_LOOPS,
    ANALYZE,
    PARAMETERS,
    EXPORT,
    MONITOR,
    PLANT_MONITOR,
    SIMULATOR_START,
    SIMULATOR_START_RESULT,
    CHART_EXPORT,
];
/// The notification that carries operation updates.
pub const UPDATE_NOTIFICATION: &str = "controlStudio/update";

/// Control Studio's engine: the methods of one engine process.
pub struct ControlStudio {
    operations: Operations,
    /// The runtime recordings that are stopped but not released, shared by every operation of this engine (E4.7).
    pending: Arc<PendingReleases>,
    /// The loops this engine's operations are working on: one operation per loop at a time (A8.5).
    claims: LoopClaims,
    /// The operations this engine started for each loop, by project folder and loop identifier, so that `controlStudio/open`
    /// names the ones that still run (A8.6). An operation that has ended is dropped from it when the loop is opened.
    started: Mutex<BTreeMap<(String, String), Vec<String>>>,
    /// The observation of the loop the panel shows, which `controlStudio/monitor` keeps (B2): the sample's scan state and the label
    /// the page was last given.
    monitor: Mutex<Option<monitor_action::MonitorSession>>,
}

impl ControlStudio {
    /// Control Studio, reporting operation updates through `sink`.
    pub fn new(sink: Arc<dyn UpdateSink>) -> Self {
        Self {
            operations: Operations::new(sink),
            pending: Arc::new(PendingReleases::default()),
            claims: LoopClaims::default(),
            started: Mutex::new(BTreeMap::new()),
            monitor: Mutex::new(None),
        }
    }

    /// The operations, for the stages that start them.
    #[must_use]
    pub fn operations(&self) -> &Operations {
        &self.operations
    }

    /// The runtime recordings this engine stopped and has not released yet, for a verb that starts its own operation, such
    /// as the command line's `cancel`.
    #[must_use]
    pub fn pending_releases(&self) -> &Arc<PendingReleases> {
        &self.pending
    }

    /// The loops this engine's operations are working on, for a verb that starts its own operation, such as the command
    /// line's `cancel`.
    #[must_use]
    pub fn loop_claims(&self) -> &LoopClaims {
        &self.claims
    }

    /// Runs the method `command` with its `arguments`; `None` when the method is not Control Studio's.
    #[must_use]
    pub fn execute(&self, command: &str, arguments: &[Value]) -> Option<Value> {
        let answer = match command {
            OPEN => argument(arguments)
                .and_then(|request: OpenRequest| self.open_loop(&request))
                .and_then(|result| to_value(&result)),
            STATUS => argument(arguments)
                .and_then(|request: OperationRequest| self.operations.status(&request.operation_id))
                .and_then(|update| to_value(&update)),
            CANCEL => argument(arguments)
                .and_then(|request: OperationRequest| self.operations.cancel(&request.operation_id))
                .and_then(|update| to_value(&update)),
            IDENTIFY => argument(arguments).and_then(|request| {
                identify_action::answer(&self.operations, &self.pending, &self.claims, request)
            }),
            PAGE => read_argument(
                arguments,
                trust_control::analysis::entries::read_page_entries,
            )
            .and_then(|request: PageRequest| set_page(&request)),
            INPUTS => {
                argument(arguments).and_then(|request| inputs_action::answer(&self.claims, request))
            }
            RUN => argument(arguments).and_then(|request| {
                run_action::answer(&self.operations, &self.pending, &self.claims, request)
            }),
            TUNE => argument(arguments)
                .and_then(|request| tune_action::answer(&self.operations, &self.claims, request)),
            PREVIEW => argument(arguments)
                .and_then(|request: PreviewRequest| preview_action::answer(&request)),
            ANALYZE => read_argument(arguments, trust_control::analysis::entries::read_entries)
                .and_then(|request: trust_control::messages::AnalyzeRequest| {
                    analyze_action::answer(&self.operations, &self.claims, &request)
                }),
            PARAMETERS => argument(arguments).and_then(|request: ParameterWorkRequest| {
                analysis_parameters_action::answer(&self.operations, &self.claims, request)
            }),
            VALIDATE => argument(arguments).and_then(|request| {
                validate_action::answer(&self.operations, &self.pending, &self.claims, request)
            }),
            APPLY => argument(arguments)
                .and_then(|request| apply_action::answer(&self.operations, &self.claims, request)),
            RESTORE => argument(arguments).and_then(|request| {
                apply_action::answer_restore(&self.operations, &self.claims, request)
            }),
            KEEP => argument(arguments)
                .and_then(|request| keep_action::answer(&self.operations, &self.claims, request)),
            MONITOR => monitor_action::answer_command(&self.monitor, arguments),
            PLANT_MONITOR => argument(arguments).and_then(|request: PlantMonitorRequest| {
                to_value(&plant_monitor_action::answer(&request))
            }),
            SIMULATOR_START => argument(arguments).and_then(|request: SimulatorStartRequest| {
                to_value(&simulator_start::decide(&request))
            }),
            SIMULATOR_START_RESULT => {
                argument(arguments).and_then(|outcome: SimulatorStartOutcome| {
                    to_value(&simulator_start::outcome(&outcome))
                })
            }
            CHART_EXPORT => argument(arguments).and_then(|request: ChartExportRequest| {
                chart_export::export(&request).and_then(|answer| to_value(&answer))
            }),
            SETUP_DISCOVER => argument(arguments).and_then(|request: SetupDiscoverRequest| {
                setup_action::answer_discover(&request, &self.claims)
            }),
            SETUP_SAVE => argument(arguments)
                .and_then(|request: SetupSaveRequest| setup_action::answer_save(&request)),
            SETUP_STEP => argument(arguments)
                .and_then(|request: SetupStepRequest| setup_step_action::answer(&request)),
            PROJECT_LOOPS => argument(arguments)
                .and_then(|request: ProjectLoopsRequest| project_loops_action::answer(&request)),
            EXPORT => argument(arguments).and_then(export_action::answer),
            _ => return None,
        };
        if let Ok(value) = &answer {
            self.remember_started(arguments, value);
        }
        Some(match answer {
            Ok(result) => json!({ "result": result }),
            Err(failure) => json!({ "failure": failure }),
        })
    }

    /// `controlStudio/open` (A8.6): the loop's answer, with the operations this engine runs for it, which the page follows.
    fn open_loop(&self, request: &OpenRequest) -> Result<OpenResult, Failure> {
        let running = request.loop_id.as_deref().map_or_else(Vec::new, |loop_id| {
            self.running_operations(&request.project_root, loop_id)
        });
        open_action::open_with_running(request, running)
    }

    /// The operations this engine started for the loop that still run, in the order they started (A8.6). One that has ended is
    /// dropped: its result is in the study, which `open` answers as the stages and the validation.
    fn running_operations(&self, project_root: &str, loop_id: &str) -> Vec<String> {
        let mut started = self.started.lock();
        let Some(operations) = started.get_mut(&(project_root.to_owned(), loop_id.to_owned()))
        else {
            return Vec::new();
        };
        operations.retain(|id| {
            self.operations
                .status(id)
                .is_ok_and(|update| update.state == OperationState::Running)
        });
        operations.clone()
    }

    /// Records the operation that a method answered with an `operationId` started for the loop its arguments name, so that
    /// `open` can name it while it runs. A method whose arguments name no loop starts nothing that `open` reports.
    fn remember_started(&self, arguments: &[Value], answer: &Value) {
        let Some(operation) = answer.get("operationId").and_then(Value::as_str) else {
            return;
        };
        let Some((project_root, loop_id)) = loop_of(arguments) else {
            return;
        };
        self.started
            .lock()
            .entry((project_root, loop_id))
            .or_default()
            .push(operation.to_owned());
    }
}

/// The project folder and the loop that a method's arguments name, when both are given.
fn loop_of(arguments: &[Value]) -> Option<(String, String)> {
    let request = arguments.first()?;
    Some((
        request.get("projectRoot")?.as_str()?.to_owned(),
        request.get("loopId")?.as_str()?.to_owned(),
    ))
}

/// Records the page the engineer last viewed in the loop's study (A8.1), so that `open` returns it (A8.6). The study is read as
/// `open` reads it, so a study of an earlier loop revision starts again at Identify (A8.2). Nothing else of the study changes,
/// and no test, tuning, validation or write starts.
///
/// # Errors
/// The loop cannot be opened, or its study cannot be read or written.
fn set_page(request: &PageRequest) -> Result<Value, Failure> {
    let project_root = Path::new(&request.project_root);
    let loop_definition = loops::open(project_root, &request.runtime_id, &request.loop_id)?;
    let mut study =
        storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    study.page = request.page;
    // The analysis the page names is the one it shows. The engine recorded the model revision it calculated that tool for (B9.9), and the
    // page's record names none, so the revision stays for the same tool.
    let calculated_for = request.analysis.as_ref().and_then(|shown| {
        study
            .analysis
            .as_ref()
            .filter(|recorded| recorded.tool == shown.tool)
            .and_then(|recorded| recorded.model_revision.clone())
    });
    study.analysis = request.analysis.clone().map(|mut shown| {
        shown.model_revision = shown.model_revision.or(calculated_for);
        shown
    });
    // A request that sends no view keeps the recorded one: the page sends its view when one of its choices changes.
    if let Some(view) = &request.view {
        if view
            .identify_method
            .as_ref()
            .is_some_and(|method| method.chars().count() > 64)
        {
            return Err(Failure::new(
                "The page's method name is longer than 64 characters.",
                Cause::Settings,
                NextAction::None,
            ));
        }
        study.view = Some(view.clone());
    }
    storage::save_study(project_root, &study, &[])?;
    Ok(json!({}))
}

fn argument<T: DeserializeOwned>(arguments: &[Value]) -> Result<T, Failure> {
    read_argument(arguments, |_| Ok(()))
}

/// The command's argument, after `read` has read it in place and before it decodes: the analysis reads its number entries that way, so
/// that an entry the engine cannot read is refused under its field (`analysis/entries.rs`).
fn read_argument<T: DeserializeOwned>(
    arguments: &[Value],
    read: impl FnOnce(&mut Value) -> Result<(), Failure>,
) -> Result<T, Failure> {
    let mut value = arguments.first().cloned().unwrap_or(Value::Null);
    default_storage_folder(&mut value, &|name| std::env::var_os(name))?;
    refuse_other_runtime(&value)?;
    read(&mut value)?;
    trust_control::json::decode_value(value).map_err(|error| {
        Failure::new(
            &format!("The Control Studio command has invalid arguments: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })
}

/// A live request carries the connection the extension selects (`connection.runtimeId`, A2.7). When that runtime is not the loop's
/// (`runtimeId`), the request is refused before anything starts or is decoded: a selection change never switches the study's target
/// implicitly. A request without a connection, such as an analysis of saved evidence, is not refused here.
///
/// # Errors
/// The connection names another runtime than the loop's.
pub(crate) fn refuse_other_runtime(value: &Value) -> Result<(), Failure> {
    let Some(selected) = value
        .pointer("/connection/runtimeId")
        .and_then(Value::as_str)
    else {
        return Ok(());
    };
    let Some(target) = value.get("runtimeId").and_then(Value::as_str) else {
        return Ok(());
    };
    if selected == target {
        return Ok(());
    }
    let selected = if selected.is_empty() {
        "none"
    } else {
        selected
    };
    Err(Failure::new(
        &format!(
            "Control Studio is connected to {target}; the selected runtime is {selected}. Reconnect to continue."
        ),
        Cause::Runtime,
        NextAction::ConnectRuntime,
    ))
}

/// Fills an empty `storageFolder` of `request` with the engine's journal folder (§0.1a, A8.3): truST's folder in the user's local
/// state folder, which the command line uses too, so that both front ends see one journal per loop. A request without the field is
/// left as it is.
///
/// # Errors
/// The environment does not give the user's state folder.
fn default_storage_folder(
    request: &mut Value,
    environment: Environment<'_>,
) -> Result<(), Failure> {
    if let Some(folder) = request
        .get_mut("storageFolder")
        .filter(|folder| folder.as_str() == Some(""))
    {
        let default = settings::default_journal_folder(Platform::current(), environment)?;
        *folder = Value::String(default.display().to_string());
    }
    Ok(())
}

/// A method's answer as JSON.
pub(crate) fn to_value(value: &impl Serialize) -> Result<Value, Failure> {
    serde_json::to_value(value).map_err(|error| {
        Failure::new(
            &format!("The Control Studio answer cannot be written: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })
}

#[cfg(test)]
#[path = "commands_test_support.rs"]
pub(crate) mod test_support;

#[cfg(test)]
#[path = "commands_tests.rs"]
mod tests;
