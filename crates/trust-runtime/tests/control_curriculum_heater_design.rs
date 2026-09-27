use std::path::Path;
use trust_runtime::config::RuntimeConfig;
use trust_runtime::debug::RuntimeEvent;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::scheduler::{ManualClock, ResourceRunner};
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

fn number(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL at {address}, got {other:?}"),
    }
}

fn setup(relative: &str) -> (Runtime, SimulationController) {
    setup_with(relative, &[])
}

fn setup_with(relative: &str, replacements: &[(&str, &str)]) -> (Runtime, SimulationController) {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative);
    let mut source = std::fs::read_to_string(project.join("src/Main.st")).unwrap();
    for (before, after) in replacements {
        source = source.replace(before, after);
    }
    let session = CompileSession::from_sources(vec![
        SourceFile::with_path("src/Main.st", source),
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
    ]);
    let mut runtime = session.build_runtime().unwrap();
    runtime
        .apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
        .unwrap();
    let simulation = SimulationController::new(
        SimulationConfig::load(&project.join("simulation.toml")).unwrap(),
    );
    (runtime, simulation)
}

#[test]
fn lesson15_records_the_same_heater_step_instead_of_a_different_heat_exchanger() {
    let (mut runtime, mut simulation) =
        setup("05-experiments-identification-and-validation/23-step-response-identification");
    let debug = runtime.enable_debug();
    for scan in 0..401 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%MX80.0").unwrap(),
            Value::Bool(scan > 0 && scan < 400),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        let temperature = number(&runtime, "%IL0");
        assert!(
            (35.0..=37.001).contains(&temperature),
            "lesson15 must start at35°C and approach37°C, got {temperature}"
        );
        assert_eq!(
            number(&runtime, "%QL0"),
            if scan > 0 && scan < 400 { 85.0 } else { 75.0 }
        );
        if scan == 0 {
            assert_eq!(temperature, 35.0);
        }
        if scan <= 10 {
            assert!(
                (temperature - 35.0).abs() < 1e-9,
                "one-second delivery delay"
            );
        }
        if scan == 61 {
            assert!(
                (temperature - 36.264).abs() < 0.05,
                "five-second heater time constant"
            );
        }
        if scan == 399 {
            assert!(
                (temperature - 37.0).abs() < 0.002,
                "steady gain .2°C per command point"
            );
        }
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
}

#[test]
fn lesson16_records_water_and_a_distinct_two_second_probe_response() {
    let (mut runtime, mut simulation) =
        setup("05-experiments-identification-and-validation/24-choose-model-structure");
    let debug = runtime.enable_debug();
    for scan in 0..401 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(scan > 0));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        let measured = number(&runtime, "%IL8");
        if scan == 0 {
            assert_eq!(
                number(&runtime, "%IL0"),
                35.0,
                "same35°C heater, not80°C dryer"
            );
            assert_eq!(measured, 35.0);
        }
        if scan <= 10 {
            assert!((number(&runtime, "%IL0") - 35.0).abs() < 1e-9);
            assert!(
                (measured - 35.0).abs() < 1e-9,
                "initial delayed input must keep both native states at the operating point"
            );
        }
        if scan == 61 {
            let water = number(&runtime, "%IL0");
            assert!((water - 36.264).abs() < 0.05);
            assert!(
                (measured - 35.883).abs() < 0.06,
                "two-second probe response: {measured}"
            );
            assert!(
                water - measured > 0.3,
                "probe lag must be observable independently of water"
            );
        }
        if scan == 400 {
            assert!((measured - 37.0).abs() < 0.003);
        }
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
}

