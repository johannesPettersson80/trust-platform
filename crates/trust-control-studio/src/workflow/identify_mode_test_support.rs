//! Test support of Identify's mode change: a fake runtime that answers the transfers of annex E6 over the values of the tank
//! loop, the tank loop with a program's parallel PID that Identify transfers, and the ports with which a connected run or a
//! recovery runs on it beside the experiment fake of `identify::test_support`. Every answer carries the fields annex E6.3, E6.5
//! and E6.10 name; the transaction record (E6.8) holds the values that a transfer kept at its boundary, applied and observed, as
//! the checks of `transfer_protocol` read them. One ordered log records the transfers with the recording starts, the keeps and
//! the progress of a run, so that a test reads what happened in its order.
//!
//! The fake applies a transfer as the runtime's declared-state tracking does when it does not align the output: the Manual
//! command takes the output it holds, a state the tracking initializes keeps its value, and every other state takes the value
//! the request writes. The transaction is confirmed by the output image at once (E6.7).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::Mutex;
use serde_json::{json, Map, Value};
use trust_control::commissioning::transfer::TransferBinding;
use trust_control::experiment::{
    ControlExperimentPlan, ControllerMode, ExperimentSample, RecordingRecoveryOwner,
};
use trust_control::failure::Failure;
use trust_control::identification::connected_run::StartRead;
use trust_control::identification::reconcile::StatusRead;
use trust_control::loop_definition::{ExperimentKind, LoopDefinition};
use trust_control::recording::evidence::RecordingEvidence;
use trust_control::recording::RecordedRun;
use trust_control::runtime::RuntimeConnection;

use crate::journal::{Journal, JournalEntry};
use crate::runtime_client::fake_runtime::{answer, refusal, FakeRuntime as ControlEndpoint};
use crate::runtime_client::runtime_recording::{
    RecordingOwnerPersistence, RecordingStartError, RecordingStartPlan,
};
use crate::workflow::identify::run::{run, RunPorts};
use crate::workflow::identify::test_support::{
    live_sample, passes, FakeClock, FakeExperiment, FakeRecording, FakeRuntime, FakeState, Study,
    Verdict,
};
use crate::workflow::identify::{
    IdentifyOutcome, IdentifyRequest, IdentifyRuntime, LoopSampler, RecordingRetention, RunPurpose,
};
use crate::workflow::identify_campaign::CampaignReport;

/// The control token of the fake runtime; it is not a credential.
const TOKEN: &str = "control-studio-fake-token";

/// The tank's mode signal: TRUE in Auto, so its Manual value is FALSE.
pub(crate) const MODE_SIGNAL: &str = "Main.TIC101_Auto";

/// The tank's values as the fake runtime holds them before identification: the setpoint and the measurement at 45 degC, the
/// output at 30 %, a Manual command of 12 % left from an earlier Manual phase, the integral state at 30 %, and the gains Kp = 2,
/// Ki = 0.1 and Kd = 0.
pub(crate) const TANK_VALUES: [(&str, f64); 9] = [
    ("Main.TIC101_Setpoint", 45.0),
    ("Main.TIC101_ManualOutput", 12.0),
    ("Main.TIC101_Integral", 30.0),
    ("Main.TIC101_PreviousError", 0.0),
    ("Main.TIC101_Parameters.Kp", 2.0),
    ("Main.TIC101_Parameters.Ki", 0.1),
    ("Main.TIC101_Parameters.Kd", 0.0),
    ("Main.TT101.Value", 45.0),
    ("Main.TIC101_Output", 30.0),
];

