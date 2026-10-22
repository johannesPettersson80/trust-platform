//! Test support of the Apply workflow: a scripted plant runtime that answers the runtime protocol annex as the runtime answers it, with
//! exactly the fields each answer names (E1.3, E3, E4, E6, E7), and the loops, model and requirements of the `TypeScript` apply tests
//! (`control-studio-apply-process-time.test.ts`, `control-studio-pid-fixtures.ts`). The transfers, the parameter record and the
//! recording share one plant, as they do in the runtime, so the mode and the gain can be read back after a run (A6.2, B6.3, B6.4). A
//! queued transfer or parameter set is committed at the next status read, as the runtime commits it at the next task boundary.

// Test support: every fixture is valid by construction, so its `expect`s only state that.
#![allow(clippy::missing_panics_doc)]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use serde_json::{json, Map, Value};
use trust_control::commissioning::adapters::ControllerGains;
use trust_control::loop_definition::{
    InitialSource, InitialValue, InitializedState, LoopDefinition, PostScan, StateType,
};
use trust_control::model::general::decode_general_model;
use trust_control::model::GeneralModel;
use trust_control::runtime::RuntimeConnection;
use trust_control::validation::engineer_verification_plan::{
    engineer_requirements, EngineerRequirements,
};

use super::review::ApplyInputs;
use super::ApplyTiming;
use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime};
use crate::storage::folder_name;
use crate::workflow::identify_campaign::CampaignReport;

/// The reviewed target of the fixtures: the model's operating output 2 plus the reference change 1 (B6.2).
pub(crate) const TARGET: f64 = 3.0;
/// The transfer binding revision the plant reports (E6.3).
pub(crate) const TRANSFER_REVISION: &str = "transfer-1";
/// The parameter binding revision the plant reports (E7.2).
pub(crate) const PARAMETER_REVISION: &str = "parameters-1";

/// The Control.PID loop of the apply tests (`pidLoop` with `pidInitialization`; `control-studio-pid-fixtures.ts`): the setpoint `Main.SP`
/// and the measurement `Main.PV` in m, the latter with the permitted range 0 to 10 m of the apply process-time test, the manipulated
/// variable `Main.MV` in V within ±20, a 20 ms task, the parallel PID gains `Main.Kp`, `Main.Ki` and `Main.Kd` within ±1e6 (N6.4), the
/// states `Main.Integral` and `Main.PreviousError`, and the mode `Main.Manual`, TRUE in Manual, which the recording reads as the
/// controller's mode.
#[must_use]
pub(crate) fn pid_loop() -> LoopDefinition {
    decode_loop(&loop_file(pid_law(), Some(parallel_initialization())))
}

/// The loop of the observation tests (`RANGE` of the apply process-time test): the PID loop whose measured output is `P1.RoomC` in degC
/// with the permitted range 0 to 10 degC, and, with `secondary`, a further measured output of role PV, `P2.Pressure` in bar with its own
/// range of 1 to 3 bar, recorded as the channel `pv-secondary` (B1.4).
#[must_use]
pub(crate) fn room_loop(secondary: bool) -> LoopDefinition {
    let mut file = loop_file(pid_law(), Some(parallel_initialization()));
    file["signals"]["processVariable"] = json!({ "symbol": "P1.RoomC", "address": "%IL0", "iecType": "LREAL",
        "direction": "read", "unit": "degC", "range": { "minimum": 0, "maximum": 10 } });
    file["signals"]["setpoint"]["unit"] = json!("degC");
    if secondary {
        file["signals"]["additional"] = json!([{ "id": "pv-secondary", "channel": "secondary", "role": "PV",
            "symbol": "P2.Pressure", "address": "%IL8", "iecType": "LREAL", "direction": "read", "unit": "bar",
            "range": { "minimum": 1, "maximum": 3 } }]);
    }
    decode_loop(&file)
}

fn pid_law() -> Value {
    json!({ "kind": "pid", "parameterRecordSymbol": "Main.Parameters", "proportionalGainSymbol": "Main.Kp",
        "integralGainSymbol": "Main.Ki", "derivativeGainSymbol": "Main.Kd" })
}

fn parallel_initialization() -> Value {
    json!({ "kind": "parallel-pid-v1",
        "integralState": { "symbol": "Main.Integral", "iecType": "LREAL", "unit": "V", "minimum": -20, "maximum": 20 },
        "derivativeState": { "symbol": "Main.PreviousError", "iecType": "LREAL", "unit": "m", "minimum": -20, "maximum": 20 },
        "proportionalSetpointWeight": 1, "derivativeSetpointWeight": 1, "maximumSnapshotAgeMs": 100 })
}

/// The proportional loop of the apply tests: the PID loop with the law `proportional` on `Main.Kp`, as the `TypeScript` fake turns it
/// (C13). Its record keeps the three gains, so a P candidate writes its integral and derivative gains as explicit zeros (C0).
#[must_use]
pub(crate) fn proportional_loop() -> LoopDefinition {
    decode_loop(&loop_file(
        json!({ "kind": "proportional", "gainSymbol": "Main.Kp" }),
        None,
    ))
}

