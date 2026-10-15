//! The scripted runtime of the Restore tests (test only). One store of the loop's values serves both the transfer status and the
//! parameter record, as the fake of control-studio-found-mode.test.ts does, so that the mode and the gains read back as the Restore wrote
//! them. It answers `status`, `control.parameters.*`, `control.transfer.*` and `control.recording.*` with exactly the fields of the
//! protocol annex (E1.3, E4.7, E6.3 to E6.10, E7.2 to E7.6), and it computes the tracked states of a transfer as the annex says the
//! runtime computes them at the task boundary (E6.4, C0). `FakeRuntime` keeps every request. The loops, the model and the requirements
//! are those of the TypeScript tests (control-studio-pid-fixtures.ts) and of trust-control's commissioning fixtures.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use serde_json::{json, Map, Value};
use trust_control::commissioning::actuator_feedback::transfer_output_basis;
use trust_control::commissioning::adapters::controller_adapter;
use trust_control::commissioning::checkpoint::{CheckpointStage, CommissioningCheckpoint};
use trust_control::commissioning::commissioning_result::TransferOwner;
use trust_control::commissioning::entry_allowance::transfer_tolerance;
use trust_control::commissioning::found_mode::FoundMode;
use trust_control::commissioning::initialization_formulas::evaluate_initialization_formula;
use trust_control::commissioning::parameter_application::ParameterOwner;
use trust_control::commissioning::parameter_status::{ParameterStatus, RecordState};
use trust_control::commissioning::transfer::{
    rounded_transfer_value, TransferBinding, TransferValue,
};
use trust_control::commissioning::transfer_tracking::TransferTracking;
use trust_control::experiment::{
    ControlEvidenceSource, RecordingRecoveryBinding, RecordingRecoveryOwner,
};
use trust_control::json::decode_value;
use trust_control::loop_definition::LoopDefinition;
use trust_control::model::general::decode_general_model;
use trust_control::validation::engineer_verification_plan::engineer_requirements;

use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime};

/// The program revision of the runtime that Apply wrote to.
pub(super) const PROGRAM: &str = "program-1";
/// The revision of the parameter binding (E7.1).
pub(super) const PARAMETER_BINDING: &str = "parameters-1";
/// The revision of the transfer binding (E6.2).
pub(super) const TRANSFER_BINDING: &str = "transfer-1";
/// The measured output's noise that Apply allowed for (`noise = 0.05` of the TypeScript tests, A6.4).
pub(super) const NOISE: f64 = 0.05;

/// The controller of a test loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// A program's parallel PID with its `parallel-pid-v1` states (C2).
    Pid,
    /// A connected P on the same signals (C13).
    Proportional,
    /// The shipped Control.PID with its declared `control-pid-v1` states (C1).
    Library,
}

/// A test loop and the checkpoint its Apply journaled.
pub(super) struct Setup {
    /// The controller of the loop.
    pub(super) kind: Kind,
    /// The loop.
    pub(super) loop_definition: LoopDefinition,
    /// The checkpoint, as Apply journals it before its first write and rewrites it as each operation is dispatched (A8.5).
    pub(super) checkpoint: CommissioningCheckpoint,
}

