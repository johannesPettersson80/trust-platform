use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const REFERENCE_KP: f64 = 0.667;
const REFERENCE_KI: f64 = 0.667;

#[derive(Clone, Copy)]
struct ControllerConfig {
    kp: f64,
    ki: f64,
    kd: f64,
    output_minimum: f64,
    output_maximum: f64,
    rate_minimum: f64,
    rate_maximum: f64,
    tracking_time: f64,
    anti_windup: &'static str,
    filtered: bool,
    cycle_time_from_io: bool,
}

impl Default for ControllerConfig {
    fn default() -> Self {
        Self {
            kp: REFERENCE_KP,
            ki: REFERENCE_KI,
            kd: 0.0,
            output_minimum: -1.0e100,
            output_maximum: 1.0e100,
            rate_minimum: -1.0e100,
            rate_maximum: 1.0e100,
            tracking_time: 1.0,
            anti_windup: "BackCalculation",
            filtered: false,
            cycle_time_from_io: false,
        }
    }
}

#[derive(Clone, Copy)]
struct Sample {
    time: f64,
    setpoint: f64,
    process_value: f64,
    output: f64,
}

#[derive(Debug)]
struct StepMetrics {
    final_error_ratio: f64,
    overshoot_percent: f64,
    settling_time: f64,
    integrated_absolute_error: f64,
    maximum_process_value: f64,
    maximum_output: f64,
}

fn temp_simulation_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-pid-performance-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime
        .io()
        .read(address)
        .expect("read LREAL process image")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LWORD storage for LREAL binding, got {other:?}"),
    }
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image bits");
}