/// The loop of a program's own PID law (C2) on the apply tests' signals and gains: the seed names `Main.Kp`, `Main.Ki` and `Main.Kd`,
/// and the declared states are `Main.Integral`, started by `pid-integral` on `Main.Kp`, and `Main.PreviousError`, started by
/// `weighted-error`: the variables of `pid_loop`.
#[must_use]
pub(crate) fn own_pid_loop() -> LoopDefinition {
    let integral = InitializedState {
        symbol: "Main.Integral".to_owned(),
        iec_type: StateType::Lreal,
        unit: "V".to_owned(),
        minimum: -20.0,
        maximum: 20.0,
        initial: InitialValue::From(InitialSource::PidIntegral {
            index: 0,
            input_index: 0,
            gain_symbol: "Main.Kp".to_owned(),
            weight_symbol: None,
            weight: None,
        }),
        post_scan: PostScan::Bounded,
    };
    let previous = InitializedState {
        symbol: "Main.PreviousError".to_owned(),
        iec_type: StateType::Lreal,
        unit: "m".to_owned(),
        minimum: -20.0,
        maximum: 20.0,
        initial: InitialValue::From(InitialSource::WeightedError {
            index: 0,
            weight_symbol: None,
            weight: Some(1.0),
        }),
        post_scan: PostScan::Bounded,
    };
    own_law_loop(
        "PID",
        json!({ "kind": "parallel-pid", "family": "PID", "proportionalSymbol": "Main.Kp",
            "integralSymbol": "Main.Ki", "derivativeSymbol": "Main.Kd" }),
        &[integral, previous],
        &[],
    )
}

/// The loop of a program's own P law (C2) with its startup-done flag (C1): the seed names `Main.Kp`, and `Main.Started` is an application
/// state that the Auto entry sets TRUE (C13 Apply step 4).
#[must_use]
pub(crate) fn own_p_loop_with_startup() -> LoopDefinition {
    let started = InitializedState {
        symbol: "Main.Started".to_owned(),
        iec_type: StateType::Bool,
        unit: "1".to_owned(),
        minimum: 0.0,
        maximum: 1.0,
        initial: InitialValue::Constant(1.0),
        post_scan: PostScan::Unchanged,
    };
    own_law_loop(
        "P",
        json!({ "kind": "parallel-pid", "family": "P", "proportionalSymbol": "Main.Kp" }),
        &[],
        &[started],
    )
}

/// The loop file of a program's own law (C2) on the apply tests' signals and gains: the controller is `design: project` with no law, and
/// its `project` block holds the family, the declared and application states and the seed, as the setup writes them.
#[allow(clippy::needless_pass_by_value)] // A test fixture: `json!` borrows the seed, and the fixture takes the JSON literal in place.
fn own_law_loop(
    family: &str,
    seed: Value,
    states: &[InitializedState],
    application: &[InitializedState],
) -> LoopDefinition {
    let mut file = loop_file(Value::Null, None);
    file["controller"]["design"] = json!("project");
    file["controller"]["project"] = json!({
        "family": family,
        "convention": "project-owned-native-controller",
        "initialization": {
            "kind": "declared-state-v1",
            "states": serde_json::to_value(states).expect("the states serialize"),
            "applicationStates": serde_json::to_value(application).expect("the states serialize")
        },
        "seed": seed,
        "axes": [],
        "objective": { "requirementId": "target-error", "direction": "minimize" },
        "maximumCandidates": 4,
        "maximumCycles": 50000,
        "wallBudgetMs": 60000
    });
    decode_loop(&file)
}