#[test]
fn lesson16_resource_schedule_keeps_native_probe_and_abort_in_physical_units() {
    let relative = "05-experiments-identification-and-validation/24-choose-model-structure";
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative);
    let period = RuntimeConfig::load(project.join("runtime.toml"))
        .unwrap()
        .cycle_interval;
    assert_eq!(period, Duration::from_millis(50));
    let (mut runtime, mut simulation) = setup(relative);
    let debug = runtime.enable_debug();
    let instance = match runtime.storage().get_global("P1") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected the delivered P1 instance, got {other:?}"),
    };
    let clock = ManualClock::new();
    let mut runner = ResourceRunner::new(runtime, clock.clone(), period);
    for tick in 0..=1000 {
        let now = Duration::from_millis(tick * 50);
        simulation
            .apply_pre_cycle(now, runner.runtime_mut())
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%MX80.0").unwrap(),
            Value::Bool(tick >= 20),
        );
        if tick == 800 {
            debug.enqueue_io_write(IoAddress::parse("%MX80.1").unwrap(), Value::Bool(true));
        }
        runner.tick().unwrap();
        let events = debug.drain_runtime_events();
        let starts: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                RuntimeEvent::TaskStart { name, time, .. } => Some((name.as_str(), *time)),
                _ => None,
            })
            .collect();
        if tick > 0 && tick % 2 == 0 {
            assert_eq!(
                starts,
                vec![("MainTask", now)],
                "one task execution per 100 ms"
            );
            assert_eq!(
                runner
                    .runtime()
                    .storage()
                    .get_instance_var(instance, "TemperatureC"),
                Some(&Value::LReal(number(runner.runtime(), "%IL8"))),
                "the program reads the native probe input"
            );
        } else {
            assert!(
                starts.is_empty(),
                "a 50 ms resource tick must not double the 100 ms task rate"
            );
        }
        assert!(!events
            .iter()
            .any(|event| matches!(event, RuntimeEvent::TaskOverrun { .. })));
        let water = number(runner.runtime(), "%IL0");
        let probe = number(runner.runtime(), "%IL8");
        if tick <= 40 {
            assert!((water - 35.0).abs() < 1e-9 && (probe - 35.0).abs() < 1e-9,
                "operating point and delayed prehistory must not create a startup pulse: {water}, {probe}");
        }
        if tick == 140 {
            assert!((water - 36.26424).abs() < 0.04);
            assert!((probe - 35.88318).abs() < 0.04);
        }
        if tick >= 800 {
            assert_eq!(number(runner.runtime(), "%QL0"), 0.0);
        }
        if tick == 1000 {
            assert!((20.0..30.0).contains(&water) && (20.0..30.0).contains(&probe),
                "zero physical heating must cool both native states despite deviation-coordinate actuator limits");
        }
        simulation.apply_post_cycle(now, runner.runtime()).unwrap();
        clock.advance(period);
    }
}

#[test]
fn lesson17_preserves_heater_probe_dynamics_with_declared_bounded_measurement_error() {
    let (mut runtime, mut simulation) =
        setup("05-experiments-identification-and-validation/25-parameter-identifiability");
    let debug = runtime.enable_debug();
    let instance = match runtime.storage().get_global("P1") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected the delivered P1 instance, got {other:?}"),
    };
    let mut maximum_error = 0.0_f64;
    for scan in 0..401 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(scan > 0));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        let measured_input = runtime
            .io()
            .read(&IoAddress::parse("%IL16").unwrap())
            .ok()
            .and_then(|value| match value {
                Value::LWord(bits) => Some(Value::LReal(f64::from_bits(bits))),
                _ => None,
            });
        assert_eq!(
            runtime.storage().get_instance_var(instance, "TemperatureC"),
            measured_input.as_ref(),
            "the reusable lesson must read measured temperature from the native PLC input"
        );
        if scan == 0 {
            assert_eq!(number(&runtime, "%IL0"), 35.0, "same heater initial state");
        }
        let error = (number(&runtime, "%IL16") - number(&runtime, "%IL8")).abs();
        assert!(
            error <= 0.050000001,
            "declared measurement error is bounded±.05°C"
        );
        maximum_error = maximum_error.max(error);
        if scan == 61 {
            assert!((number(&runtime, "%IL8") - 35.883).abs() < 0.06);
        }
        if scan == 400 {
            assert!((number(&runtime, "%IL16") - 37.0).abs() < 0.053);
        }
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
    assert!(
        maximum_error > 0.04,
        "the declared noise must actually reach the recorded measurement"
    );
}