fn controller_source(config: ControllerConfig, cycle_time: f64, setpoint_body: &str) -> String {
    let process_value = if config.filtered {
        "FilteredValue := Filter.Update(Input := ProcessValue, CycleTime := Input.CycleTime);\nInput.ProcessValue := FilteredValue;"
    } else {
        "Input.ProcessValue := ProcessValue;"
    };
    let cycle_assignment = if config.cycle_time_from_io {
        "Input.CycleTime := ActualCycleTime;".to_owned()
    } else {
        format!("Input.CycleTime := LREAL#{cycle_time:.17e};")
    };
    format!(
        r#"
USING Control;

PROGRAM Main
VAR
    Controller : PID;
    Filter : LowPass;
    Parameters : PIDParameters;
    FilterParameters : LowPassParameters;
    State : PIDInitialState;
    Input : PIDInput;
    ProcessValue AT %IL0 : LREAL;
    ActualCycleTime AT %IL8 : LREAL;
    Command AT %QL0 : LREAL;
    FilteredValue : LREAL;
    Scan : DINT;
    Initialized : BOOL;
END_VAR

IF NOT Initialized THEN
    Parameters.ProportionalGain := LREAL#{kp:.17e};
    Parameters.IntegralGain := LREAL#{ki:.17e};
    Parameters.DerivativeGain := LREAL#{kd:.17e};
    Parameters.ProportionalSetpointWeight := LREAL#1.0;
    Parameters.DerivativeSetpointWeight := LREAL#0.0;
    Parameters.OutputMinimum := LREAL#{output_minimum:.17e};
    Parameters.OutputMaximum := LREAL#{output_maximum:.17e};
    Parameters.RateMinimum := LREAL#{rate_minimum:.17e};
    Parameters.RateMaximum := LREAL#{rate_maximum:.17e};
    Parameters.TrackingTime := LREAL#{tracking_time:.17e};
    Parameters.AntiWindup := PIDAntiWindup#{anti_windup};
    Controller.Configure(Parameters := Parameters);
    Controller.Initialize(State := State);
    FilterParameters.TimeConstant := LREAL#0.1;
    FilterParameters.Order := FilterOrder#SecondOrder;
    Filter.Configure(Parameters := FilterParameters);
    Filter.Initialize(Input := LREAL#0.0);
    Initialized := TRUE;
END_IF;

{setpoint_body}
{cycle_assignment}
{process_value}
Input.Mode := PIDMode#Auto;
Command := Controller.Update(Input := Input);
Scan := Scan + 1;
END_PROGRAM
"#,
        kp = config.kp,
        ki = config.ki,
        kd = config.kd,
        output_minimum = config.output_minimum,
        output_maximum = config.output_maximum,
        rate_minimum = config.rate_minimum,
        rate_maximum = config.rate_maximum,
        tracking_time = config.tracking_time,
        anti_windup = config.anti_windup,
    )
}

fn fopdt_configuration(
    step_ms: i64,
    gain: f64,
    time_constant: f64,
    dead_time: f64,
    extra: &str,
) -> String {
    format!(
        r#"
[simulation]
enabled = true
seed = 42
plant_step_ms = {step_ms}

[[plants]]
id = "reference-fopdt"
model = "first_order_dead_time"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = [0.0]
[plants.parameters]
gain = {gain}
time_constant_s = {time_constant}
dead_time_s = {dead_time}
initial_delayed_input = 0.0
output_bias = 0.0
{extra}
"#
    )
}

fn scalar_plant_configuration(
    step_ms: i64,
    id: &str,
    model: &str,
    initial_state: &str,
    parameters: &str,
    extra: &str,
) -> String {
    format!(
        r#"
[simulation]
enabled = true
seed = 42
plant_step_ms = {step_ms}

[[plants]]
id = "{id}"
model = "{model}"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
initial_state = {initial_state}
[plants.parameters]
{parameters}
{extra}
"#
    )
}

fn run_loop(
    name: &str,
    controller: ControllerConfig,
    step_ms: i64,
    cycles: usize,
    setpoint_body: &str,
    setpoint_at: impl Fn(usize) -> f64,
    plant_configuration: String,
) -> Vec<Sample> {
    run_loop_with_intervals(
        name,
        controller,
        &vec![step_ms; cycles],
        setpoint_body,
        setpoint_at,
        plant_configuration,
    )
}

fn run_loop_with_intervals(
    name: &str,
    controller: ControllerConfig,
    intervals_ms: &[i64],
    setpoint_body: &str,
    setpoint_at: impl Fn(usize) -> f64,
    plant_configuration: String,
) -> Vec<Sample> {
    let nominal_cycle_time =
        intervals_ms.iter().sum::<i64>() as f64 / intervals_ms.len() as f64 / 1_000.0;
    let source = controller_source(controller, nominal_cycle_time, setpoint_body);
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .unwrap_or_else(|error| panic!("compile {name} Control.PID fixture: {error}"));

    let path = temp_simulation_path(name);
    std::fs::write(&path, plant_configuration).expect("write plant configuration");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load {name} plant configuration: {error}")),
    );
    let feedback = IoAddress::parse("%IL0").expect("feedback address");
    let actual_cycle_time = IoAddress::parse("%IL8").expect("cycle-time address");
    let command = IoAddress::parse("%QL0").expect("command address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial plant feedback");

    let mut trace = Vec::with_capacity(intervals_ms.len());
    let mut elapsed_ms = 0_i64;
    for (index, interval_ms) in intervals_ms.iter().copied().enumerate() {
        let step = index + 1;
        if controller.cycle_time_from_io {
            write_lreal(
                &mut runtime,
                &actual_cycle_time,
                interval_ms as f64 / 1_000.0,
            );
        }
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("execute {name} cycle {step}: {error}"));
        let output = read_lreal(&runtime, &command);
        elapsed_ms += interval_ms;
        let now = Duration::from_millis(elapsed_ms);
        simulation
            .apply_post_cycle(now, &runtime)
            .unwrap_or_else(|error| panic!("advance {name} plant at cycle {step}: {error}"));
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .unwrap_or_else(|error| panic!("publish {name} plant at cycle {step}: {error}"));
        trace.push(Sample {
            time: elapsed_ms as f64 / 1_000.0,
            setpoint: setpoint_at(index),
            process_value: read_lreal(&runtime, &feedback),
            output,
        });
    }
    let _ = std::fs::remove_file(path);
    trace
}

