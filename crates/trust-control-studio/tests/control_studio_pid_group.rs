//! Control Studio's PID group through the engine's library entry on a real runtime: Tune designs both members, Validate runs and gives
//! its rows, Apply writes the group's parameters and Restore writes them back, while a third PID of the program stays as it was
//! (specification 35, C7, §0.1a, B4.1, B5.4, B6.1 to B6.4; checklist V7 wave 5b, rule 13).
//!
//! It is the native replacement of the VS Code journey "Control Studio native PID group tunes validates applies and restores two actual
//! library PIDs without changing an unselected PID" (`control-studio-pid-group-native.integration.test.ts`), which drove the TypeScript
//! route that the engine now owns. The project is the fixture `editors/vscode/src/test/fixtures/control-studio-pid-group`: its program
//! `Main` holds two library `Control.PID` instances, `P1.Controller` and `P1.Secondary`, which form the group, and its program
//! `Unselected` holds `P3.Controller`, a third `Control.PID` that the group does not name. Each controller drives its own first-order
//! plant (`simulation.toml`), and the runtime runs at real time (`time_scale = 1`).
//!
//! The copy's loop file is the fixture's, with one declaration changed: the program's own law is the `decentralized-pid` seed and has no
//! tuning axes. The fixture declares axes and no seed, which the engine reads as a finite search (C5), not as a PID group, because Tune
//! checks the search form before the group. The fixture itself is not changed.
//!
//! The model is declared, as the journey's own equations were: each path is the fixture's plant (gain 1.5, time constant 4 s, delay 0.5 s),
//! and the two paths do not interact. It enters with `identify` (`importModel`, B3.7) and names its evidence, so Validate runs no
//! independent model check. The test proves the group's ownership and the runtime's writes; it does not prove how well the plant is
//! identified.
//!
//! The journey, in the engineer's order:
//! - the requirements: a setpoint change of 1 °C, a settling band of 0.03 °C within 18 s, and a validation duration of 24 s;
//! - Tune designs both members: the answer names the group, each member's path and gains, and the values it would write;
//! - Validate runs and gives its rows, with a reference case for each controlled output, and the record stays as it was;
//! - the review of Apply names the group's six parameter symbols and no other, with the values in force and the values it would write;
//! - Apply writes those values, the runtime confirms them, and the loop is in Auto;
//! - the review of Restore names the values from before Apply; Restore writes them back, the runtime confirms them, and the loop is in
//!   Manual.
//!
//! The unselected PID's gains are read from the runtime before the journey and after each write; they never change.
//!
//! A check that does not block the next step is recorded and the journey goes on. A step that the next ones need ends the run at once,
//! with the findings so far. Run time: the validation duration of the Apply observation, 24 s at real time, plus the verbs' own work.
//!
//! The two other tests are Identify on the same group (specification 35, C7 "Identify", A6.2, A6.3, B3.4; annex E6.4 and E6.9). Their
//! copy of the program starts in Auto with the members' Manual commands at 30 % and 40 %, so that an open-loop method must move the whole
//! group to Manual and return it:
//! - the mode change alone drives `IdentificationMode`, the engine's own, against the runtime: one transfer of the group's binding to
//!   Manual, whose tracking the runtime accepts only when it names the second member's setpoint, Manual command and output
//!   (`validate_declared_states`); then the write-back in Manual of both members' setpoints, as the end of a campaign does, and the
//!   return to Auto from the outputs the group holds, whose entry sets each Manual command to its output (C1). It needs no experiment,
//!   so it takes a few seconds at real time;
//! - the campaign runs `controlStudio/identify` on the group, whose inputs declare an identification step of their own (`experimentStep`):
//!   it steps the two plant inputs in turn in one recording, fits the multivariable model, and returns the group to Auto.
//!
//! Run it on the builder, after `cargo build -p trust-runtime --bin trust-runtime`:
//! `cargo test -p trust-control-studio --test control_studio_pid_group`. The file is Unix only, as the runtime's endpoint is a Unix socket.
#![cfg(unix)]

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use std::thread::sleep;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{json, Value};
use trust_control::commissioning::commissioning_result::TransferStatus;
use trust_control::commissioning::transfer::{TransferBinding, TransferValue};
use trust_control::identification::acquisition_settings::{
    declared_settings, AcquisitionMethod, AcquisitionSettings,
};
use trust_control::identification::mode_transfer::mode_binding;
use trust_control::loop_definition::{LoopDefinition, ModeBinding, Parameter, ParameterBinding};
use trust_control::messages::{
    ApplicationRequest, IdentifyAction, IdentifyRequest, OperationState, OperationUpdate,
    TuneRequest, ValidateRequest,
};
use trust_control::runtime::RuntimeConnection;
use trust_control_studio::commands::{self, ControlStudio};
use trust_control_studio::journal::Journal;
use trust_control_studio::operation::UpdateSink;
use trust_control_studio::workflow::identify_mode::{mode_journaled, IdentificationMode};

use common::engine::{close, journal_of, read_text, same_values, Findings, Lab, RUNTIME_ID};
use common::{copy_dir, RemoveDir};