/// The tank of `identify::test_support` (`tank_loop`) at `revision`, with a program's parallel PID that Identify transfers (C2):
/// the gains `Main.TIC101_Parameters.Kp`, `Ki` and `Kd`, the integral and derivative states, the Manual command
/// `Main.TIC101_ManualOutput`, and the mode `Main.TIC101_Auto`, TRUE in Auto, on the task `MainTask`. With `follows`, the program
/// copies the output into the Manual command outside Manual (§17.1). Its experiment is an explicit plant-input step of 10 %.
pub(crate) fn pid_tank_loop(revision: &str, follows: bool) -> LoopDefinition {
    let gain = |symbol: &str, convention: &str| {
        json!({ "symbol": symbol, "iec_type": "LREAL", "unit": "%/degC", "convention": convention,
            "minimum": -100, "maximum": 100 })
    };
    let mut controller = json!({
        "instance": "Main.TIC101", "type": "Control.PID", "revision": "sha256:test", "action": "direct",
        "taskPeriodMs": 100, "modeSignal": MODE_SIGNAL, "modeWhenTrue": "Auto",
        "manualOutputSignal": "Main.TIC101_ManualOutput", "manualOutputType": "LREAL",
        "law": { "kind": "pid", "parameterRecordSymbol": "Main.TIC101_Parameters",
            "proportionalGainSymbol": "Main.TIC101_Parameters.Kp",
            "integralGainSymbol": "Main.TIC101_Parameters.Ki",
            "derivativeGainSymbol": "Main.TIC101_Parameters.Kd" },
        "pidInitialization": { "kind": "parallel-pid-v1",
            "integralState": { "symbol": "Main.TIC101_Integral", "iecType": "LREAL", "unit": "%",
                "minimum": -100, "maximum": 100 },
            "derivativeState": { "symbol": "Main.TIC101_PreviousError", "iecType": "LREAL", "unit": "degC",
                "minimum": -100, "maximum": 100 },
            "proportionalSetpointWeight": 1, "derivativeSetpointWeight": 0, "maximumSnapshotAgeMs": 100 }
    });
    if follows {
        controller["manualOutputFollowsOutput"] = json!(true);
    }
    // Decoded as the engine decodes JSON: the PID's initialization is a tagged record that holds numbers.
    let mut loop_definition: LoopDefinition = trust_control::json::decode_value(json!({
        "schemaVersion": 2, "id": "tank", "name": "Tank", "revision": revision,
        "projectId": "project", "runtimeId": "runtime",
        "controller": controller,
        "parameterBinding": { "loop_id": "tank", "task_name": "MainTask",
            "parameters": [
                gain("Main.TIC101_Parameters.Kp", "proportional gain"),
                gain("Main.TIC101_Parameters.Ki", "integral gain"),
                gain("Main.TIC101_Parameters.Kd", "derivative gain")
            ],
            "mode": { "symbol": MODE_SIGNAL, "manual_value": false } },
        "signals": {
            "setpoint": { "symbol": "Main.TIC101_Setpoint", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "processVariable": { "symbol": "Main.TT101.Value", "iecType": "LREAL", "direction": "read",
                "unit": "degC", "range": { "minimum": 0, "maximum": 100 } },
            "manipulatedVariable": { "symbol": "Main.TIC101_Output", "iecType": "LREAL", "direction": "write",
                "unit": "%", "range": { "minimum": 0, "maximum": 100 } }
        },
        "limits": { "outputMinimum": 0, "outputMaximum": 100 },
        "observedSampleTiming": { "nominalPeriodMs": 100 },
        "experiment": { "execution": "control-studio", "kind": "observation", "amplitude": 0,
            "durationSeconds": 600, "seed": 1 }
    }))
    .expect("the PID tank loop");
    let experiment = loop_definition
        .experiment
        .as_mut()
        .expect("the tank declares its experiment");
    experiment.kind = ExperimentKind::MvStep;
    experiment.amplitude = 10.0;
    loop_definition
}

/// The loop as the fake runtime holds it: the program revision, the mode signal, the value of every member, the last
/// transaction of each transfer loop (annex E12.2), the ordered log, and the journal folder whose mode checkpoint each apply is
/// checked against, when a test gives one.
struct World {
    program_revision: String,
    mode: bool,
    values: BTreeMap<String, f64>,
    transactions: BTreeMap<String, Value>,
    log: Vec<String>,
    journal: Option<PathBuf>,
}

impl World {
    /// The value of `symbol`; a member the test gave no value is 0.
    fn value(&self, symbol: &str) -> f64 {
        self.values.get(symbol).copied().unwrap_or(0.0)
    }

    /// The values of `binding` (annex E6.3): one for each mode, a BOOL, and one for each member, a number.
    fn values_of(&self, binding: &TransferBinding) -> Value {
        let mut values = Map::new();
        for mode in std::iter::once(&binding.mode).chain(&binding.additional_modes) {
            values.insert(mode.symbol.clone(), json!(self.mode));
        }
        for member in binding
            .states
            .iter()
            .chain(&binding.guards)
            .chain(std::iter::once(&binding.output))
            .chain(&binding.additional_outputs)
        {
            values.insert(member.symbol.clone(), json!(self.value(&member.symbol)));
        }
        Value::Object(values)
    }

    /// The answer to `control.transfer.status` (annex E6.3): the values of `binding`, and the last transaction of its loop.
    fn status(&self, binding: &TransferBinding) -> Value {
        let transaction = self
            .transactions
            .get(&binding.loop_id)
            .cloned()
            .unwrap_or(Value::Null);
        let state = if transaction.is_null() {
            json!("idle")
        } else {
            transaction["status"].clone()
        };
        json!({
            "program_revision": self.program_revision,
            "binding_revision": binding_revision(&binding.loop_id),
            "values": self.values_of(binding),
            "status": state,
            "reason": null,
            "transaction": transaction
        })
    }

    /// The answer to `control.transfer.apply` (annex E6.5): the transfer is applied at once and confirmed by the output image,
    /// its record keeping the values before, applied and observed (E6.7, E6.8).
    fn apply(&mut self, params: &Value) -> Value {
        let binding = binding_of(params);
        let id = text_of(params, "transaction_id");
        let manual = params["manual"].as_bool().expect("an apply names its mode");
        let values: BTreeMap<String, f64> =
            trust_control::json::decode_value(params["values"].clone()).expect("the states");
        if self
            .journal
            .as_deref()
            .is_some_and(|folder| journal_owns(folder, &id))
        {
            self.log.push(format!("journaled {}", binding.loop_id));
        }
        let initial = self.values_of(&binding);
        let tracking = params
            .get("tracking")
            .filter(|tracking| tracking.is_object());
        let basis = binding
            .tracking_outputs
            .get(&binding.output.symbol)
            .cloned()
            .unwrap_or_else(|| binding.output.symbol.clone());
        let mut applied = BTreeMap::new();
        for member in &binding.states {
            let symbol = member.symbol.as_str();
            let value = match tracking {
                Some(tracking) if tracking["manual_command_symbol"] == symbol => self.value(&basis),
                Some(tracking) if tracking["initialization_values"].get(symbol).is_some() => {
                    self.value(symbol)
                }
                _ => values
                    .get(symbol)
                    .copied()
                    .expect("every state has a value"),
            };
            applied.insert(member.symbol.clone(), value);
        }
        self.values.extend(applied.clone());
        self.mode = if manual {
            binding.mode.manual_value
        } else {
            !binding.mode.manual_value
        };
        let word = if manual { "manual" } else { "auto" };
        self.log
            .push(format!("transfer.apply {} {word}", binding.loop_id));
        let transaction = json!({
            "id": id, "program_revision": self.program_revision,
            "binding_revision": binding_revision(&binding.loop_id), "status": "applied", "reason": null,
            "restored": false, "initial_values": initial, "applied_values": applied,
            "observed_values": self.values_of(&binding), "confirmation": "output-image"
        });
        self.transactions
            .insert(binding.loop_id.clone(), transaction);
        json!({ "transaction_id": id, "status": "pending", "confirmed": false })
    }

    /// The answer to `control.transfer.cancel` (annex E6.10): a transaction the runtime holds answers with its outcome, and an
    /// identifier it does not hold is answered cancelled before dispatch.
    fn cancel(&mut self, params: &Value) -> Value {
        let loop_id = text_of(params, "loop_id");
        let id = text_of(params, "transaction_id");
        self.log.push(format!("transfer.cancel {loop_id} {id}"));
        match self.transactions.get(&loop_id) {
            Some(transaction) if transaction["id"] == id.as_str() => json!({
                "status": transaction["status"].clone(), "confirmed": true, "reason": null
            }),
            _ => json!({
                "status": "cancelled", "confirmed": true, "reason": null, "transaction_id": id,
                "program_revision": self.program_revision, "binding_revision": binding_revision(&loop_id),
                "before_dispatch": true
            }),
        }
    }
}

/// The binding revision of a transfer loop: one binding per loop in these tests.
fn binding_revision(loop_id: &str) -> String {
    format!("binding:{loop_id}")
}

/// The binding a request names.
fn binding_of(params: &Value) -> TransferBinding {
    trust_control::json::decode_value(params["binding"].clone())
        .expect("the request names its binding")
}

/// The text field `key` of a request.
fn text_of(params: &Value, key: &str) -> String {
    params[key]
        .as_str()
        .expect("the request names the field")
        .to_owned()
}

/// Whether the tank's journal in `folder` holds a mode checkpoint whose owned transfer is `transaction_id`: the owner is
/// journaled before its apply is dispatched (E11.2).
fn journal_owns(folder: &Path, transaction_id: &str) -> bool {
    let Ok(journal) = Journal::open(folder, "project", "tank") else {
        return false;
    };
    journal.entries().iter().any(|entry| {
        if let JournalEntry::IdentificationMode { checkpoint, .. } = entry {
            checkpoint
                .transfer
                .as_ref()
                .is_some_and(|owned| owned.owner.id == transaction_id)
        } else {
            false
        }
    })
}

/// Answers one request on the fake runtime: the status the confirmation timing reads, and the transfers of annex E6.
fn handle(world: &Mutex<World>, request: &Value) -> Option<Value> {
    let params = &request["params"];
    let reply = match request["type"].as_str()? {
        "status" => json!({ "tasks": [] }),
        "control.transfer.status" => world.lock().status(&binding_of(params)),
        "control.transfer.apply" => world.lock().apply(params),
        "control.transfer.cancel" => world.lock().cancel(params),
        _ => {
            return Some(refusal(
                request,
                "invalid_request",
                "The fake runtime does not serve this request.",
            ))
        }
    };
    Some(answer(request, reply))
}

/// The fake runtime of the transfers: a control endpoint whose handler answers annex E6 over the tank's values.
pub(crate) struct TransferFake {
    endpoint: ControlEndpoint,
    world: Arc<Mutex<World>>,
}

impl TransferFake {
    /// The fake of the tank at [`TANK_VALUES`], on the program `program`, with the mode signal at `auto`: TRUE in Auto.
    pub(crate) fn start(auto: bool) -> Self {
        let world = Arc::new(Mutex::new(World {
            program_revision: "program".to_owned(),
            mode: auto,
            values: TANK_VALUES
                .iter()
                .map(|(symbol, value)| ((*symbol).to_owned(), *value))
                .collect(),
            transactions: BTreeMap::new(),
            log: Vec::new(),
            journal: None,
        }));
        let served = Arc::clone(&world);
        let endpoint = ControlEndpoint::start(move |request| handle(&served, request));
        Self { endpoint, world }
    }

    /// The connection a client uses, with the test's control token.
    pub(crate) fn connection(&self) -> RuntimeConnection {
        self.endpoint.connection(Some(TOKEN))
    }

    /// From now on each apply is checked against the tank's journal in `folder`: the log records `journaled <loop>` before the
    /// apply when the journal held its owner already.
    pub(crate) fn watch_journal(&self, folder: &Path) {
        self.world.lock().journal = Some(folder.to_path_buf());
    }

    /// Whether the mode signal reads TRUE, which is Auto.
    pub(crate) fn auto(&self) -> bool {
        self.world.lock().mode
    }

    /// The value the runtime holds for `symbol`.
    pub(crate) fn value(&self, symbol: &str) -> f64 {
        self.world.lock().value(symbol)
    }

    /// A reload of the program: from now on the runtime reports the revision `program` (A6.7).
    pub(crate) fn reload(&self, program: &str) {
        self.world.lock().program_revision = program.to_owned();
    }

    /// Records `entry` in the ordered log.
    pub(crate) fn record(&self, entry: String) {
        self.world.lock().log.push(entry);
    }

    /// The ordered log: each apply and cancel with its transfer loop, each journal check, and what the run's ports recorded.
    pub(crate) fn log(&self) -> Vec<String> {
        self.world.lock().log.clone()
    }

    /// The log without the progress, and with the identifiers of the recordings left out: the order of what was written.
    pub(crate) fn events(&self) -> Vec<String> {
        self.log()
            .into_iter()
            .filter(|entry| !entry.starts_with("progress "))
            .map(|entry| {
                if entry.starts_with("recording.start ") || entry.starts_with("retain ") {
                    entry.split(' ').next().unwrap_or_default().to_owned()
                } else {
                    entry
                }
            })
            .collect()
    }

    /// The parameters of the applies the runtime received, in order.
    pub(crate) fn applies(&self) -> Vec<Value> {
        self.endpoint
            .requests_of("control.transfer.apply")
            .into_iter()
            .map(|request| request["params"].clone())
            .collect()
    }
}

/// The runtime of a connected run with transfers: the experiment fake of `identify::test_support` for its recordings and
/// experiments, and the transfer fake for its mode changes. Each recording start reaches the transfer fake's log.
pub(crate) struct ModeRuntime<'f> {
    inner: FakeRuntime,
    transfers: &'f TransferFake,
}

