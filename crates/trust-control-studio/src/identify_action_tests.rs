//! The routing of `controlStudio/identify` and of the engine's cancel (specification 35, §0.1a, §0.1b, A8.4 to A8.6, B3.4
//! and B3.6), with the Identify-routing assertions of `control-studio-final-wiring.test.ts` (tests 7 and 8, applied to the
//! study's own keep of a recording) and `control-studio-identification-setpoint.test.ts` (test 1, the explicitly selected
//! reference, as the method receives it). The keep before the release on the run itself is tested in
//! `workflow/identify/final_wiring_tests.rs`, and the whole method against a real runtime in `tests/control_studio_identify.rs`.
//!
//! No runtime runs here. The endpoint is mostly a port nothing listens on, so an operation that reaches the runtime ends with
//! "Control Studio cannot reach the runtime", which shows that it started and that nothing came before the runtime. The cancel
//! of a lone recording owner talks to a stand-in that answers the runtime's status and refuses every other request, so the stop
//! of the recording is refused and the owner stays journaled.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::json;
use trust_control::experiment::{
    ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::failure::{Cause, NextAction};
use trust_control::identification::connected_run::ACQUISITION_SETTINGS_TEXT;
use trust_control::identification::model_comparison::ComparisonPresentation;
use trust_control::messages::{OperationState, OperationUpdate};
use trust_control::model::{decode_general_model, GeneralModel};
use trust_control::recording::evidence::recording_evidence;
use trust_control::recording::{Completeness, RecordedRun, RecordingFile};
use trust_control::study::identify::{IdentifySection, IdentifySignals};

use super::*;
use crate::commands::{ControlStudio, IDENTIFY, METHODS, OPEN};
use crate::operation::UpdateSink;
use crate::storage::study_directory;
use crate::workflow::identify::RecordingRetention;

/// The example project's loop file.
const EXAMPLE: &str = include_str!("../../../examples/control-pid-commissioning/control-loop.json");
/// The example loop, the runtime it names, and the symbols of its plant input and measured output.
const LOOP_ID: &str = "control-pid-commissioning";
const RUNTIME_ID: &str = "simulator";
const PLANT_INPUT: &str = "P1.ManipulatedValue";
const MEASUREMENT: &str = "P1.ProcessValue";
/// The runtime identity of the operation the tests journal.
const JOURNALED: &str = "recording-7";
/// The refusal of a setpoint reference that is not a declared setpoint in the measured output's units.
const REFERENCE_TEXT: &str = "Choose a declared setpoint signal in the measured output's units.";

/// The updates of an engine's operations, in the order the test receives them.
struct Updates(Mutex<mpsc::Sender<OperationUpdate>>);

impl UpdateSink for Updates {
    fn send(&self, update: OperationUpdate) {
        let _ = self.0.lock().send(update);
    }
}

/// An engine whose operation updates the test receives.
fn engine() -> (ControlStudio, mpsc::Receiver<OperationUpdate>) {
    let (sender, receiver) = mpsc::channel();
    let studio = ControlStudio::new(Arc::new(Updates(Mutex::new(sender))));
    (studio, receiver)
}

/// The update that ends the operation: its result, its failure or its cancellation.
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

/// The next action of an operation that ended with a failure.
fn failure_action(update: OperationUpdate) -> Option<NextAction> {
    update.failure.map(|failure| failure.next_action)
}

/// The endpoint of a port that nothing listens on: the runtime cannot be reached there.
fn closed_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("the port").port();
    drop(listener);
    format!("tcp://127.0.0.1:{port}")
}

/// The endpoint of a runtime that answers `status` as a simulated target (E1.3) and refuses every other request (E0.4).
/// Its threads end with the test process.
fn simulated_runtime() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("a free port");
    let endpoint = format!("tcp://{}", listener.local_addr().expect("the port"));
    thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            thread::spawn(move || answer_requests(stream));
        }
    });
    endpoint
}

/// Answers the request lines of one connection to the simulated runtime.
fn answer_requests(stream: TcpStream) {
    let Ok(mut writer) = stream.try_clone() else {
        return;
    };
    for line in BufReader::new(stream).lines() {
        let Some(request) = line
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        else {
            return;
        };
        let answer = if request["type"] == "status" {
            json!({ "id": request["id"], "ok": true, "result": {
                "session_epoch": 7, "simulation_enabled": true, "simulation_only_io": true,
                "simulation_time_scale": 1, "simulation_achieved_time_scale": null,
                "program_revision": "program", "tasks": [{ "name": "MainTask", "interval_ms": 100 }],
                "cycle_interval_ms": 100
            } })
        } else {
            json!({ "id": request["id"], "ok": false, "error": "Not in this test.", "error_code": "invalid_request" })
        };
        if writeln!(writer, "{answer}").is_err() {
            return;
        }
    }
}