#[test]
fn lesson18_runs_independent_hot_reset_holdouts_and_an_immediate_physical_load() {
    let loop_path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../examples/control/05-experiments-identification-and-validation/26-independent-model-validation/control-loop.json",
    );
    let loop_definition: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(loop_path).unwrap()).unwrap();
    assert_eq!(
        loop_definition["experiment"]["baselineSeconds"].as_f64(),
        Some(5.0),
        "independent holdouts need the same five-second measured baseline as estimation"
    );
    assert_eq!(
        loop_definition["experiment"]["durationSeconds"].as_f64(),
        Some(45.1)
    );
    for (trial, command, final_temperature) in
        [(0_u16, 90.0, 38.0), (1, 65.0, 33.0), (2, 75.0, 32.0)]
    {
        let chosen_trial = format!("TrialKind : INT := {trial};");
        let (mut runtime, mut simulation) = setup_with(
            "05-experiments-identification-and-validation/26-independent-model-validation",
            &[("TrialKind : INT := 0;", &chosen_trial)],
        );
        let debug = runtime.enable_debug();
        let instance = match runtime.storage().get_global("P1") {
            Some(Value::Instance(id)) => *id,
            other => panic!("expected the delivered P1 instance, got {other:?}"),
        };
        for scan in 0..451 {
            simulation
                .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%MX64.0").unwrap(),
                Value::Bool(scan >= 50 && scan < 450),
            );
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            assert_eq!(
                runtime.storage().get_instance_var(instance, "TemperatureC"),
                Some(&Value::LReal(number(&runtime, "%IL8"))),
                "the validation lesson must read its probe from native plant input"
            );
            if scan < 50 {
                assert_eq!(
                    number(&runtime, "%IL8"),
                    35.0,
                    "holdout must start from the35°C heater reset"
                );
            }
            assert_eq!(
                number(&runtime, "%QL0"),
                if scan >= 50 && scan < 450 {
                    command
                } else {
                    75.0
                }
            );
            assert_eq!(
                number(&runtime, "%QL8"),
                if trial == 2 && scan >= 50 && scan < 450 {
                    30.0
                } else {
                    0.0
                }
            );
            if trial == 2 && scan == 51 {
                assert!(
                    number(&runtime, "%IL0") < 34.99,
                    "load changes water before heater delivery delay elapses"
                );
            }
            if scan == 449 {
                assert!((number(&runtime, "%IL8") - final_temperature).abs() < 0.005);
            }
            simulation
                .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                .unwrap();
        }
    }
}

#[test]
fn lesson19_applies_coefficient_matched_pi_from_the_common_hot_reset() {
    let (mut runtime, mut simulation) = setup("06-pid-tuning-methods/30-model-based-pi-tuning");
    let debug = runtime.enable_debug();
    for scan in 0..602 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%MX96.1").unwrap(),
            Value::Bool(scan > 0 && scan <= 600),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        if scan == 0 {
            assert_eq!(
                number(&runtime, "%IL0"),
                35.0,
                "same heater reset, not a50°C heat exchanger"
            );
            assert_eq!(number(&runtime, "%QL0"), 75.0);
        }
        assert!((number(&runtime, "%IL8") - number(&runtime, "%IL0")).abs() < 1e-12,
            "The controller must use the same heater measurement validated in Lesson18, without an unmodelled probe lag");
        if scan == 1 {
            let Value::Instance(program) = runtime.storage().get_global("P1").unwrap() else {
                panic!("Lesson19 must execute its P1 program");
            };
            let Some(Value::Instance(controller)) =
                runtime.storage().get_instance_var(*program, "Controller")
            else {
                panic!("Lesson19 must instantiate the real Control.PID block");
            };
            assert_eq!(
                runtime
                    .storage()
                    .get_instance(*controller)
                    .unwrap()
                    .type_name
                    .as_str(),
                "Control.PID"
            );
            let Some(Value::Struct(state)) = runtime
                .storage()
                .get_instance_var(*controller, "StateValue")
            else {
                panic!("Control.PID must retain its executed state");
            };
            assert_eq!(state.field("Setpoint"), Some(&Value::LReal(38.0)));
            assert_eq!(state.field("ProcessValue"), Some(&Value::LReal(35.0)));
            assert_eq!(state.field("NominalOutput"), Some(&Value::LReal(90.3)));
            assert_eq!(
                state.field("SaturatedOutput"),
                Some(&Value::LReal(number(&runtime, "%QL0")))
            );
            assert_eq!(
                runtime.storage().get_instance_var(*program, "TemperatureC"),
                Some(&Value::LReal(number(&runtime, "%IL8"))),
                "Lesson19 must read the native probe input"
            );
            assert!(
                (number(&runtime, "%ML32") - 90.3).abs() < 1e-9,
                "Kp5 Ki1 and I75 give90.3% first request"
            );
        }
        assert!((0.0..=100.0).contains(&number(&runtime, "%QL0")));
        if scan == 601 {
            assert_eq!(number(&runtime, "%QL0"), 75.0);
        }
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
}