#[allow(clippy::needless_pass_by_value)] // A test fixture: it takes the JSON literals its callers write in place.
fn loop_file(law: Value, initialization: Option<Value>) -> Value {
    let gain = |symbol: &str, unit: &str, convention: &str| json!({ "symbol": symbol, "iec_type": "LREAL", "unit": unit, "convention": convention, "minimum": -1e6, "maximum": 1e6 });
    let mut controller = json!({
        "instance": "Main", "type": "Control.PID", "revision": "sha256:pid-controller-1", "action": "direct",
        "taskPeriodMs": 20, "modeSignal": "Main.Manual", "modeWhenTrue": "Manual",
        "manualOutputSignal": "Main.ManualCommand", "manualOutputAddress": "%ML8", "manualOutputType": "LREAL"
    });
    // A loop without a controller law leaves the key out: the engine refuses a null in a field it reads (B1.2).
    if !law.is_null() {
        controller["law"] = law;
    }
    if let Some(initialization) = initialization {
        controller["design"] = json!("pid");
        controller["pidInitialization"] = initialization;
    }
    json!({
        "schemaVersion": 2, "id": "loop:pid-apply", "name": "PID apply", "revision": "sha256:pid-apply-1",
        "projectId": "project:pid-apply", "runtimeId": "runtime:pid-apply",
        "controller": controller,
        "signals": {
            "setpoint": { "symbol": "Main.SP", "address": "%ML0", "iecType": "LREAL", "direction": "read",
                "unit": "m", "range": { "minimum": -20, "maximum": 20 } },
            "processVariable": { "symbol": "Main.PV", "address": "%IL0", "iecType": "LREAL", "direction": "read",
                "unit": "m", "range": { "minimum": 0, "maximum": 10 } },
            "manipulatedVariable": { "symbol": "Main.MV", "address": "%QL0", "iecType": "LREAL", "direction": "write",
                "unit": "V", "range": { "minimum": -20, "maximum": 20 } }
        },
        "limits": { "outputMinimum": -20, "outputMaximum": 20 },
        "observedSampleTiming": { "nominalPeriodMs": 20, "maximumJitterMs": 1 },
        "parameterBinding": { "loop_id": "loop:pid-apply", "task_name": "MainTask",
            "parameters": [
                gain("Main.Kp", "V/m", "proportional gain"),
                gain("Main.Ki", "V/(m*s)", "integral gain"),
                gain("Main.Kd", "V*s/m", "derivative gain")
            ],
            "mode": { "symbol": "Main.Manual", "manual_value": true } }
    })
}

fn decode_loop(file: &Value) -> LoopDefinition {
    LoopDefinition::decode(&file.to_string()).expect("the fixture loop decodes")
}

/// The model of the apply tests (`singleOutputModel`): `1/(s + 1)` from `u` in V to `y` in m, each operating within ±20, validated on a
/// bench, at the operating point input 0 V and output 2 m (B6.2).
#[must_use]
pub(crate) fn model() -> GeneralModel {
    decode_general_model(&json!({
        "kind": "transfer-function", "revision": "first-order", "source": "provided", "validation": "validated",
        "evidence": ["bench"], "delaySeconds": 0, "numerator": [1], "denominator": [1, 1],
        "inputs": [{ "id": "u", "unit": "V", "operatingRange": { "minimum": -20, "maximum": 20 } }],
        "outputs": [{ "id": "y", "unit": "m", "operatingRange": { "minimum": -20, "maximum": 20 } }],
        "operatingPoint": { "inputs": [0], "outputs": [2] }
    }))
    .expect("the fixture model decodes")
}

/// The requirements of the apply tests (`TASK`): a reference change of 1, an error limit of 0.1, a settling band of 0.02, a settling time
/// of 2 s and a test duration of 4 s.
#[must_use]
pub(crate) fn task() -> EngineerRequirements {
    engineer_requirements(
        &json!({ "referenceChange": 1, "maximumError": 0.1, "settlingBand": 0.02,
        "settlingTimeSeconds": 2, "durationSeconds": 4 }),
    )
    .expect("the fixture requirements are valid")
}

/// The candidate of the `TypeScript` apply tests as its gains: `Kp 2`, `Ki 0.5` for the PID or `0` for the P law, and `Kd 0`.
#[must_use]
pub(crate) fn gains(integral_gain: f64) -> ControllerGains {
    ControllerGains {
        proportional_gain: Some(2.0),
        integral_gain: Some(integral_gain),
        derivative_gain: Some(0.0),
        ..ControllerGains::default()
    }
}

/// What the study gives the run in the tests: the model, the requirements, the gains, a validation identity and the measured noise.
#[must_use]
pub(crate) fn inputs(gains: &ControllerGains, process_noise: f64) -> ApplyInputs {
    ApplyInputs {
        model: model(),
        task: task(),
        gains: *gains,
        coefficients: None,
        validation_identity: "validation:1".into(),
        process_noise,
        schedule: None,
    }
}

/// The pauses of the tests: none, with the wall clock `wall_ms`.
#[must_use]
pub(crate) fn instant(wall_ms: &dyn Fn() -> u64) -> ApplyTiming<'_> {
    ApplyTiming {
        wait_poll: Duration::ZERO,
        follow_poll: Duration::ZERO,
        wall_ms,
    }
}

/// A folder of its own for one test's journal.
#[must_use]
pub(crate) fn scratch(name: &str) -> PathBuf {
    let folder =
        std::env::temp_dir().join(format!("trust-control-apply-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    folder
}

/// The journal file of `loop_definition` in `storage_folder`, where `Journal` keeps it (A8.3).
#[must_use]
pub(crate) fn journal_file(storage_folder: &Path, loop_definition: &LoopDefinition) -> PathBuf {
    storage_folder.join("control-studio").join(format!(
        "{}.journal.json",
        folder_name(&format!(
            "{} {}",
            loop_definition.project_id, loop_definition.id
        ))
    ))
}

/// The operation's report in the tests: the progress shown, and the cancel, which a test may set at once or on the first progress.
#[derive(Default)]
pub(crate) struct Report {
    cancelled: AtomicBool,
    cancel_on_progress: bool,
    lines: Mutex<Vec<String>>,
}

impl Report {
    /// Cancels the operation.
    pub(crate) fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// The progress shown, oldest first.
    #[must_use]
    pub(crate) fn lines(&self) -> Vec<String> {
        self.lines.lock().expect("the progress").clone()
    }
}

impl CampaignReport for Report {
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
            || (self.cancel_on_progress && !self.lines.lock().expect("the progress").is_empty())
    }

    fn progress(&self, text: &str) {
        self.lines
            .lock()
            .expect("the progress")
            .push(text.to_owned());
    }
}

/// How the measured output answers the reviewed target once the loop is in Auto at it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Response {
    /// The measurement follows the target exactly: the response passes.
    FollowsTarget,
    /// The measurement stays where it was: the target error fails its requirement (B6.3).
    Stays,
    /// The measurement goes to this value, outside its permitted range (B6.3, B6.7).
    Departs(f64),
}