/// Asks the engine for `controlStudio/identify`.
fn identify(studio: &ControlStudio, request: Value) -> Value {
    studio
        .execute(IDENTIFY, &[request])
        .expect("the engine answers its own method")
}

/// The message and the next action of a refused answer.
fn refusal(answer: &Value) -> (&str, &str) {
    let failure = &answer["failure"];
    (
        failure["message"].as_str().unwrap_or_default(),
        failure["nextAction"].as_str().unwrap_or_default(),
    )
}

/// The status of `stage` in an answer's stages.
fn status_of<'a>(stages: &'a Value, stage: &str) -> Option<&'a str> {
    stages
        .as_array()?
        .iter()
        .find(|item| item["stage"] == stage)?
        .get("status")?
        .as_str()
}

/// Whether the engine started no operation.
fn nothing_started(studio: &ControlStudio) -> bool {
    studio.operations().status("operation-1").is_err()
}

/// The `identify` action with the acquisition `settings`.
fn live(settings: Value) -> Value {
    let mut action = json!({ "action": "identify" });
    action["settings"] = settings;
    action
}

/// Automatic identification within the example loop's declared duration.
fn automatic() -> Value {
    json!({
        "method": "automatic", "durationSeconds": 16, "amplitude": 0, "reuseRecordings": false
    })
}

/// A setpoint step that names `reference` as its setpoint (the settings of the `TypeScript` setpoint test).
fn setpoint_step(reference: &str) -> Value {
    json!({
        "method": "setpoint_step", "durationSeconds": 10, "amplitude": 1, "reuseRecordings": true,
        "setpointSignalId": reference
    })
}

/// The `openRecording` action for `recording_id`.
fn open_recording(recording_id: &str) -> Value {
    json!({ "action": "openRecording", "recordingId": recording_id })
}

/// The example loop with a declared further setpoint, `additional-reference`: the loop's own setpoint, in the measured
/// output's unit.
fn loop_with_reference() -> String {
    let mut value: Value = serde_json::from_str(EXAMPLE).expect("the example loop");
    value["signals"]["additional"] = json!([{
        "id": "additional-reference", "channel": "primary", "role": "SP", "symbol": "P1.Setpoint",
        "iecType": "LREAL", "direction": "read", "unit": "°C", "range": { "minimum": 0, "maximum": 100 }
    }]);
    value.to_string()
}

/// A complete run of two consecutive samples (the run of `storage_tests.rs`).
fn recorded_run() -> RecordedRun {
    serde_json::from_value(json!({
        "recording_id": "recording-1", "loop_revision": "sha256:loop-1",
        "bindings": [{ "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" }],
        "stop_reason": "sample_limit",
        "samples": [
            { "scan": 1, "runtime_time_ns": 100_000_000, "captured_at_ms": 1, "values": [20.5], "forced": [false], "quality": ["good"] },
            { "scan": 2, "runtime_time_ns": 200_000_000, "captured_at_ms": 2, "values": [21.0], "forced": [false], "quality": ["good"] }
        ],
        "acquisition": { "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 2,
            "scanRange": { "first": 1, "last": 2 }, "missingScanRanges": [], "droppedObservedSamples": 0,
            "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "" }
    }))
    .expect("a recorded run")
}

/// The recording file of the run, with the verdict it earned (E4.5).
fn recording() -> RecordingFile {
    let run = recorded_run();
    let evidence = recording_evidence(&run, None);
    RecordingFile::new(LOOP_ID, EvidenceSource::SimulatedPlant, run, evidence)
}

/// A fitted model `1 / (s + 1)` from a command in V to a position in m (the model of `storage_tests.rs`).
fn fitted_model() -> GeneralModel {
    let channel = |id: &str, unit: &str| json!({ "id": id, "unit": unit, "operatingRange": { "minimum": -10.0, "maximum": 10.0 } });
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:fitted:1", "source": "fitted",
        "validation": "unvalidated", "evidence": ["sha256:recording"], "delaySeconds": 0.0,
        "inputs": [channel("command", "V")], "outputs": [channel("position", "m")],
        "numerator": [1.0], "denominator": [1.0, 1.0]
    }))
    .expect("a fitted model")
}

