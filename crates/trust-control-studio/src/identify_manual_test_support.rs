//! The fixtures that the tests of the manual identification share (the Recorded-data fit, B3.3, B3.6 and B3.7): the project folder of
//! the example loop, its first-order step records, the form of the fit, and the engine's answers to the fit's actions.

use std::fs;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::identification::form_draft::IDENTIFICATION_FORM_KEYS;
use trust_control::loop_definition::LoopDefinition;
use trust_control::messages::{OperationState, OperationUpdate};
use trust_control::recording::evidence::recording_evidence;
use trust_control::recording::{EvidenceSource, RecordedRun, RecordingFile};
use trust_control::study::manual_identification::ManualIdentificationSection;
use trust_control::study::{Study, StudySection};

use crate::commands::{ControlStudio, IDENTIFY};
use crate::operation::UpdateSink;
use crate::{loops, storage};

/// The example loop of the identification tests, the runtime it names.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");
pub(super) const LOOP_ID: &str = "control-pid-commissioning";
pub(super) const RUNTIME_ID: &str = "simulator";

/// The updates of an engine's operations, in the order the test receives them.
struct Updates(Mutex<mpsc::Sender<OperationUpdate>>);

impl UpdateSink for Updates {
    fn send(&self, update: OperationUpdate) {
        let _ = self.0.lock().send(update);
    }
}

/// An engine whose operation updates the test receives.
pub(super) fn engine() -> (ControlStudio, mpsc::Receiver<OperationUpdate>) {
    let (sender, receiver) = mpsc::channel();
    (
        ControlStudio::new(Arc::new(Updates(Mutex::new(sender)))),
        receiver,
    )
}

/// The update that ends an operation: its result, its failure or its cancellation.
fn final_update(updates: &mpsc::Receiver<OperationUpdate>) -> OperationUpdate {
    loop {
        let update = updates
            .recv_timeout(Duration::from_secs(30))
            .expect("an update of the operation");
        if update.state != OperationState::Running {
            return update;
        }
    }
}

/// The process value and the manipulated value of sample `index` (every 0.5 s) of the first-order step record of `from` to `to`:
/// the manipulated value steps at 5 s, and the process value starts at 20 and follows it with gain 0.5 and time constant 5 s.
pub(super) fn step_values(index: u32, from: f64, to: f64) -> (f64, f64) {
    let time = f64::from(index) * 0.5;
    let shape = if time < 5.0 {
        0.0
    } else {
        1.0 - (-(time - 5.0) / 5.0).exp()
    };
    let manipulated = if time < 5.0 { from } else { to };
    (20.0 + 0.5 * (to - from) * shape, manipulated)
}

/// A first-order step record of the example loop, sampled every 0.5 s for 60 s: the process value starts at 20 and, after the
/// manipulated value steps from `from` to `to` at 5 s, follows it with gain 0.5 and time constant 5 s.
pub(super) fn step_run(name: &str, from: f64, to: f64) -> RecordedRun {
    let samples: Vec<Value> = (0..120_u32)
        .map(|index| {
            let (process, manipulated) = step_values(index, from, to);
            json!({
                "scan": u64::from(index) + 1,
                "runtime_time_ns": 1_000_000_000_u64 + u64::from(index) * 500_000_000,
                "captured_at_ms": u64::from(index) * 500,
                "values": [process, manipulated],
                "forced": [false, false],
                "quality": ["good", "good"]
            })
        })
        .collect();
    serde_json::from_value(json!({
        "recording_id": name, "loop_revision": "sha256:loop-1",
        "bindings": [
            { "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" },
            { "id": "manipulatedVariable", "address": "%QL0", "iec_type": "LREAL" }
        ],
        "stop_reason": "sample_limit",
        "samples": samples,
        "acquisition": { "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 120,
            "scanRange": { "first": 1, "last": 120 }, "missingScanRanges": [], "droppedObservedSamples": 0,
            "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "" }
    }))
    .expect("a recorded run")
}

/// The Recorded-data fit's form for a first-order model without held-step settings or exclusions.
pub(super) fn first_order_form() -> Value {
    let values: serde_json::Map<String, Value> = IDENTIFICATION_FORM_KEYS
        .iter()
        .map(|key| ((*key).to_string(), json!("")))
        .collect();
    json!({
        "schemaVersion": 1, "model": "first_order", "heldStep": false, "values": values,
        "excludedStart": "", "excludedEnd": ""
    })
}

/// A project folder of one test with the example loop, removed when the test ends.
pub(super) struct Project {
    pub(super) root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Project {
    pub(super) fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-identify-manual-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("storage")).expect("the storage folder");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("the loop file");
        Self { root }
    }

    pub(super) fn loop_definition(&self) -> LoopDefinition {
        loops::open(&self.root, RUNTIME_ID, LOOP_ID).expect("the example loop opens")
    }

    pub(super) fn study(&self) -> Study {
        let loop_definition = self.loop_definition();
        storage::open_study(&self.root, &loop_definition.id, &loop_definition.revision)
            .expect("the working study")
    }

    /// Keeps the recording of `run` as the study's opened recording, and returns its identifier.
    pub(super) fn keep(&self, run: RecordedRun) -> String {
        let evidence = recording_evidence(&run, None);
        let mut file = RecordingFile::new(LOOP_ID, EvidenceSource::SimulatedPlant, run, evidence);
        file.opened = true;
        let id = file.id().expect("the recording identifier");
        storage::save_recording(&self.root, &self.study(), &file).expect("the recording is kept");
        id
    }

    /// The request of `action` on the loop; the manual actions need no runtime connection.
    fn request(&self, action: &Value) -> Value {
        json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": RUNTIME_ID,
            "loopId": LOOP_ID,
            "storageFolder": self.root.join("storage").display().to_string(),
            "action": action,
        })
    }
}

/// The engine's answer to `action`, which must be an answer at once: the `result` of a request that succeeded, which `execute` wraps
/// as `{"result": …}`, or the `failure` envelope of a refusal, which the refusal tests read (as `identify_import_tests.rs` does).
pub(super) fn answer(studio: &ControlStudio, project: &Project, action: &Value) -> Value {
    let envelope = studio
        .execute(IDENTIFY, &[project.request(action)])
        .expect("the engine answers its own method");
    envelope.get("result").cloned().unwrap_or(envelope)
}

/// The result of the manual operation `action` starts, which must finish with a result.
pub(super) fn finished(
    studio: &ControlStudio,
    updates: &mpsc::Receiver<OperationUpdate>,
    project: &Project,
    action: &Value,
) -> Value {
    let started = answer(studio, project, action);
    assert!(started["operationId"].is_string(), "{started}");
    let end = final_update(updates);
    assert_eq!(end.state, OperationState::Finished, "{end:?}");
    end.result.expect("the operation's result")
}

/// The state of the loop's manual section in the study, as its JSON.
pub(super) fn manual_section_json(project: &Project) -> Option<Value> {
    project
        .study()
        .sections
        .get(ManualIdentificationSection::KEY)
        .cloned()
}
