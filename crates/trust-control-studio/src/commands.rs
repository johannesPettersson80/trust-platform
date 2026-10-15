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
use trust_control::commissioning::observation_assessment::observation_in_progress;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::identify_result::{study_candidate, study_identification};
use trust_control::messages::{
    OpenRequest, OpenResult, OperationRequest, OperationState, PageRequest, ParameterWorkRequest,
    PreviewRequest,
};
use trust_control::setup::messages::{SetupDiscoverRequest, SetupSaveRequest};
use trust_control::stages::stage_availability;
use trust_control::study::analysis_records::current_records;
use trust_control::study::apply::ApplySection;
use trust_control::study::current_model::current_model;
use trust_control::study::inputs::{stage_evidence_of, InputsSection};
use trust_control::study::validate::ValidateSection;

use super::analysis_parameters_action;
use super::analyze_action;
use super::apply_action;
use super::busy_loops::LoopClaims;
use super::identify_action;
use super::inputs_action;
use super::journal::{Journal, JournalEntry};
use super::keep_action;
use super::operation::{Operations, UpdateSink};
use super::preview_action;
use super::run_action;
use super::runtime_client::runtime_recording::PendingReleases;
use super::settings::{self, Environment, Platform};
use super::setup_action;
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
/// Records the study's inputs (A8.1, A8.4; B4.2, B4.3): the acquisition settings, the requirements, the tuning method, its response
/// and structure, the pins and the manual settings. It answers at once with the stages and what the change invalidated.
pub const INPUTS: &str = "controlStudio/inputs";
/// The automatic progression (B8): runs, from the earliest stage whose result is missing or no longer current, the stages up to the one
/// it names (Identify, Tune or Validate), as one operation. It never applies.
pub const RUN: &str = "controlStudio/run";
/// The methods this module answers.
pub const METHODS: [&str; 17] = [
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
    ANALYZE,
    PARAMETERS,
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
}