fn step_metrics(trace: &[Sample], amplitude: f64) -> StepMetrics {
    let band = (0.02 * amplitude.abs()).max(0.02);
    let mut last_outside = None;
    let mut integrated_absolute_error = 0.0;
    let mut peak = f64::NEG_INFINITY;
    let mut maximum_process_value = 0.0_f64;
    let mut maximum_output = 0.0_f64;
    let mut previous_time = 0.0;
    for (index, sample) in trace.iter().enumerate() {
        let cycle_time = sample.time - previous_time;
        previous_time = sample.time;
        let error = (sample.setpoint - sample.process_value).abs();
        integrated_absolute_error += cycle_time * error;
        if error > band {
            last_outside = Some(index);
        }
        peak = peak.max(sample.process_value);
        maximum_process_value = maximum_process_value.max(sample.process_value.abs());
        maximum_output = maximum_output.max(sample.output.abs());
        assert!(
            sample.process_value.is_finite() && sample.output.is_finite(),
            "non-finite closed-loop sample at {} s",
            sample.time
        );
    }
    let final_sample = trace.last().expect("nonempty trace");
    let settling_time = last_outside
        .and_then(|index| trace.get(index + 1))
        .map_or_else(|| trace[0].time, |sample| sample.time);
    StepMetrics {
        final_error_ratio: (final_sample.setpoint - final_sample.process_value).abs()
            / amplitude.abs().max(1e-12),
        overshoot_percent: 100.0 * (peak - amplitude).max(0.0) / amplitude.abs().max(1e-12),
        settling_time,
        integrated_absolute_error,
        maximum_process_value,
        maximum_output,
    }
}

fn assert_stable(trace: &[Sample], bound: f64) {
    for sample in trace {
        assert!(sample.process_value.is_finite() && sample.output.is_finite());
        assert!(
            sample.process_value.abs() <= bound,
            "PV {} exceeded stability bound {bound} at {} s",
            sample.process_value,
            sample.time
        );
    }
}

fn assert_step_gates(
    name: &str,
    trace: &[Sample],
    maximum_overshoot: f64,
    maximum_settling_time: f64,
    maximum_final_error_ratio: f64,
) -> StepMetrics {
    let metrics = step_metrics(trace, 1.0);
    eprintln!("plant benchmark {name}: {metrics:?}");
    assert_stable(trace, 10.0);
    assert!(
        metrics.overshoot_percent <= maximum_overshoot,
        "{name}: {metrics:?}"
    );
    assert!(
        metrics.settling_time <= maximum_settling_time,
        "{name}: {metrics:?}"
    );
    assert!(
        metrics.final_error_ratio <= maximum_final_error_ratio,
        "{name}: {metrics:?}"
    );
    metrics
}

#[test]
fn pid_advanced_benchmark_stabilizes_an_open_loop_unstable_plant() {
    let trace = run_loop(
        "open-loop-unstable",
        ControllerConfig {
            kp: 3.0,
            ki: 1.0,
            ..ControllerConfig::default()
        },
        50,
        600,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        scalar_plant_configuration(
            50,
            "open-loop-unstable",
            "state_space",
            "[0.0]",
            "a = [[0.2]]\nb = [[1.0]]\nc = [[1.0]]\nd = [[0.0]]",
            "",
        ),
    );
    assert_step_gates("open-loop unstable", &trace, 20.0, 15.0, 0.02);
}

#[test]
fn pid_advanced_benchmark_recovers_from_a_scheduled_gain_change() {
    let trace = run_loop(
        "scheduled-gain-change",
        ControllerConfig::default(),
        100,
        450,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        fopdt_configuration(
            100,
            0.6,
            1.0,
            0.5,
            r#"

[[plant_events]]
at_ms = 15000
plant = "reference-fopdt"
kind = "set_parameter"
parameter = "gain"
value = 1.4
"#,
        ),
    );
    assert_stable(&trace, 2.0);
    let post_change = trace
        .iter()
        .filter(|sample| sample.time >= 15.0)
        .collect::<Vec<_>>();
    let post_change_iae = post_change
        .iter()
        .map(|sample| 0.1 * (1.0 - sample.process_value).abs())
        .sum::<f64>();
    let last_outside = post_change
        .iter()
        .rposition(|sample| (1.0 - sample.process_value).abs() > 0.02)
        .and_then(|index| post_change.get(index + 1))
        .map_or(15.0, |sample| sample.time);
    let recovery_time = last_outside - 15.0;
    let final_error = (1.0 - trace.last().unwrap().process_value).abs();
    eprintln!(
        "advanced scheduled gain: recovery={recovery_time:.3} s, post_change_iae={post_change_iae:.8}, final_error={final_error:.8}"
    );
    assert!(recovery_time <= 15.0);
    assert!(post_change_iae <= 2.0);
    assert!(final_error <= 0.02);
}

