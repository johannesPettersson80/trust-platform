//! Control Studio's isolated test runs through the engine's client on a real runtime (stage V3, layer 2a; runtime protocol
//! annex E8.4 and E8.3; specification 35, A2.6, A4.6 and B5.3).
//!
//! The tests copy lesson 30's heater project, `examples/control/06-pid-tuning-methods/30-model-based-pi-tuning` (`EXAMPLE`
//! and `LOOP_ID` are the only places that name it), and run `trust-runtime run --simulation` on the copy at ten times real
//! time, the speed `control_studio_tune.rs` sets. Each test starts its own runtime, because a runtime runs one isolated
//! worker at a time (E8.5) and a finished job must not still hold it when the next test starts.
//!
//! The first test completes a five-second run of 51 grid points at a 100 ms step, observing the loop's measured output
//! and plant input. It checks that the run is completed and isolated, that its samples lie on the requested grid, that its
//! observations are present, and that the live controller's gains read the same before and after the run: the isolated
//! run is a separate process and writes nothing to the target. The second test cancels a ten-minute run right after its
//! start and checks that the job ends `cancelled` and that the prefix it may keep never validates (E8.4).
//!
//! Run time at ten times real time: the isolated runs do not follow the live speed, so each test takes the runtime's
//! start-up plus the run. The time has not been measured yet; the builder run reports it.
//!
//! # Running
//!
//! `cargo test -p trust-control-studio --test control_studio_isolated_run`. The tests run the `trust-runtime` executable of
//! their own target folder, so build it first with `cargo build -p trust-runtime --bin trust-runtime`, or name another one
//! in `TRUST_RUNTIME_BIN`. The file is Unix only, as `control_studio_tune.rs` is: the runtime's endpoint is a Unix socket.
#![cfg(unix)]
// The sample times are compared exactly with the requested grid, as the runtime writes them (E8.3).
#![allow(clippy::float_cmp)]

mod common;

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::json;
use trust_control::identification::connected_record::{RunEvidence, ValidationChannel};
use trust_control::loop_definition::LoopDefinition;
use trust_control::recording::EvidenceSource;
use trust_control::runtime::RuntimeConnection;
use trust_control::signals::{loop_inspection_bindings, InspectionSample, SignalBinding};
use trust_control::validation::isolated_run_protocol::{
    ready_project_revision, IsolatedExecution, IsolatedJobState, IsolatedRunEvidence,
    IsolatedRunObservation, IsolatedRunRequest, IsolatedRunStatus,
};
use trust_control_studio::loops;
use trust_control_studio::runtime_client::{RuntimeClient, REQUEST_TIMEOUT};
use trust_runtime_client::{ControlClient, ControlEndpoint};

use common::{
    copy_dir, inspect_until_available, prepare_runtime_toml, run_trust_runtime, start_runtime,
    wait_for_status, RemoveDir, Runtime, DEADLINE, TOKEN,
};

/// The heater project the tests copy (see the module documentation).
const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control/06-pid-tuning-methods/30-model-based-pi-tuning"
);
/// The identifier of the example's loop (`control-loop.json`).
const LOOP_ID: &str = "control-model-based-tuning";
/// The runtime the example's loop is bound to.
const RUNTIME_ID: &str = "simulator";
/// The speed of the live runtime, ten times real time, as `control_studio_tune.rs` sets it.
const TIME_SCALE: u32 = 10;
/// The scan interval of every run here, in ms: the loop's task period (`control-loop.json`, `taskPeriodMs`).
const STEP_MS: u64 = 100;
/// The measured output the runs observe (`P1.TemperatureC`, `control-loop.json`): its symbol and unit.
const TEMPERATURE: (&str, &str) = ("P1.TemperatureC", "degC");
/// The plant input the runs observe (`P1.AppliedPowerPct`): its symbol and unit.
const POWER: (&str, &str) = ("P1.AppliedPowerPct", "%");
/// The live controller's gains, which an isolated run must leave alone (`control-loop.json`, `parameterBinding`).
const GAINS: [&str; 2] = [
    "P1.Parameters.ProportionalGain",
    "P1.Parameters.IntegralGain",
];
/// How often a test reads a run's status.
const POLL: Duration = Duration::from_millis(25);

/// A folder of one test with a copy of the example in it, removed when the test ends, even when it fails.
struct Scratch {
    project: PathBuf,
    _remove: RemoveDir,
}