/// A transfer the plant accepted and commits at its next status read (E6.5, E6.6).
struct PendingTransfer {
    id: String,
    binding: Value,
    values: Map<String, Value>,
    manual: bool,
    tracking: Value,
}

/// A parameter set the plant accepted and commits at its next status read (E7.3, E7.5).
struct PendingParameters {
    id: String,
    values: BTreeMap<String, f64>,
    rollback: bool,
}

/// The recording the plant holds (E4).
pub(crate) struct Recording {
    id: String,
    loop_revision: String,
    program_revision: Option<String>,
    bindings: Vec<Value>,
    maximum_duration_ms: u64,
    samples: Vec<Value>,
    stopped: bool,
    released: bool,
}

/// The plant behind the fake runtime: its values, its transactions, its recording, and the log of every write and recording call.
pub(crate) struct World {
    /// The loaded program's revision (A2.2); a test changes it to reload the program.
    pub(crate) program: String,
    /// Whether the runtime is physical (A2.4): some attached I/O is not simulated.
    pub(crate) physical: bool,
    /// How the measurement answers the reviewed target.
    pub(crate) response: Response,
    /// Every numeric variable of the plant, by symbol.
    pub(crate) numbers: BTreeMap<String, f64>,
    /// Every BOOL variable of the plant, by symbol.
    pub(crate) modes: BTreeMap<String, bool>,
    /// The writes and recording calls, in arrival order: `transfer manual`, `transfer auto`, `parameters apply`, `parameters
    /// rollback`, `transfer cancel`, `parameters cancel`, `recording start`, `recording stop`, `recording release`.
    pub(crate) log: Vec<String>,
    /// The journal file to read at the first write, to see what was journaled before it (E11.2).
    pub(crate) journal_file: Option<PathBuf>,
    /// The journal's text when the first write arrived, and that write's transaction identifier.
    pub(crate) first_write: Option<(Option<String>, String)>,
    /// The last transfer transaction (E6.8).
    pub(crate) transaction: Option<Value>,
    /// The recording the plant holds.
    pub(crate) recording: Option<Recording>,
    addresses: BTreeMap<String, String>,
    mode_symbol: String,
    manual_value: bool,
    setpoint: String,
    process: String,
    parameters: Vec<String>,
    time_ns: i64,
    scan: u64,
    active: String,
    previous: Option<String>,
    previous_values: Option<BTreeMap<String, f64>>,
    parameter_status: &'static str,
    parameter_transaction: Option<String>,
    revisions: u32,
    pending_transfer: Option<PendingTransfer>,
    pending_parameters: Option<PendingParameters>,
}

/// The fake runtime of one plant (`FakeRuntime`), and its plant.
pub(crate) struct Plant {
    fake: FakeRuntime,
    world: Arc<Mutex<World>>,
}