/// The fixture that the copy is made of (see the module documentation).
const FIXTURE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../editors/vscode/src/test/fixtures/control-studio-pid-group"
);
/// The loop's file, in the fixture and in the copy.
const LOOP_FILE: &str = "control-loop.json";
/// Real time: the runtime runs at the speed of its clock, as `simulation.toml` of the fixture sets it.
const TIME_SCALE: u32 = 1;
/// The members of the group, in the order of the loop's channels.
const MEMBERS: [&str; 2] = ["P1.Controller", "P1.Secondary"];
/// The program of the unselected PID: `P3.Controller` is the fixture's third `Control.PID`, which the group does not name.
const UNSELECTED: &str = "P3";
/// The evidence that the declared model names: a validated model must name its evidence.
const EVIDENCE: &str = "the fixture's simulation.toml plant equations";
/// The setpoints of the members, which the program leaves at the plants' resting point of 20 °C.
const SETPOINTS: [&str; 2] = ["P1.Setpoint", "P1.SecondarySetpoint"];
/// The Manual commands of the members.
const COMMANDS: [&str; 2] = ["P1.ManualOutput", "P1.SecondaryManualOutput"];
/// The outputs of the members.
const OUTPUTS: [&str; 2] = ["P1.ManipulatedValue", "P1.SecondaryManipulatedValue"];
/// The Manual commands that the program of the mode test holds beside Auto, in member order (`prepare_source_in_auto`). They lie above
/// every output the group holds in these tests, from 0 % to the campaign's [`IDENTIFY_STEP`], so a Manual command that the Auto entry set
/// to a held output is never taken for one written back.
const FOUND_COMMANDS: [f64; 2] = [30.0, 40.0];
/// The longest that one operation may run; the Apply observation lasts the validation duration of 24 s at real time, and the Identify
/// campaign of the group steps two plant inputs in turn within `IDENTIFY_SECONDS`.
const LIMIT: Duration = Duration::from_secs(300);
/// The maximum duration of the Identify campaign of the group, which its loop declares (B3.4): the baseline, two steps held until they
/// settle (about 17 s each for the fixture's plants) and the restorations fit within it.
const IDENTIFY_SECONDS: f64 = 90.0;
/// The identification step of each plant input of the group in the Identify campaign, in %: the plant's gain of 1.5 makes it 15 degC.
const IDENTIFY_STEP: f64 = 10.0;
/// How often the test looks at an operation that runs.
const POLL: Duration = Duration::from_millis(25);

/// An engine that discards its operation updates: the test reads each operation's final state from `Operations::status`.
struct Silent;

impl UpdateSink for Silent {
    fn send(&self, _update: OperationUpdate) {}
}

/// Control Studio's library entry for one loop of the lab. Each request names the project, the journal folder, the loop and the
/// runtime, as the command line names them.
struct Engine {
    studio: ControlStudio,
    project: String,
    journal: String,
    loop_id: String,
    connection: RuntimeConnection,
}

impl Engine {
    fn new(lab: &Lab) -> Self {
        Self {
            studio: ControlStudio::new(Arc::new(Silent)),
            project: lab.project.to_string_lossy().into_owned(),
            journal: lab.journal.to_string_lossy().into_owned(),
            loop_id: lab.loop_definition.id.clone(),
            connection: lab.connection.clone(),
        }
    }

    /// Runs `method` with `request`, and answers as the command line does: `{"result": …}` or `{"failure": …}`. A method that starts an
    /// operation is followed to its end, so its answer is the operation's final result.
    fn call(&self, method: &str, request: &Value) -> Value {
        let answer = self
            .studio
            .execute(method, std::slice::from_ref(request))
            .unwrap_or_else(|| panic!("the engine does not answer {method}"));
        let operation = answer
            .pointer("/result/operationId")
            .and_then(Value::as_str)
            .map(str::to_owned);
        match operation {
            Some(operation) => self.finish(&operation),
            None => answer,
        }
    }

    /// Waits until operation `operation` ends, and answers with its result or its failure.
    fn finish(&self, operation: &str) -> Value {
        let deadline = Instant::now() + LIMIT;
        loop {
            let update = self
                .studio
                .operations()
                .status(operation)
                .expect("the engine knows the operation it started");
            match update.state {
                OperationState::Running => {}
                OperationState::Finished => return json!({ "result": update.result }),
                OperationState::Failed => return json!({ "failure": update.failure }),
                OperationState::Cancelled => return json!({ "cancelled": true }),
            }
            assert!(
                Instant::now() < deadline,
                "{operation} did not end within {} s; its last progress: {:?}",
                LIMIT.as_secs(),
                update.progress
            );
            sleep(POLL);
        }
    }

    /// The Tune request of the loop: Automatic, with no settings. It names no runtime, so Tune designs without the comparison (B4.1).
    fn tune_request(&self) -> Value {
        json_of(&TuneRequest {
            project_root: self.project.clone(),
            runtime_id: RUNTIME_ID.to_owned(),
            loop_id: self.loop_id.clone(),
            storage_folder: self.journal.clone(),
            method: None,
            choice: None,
            structure: None,
            pins: None,
            settings: None,
            connection: None,
        })
    }