impl<'f> ModeRuntime<'f> {
    /// The runtime of the experiment fake `state` and the transfer fake `transfers`.
    pub(crate) fn new(state: &Arc<FakeState>, transfers: &'f TransferFake) -> Self {
        Self {
            inner: FakeRuntime::new(state),
            transfers,
        }
    }
}

impl IdentifyRuntime for ModeRuntime<'_> {
    type Recording = FakeRecording;
    type Experiment = FakeExperiment;

    fn start_recording(
        &self,
        loop_definition: &LoopDefinition,
        plan: &RecordingStartPlan,
        persistence: &dyn RecordingOwnerPersistence,
    ) -> Result<FakeRecording, RecordingStartError<FakeRecording>> {
        self.transfers
            .record(format!("recording.start {}", plan.recording_id));
        self.inner
            .start_recording(loop_definition, plan, persistence)
    }

    fn recover_recording(
        &self,
        loop_definition: &LoopDefinition,
        owner: &RecordingRecoveryOwner,
    ) -> Result<FakeRecording, Failure> {
        self.inner.recover_recording(loop_definition, owner)
    }

    fn experiment(&self) -> FakeExperiment {
        self.inner.experiment()
    }

    fn start_experiment(&self, plan: &ControlExperimentPlan) -> StartRead {
        self.inner.start_experiment(plan)
    }

    fn experiment_status(&self, experiment_id: &str) -> StatusRead {
        self.inner.experiment_status(experiment_id)
    }

    fn transfer_connection(&self) -> Result<RuntimeConnection, Failure> {
        Ok(self.transfers.connection())
    }
}