#[test]
fn lesson19_halves_gains_for_double_response_time_and_inhibits_invalid_models() {
    for (before, after, expected) in [
        ("TrialKind : INT := 0;", "TrialKind : INT := 1;", 75.0),
        (
            "DesiredResponseS : LREAL := 5.0;",
            "DesiredResponseS : LREAL := 10.0;",
            82.65,
        ),
        (
            "ProcessGainCPerPoint : LREAL := 0.2;",
            "ProcessGainCPerPoint : LREAL := 0.0;",
            75.0,
        ),
        (
            "DesiredResponseS : LREAL := 5.0;",
            "DesiredResponseS : LREAL := 0.0;",
            75.0,
        ),
    ] {
        let (mut runtime, mut simulation) = setup_with(
            "06-pid-tuning-methods/30-model-based-pi-tuning",
            &[(before, after)],
        );
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX96.1").unwrap(), Value::Bool(true));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        if before.starts_with("TrialKind") {
            assert_eq!(
                number(&runtime, "%QL8"),
                30.0,
                "the chosen load trial must execute"
            );
        }
        assert!(
            (number(&runtime, "%QL0") - expected).abs() < 1e-9,
            "declared model arithmetic and no division by zero"
        );
    }
}

#[test]
fn lesson20_runs_feedback_relays_on_the_native_hot_heater_and_aborts_to_zero() {
    let (mut runtime, mut simulation) = setup("06-pid-tuning-methods/29-relay-tuning");
    let debug = runtime.enable_debug();
    let mut rising_edges = Vec::new();
    let mut previous = 75.0;
    for scan in 0..603 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%MX8.0").unwrap(),
            Value::Bool(scan > 0 && scan <= 600),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        let pv = number(&runtime, "%IL0");
        let command = number(&runtime, "%QL0");
        if scan == 0 {
            assert!(
                (pv - 35.0).abs() <= 0.020001,
                "relay tuning must use the35°C heater with bounded±.02°C noise: {pv}"
            );
        }
        assert!((33.0..=37.0).contains(&pv));
        if scan == 1 {
            assert_eq!(
                command, 80.0,
                "the reviewed relay starts High at the hot reset"
            );
        }
        if scan > 0 && scan <= 600 {
            assert!(
                command == 70.0 || command == 80.0,
                "the default relay switches 75 +/- 5 points: {command}"
            );

            if command > 75.0 && previous <= 75.0 {
                rising_edges.push(scan);
                if scan > 1 {
                    assert!(pv <= 34.9, "feedback must cause the rising switch");
                }
            }
            if command < 75.0 && previous > 75.0 {
                assert!(pv >= 35.1, "feedback must cause the falling switch");
            }
        } else {
            assert_eq!(
                command, 75.0,
                "normal completion restores the holding command"
            );
        }
        previous = command;
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
    assert!(
        rising_edges.len() >= 6,
        "need at least five complete cycles to discard two and retain three: {rising_edges:?}"
    );
    let instance = match runtime.storage().get_global("P1") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected P1, got {other:?}"),
    };
    let scalar = |name| match runtime.storage().get_instance_var(instance, name) {
        Some(Value::LReal(value)) => *value,
        other => panic!("expected retained scalar {name}, got {other:?}"),
    };
    let cycles = scalar("RetainedCycles");
    assert!(
        cycles >= 3.0,
        "discard two startup cycles and retain at least three complete cycles"
    );
    let amplitude = scalar("MeanAmplitudeC");
    let period = scalar("MeanPeriodS");
    assert!(amplitude > 0.0 && period > 0.0);
    assert!(scalar("MaximumAmplitudeC") <= 1.1 * amplitude);
    assert!(scalar("MinimumAmplitudeC") >= 0.9 * amplitude);
    assert!(scalar("MaximumPeriodS") <= 1.1 * period);
    assert!(scalar("MinimumPeriodS") >= 0.9 * period);
    assert_eq!(
        runtime
            .storage()
            .get_instance_var(instance, "CandidateValid"),
        Some(&Value::Bool(true))
    );
    let expected_kp = 0.45 * 20.0 / (std::f64::consts::PI * amplitude);
    assert!((scalar("CandidateKp") - expected_kp).abs() < 1e-9);
    assert!((scalar("CandidateKi") - expected_kp * 1.2 / period).abs() < 1e-9);
    debug.enqueue_io_write(IoAddress::parse("%MX8.0").unwrap(), Value::Bool(true));
    debug.enqueue_io_write(IoAddress::parse("%MX8.1").unwrap(), Value::Bool(true));
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    assert_eq!(number(&runtime, "%QL0"), 0.0);
    debug.enqueue_io_write(IoAddress::parse("%MX8.1").unwrap(), Value::Bool(false));
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(32.0_f64.to_bits()),
    );
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "out-of-bounds measurement removes heat"
    );
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX8.1").unwrap())
            .unwrap(),
        Value::Bool(true)
    );
    // Non-finite typed I/O is rejected at the runtime boundary before ST runs.
    // The already latched zero command must remain in effect.
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(f64::NAN.to_bits()),
    );
    runtime.advance_time(Duration::from_millis(100));
    let error = runtime.execute_cycle().unwrap_err();
    assert!(error.to_string().contains("must be finite"));
    assert_eq!(number(&runtime, "%QL0"), 0.0);
}