impl ControlStudio {
    /// Control Studio, reporting operation updates through `sink`.
    pub fn new(sink: Arc<dyn UpdateSink>) -> Self {
        Self {
            operations: Operations::new(sink),
            pending: Arc::new(PendingReleases::default()),
            claims: LoopClaims::default(),
            started: Mutex::new(BTreeMap::new()),
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
            PAGE => argument(arguments).and_then(|request| set_page(&request)),
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
            ANALYZE => argument(arguments).and_then(|request| {
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
            SETUP_DISCOVER => argument(arguments)
                .and_then(|request: SetupDiscoverRequest| setup_action::answer_discover(&request)),
            SETUP_SAVE => argument(arguments)
                .and_then(|request: SetupSaveRequest| setup_action::answer_save(&request)),
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
        open_with_running(request, running)
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

/// Lists the project's loops, or opens the loop's study: the checked loop, its stages and their evidence, the page last
/// viewed (A8.6), the last Validate result and the Identify result while each is current, and the journaled operations it
/// reattaches to. The stages follow
/// the study's evidence: its recordings, its Identify result while that is current for the loop's signals, and its Tune candidate
/// while it was designed on that current model (A8.4). Opening starts no test, tuning, validation or write (A8.6).
///
/// # Errors
/// The loop cannot be opened, or its study, its Identify or Tune section or its journal cannot be read.
pub fn open(request: &OpenRequest) -> Result<OpenResult, Failure> {
    open_with_running(request, Vec::new())
}

/// [`open`] with the operations this engine runs for the loop, which the page follows (A8.6). The command line runs its own
/// operations inside its verb, so it opens with none.
fn open_with_running(request: &OpenRequest, running: Vec<String>) -> Result<OpenResult, Failure> {
    let project_root = Path::new(&request.project_root);
    let Some(loop_id) = &request.loop_id else {
        return Ok(OpenResult::Loops {
            loops: loops::list(project_root, &request.runtime_id)?,
        });
    };
    let loop_definition = loops::open(project_root, &request.runtime_id, loop_id)?;
    // A loop that uses a feature the Rust engine does not run yet stays on the TypeScript engine until its port (V6 decision D9):
    // its study is that engine's, so `open` refuses before it reads, or moves, the study.
    loop_definition.rust_engine_support()?;
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let journal = Journal::open(
        Path::new(&request.storage_folder),
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let applied = study.section::<ApplySection>()?.unwrap_or_default();
    let offers = apply_offers(journal.entries(), &applied);
    let evidence = stage_evidence_of(&loop_definition, &study)?;
    let stages = stage_availability(&evidence);
    // The last Validate result is answered while the study holds it as current (A8.4, A8.6): a stale one is history.
    let validation = if evidence.validation_has_result {
        study
            .section::<ValidateSection>()?
            .map(|section| section.result(evidence))
    } else {
        None
    };
    let identification = study_identification(&study, &loop_definition, evidence)?;
    let tune = study_candidate(&study, &loop_definition, evidence)?;
    // The kept analysis records that are current for the loop and the study's current model, fitted or supplied (B10.2, B3.7, A8.4).
    let selected_model = current_model(&study)?.map(|current| current.model);
    let analysis = current_records(&study, &loop_definition.revision, selected_model.as_ref());
    Ok(OpenResult::Opened {
        stages,
        reattached: journaled_operations(journal.entries()),
        running,
        validation,
        stage_evidence: evidence,
        identification,
        tune,
        loop_definition: Box::new(loop_definition),
        page: study.page,
        page_analysis: study.analysis.clone(),
        page_view: study.view.clone(),
        analysis,
        inputs: study.section::<InputsSection>()?.unwrap_or_default(),
        restore_available: offers.restore,
        keep_available: offers.keep,
        apply_observation_resumable: offers.observation_resumable,
    })
}

/// The journaled operations that reopening reattaches to (A8.5, A8.6): the recordings, experiments, campaigns and isolated jobs that
/// the journal holds unfinished. A checkpoint is not an operation the runtime runs: an Apply's checkpoint is answered as Restore and
/// Keep availability, and a mode checkpoint is returned by the identification's resume (B6.4), so neither is listed here.
fn journaled_operations(entries: &[JournalEntry]) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| {
            entry.holds_unfinished_operation()
                && !matches!(
                    entry,
                    JournalEntry::Commissioning { .. } | JournalEntry::IdentificationMode { .. }
                )
        })
        .map(|entry| entry.operation_id().to_string())
        .collect()
}

/// What Apply offers an opened loop (A8.6): Restore previous settings while the journal holds the Apply's checkpoint (B6.4), Keep in
/// project as `controlStudio/keep` decides it from the study's receipts (B6.5), and whether the journaled Apply observation can be
/// followed to its end (A8.5, A8.6): a checkpoint whose observation the runtime may still record. Opening reads the runtime for none of
/// it; a Restore or a Keep whose review the runtime no longer matches is refused when it is asked for.
pub(crate) struct ApplyOffers {
    pub(crate) restore: bool,
    pub(crate) keep: bool,
    pub(crate) observation_resumable: bool,
}

/// The offers of [`ApplyOffers`] for the journal's entries and the study's Apply section.
pub(crate) fn apply_offers(journal: &[JournalEntry], applied: &ApplySection) -> ApplyOffers {
    let checkpoint = journal.iter().find_map(|entry| match entry {
        JournalEntry::Commissioning { checkpoint } => Some(&**checkpoint),
        _ => None,
    });
    let observation_resumable = checkpoint.is_some_and(|checkpoint| {
        let recorded = checkpoint.recording.is_some();
        observation_in_progress(checkpoint.stage.journal_stage(), recorded, recorded)
    });
    ApplyOffers {
        restore: checkpoint.is_some(),
        keep: applied.keep_available(),
        observation_resumable,
    }
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
    study.analysis.clone_from(&request.analysis);
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
    let mut value = arguments.first().cloned().unwrap_or(Value::Null);
    default_storage_folder(&mut value, &|name| std::env::var_os(name))?;
    trust_control::json::decode_value(value).map_err(|error| {
        Failure::new(
            &format!("The Control Studio command has invalid arguments: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })
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
mod tests {
    use std::fs;

    use trust_control::experiment::ControlExperimentKind;
    use trust_control::identification::identify_result::identify_signals;
    use trust_control::identification::model_comparison::ComparisonPresentation;
    use trust_control::loop_definition::{LoopDefinition, NO_CONTROLLER_LAW};
    use trust_control::messages::OperationUpdate;
    use trust_control::model::decode_general_model;
    use trust_control::recording::EvidenceSource;
    use trust_control::study::identify::{IdentifyMethod, IdentifySection};
    use trust_control::study::{Page, Study};

    use super::*;

    #[test]
    fn an_empty_storage_folder_is_the_engines_journal_folder_and_a_named_one_is_kept() {
        // §0.1a and A8.3: the extension asks for the folder the command line uses by leaving the field empty.
        let environment = |name: &str| {
            matches!(name, "HOME" | "LOCALAPPDATA")
                .then(|| std::ffi::OsString::from("/home/engineer"))
        };
        let mut request = json!({ "projectRoot": "/project", "storageFolder": "" });
        default_storage_folder(&mut request, &environment).expect("the folder is found");
        let expected = settings::default_journal_folder(Platform::current(), &environment)
            .expect("the folder is found");
        assert_eq!(request["storageFolder"], expected.display().to_string());
        let mut named = json!({ "storageFolder": "/journals" });
        default_storage_folder(&mut named, &environment).expect("kept");
        assert_eq!(named["storageFolder"], "/journals");
        let mut without = json!({ "projectRoot": "/project" });
        default_storage_folder(&mut without, &environment).expect("kept");
        assert_eq!(
            without,
            json!({ "projectRoot": "/project" }),
            "no field is added"
        );
    }

    struct Ignore;

    impl UpdateSink for Ignore {
        fn send(&self, _update: OperationUpdate) {}
    }

    const EXAMPLE: &str =
        include_str!("../../../examples/control-pid-commissioning/control-loop.json");

    #[test]
    fn open_refuses_a_parked_loop_before_it_reads_or_moves_the_typescript_study() {
        // V6 decision D9: a loop with a parked feature stays on the TypeScript engine, whose study document is its own; opening it
        // through the engine must leave that document where it is.
        let root =
            std::env::temp_dir().join(format!("trust-control-open-parked-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("project");
        let mut file: Value = serde_json::from_str(EXAMPLE).expect("example");
        file["controller"]["stateFeedback"] = json!({ "family": "state-feedback" });
        fs::write(root.join("control-loop.json"), file.to_string()).expect("loop");
        let example = LoopDefinition::decode(EXAMPLE).expect("example");
        let directory = storage::study_directory(&root, &example.id);
        fs::create_dir_all(&directory).expect("study folder");
        let typescript_study = r#"{"schemaVersion":1,"loop":{"id":"x"},"state":{"page":"model"}}"#;
        fs::write(directory.join("study.json"), typescript_study).expect("study");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let request = json!({ "projectRoot": root.display().to_string(), "runtimeId": example.runtime_id,
            "loopId": example.id, "storageFolder": root.join("storage").display().to_string() });
        let answer = studio.execute(OPEN, &[request]).expect("answered");
        assert_eq!(
            answer["failure"]["cause"], "controller-capability",
            "{answer}"
        );
        assert_eq!(
            fs::read_to_string(directory.join("study.json")).expect("still there"),
            typescript_study,
            "the TypeScript study is neither read into a new study nor moved"
        );
        assert!(
            !directory.join("history").exists(),
            "nothing moved to history"
        );
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn open_lists_the_loops_then_opens_one_with_its_stages_and_no_operation() {
        let root = std::env::temp_dir().join(format!("trust-control-open-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("project");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
        let example = LoopDefinition::decode(EXAMPLE).expect("example");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let mut request = json!({ "projectRoot": root.display().to_string(), "runtimeId": example.runtime_id, "storageFolder": root.join("storage").display().to_string() });
        let listed = studio
            .execute(OPEN, std::slice::from_ref(&request))
            .expect("answered");
        assert_eq!(listed["result"]["kind"], "loops");
        assert_eq!(listed["result"]["loops"][0]["loopId"], example.id.as_str());
        request["loopId"] = json!(example.id);
        let opened = studio.execute(OPEN, &[request]).expect("answered");
        assert_eq!(opened["result"]["kind"], "opened", "{opened}");
        assert_eq!(opened["result"]["loop"]["id"], example.id.as_str());
        assert_eq!(opened["result"]["stages"][2]["stage"], "Identify");
        assert_eq!(opened["result"]["stages"][2]["status"], "needs-data");
        assert_eq!(opened["result"]["reattached"], json!([]));
        // A8.6: a study that never applied offers neither Restore nor Keep, and no Apply observation waits to be resumed.
        assert_eq!(opened["result"]["restoreAvailable"], json!(false));
        assert_eq!(opened["result"]["keepAvailable"], json!(false));
        assert_eq!(opened["result"]["applyObservationResumable"], json!(false));
        assert!(
            !root.join("control-studio").exists(),
            "opening writes nothing"
        );
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn open_serves_an_observation_lesson_whose_controller_stages_are_not_applicable() {
        // Specification 35, C12: a loop without a controller law is not parked, so `open` opens it on the engine. Tune, Validate and
        // Apply are Not applicable with the reason, and Identify still runs on it. Heater lesson 04 is such an observation loop.
        const OBSERVATION_LESSON: &str = include_str!(
            "../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/control-loop.json"
        );
        let root =
            std::env::temp_dir().join(format!("trust-control-open-c12-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("project");
        fs::write(root.join("control-loop.json"), OBSERVATION_LESSON).expect("loop");
        let lesson = LoopDefinition::decode(OBSERVATION_LESSON).expect("lesson");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let request = json!({ "projectRoot": root.display().to_string(), "runtimeId": lesson.runtime_id,
            "loopId": lesson.id, "storageFolder": root.join("storage").display().to_string() });
        let answer = studio.execute(OPEN, &[request]).expect("answered");
        assert_eq!(answer["result"]["kind"], "opened", "{answer}");
        let stages = answer["result"]["stages"].as_array().expect("stages");
        let stage = |name: &str| {
            stages
                .iter()
                .find(|stage| stage["stage"] == name)
                .unwrap_or_else(|| panic!("{name}: {answer}"))
        };
        assert_eq!(stage("Identify")["status"], "needs-data", "{answer}");
        for name in ["Tune", "Validate", "Apply"] {
            assert_eq!(stage(name)["status"], "not-applicable", "{name}: {answer}");
            assert_eq!(stage(name)["label"], "Not applicable", "{name}: {answer}");
            assert_eq!(stage(name)["reason"], NO_CONTROLLER_LAW, "{name}: {answer}");
        }
        assert!(stage("Identify").get("reason").is_none(), "{answer}");
        assert_eq!(
            answer["result"]["stageEvidence"]["controllerStagesApply"],
            json!(false)
        );
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_malformed_request_or_an_unknown_operation_answers_with_a_failure() {
        let studio = ControlStudio::new(Arc::new(Ignore));
        let malformed = studio
            .execute(OPEN, &[json!({ "projectRoot": "/x" })])
            .expect("answered");
        assert!(
            malformed["failure"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("invalid arguments")),
            "{malformed}"
        );
        let unknown = studio
            .execute(STATUS, &[json!({ "operationId": "operation-0" })])
            .expect("answered");
        assert!(unknown.get("failure").is_some(), "{unknown}");
        assert!(
            studio.execute("textDocument/hover", &[]).is_none(),
            "another method is not Control Studio's"
        );
    }

    /// A project folder with the example loop, and the arguments of `open` and `page` for it.
    fn example_project(name: &str) -> (std::path::PathBuf, LoopDefinition, Value) {
        let root =
            std::env::temp_dir().join(format!("trust-control-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("project");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("loop");
        let example = LoopDefinition::decode(EXAMPLE).expect("example");
        let request = json!({
            "projectRoot": root.display().to_string(),
            "runtimeId": example.runtime_id,
            "loopId": example.id,
            "storageFolder": root.join("storage").display().to_string(),
        });
        (root, example, request)
    }

    #[test]
    fn the_page_last_viewed_is_recorded_and_opening_returns_it_and_starts_nothing() {
        let (root, example, request) = example_project("page");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let opened = studio
            .execute(OPEN, std::slice::from_ref(&request))
            .expect("answered");
        assert_eq!(
            opened["result"]["page"], "Identify",
            "a new study opens at Identify (A8.6): {opened}"
        );
        let page = json!({
            "projectRoot": root.display().to_string(),
            "runtimeId": example.runtime_id,
            "loopId": example.id,
            "page": "Tune",
        });
        let answered = studio.execute(PAGE, &[page]).expect("answered");
        assert_eq!(answered, json!({ "result": {} }), "{answered}");
        let reopened = studio.execute(OPEN, &[request]).expect("answered");
        assert_eq!(reopened["result"]["page"], "Tune", "{reopened}");
        assert_eq!(
            reopened["result"]["reattached"],
            json!([]),
            "setting the page starts no operation"
        );
        let study = storage::open_study(&root, &example.id, &example.revision).expect("study");
        assert_eq!(study.page, Page::Tune);
        assert!(
            study.sections.is_empty() && study.recordings.is_empty(),
            "setting the page changes nothing else"
        );
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn the_page_keeps_its_method_choice_as_view_state_and_a_request_without_a_view_keeps_it() {
        // §0.1a `controlStudio/page`, A8.6: the Identify page's Method, including the opened-file and supplied-model entries that start
        // no campaign, comes back on open; a later page change without a view keeps it.
        let (root, example, request) = example_project("page-view");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let page = |view: Option<Value>| {
            let mut body = json!({
                "projectRoot": root.display().to_string(),
                "runtimeId": example.runtime_id,
                "loopId": example.id,
                "page": "Identify",
            });
            if let Some(view) = view {
                body["view"] = view;
            }
            body
        };
        let answered = studio
            .execute(
                PAGE,
                &[page(Some(json!({ "identifyMethod": "supplied-model" })))],
            )
            .expect("answered");
        assert_eq!(answered, json!({ "result": {} }), "{answered}");
        let reopened = studio
            .execute(OPEN, std::slice::from_ref(&request))
            .expect("answered");
        assert_eq!(
            reopened["result"]["pageView"],
            json!({ "identifyMethod": "supplied-model" }),
            "{reopened}"
        );
        studio.execute(PAGE, &[page(None)]).expect("answered");
        let again = studio.execute(OPEN, &[request]).expect("answered");
        assert_eq!(
            again["result"]["pageView"]["identifyMethod"], "supplied-model",
            "a request without a view keeps the recorded one: {again}"
        );
        let refused = studio
            .execute(
                PAGE,
                &[page(Some(json!({ "identifyMethod": "m".repeat(65) })))],
            )
            .expect("answered");
        assert!(refused.get("failure").is_some(), "{refused}");
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn a_study_of_an_earlier_loop_revision_opens_at_identify() {
        let (root, example, request) = example_project("page-revision");
        let mut earlier = Study::new(&example.id, "sha256:an-earlier-revision");
        earlier.page = Page::Validate;
        storage::save_study(&root, &earlier, &[]).expect("the earlier study is kept");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let opened = studio.execute(OPEN, &[request]).expect("answered");
        assert_eq!(
            opened["result"]["page"], "Identify",
            "a study of another loop revision starts again at Identify (A8.2): {opened}"
        );
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn an_unknown_page_is_a_failure_with_its_next_action() {
        let studio = ControlStudio::new(Arc::new(Ignore));
        let unknown = studio
            .execute(
                PAGE,
                &[json!({
                    "projectRoot": "/x",
                    "runtimeId": "simulator",
                    "loopId": "tank",
                    "page": "Monitor",
                })],
            )
            .expect("answered");
        assert!(
            unknown["failure"]["message"]
                .as_str()
                .is_some_and(|message| message.contains("invalid arguments")),
            "{unknown}"
        );
        assert_eq!(unknown["failure"]["cause"], "implementation", "{unknown}");
        assert_eq!(unknown["failure"]["nextAction"], "none", "{unknown}");
        assert!(unknown.get("result").is_none(), "{unknown}");
    }

    #[test]
    fn an_opened_loop_offers_restore_and_keep_only_from_the_journal_and_the_study() {
        // A8.6: Restore starts from the journaled checkpoint, so an interrupted Apply offers it; Keep follows the study's receipts.
        let nothing = apply_offers(&[], &ApplySection::default());
        assert!(!nothing.restore && !nothing.keep && !nothing.observation_resumable);
    }

    #[test]
    fn open_names_the_operations_the_loop_runs_while_they_run_and_forgets_them_once_they_end() {
        // A8.6: a reopened panel follows the operations that still run for the loop. An ended one is in the study, not in `running`.
        let (root, _example, request) = example_project("running-operations");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let (release, wait) = std::sync::mpsc::channel::<()>();
        let started = studio.operations().start("identify", move |_| {
            let _ = wait.recv();
            Ok(json!({}))
        });
        studio.remember_started(
            std::slice::from_ref(&request),
            &json!({ "operationId": started.operation_id }),
        );
        let running = studio
            .execute(OPEN, std::slice::from_ref(&request))
            .expect("answered");
        assert_eq!(
            running["result"]["running"],
            json!([started.operation_id]),
            "{running}"
        );
        release.send(()).expect("the operation is released");
        for _ in 0..500 {
            let ended = studio
                .operations()
                .status(&started.operation_id)
                .is_ok_and(|update| update.state != OperationState::Running);
            if ended {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let ended = studio
            .execute(OPEN, std::slice::from_ref(&request))
            .expect("answered");
        assert_eq!(ended["result"]["running"], json!([]), "{ended}");
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn an_operation_of_another_loop_is_not_named_by_this_loops_open() {
        let (root, _example, request) = example_project("running-other-loop");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let started = studio.operations().start("identify", |_| Ok(json!({})));
        let other = json!({ "projectRoot": root.display().to_string(), "loopId": "another-loop" });
        studio.remember_started(
            std::slice::from_ref(&other),
            &json!({ "operationId": started.operation_id }),
        );
        studio.remember_started(
            std::slice::from_ref(&request),
            &json!({ "operationId": "operation-unknown" }),
        );
        let opened = studio
            .execute(OPEN, std::slice::from_ref(&request))
            .expect("answered");
        assert_eq!(opened["result"]["running"], json!([]), "{opened}");
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn reattached_lists_the_unfinished_journaled_recording_by_its_identifier() {
        // A8.5, A8.6: a journaled recording is reattached by its identifier; the list holds journaled operations only.
        use trust_control::experiment::{
            ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
        };
        let owner = RecordingRecoveryOwner {
            schema_version: 1,
            recording_id: "recording-1".into(),
            project_id: "project:sha256:abc".into(),
            runtime_id: "runtime:local".into(),
            loop_id: "loop:tank".into(),
            loop_revision: "sha256:loop".into(),
            source: ControlEvidenceSource::Simulation,
            program_revision: None,
            start_acknowledgement_unconfirmed: None,
            bindings: vec![RecordingRecoveryBinding {
                id: "processVariable".into(),
                address: Some("%IL0".into()),
                symbol: None,
                iec_type: "LREAL".into(),
            }],
        };
        let entries = [JournalEntry::RecordingOwner { owner }];
        assert_eq!(
            journaled_operations(&entries),
            vec!["recording-1".to_owned()]
        );
        assert_eq!(journaled_operations(&[]), Vec::<String>::new());
    }

    /// The estimation recording of the Identify sections the tests keep, and the validation recording of a qualified one.
    const ESTIMATION: &str = "rec-00112233445566778899aabbccddeeff";
    const VALIDATION: &str = "rec-ffeeddccbbaa99887766554433221100";

    /// What the page shows of the fitted model `1 / (s + 1)`, as the identify fixtures of `identify_action_tests` show it.
    fn presentation() -> ComparisonPresentation {
        trust_control::json::decode_value(json!({
            "equation": "G(s) = 1 / (1s + 1)",
            "units": { "input": "V", "output": "m", "gain": "m/V", "time": "s" },
            "parameters": [
                { "name": "Gain", "value": 1.0, "unit": "m/V" },
                { "name": "Time constant 1", "value": 1.0, "unit": "s" },
                { "name": "Dead time", "value": 0.0, "unit": "s" }
            ],
            "poles": [-1.0], "zeros": [], "delaySeconds": 0.0,
            "uncertainty": { "label": "Sample-resolution envelope from the estimation record; not statistical confidence" },
            "estimation": { "normalizedError": 0.1 }, "validation": {},
            "predictedResponse": [
                { "timeSeconds": 0.0, "measured": 0.0, "predicted": 0.0, "residual": 0.0 },
                { "timeSeconds": 2.0, "measured": 1.0, "predicted": 0.9, "residual": 0.1 }
            ],
            "validityRange": { "inputMinimum": 0.0, "inputMaximum": 1.0, "outputMinimum": 0.0, "outputMaximum": 1.0 },
            "warnings": []
        }))
        .expect("the presentation decodes")
    }

    /// The Identify section of the example loop: the fitted model `1 / (s + 1)` from a command in V to a position in m. A
    /// `validated` section names a validation recording, as a model that Validate qualified does (B5.2), so it is not provisional.
    fn identified_section(example: &LoopDefinition, validated: bool) -> IdentifySection {
        let channel = |id: &str, unit: &str| json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } });
        let (validation, evidence) = if validated {
            (
                "validated",
                json!(["sha256:recording", "sha256:validation"]),
            )
        } else {
            ("unvalidated", json!(["sha256:recording"]))
        };
        let model = decode_general_model(&json!({
            "kind": "transfer-function", "revision": "model:fitted:1", "source": "fitted",
            "validation": validation, "evidence": evidence, "delaySeconds": 0.0,
            "inputs": [channel("command", "V")], "outputs": [channel("position", "m")],
            "numerator": [1.0], "denominator": [1.0, 1.0]
        }))
        .expect("a fitted model");
        IdentifySection::new(
            IdentifyMethod::Live(ControlExperimentKind::MvStep),
            EvidenceSource::SimulatedPlant,
            identify_signals(example, ControlExperimentKind::MvStep),
            ESTIMATION,
            validated.then_some(VALIDATION),
            model,
            presentation(),
        )
    }

    #[test]
    fn open_holds_a_current_identification_with_its_evidence_and_none_after_reset() {
        // A8.6, B9.4: a reopened study with a current model holds its identification and says it has a model, so the page enables
        // Tune without deducing it. Reset starts a new study, which holds neither.
        let (root, example, request) = example_project("identification");
        let mut study = Study::new(&example.id, &example.revision);
        study
            .set_section(&identified_section(&example, false))
            .expect("the Identify section is kept");
        storage::save_study(&root, &study, &[]).expect("the study is kept");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let opened = studio
            .execute(OPEN, std::slice::from_ref(&request))
            .expect("answered");
        assert_eq!(
            opened["result"]["stageEvidence"]["hasModel"],
            json!(true),
            "{opened}"
        );
        let identification = &opened["result"]["identification"];
        assert_eq!(
            identification["modelRevision"], "model:fitted:1",
            "{opened}"
        );
        assert_eq!(identification["provisional"], json!(true), "{opened}");
        assert!(
            opened["result"].get("tune").is_none(),
            "a study with no candidate holds none: {opened}"
        );
        assert!(
            identification["plot"]["timeSeconds"]
                .as_array()
                .is_some_and(|times| !times.is_empty()),
            "the answer holds the plot: {opened}"
        );
        storage::start_new_study(&root, &study).expect("the study is reset");
        let reset = studio.execute(OPEN, &[request]).expect("answered");
        assert_eq!(
            reset["result"]["stageEvidence"]["hasModel"],
            json!(false),
            "{reset}"
        );
        assert!(reset["result"].get("identification").is_none(), "{reset}");
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn open_holds_a_model_that_validate_qualified_as_not_provisional() {
        // B5.2, A8.6: a model whose independent check passed names its validation recording, and reopens as not provisional.
        let (root, example, request) = example_project("identification-qualified");
        let mut study = Study::new(&example.id, &example.revision);
        study
            .set_section(&identified_section(&example, true))
            .expect("the qualified Identify section is kept");
        storage::save_study(&root, &study, &[]).expect("the study is kept");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let opened = studio.execute(OPEN, &[request]).expect("answered");
        assert_eq!(
            opened["result"]["identification"]["provisional"],
            json!(false),
            "{opened}"
        );
        fs::remove_dir_all(&root).expect("clean up");
    }

    #[test]
    fn reset_answers_the_stage_evidence_of_the_fresh_study() {
        // A8.4, B9.4: after Reset the new study holds no model, candidate or passed validation, and the answer says so, so the page
        // takes that evidence from the engine rather than assuming it.
        let (root, example, request) = example_project("reset-evidence");
        let mut study = Study::new(&example.id, &example.revision);
        study
            .set_section(&identified_section(&example, false))
            .expect("the Identify section is kept");
        storage::save_study(&root, &study, &[]).expect("the study is kept");
        let studio = ControlStudio::new(Arc::new(Ignore));
        let mut reset_request = request;
        reset_request["action"] = json!({ "action": "reset" });
        let answer = studio
            .execute(IDENTIFY, &[reset_request])
            .expect("answered");
        let evidence = &answer["result"]["stageEvidence"];
        assert_eq!(evidence["hasModel"], json!(false), "{answer}");
        assert_eq!(evidence["hasCandidate"], json!(false), "{answer}");
        assert_eq!(evidence["validationHasResult"], json!(false), "{answer}");
        assert_eq!(evidence["validationPassed"], json!(false), "{answer}");
        fs::remove_dir_all(&root).expect("clean up");
    }
}