#[test]
fn pid_advanced_benchmark_uses_actual_cycle_time_under_severe_jitter() {
    let intervals = [20_i64, 180, 50, 150, 100].repeat(60);
    let trace = run_loop_with_intervals(
        "severe-scan-jitter",
        ControllerConfig {
            cycle_time_from_io: true,
            ..ControllerConfig::default()
        },
        &intervals,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        fopdt_configuration(50, 1.0, 1.0, 0.5, ""),
    );
    assert_eq!(trace.last().unwrap().time, 30.0);
    assert_step_gates("severe deterministic jitter", &trace, 20.0, 8.0, 0.02);
}

#[test]
fn pid_benchmark_tracks_an_integrating_process() {
    let trace = run_loop(
        "integrating-process",
        ControllerConfig {
            kp: 0.8,
            ki: 0.0,
            ..ControllerConfig::default()
        },
        100,
        300,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        scalar_plant_configuration(
            100,
            "integrating-process",
            "integrator",
            "[0.0]",
            "gain = 1.0\ninput_bias = 0.0\noutput_bias = 0.0",
            "",
        ),
    );
    assert_step_gates("integrating process", &trace, 10.0, 12.0, 0.01);
}

#[test]
fn pid_benchmark_damps_an_underdamped_second_order_process() {
    let trace = run_loop(
        "underdamped-process",
        ControllerConfig {
            kp: 1.5,
            ki: 0.5,
            kd: 1.0,
            ..ControllerConfig::default()
        },
        50,
        600,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        scalar_plant_configuration(
            50,
            "underdamped-process",
            "second_order",
            "[0.0, 0.0]",
            "gain = 1.0\nnatural_frequency_rad_s = 1.0\ndamping_ratio = 0.2\ninput_bias = 0.0\noutput_bias = 0.0",
            "",
        ),
    );
    assert_step_gates("underdamped second order", &trace, 20.0, 15.0, 0.02);
}

#[test]
fn pid_benchmark_tracks_a_delay_dominant_process() {
    let trace = run_loop(
        "delay-dominant-process",
        ControllerConfig {
            kp: 1.0 / 6.0,
            ki: 1.0 / 6.0,
            ..ControllerConfig::default()
        },
        100,
        600,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        fopdt_configuration(100, 1.0, 1.0, 2.0, ""),
    );
    assert_step_gates("delay dominant", &trace, 20.0, 25.0, 0.02);
}

#[test]
fn pid_benchmark_handles_nonlinear_actuator_dynamics() {
    let trace = run_loop(
        "nonlinear-actuator-process",
        ControllerConfig {
            kp: 1.0,
            ki: 0.5,
            output_minimum: 0.0,
            output_maximum: 1.2,
            tracking_time: 1.0,
            anti_windup: "Combined",
            ..ControllerConfig::default()
        },
        50,
        600,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        scalar_plant_configuration(
            50,
            "nonlinear-actuator-process",
            "first_order",
            "[0.0]",
            "gain = 1.0\ntime_constant_s = 1.0\ninput_bias = 0.0\noutput_bias = 0.0",
            r#"
[plants.actuator]
minimum = 0.0
maximum = 1.2
rate_up_per_s = 1.0
rate_down_per_s = 1.0
deadband = 0.02
lag_s = 0.2
initial_applied = 0.0
"#,
        ),
    );
    assert!(
        trace
            .iter()
            .all(|sample| (0.0..=1.2 + 1e-12).contains(&sample.output)),
        "controller output escaped configured limits"
    );
    assert_step_gates("nonlinear actuator", &trace, 10.0, 15.0, 0.03);
}

