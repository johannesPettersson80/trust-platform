//! Control Studio runtime protocol matrix: the sixteen D5 combinations, driven directly against trust-runtime.
//!
//! Specification 35, Part D, D5 (last sentence) asks for the runtime protocol to be exercised directly against trust-runtime,
//! in a Rust integration test, for each combination of: loop in Manual or in Auto; addressed or unaddressed controller output;
//! task period equal to the resource cycle or ten times it; and time scale 1 or 10. Each step follows annex E3 to E7 of
//! `docs/specs/control-studio-runtime-protocol.md`. It replaces the extension's journey of the same name, whose steps it
//! follows (removed on 9 October 2026, checklist `rust-logic-migration.md`, V0.10).
#![cfg(unix)]

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use serde_json::{json, Map, Value};

/// The example whose program and plants the matrix replaces. Its Control.PID library stays in place.
const EXAMPLE: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../examples/control-pid-commissioning"
);
/// A known control token for the test runtime. It is not a credential.
const TOKEN: &str = "protocol-matrix-test-token";
const SOCKET_NAME: &str = ".trust-control.sock";
/// The longest unix socket path that the control endpoint accepts in the extension's own tests.
const MAX_SOCKET_PATH_BYTES: usize = 100;
/// The D5 resource cycle: the fast task equals it, and the slow task is ten times it.
const RESOURCE_CYCLE_MS: u64 = 10;
/// The output rate limit of the program's Control.PID, in percent per second (RateMinimum and RateMaximum).
const RATE_LIMIT_PER_S: f64 = 25.0;
/// The Manual step is 10 % of output and the Auto step is 1 degC, each held for 3 s of process time.
const OUTPUT_STEP: f64 = 10.0;
const SETPOINT_STEP: f64 = 1.0;
const EXPERIMENT_MS: u64 = 3_000;
/// The parameter check multiplies the proportional gain by this factor, then rolls it back (E7.3, E7.5).
const PARAMETER_FACTOR: f64 = 1.01;
/// Each sample covers two executions of the loop's task (annex E4.2 and E4.4).
const RECORD_EVERY: u64 = 2;
const RECORDING_SAMPLE_LIMIT: u64 = 100_000;
const RECORDING_PLANNED_MS: u64 = 60_000;
/// How long the runtime may take to confirm a transfer or a parameter set: ten seconds for these tasks (confirmationWindow.ts).
const CONFIRMATION_DEADLINE: Duration = Duration::from_secs(10);
const EXPERIMENT_GRACE: Duration = Duration::from_secs(60);
const RECORDING_GRACE: Duration = Duration::from_secs(60);
const INSPECTION_DEADLINE: Duration = Duration::from_secs(15);
const STATUS_DEADLINE: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const POLL: Duration = Duration::from_millis(25);
/// The noise floor of an Auto entry's tolerance when no noise has been measured (proportionalTransfer.ts).
const AUTO_ENTRY_FLOOR: f64 = 1e-4;

/// Positions of the loop's inspection bindings. Every answer of `control.signals.inspect` keeps the order of its bindings (E3).
/// The process value is at position 1, and nothing reads it by position.
const SETPOINT: usize = 0;
const OUTPUT: usize = 2;
const MODE: usize = 3;
const MANUAL_OUTPUT: usize = 4;

/// The eleven parameters of each Control.PID: field, unit, minimum and maximum. The units and bounds are those of the
/// extension's loop definition (pidBindings.ts).
const PARAMETERS: [(&str, &str, f64, f64); 11] = [
    ("ProportionalGain", "%/degC", -1e6, 1e6),
    ("IntegralGain", "%/(degC*s)", -1e6, 1e6),
    ("DerivativeGain", "%*s/degC", -1e6, 1e6),
    ("ProportionalSetpointWeight", "1", 0.0, 1.0),
    ("DerivativeSetpointWeight", "1", 0.0, 1.0),
    ("Bias", "%", -100.0, 100.0),
    ("OutputMinimum", "%", 0.0, 100.0),
    ("OutputMaximum", "%", 0.0, 100.0),
    ("RateMinimum", "%/s", -1e9, 0.0),
    ("RateMaximum", "%/s", 0.0, 1e9),
    ("TrackingTime", "s", 0.001, 10_000.0),
];

/// The seven retained states of the Control.PID library (`StateValue` in control.st), with their units and bounds
/// (declaredTransferBinding and libraryPidInitialization).
const RETAINED_STATES: [(&str, &str, f64, f64); 7] = [
    ("Setpoint", "degC", 0.0, 100.0),
    ("SetpointRate", "degC/s", -100_000.0, 100_000.0),
    ("ProcessValue", "degC", 0.0, 100.0),
    ("ProcessValueRate", "degC/s", -100_000.0, 100_000.0),
    ("FeedForward", "%", -100.0, 100.0),
    ("NominalOutput", "%", -1000.0, 1100.0),
    ("SaturatedOutput", "%", 0.0, 100.0),
];

/// One of the four D5 loops: its program in the matrix, its task and period, and the addresses of its signals.
struct LoopSpec {
    label: &'static str,
    program: &'static str,
    task: &'static str,
    period_ms: u64,
    setpoint: &'static str,
    manual_output: &'static str,
    process_value: &'static str,
    /// The address of an addressed output. An unaddressed output is read by its symbol, because the program copies it to a command.
    output_address: Option<&'static str>,
}