/// What the page shows of the fitted model: its first-order fit, in the model's units (the presentation of
/// `storage_tests.rs`).
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

/// A project folder of one test with the example loop, removed when the test ends, whether or not it passed.
struct Project {
    root: PathBuf,
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-identify-action-{name}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the project folder");
        fs::write(root.join("control-loop.json"), EXAMPLE).expect("the loop file");
        Self { root }
    }

    /// The folder of the operation journals (A8.3).
    fn storage(&self) -> PathBuf {
        self.root.join("storage")
    }

    fn loop_file(&self) -> String {
        fs::read_to_string(self.root.join("control-loop.json")).expect("the loop file")
    }

    fn loop_definition(&self) -> LoopDefinition {
        loops::open(&self.root, RUNTIME_ID, LOOP_ID).expect("the example loop opens")
    }

    fn working_study(&self) -> Study {
        let loop_definition = self.loop_definition();
        storage::open_study(&self.root, &loop_definition.id, &loop_definition.revision)
            .expect("the working study")
    }

    /// The method's request for `action` on the loop, with a runtime that cannot be reached.
    fn request(&self, action: Value) -> Value {
        let mut request = json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": RUNTIME_ID,
            "loopId": LOOP_ID,
            "storageFolder": self.storage().display().to_string(),
            "connection": { "endpoint": closed_endpoint() },
        });
        request["action"] = action;
        request
    }

    /// The method's request for `action` on the loop, carrying no runtime connection (E11.1).
    fn request_without_connection(&self, action: Value) -> Value {
        let mut request = self.request(action);
        if let Some(object) = request.as_object_mut() {
            object.remove("connection");
        }
        request
    }

    fn journal(&self) -> Journal {
        let loop_definition = self.loop_definition();
        Journal::open(
            &self.storage(),
            &loop_definition.project_id,
            &loop_definition.id,
        )
        .expect("the loop's journal")
    }

    /// Journals a recording of the loop, as a start journals its owner before it is dispatched (E11.2).
    fn journal_an_operation(&self) {
        let loop_definition = self.loop_definition();
        let owner = RecordingRecoveryOwner {
            schema_version: 1,
            recording_id: JOURNALED.to_string(),
            project_id: loop_definition.project_id.clone(),
            runtime_id: loop_definition.runtime_id.clone(),
            loop_id: loop_definition.id.clone(),
            loop_revision: loop_definition.revision.clone(),
            source: ControlEvidenceSource::Simulation,
            program_revision: None,
            start_acknowledgement_unconfirmed: None,
            bindings: vec![RecordingRecoveryBinding {
                id: "processVariable".to_string(),
                address: Some("%IL0".to_string()),
                symbol: None,
                iec_type: "LREAL".to_string(),
            }],
        };
        self.journal()
            .record(JournalEntry::RecordingOwner { owner })
            .expect("the operation is journaled");
    }

    /// The runtime identities the loop's journal holds.
    fn journaled(&self) -> Vec<String> {
        self.journal()
            .entries()
            .iter()
            .map(|entry| entry.operation_id().to_string())
            .collect()
    }

    /// Keeps an Identify result whose plant input is `plant_input` in the working study, with the recording it was fitted
    /// on. The section keeps no record request.
    fn keep_identify_result(&self, plant_input: &str) -> IdentifySection {
        let file = recording();
        let signals = IdentifySignals {
            inputs: vec![plant_input.to_string()],
            outputs: vec![MEASUREMENT.to_string()],
            setpoint: None,
        };
        let section = IdentifySection::new(
            IdentifyMethod::Live(ControlExperimentKind::MvStep),
            EvidenceSource::SimulatedPlant,
            signals,
            &file.id().expect("the recording's identifier"),
            None,
            fitted_model(),
            presentation(),
        );
        storage::save_identify(&self.root, &self.working_study(), &section, &[file])
            .expect("the Identify result is kept");
        section
    }

    /// The status of the Identify and Tune stages that `controlStudio/open` answers for the loop.
    fn opened_stages(&self, studio: &ControlStudio) -> (Option<String>, Option<String>) {
        let request = json!({
            "projectRoot": self.root.display().to_string(),
            "runtimeId": RUNTIME_ID,
            "loopId": LOOP_ID,
            "storageFolder": self.storage().display().to_string(),
        });
        let opened = studio.execute(OPEN, &[request]).expect("answered");
        let stages = &opened["result"]["stages"];
        (
            status_of(stages, "Identify").map(str::to_string),
            status_of(stages, "Tune").map(str::to_string),
        )
    }
}