#[test]
fn lesson20_aborts_if_the_reviewed_relay_amplitude_changes_during_acquisition() {
    let (mut runtime, mut simulation) = setup("06-pid-tuning-methods/29-relay-tuning");
    let debug = runtime.enable_debug();
    for scan in 0..10 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX8.0").unwrap(), Value::Bool(true));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
    let program = match runtime.storage().get_global("P1") {
        Some(Value::Instance(id)) => *id,
        other => panic!("expected P1, got {other:?}"),
    };
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "RelayAmplitude", Value::LReal(10.0)));
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "a different in-range amplitude invalidates the reviewed experiment"
    );
    assert_eq!(
        runtime
            .storage()
            .get_instance_var(program, "CandidateValid"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        runtime.storage().get_instance_var(program, "SafetyAbort"),
        Some(&Value::Bool(true))
    );
}

#[test]
fn lesson20_hmi_only_binds_signals_of_the_executed_heater_relay() {
    let (runtime, _) = setup("06-pid-tuning-methods/29-relay-tuning");
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/control/06-pid-tuning-methods/29-relay-tuning");
    let descriptor = trust_runtime::hmi::load_hmi_dir(&project).unwrap();
    let diagnostics = trust_runtime::hmi::validate_hmi_bindings("Res", &runtime.metadata_snapshot(), None, &descriptor);
    assert!(diagnostics.is_empty(), "The relay HMI must bind the executed one-heater project: {diagnostics:?}");
}