impl Plant {
    /// A plant of `loop_definition` at the starting point of the apply tests: the setpoint at 2 m, the measurement at 2.004 m (the
    /// validated point is 2 m), the output at 0 V, the states at zero, the gains `Kp 1`, `Ki 0`, `Kd 0` in force under `original`, and
    /// the loop in Auto when `found_in_auto`, in Manual otherwise.
    #[must_use]
    pub(crate) fn start(
        loop_definition: &LoopDefinition,
        found_in_auto: bool,
        response: Response,
    ) -> Self {
        let signals = &loop_definition.signals;
        let binding = loop_definition
            .parameter_binding
            .as_ref()
            .expect("the fixture binds its parameters");
        let mut addresses = BTreeMap::new();
        for signal in [
            signals.setpoint.as_ref(),
            Some(&signals.process_variable),
            Some(&signals.manipulated_variable),
        ]
        .into_iter()
        .flatten()
        {
            if let Some(address) = &signal.address {
                addresses.insert(address.clone(), signal.symbol.clone());
            }
        }
        let numbers = [
            ("Main.SP", 2.0),
            ("Main.PV", 2.004),
            ("Main.MV", 0.0),
            ("Main.ManualCommand", 0.0),
            ("Main.Integral", 0.0),
            ("Main.PreviousError", 0.0),
            ("Main.Kp", 1.0),
            ("Main.Ki", 0.0),
            ("Main.Kd", 0.0),
        ]
        .into_iter()
        .map(|(symbol, value)| (symbol.to_owned(), value))
        .collect();
        let manual_value = binding.mode.manual_value;
        let world = World {
            program: "program-1".into(),
            physical: false,
            response,
            numbers,
            modes: BTreeMap::from([(
                binding.mode.symbol.clone(),
                if found_in_auto {
                    !manual_value
                } else {
                    manual_value
                },
            )]),
            log: Vec::new(),
            journal_file: None,
            first_write: None,
            transaction: None,
            recording: None,
            addresses,
            mode_symbol: binding.mode.symbol.clone(),
            manual_value,
            setpoint: signals
                .setpoint
                .as_ref()
                .map(|signal| signal.symbol.clone())
                .unwrap_or_default(),
            process: signals.process_variable.symbol.clone(),
            parameters: binding
                .parameters
                .iter()
                .map(|parameter| parameter.symbol.clone())
                .collect(),
            time_ns: 1_000_000_000,
            scan: 0,
            active: "original".into(),
            previous: None,
            previous_values: None,
            parameter_status: "applied",
            parameter_transaction: None,
            revisions: 0,
            pending_transfer: None,
            pending_parameters: None,
        };
        let world = Arc::new(Mutex::new(world));
        let served = Arc::clone(&world);
        let fake = FakeRuntime::start(move |request| {
            Some(served.lock().expect("the plant").handle(request))
        });
        Self { fake, world }
    }

    /// The connection a client uses.
    #[must_use]
    pub(crate) fn connection(&self) -> RuntimeConnection {
        self.fake.connection(None)
    }

    /// The plant, for a test to read or script it.
    pub(crate) fn world(&self) -> MutexGuard<'_, World> {
        self.world.lock().expect("the plant")
    }
}

impl World {
    /// The value of a numeric variable.
    #[must_use]
    pub(crate) fn number(&self, symbol: &str) -> f64 {
        self.numbers.get(symbol).copied().unwrap_or(0.0)
    }

    /// Whether the loop is in Auto.
    #[must_use]
    pub(crate) fn in_auto(&self) -> bool {
        self.modes.get(&self.mode_symbol) == Some(&!self.manual_value)
    }

    /// Holds a recording that a runtime started earlier, so that a reopened engine reattaches to it (A8.6): its identity, its bindings
    /// and its duration limit, with no sample yet.
    pub(crate) fn hold_recording(
        &mut self,
        id: &str,
        loop_revision: &str,
        program_revision: Option<&str>,
        bindings: Vec<Value>,
        maximum_duration_ms: u64,
    ) {
        self.recording = Some(Recording {
            id: id.to_owned(),
            loop_revision: loop_revision.to_owned(),
            program_revision: program_revision.map(str::to_owned),
            bindings,
            maximum_duration_ms,
            samples: Vec::new(),
            stopped: false,
            released: false,
        });
    }

    /// Whether the plant's recording has been released (E4.7).
    #[must_use]
    pub(crate) fn recording_released(&self) -> bool {
        self.recording
            .as_ref()
            .is_some_and(|recording| recording.released)
    }

    fn handle(&mut self, request: &Value) -> Value {
        let params = &request["params"];
        match request["type"].as_str().unwrap_or_default() {
            "status" => answer(request, self.runtime_status()),
            "control.signals.inspect" => answer(request, self.inspection(params)),
            "control.recording.start" => self.start_recording(request, params),
            "control.recording.status" => self.recording_status(request, params),
            "control.recording.stop" => self.stop_recording(request),
            "control.recording.release" => self.release_recording(request),
            "control.transfer.status" => {
                self.commit();
                self.time_ns += 100_000_000;
                answer(request, self.transfer_status(&params["binding"]))
            }
            "control.transfer.apply" => self.apply_transfer(request, params),
            "control.transfer.cancel" => self.cancel_transfer(request, params),
            "control.parameters.status" => {
                self.commit();
                answer(request, self.parameter_record())
            }
            "control.parameters.apply" => self.apply_parameters(request, params, false),
            "control.parameters.rollback" => self.apply_parameters(request, params, true),
            "control.parameters.cancel" => self.cancel_parameters(request, params),
            other => refusal(
                request,
                "invalid_request",
                &format!("The plant does not answer {other}."),
            ),
        }
    }

    /// The runtime's facts (E1.3): a simulation at 1x whose I/O is simulated unless the plant is physical, the task `MainTask` at
    /// 20 ms and a 10 ms resource cycle.
    fn runtime_status(&self) -> Value {
        json!({
            "state": "running", "session_epoch": 7, "simulation_enabled": true, "simulation_only_io": !self.physical,
            "simulation_time_scale": 1, "simulation_achieved_time_scale": null, "program_revision": self.program,
            "tasks": [{ "name": "MainTask", "interval_ms": 20 }], "cycle_interval_ms": 10
        })
    }