    /// The Validate request of the loop, with the runtime whose isolated simulations it runs (E8.1).
    fn validate_request(&self) -> Value {
        json_of(&ValidateRequest {
            project_root: self.project.clone(),
            runtime_id: RUNTIME_ID.to_owned(),
            loop_id: self.loop_id.clone(),
            storage_folder: self.journal.clone(),
            connection: Some(self.connection.clone()),
        })
    }

    /// The Apply or Restore request: the review when `review` is absent, else the confirmation of the review that it names.
    fn application_request(&self, review: Option<&str>) -> Value {
        json_of(&ApplicationRequest {
            project_root: self.project.clone(),
            runtime_id: RUNTIME_ID.to_owned(),
            loop_id: self.loop_id.clone(),
            storage_folder: self.journal.clone(),
            connection: Some(self.connection.clone()),
            review: review.map(str::to_owned),
            physical_approved: false,
            resume: false,
        })
    }

    /// The Identify request that imports `model` as the study's model (B3.7). It starts no campaign and writes nothing to the runtime.
    fn import_request(&self, model: &Value) -> Value {
        json_of(&IdentifyRequest {
            project_root: self.project.clone(),
            runtime_id: RUNTIME_ID.to_owned(),
            loop_id: self.loop_id.clone(),
            storage_folder: self.journal.clone(),
            connection: None,
            runtime_mode: None,
            action: IdentifyAction::ImportModel {
                json: model.to_string(),
            },
        })
    }

    /// The Identify request that runs a live campaign with `settings` (B3.4). It names no review: the target is a simulation, where
    /// pressing the action is the confirmation (A2.5).
    fn identify_run_request(&self, settings: AcquisitionSettings) -> Value {
        json_of(&IdentifyRequest {
            project_root: self.project.clone(),
            runtime_id: RUNTIME_ID.to_owned(),
            loop_id: self.loop_id.clone(),
            storage_folder: self.journal.clone(),
            connection: Some(self.connection.clone()),
            runtime_mode: None,
            action: IdentifyAction::Identify {
                settings: Some(settings),
                review: None,
                physical_approved: false,
            },
        })
    }

    /// The Inputs request that records `change`, the requirements record that the page sends, marked by its `referenceChange` (B4.3).
    fn inputs_request(&self, change: &Value) -> Value {
        json!({
            "projectRoot": self.project,
            "runtimeId": RUNTIME_ID,
            "loopId": self.loop_id,
            "change": change,
        })
    }
}

/// The JSON of a typed request, as the engine reads it.
fn json_of<T: serde::Serialize>(request: &T) -> Value {
    serde_json::to_value(request).expect("the request serializes")
}

/// Copies the fixture to `folder`, with its loop file changed to the declaration that the engine serves for the group: the program's own
/// law is the `decentralized-pid` seed (C7), and it declares no tuning axes. Everything else is the fixture's.
fn prepare_source(folder: &Path) {
    copy_dir(Path::new(FIXTURE), folder);
    let path = folder.join(LOOP_FILE);
    let mut loop_file: Value =
        serde_json::from_str(&read_text(&path)).expect("the fixture's loop file is JSON");
    let project = &mut loop_file["controller"]["project"];
    project["seed"] = json!({ "kind": "decentralized-pid", "family": "PI" });
    project["axes"] = json!([]);
    fs::write(&path, format!("{loop_file:#}")).expect("the copy's loop file is written");
}

/// Copies the fixture to `folder` as [`prepare_source`] does, and starts its program `Main` in Auto with both setpoints at the plants'
/// resting point of 20 degC and its members' Manual commands at 30 % and 40 %. The fixture starts in Manual with the commands at rest at
/// zero, which leaves a mode change nothing to return. The program sets the located variables in its first scan, as the lessons'
/// programs do, and never writes a Manual command again, so the values it holds beside Auto are the values Identify finds. Found in
/// Auto, they are not written back: the Auto entry sets each to the output its member holds (A6.2, C1).
fn prepare_source_in_auto(folder: &Path) {
    prepare_source(folder);
    let path = folder.join("src").join("main.st");
    let program = read_text(&path);
    let started = program
        .replacen("AutoMode : BOOL := FALSE;", "AutoMode : BOOL := TRUE;", 1)
        .replacen(
            "\n    InitialState.Setpoint := Setpoint;",
            "\n    Setpoint := LREAL#20.0;\n    ManualOutput := LREAL#30.0;\n    SecondarySetpoint := LREAL#20.0;\n    SecondaryManualOutput := LREAL#40.0;\n    InitialState.Setpoint := Setpoint;",
            1,
        );
    assert!(
        started.contains("AutoMode : BOOL := TRUE;")
            && started.contains("\n    SecondaryManualOutput := LREAL#40.0;"),
        "the fixture's program declares the variables and the first scan that this test starts from"
    );
    fs::write(&path, started).expect("the copy's program is written");
}