#[test]
fn lesson21_compares_selected_scalar_candidates_with_matched_trials_and_activity() {
    for (kp, ki) in [(5.0, 1.0), (2.5, 0.5)] {
        for trial in [0_u16, 1] {
            let kp_line = format!("KpPctPerC : LREAL := {kp};");
            let ki_line = format!("KiPctPerCs : LREAL := {ki};");
            let (mut runtime, mut simulation) = setup_with(
                "06-pid-tuning-methods/31-tuning-method-comparison",
                &[
                    ("KpPctPerC : LREAL := 5.0;", &kp_line),
                    ("KiPctPerCs : LREAL := 1.0;", &ki_line),
                ],
            );
            let debug = runtime.enable_debug();
            let mut last_activity = 0.0;
            let mut peak = 35.0_f64;
            let mut minimum = 35.0_f64;
            let mut late_error = 0.0_f64;
            for scan in 0..602 {
                simulation
                    .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                    .unwrap();
                debug.enqueue_io_write(
                    IoAddress::parse("%MX104.0").unwrap(),
                    Value::Bool(scan > 0 && scan <= 600),
                );
                debug.enqueue_io_write(IoAddress::parse("%MW80").unwrap(), Value::Word(trial));
                runtime.advance_time(Duration::from_millis(100));
                runtime.execute_cycle().unwrap();
                if scan == 0 {
                    assert_eq!(
                        number(&runtime, "%IL0"),
                        35.0,
                        "all candidates start on the same heater"
                    );
                    assert_eq!(number(&runtime, "%QL0"), 75.0);
                }
                if scan == 1 {
                    let expected = if trial == 0 {
                        75.0 + 3.0 * kp + 0.3 * ki
                    } else {
                        75.0
                    };
                    assert!((number(&runtime, "%QL0") - expected).abs() < 1e-9);
                    assert!((number(&runtime, "%ML64") - (expected - 75.0).abs()).abs() < 1e-9);
                }
                if scan > 0 && scan <= 600 {
                    assert_eq!(
                        number(&runtime, "%ML0"),
                        if trial == 0 { 38.0 } else { 35.0 }
                    );
                    assert_eq!(
                        number(&runtime, "%QL8"),
                        if trial == 0 { 0.0 } else { 30.0 }
                    );
                    let pv = number(&runtime, "%ML40");
                    peak = peak.max(pv);
                    minimum = minimum.min(pv);
                    if scan >= 301 {
                        late_error =
                            late_error.max((pv - if trial == 0 { 38.0 } else { 35.0 }).abs());
                    }
                    assert!((0.0..=100.0).contains(&number(&runtime, "%QL0")));
                    let activity = number(&runtime, "%ML64");
                    assert!(activity >= last_activity);
                    last_activity = activity;
                }
                if scan == 601 {
                    assert_eq!(
                        number(&runtime, "%ML64"),
                        last_activity,
                        "shutdown movement must not enter activity"
                    );
                    assert_eq!(number(&runtime, "%QL8"), 0.0);
                }
                simulation
                    .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                    .unwrap();
            }
            eprintln!("Kp={kp} Ki={ki} trial={trial}: peak={peak:.5} minimum={minimum:.5} lateError={late_error:.5} activity={last_activity:.5}");
        }
    }
}

#[test]
fn heater_design_lessons_latch_invalid_measurements_and_command_zero_heat_on_abort() {
    for (project, enable, abort) in [
        (
            "05-experiments-identification-and-validation/23-step-response-identification",
            "%MX80.0",
            "%MX80.1",
        ),
        (
            "05-experiments-identification-and-validation/24-choose-model-structure",
            "%MX80.0",
            "%MX80.1",
        ),
        (
            "05-experiments-identification-and-validation/25-parameter-identifiability",
            "%MX80.0",
            "%MX80.1",
        ),
        (
            "05-experiments-identification-and-validation/26-independent-model-validation",
            "%MX64.0",
            "%MX64.1",
        ),
        (
            "06-pid-tuning-methods/30-model-based-pi-tuning",
            "%MX96.1",
            "%MX96.3",
        ),
        (
            "06-pid-tuning-methods/31-tuning-method-comparison",
            "%MX104.0",
            "%MX104.2",
        ),
    ] {
        let (mut runtime, mut simulation) = setup(project);
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        for (address, value) in [(enable, Value::Bool(true)), (abort, Value::Bool(true))] {
            debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value);
        }
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            number(&runtime, "%QL0"),
            0.0,
            "{project}: explicit abort must remove heat, not restore the normal75% bias"
        );
        debug.enqueue_io_write(IoAddress::parse(abort).unwrap(), Value::Bool(false));
        let measured_input = if project.ends_with("24-choose-model-structure")
            || project.ends_with("26-independent-model-validation")
            || project.ends_with("30-model-based-pi-tuning")
        {
            "%IL8"
        } else if project.ends_with("25-parameter-identifiability") {
            "%IL16"
        } else {
            "%IL0"
        };
        debug.enqueue_io_write(
            IoAddress::parse(measured_input).unwrap(),
            Value::LWord(
                if project.contains("26-independent") {
                    200.0_f64
                } else {
                    1000.0_f64
                }
                .to_bits(),
            ),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            number(&runtime, "%QL0"),
            0.0,
            "invalid measurement inhibits heating"
        );
        assert_eq!(
            runtime
                .io()
                .read(&IoAddress::parse(abort).unwrap())
                .unwrap(),
            Value::Bool(true)
        );
    }
}