    fn value_of(&self, symbol: &str) -> f64 {
        match self.modes.get(symbol) {
            Some(mode) => f64::from(u8::from(*mode)),
            None => self.number(symbol),
        }
    }

    fn binding_value(&self, binding: &Value) -> f64 {
        if let Some(symbol) = binding["symbol"].as_str() {
            return self.value_of(symbol);
        }
        binding["address"]
            .as_str()
            .and_then(|address| self.addresses.get(address))
            .map_or(0.0, |symbol| self.value_of(symbol))
    }

    /// One coherent scan of the inspected signals (E3).
    fn inspection(&self, params: &Value) -> Value {
        let bindings = params["bindings"].as_array().cloned().unwrap_or_default();
        let values: Vec<Value> = bindings
            .iter()
            .map(|binding| json!(self.binding_value(binding)))
            .collect();
        json!({
            "program_revision": self.program, "loop_revision": params["loop_revision"], "bindings": bindings,
            "available": true, "scan": self.scan, "runtime_time_ns": self.time_ns, "captured_at_ms": 1000,
            "values": values, "forced": vec![false; bindings.len()], "quality": vec!["good"; bindings.len()]
        })
    }

    fn start_recording(&mut self, request: &Value, params: &Value) -> Value {
        let id = params["recording_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        self.log.push("recording start".into());
        self.hold_recording(
            &id,
            params["loop_revision"].as_str().unwrap_or_default(),
            params["program_revision"].as_str(),
            params["bindings"].as_array().cloned().unwrap_or_default(),
            params["maximum_duration_ms"].as_u64().unwrap_or(0),
        );
        answer(
            request,
            json!({ "recording_id": id, "status": "recording" }),
        )
    }

    /// One page of the recording (E4.6). Each read records another second of the plant at the task's 20 ms, until the recording's
    /// duration limit stops it.
    fn recording_status(&mut self, request: &Value, params: &Value) -> Value {
        self.commit();
        self.respond();
        let Some(mut recording) = self.recording.take() else {
            return refusal(
                request,
                "recording_unavailable",
                "The recording is not held.",
            );
        };
        if recording.released {
            self.recording = Some(recording);
            return refusal(
                request,
                "recording_unavailable",
                "The recording is not held.",
            );
        }
        if !recording.stopped {
            for _ in 0..50 {
                if elapsed_ms(&recording.samples) >= recording.maximum_duration_ms {
                    recording.stopped = true;
                    break;
                }
                self.time_ns += 20_000_000;
                self.scan += 1;
                let values: Vec<Value> = recording
                    .bindings
                    .iter()
                    .map(|binding| json!(self.binding_value(binding)))
                    .collect();
                let count = values.len();
                recording.samples.push(json!({
                    "scan": self.scan, "runtime_time_ns": self.time_ns, "captured_at_ms": 1000,
                    "values": values, "forced": vec![false; count], "quality": vec!["good"; count]
                }));
            }
        }
        let from = params["from_sample"]
            .as_u64()
            .map_or(0, |from| usize::try_from(from).unwrap_or(usize::MAX))
            .min(recording.samples.len());
        let page: Vec<Value> = recording.samples[from..].to_vec();
        let status = json!({
            "recording_id": recording.id, "state": if recording.stopped { "stopped" } else { "recording" },
            "elapsed_ms": elapsed_ms(&recording.samples), "sample_count": recording.samples.len(), "complete": true,
            "reason": null, "next_sample": from + page.len(), "samples": page
        });
        self.recording = Some(recording);
        answer(request, status)
    }

    fn stop_recording(&mut self, request: &Value) -> Value {
        self.log.push("recording stop".into());
        let Some(recording) = self
            .recording
            .as_mut()
            .filter(|recording| !recording.released)
        else {
            return refusal(
                request,
                "recording_unavailable",
                "The recording is not held.",
            );
        };
        recording.stopped = true;
        let count = recording.samples.len();
        let scan_range = match (recording.samples.first(), recording.samples.last()) {
            (Some(first), Some(last)) => json!({ "first": first["scan"], "last": last["scan"] }),
            _ => Value::Null,
        };
        let mut run = json!({
            "recording_id": recording.id, "loop_revision": recording.loop_revision, "bindings": recording.bindings,
            "stop_reason": "stopped", "samples": recording.samples,
            "acquisition": { "method": "runtime_scan_buffer", "completeness": "complete", "observedSampleCount": count,
                "scanRange": scan_range, "missingScanRanges": [], "droppedObservedSamples": 0,
                "actualCycleTimingAvailable": true, "freshnessAvailable": false, "reason": "Captured every scan." }
        });
        if let Some(program) = &recording.program_revision {
            run["program_revision"] = json!(program);
        }
        answer(request, run)
    }

    fn release_recording(&mut self, request: &Value) -> Value {
        self.log.push("recording release".into());
        let id = request["params"]["recording_id"].clone();
        if let Some(recording) = self.recording.as_mut() {
            recording.released = true;
        }
        answer(request, json!({ "recording_id": id, "status": "released" }))
    }

    /// The transfer status of `binding` (E6.3): its modes and members, and the last transaction.
    fn transfer_status(&self, binding: &Value) -> Value {
        let transaction = self.transaction.clone().unwrap_or(Value::Null);
        json!({
            "program_revision": self.program, "binding_revision": TRANSFER_REVISION, "runtime_time_ns": self.time_ns,
            "values": self.transfer_values(binding),
            "supported_tracking": ["proportional", "parallel-pid-v1", "declared-state-v1", "declared-initialization-formulas"],
            "transaction_id": transaction["id"], "status": transaction.get("status").cloned().unwrap_or(json!("idle")),
            "reason": transaction.get("reason").cloned().unwrap_or(Value::Null), "transaction": transaction
        })
    }

    fn transfer_values(&self, binding: &Value) -> Map<String, Value> {
        let mut values = Map::new();
        let modes = std::iter::once(&binding["mode"]).chain(
            binding["additional_modes"]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default(),
        );
        for mode in modes {
            let symbol = mode["symbol"].as_str().unwrap_or_default();
            values.insert(
                symbol.to_owned(),
                json!(self.modes.get(symbol).copied().unwrap_or(false)),
            );
        }
        for group in ["states", "guards", "additional_outputs"] {
            for member in binding[group]
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                let symbol = member["symbol"].as_str().unwrap_or_default();
                values.insert(symbol.to_owned(), json!(self.number(symbol)));
            }
        }
        let output = binding["output"]["symbol"].as_str().unwrap_or_default();
        values.insert(output.to_owned(), json!(self.number(output)));
        values
    }