#[test]
fn identify_is_a_method_of_the_engine_and_a_malformed_request_or_an_unknown_action_is_refused() {
    assert!(METHODS.contains(&IDENTIFY), "the engine answers {IDENTIFY}");
    let project = Project::new("malformed");
    let (studio, _updates) = engine();
    let unknown = identify(&studio, project.request(json!({ "action": "frobnicate" })));
    let malformed = identify(&studio, json!({ "projectRoot": "/x" }));
    for answer in [unknown, malformed] {
        assert!(refusal(&answer).0.contains("invalid arguments"), "{answer}");
    }
    assert!(nothing_started(&studio), "nothing was started");
}

#[test]
fn identify_answers_with_its_operation_at_once_and_writes_nothing_before_the_runtime_answers() {
    let project = Project::new("identify");
    let (studio, updates) = engine();
    let answer = identify(&studio, project.request(live(automatic())));
    assert_eq!(answer["result"]["operationId"], "operation-1", "{answer}");
    let update = final_update(&updates);
    assert_eq!(update.operation_id, "operation-1");
    assert_eq!(update.command, "identify");
    assert_eq!(update.state, OperationState::Failed);
    assert_eq!(failure_action(update), Some(NextAction::ConnectRuntime));
    assert!(
        !study_directory(&project.root, LOOP_ID).exists(),
        "no study is written before the runtime answers"
    );
    assert_eq!(
        project.journaled(),
        Vec::<String>::new(),
        "nothing is journaled"
    );
}

#[test]
fn identify_and_open_recording_are_refused_at_once_while_the_loop_has_an_unfinished_operation() {
    let project = Project::new("unfinished");
    project.journal_an_operation();
    let (studio, updates) = engine();
    let any_recording = format!("rec-{}", "0".repeat(32));
    for action in [live(automatic()), open_recording(&any_recording)] {
        let answer = identify(&studio, project.request(action));
        assert_eq!(
            refusal(&answer),
            ("This loop has an unfinished operation.", "wait"),
            "{answer}"
        );
        assert_eq!(
            answer["failure"]["detail"],
            "This loop has an unfinished operation. Wait for it to end, or resume or cancel it. Journaled: recording-7."
        );
    }
    assert!(nothing_started(&studio), "nothing was started");
    let resumed = identify(&studio, project.request(json!({ "action": "resume" })));
    assert_eq!(
        resumed["result"]["operationId"], "operation-1",
        "resume follows the journal: {resumed}"
    );
    assert_eq!(
        failure_action(final_update(&updates)),
        Some(NextAction::ConnectRuntime)
    );
    assert_eq!(
        project.journaled(),
        [JOURNALED],
        "the journal stays the runtime's until it is resumed or cancelled"
    );
}

#[test]
fn identify_refuses_acquisition_settings_that_fail_their_checks_before_anything_starts() {
    let project = Project::new("settings");
    let (studio, _updates) = engine();
    let short = live(json!({
        "method": "automatic", "durationSeconds": 0.5, "amplitude": 0, "reuseRecordings": false
    }));
    let refused = identify(&studio, project.request(short));
    assert_eq!(
        refusal(&refused),
        (ACQUISITION_SETTINGS_TEXT, "open-limits"),
        "{refused}"
    );
    for reference in ["unknown", "manipulatedVariable"] {
        let refused = identify(&studio, project.request(live(setpoint_step(reference))));
        assert_eq!(
            refusal(&refused),
            (REFERENCE_TEXT, "none"),
            "{reference} is not a declared setpoint: {refused}"
        );
    }
    assert!(nothing_started(&studio), "nothing was started");
}

/// `control-studio-identification-setpoint.test.ts`, test 1, as the method receives it: only an explicitly selected, declared
/// reference starts a setpoint step, and the loop, with its controller, is not changed.
#[test]
fn a_setpoint_step_takes_only_a_selected_declared_reference_and_leaves_the_loop_unchanged() {
    let project = Project::new("reference");
    let with_reference = loop_with_reference();
    fs::write(project.root.join("control-loop.json"), &with_reference).expect("the loop file");
    let (studio, updates) = engine();
    let unknown = identify(&studio, project.request(live(setpoint_step("unknown"))));
    assert_eq!(refusal(&unknown), (REFERENCE_TEXT, "none"), "{unknown}");
    let declared = live(setpoint_step("additional-reference"));
    let started = identify(&studio, project.request(declared));
    assert_eq!(started["result"]["operationId"], "operation-1", "{started}");
    assert_eq!(
        failure_action(final_update(&updates)),
        Some(NextAction::ConnectRuntime)
    );
    assert_eq!(
        project.loop_file(),
        with_reference,
        "the loop and its controller target are not changed"
    );
}