/// Copies the fixture to `folder` as [`prepare_source_in_auto`] does, and gives its loop what a group's campaign needs: a maximum
/// duration of [`IDENTIFY_SECONDS`], and an identification step of [`IDENTIFY_STEP`] for each plant input, so that Identify steps each
/// member's Manual command in turn (C7, B3.4). The plants have no dead time in the copy: this test proves the campaign's wiring on the
/// group, and the multichannel ARX fit of a delayed plant is `trust-control-math`'s to prove. Everything else is the fixture's.
fn prepare_source_for_identify(folder: &Path) {
    prepare_source_in_auto(folder);
    let simulation_path = folder.join("simulation.toml");
    let simulation = read_text(&simulation_path);
    assert!(
        simulation.contains("dead_time_s = 0.5"),
        "the fixture's plants declare their dead time"
    );
    fs::write(
        &simulation_path,
        simulation.replace("dead_time_s = 0.5", "dead_time_s = 0.0"),
    )
    .expect("the copy's simulation is written");
    let path = folder.join(LOOP_FILE);
    let mut loop_file: Value =
        serde_json::from_str(&read_text(&path)).expect("the copy's loop file is JSON");
    loop_file["experiment"]["durationSeconds"] = json!(IDENTIFY_SECONDS);
    let inputs = loop_file["channels"]["inputs"]
        .as_array_mut()
        .expect("the group's loop declares its plant inputs");
    for input in inputs {
        input["experimentStep"] = json!(IDENTIFY_STEP);
    }
    fs::write(&path, format!("{loop_file:#}")).expect("the copy's loop file is written");
}

/// The number that `status` reports for each of `symbols`, NaN for a symbol it does not report as a number.
fn numbers(status: &TransferStatus, symbols: &[&str]) -> Vec<f64> {
    symbols
        .iter()
        .map(|symbol| {
            status
                .values
                .get(*symbol)
                .copied()
                .and_then(TransferValue::as_number)
                .unwrap_or(f64::NAN)
        })
        .collect()
}

/// Whether `actual` and `expected` are the same length and each pair of numbers is the same to the eight significant digits that the
/// engine writes values with.
fn all_close(actual: &[f64], expected: &[f64]) -> bool {
    actual.len() == expected.len()
        && actual
            .iter()
            .zip(expected)
            .all(|(measured, wanted)| close(*measured, *wanted))
}

/// What a group returned to the mode it was found in shows (A6.2, C1): it is in Auto and both setpoints are as they were found. A group
/// found in Auto enters Auto from the outputs it holds, the entry setting each Manual command to that output, so the Manual commands
/// `prepare_source_in_auto` made 30 % and 40 % are not written back (specification 35, Identify, Group, 29 October 2026). Every output
/// the group holds lies between 0 %, the plants' resting input where the mode test leaves them, and the campaign's step of
/// [`IDENTIFY_STEP`], wherever in its return the entry comes (member 2 held 2.5 % on macOS, 30 October 2026); so both outputs and both
/// Manual commands stay below half the found commands. Catches the former write-back, which first stepped each output to its stale
/// Manual command (lesson 30's heater went from 47.55 % to 75 %).
fn check_returned_to_auto(
    findings: &mut Findings,
    lab: &Lab,
    binding: &TransferBinding,
    found: &TransferStatus,
) {
    let returned = lab
        .client()
        .transfer_status(binding)
        .expect("the runtime reports the group's transfer status (E6.3)");
    findings.check(
        lab.is_auto() == Some(true),
        "the group is in Auto again, the mode it was found in (A6.2)",
    );
    findings.check(
        all_close(&numbers(&returned, &SETPOINTS), &numbers(found, &SETPOINTS)),
        format!(
            "both setpoints are as they were found (C1): {:?} against {:?}",
            numbers(&returned, &SETPOINTS),
            numbers(found, &SETPOINTS)
        ),
    );
    let outputs = numbers(&returned, &OUTPUTS);
    findings.check(
        outputs
            .iter()
            .zip(FOUND_COMMANDS)
            .all(|(output, found_command)| *output < 0.5 * found_command),
        format!("both members' outputs stay where the group held them, not at the found Manual commands (C1): {outputs:?}"),
    );
    let commands = numbers(&returned, &COMMANDS);
    findings.check(
        commands
            .iter()
            .zip(FOUND_COMMANDS)
            .all(|(command, found_command)| *command < 0.5 * found_command),
        format!("the Auto entry set both Manual commands to the held outputs, not back to 30 and 40 (C1): {commands:?}"),
    );
}

/// The declared model of the group, in the JSON of the model contract (N4.1). Each member's path is the fixture's plant, a first-order
/// process of gain 1.5, time constant 4 s and delay 0.5 s, and the paths do not interact. The channels are the loop's model channels: `u1`
/// and `u2` drive the heaters, and `y1` and `y2` measure the process values, in member order. The operating point is the fixture's resting
/// point: both inputs at 0 % and both outputs at 20 °C.
fn model() -> Value {
    let channel = |id: &str, unit: &str| json!({ "id": id, "unit": unit, "operatingRange": { "minimum": 0, "maximum": 100 } });
    json!({
        "kind": "continuous-state-space",
        "revision": "model:pid-group-fixture",
        "source": "provided",
        "validation": "validated",
        "evidence": [EVIDENCE],
        "delaySeconds": 0.5,
        "inputs": [channel("u1", "%"), channel("u2", "%")],
        "outputs": [channel("y1", "°C"), channel("y2", "°C")],
        "states": [channel("x1", "°C"), channel("x2", "°C")],
        "a": [[-0.25, 0.0], [0.0, -0.25]],
        "b": [[0.375, 0.0], [0.0, 0.375]],
        "c": [[1.0, 0.0], [0.0, 1.0]],
        "d": [[0.0, 0.0], [0.0, 0.0]],
        "operatingPoint": { "inputs": [0.0, 0.0], "outputs": [20.0, 20.0] }
    })
}