/// The four loops of D5, one per task period and output kind: FA and FU on the 10 ms task, SA and SU on the 100 ms task.
const LOOPS: [LoopSpec; 4] = [
    LoopSpec {
        label: "Fast addressed",
        program: "FA",
        task: "FastTask",
        period_ms: 10,
        setpoint: "%ML0",
        manual_output: "%ML8",
        process_value: "%IL0",
        output_address: Some("%QL0"),
    },
    LoopSpec {
        label: "Fast unaddressed",
        program: "FU",
        task: "FastTask",
        period_ms: 10,
        setpoint: "%ML16",
        manual_output: "%ML24",
        process_value: "%IL8",
        output_address: None,
    },
    LoopSpec {
        label: "Slow addressed",
        program: "SA",
        task: "SlowTask",
        period_ms: 100,
        setpoint: "%ML32",
        manual_output: "%ML40",
        process_value: "%IL16",
        output_address: Some("%QL16"),
    },
    LoopSpec {
        label: "Slow unaddressed",
        program: "SU",
        task: "SlowTask",
        period_ms: 100,
        setpoint: "%ML48",
        manual_output: "%ML56",
        process_value: "%IL24",
        output_address: None,
    },
];

impl LoopSpec {
    fn symbol(&self, member: &str) -> String {
        format!("{}.{member}", self.program)
    }

    fn parameter(&self, field: &str) -> String {
        format!("{}.Regulator.Parameters.{field}", self.program)
    }

    fn state(&self, field: &str) -> String {
        format!("{}.Regulator.Controller.StateValue.{field}", self.program)
    }

    fn slug(&self) -> String {
        self.label.to_lowercase().replace(' ', "-")
    }

    fn loop_id(&self) -> String {
        format!("loop:{}", self.slug())
    }

    fn output_kind(&self) -> &'static str {
        if self.output_address.is_some() {
            "addressed"
        } else {
            "unaddressed"
        }
    }
}

/// The name of one combination, as every failure message gives it: its mode, output kind, loop, task and time scale.
fn combination(spec: &LoopSpec, manual: bool, time_scale: u32) -> String {
    let mode = if manual { "Manual" } else { "Auto" };
    format!(
        "{mode} mode, {} output ({}), {}, time scale {time_scale}x",
        spec.output_kind(),
        spec.label,
        spec.task
    )
}

fn loop_revision(spec: &LoopSpec) -> String {
    format!("{}-revision-1", spec.loop_id())
}

#[test]
fn every_combination_runs_at_time_scale_1() {
    run_matrix(1);
}

#[test]
fn every_combination_runs_at_time_scale_10() {
    run_matrix(10);
}