#[test]
fn pid_meets_nominal_robustness_and_sampling_gates() {
    let nominal = run_loop(
        "nominal",
        ControllerConfig::default(),
        100,
        200,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        fopdt_configuration(100, 1.0, 1.0, 0.5, ""),
    );
    let nominal_metrics = step_metrics(&nominal, 1.0);
    eprintln!("nominal PI: {nominal_metrics:?}");
    assert!(nominal_metrics.maximum_process_value.is_finite());
    assert!(nominal_metrics.maximum_output.is_finite());
    assert_stable(&nominal, 10.0);
    assert!(nominal_metrics.overshoot_percent <= 20.0);
    assert!(nominal_metrics.settling_time <= 4.0);
    assert!(nominal_metrics.final_error_ratio <= 0.0001);
    assert!(nominal_metrics.integrated_absolute_error <= 1.4);

    for (name, gain, time_constant, dead_time) in [
        ("low-fast-short", 0.8, 0.8, 0.4),
        ("high-slow-long", 1.2, 1.2, 0.6),
        ("high-fast-long", 1.2, 0.8, 0.6),
        ("low-slow-short", 0.8, 1.2, 0.4),
    ] {
        let trace = run_loop(
            name,
            ControllerConfig::default(),
            100,
            300,
            "Input.Setpoint := LREAL#1.0;",
            |_| 1.0,
            fopdt_configuration(100, gain, time_constant, dead_time, ""),
        );
        let metrics = step_metrics(&trace, 1.0);
        eprintln!("robustness {name}: {metrics:?}");
        assert_stable(&trace, 10.0);
        assert!(metrics.overshoot_percent <= 20.0, "{name}: {metrics:?}");
        assert!(metrics.settling_time <= 8.0, "{name}: {metrics:?}");
        assert!(metrics.final_error_ratio <= 0.02, "{name}: {metrics:?}");
    }

    for step_ms in [50_i64, 100, 200] {
        let name = format!("sample-{step_ms}");
        let cycles = (30_000 / step_ms) as usize;
        let trace = run_loop(
            &name,
            ControllerConfig::default(),
            step_ms,
            cycles,
            "Input.Setpoint := LREAL#1.0;",
            |_| 1.0,
            fopdt_configuration(step_ms, 1.0, 1.0, 0.5, ""),
        );
        let metrics = step_metrics(&trace, 1.0);
        eprintln!("sampling {step_ms} ms: {metrics:?}");
        assert_stable(&trace, 10.0);
        assert!(metrics.overshoot_percent <= 20.0, "{name}: {metrics:?}");
        assert!(metrics.settling_time <= 8.0, "{name}: {metrics:?}");
        assert!(metrics.final_error_ratio <= 0.02, "{name}: {metrics:?}");
    }
}

#[test]
fn qualified_formula_candidates_execute_through_public_control_pid() {
    // These constants are independently evaluated Specification 35 oracle
    // values. The test deliberately injects them into the public ST block; it
    // does not implement another tuning formula or controller equation here.
    for (method, kp, ti) in [
        ("simc-balanced", 2.0 / 3.0, 1.0),
        ("amigo", 0.405_555_555_555_555_56, 0.917_857_142_857_142_8),
    ] {
        let trace = run_loop(
            method,
            ControllerConfig {
                kp,
                ki: kp / ti,
                ..ControllerConfig::default()
            },
            100,
            300,
            "Input.Setpoint := LREAL#1.0;",
            |_| 1.0,
            fopdt_configuration(100, 1.0, 1.0, 0.5, ""),
        );
        let metrics = step_metrics(&trace, 1.0);
        eprintln!("qualified {method} PI: {metrics:?}");
        assert_stable(&trace, 10.0);
        assert!(metrics.overshoot_percent <= 20.0, "{method}: {metrics:?}");
        assert!(metrics.settling_time <= 8.0, "{method}: {metrics:?}");
        assert!(metrics.final_error_ratio <= 0.02, "{method}: {metrics:?}");
    }
}

#[test]
fn pi_rejects_a_plant_side_load_disturbance() {
    let trace = run_loop(
        "load-disturbance",
        ControllerConfig::default(),
        100,
        200,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        fopdt_configuration(
            100,
            1.0,
            1.0,
            0.5,
            r#"

[[plant_events]]
at_ms = 10000
plant = "reference-fopdt"
kind = "set_parameter"
parameter = "output_bias"
value = 0.2
"#,
        ),
    );
    assert_stable(&trace, 10.0);
    let disturbance_iae: f64 = trace
        .iter()
        .filter(|sample| sample.time >= 10.0)
        .map(|sample| 0.1 * (sample.setpoint - sample.process_value).abs())
        .sum();
    let final_error = (1.0 - trace.last().unwrap().process_value).abs();
    eprintln!(
        "load disturbance: final_error={final_error:.8}, post_disturbance_iae={disturbance_iae:.8}"
    );
    assert!(final_error <= 0.02);
    assert!(disturbance_iae <= 0.35);
}