/// The loop with the parameter record of the unselected PID, `P3`: the binding of its three gains (`P1.Parameters.*` renamed), in its
/// own program's mode. The runtime reads a record through the loop's binding, so this copy of the loop reads `P3` the same way.
fn unselected_loop(loop_definition: &LoopDefinition) -> LoopDefinition {
    let group = loop_definition
        .parameter_binding
        .as_ref()
        .expect("the loop declares its parameter record");
    let parameters = group
        .parameters
        .iter()
        .filter(|parameter| parameter.symbol.starts_with("P1.Parameters."))
        .map(|parameter| Parameter {
            symbol: parameter.symbol.replacen("P1.", "P3.", 1),
            ..parameter.clone()
        })
        .collect();
    let mut unselected = loop_definition.clone();
    unselected.parameter_binding = Some(ParameterBinding {
        loop_id: group.loop_id.clone(),
        task_name: group.task_name.clone(),
        parameters,
        mode: ModeBinding {
            symbol: format!("{UNSELECTED}.AutoMode"),
            manual_value: false,
        },
    });
    unselected
}

/// The unselected PID's gains, as the runtime reports them (`control.parameters.status`).
fn unselected_values(lab: &Lab) -> BTreeMap<String, f64> {
    lab.client()
        .parameter_values(&unselected_loop(&lab.loop_definition))
        .expect("the runtime reports the unselected PID's gains")
}

/// The numbers of a JSON object, by key; a key whose value is not a number is left out.
fn numbers_of(object: &Value) -> BTreeMap<String, f64> {
    object
        .as_object()
        .map(|map| {
            map.iter()
                .filter_map(|(key, value)| Some((key.clone(), value.as_f64()?)))
                .collect()
        })
        .unwrap_or_default()
}