#[test]
fn open_recording_fits_again_only_the_recording_whose_request_the_identify_result_keeps() {
    let project = Project::new("refit");
    let (studio, _updates) = engine();
    let other = format!("rec-{}", "0".repeat(32));
    let refused = |recording_id: &str| {
        let answer = identify(&studio, project.request(open_recording(recording_id)));
        let (message, next_action) = refusal(&answer);
        let expected = format!("Recording {recording_id} cannot be fitted again");
        assert!(message.starts_with(&expected), "{answer}");
        assert_eq!(next_action, "identify", "{answer}");
    };
    refused(other.as_str());
    let section = project.keep_identify_result(PLANT_INPUT);
    refused(section.estimation_recording_id.as_str());
    refused(other.as_str());
    assert!(
        nothing_started(&studio),
        "no recording is fitted and no experiment starts"
    );
}

#[test]
fn reset_starts_a_new_working_study_and_keeps_the_previous_one_and_the_loop() {
    let project = Project::new("reset");
    let section = project.keep_identify_result(PLANT_INPUT);
    let loop_file = project.loop_file();
    let (studio, _updates) = engine();
    let answer = identify(&studio, project.request(json!({ "action": "reset" })));
    // §0.1a: the reset names its kind, so the page clears the model for this answer alone, never for any answer without a model.
    assert_eq!(answer["result"]["kind"], "reset", "{answer}");
    let stages = &answer["result"]["stages"];
    assert_eq!(
        status_of(stages, "Identify"),
        Some("needs-data"),
        "{answer}"
    );
    assert!(nothing_started(&studio), "Reset answers at once");
    let fresh = project.working_study();
    assert!(
        fresh.sections.is_empty() && fresh.recordings.is_empty(),
        "the new working study starts empty"
    );
    let history = study_directory(&project.root, LOOP_ID).join("history");
    let archived: Vec<PathBuf> = fs::read_dir(&history)
        .expect("the history")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .collect();
    assert_eq!(archived.len(), 1, "one previous study");
    let text = fs::read_to_string(archived[0].join("study.json")).expect("the previous study");
    let previous = Study::decode(&text, LOOP_ID).expect("the previous study reads");
    assert_eq!(
        previous.section::<IdentifySection>().expect("its section"),
        Some(section),
        "the previous study keeps its Identify result"
    );
    let kept = archived[0]
        .join("recordings")
        .join(format!("{}.json", previous.recordings[0]));
    assert!(kept.is_file(), "its recording is kept");
    assert_eq!(
        project.loop_file(),
        loop_file,
        "the loop keeps its signals and limits"
    );
}

/// A8.4, B9.9: the answer of a reset states the Identify page's status line, so the page writes none (the sentence of a fit or a supplied model
/// is `each_identify_answer_states_its_status_sentence` of trust-control).
#[test]
fn a_reset_states_its_status_sentence() {
    let project = Project::new("reset-sentence");
    project.keep_identify_result(PLANT_INPUT);
    let (studio, _updates) = engine();
    let answer = identify(&studio, project.request(json!({ "action": "reset" })));
    assert_eq!(
        answer["result"]["statusSentence"],
        json!("New identification study started."),
        "{answer}"
    );
}

#[test]
fn reset_is_refused_while_the_loop_has_a_running_or_unconfirmed_operation() {
    let project = Project::new("reset-refused");
    let section = project.keep_identify_result(PLANT_INPUT);
    project.journal_an_operation();
    let (studio, _updates) = engine();
    let answer = identify(&studio, project.request(json!({ "action": "reset" })));
    assert_eq!(
        refusal(&answer),
        ("This loop has an unfinished operation.", "wait"),
        "{answer}"
    );
    let kept = project.working_study().section::<IdentifySection>();
    assert_eq!(
        kept.expect("the section"),
        Some(section),
        "the study stays as it was"
    );
}