    fn first_write(&mut self, transaction_id: &str) {
        if self.first_write.is_none() {
            let journal = self
                .journal_file
                .as_ref()
                .and_then(|file| std::fs::read_to_string(file).ok());
            self.first_write = Some((journal, transaction_id.to_owned()));
        }
    }

    fn apply_transfer(&mut self, request: &Value, params: &Value) -> Value {
        let manual = params["manual"].as_bool().unwrap_or(true);
        let id = params["transaction_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        self.log.push(
            if manual {
                "transfer manual"
            } else {
                "transfer auto"
            }
            .into(),
        );
        self.first_write(&id);
        self.pending_transfer = Some(PendingTransfer {
            id,
            binding: params["binding"].clone(),
            values: params["values"].as_object().cloned().unwrap_or_default(),
            manual,
            tracking: params["tracking"].clone(),
        });
        answer(request, json!({ "status": "pending", "confirmed": false }))
    }

    fn cancel_transfer(&mut self, request: &Value, params: &Value) -> Value {
        self.log.push("transfer cancel".into());
        let id = params["transaction_id"].clone();
        match &self.transaction {
            Some(transaction) if transaction["id"] == id => answer(
                request,
                json!({ "status": transaction["status"], "confirmed": true }),
            ),
            _ => answer(
                request,
                json!({ "status": "cancelled", "confirmed": true, "before_dispatch": true, "transaction_id": id,
                    "program_revision": self.program, "binding_revision": TRANSFER_REVISION }),
            ),
        }
    }