#[test]
fn pid_recovers_from_saturation_and_respects_rate_limits() {
    let setpoint_body =
        "IF Scan >= 10 AND Scan < 200 THEN Input.Setpoint := LREAL#4.5; ELSE Input.Setpoint := LREAL#0.0; END_IF;";
    for anti_windup in ["BackCalculation", "Combined"] {
        let config = ControllerConfig {
            kp: 2.83,
            ki: 2.83 / 3.25,
            kd: 2.83 * 0.23,
            output_minimum: -3.5,
            output_maximum: 3.5,
            tracking_time: 3.25,
            anti_windup,
            ..ControllerConfig::default()
        };
        let trace = run_loop(
            anti_windup,
            config,
            100,
            500,
            setpoint_body,
            |step| if (10..200).contains(&step) { 4.5 } else { 0.0 },
            fopdt_configuration(100, 1.0, 3.0, 0.5, ""),
        );
        assert_stable(&trace, 45.0);
        assert!(trace
            .iter()
            .all(|sample| sample.output.abs() <= 3.5 + 1e-12));
        assert!(
            trace
                .iter()
                .any(|sample| sample.time > 20.0 && sample.output < 3.5 - 1e-6),
            "{anti_windup} never left upper saturation"
        );
        let final_pv = trace.last().unwrap().process_value;
        eprintln!("anti-windup {anti_windup}: final_pv={final_pv:.8}");
        assert!(final_pv.abs() <= 0.05, "{anti_windup}: final PV {final_pv}");
    }

    let rate_limited = run_loop(
        "rate-limited",
        ControllerConfig {
            rate_minimum: -1.0,
            rate_maximum: 1.0,
            ..ControllerConfig::default()
        },
        100,
        300,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        fopdt_configuration(100, 1.0, 1.0, 0.5, ""),
    );
    let maximum_rate = rate_limited
        .windows(2)
        .map(|window| (window[1].output - window[0].output).abs() / 0.1)
        .fold(0.0_f64, f64::max);
    let final_error = (1.0 - rate_limited.last().unwrap().process_value).abs();
    eprintln!("rate limit: max_rate={maximum_rate:.12}, final_error={final_error:.8}");
    assert!(maximum_rate <= 1.0 + 1e-10);
    assert!(final_error <= 0.02);
}

#[test]
fn separate_low_pass_reduces_seeded_measurement_noise_activity() {
    let noise = r#"

[[plants.uncertainties]]
id = "measurement-noise"
target = "measurement"
channel = 0
mode = "add"
generator = "normal_approx_white"
mean = 0.0
standard_deviation = 0.02
"#;
    let unfiltered = run_loop(
        "noise-unfiltered",
        ControllerConfig::default(),
        100,
        300,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        fopdt_configuration(100, 1.0, 1.0, 0.5, noise),
    );
    let filtered = run_loop(
        "noise-filtered",
        ControllerConfig {
            filtered: true,
            ..ControllerConfig::default()
        },
        100,
        300,
        "Input.Setpoint := LREAL#1.0;",
        |_| 1.0,
        fopdt_configuration(100, 1.0, 1.0, 0.5, noise),
    );
    let increment_rms = |trace: &[Sample]| {
        let increments = trace
            .windows(2)
            .filter(|window| window[1].time >= 10.0)
            .map(|window| (window[1].output - window[0].output).powi(2))
            .collect::<Vec<_>>();
        (increments.iter().sum::<f64>() / increments.len() as f64).sqrt()
    };
    let raw_rms = increment_rms(&unfiltered);
    let filtered_rms = increment_rms(&filtered);
    let raw_final_error = (1.0 - unfiltered.last().unwrap().process_value).abs();
    let filtered_final_error = (1.0 - filtered.last().unwrap().process_value).abs();
    eprintln!(
        "measurement noise: raw_mv_increment_rms={raw_rms:.8}, filtered={filtered_rms:.8}, raw_final_error={raw_final_error:.8}, filtered_final_error={filtered_final_error:.8}"
    );
    assert_stable(&unfiltered, 10.0);
    assert_stable(&filtered, 10.0);
    assert!(raw_final_error <= 0.05);
    assert!(filtered_final_error <= 0.05);
    assert!(filtered_rms < raw_rms, "filter did not reduce MV activity");
}