/// Runs the eight combinations of one time scale against one runtime, and fails once with every failure named.
fn run_matrix(time_scale: u32) {
    let project = unique_dir(&format!("matrix-{time_scale}x"));
    let _remove_project = RemoveDir(project.clone());
    let socket = control_socket(&project);
    write_matrix_project(&project, &socket, time_scale);
    build_project(&project);
    let mut runtime = start_runtime(&project, time_scale);
    let control = Control { socket };
    let status = wait_for_status(&control, &mut runtime, &project.join("runtime.log"));
    assert_eq!(
        status["simulation_time_scale"],
        json!(time_scale),
        "the runtime does not run at time scale {time_scale}"
    );
    // The runtime refuses a write before its first completed scan, so the matrix waits for one.
    wait_for_scan(&control, &LOOPS[0], None)
        .unwrap_or_else(|error| panic!("the runtime completed no scan: {error}"));

    let mut failures = Vec::new();
    for spec in &LOOPS {
        // Auto is entered first. Each loop starts in Manual, so each one goes Manual -> Auto -> Manual.
        for manual in [false, true] {
            if let Err(error) = exercise(&control, spec, manual, time_scale) {
                failures.push(format!(
                    "{}: {error}",
                    combination(spec, manual, time_scale)
                ));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} combinations failed at time scale {time_scale}x:\n{}",
        failures.len(),
        2 * LOOPS.len(),
        failures.join("\n")
    );
}

/// Runs one combination through the extension's steps: transfer (E6), inspection (E3), recording (E4), experiment (E5) and
/// parameter (E7). The first failed check is the error.
fn exercise(
    control: &Control,
    spec: &LoopSpec,
    manual: bool,
    time_scale: u32,
) -> Result<(), String> {
    // 1. The loop enters the mode under test through a transfer, from whatever mode it holds (E6).
    enter_mode(control, spec, manual)?;
    // 2. Its signals are inspected: a fresh, coherent scan that is good and unforced, in the mode under test (E3).
    let first = wait_for_scan(control, spec, None)?;
    let inspected = wait_for_scan(control, spec, Some(first.scan))?;
    check_scan(&inspected, manual)?;
    // 3. The experiment is planned from that scan (E5.3).
    let step = plan_step(spec, manual, &inspected)?;
    // 4. The loop's task is recorded at two executions per sample while the experiment runs (E4, E5).
    let recording = start_recording(control, spec, manual)?;
    let experiment = start_experiment(control, spec, &step, &inspected)
        .and_then(|()| wait_for_experiment(control, &step, time_scale));
    let settled = match experiment {
        Ok(_) => wait_for_one_more_sample(control, spec, &recording),
        Err(_) => Ok(()),
    };
    // The recording is stopped and released even when the experiment failed, so no buffer is left behind.
    let stopped = stop_recording(control, spec, &recording);
    let status = experiment?;
    settled?;
    let stopped = stopped?;
    // The remaining checks are independent: each one that fails is reported, so one run names every failure.
    let checks = [
        // 5. The experiment ended restored, and the recording holds the step at two executions per sample (E4.4, E5.6, E5.7).
        check_experiment(&status, &step),
        check_recording(&stopped, &step, manual),
        // 6. The loop still reads the mode under test (E3).
        wait_for_scan(control, spec, Some(inspected.scan))
            .and_then(|after| check_scan(&after, manual)),
        // 7. One parameter change is written and rolled back in Manual, and refused in Auto (E7).
        check_parameter(control, spec, manual),
    ];
    let problems: Vec<String> = checks.into_iter().filter_map(Result::err).collect();
    if problems.is_empty() {
        Ok(())
    } else {
        Err(problems.join("; "))
    }
}

/// Puts the loop in the mode under test through one transfer (E6). Nothing is sent when the loop already holds that mode,
/// as the extension's identification-mode client does (connectedIdentificationMode.ts, `enter`).
fn enter_mode(control: &Control, spec: &LoopSpec, manual: bool) -> Result<(), String> {
    let binding = transfer_binding(spec);
    let status = control.request("control.transfer.status", json!({ "binding": binding }))?;
    let auto = status["values"][spec.symbol("AutoMode").as_str()]
        .as_bool()
        .ok_or("the transfer status holds no mode")?;
    if auto != manual {
        return Ok(());
    }

    let states = state_symbols(spec);
    let mut values = Map::new();
    for symbol in &states {
        values.insert(symbol.clone(), json!(number_of(&status, symbol)?));
    }
    // The seven retained states are initialized from their current values, and the Manual output follows the output (A6.4).
    let retained = retained_states(spec);
    let mut initialization = Map::new();
    for symbol in &retained {
        initialization.insert(symbol.clone(), json!(number_of(&status, symbol)?));
    }
    let mut bounded = retained;
    bounded.push(spec.symbol("ManualOutput"));
    let tracking = json!({
        "kind": "declared-state-v1",
        "reference_symbol": spec.symbol("Setpoint"),
        "manual_command_symbol": spec.symbol("ManualOutput"),
        "process_variable": spec.symbol("ProcessValue"),
        "align_output": false,
        "initialization_values": initialization,
        "bounded_state_symbols": bounded,
        "additional_measurements": [spec.symbol("ProcessValue")],
        "additional_references": [],
        "additional_commands": [],
    });
    let tolerance = transfer_tolerance(spec, &status, manual)?;
    let transaction = format!(
        "{}-mode-{}",
        spec.slug(),
        if manual { "manual" } else { "auto" }
    );
    control.request(
        "control.transfer.apply",
        json!({
            "transaction_id": transaction,
            "binding": binding,
            "expected_program_revision": status["program_revision"],
            "expected_binding_revision": status["binding_revision"],
            "expected_values": status["values"],
            "values": values,
            "manual": manual,
            "maximum_output_change": tolerance,
            "tracking": tracking,
            "source": "simulation",
            "physical_approved": false,
        }),
    )?;
    let outcome = wait_for_transfer(control, &binding, &transaction)?;
    if outcome["status"] != json!("applied")
        || outcome["transaction"]["confirmation"] != json!("output-image")
    {
        return Err(format!(
            "the transfer ended {} ({}) without being applied to the output image",
            text(&outcome["status"]),
            text(&outcome["reason"])
        ));
    }
    Ok(())
}

/// The identification-mode transfer binding of one loop (E6.2), declared as the extension's `declaredTransferBinding` declares it:
/// the setpoint and the Manual output as the reference and command states, the seven retained states of Control.PID, the eleven
/// parameters and the process value as guards, and the manipulated value as the output.
fn transfer_binding(spec: &LoopSpec) -> Value {
    let mut states = vec![
        member(spec.symbol("Setpoint"), "degC", 0.0, 100.0),
        member(spec.symbol("ManualOutput"), "%", 0.0, 100.0),
    ];
    states.extend(
        RETAINED_STATES
            .iter()
            .map(|&(field, unit, minimum, maximum)| {
                member(spec.state(field), unit, minimum, maximum)
            }),
    );
    let mut guards: Vec<Value> = PARAMETERS
        .iter()
        .map(|&(field, unit, minimum, maximum)| {
            member(spec.parameter(field), unit, minimum, maximum)
        })
        .collect();
    guards.push(member(spec.symbol("ProcessValue"), "degC", 0.0, 100.0));
    json!({
        "loop_id": format!("{}:identification-mode", spec.loop_id()),
        "task_name": spec.task,
        "mode": { "symbol": spec.symbol("AutoMode"), "manual_value": false },
        "states": states,
        "guards": guards,
        "output": member(spec.symbol("ManipulatedValue"), "%", 0.0, 100.0),
    })
}

/// One LREAL member of a transfer binding (E6.2).
fn member(symbol: String, unit: &str, minimum: f64, maximum: f64) -> Value {
    json!({ "symbol": symbol, "iec_type": "LREAL", "unit": unit, "minimum": minimum, "maximum": maximum })
}

/// The states of the transfer in the order of the binding: the setpoint, the Manual output and the seven retained states.
fn state_symbols(spec: &LoopSpec) -> Vec<String> {
    let mut states = vec![spec.symbol("Setpoint"), spec.symbol("ManualOutput")];
    states.extend(retained_states(spec));
    states
}

fn retained_states(spec: &LoopSpec) -> Vec<String> {
    RETAINED_STATES
        .iter()
        .map(|&(field, _, _, _)| spec.state(field))
        .collect()
}

/// The output-change tolerance of a transfer (E6.9), as the extension computes it: the rate limit over one execution, and for an
/// Auto entry also the entry's own response, the noise floor plus the integral step on the current error.
fn transfer_tolerance(spec: &LoopSpec, status: &Value, manual: bool) -> Result<f64, String> {
    let cycle_seconds = spec.period_ms as f64 / 1000.0;
    let mut tolerance = (RATE_LIMIT_PER_S * cycle_seconds).max(1e-6);
    if !manual {
        let integral_gain = number_of(status, &spec.parameter("IntegralGain"))?;
        let error = number_of(status, &spec.symbol("Setpoint"))?
            - number_of(status, &spec.symbol("ProcessValue"))?;
        let response = AUTO_ENTRY_FLOOR + integral_gain.abs() * cycle_seconds * error.abs();
        if response.is_finite() {
            tolerance = tolerance.max(response);
        }
    }
    Ok(tolerance)
}

/// Polls the transfer status until the transaction is no longer pending (E6.3, E6.7), and returns that status.
fn wait_for_transfer(
    control: &Control,
    binding: &Value,
    transaction: &str,
) -> Result<Value, String> {
    let deadline = Instant::now() + CONFIRMATION_DEADLINE;
    loop {
        let status = control.request("control.transfer.status", json!({ "binding": binding }))?;
        if status["transaction"]["id"] == json!(transaction) && status["status"] != json!("pending")
        {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "transfer {transaction} is still {} after {} s",
                text(&status["status"]),
                CONFIRMATION_DEADLINE.as_secs()
            ));
        }
        sleep(POLL);
    }
}