impl Scratch {
    fn new(name: &str) -> Self {
        let folder =
            std::env::temp_dir().join(format!("trust-cs-isolated-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&folder);
        let remove = RemoveDir(folder.clone());
        let project = folder.join("project");
        copy_dir(Path::new(EXAMPLE), &project);
        Self {
            project,
            _remove: remove,
        }
    }
}

/// A runtime on a copy of lesson 30, started and past its first scan. Dropping it ends the runtime, then removes the copy.
struct Lab {
    _runtime: Runtime,
    _scratch: Scratch,
    connection: RuntimeConnection,
    loop_definition: LoopDefinition,
    program_revision: String,
}

impl Lab {
    /// Copies lesson 30, sets its speed, builds it, starts the runtime and waits until the first scan makes the target a
    /// simulated plant (E1.3), which is when an isolated run may start (E8.1).
    fn start(name: &str) -> Self {
        let scratch = Scratch::new(name);
        let socket = scratch.project.join(".control.sock");
        assert!(
            socket.as_os_str().len() <= 100,
            "the socket path {} is too long; use a shorter TMPDIR",
            socket.display()
        );
        prepare_runtime_toml(&scratch.project, &socket);
        set_time_scale(&scratch.project, TIME_SCALE);
        run_trust_runtime(&["build", "--project", &scratch.project.display().to_string()]);
        let loop_definition = loops::open(&scratch.project, RUNTIME_ID, LOOP_ID)
            .expect("Control Studio opens the example's loop");

        let log = scratch.project.join("runtime.log");
        let runtime = start_runtime(&scratch.project, &log);
        let connection = RuntimeConnection {
            endpoint: format!("unix://{}", socket.display()),
            token: Some(TOKEN.into()),
        };
        let first = wait_for_status(&connection, &log);
        assert_eq!(
            first.simulation_time_scale, TIME_SCALE,
            "the runtime runs at the speed of simulation.toml"
        );
        let mut client = RuntimeClient::connect(&connection).expect("connected");
        let bindings = loop_inspection_bindings(&loop_definition);
        inspect_until_available(
            &mut client,
            &loop_definition,
            &bindings,
            &first.program_revision,
        );
        assert_eq!(
            client.status().expect("status").source(),
            EvidenceSource::SimulatedPlant,
            "a simulated runtime with simulated I/O is a simulated plant"
        );
        Self {
            _runtime: runtime,
            _scratch: scratch,
            connection,
            loop_definition,
            program_revision: first.program_revision,
        }
    }

    /// The live controller's gains, read through the runtime's own inspection (E3).
    fn gains(&self) -> InspectionSample {
        let bindings: Vec<SignalBinding> = [("kp", GAINS[0]), ("ki", GAINS[1])]
            .into_iter()
            .map(|(id, symbol)| SignalBinding {
                id: id.into(),
                address: None,
                symbol: Some(symbol.into()),
                iec_type: "LREAL".into(),
                unit: None,
            })
            .collect();
        let mut client = RuntimeClient::connect(&self.connection).expect("connected");
        inspect_until_available(
            &mut client,
            &self.loop_definition,
            &bindings,
            &self.program_revision,
        )
    }
}

/// Sets the copy's speed: its `simulation.toml` has one `time_scale` line.
fn set_time_scale(project: &Path, scale: u32) {
    let path = project.join("simulation.toml");
    let text = fs::read_to_string(&path).expect("simulation.toml");
    let mut replaced = 0_u32;
    let lines: Vec<String> = text
        .lines()
        .map(|line| {
            if line.split('=').next().map(str::trim) == Some("time_scale") {
                replaced += 1;
                format!("time_scale = {scale}")
            } else {
                line.to_string()
            }
        })
        .collect();
    assert_eq!(replaced, 1, "simulation.toml has one time_scale line");
    fs::write(&path, format!("{}\n", lines.join("\n"))).expect("write simulation.toml");
}

/// The project revision an isolated run starts on, read from the runtime's readiness answer (E8.2). The engine's client has
/// no readiness call, so the test reads the answer through the control client, as the runtime's own search test does.
fn isolated_run_revision(connection: &RuntimeConnection) -> String {
    let endpoint = ControlEndpoint::parse(&connection.endpoint).expect("a control endpoint");
    let mut client =
        ControlClient::connect(&endpoint, connection.token.as_deref(), REQUEST_TIMEOUT)
            .expect("connected");
    let answer = client
        .request("control.simulation.run.status", Some(json!({})))
        .expect("the readiness answer");
    ready_project_revision(answer).expect("the isolated run is ready")
}

/// A run of `duration_ms` at `STEP_MS` that observes the loop's measured output and plant input, with no overrides (E8.4).
fn run_request(
    revision: &str,
    request_id: &str,
    duration_ms: u64,
    wall_budget_ms: u64,
) -> IsolatedRunRequest {
    IsolatedRunRequest {
        request_id: request_id.into(),
        expected_project_revision: revision.into(),
        execution: IsolatedExecution::IsolatedSimulation,
        step_ms: STEP_MS,
        duration_ms,
        seed: 7,
        max_total_cycles: duration_ms / STEP_MS + 1,
        wall_budget_ms,
        record_every: 1,
        initial_values: BTreeMap::new(),
        events: Vec::new(),
        observations: vec![
            IsolatedRunObservation {
                symbol: TEMPERATURE.0.into(),
                unit: TEMPERATURE.1.into(),
            },
            IsolatedRunObservation {
                symbol: POWER.0.into(),
                unit: POWER.1.into(),
            },
        ],
        parameter_overrides: Vec::new(),
        simulation_override: None,
        feedback_relay: None,
    }
}

/// Polls the thin status of `request` until its job has ended, within `limit`.
fn await_end(
    client: &mut RuntimeClient,
    request: &IsolatedRunRequest,
    limit: Duration,
) -> IsolatedRunStatus {
    let deadline = Instant::now() + limit;
    loop {
        let status = client
            .isolated_run_status(request, Some(false))
            .expect("the status of the run");
        if status.status.is_terminal() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "the run has not ended within {} s: {status:?}",
            limit.as_secs()
        );
        sleep(POLL);
    }
}

#[test]
fn a_five_second_run_completes_on_the_grid_and_leaves_the_live_gains_alone() {
    let lab = Lab::start("completes");
    let revision = isolated_run_revision(&lab.connection);
    let gains_before = lab.gains();
    let mut client = RuntimeClient::connect(&lab.connection).expect("connected");
    let request = run_request(&revision, "lesson-30-run-1", 5_000, 60_000);
    let accepted = client.start_isolated_run(&request).expect("the run starts");
    assert_eq!(accepted.job_id, request.request_id);
    let ended = await_end(&mut client, &request, DEADLINE);
    assert_eq!(ended.status, IsolatedJobState::Completed, "{ended:?}");

    let result = client
        .isolated_run_status(&request, Some(true))
        .expect("the completed run");
    assert_eq!(result.status, IsolatedJobState::Completed);
    assert_eq!(
        result.io_provenance,
        "isolated-simulation-no-physical-drivers"
    );
    assert_eq!(result.captured_sample_count, 51);
    let IsolatedRunEvidence::Complete(run) = result.evidence() else {
        panic!("a completed run is complete evidence: {result:?}")
    };
    assert_eq!(run.evidence, RunEvidence::StExecuted);
    let grid: Vec<f64> = (0_u32..=50)
        .map(|index| f64::from(index) * 100.0 / 1000.0)
        .collect();
    assert_eq!(run.conditions.sample_times_seconds, grid);
    let sample_times: Vec<f64> = run.samples.iter().map(|frame| frame.time_seconds).collect();
    assert_eq!(sample_times, grid);
    assert_eq!(
        run.channels,
        vec![
            ValidationChannel {
                id: TEMPERATURE.0.into(),
                unit: TEMPERATURE.1.into(),
            },
            ValidationChannel {
                id: POWER.0.into(),
                unit: POWER.1.into(),
            },
        ]
    );
    assert!(run.samples.iter().all(|frame| {
        frame.values.contains_key(TEMPERATURE.0) && frame.values.contains_key(POWER.0)
    }));
    assert!(result.evidence().validates());

    let gains_after = lab.gains();
    assert_eq!(
        gains_after.values, gains_before.values,
        "an isolated run never writes the live controller"
    );
}

#[test]
fn a_ten_minute_run_cancelled_right_after_its_start_ends_cancelled_and_its_prefix_never_validates()
{
    let lab = Lab::start("cancelled");
    let revision = isolated_run_revision(&lab.connection);
    let mut client = RuntimeClient::connect(&lab.connection).expect("connected");
    let request = run_request(&revision, "lesson-30-run-2", 600_000, 600_000);
    client.start_isolated_run(&request).expect("the run starts");
    let requested = client
        .cancel_isolated_run(&request)
        .expect("the cancel is accepted");
    assert!(requested.cancel_requested, "{requested:?}");

    let ended = await_end(&mut client, &request, DEADLINE);
    assert_eq!(ended.status, IsolatedJobState::Cancelled, "{ended:?}");
    let detail = client
        .isolated_run_status(&request, Some(true))
        .expect("the cancelled run");
    assert!(
        !detail.evidence().validates(),
        "a cancelled prefix never validates: {detail:?}"
    );
    assert!(matches!(
        detail.evidence(),
        IsolatedRunEvidence::Prefix(_) | IsolatedRunEvidence::NoRun
    ));
}