/// The loop of `kind`, its checkpoint at `stage`, and the mode Apply found it in. The checkpoint holds the record before Apply (`Kp 1, Ki
/// 0, Kd 0` under `active-origin`), the candidate, Apply's parameter operation `apply-1` and its last transfer `target-1`, the reference
/// at 2, the measured noise [`NOISE`], and the adapter's binding and recovery tracking.
pub(super) fn setup(kind: Kind, found: Option<FoundMode>, stage: CheckpointStage) -> Setup {
    let loop_definition = study_loop(kind);
    let model = decode_general_model(&json!({
        "kind": "transfer-function", "revision": "model:1", "source": "provided", "validation": "validated",
        "evidence": ["bench"], "delaySeconds": 0, "numerator": [1], "denominator": [1, 1],
        "inputs": [{ "id": "u", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "outputs": [{ "id": "y", "unit": "%", "operatingRange": { "minimum": 0, "maximum": 100 } }],
        "operatingPoint": { "inputs": [0], "outputs": [2] }
    }))
    .expect("the model decodes");
    let task = engineer_requirements(
        &json!({ "referenceChange": 1, "maximumError": 0.1, "settlingBand": 0.02,
        "settlingTimeSeconds": 2, "durationSeconds": 4 }),
    )
    .expect("the requirements are valid");
    let adapter = controller_adapter(&loop_definition).expect("the test loop is adapted");
    let binding = adapter.transfer_binding().expect("the test loop binds");
    let recovery_tracking = adapter
        .recovery_tracking(&model)
        .expect("the test loop tracks");
    let tolerance = transfer_tolerance(&loop_definition, task.transfer_tolerance)
        .expect("the output limits are set");
    let checkpoint = CommissioningCheckpoint {
        id: "checkpoint-1".to_owned(),
        loop_revision: loop_definition.revision.clone(),
        stage,
        model,
        task,
        origin: origin_record(kind),
        candidate: candidate_gains(kind),
        parameter_operation: ParameterOwner {
            id: "apply-1".to_owned(),
            program: PROGRAM.to_owned(),
            binding: PARAMETER_BINDING.to_owned(),
        },
        references: BTreeMap::from([(reference_symbol(kind).to_owned(), 2.0)]),
        binding,
        recovery_tracking,
        tolerance,
        replacement_program_revision: None,
        transfer: Some(TransferOwner {
            id: "target-1".to_owned(),
            program: PROGRAM.to_owned(),
            binding: TRANSFER_BINDING.to_owned(),
        }),
        recording: None,
        recording_result: None,
        found_mode: found,
        process_noise: Some(NOISE),
        validation_identity: "sha256:validation".to_owned(),
        transfers: Vec::new(),
        schedule_origin: None,
    };
    Setup {
        kind,
        loop_definition,
        checkpoint,
    }
}

/// The gains of `kind`'s parameter record, by symbol.
pub(super) fn gains(kind: Kind, kp: f64, ki: f64, kd: f64) -> BTreeMap<String, f64> {
    let prefix = if kind == Kind::Library {
        "Main.Parameters."
    } else {
        "Main."
    };
    BTreeMap::from([
        (format!("{prefix}Kp"), kp),
        (format!("{prefix}Ki"), ki),
        (format!("{prefix}Kd"), kd),
    ])
}

/// The record before Apply: `Kp 1, Ki 0, Kd 0`.
pub(super) fn origin_gains(kind: Kind) -> BTreeMap<String, f64> {
    gains(kind, 1.0, 0.0, 0.0)
}

/// The candidate Apply wrote: `Kp 2, Ki 0.5, Kd 0`; a P law writes its integral and derivative gains as explicit zeros (C0, C13).
pub(super) fn candidate_gains(kind: Kind) -> BTreeMap<String, f64> {
    if kind == Kind::Proportional {
        gains(kind, 2.0, 0.0, 0.0)
    } else {
        gains(kind, 2.0, 0.5, 0.0)
    }
}

/// The reference of `kind`'s loop, which the checkpoint keeps.
pub(super) fn reference_symbol(kind: Kind) -> &'static str {
    if kind == Kind::Library {
        "LevelSP"
    } else {
        "Main.SP"
    }
}

/// The parameter record before Apply, as the runtime reported it to Apply's review (E7.2).
pub(super) fn origin_record(kind: Kind) -> ParameterStatus {
    ParameterStatus {
        supports_request_id: Some(true),
        transaction_id: Some("before-1".to_owned()),
        program_revision: PROGRAM.to_owned(),
        binding_revision: PARAMETER_BINDING.to_owned(),
        active_revision: "active-origin".to_owned(),
        values: origin_gains(kind),
        status: RecordState::Applied,
        previous_revision: None,
        previous_values: None,
        reason: None,
    }
}

/// The owner of the recording an Apply observation started on `loop_definition`'s process variable (E11.2).
pub(super) fn recording_owner(loop_definition: &LoopDefinition) -> RecordingRecoveryOwner {
    RecordingRecoveryOwner {
        schema_version: 1,
        recording_id: "recording-1".to_owned(),
        project_id: loop_definition.project_id.clone(),
        runtime_id: loop_definition.runtime_id.clone(),
        loop_id: loop_definition.id.clone(),
        loop_revision: loop_definition.revision.clone(),
        source: ControlEvidenceSource::Simulation,
        program_revision: Some(PROGRAM.to_owned()),
        start_acknowledgement_unconfirmed: None,
        bindings: vec![RecordingRecoveryBinding {
            id: "processVariable".to_owned(),
            address: Some("%IL0".to_owned()),
            symbol: None,
            iec_type: "LREAL".to_owned(),
        }],
    }
}

/// The stop answer of the recording of `owner`, as the runtime returns it once (E4.7): its identity, no samples, and an incomplete
/// acquisition.
pub(super) fn held_run(owner: &RecordingRecoveryOwner) -> Value {
    json!({
        "recording_id": owner.recording_id, "loop_revision": owner.loop_revision, "program_revision": owner.program_revision,
        "bindings": [{ "id": "processVariable", "address": "%IL0", "iec_type": "LREAL" }],
        "stop_reason": "stopped", "samples": [],
        "acquisition": { "method": "runtime_scan_buffer", "completeness": "incomplete", "observedSampleCount": 0,
            "scanRange": null, "missingScanRanges": [], "droppedObservedSamples": 0, "actualCycleTimingAvailable": false,
            "freshnessAvailable": false, "reason": "Stopped by the Restore." }
    })
}

/// The loop of `kind`, as its loop file declares it.
fn study_loop(kind: Kind) -> LoopDefinition {
    let file = match kind {
        Kind::Pid => pid_file(true),
        Kind::Proportional => pid_file(false),
        Kind::Library => library_file(),
    };
    LoopDefinition::decode(&file.to_string()).expect("the test loop decodes")
}

/// The PID apply loop of the TypeScript tests (`pidLoop` with `pidInitialization`): the set point `Main.SP` and the measurement `Main.PV`
/// in m, the output `Main.MV` in V within ±20, a 20 ms task, the gains `Main.Kp`, `Main.Ki` and `Main.Kd` within ±1e6, and the mode
/// `Main.Manual`, TRUE in Manual. Without `parallel_pid` the law is the connected P on `Main.Kp`.
fn pid_file(parallel_pid: bool) -> Value {
    let mut controller = json!({
        "instance": "Main", "type": "Control.PID", "revision": "sha256:pid-controller-1", "action": "direct", "taskPeriodMs": 20,
        "manualOutputSignal": "Main.ManualCommand", "manualOutputAddress": "%ML8", "manualOutputType": "LREAL"
    });
    if parallel_pid {
        controller["design"] = json!("pid");
        controller["law"] = json!({ "kind": "pid", "parameterRecordSymbol": "Main.Parameters", "proportionalGainSymbol": "Main.Kp",
            "integralGainSymbol": "Main.Ki", "derivativeGainSymbol": "Main.Kd" });
        controller["pidInitialization"] = json!({ "kind": "parallel-pid-v1",
            "integralState": { "symbol": "Main.Integral", "iecType": "LREAL", "unit": "V", "minimum": -20, "maximum": 20 },
            "derivativeState": { "symbol": "Main.PreviousError", "iecType": "LREAL", "unit": "m", "minimum": -20, "maximum": 20 },
            "proportionalSetpointWeight": 1, "derivativeSetpointWeight": 1, "maximumSnapshotAgeMs": 100 });
    } else {
        controller["law"] = json!({ "kind": "proportional", "gainSymbol": "Main.Kp" });
    }
    json!({
        "schemaVersion": 2, "id": "loop:pid-apply", "name": "PID apply", "revision": "sha256:pid-apply-1",
        "projectId": "project:pid-apply", "runtimeId": "runtime:pid-apply",
        "controller": controller,
        "signals": {
            "setpoint": { "symbol": "Main.SP", "address": "%ML0", "iecType": "LREAL", "direction": "read",
                "unit": "m", "range": { "minimum": -20, "maximum": 20 } },
            "processVariable": { "symbol": "Main.PV", "address": "%IL0", "iecType": "LREAL", "direction": "read",
                "unit": "m", "range": { "minimum": -20, "maximum": 20 } },
            "manipulatedVariable": { "symbol": "Main.MV", "address": "%QL0", "iecType": "LREAL", "direction": "write",
                "unit": "V", "range": { "minimum": -20, "maximum": 20 } }
        },
        "limits": { "outputMinimum": -20, "outputMaximum": 20 },
        "observedSampleTiming": { "nominalPeriodMs": 20, "maximumJitterMs": 1 },
        "parameterBinding": { "loop_id": "loop:pid-apply", "task_name": "MainTask",
            "parameters": [
                { "symbol": "Main.Kp", "iec_type": "LREAL", "unit": "V/m", "convention": "proportional gain", "minimum": -1e6, "maximum": 1e6 },
                { "symbol": "Main.Ki", "iec_type": "LREAL", "unit": "V/(m*s)", "convention": "integral gain", "minimum": -1e6, "maximum": 1e6 },
                { "symbol": "Main.Kd", "iec_type": "LREAL", "unit": "V*s/m", "convention": "derivative gain", "minimum": -1e6, "maximum": 1e6 }
            ],
            "mode": { "symbol": "Main.Manual", "manual_value": true } }
    })
}

/// The tank loop of the shipped Control.PID `Main.TankController` (trust-control's `library_pid_loop`): its seven retained states, its
/// own `StateValue.ProcessValue` as the measurement, the set point `LevelSP`, the output `ValveMV` with its measured actuator
/// `ValvePosition`, all in % within 0 to 100, a 10 ms task, the gains `Main.Parameters.Kp`, `Ki` and `Kd`, and the mode `Main.Auto`,
/// FALSE in Manual.
fn library_file() -> Value {
    let state = |member: &str, unit: &str, initial: Value| {
        json!({ "symbol": format!("Main.TankController.StateValue.{member}"), "iec_type": "LREAL", "unit": unit,
            "minimum": -10_000, "maximum": 10_000, "initial": initial, "postScan": "bounded" })
    };
    let output = || json!({ "source": "model-output", "index": 0 });
    let input = || json!({ "source": "model-input", "index": 0 });
    let gain = |symbol: &str, convention: &str| json!({ "symbol": symbol, "iec_type": "LREAL", "unit": "%/%", "convention": convention, "minimum": -100, "maximum": 100 });
    json!({
        "schemaVersion": 2, "id": "loop:tank", "name": "Tank level", "revision": "sha256:loop-1",
        "projectId": "project:sha256:abc", "runtimeId": "runtime:local",
        "controller": {
            "instance": "Main.TankController", "type": "Control.PID", "revision": "sha256:controller-1", "action": "direct",
            "taskPeriodMs": 10,
            "law": { "kind": "pid", "parameterRecordSymbol": "Main.Parameters", "proportionalGainSymbol": "Main.Parameters.Kp",
                "integralGainSymbol": "Main.Parameters.Ki", "derivativeGainSymbol": "Main.Parameters.Kd" },
            "manualOutputSignal": "Main.ManualCommand", "manualOutputType": "LREAL",
            "pidInitialization": { "kind": "control-pid-v1", "maximumSnapshotAgeMs": 100, "states": [
                state("Setpoint", "%", output()), state("SetpointRate", "%/s", json!(0)),
                state("ProcessValue", "%", output()), state("ProcessValueRate", "%/s", json!(0)),
                state("FeedForward", "%", json!(0)), state("NominalOutput", "%", input()),
                state("SaturatedOutput", "%", input())
            ] }
        },
        "parameterBinding": { "loop_id": "loop:tank", "task_name": "MainTask",
            "parameters": [gain("Main.Parameters.Kp", "proportional gain"), gain("Main.Parameters.Ki", "integral gain"),
                gain("Main.Parameters.Kd", "derivative gain")],
            "mode": { "symbol": "Main.Auto", "manual_value": false } },
        "signals": {
            "setpoint": { "symbol": "LevelSP", "address": "%ML0", "iecType": "LREAL", "direction": "read",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "Main.TankController.StateValue.ProcessValue", "iecType": "LREAL",
                "direction": "read", "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "ValveMV", "address": "%QL0", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } },
            "actuator": { "symbol": "ValvePosition", "address": "%IL8", "iecType": "LREAL", "direction": "read",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 10, "maximumJitterMs": 1 }
    })
}

/// The values the runtime holds besides the gains: the set point at 2, the measurement at 2.004 near the validated point 2, and the
/// output (0 V for the PID loops, 0.36 % for the tank). The other members read 0.
fn readings(kind: Kind) -> &'static [(&'static str, f64)] {
    match kind {
        Kind::Pid | Kind::Proportional => &[("Main.SP", 2.0), ("Main.PV", 2.004), ("Main.MV", 0.0)],
        Kind::Library => &[
            ("LevelSP", 2.0),
            ("Main.TankController.StateValue.ProcessValue", 2.004),
            ("ValveMV", 0.36),
            ("ValvePosition", 0.36),
            ("Main.ManualCommand", 0.36),
            ("Main.TankController.StateValue.Setpoint", 2.0),
            ("Main.TankController.StateValue.NominalOutput", 0.36),
            ("Main.TankController.StateValue.SaturatedOutput", 0.36),
        ],
    }
}

/// A rollback the runtime accepted and has not confirmed yet (E7.5).
struct PendingRollback {
    id: String,
    revision: String,
    values: BTreeMap<String, f64>,
}

/// The scripted runtime's store and records.
pub(super) struct World {
    binding: TransferBinding,
    parameters: Vec<String>,
    values: BTreeMap<String, Value>,
    /// The program revision the runtime runs (A2.2).
    pub(super) program: String,
    /// Whether the target is physical (A2.4).
    pub(super) physical: bool,
    /// The revision of the values in force (E7.2).
    pub(super) active: String,
    /// The retained previous revision and values (E7.2).
    pub(super) previous: Option<(String, BTreeMap<String, f64>)>,
    /// The state of the last parameter transaction.
    pub(super) parameter_state: &'static str,
    /// The identifier of the last parameter transaction.
    pub(super) parameter_transaction: Option<String>,
    /// How many status reads still report an accepted rollback as pending before it is applied.
    pub(super) held_rollback_reads: usize,
    pending_rollback: Option<PendingRollback>,
    /// The status of the last transfer transaction (E6.3).
    pub(super) transfer_state: &'static str,
    /// The last transfer transaction record (E6.8).
    pub(super) transfer_transaction: Option<Value>,
    /// How many Auto entries the runtime rejects at the boundary before it accepts one (E6.6).
    pub(super) rejected_auto_entries: usize,
    /// The stop answer of each recording the runtime holds, by identifier (E4.7).
    pub(super) recordings: BTreeMap<String, Value>,
    /// Whether a recording stop is received and its acknowledgement lost: the connection closes without an answer (E11.4).
    pub(super) lost_stops: bool,
}

impl World {
    /// The runtime after Apply: the candidate in force under `active-candidate` with the record before Apply retained (E7.2), Apply's
    /// parameter operation `apply-1` the last one, the loop in Auto or in Manual, and Apply's target change `target-1` the last transfer,
    /// applied (E6.8).
    pub(super) fn applied(setup: &Setup, in_auto: bool) -> Self {
        let parameters = setup
            .loop_definition
            .parameter_binding
            .iter()
            .flat_map(|binding| binding.parameters.iter())
            .map(|parameter| parameter.symbol.clone())
            .collect();
        let mut world = Self {
            binding: setup.checkpoint.binding.clone(),
            parameters,
            values: BTreeMap::new(),
            program: PROGRAM.to_owned(),
            physical: false,
            active: "active-candidate".to_owned(),
            previous: Some(("active-origin".to_owned(), origin_gains(setup.kind))),
            parameter_state: "applied",
            parameter_transaction: Some("apply-1".to_owned()),
            held_rollback_reads: 0,
            pending_rollback: None,
            transfer_state: "applied",
            transfer_transaction: None,
            rejected_auto_entries: 0,
            recordings: BTreeMap::new(),
            lost_stops: false,
        };
        for symbol in world.member_symbols() {
            world.values.insert(symbol, json!(0.0));
        }
        for (symbol, value) in readings(setup.kind) {
            world.values.insert((*symbol).to_owned(), json!(value));
        }
        world.write_numbers(&candidate_gains(setup.kind));
        world.set_mode(!in_auto);
        let snapshot = world.binding_values();
        let applied = world.state_values();
        world.transfer_transaction =
            Some(world.transaction("target-1", &snapshot, &applied, &snapshot));
        world
    }

    /// Starts the scripted runtime on this store.
    pub(super) fn serve(self) -> (FakeRuntime, Arc<Mutex<World>>) {
        let shared = Arc::new(Mutex::new(self));
        let handler = Arc::clone(&shared);
        let fake = FakeRuntime::start(move |request| {
            handler.lock().expect("the scripted runtime").reply(request)
        });
        (fake, shared)
    }

    /// Whether the loop is in Auto.
    pub(super) fn in_auto(&self) -> bool {
        self.values.get(&self.binding.mode.symbol) == Some(&json!(!self.binding.mode.manual_value))
    }

    /// The values in force of the parameter record.
    pub(super) fn parameter_values(&self) -> BTreeMap<String, f64> {
        self.parameters
            .iter()
            .map(|symbol| (symbol.clone(), self.number(symbol)))
            .collect()
    }

    /// The value of `symbol` as a number; not a number when the store holds none.
    pub(super) fn number(&self, symbol: &str) -> f64 {
        self.values
            .get(symbol)
            .and_then(Value::as_f64)
            .unwrap_or(f64::NAN)
    }

    /// Writes numbers into the store, as another client's write does.
    pub(super) fn write_numbers(&mut self, values: &BTreeMap<String, f64>) {
        for (symbol, value) in values {
            self.values.insert(symbol.clone(), json!(value));
        }
    }

    /// Sets the loop's modes to Manual or to Auto.
    pub(super) fn set_mode(&mut self, manual: bool) {
        for mode in std::iter::once(&self.binding.mode).chain(&self.binding.additional_modes) {
            let value = if manual {
                mode.manual_value
            } else {
                !mode.manual_value
            };
            self.values.insert(mode.symbol.clone(), json!(value));
        }
    }

    /// The answer to one request (E0.1); none for a stop whose acknowledgement is lost (E11.4).
    fn reply(&mut self, request: &Value) -> Option<Value> {
        let params = request.get("params").cloned().unwrap_or(Value::Null);
        let identifier = params["recording_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let operation = request["type"].as_str().unwrap_or_default();
        if operation == "control.recording.stop" && self.lost_stops {
            return None;
        }
        Some(match operation {
            "status" => answer(request, self.runtime_status()),
            "control.parameters.status" => {
                let record = self.parameter_status();
                answer(request, record)
            }
            "control.parameters.cancel" => answer(request, self.cancel_parameters(&params)),
            "control.parameters.rollback" => match self.rollback(&params) {
                Ok(result) => answer(request, result),
                Err(text) => refusal(request, "invalid_parameter_request", text),
            },
            "control.transfer.status" => answer(request, self.transfer_status()),
            "control.transfer.cancel" => answer(request, self.cancel_transfer(&params)),
            "control.transfer.apply" => {
                let result = self.apply_transfer(&params);
                answer(request, result)
            }
            "control.recording.stop" => match self.recordings.get(&identifier) {
                Some(run) => answer(request, run.clone()),
                None => refusal(
                    request,
                    "recording_unavailable",
                    "The runtime holds no recording of that identifier.",
                ),
            },
            "control.recording.status" => refusal(
                request,
                "recording_unavailable",
                "The runtime holds no recording of that identifier.",
            ),
            "control.recording.release" => {
                self.recordings.remove(&identifier);
                answer(
                    request,
                    json!({ "recording_id": identifier, "status": "released" }),
                )
            }
            other => refusal(
                request,
                "unknown_operation",
                &format!("The scripted runtime does not answer {other}."),
            ),
        })
    }

    /// The runtime facts (E1.3): a simulated target unless `physical`, and the loop's task every 10 ms.
    fn runtime_status(&self) -> Value {
        json!({
            "state": "running", "session_epoch": 1, "simulation_enabled": !self.physical,
            "simulation_only_io": !self.physical, "simulation_time_scale": 1, "simulation_achieved_time_scale": null,
            "program_revision": self.program, "tasks": [{ "name": self.binding.task_name, "interval_ms": 10 }],
            "cycle_interval_ms": 10
        })
    }

    /// The parameter record (E7.2). An accepted rollback is reported pending for `held_rollback_reads` reads, then applied (E7.4).
    fn parameter_status(&mut self) -> Value {
        if let Some(rollback) = self.pending_rollback.take() {
            if self.held_rollback_reads > 0 {
                self.held_rollback_reads -= 1;
                let pending = self.record("pending", Some(rollback.id.as_str()));
                self.pending_rollback = Some(rollback);
                return pending;
            }
            self.write_numbers(&rollback.values);
            self.active = rollback.revision;
            self.previous = None;
            self.parameter_state = "applied";
            self.parameter_transaction = Some(rollback.id);
        }
        let transaction = self.parameter_transaction.clone();
        self.record(self.parameter_state, transaction.as_deref())
    }

    /// The parameter record in the form of E7.2.
    fn record(&self, state: &str, transaction: Option<&str>) -> Value {
        json!({
            "supports_request_id": true, "transaction_id": transaction, "program_revision": self.program,
            "binding_revision": PARAMETER_BINDING, "active_revision": self.active, "values": self.parameter_values(),
            "status": state, "previous_revision": self.previous.as_ref().map(|(revision, _)| revision),
            "previous_values": self.previous.as_ref().map(|(_, values)| values), "reason": null
        })
    }

    /// A parameter cancel (E7.6): a transaction the runtime holds is committed and not undone; any other identifier is fenced before
    /// dispatch.
    fn cancel_parameters(&self, params: &Value) -> Value {
        let id = params["transaction_id"].as_str().unwrap_or_default();
        let held = self.parameter_transaction.as_deref() == Some(id)
            || self
                .pending_rollback
                .as_ref()
                .is_some_and(|rollback| rollback.id == id);
        if held {
            json!({ "confirmed": false, "reason": "parameter commit requires post-ST observation" })
        } else {
            json!({ "transaction_id": id, "program_revision": self.program, "binding_revision": PARAMETER_BINDING,
                "confirmed": true, "before_dispatch": true })
        }
    }

    /// A parameter rollback (E7.5): accepted while the expected revisions hold, confirmed by a later status read.
    fn rollback(&mut self, params: &Value) -> Result<Value, &'static str> {
        let Some((revision, values)) = self.previous.clone() else {
            return Err("rollback unavailable");
        };
        if params["expected_active_revision"].as_str() != Some(self.active.as_str())
            || params["previous_revision"].as_str() != Some(revision.as_str())
            || params["expected_program_revision"].as_str() != Some(self.program.as_str())
        {
            return Err("stale rollback revision");
        }
        self.pending_rollback = Some(PendingRollback {
            id: params["transaction_id"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            revision,
            values,
        });
        Ok(json!({ "status": "pending", "confirmed": false }))
    }

    /// The transfer status (E6.3).
    fn transfer_status(&self) -> Value {
        json!({
            "program_revision": self.program, "binding_revision": TRANSFER_BINDING, "values": self.binding_values(),
            "runtime_time_ns": 1_000_000_000_u64, "status": self.transfer_state, "reason": null,
            "supported_tracking": ["proportional", "parallel-pid-v1", "declared-state-v1", "declared-initialization-formulas"],
            "transaction": self.transfer_transaction
        })
    }

    /// A transfer cancel (E6.10): the last transaction returns its outcome; any other identifier is fenced before dispatch.
    fn cancel_transfer(&self, params: &Value) -> Value {
        let id = params["transaction_id"].as_str().unwrap_or_default();
        let last = self
            .transfer_transaction
            .as_ref()
            .is_some_and(|transaction| transaction["id"].as_str() == Some(id));
        if last {
            json!({ "status": self.transfer_state, "confirmed": true })
        } else {
            json!({ "status": "cancelled", "confirmed": true, "before_dispatch": true, "transaction_id": id,
                "program_revision": self.program, "binding_revision": TRANSFER_BINDING })
        }
    }

    /// A transfer apply (E6.5 to E6.7), confirmed at the output image of the next scan, unless the runtime is scripted to reject an
    /// Auto entry at the boundary, which writes nothing (E6.6).
    fn apply_transfer(&mut self, params: &Value) -> Value {
        let id = params["transaction_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        let manual = params["manual"].as_bool().unwrap_or(true);
        if !manual && self.rejected_auto_entries > 0 {
            self.rejected_auto_entries -= 1;
            self.transfer_state = "rejected";
            self.transfer_transaction = Some(json!({
                "id": id, "program_revision": self.program, "binding_revision": TRANSFER_BINDING, "status": "rejected",
                "reason": "first output change 0.2 % exceeds the allowed 0.1 %", "observed_values": null, "confirmation": null
            }));
            return json!({ "status": "pending", "confirmed": false });
        }
        let requested: BTreeMap<String, f64> =
            decode_value(params["values"].clone()).expect("the transfer values decode");
        let tracking: Option<TransferTracking> = params
            .get("tracking")
            .filter(|tracking| !tracking.is_null())
            .map(|tracking| decode_value(tracking.clone()).expect("the tracking decodes"));
        let initial = self.binding_values();
        let applied = self.tracked_states(&requested, tracking.as_ref());
        self.write_numbers(&applied);
        self.set_mode(manual);
        let observed = self.binding_values();
        self.transfer_state = "applied";
        self.transfer_transaction = Some(self.transaction(&id, &initial, &applied, &observed));
        json!({ "status": "pending", "confirmed": false })
    }

    /// The states the boundary writes (E6.4, C0): the requested ones, the Manual command at the held output, an aligned proportional
    /// reference, and the declared initialization of each declared state (its fresh formula or its operating value at an Auto entry,
    /// its own value otherwise).
    fn tracked_states(
        &self,
        requested: &BTreeMap<String, f64>,
        tracking: Option<&TransferTracking>,
    ) -> BTreeMap<String, f64> {
        let binding = &self.binding;
        let held = transfer_output_basis(binding, &binding.output).map_or_else(
            || binding.output.symbol.clone(),
            |member| member.symbol.clone(),
        );
        let mut applied: BTreeMap<String, f64> = binding
            .states
            .iter()
            .map(|member| {
                let value = requested
                    .get(&member.symbol)
                    .copied()
                    .unwrap_or_else(|| self.number(&member.symbol));
                (member.symbol.clone(), value)
            })
            .collect();
        match tracking {
            Some(TransferTracking::Proportional(tracking)) => {
                let output = self.number(&held);
                applied.insert(
                    tracking.manual_command_symbol.clone(),
                    self.rounded(&tracking.manual_command_symbol, output),
                );
                if tracking.align_reference {
                    let aligned = self.number(&tracking.process_variable)
                        + output / self.number(&tracking.gain_symbol);
                    applied.insert(
                        tracking.reference_symbol.clone(),
                        self.rounded(&tracking.reference_symbol, aligned),
                    );
                }
            }
            Some(TransferTracking::DeclaredState(tracking)) => {
                let snapshot = self.snapshot();
                for (symbol, value) in &tracking.initialization_values {
                    let adopted = if tracking.align_output {
                        match tracking
                            .initialization_formulas
                            .as_ref()
                            .and_then(|formulas| formulas.get(symbol))
                        {
                            Some(formula) => self.rounded(
                                symbol,
                                evaluate_initialization_formula(binding, formula, &snapshot)
                                    .expect("the formula evaluates"),
                            ),
                            None => *value,
                        }
                    } else {
                        self.number(symbol)
                    };
                    applied.insert(symbol.clone(), adopted);
                }
                if tracking.align_output {
                    applied.insert(
                        tracking.reference_symbol.clone(),
                        self.rounded(
                            &tracking.reference_symbol,
                            self.number(&tracking.process_variable),
                        ),
                    );
                }
                applied.insert(
                    tracking.manual_command_symbol.clone(),
                    self.rounded(&tracking.manual_command_symbol, self.number(&held)),
                );
            }
            Some(TransferTracking::ParallelPid(_)) | None => {}
        }
        applied
    }

    /// `value` as the state `symbol` holds it, rounded to its IEC type.
    fn rounded(&self, symbol: &str, value: f64) -> f64 {
        self.binding
            .states
            .iter()
            .find(|member| member.symbol == symbol)
            .map_or(value, |member| {
                rounded_transfer_value(member, value).unwrap_or(value)
            })
    }

    /// An applied transaction record (E6.8), confirmed at the output image.
    fn transaction(
        &self,
        id: &str,
        initial: &Value,
        applied: &BTreeMap<String, f64>,
        observed: &Value,
    ) -> Value {
        json!({
            "id": id, "program_revision": self.program, "binding_revision": TRANSFER_BINDING, "status": "applied",
            "reason": null, "restored": false, "observed_values": observed, "confirmation": "output-image",
            "initial_values": initial, "applied_values": applied
        })
    }

    /// The symbols of the binding's members: its states, guards and outputs.
    fn member_symbols(&self) -> Vec<String> {
        self.binding
            .states
            .iter()
            .chain(&self.binding.guards)
            .chain(std::iter::once(&self.binding.output))
            .chain(&self.binding.additional_outputs)
            .map(|member| member.symbol.clone())
            .collect()
    }

    /// The values of the binding's states.
    fn state_values(&self) -> BTreeMap<String, f64> {
        self.binding
            .states
            .iter()
            .map(|member| (member.symbol.clone(), self.number(&member.symbol)))
            .collect()
    }

    /// The value of every mode and member of the binding, as a status reports them (E6.3).
    fn binding_values(&self) -> Value {
        let modes = std::iter::once(&self.binding.mode)
            .chain(&self.binding.additional_modes)
            .map(|mode| mode.symbol.clone());
        let mut values = Map::new();
        for symbol in modes.chain(self.member_symbols()) {
            let value = self.values.get(&symbol).cloned().unwrap_or(Value::Null);
            values.insert(symbol, value);
        }
        Value::Object(values)
    }

    /// The boundary snapshot of the binding's modes and members.
    fn snapshot(&self) -> BTreeMap<String, TransferValue> {
        decode_value(self.binding_values()).expect("the snapshot decodes")
    }
}