/// One coherent scan of the loop's inspection bindings, in binding order (E3).
struct Scan {
    scan: u64,
    program_revision: String,
    values: Vec<Option<f64>>,
    qualities: Vec<String>,
    forced: Vec<bool>,
}

impl Scan {
    fn number(&self, position: usize) -> Result<f64, String> {
        self.values[position]
            .ok_or_else(|| format!("the value at binding {position} is not a number"))
    }
}

/// The loop's inspection bindings (E3). The output is read by its address when it has one, and by its symbol when it has none.
/// The mode is read by its symbol.
fn inspection_bindings(spec: &LoopSpec) -> Value {
    let output = match spec.output_address {
        Some(address) => {
            json!({ "id": "manipulatedVariable", "iec_type": "LREAL", "address": address })
        }
        None => {
            json!({ "id": "manipulatedVariable", "iec_type": "LREAL", "symbol": spec.symbol("ManipulatedValue") })
        }
    };
    json!([
        { "id": "setpoint", "iec_type": "LREAL", "address": spec.setpoint },
        { "id": "processVariable", "iec_type": "LREAL", "address": spec.process_value },
        output,
        { "id": "controllerMode", "iec_type": "BOOL", "symbol": spec.symbol("AutoMode") },
        { "id": "manualOutput", "iec_type": "LREAL", "address": spec.manual_output },
    ])
}

/// Polls the inspection until it answers a coherent scan newer than `newer_than` (E3). Polling never adds a watch (E3 rule 1).
fn wait_for_scan(
    control: &Control,
    spec: &LoopSpec,
    newer_than: Option<u64>,
) -> Result<Scan, String> {
    let deadline = Instant::now() + INSPECTION_DEADLINE;
    loop {
        let answer = control.request(
            "control.signals.inspect",
            json!({ "loop_revision": loop_revision(spec), "bindings": inspection_bindings(spec) }),
        )?;
        if answer["available"] == json!(true) {
            let scan = decode_scan(&answer)?;
            if newer_than.is_none_or(|previous| scan.scan > previous) {
                return Ok(scan);
            }
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "no coherent scan newer than {newer_than:?} within {} s",
                INSPECTION_DEADLINE.as_secs()
            ));
        }
        sleep(POLL);
    }
}

fn decode_scan(answer: &Value) -> Result<Scan, String> {
    let list = |key: &str| {
        answer[key]
            .as_array()
            .cloned()
            .ok_or_else(|| format!("the inspection has no {key} list"))
    };
    Ok(Scan {
        scan: answer["scan"]
            .as_u64()
            .ok_or("the inspection names no scan")?,
        program_revision: text(&answer["program_revision"]),
        values: list("values")?.iter().map(Value::as_f64).collect(),
        qualities: list("quality")?.iter().map(text).collect(),
        forced: list("forced")?
            .iter()
            .map(|flag| flag.as_bool().unwrap_or(true))
            .collect(),
    })
}

/// The checks of one scan (step 2): the mode reads as the mode under test, and every signal is good and unforced.
fn check_scan(scan: &Scan, manual: bool) -> Result<(), String> {
    let wanted = if manual { "Manual" } else { "Auto" };
    let reads = if scan.number(MODE)? == 1.0 {
        "Auto"
    } else {
        "Manual"
    };
    if reads != wanted {
        return Err(format!(
            "the loop reads {reads} after the transfer to {wanted}"
        ));
    }
    if scan.qualities.iter().any(|quality| quality != "good")
        || scan.forced.iter().any(|&forced| forced)
    {
        return Err(format!(
            "not every signal is good and unforced: qualities {}, forced {:?}",
            scan.qualities.join(", "),
            scan.forced
        ));
    }
    Ok(())
}

/// The step of one experiment: its kind, the address it writes, the value it steps from and to, and its identifier.
struct Step {
    id: String,
    kind: &'static str,
    address: &'static str,
    from: f64,
    to: f64,
}