/// The loop's live sample (`ports.sample()`): the tank's sample, in the mode that the transfer fake's mode signal holds now.
pub(crate) struct ModeSampler<'f> {
    transfers: &'f TransferFake,
}

impl LoopSampler for ModeSampler<'_> {
    fn sample(&mut self) -> Result<Option<ExperimentSample>, Failure> {
        let mut sample = live_sample();
        sample.controller_mode = if self.transfers.auto() {
            ControllerMode::Auto
        } else {
            ControllerMode::Manual
        };
        Ok(Some(sample))
    }
}

/// The study of a run with transfers: it keeps each stopped recording as `identify::test_support::Study` does, and logs the
/// keep.
pub(crate) struct LoggedStudy<'f> {
    inner: Study,
    transfers: &'f TransferFake,
}

impl<'f> LoggedStudy<'f> {
    /// The study of the experiment fake `state`, logging into `transfers`.
    pub(crate) fn new(state: &Arc<FakeState>, transfers: &'f TransferFake) -> Self {
        Self {
            inner: Study::new(state),
            transfers,
        }
    }
}

impl RecordingRetention for LoggedStudy<'_> {
    fn retain(&mut self, run: &RecordedRun, evidence: &RecordingEvidence) -> Result<(), Failure> {
        self.transfers
            .record(format!("retain {}", run.recording_id));
        self.inner.retain(run, evidence)
    }
}