#[test]
fn nominal_reference_loop_has_adequate_model_predicted_stability_margins() {
    let mut points = Vec::with_capacity(20_001);
    for index in 0..=20_000 {
        let log_frequency = -4.0 + 8.0 * index as f64 / 20_000.0;
        let frequency = 10.0_f64.powf(log_frequency);
        let controller_magnitude =
            (REFERENCE_KP.powi(2) + (REFERENCE_KI / frequency).powi(2)).sqrt();
        let plant_magnitude = 1.0 / (1.0 + frequency.powi(2)).sqrt();
        let magnitude_db = 20.0 * (controller_magnitude * plant_magnitude).log10();
        let phase_degrees = (-f64::atan2(REFERENCE_KI / frequency, REFERENCE_KP)
            - frequency.atan()
            - 0.5 * frequency)
            .to_degrees();
        points.push((frequency, magnitude_db, phase_degrees));
    }
    let interpolate_crossing = |field: usize, target: f64| {
        points.windows(2).find_map(|pair| {
            let before = if field == 1 { pair[0].1 } else { pair[0].2 } - target;
            let after = if field == 1 { pair[1].1 } else { pair[1].2 } - target;
            if before * after > 0.0 {
                return None;
            }
            let fraction = before.abs() / (before.abs() + after.abs());
            Some((
                pair[0].0 + fraction * (pair[1].0 - pair[0].0),
                pair[0].1 + fraction * (pair[1].1 - pair[0].1),
                pair[0].2 + fraction * (pair[1].2 - pair[0].2),
            ))
        })
    };
    let gain_cross = interpolate_crossing(1, 0.0).expect("resolve gain crossover");
    let phase_cross = interpolate_crossing(2, -180.0).expect("resolve phase crossover");
    let phase_margin = 180.0 + gain_cross.2;
    let gain_margin = -phase_cross.1;
    eprintln!(
        "model-predicted margins: gain={gain_margin:.4} dB, phase={phase_margin:.4} deg, gain_cross={:.5} rad/s, phase_cross={:.5} rad/s",
        gain_cross.0, phase_cross.0
    );
    assert!(gain_margin >= 6.0);
    assert!(phase_margin >= 30.0);
}

#[test]
fn control_water_heater_p_and_pi_show_integral_offset_removal() {
    let plant = || {
        scalar_plant_configuration(
            100,
            "recirculating-water-heater",
            "first_order_dead_time",
            "[20.0]",
            "gain = 0.2\ntime_constant_s = 5.0\ndead_time_s = 1.0\ninitial_delayed_input = 0.0\noutput_bias = 20.0",
            "",
        )
    };
    let controller = |integral_gain| ControllerConfig {
        kp: 4.0,
        ki: integral_gain,
        output_minimum: 0.0,
        output_maximum: 100.0,
        tracking_time: 1.0,
        ..ControllerConfig::default()
    };
    let run = |name, integral_gain| {
        run_loop(
            name,
            controller(integral_gain),
            100,
            1_200,
            "Input.Setpoint := LREAL#35.0;",
            |_| 35.0,
            plant(),
        )
    };
    let proportional = run("water-heater-p", 0.0);
    let proportional_integral = run("water-heater-pi", 0.8);
    let p_error = (35.0 - proportional.last().unwrap().process_value).abs();
    let pi_error = (35.0 - proportional_integral.last().unwrap().process_value).abs();

    eprintln!(
        "water heater: p_final_error={p_error:.4} C, pi_final_error={pi_error:.4} C, p_mv={:.4} %, pi_mv={:.4} %",
        proportional.last().unwrap().output,
        proportional_integral.last().unwrap().output
    );
    assert_stable(&proportional, 120.0);
    assert_stable(&proportional_integral, 120.0);
    assert!(
        p_error >= 5.0,
        "P-only baseline must expose steady-state offset"
    );
    assert!(
        pi_error <= 0.2,
        "integral action must remove the heater offset"
    );
    assert!(proportional_integral
        .iter()
        .all(|sample| sample.output >= 0.0 && sample.output <= 100.0));
}