/// Plans the step from the loop's current scan (E5.2, E5.3): an output step in Manual, a setpoint step in Auto. The restoration
/// value is the target's current value, which E5.3 requires it to equal.
fn plan_step(spec: &LoopSpec, manual: bool, scan: &Scan) -> Result<Step, String> {
    let (kind, address, amplitude, from) = if manual {
        (
            "mv_step",
            spec.manual_output,
            OUTPUT_STEP,
            scan.number(MANUAL_OUTPUT)?,
        )
    } else {
        (
            "setpoint_step",
            spec.setpoint,
            SETPOINT_STEP,
            scan.number(SETPOINT)?,
        )
    };
    let to = from + amplitude;
    if !(0.0..=100.0).contains(&to) {
        return Err(format!(
            "a step from {from} to {to} leaves the 0 to 100 limits"
        ));
    }
    Ok(Step {
        id: format!("experiment-{}-{kind}-{}", spec.slug(), scan.scan),
        kind,
        address,
        from,
        to,
    })
}

/// Starts the experiment (E5.3): one step, held for the experiment's duration, with a one-point trajectory.
fn start_experiment(
    control: &Control,
    spec: &LoopSpec,
    step: &Step,
    scan: &Scan,
) -> Result<(), String> {
    control
        .request(
            "control.experiment.start",
            json!({
                "experiment_id": step.id,
                "source": "simulation",
                "kind": step.kind,
                "task": spec.task,
                "address": step.address,
                "value": step.to,
                "restoration_value": step.from,
                "value_type": "LREAL",
                "minimum": 0,
                "maximum": 100,
                "duration_ms": EXPERIMENT_MS,
                "expected_scan": scan.scan,
                "mode_binding": { "symbol": spec.symbol("AutoMode"), "manual_value": false },
                "trajectory": [{ "at_ms": 0, "value": step.to }],
            }),
        )
        .map(|_| ())
}

/// Follows the experiment until it is settled: its status is no longer running and its restoration is no longer pending (E5.6).
/// An experiment that outlasts its duration is aborted (E5.5), and the wait fails.
fn wait_for_experiment(control: &Control, step: &Step, time_scale: u32) -> Result<Value, String> {
    let duration = Duration::from_millis(EXPERIMENT_MS / u64::from(time_scale));
    let deadline = Instant::now() + EXPERIMENT_GRACE + duration;
    loop {
        let status = control.request(
            "control.experiment.status",
            json!({ "experiment_id": step.id }),
        )?;
        if status["status"] != json!("running") && status["restoration"] != json!("pending") {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            let _ = control.request(
                "control.experiment.abort",
                json!({ "experiment_id": step.id }),
            );
            return Err(format!(
                "the experiment is {} with restoration {} after its duration",
                text(&status["status"]),
                text(&status["restoration"])
            ));
        }
        sleep(POLL);
    }
}

/// The experiment ended restored and its last applied value is the step (E5.6, E5.7).
fn check_experiment(status: &Value, step: &Step) -> Result<(), String> {
    if status["status"] != json!("completed")
        || status["stop_reason"] != json!("duration_complete")
        || status["restoration"] != json!("restored")
    {
        return Err(format!(
            "the experiment ended {} ({}) with restoration {}",
            text(&status["status"]),
            text(&status["stop_reason"]),
            text(&status["restoration"])
        ));
    }
    if status["last_applied"]["value"].as_f64() != Some(step.to) {
        return Err(format!(
            "the step {} was not read back after an execution: last_applied is {}",
            step.to, status["last_applied"]
        ));
    }
    Ok(())
}

/// The recording's identifier: one per loop and mode. Each recording is released before the next one starts (E4.2).
fn recording_id(spec: &LoopSpec, manual: bool) -> String {
    format!(
        "recording-{}-{}",
        spec.slug(),
        if manual { "manual" } else { "auto" }
    )
}

/// Starts the recording of the loop's task (E4.2). The start follows a current coherent inspection of the same bindings, and a
/// start that races a scan is repeated on the next one.
fn start_recording(control: &Control, spec: &LoopSpec, manual: bool) -> Result<String, String> {
    let id = recording_id(spec, manual);
    let deadline = Instant::now() + INSPECTION_DEADLINE;
    loop {
        let scan = wait_for_scan(control, spec, None)?;
        let started = control.request(
            "control.recording.start",
            json!({
                "recording_id": id,
                "loop_revision": loop_revision(spec),
                "program_revision": scan.program_revision,
                "bindings": inspection_bindings(spec),
                "maximum_samples": RECORDING_SAMPLE_LIMIT,
                "maximum_duration_ms": RECORDING_PLANNED_MS,
                "task": spec.task,
                "record_every": RECORD_EVERY,
            }),
        );
        match started {
            Ok(_) => return Ok(id),
            Err(error)
                if error.contains("current coherent inspection") && Instant::now() < deadline =>
            {
                sleep(POLL)
            }
            Err(error) => return Err(error),
        }
    }
}

fn sample_count(control: &Control, spec: &LoopSpec, id: &str) -> Result<u64, String> {
    let page = control.request(
        "control.recording.status",
        json!({ "recording_id": id, "loop_revision": loop_revision(spec) }),
    )?;
    page["sample_count"]
        .as_u64()
        .ok_or_else(|| "the recording status holds no sample count".to_string())
}

/// Follows the recording until one more sample closes after the restoration. The runtime confirms the restoration in the
/// execution that restores the loop, so the sample that closes there can still hold the step.
fn wait_for_one_more_sample(control: &Control, spec: &LoopSpec, id: &str) -> Result<(), String> {
    let closed = sample_count(control, spec, id)?;
    let deadline = Instant::now() + RECORDING_GRACE;
    while sample_count(control, spec, id)? <= closed {
        if Instant::now() >= deadline {
            return Err("the recording closed no sample after the restoration".to_string());
        }
        sleep(POLL);
    }
    Ok(())
}