#[test]
fn open_reads_the_identify_result_and_a_model_of_other_signals_is_no_longer_current() {
    let project = Project::new("open");
    let (studio, _updates) = engine();
    project.keep_identify_result(PLANT_INPUT);
    assert_eq!(
        project.opened_stages(&studio),
        (Some("complete".to_string()), Some("ready".to_string())),
        "the current model completes Identify and lets Tune run"
    );
    project.keep_identify_result("P1.EarlierOutput");
    assert_eq!(
        project.opened_stages(&studio),
        (Some("ready".to_string()), Some("needs-data".to_string())),
        "a model of other signals is no longer current (A8.4); its recording stays"
    );
}

/// `control-studio-final-wiring.test.ts`, tests 7 and 8, for the study's own keep: each stopped recording becomes a recording
/// file of the working study with the verdict that labels it, and a keep the study cannot make keeps nothing, so the run
/// releases nothing.
#[test]
fn the_study_keeps_each_stopped_recording_with_its_verdict_and_reports_a_keep_it_cannot_make() {
    let project = Project::new("retention");
    let run = recorded_run();
    let evidence = recording_evidence(&run, None);
    let source = EvidenceSource::SimulatedPlant;
    let mut retention =
        StudyRetention::new(&project.root, LOOP_ID, source, project.working_study());
    retention.retain(&run, &evidence).expect("kept");
    let file_id = retention
        .kept_file_id("recording-1")
        .expect("the file of the run")
        .to_string();
    assert_eq!(retention.study().recordings, [file_id.as_str()]);
    assert_eq!(
        project.working_study().recordings,
        [file_id.as_str()],
        "the study file lists the recording"
    );
    let file = storage::read_recording(&project.root, LOOP_ID, &file_id).expect("it reads back");
    assert_eq!(file.run, run, "the run as the runtime returned it");
    assert_eq!(file.source, source);
    assert_eq!(file.evidence.completeness, Completeness::Complete);

    let blocked = Project::new("retention-blocked");
    let folder = study_directory(&blocked.root, LOOP_ID);
    fs::create_dir_all(&folder).expect("the study folder");
    fs::write(folder.join("recordings"), "").expect("a file where the recordings go");
    let mut refused = StudyRetention::new(&blocked.root, LOOP_ID, source, blocked.working_study());
    let failure = refused
        .retain(&run, &evidence)
        .expect_err("nothing can be kept");
    assert!(
        failure
            .message
            .starts_with("The study could not be stored: "),
        "{failure:?}"
    );
    assert!(
        refused.study().recordings.is_empty(),
        "a keep that failed lists nothing"
    );
    assert_eq!(refused.kept_file_id("recording-1"), None);
}

/// Starts the engine's cancel of the journaled operation `JOURNALED` of the project's loop, on the runtime at `endpoint`.
fn cancel(studio: &ControlStudio, project: &Project, endpoint: String) -> OperationStarted {
    let connection = RuntimeConnection {
        endpoint,
        token: None,
        runtime_id: None,
    };
    let loop_definition = project.loop_definition();
    cancel_loop(
        studio.operations(),
        studio.pending_releases(),
        studio.loop_claims(),
        CancelRequest {
            connection: &connection,
            project_root: &project.root,
            storage_folder: &project.storage(),
            loop_definition: &loop_definition,
            operation_id: JOURNALED,
        },
    )
    .expect("the cancel starts")
}

#[test]
fn cancel_starts_an_operation_of_this_engine_on_the_loops_journal() {
    let project = Project::new("cancel");
    project.journal_an_operation();
    let (studio, updates) = engine();
    let started = cancel(&studio, &project, closed_endpoint());
    let update = final_update(&updates);
    assert_eq!(update.operation_id, started.operation_id);
    assert_eq!(update.command, "cancel");
    assert_eq!(
        failure_action(update),
        Some(NextAction::ConnectRuntime),
        "the cancel reattaches through the runtime"
    );
    assert_eq!(
        project.journaled(),
        [JOURNALED],
        "without the runtime the journal stays"
    );
}

/// E11.4 and E11.6: a lone recording owner is cancelled by stopping its recording through the runtime, and the owner is cleared
/// only once the recording is kept. A runtime that refuses the stop keeps the owner journaled, and the cancel fails with the
/// runtime's own words.
#[test]
fn a_cancel_whose_runtime_refuses_the_stop_keeps_the_owner_journaled_and_fails_with_its_words() {
    let project = Project::new("cancel-refused");
    project.journal_an_operation();
    let (studio, updates) = engine();
    cancel(&studio, &project, simulated_runtime());
    let update = final_update(&updates);
    assert_eq!(update.state, OperationState::Failed);
    let Some(failure) = update.failure else {
        panic!("the cancel fails");
    };
    assert_eq!(failure.message, "Not in this test.");
    assert_eq!(
        (failure.cause, failure.next_action),
        (Cause::Runtime, NextAction::None)
    );
    assert_eq!(
        project.journaled(),
        [JOURNALED],
        "the owner stays journaled"
    );
}