/// The `field` value (`running` or `next`) of each parameter of a review, by symbol.
fn review_values(review: &Value, field: &str) -> BTreeMap<String, f64> {
    review["parameters"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    Some((item["symbol"].as_str()?.to_owned(), item[field].as_f64()?))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The symbols of the parameters that a review names.
fn review_symbols(review: &Value) -> BTreeSet<String> {
    review["parameters"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["symbol"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// The text of a string field of `value`; an empty text when the field is missing.
fn text_of(value: &Value, field: &str) -> String {
    value[field].as_str().unwrap_or_default().to_owned()
}

/// Whether `symbol` holds one value in `before` and another in `after`.
fn changed(before: &BTreeMap<String, f64>, after: &BTreeMap<String, f64>, symbol: &str) -> bool {
    matches!(
        (before.get(symbol), after.get(symbol)),
        (Some(old), Some(new)) if !close(*old, *new)
    )
}

/// The whole journey of Tune, Validate, Apply and Restore on the PID group of the fixture (C7), in the order described in the module
/// documentation.
#[test]
#[allow(clippy::too_many_lines)] // One journey through the engine, in the order of a commissioning; each step needs the one before.
fn pid_group_tunes_validates_applies_and_restores_two_members_without_changing_the_third_pid() {
    let source =
        std::env::temp_dir().join(format!("trust-cs-pid-group-source-{}", std::process::id()));
    let _ = fs::remove_dir_all(&source);
    let _source = RemoveDir(source.clone());
    prepare_source(&source);
    let lab = Lab::start(
        "pid-group",
        source.to_str().expect("the copy's folder is UTF-8"),
        LOOP_FILE,
        TIME_SCALE,
    );
    let engine = Engine::new(&lab);
    let mut findings = Findings::default();

    // The group as the loop declares it: two members of program P1, each on its own input and output, sharing one mode.
    let group = lab
        .loop_definition
        .controller
        .group
        .clone()
        .expect("the loop declares its PID group");
    let instances: Vec<&str> = group
        .members
        .iter()
        .map(|member| member.instance.as_str())
        .collect();
    findings.require(
        instances == MEMBERS,
        format!("the group names its two members in order: {instances:?}"),
    );
    let symbols: BTreeSet<String> = group
        .members
        .iter()
        .flat_map(|member| member.parameter_symbols.iter().cloned())
        .collect();
    let record: BTreeSet<String> = lab
        .loop_definition
        .parameter_binding
        .as_ref()
        .map(|binding| {
            binding
                .parameters
                .iter()
                .map(|parameter| parameter.symbol.clone())
                .collect()
        })
        .unwrap_or_default();
    findings.require(
        symbols.len() == 6 && symbols == record,
        format!("the members' gains are the six parameters of the record: {symbols:?}"),
    );

    // Before the journey: the group's record and the unselected PID's gains, as the runtime holds them.
    let before = lab.parameter_values();
    findings.require(
        before.keys().cloned().collect::<BTreeSet<_>>() == symbols,
        format!("the runtime reports the group's six gains: {before:?}"),
    );
    let unselected_before = unselected_values(&lab);
    findings.require(
        unselected_before.len() == 3,
        format!("the runtime reports the three gains of {UNSELECTED}: {unselected_before:?}"),
    );
    findings.check(
        lab.is_auto() == Some(false),
        "the program starts the loop in Manual (P1.AutoMode is FALSE)",
    );

    // The requirements of the study (B4.3, C7): for each controlled output a setpoint change of 1 °C and a settling band of 0.03 °C within
    // 18 s, and a validation run of 24 s, which is longer than the settling time. The Requirements editor sends each output's rows as typed
    // and the engine assembles the record; each member is designed to its own output's requirements.
    let output = |channel: &str| {
        json!({
            "channel": channel,
            "referenceChange": 1.0,
            "maximumError": 0.15,
            "settlingBand": 0.03,
            "settlingTimeSeconds": 18.0,
        })
    };
    let change = json!({
        "outputRequirements": [output("y1"), output("y2")],
        "durationSeconds": 24.0,
    });
    let recorded = engine.call(commands::INPUTS, &engine.inputs_request(&change));
    findings.require(
        recorded["result"]["changed"] == true,
        format!("the requirements are recorded (B4.3): {recorded}"),
    );

    // The model of the group, imported as the study's model (B3.7): no campaign runs, and nothing is written to the runtime.
    let imported = engine.call(commands::IDENTIFY, &engine.import_request(&model()));
    findings.require(
        imported.get("result").is_some(),
        format!("the declared model is kept as the study's model (B3.7): {imported}"),
    );

    // Tune designs both members (C7): the answer names the group, each member's path and gains, and the values it would write. It is
    // unapplied (B4.1, N8.9).
    let tuned = engine.call(commands::TUNE, &engine.tune_request());
    let design = tuned["result"]["group"].clone();
    findings.require(
        design.is_object(),
        format!("Tune designs the PID group (C7): {tuned}"),
    );
    let members = design["members"].as_array().cloned().unwrap_or_default();
    let designed: Vec<String> = members
        .iter()
        .map(|member| text_of(member, "instance"))
        .collect();
    findings.check(
        designed == MEMBERS,
        format!("the design names both members in the group's order (C7): {designed:?}"),
    );
    findings.check(
        design["applied"] == false,
        "Tune's candidate is unapplied (B4.1, N8.9)",
    );
    let candidate = numbers_of(&design["values"]);
    findings.check(
        candidate.keys().cloned().collect::<BTreeSet<_>>() == symbols,
        format!(
            "the candidate holds a value for each of the group's six symbols (C7): {candidate:?}"
        ),
    );
    for member in &members {
        let symbol = text_of(member, "proportionalSymbol");
        let gain = member["gains"]["proportionalGain"]
            .as_f64()
            .unwrap_or(f64::NAN);
        findings.check(
            gain > 0.0
                && candidate
                    .get(&symbol)
                    .is_some_and(|value| close(*value, gain)),
            format!(
                "{} has a positive proportional gain that Apply writes by its symbol (C7): {gain}",
                text_of(member, "instance")
            ),
        );
    }

    // Validate runs the members' actual Structured Text on the model, with a reference case for each controlled output (C7), and gives
    // its rows (B5.4, B9.9). It leaves the record as it was.
    let validated = engine.call(commands::VALIDATE, &engine.validate_request());
    let result = validated["result"].clone();
    findings.require(
        result["status"] == "Pass" && result["applyAvailable"] == true,
        format!(
            "Validate passes the candidate, which leaves Apply available (B5.4). The engine answered: {validated}"
        ),
    );
    let rows = result["table"].as_array().map_or(0, Vec::len);
    findings.check(rows > 0, "Validate gives its table of rows (B5.4, B9.9)");
    let cases: Vec<String> = result["scenarios"]
        .as_array()
        .map(|items| items.iter().map(|item| text_of(item, "id")).collect())
        .unwrap_or_default();
    findings.check(
        cases.iter().any(|id| id == "independent-reference:y1")
            && cases.iter().any(|id| id == "independent-reference:y2"),
        format!("Validate runs one reference case for each controlled output (C7): {cases:?}"),
    );
    findings.check(
        same_values(&lab.parameter_values(), &before),
        "Tune and Validate leave the running record unchanged (B4.1, B5.1)",
    );

    // The review of Apply names the group's six symbols and no other, with the values in force and the values that the confirmation
    // writes (B6.1, §0.1b). It writes nothing.
    let reviewed = engine.call(commands::APPLY, &engine.application_request(None));
    let review = reviewed["result"].clone();
    findings.require(
        review["action"] == "apply" && text_of(&review, "identity").starts_with("sha256:"),
        format!("the review of Apply names its identity (§0.1b): {reviewed}"),
    );
    findings.check(
        review_symbols(&review) == symbols,
        format!(
            "the review names the group's six symbols and no other (B6.1): {:?}",
            review_symbols(&review)
        ),
    );
    let next = review_values(&review, "next");
    findings.check(
        same_values(&next, &candidate),
        format!("the review writes the values that Tune designed (B6.1): {next:?}"),
    );
    findings.check(
        same_values(&review_values(&review, "running"), &before),
        "the review reads the values in force from the runtime (E7.2)",
    );
    findings.check(
        review["foundMode"] == "manual",
        "the review finds the loop in Manual (A6.2)",
    );
    let proportional: Vec<String> = members
        .iter()
        .map(|member| text_of(member, "proportionalSymbol"))
        .collect();
    findings.check(
        proportional
            .iter()
            .all(|symbol| changed(&before, &candidate, symbol)),
        "the candidate changes each member's proportional gain, so Restore has a change to undo",
    );

    // Apply writes the candidate, and the runtime confirms the write (B6.3, E7.3). The loop enters Auto (A6.2).
    let identity = text_of(&review, "identity");
    let applied = engine.call(
        commands::APPLY,
        &engine.application_request(Some(identity.as_str())),
    );
    let outcome = applied["result"].clone();
    findings.require(
        outcome["result"]["status"] == "passed",
        format!("the Apply passes, and the runtime confirms it (B6.3): {applied}"),
    );
    findings.check(
        outcome["restored"] == false
            && outcome["restoreAvailable"] == true
            && !text_of(&outcome, "checkpointId").is_empty(),
        "a passed Apply restores nothing, offers Restore, and names its checkpoint (B6.3, A8.5)",
    );
    findings.check(
        same_values(&lab.parameter_values(), &next),
        "the runtime's record holds the candidate that the review named (B6.1, E7.3)",
    );
    findings.check(
        lab.is_auto() == Some(true),
        "Apply leaves the loop in Auto (A6.2, B6.1)",
    );
    findings.check(
        same_values(&unselected_values(&lab), &unselected_before),
        format!("Apply writes none of {UNSELECTED}'s gains (C7)"),
    );

    // The review of Restore names the values from before Apply (B6.4, E7.5). Restore is then confirmed by the runtime.
    let restore_answer = engine.call(commands::RESTORE, &engine.application_request(None));
    let restore_review = restore_answer["result"].clone();
    findings.require(
        restore_review["action"] == "restore"
            && text_of(&restore_review, "identity").starts_with("sha256:"),
        format!("the review of Restore names its identity (§0.1b): {restore_answer}"),
    );
    findings.check(
        same_values(&review_values(&restore_review, "next"), &before),
        "the review of Restore names the values from before Apply (B6.4, E7.5)",
    );
    findings.check(
        restore_review["foundMode"] == "manual",
        "the review of Restore names Manual, the mode before Apply (A6.2)",
    );
    let restore_identity = text_of(&restore_review, "identity");
    let restored = engine.call(
        commands::RESTORE,
        &engine.application_request(Some(restore_identity.as_str())),
    );
    let restoration = restored["result"].clone();
    findings.check(
        restoration["restored"] == true && restoration["mode"] == "manual",
        format!("Restore is confirmed by the runtime and names Manual (B6.4): {restored}"),
    );
    findings.check(
        same_values(&lab.parameter_values(), &before),
        "Restore writes the values from before Apply back, and the runtime holds them (B6.4)",
    );
    findings.check(
        lab.is_auto() == Some(false),
        "Restore returns the loop to Manual (B6.4)",
    );
    findings.check(
        same_values(&unselected_values(&lab), &unselected_before),
        format!("Apply and Restore leave {UNSELECTED}'s gains as they were (C7)"),
    );
    findings.conclude();
}

/// Identify's mode change on the PID group of the fixture, against the runtime (specification 35, C7 "Identify", A6.2, A6.3; annex E6.4,
/// E6.9 and E11.9): the group found in Auto moves to Manual through one transfer that the runtime accepts, each member's Manual command
/// holds the output it drives, and the return writes both members' setpoints back in Manual, enters Auto again from the outputs the group
/// holds (the found Manual commands are not written back, C1), clears its checkpoint from the journal, and leaves both outputs where the
/// group held them.
#[test]
fn identify_moves_the_pid_group_to_manual_and_back_to_auto_through_the_runtime() {
    let source = std::env::temp_dir().join(format!(
        "trust-cs-pid-group-mode-source-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&source);
    let _source = RemoveDir(source.clone());
    prepare_source_in_auto(&source);
    let lab = Lab::start(
        "pid-group-mode",
        source.to_str().expect("the copy's folder is UTF-8"),
        LOOP_FILE,
        TIME_SCALE,
    );
    let mut findings = Findings::default();
    let binding = mode_binding(&lab.loop_definition).expect("the group has a mode binding (A6.3)");
    let status = || {
        lab.client()
            .transfer_status(&binding)
            .expect("the runtime reports the group's transfer status (E6.3)")
    };

    // The group as Identify finds it: in Auto, with the Manual commands at 30 % and 40 %.
    let found = status();
    findings.require(
        lab.is_auto() == Some(true),
        "the program starts the group in Auto (P1.AutoMode is TRUE)",
    );
    findings.check(
        all_close(&numbers(&found, &COMMANDS), &FOUND_COMMANDS),
        format!("the group is found with its Manual commands at 30 and 40: {found:?}"),
    );

    let journal = Mutex::new(
        Journal::open(
            &lab.journal,
            &lab.loop_definition.project_id,
            &lab.loop_definition.id,
        )
        .expect("the loop's journal opens"),
    );
    let mut mode = IdentificationMode::new(&lab.loop_definition, lab.connection.clone(), &journal)
        .expect("the group's mode change is built");

    // Auto to Manual: one transfer of the group's binding. The runtime accepts its tracking only when it names the second member's
    // setpoint, Manual command and output, and every state once (`validate_declared_states`).
    let entered = mode.enter(true);
    findings.require(
        entered.is_ok(),
        format!("Identify moves the group to Manual through a transfer that the runtime accepts (A6.3, E6.4): {entered:?}"),
    );
    let in_manual = status();
    findings.check(
        lab.is_auto() == Some(false),
        "the group is in Manual after the mode change (B3.4 step 1)",
    );
    findings.check(
        all_close(
            &numbers(&in_manual, &SETPOINTS),
            &numbers(&found, &SETPOINTS),
        ),
        "the mode change leaves both setpoints as they were (E6.4: without alignment)",
    );
    findings.check(
        all_close(
            &numbers(&in_manual, &COMMANDS),
            &numbers(&in_manual, &OUTPUTS),
        ),
        format!("each member's Manual command holds the output it drives, not the command it was found with (E6.4): {in_manual:?}"),
    );
    findings.check(
        mode_journaled(&journal.lock()),
        "the mode found is journaled while the group is in Manual (A6.2, A8.6)",
    );

    // The return: both members' setpoints are written back in Manual, the group enters Auto from the outputs it holds, and the
    // checkpoint is cleared.
    let returned = mode.restore();
    findings.require(
        returned.is_ok(),
        format!("Identify returns the group to Auto through transfers that the runtime accepts (A6.2, E6.9): {returned:?}"),
    );
    findings.check(
        !mode_journaled(&journal.lock()),
        "the mode checkpoint is cleared once the runtime has confirmed every transfer (E11.9)",
    );
    check_returned_to_auto(&mut findings, &lab, &binding, &found);
    findings.conclude();
}

/// Identify's campaign on the PID group of the fixture, against the runtime (specification 35, C7 "Identify", A6.2, B3.4): the group is
/// found in Auto, and each of its two plant inputs declares an identification step of its own, so the campaign moves the group to Manual,
/// steps the first member's Manual command and then the second's in one recording, fits the multivariable model from it, and returns the
/// group to the mode and the commands it was found with. Run time: the campaign's baseline and two steps held until they settle, about a
/// minute at real time.
#[test]
fn identify_runs_the_pid_group_campaign_and_returns_the_group_to_auto() {
    let source = std::env::temp_dir().join(format!(
        "trust-cs-pid-group-identify-source-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&source);
    let _source = RemoveDir(source.clone());
    prepare_source_for_identify(&source);
    let lab = Lab::start(
        "pid-group-identify",
        source.to_str().expect("the copy's folder is UTF-8"),
        LOOP_FILE,
        TIME_SCALE,
    );
    let engine = Engine::new(&lab);
    let mut findings = Findings::default();
    let binding = mode_binding(&lab.loop_definition).expect("the group has a mode binding (A6.3)");
    let found = lab
        .client()
        .transfer_status(&binding)
        .expect("the runtime reports the group's transfer status (E6.3)");
    findings.require(
        lab.is_auto() == Some(true),
        "the program starts the group in Auto (P1.AutoMode is TRUE)",
    );
    let record_before = lab.parameter_values();

    // The campaign: Automatic in Auto, with the loop's declared duration. Every input declares its own step, so the campaign is a
    // manual-output campaign, which needs the group in Manual (B3.4 step 1, C7).
    let settings = declared_settings(&lab.loop_definition, AcquisitionMethod::Automatic);
    let identified = engine.call(commands::IDENTIFY, &engine.identify_run_request(settings));
    let result = identified.get("result").cloned().unwrap_or(Value::Null);
    findings.check(
        result.is_object() && !text_of(&result, "modelType").is_empty(),
        format!(
            "Identify gives a model of the group (C7, B3.8); the engine answered: {}",
            identified.get("failure").unwrap_or(&Value::Null)
        ),
    );
    findings.check(
        text_of(&result, "recordingId").starts_with("rec-"),
        "the model names the recording that both steps were fitted from (A8.3)",
    );

    // Whatever the fit gave, the campaign ended: the group is back in the mode and with the commands it was found with, nothing is
    // left in the journal, and the running record is unchanged.
    check_returned_to_auto(&mut findings, &lab, &binding, &found);
    let journal = journal_of(&lab.journal, &lab.loop_definition);
    findings.check(
        journal.entries().is_empty(),
        format!(
            "the journal holds nothing once the campaign has ended and the mode is back (A6.2, A8.5): {:?}",
            journal.entries()
        ),
    );
    findings.check(
        same_values(&lab.parameter_values(), &record_before),
        "Identify writes no parameter of the group (B3.4)",
    );
    findings.conclude();
}