/// Stops the recording and releases it (E4.7). The stop answer carries the samples.
fn stop_recording(control: &Control, spec: &LoopSpec, id: &str) -> Result<Value, String> {
    let identity = json!({ "recording_id": id, "loop_revision": loop_revision(spec) });
    let stopped = control.request("control.recording.stop", identity.clone())?;
    control.request("control.recording.release", identity)?;
    Ok(stopped)
}

/// One recorded sample: its sequence number, which is the last of its executions (E4.4), and its values in binding order.
struct Recorded {
    scan: u64,
    values: Vec<Option<f64>>,
}

fn decode_recording(stopped: &Value) -> Result<Vec<Recorded>, String> {
    stopped["samples"]
        .as_array()
        .ok_or("the stop answer lists no samples")?
        .iter()
        .map(decode_sample)
        .collect()
}

fn decode_sample(sample: &Value) -> Result<Recorded, String> {
    Ok(Recorded {
        scan: sample["scan"]
            .as_u64()
            .ok_or("a sample names no sequence number")?,
        values: sample["values"]
            .as_array()
            .ok_or("a sample holds no values")?
            .iter()
            .map(Value::as_f64)
            .collect(),
    })
}

/// The stopped recording holds the step: it is complete, each sample covers two executions with no gap, and the step is in it
/// (E4.4, E4.5).
fn check_recording(stopped: &Value, step: &Step, manual: bool) -> Result<(), String> {
    if stopped["acquisition"]["completeness"] != json!("complete") {
        return Err(format!(
            "the recording is incomplete: {}",
            text(&stopped["acquisition"]["reason"])
        ));
    }
    let samples = decode_recording(stopped)?;
    if samples.len() < 3 {
        return Err(format!(
            "the recording holds {} samples, not at least 3",
            samples.len()
        ));
    }
    // Each sample covers two executions, so the sequence advances by two. Only the last sample may cover fewer.
    let steps: Vec<u64> = samples
        .windows(2)
        .map(|pair| pair[1].scan.saturating_sub(pair[0].scan))
        .collect();
    let (last, body) = steps.split_last().ok_or("the recording has no steps")?;
    if !body.iter().all(|&advance| advance == RECORD_EVERY) || !(1..=RECORD_EVERY).contains(last) {
        return Err(format!(
            "consecutive samples advance by {steps:?} executions, not by a record period of {RECORD_EVERY}"
        ));
    }
    let near = |value: Option<f64>, expected: f64| {
        value.is_some_and(|value| (value - expected).abs() < 1e-9)
    };
    if manual {
        if !samples
            .iter()
            .any(|sample| near(sample.values[OUTPUT], step.to))
        {
            return Err(format!(
                "the plant input never reached the step {}",
                step.to
            ));
        }
    } else {
        if !samples
            .iter()
            .any(|sample| near(sample.values[SETPOINT], step.to))
        {
            return Err(format!("the setpoint never reached the step {}", step.to));
        }
        let final_setpoint = samples[samples.len() - 1].values[SETPOINT];
        if !near(final_setpoint, step.from) {
            return Err(format!("the setpoint was not restored to {}", step.from));
        }
    }
    Ok(())
}

fn parameter_binding(spec: &LoopSpec) -> Value {
    let parameters: Vec<Value> = PARAMETERS
        .iter()
        .map(|&(field, unit, minimum, maximum)| {
            json!({
                "symbol": spec.parameter(field),
                "iec_type": "LREAL",
                "unit": unit,
                "convention": format!("Control.PID {field}"),
                "minimum": minimum,
                "maximum": maximum,
            })
        })
        .collect();
    json!({
        "loop_id": spec.loop_id(),
        "task_name": spec.task,
        "parameters": parameters,
        "mode": { "symbol": spec.symbol("AutoMode"), "manual_value": false },
    })
}

/// Writes the proportional gain and rolls it back in Manual (E7.3, E7.5). In Auto the write is refused (E7.8) and the value stays.
fn check_parameter(control: &Control, spec: &LoopSpec, manual: bool) -> Result<(), String> {
    let binding = parameter_binding(spec);
    let symbol = spec.parameter("ProportionalGain");
    let before = control.request("control.parameters.status", json!({ "binding": binding }))?;
    let original = number_of(&before, &symbol)?;
    let changed = original * PARAMETER_FACTOR;
    let mut candidate = before["values"].clone();
    candidate[symbol.as_str()] = json!(changed);
    let transaction = format!(
        "{}-parameter-apply-{}",
        spec.slug(),
        if manual { "manual" } else { "auto" }
    );
    let apply = json!({
        "transaction_id": transaction,
        "binding": binding,
        "expected_program_revision": before["program_revision"],
        "expected_binding_revision": before["binding_revision"],
        "expected_active_revision": before["active_revision"],
        "expected_values": before["values"],
        "values": candidate,
        "source": "simulation",
        "physical_approved": false,
    });
    if !manual {
        match control.request("control.parameters.apply", apply) {
            Ok(_) => {
                return Err(format!(
                    "{symbol} was written in Auto, but apply needs Manual (E7.3)"
                ))
            }
            Err(error) if error.ends_with("(invalid_parameter_request)") => {}
            Err(error) => {
                return Err(format!(
                    "the Auto write was refused with the wrong code: {error}"
                ))
            }
        }
        let after = control.request("control.parameters.status", json!({ "binding": binding }))?;
        return if number_of(&after, &symbol)? == original {
            Ok(())
        } else {
            Err(format!("{symbol} changed although its write was refused"))
        };
    }

    control.request("control.parameters.apply", apply)?;
    let applied = wait_for_parameters(control, &binding, &transaction)?;
    let written = number_of(&applied, &symbol)?;
    if written != changed {
        return Err(format!(
            "{symbol} was confirmed at {written}, not {changed}"
        ));
    }
    let rollback = format!("{}-parameter-rollback", spec.slug());
    let current = control.request("control.parameters.status", json!({ "binding": binding }))?;
    control.request(
        "control.parameters.rollback",
        json!({
            "transaction_id": rollback,
            "binding": binding,
            "expected_program_revision": current["program_revision"],
            "expected_binding_revision": current["binding_revision"],
            "expected_active_revision": current["active_revision"],
            "expected_values": current["values"],
            "previous_revision": current["previous_revision"],
            "source": "simulation",
            "physical_approved": false,
        }),
    )?;
    let restored = wait_for_parameters(control, &binding, &rollback)?;
    if number_of(&restored, &symbol)? != original {
        return Err(format!(
            "the rollback did not restore {symbol} to {original}"
        ));
    }
    Ok(())
}

