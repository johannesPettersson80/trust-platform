//! The engine's methods (specification 35, §0.1a). Each answers `{"result": …}` or
//! `{"failure": …}`; a failure carries its cause and next action (A11). Each stage adds its
//! methods: `controlStudio/identify` (V1), `controlStudio/tune` and `controlStudio/preview` (V2), `controlStudio/validate`
//! (V3), `controlStudio/apply`, `controlStudio/restore` and `controlStudio/keep` (V4),
//! `controlStudio/setupDiscover` and `controlStudio/setupSave` (V5).

use std::path::Path;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::messages::{
    OpenRequest, OpenResult, OperationRequest, PageRequest, PreviewRequest,
};
use trust_control::setup::messages::{SetupDiscoverRequest, SetupSaveRequest};
use trust_control::stages::stage_availability;
use trust_control::study::inputs::{stage_evidence_of, InputsSection};

use super::apply_action;
use super::busy_loops::LoopClaims;
use super::identify_action;
use super::inputs_action;
use super::journal::Journal;
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
pub const METHODS: [&str; 15] = [
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
}

impl ControlStudio {
    /// Control Studio, reporting operation updates through `sink`.
    pub fn new(sink: Arc<dyn UpdateSink>) -> Self {
        Self {
            operations: Operations::new(sink),
            pending: Arc::new(PendingReleases::default()),
            claims: LoopClaims::default(),
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
                .and_then(|request| open(&request))
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
        Some(match answer {
            Ok(result) => json!({ "result": result }),
            Err(failure) => json!({ "failure": failure }),
        })
    }
}

/// Lists the project's loops, or opens the loop's study: the checked loop, its stages, the page last
/// viewed (A8.6) and the journaled operations it reattaches to. The stages follow the study's evidence: its recordings,
/// its Identify result while that is current for the loop's signals, and its Tune candidate while it was designed on that
/// current model (A8.4). Opening starts no test, tuning, validation or write (A8.6).
///
/// # Errors
/// The loop cannot be opened, or its study, its Identify or Tune section or its journal cannot be read.
pub fn open(request: &OpenRequest) -> Result<OpenResult, Failure> {
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
    Ok(OpenResult::Opened {
        stages: stage_availability(&stage_evidence_of(&loop_definition, &study)?),
        reattached: journal
            .entries()
            .iter()
            .filter(|entry| entry.holds_unfinished_operation())
            .map(|entry| entry.operation_id().to_string())
            .collect(),
        loop_definition: Box::new(loop_definition),
        page: study.page,
        inputs: study.section::<InputsSection>()?.unwrap_or_default(),
    })
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

    use trust_control::loop_definition::LoopDefinition;
    use trust_control::messages::OperationUpdate;
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
        assert!(
            !root.join("control-studio").exists(),
            "opening writes nothing"
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
}
