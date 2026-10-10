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
use trust_control::stages::stage_availability;
use trust_control::study::identify::IdentifySection;
use trust_control::study::tune::{stage_evidence, TuneSection};

use super::busy_loops::LoopClaims;
use super::identify_action;
use super::journal::Journal;
use super::operation::{Operations, UpdateSink};
use super::preview_action;
use super::runtime_client::runtime_recording::PendingReleases;
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
/// The methods this module answers.
pub const METHODS: [&str; 8] = [
    OPEN, STATUS, CANCEL, IDENTIFY, PAGE, TUNE, PREVIEW, VALIDATE,
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
            TUNE => argument(arguments)
                .and_then(|request| tune_action::answer(&self.operations, &self.claims, request)),
            PREVIEW => argument(arguments)
                .and_then(|request: PreviewRequest| preview_action::answer(&request)),
            VALIDATE => argument(arguments).and_then(|request| {
                validate_action::answer(&self.operations, &self.pending, &self.claims, request)
            }),
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
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let identify = study.section::<IdentifySection>()?;
    let tune = study.section::<TuneSection>()?;
    let journal = Journal::open(
        Path::new(&request.storage_folder),
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let evidence = stage_evidence(
        &loop_definition,
        identify.as_ref(),
        tune.as_ref(),
        !study.recordings.is_empty(),
    );
    Ok(OpenResult::Opened {
        stages: stage_availability(&evidence),
        reattached: journal
            .entries()
            .iter()
            .map(|entry| entry.operation_id().to_string())
            .collect(),
        engine_refusal: loop_definition.rust_engine_support().err(),
        loop_definition: Box::new(loop_definition),
        page: study.page,
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
    let value = arguments.first().cloned().unwrap_or(Value::Null);
    trust_control::json::decode_value(value).map_err(|error| {
        Failure::new(
            &format!("The Control Studio command has invalid arguments: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })
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

    struct Ignore;

    impl UpdateSink for Ignore {
        fn send(&self, _update: OperationUpdate) {}
    }

    const EXAMPLE: &str =
        include_str!("../../../examples/control-pid-commissioning/control-loop.json");

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
            opened["result"].get("engineRefusal").is_none(),
            "the example runs on the Rust engine"
        );
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