/// §0.1b, D6: a named operation that the loop's journal does not hold is refused before anything starts, and the refusal names
/// it. Nothing is stopped and the journal stays as it is.
#[test]
fn a_cancel_of_an_operation_the_journal_does_not_hold_is_refused_before_anything_starts() {
    let project = Project::new("cancel-unknown");
    project.journal_an_operation();
    let (studio, _updates) = engine();
    let connection = RuntimeConnection {
        endpoint: closed_endpoint(),
        token: None,
        runtime_id: None,
    };
    let loop_definition = project.loop_definition();
    let refused = cancel_loop(
        studio.operations(),
        studio.pending_releases(),
        studio.loop_claims(),
        CancelRequest {
            connection: &connection,
            project_root: &project.root,
            storage_folder: &project.storage(),
            loop_definition: &loop_definition,
            operation_id: "recording-9",
        },
    )
    .expect_err("the operation is not journaled");
    assert_eq!(
        refused.message,
        "Operation recording-9 is not journaled for loop control-pid-commissioning."
    );
    assert_eq!(
        (refused.cause, refused.next_action),
        (Cause::Settings, NextAction::None)
    );
    assert!(nothing_started(&studio), "nothing was started");
    assert_eq!(project.journaled(), [JOURNALED], "the journal is untouched");
}

/// E11.1: `identify` and `resume` reach the runtime, so without its connection they are refused at once, before anything starts
/// and before anything is journaled, with the action that connects the runtime.
#[test]
fn a_live_action_or_a_resume_without_a_runtime_connection_is_refused_before_anything_starts() {
    let project = Project::new("no-connection");
    let (studio, _updates) = engine();
    for action in [live(automatic()), json!({ "action": "resume" })] {
        let answer = identify(&studio, project.request_without_connection(action));
        assert_eq!(
            refusal(&answer),
            (
                "Start the runtime before running a live action.",
                "connect-runtime"
            ),
            "{answer}"
        );
    }
    assert!(nothing_started(&studio), "nothing was started");
    assert_eq!(
        project.journaled(),
        Vec::<String>::new(),
        "nothing is journaled"
    );
}

/// E11.1: a reset and an opened recording need no runtime, so a request without a connection is not refused for one. A reset
/// answers at once with the stages of the new working study; an opened recording is refused only for its own reason.
#[test]
fn a_reset_and_an_open_recording_need_no_runtime_connection() {
    let project = Project::new("no-runtime-needed");
    let (studio, _updates) = engine();
    let reset = identify(
        &studio,
        project.request_without_connection(json!({ "action": "reset" })),
    );
    assert_eq!(
        status_of(&reset["result"]["stages"], "Identify"),
        Some("needs-data"),
        "{reset}"
    );
    let other = format!("rec-{}", "0".repeat(32));
    let refit = identify(
        &studio,
        project.request_without_connection(open_recording(&other)),
    );
    let (message, next_action) = refusal(&refit);
    assert!(
        message.starts_with(&format!("Recording {other} cannot be fitted again")),
        "{refit}"
    );
    assert_eq!(next_action, "identify", "{refit}");
    assert!(nothing_started(&studio), "no recording is fitted");
}

/// A8.5: one operation per loop at a time in one engine. A request on a loop the engine works on is refused at once, and the
/// loop is free again when that work ends: first a held claim, then an identification that ends with the runtime's failure.
#[test]
fn a_loop_this_engine_works_on_refuses_another_request_until_that_work_ends() {
    let project = Project::new("busy");
    let (studio, updates) = engine();
    let loop_definition = project.loop_definition();
    let held = studio
        .loop_claims()
        .claim(
            &loop_definition.project_id,
            &loop_definition.id,
            "Identify plant",
        )
        .expect("a free loop");
    for action in [
        live(automatic()),
        json!({ "action": "resume" }),
        json!({ "action": "reset" }),
    ] {
        let answer = identify(&studio, project.request(action));
        assert_eq!(
            refusal(&answer),
            (
                "Identify plant is running in this engine on this loop.",
                "wait"
            ),
            "{answer}"
        );
    }
    assert!(nothing_started(&studio), "nothing was started");
    drop(held);
    let started = identify(&studio, project.request(live(automatic())));
    assert_eq!(started["result"]["operationId"], "operation-1", "{started}");
    final_update(&updates);
    let reset = identify(&studio, project.request(json!({ "action": "reset" })));
    assert!(
        reset.get("result").is_some(),
        "the ended operation let the loop go: {reset}"
    );
}