/// The operation of a run with transfers: its progress reaches the transfer fake's log, and it is cancelled from the start when
/// the test says so (the engineer's Cancel).
pub(crate) struct LoggedReport<'f> {
    transfers: &'f TransferFake,
    cancelled: bool,
}

impl<'f> LoggedReport<'f> {
    /// The operation logging into `transfers`, cancelled when `cancelled` says so.
    pub(crate) fn new(transfers: &'f TransferFake, cancelled: bool) -> Self {
        Self {
            transfers,
            cancelled,
        }
    }
}

impl CampaignReport for LoggedReport<'_> {
    fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    fn progress(&self, text: &str) {
        self.transfers.record(format!("progress {text}"));
    }
}

/// Runs one connected identification of `loop_definition` (`ConnectedIdentification.run`) on the experiment fake `state` and
/// the transfer fake `transfers`, journaled in `journal`, with the loop's sample in the mode the transfer fake holds. The
/// operation is cancelled from the start when `cancelled` says so.
pub(crate) fn run_with_transfers(
    state: &Arc<FakeState>,
    transfers: &TransferFake,
    journal: &Mutex<Journal>,
    loop_definition: &LoopDefinition,
    cancelled: bool,
) -> Result<IdentifyOutcome<Verdict>, Failure> {
    let runtime = ModeRuntime::new(state, transfers);
    let clock = FakeClock::new(state);
    let report = LoggedReport::new(transfers, cancelled);
    let mut sampler = ModeSampler { transfers };
    let mut study = LoggedStudy::new(state, transfers);
    let request = IdentifyRequest {
        loop_definition,
        expected_settling_seconds: None,
        evaluate: &passes,
    };
    run(
        RunPorts {
            runtime: &runtime,
            sampler: &mut sampler,
            retention: &mut study,
            clock: &clock,
            report: &report,
            journal,
        },
        &request,
        RunPurpose::Identify,
    )
}