#[test]
fn lesson52_shapes_the_heater_request_before_limiting_and_scales_integral_tracking() {
    for (zero, request, integral) in [(3.0, 97.95, 75.3), (10.0, 151.5, 74.27)] {
        let declaration = format!("ZeroTimeS : LREAL := {zero};");
        let (mut runtime, mut simulation) = setup_with(
            "07-classical-stability-and-frequency-design/49-loop-shaping",
            &[("ZeroTimeS : LREAL := 3.0;", &declaration)],
        );
        let debug = runtime.enable_debug();
        for scan in 0..2 {
            simulation
                .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(IoAddress::parse("%MX96.0").unwrap(), Value::Bool(scan > 0));
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            if scan == 0 {
                assert_eq!(
                    number(&runtime, "%IL0"),
                    35.0,
                    "loop shaping uses the same35°C heater, not a speed drive"
                );
            }
            if scan == 1 {
                assert!(
                    (number(&runtime, "%ML32") - request).abs() < 1e-9,
                    "shape the complete command deviation using the old state"
                );
                assert!(
                    (number(&runtime, "%ML16") - integral).abs() < 1e-9,
                    "track the shaped request with1/a correction"
                );
                assert!((number(&runtime, "%QL0") - request.min(100.0)).abs() < 1e-9);
            }
            simulation
                .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                .unwrap();
        }
    }
}

#[test]
fn lesson52_runs_all_three_matched_trials_and_measures_the_declared_noise_window() {
    for zero in [2.0, 3.0, 10.0] {
        for trial in [0_u16, 1, 2] {
            let declaration = format!("ZeroTimeS : LREAL := {zero};");
            let (mut runtime, mut simulation) = setup_with(
                "07-classical-stability-and-frequency-design/49-loop-shaping",
                &[("ZeroTimeS : LREAL := 3.0;", &declaration)],
            );
            let debug = runtime.enable_debug();
            let mut peak = 35.0_f64;
            let mut minimum = 35.0_f64;
            let mut late_error = 0.0_f64;
            let mut noise_min = 100.0_f64;
            let mut noise_max = 0.0_f64;
            for scan in 0..602 {
                simulation
                    .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                    .unwrap();
                debug.enqueue_io_write(
                    IoAddress::parse("%MX96.0").unwrap(),
                    Value::Bool(scan > 0 && scan <= 600),
                );
                debug.enqueue_io_write(IoAddress::parse("%MW80").unwrap(), Value::Word(trial));
                runtime.advance_time(Duration::from_millis(100));
                runtime.execute_cycle().unwrap();
                if scan == 0 {
                    assert_eq!(
                        number(&runtime, "%IL0"),
                        35.0,
                        "all shaping trials use the common heater reset"
                    );
                }
                if scan > 0 && scan <= 600 {
                    let elapsed = (scan - 1) as f64 * 0.1;
                    let expected_noise = if trial == 2 {
                        0.05 * elapsed.sin()
                    } else {
                        0.0
                    };
                    assert!(
                        (number(&runtime, "%ML24") - expected_noise).abs() < 1e-9,
                        "noise is.05sin(t),1rad/s, from experiment start"
                    );
                    assert_eq!(
                        number(&runtime, "%QL8"),
                        if trial == 1 { 30.0 } else { 0.0 }
                    );
                    let pv = number(&runtime, "%ML48");
                    peak = peak.max(pv);
                    minimum = minimum.min(pv);
                    if elapsed >= 30.0 {
                        late_error =
                            late_error.max((pv - if trial == 0 { 38.0 } else { 35.0 }).abs());
                    }
                    let applied = number(&runtime, "%QL0");
                    assert!((0.0..=100.0).contains(&applied));
                    if elapsed >= 12.0 * std::f64::consts::PI
                        && elapsed <= 18.0 * std::f64::consts::PI
                    {
                        noise_min = noise_min.min(applied);
                        noise_max = noise_max.max(applied);
                    }
                }
                if scan == 601 {
                    assert_eq!(number(&runtime, "%QL8"), 0.0);
                    assert_eq!(number(&runtime, "%ML24"), 0.0);
                }
                simulation
                    .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                    .unwrap();
            }
            eprintln!("Tz={zero} trial={trial}: peak={peak:.5} minimum={minimum:.5} lateError={late_error:.5} commandHalfAmplitude={:.5}",(noise_max-noise_min)*0.5);
        }
    }
}