/// Polls the parameter status until the transaction is applied or rejected (E7.2, E7.4), and returns that status.
fn wait_for_parameters(
    control: &Control,
    binding: &Value,
    transaction: &str,
) -> Result<Value, String> {
    let deadline = Instant::now() + CONFIRMATION_DEADLINE;
    loop {
        let status = control.request("control.parameters.status", json!({ "binding": binding }))?;
        if status["transaction_id"] == json!(transaction) {
            if status["status"] == json!("applied") {
                return Ok(status);
            }
            if status["status"] == json!("rejected") {
                return Err(format!(
                    "transaction {transaction} was rejected: {}",
                    text(&status["reason"])
                ));
            }
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "transaction {transaction} is still {} after {} s",
                text(&status["status"]),
                CONFIRMATION_DEADLINE.as_secs()
            ));
        }
        sleep(POLL);
    }
}

/// The number that a status holds under `symbol` in its `values` (E3, E6.3, E7.2).
fn number_of(status: &Value, symbol: &str) -> Result<f64, String> {
    status["values"][symbol]
        .as_f64()
        .ok_or_else(|| format!("the status holds no number for {symbol}"))
}

/// The control endpoint of the runtime under test.
struct Control {
    socket: PathBuf,
}

impl Control {
    /// One control request on its own connection: a JSON line out and a JSON line back (E0.1). A refusal is an `Err` that names
    /// the operation and carries the runtime's error code.
    fn request(&self, operation: &str, params: Value) -> Result<Value, String> {
        let mut stream = UnixStream::connect(&self.socket)
            .map_err(|error| format!("connect {}: {error}", self.socket.display()))?;
        stream
            .set_read_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(REQUEST_TIMEOUT))
            .map_err(|error| error.to_string())?;
        let line = json!({ "id": 1, "type": operation, "auth": TOKEN, "params": params });
        writeln!(stream, "{line}").map_err(|error| error.to_string())?;
        let mut answer = String::new();
        BufReader::new(stream)
            .read_line(&mut answer)
            .map_err(|error| error.to_string())?;
        let response: Value = serde_json::from_str(&answer)
            .map_err(|error| format!("{operation}: the answer is not JSON: {error}"))?;
        if response["ok"] == json!(true) {
            Ok(response["result"].clone())
        } else {
            Err(format!(
                "{operation} refused: {} ({})",
                text(&response["error"]),
                text(&response["error_code"])
            ))
        }
    }
}

/// A JSON string as plain text, and any other value as its JSON text.
fn text(value: &Value) -> String {
    value
        .as_str()
        .map_or_else(|| value.to_string(), str::to_string)
}

/// Waits until the runtime answers `status` (E1.3) and returns its answer. A runtime that exits first fails the test.
fn wait_for_status(control: &Control, runtime: &mut Runtime, log: &Path) -> Value {
    let deadline = Instant::now() + STATUS_DEADLINE;
    loop {
        if let Ok(Some(exit)) = runtime.0.try_wait() {
            panic!(
                "trust-runtime exited before it answered status ({exit}); its log:\n{}",
                log_tail(log)
            );
        }
        if let Ok(status) = control.request("status", json!({})) {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "trust-runtime did not answer status within {} s; its log:\n{}",
            STATUS_DEADLINE.as_secs(),
            log_tail(log)
        );
        sleep(Duration::from_millis(200));
    }
}

/// A `trust-runtime run` child. Dropping it kills the process and reaps it, so a failed test leaves no runtime behind.
struct Runtime(Child);