    fn apply_parameters(&mut self, request: &Value, params: &Value, rollback: bool) -> Value {
        let id = params["transaction_id"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        self.log.push(
            if rollback {
                "parameters rollback"
            } else {
                "parameters apply"
            }
            .into(),
        );
        self.first_write(&id);
        if self.in_auto() {
            return refusal(
                request,
                "invalid_parameter_request",
                "confirmed Manual mode required",
            );
        }
        let values = if rollback {
            self.previous_values.clone().unwrap_or_default()
        } else {
            params["values"]
                .as_object()
                .map(|values| {
                    values
                        .iter()
                        .map(|(symbol, value)| (symbol.clone(), value.as_f64().unwrap_or(f64::NAN)))
                        .collect()
                })
                .unwrap_or_default()
        };
        self.pending_parameters = Some(PendingParameters {
            id,
            values,
            rollback,
        });
        answer(request, json!({ "status": "pending", "confirmed": false }))
    }

    fn cancel_parameters(&mut self, request: &Value, params: &Value) -> Value {
        self.log.push("parameters cancel".into());
        let id = params["transaction_id"].clone();
        if self.parameter_transaction.as_deref() == id.as_str() {
            return answer(
                request,
                json!({ "confirmed": false, "reason": "parameter commit requires post-ST observation" }),
            );
        }
        answer(
            request,
            json!({ "transaction_id": id, "program_revision": self.program, "binding_revision": PARAMETER_REVISION,
                "confirmed": true, "before_dispatch": true }),
        )
    }

    /// The parameter record (E7.2).
    fn parameter_record(&self) -> Value {
        let values: BTreeMap<&str, f64> = self
            .parameters
            .iter()
            .map(|symbol| (symbol.as_str(), self.number(symbol)))
            .collect();
        json!({
            "supports_request_id": true, "transaction_id": self.parameter_transaction, "program_revision": self.program,
            "binding_revision": PARAMETER_REVISION, "active_revision": self.active, "values": values,
            "status": self.parameter_status, "previous_revision": self.previous, "previous_values": self.previous_values,
            "reason": null
        })
    }

    /// Commits the queued transfer and parameter set at this task boundary (E6.6, E6.7, E7.4).
    fn commit(&mut self) {
        if let Some(pending) = self.pending_parameters.take() {
            let current: BTreeMap<String, f64> = self
                .parameters
                .iter()
                .map(|symbol| (symbol.clone(), self.number(symbol)))
                .collect();
            self.revisions += 1;
            if pending.rollback {
                self.active = self.previous.take().unwrap_or_default();
                self.previous_values = None;
            } else {
                self.previous = Some(self.active.clone());
                self.previous_values = Some(current);
                self.active = format!("applied-{}", self.revisions);
            }
            self.numbers.extend(pending.values);
            self.parameter_status = "applied";
            self.parameter_transaction = Some(pending.id);
        }
        if let Some(pending) = self.pending_transfer.take() {
            self.commit_transfer(&pending);
        }
    }

    /// Commits a transfer (E6.4, E6.6, E6.7): the mode and the states it writes, the Manual command at the held output and the aligned
    /// reference of a proportional tracking, then the observation after the task executed.
    fn commit_transfer(&mut self, pending: &PendingTransfer) {
        let initial = self.transfer_values(&pending.binding);
        let mut applied: BTreeMap<String, f64> = pending
            .values
            .iter()
            .map(|(symbol, value)| (symbol.clone(), value.as_f64().unwrap_or(f64::NAN)))
            .collect();
        if pending.tracking["kind"] == "proportional" {
            let held = self.number(
                pending.binding["output"]["symbol"]
                    .as_str()
                    .unwrap_or_default(),
            );
            let manual = pending.tracking["manual_command_symbol"]
                .as_str()
                .unwrap_or_default();
            applied.insert(manual.to_owned(), held);
            if pending.tracking["align_reference"] == true {
                let process = self.number(
                    pending.tracking["process_variable"]
                        .as_str()
                        .unwrap_or_default(),
                );
                let gain =
                    self.number(pending.tracking["gain_symbol"].as_str().unwrap_or_default());
                let reference = pending.tracking["reference_symbol"]
                    .as_str()
                    .unwrap_or_default();
                applied.insert(reference.to_owned(), process + held / gain);
            }
        }
        // A declared-state entry (E6.4, `declared-state-v1`) holds the Manual command at the held output and, when it aligns its
        // output, sets the reference to the measurement, as the runtime's tracking does (`transfer/tracking.rs` `calculate`).
        if pending.tracking["kind"] == "declared-state-v1" {
            let held = self.number(
                pending.binding["output"]["symbol"]
                    .as_str()
                    .unwrap_or_default(),
            );
            if let Some(manual) = pending.tracking["manual_command_symbol"].as_str() {
                applied.insert(manual.to_owned(), held);
            }
            if pending.tracking["align_output"] == true {
                if let (Some(reference), Some(process)) = (
                    pending.tracking["reference_symbol"].as_str(),
                    pending.tracking["process_variable"].as_str(),
                ) {
                    applied.insert(reference.to_owned(), self.number(process));
                }
            }
        }
        let mode = if pending.manual {
            self.manual_value
        } else {
            !self.manual_value
        };
        self.modes.insert(self.mode_symbol.clone(), mode);
        self.numbers.extend(applied.clone());
        self.respond();
        self.transaction = Some(json!({
            "id": pending.id, "program_revision": self.program, "binding_revision": TRANSFER_REVISION,
            "status": "applied", "reason": null, "restored": false, "initial_values": initial,
            "applied_values": applied, "observed_values": self.transfer_values(&pending.binding),
            "confirmation": "output-image"
        }));
    }

    /// The measurement's answer once the loop is in Auto at the reviewed target.
    #[allow(clippy::float_cmp)] // The plant holds the target the transfer wrote, exactly.
    fn respond(&mut self) {
        if !self.in_auto() || self.number(&self.setpoint) != TARGET {
            return;
        }
        let process = self.process.clone();
        match self.response {
            Response::FollowsTarget => {
                self.numbers.insert(process, TARGET);
            }
            Response::Stays => {}
            Response::Departs(value) => {
                self.numbers.insert(process, value);
            }
        }
    }
}

/// The process time from the first sample of a recording to its last, in ms.
fn elapsed_ms(samples: &[Value]) -> u64 {
    match (samples.first(), samples.last()) {
        (Some(first), Some(last)) => {
            let span = last["runtime_time_ns"].as_i64().unwrap_or(0)
                - first["runtime_time_ns"].as_i64().unwrap_or(0);
            u64::try_from(span / 1_000_000).unwrap_or(0)
        }
        _ => 0,
    }
}