#[test]
fn import_recording_refuses_a_file_that_is_not_a_saved_result_with_its_sentence_and_starts_nothing()
{
    let project = Project::new("import-recording");
    let (studio, _updates) = engine();
    let answer = identify(
        &studio,
        project.request(json!({ "action": "importRecording", "json": "{" })),
    );
    assert_eq!(
        refusal(&answer),
        (
            "Choose a valid Control Studio recording or result JSON file.",
            "none"
        ),
        "a refused file names no next action: {answer}"
    );
    assert!(nothing_started(&studio), "the check starts no operation");
}

#[path = "identify_import_tests.rs"]
mod import_tests;

#[path = "identify_context_tests.rs"]
mod context_tests;

/// A complete run of two consecutive samples of the measurement, 30.5 and 31, made after the run of `recorded_run`.
fn newer_recorded_run() -> RecordedRun {
    serde_json::from_value(json!({
        "recording_id": "recording-2", "loop_revision": "sha256:loop-1",
        "bindings": [{ "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" }],
        "stop_reason": "sample_limit",
        "samples": [
            { "scan": 1, "runtime_time_ns": 100_000_000, "captured_at_ms": 1, "values": [30.5], "forced": [false], "quality": ["good"] },
            { "scan": 2, "runtime_time_ns": 200_000_000, "captured_at_ms": 2, "values": [31.0], "forced": [false], "quality": ["good"] }
        ],
        "acquisition": { "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": 2,
            "scanRange": { "first": 1, "last": 2 }, "missingScanRanges": [], "droppedObservedSamples": 0,
            "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "" }
    }))
    .expect("a newer recorded run")
}

#[test]
fn the_csv_export_writes_the_recording_of_the_current_result_or_the_one_it_names() {
    // `control-studio-validate-apply.test.ts`, "exports the current completed recording instead of an older working draft" (specification
    // 35, §0.1a and B10.5): the engine reads the study when it is asked, so a request that names no recording exports the recording the
    // study's current Identify result was fitted from, not an older one, and a request that names a recording exports that recording as
    // it is kept, however many results have been kept since.
    use trust_control::export::{ExportFormat, ExportRequest};
    let project = Project::new("export-current-recording");
    project.keep_identify_result(PLANT_INPUT);
    let older = recording().id().expect("the older recording's identifier");
    let run = newer_recorded_run();
    let evidence = recording_evidence(&run, None);
    let newer = RecordingFile::new(LOOP_ID, EvidenceSource::SimulatedPlant, run, evidence);
    let section = IdentifySection::new(
        IdentifyMethod::Live(ControlExperimentKind::MvStep),
        EvidenceSource::SimulatedPlant,
        IdentifySignals {
            inputs: vec![PLANT_INPUT.to_string()],
            outputs: vec![MEASUREMENT.to_string()],
            setpoint: None,
        },
        &newer.id().expect("the newer recording's identifier"),
        None,
        fitted_model(),
        presentation(),
    );
    storage::save_identify(&project.root, &project.working_study(), &section, &[newer])
        .expect("the newer Identify result is kept");
    let measurements = |recording_id: Option<&str>| -> Vec<String> {
        let answer = crate::export_action::answer(ExportRequest {
            project_root: project.root.display().to_string(),
            runtime_id: RUNTIME_ID.to_string(),
            loop_id: LOOP_ID.to_string(),
            format: ExportFormat::Csv,
            recording_id: recording_id.map(str::to_string),
        })
        .expect("the CSV export");
        answer["text"]
            .as_str()
            .expect("the CSV text")
            .lines()
            .skip(1)
            .map(|row| {
                row.split(',')
                    .nth(4)
                    .expect("the measurement cell")
                    .to_string()
            })
            .collect()
    };
    assert_eq!(measurements(None), ["30.5", "31"]);
    assert_eq!(measurements(Some(older.as_str())), ["20.5", "21"]);
}
