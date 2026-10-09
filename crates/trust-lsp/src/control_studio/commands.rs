//! The `trust-lsp.controlStudio…` commands (specification 35, §0.1a). Each answers
//! `{"result": …}` or `{"failure": …}`; a failure carries its cause and next action (A11). Each
//! stage adds its commands: `controlStudioIdentify` (V1), `controlStudioTune` (V2),
//! `controlStudioValidate` (V3), `controlStudioApply`, `controlStudioRestore` and
//! `controlStudioKeep` (V4), `controlStudioSetupDiscover` and `controlStudioSetupSave` (V5).
//!
//! The language client registers every command the server advertises as a VS Code command, so a
//! server command never shares a name with one of the extension's own commands, such as
//! `trust-lsp.controlStudio.open`, which opens the panel.

use std::path::Path;
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::messages::{OpenRequest, OpenResult, OperationRequest};
use trust_control::stages::{stage_availability, StageEvidence};

use super::journal::Journal;
use super::operation::{Operations, UpdateSink};
use super::{loops, storage};

/// Opens a project loop's study, or lists the project's loops.
pub const OPEN: &str = "trust-lsp.controlStudioOpen";
/// The latest update of an operation, for a panel that reattaches.
pub const STATUS: &str = "trust-lsp.controlStudioStatus";
/// Cancels an operation at its next step.
pub const CANCEL: &str = "trust-lsp.controlStudioCancel";
/// The commands this module answers.
pub const COMMANDS: [&str; 3] = [OPEN, STATUS, CANCEL];
/// The notification that carries operation updates.
pub const UPDATE_NOTIFICATION: &str = "trust/controlStudio/update";

/// Control Studio in one language server.
pub struct ControlStudio {
    operations: Operations,
}

impl ControlStudio {
    /// Control Studio, reporting operation updates through `sink`.
    pub fn new(sink: Arc<dyn UpdateSink>) -> Self {
        Self {
            operations: Operations::new(sink),
        }
    }

    /// The operations, for the stages that start them.
    #[must_use]
    pub fn operations(&self) -> &Operations {
        &self.operations
    }

    /// Runs `command` with its `arguments`; `None` when the command is not Control Studio's.
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
            _ => return None,
        };
        Some(match answer {
            Ok(result) => json!({ "result": result }),
            Err(failure) => json!({ "failure": failure }),
        })
    }
}

/// Lists the project's loops, or opens the loop's study: the checked loop, its stages and the
/// journaled operations it reattaches to. Opening starts no test, tuning, validation or write
/// (A8.6).
///
/// # Errors
/// The loop cannot be opened, or its study or journal cannot be read.
pub fn open(request: &OpenRequest) -> Result<OpenResult, Failure> {
    let project_root = Path::new(&request.project_root);
    let Some(loop_id) = &request.loop_id else {
        return Ok(OpenResult::Loops {
            loops: loops::list(project_root, &request.runtime_id)?,
        });
    };
    let loop_definition = loops::open(project_root, &request.runtime_id, loop_id)?;
    let study = storage::open_study(project_root, &loop_definition.id, &loop_definition.revision)?;
    let journal = Journal::open(
        Path::new(&request.storage_folder),
        &loop_definition.project_id,
        &loop_definition.id,
    )?;
    let evidence = StageEvidence {
        has_recording: !study.recordings.is_empty(),
        ..StageEvidence::default()
    };
    Ok(OpenResult::Opened {
        stages: stage_availability(&evidence),
        reattached: journal
            .entries()
            .iter()
            .map(|entry| entry.operation_id().to_string())
            .collect(),
        engine_refusal: loop_definition.rust_engine_support().err(),
        loop_definition: Box::new(loop_definition),
    })
}

fn argument<T: DeserializeOwned>(arguments: &[Value]) -> Result<T, Failure> {
    let value = arguments.first().cloned().unwrap_or(Value::Null);
    serde_json::from_value(value).map_err(|error| {
        Failure::new(
            &format!("The Control Studio command has invalid arguments: {error}."),
            Cause::Implementation,
            NextAction::None,
        )
    })
}

fn to_value(value: &impl Serialize) -> Result<Value, Failure> {
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

    use super::*;

    struct Ignore;

    impl UpdateSink for Ignore {
        fn send(&self, _update: OperationUpdate) {}
    }

    const EXAMPLE: &str =
        include_str!("../../../../examples/control-pid-commissioning/control-loop.json");

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
            studio.execute("trust-lsp.projectInfo", &[]).is_none(),
            "another command is not Control Studio's"
        );
    }
}