impl Drop for Runtime {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Removes a temporary folder when the test ends, whether it passed or failed.
struct RemoveDir(PathBuf);

impl Drop for RemoveDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Starts `trust-runtime run --simulation --time-scale N` on the project, with its output in runtime.log.
fn start_runtime(project: &Path, time_scale: u32) -> Runtime {
    let log = fs::File::create(project.join("runtime.log")).expect("create runtime.log");
    let stderr = log.try_clone().expect("share runtime.log");
    let child = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("run")
    .arg("--project")
    .arg(project)
    .arg("--simulation")
    .arg("--time-scale")
    .arg(time_scale.to_string())
    .stdin(Stdio::null())
    .stdout(Stdio::from(log))
    .stderr(Stdio::from(stderr))
    .spawn()
    .expect("spawn trust-runtime run");
    Runtime(child)
}

/// The last lines of a runtime log, for a failure message.
fn log_tail(log: &Path) -> String {
    let text = fs::read_to_string(log).unwrap_or_default();
    let lines: Vec<&str> = text.lines().collect();
    lines[lines.len().saturating_sub(40)..].join("\n")
}

/// Builds the project with `trust-runtime build --project`, as the extension does before it starts a runtime.
fn build_project(project: &Path) {
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .arg("build")
    .arg("--project")
    .arg(project)
    .output()
    .expect("run trust-runtime build");
    assert!(
        output.status.success(),
        "trust-runtime build failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Writes the four-loop program, the 10 ms resource cycle and the four plants into a copy of the example, as the extension does.
/// The copy's control endpoint is a unix socket in the copy, and its web server is off.
fn write_matrix_project(project: &Path, socket: &Path, time_scale: u32) {
    copy_dir(Path::new(EXAMPLE), project);
    fs::write(project.join("src").join("Main.st"), MATRIX_PROGRAM)
        .expect("write the matrix program");
    let runtime = fs::read_to_string(project.join("runtime.toml")).expect("read runtime.toml");
    let runtime = set_key(
        &runtime,
        "cycle_interval_ms",
        &RESOURCE_CYCLE_MS.to_string(),
    );
    let runtime = set_control_endpoint(&runtime, socket);
    fs::write(
        project.join("runtime.toml"),
        format!("{runtime}\n[runtime.web]\nenabled = false\n"),
    )
    .expect("write runtime.toml");
    fs::write(
        project.join("simulation.toml"),
        matrix_simulation(time_scale),
    )
    .expect("write simulation.toml");
    // The example's control loop describes the old P1 loop, whose symbols the matrix program no longer has.
    let _ = fs::remove_file(project.join("control-loop.json"));
}

/// The simulation of the matrix: four first-order plants, one per loop, each with a gain of 1.5 degC per percent of output, a 2 s
/// time constant, no noise and the example's actuator limits. The resource cycle is the plants' step.
fn matrix_simulation(time_scale: u32) -> String {
    format!(
        r##"[simulation]
enabled = true
seed = 20260824
time_scale = {time_scale}
plant_step_ms = {RESOURCE_CYCLE_MS}
max_plant_substeps_per_cycle = 100
plant_trace_capacity = 2000

{}
{}
{}
{}"##,
        plant("fast-addressed", "%QL0", "%IL0"),
        plant("fast-unaddressed", "%QL8", "%IL8"),
        plant("slow-addressed", "%QL16", "%IL16"),
        plant("slow-unaddressed", "%QL24", "%IL24"),
    )
}

fn plant(id: &str, command: &str, feedback: &str) -> String {
    format!(
        r##"[[plants]]
id = "{id}"
model = "first_order"
command_sources = ["{command}"]
feedback_targets = ["{feedback}"]
initial_state = [0.0]
initial_input = [0.0]
input_offset = [0.0]
output_offset = [20.0]

[plants.parameters]
gain = 1.5
time_constant_s = 2.0

[plants.actuator]
minimum = 0.0
maximum = 100.0
rate_up_per_s = 25.0
rate_down_per_s = 25.0
"##
    )
}

/// Gives a copy's `runtime.toml` text the matrix's control endpoint, the socket in the copy, and its known token. Both
/// keys are optional and the example names neither (specification 11, section 6.8.0a), so they are set in the
/// `[runtime.control]` table, added there when absent.
fn set_control_endpoint(text: &str, socket: &Path) -> String {
    let mut document = text
        .parse::<toml_edit::DocumentMut>()
        .expect("parse runtime.toml");
    let control = &mut document["runtime"]["control"];
    control["endpoint"] = toml_edit::value(format!("unix://{}", socket.display()));
    control["auth_token"] = toml_edit::value(TOKEN);
    document.to_string()
}

/// Replaces the value of one `key = value` line of a TOML text, and fails when the text has no such line.
fn set_key(text: &str, key: &str, value: &str) -> String {
    let mut replaced = false;
    let mut lines = Vec::new();
    for line in text.lines() {
        let is_key = line
            .trim_start()
            .strip_prefix(key)
            .is_some_and(|rest| rest.trim_start().starts_with('='));
        if is_key {
            lines.push(format!("{key} = {value}"));
            replaced = true;
        } else {
            lines.push(line.to_string());
        }
    }
    assert!(replaced, "runtime.toml has no `{key}` line to replace");
    lines.join("\n")
}

/// Copies a folder tree. The example holds only plain files and folders.
fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create the copy of the example");
    for entry in fs::read_dir(from).expect("read the example folder") {
        let entry = entry.expect("read an example entry");
        let target = to.join(entry.file_name());
        if entry
            .file_type()
            .expect("type of an example entry")
            .is_dir()
        {
            copy_dir(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), &target).expect("copy an example file");
        }
    }
}

/// The runtime's control socket: in the project folder while that path fits a socket address
/// (`MAX_SOCKET_PATH_BYTES`), else beside the folder under the folder's name, as under the release
/// guard's long `TMPDIR`.
fn control_socket(project: &Path) -> PathBuf {
    let socket = project.join(SOCKET_NAME);
    if socket.as_os_str().len() <= MAX_SOCKET_PATH_BYTES {
        return socket;
    }
    let name = project
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let short = project.with_file_name(format!("{name}.sock"));
    assert!(
        short.as_os_str().len() <= MAX_SOCKET_PATH_BYTES,
        "the control socket path is {} bytes; use a shorter TMPDIR (the limit is {MAX_SOCKET_PATH_BYTES})",
        short.as_os_str().len()
    );
    short
}

/// A fresh folder under the temporary directory. The name stays short because a unix socket path has a length limit.
fn unique_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("trust-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create the protocol matrix folder");
    path
}

/// The matrix program: four loops of one Control.PID type, two on the 10 ms task and two on the 100 ms task, each addressed or
/// unaddressed in its output. The text is the extension's `MATRIX_PROGRAM`.
const MATRIX_PROGRAM: &str = include_str!("fixtures/control_studio_protocol_matrix/Main.st");
