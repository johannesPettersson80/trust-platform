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
    let simulation =
        SimulationController::new(SimulationConfig::load(project.join("simulation.toml")).unwrap());
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
                Value::Bool((50..450).contains(&scan)),
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
                if (50..450).contains(&scan) {
                    command
                } else {
                    75.0
                }
            );
            assert_eq!(
                number(&runtime, "%QL8"),
                if trial == 2 && (50..450).contains(&scan) {
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

fn lesson19_scan(runtime: &mut Runtime, simulation: &mut SimulationController, scan: i64) {
    simulation
        .apply_pre_cycle(Duration::from_millis(scan * 100), runtime)
        .unwrap();
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    simulation
        .apply_post_cycle(Duration::from_millis((scan + 1) * 100), runtime)
        .unwrap();
}

fn lesson19_gain(
    runtime: &Runtime,
    program: trust_runtime::memory::InstanceId,
    field: &str,
) -> f64 {
    let Some(Value::Struct(parameters)) = runtime.storage().get_instance_var(program, "Parameters")
    else {
        panic!("Lesson19 must keep a complete Control.PID parameter record");
    };
    match parameters.field(field) {
        Some(Value::LReal(value)) => *value,
        other => panic!("expected LREAL {field}, got {other:?}"),
    }
}

#[test]
fn lesson19_executes_the_gains_control_studio_applies_without_overwriting_them() {
    let (mut runtime, mut simulation) = setup("06-pid-tuning-methods/30-model-based-pi-tuning");
    let debug = runtime.enable_debug();
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("Lesson19 must execute its P1 program");
    };
    for scan in 0..10 {
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            number(&runtime, "%QL0"),
            75.0,
            "Manual holds the hot-reset heater command"
        );
        assert!(
            (number(&runtime, "%IL8") - 35.0).abs() < 1e-9,
            "the same heater holds 35°C"
        );
        assert!((number(&runtime, "%IL8") - number(&runtime, "%IL0")).abs() < 1e-12,
            "The controller must use the same heater measurement validated in Lesson18, without an unmodelled probe lag");
    }
    let Some(Value::Instance(controller)) =
        runtime.storage().get_instance_var(program, "Controller")
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
    // Control Studio's Apply writes the complete reviewed record at a task boundary.
    let mut proposed = runtime
        .storage()
        .get_instance_var(program, "Parameters")
        .unwrap()
        .clone();
    let Value::Struct(ref mut record) = proposed else {
        panic!("complete Control.PID parameter record");
    };
    for (field, value) in [("ProportionalGain", 2.5), ("IntegralGain", 0.5)] {
        *std::sync::Arc::make_mut(record).field_mut(field).unwrap() = Value::LReal(value);
    }
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "Parameters", proposed.clone()));
    for scan in 10..13 {
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            runtime.storage().get_instance_var(program, "Parameters"),
            Some(&proposed),
            "cyclic code must preserve the gains Control Studio applied"
        );
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    debug.enqueue_io_write(
        IoAddress::parse("%ML0").unwrap(),
        Value::LWord(38.0_f64.to_bits()),
    );
    lesson19_scan(&mut runtime, &mut simulation, 13);
    assert!(
        (number(&runtime, "%QL0") - 82.65).abs() < 1e-9,
        "applied Kp2.5 Ki0.5 from I75 give 82.65% for SP35→38, got {}",
        number(&runtime, "%QL0")
    );
    assert_eq!(
        runtime.storage().get_instance_var(program, "TemperatureC"),
        Some(&Value::LReal(number(&runtime, "%IL8"))),
        "Lesson19 must read the native heater input"
    );
    for scan in 14..20 {
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            number(&runtime, "%ML0"),
            38.0,
            "the program keeps the setpoint it was given"
        );
        assert!((0.0..=100.0).contains(&number(&runtime, "%QL0")));
    }
}

#[test]
fn lesson19_startup_design_uses_coefficient_matching_and_inhibits_invalid_models() {
    for (before, after, kp, ki) in [
        (
            "ProcessGainCPerPoint : LREAL := 0.2;",
            "ProcessGainCPerPoint : LREAL := 0.0;",
            0.0,
            0.0,
        ),
        (
            "DesiredResponseS : LREAL := 5.0;",
            "DesiredResponseS : LREAL := 0.0;",
            0.0,
            0.0,
        ),
        (
            "DesiredResponseS : LREAL := 5.0;",
            "DesiredResponseS : LREAL := 10.0;",
            5.0 / (0.2 * (10.0 + 1.0)),
            5.0 / (0.2 * (10.0 + 1.0)) / 5.0,
        ),
        (
            "DesiredResponseS : LREAL := 5.0;",
            "DesiredResponseS : LREAL := 5.0;",
            5.0 / (0.2 * (5.0 + 1.0)),
            5.0 / (0.2 * (5.0 + 1.0)) / 5.0,
        ),
    ] {
        let (mut runtime, mut simulation) = setup_with(
            "06-pid-tuning-methods/30-model-based-pi-tuning",
            &[(before, after)],
        );
        let debug = runtime.enable_debug();
        let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
            panic!("Lesson19 must execute its P1 program");
        };
        lesson19_scan(&mut runtime, &mut simulation, 0);
        assert!(
            (lesson19_gain(&runtime, program, "ProportionalGain") - kp).abs() < 1e-12,
            "{after}: the delay-aware rule gives Kp = tau/(K*(lambda+L)) = {kp}"
        );
        assert!(
            (lesson19_gain(&runtime, program, "IntegralGain") - ki).abs() < 1e-12,
            "{after}: with Ti = tau the rule gives Ki = Kp/tau = {ki}"
        );
        debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
        debug.enqueue_io_write(
            IoAddress::parse("%ML0").unwrap(),
            Value::LWord(38.0_f64.to_bits()),
        );
        lesson19_scan(&mut runtime, &mut simulation, 1);
        let expected = if kp > 0.0 {
            75.0 + kp * 3.0 + ki * 3.0 * 0.1
        } else {
            75.0
        };
        assert!(
            (number(&runtime, "%QL0") - expected).abs() < 1e-9,
            "{after}: first automatic request must be {expected}%, got {}",
            number(&runtime, "%QL0")
        );
    }
}

#[test]
fn lesson20_executes_the_gains_control_studio_applies_without_overwriting_them() {
    let (mut runtime, mut simulation) = setup("06-pid-tuning-methods/29-relay-tuning");
    let debug = runtime.enable_debug();
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("Lesson20 must execute its P1 program");
    };
    for scan in 0..10 {
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            number(&runtime, "%QL0"),
            75.0,
            "Manual holds the hot-reset heater command"
        );
        assert!(
            (number(&runtime, "%IL8") - 35.0).abs() <= 0.020001,
            "the same heater holds 35°C within the ±.02°C noise"
        );
    }
    let Some(Value::Instance(controller)) = runtime
        .storage()
        .get_instance_var(program, "Controller")
        .cloned()
    else {
        panic!("Lesson20 must instantiate the real Control.PID block");
    };
    assert_eq!(
        runtime
            .storage()
            .get_instance(controller)
            .unwrap()
            .type_name
            .as_str(),
        "Control.PID"
    );
    let mut proposed = runtime
        .storage()
        .get_instance_var(program, "Parameters")
        .unwrap()
        .clone();
    let Value::Struct(ref mut record) = proposed else {
        panic!("complete Control.PID parameter record");
    };
    for (field, value) in [("ProportionalGain", 4.0), ("IntegralGain", 0.4)] {
        *std::sync::Arc::make_mut(record).field_mut(field).unwrap() = Value::LReal(value);
    }
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "Parameters", proposed.clone()));
    for scan in 10..13 {
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            runtime.storage().get_instance_var(program, "Parameters"),
            Some(&proposed),
            "cyclic code must preserve the gains Control Studio applied"
        );
    }
    // Velocity form from the retained state: the setpoint started at the noisy measured 35 °C.
    let (setpoint_before, pv_before) = {
        let Some(Value::Struct(state)) =
            runtime.storage().get_instance_var(controller, "StateValue")
        else {
            panic!("Control.PID must retain its executed state");
        };
        let field = |name: &str| match state.field(name) {
            Some(Value::LReal(value)) => *value,
            other => panic!("{name}: {other:?}"),
        };
        (field("Setpoint"), field("ProcessValue"))
    };
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    debug.enqueue_io_write(
        IoAddress::parse("%ML0").unwrap(),
        Value::LWord(38.0_f64.to_bits()),
    );
    lesson19_scan(&mut runtime, &mut simulation, 13);
    let pv = number(&runtime, "%IL8");
    let expected =
        75.0 + 4.0 * ((38.0 - setpoint_before) - (pv - pv_before)) + 0.4 * 0.1 * (38.0 - pv);
    assert!(
        (number(&runtime, "%QL0") - expected).abs() < 1e-9,
        "applied Kp4 Ki.4 drive the first automatic request, got {} (expected {expected})",
        number(&runtime, "%QL0")
    );
    for scan in 14..20 {
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            number(&runtime, "%ML0"),
            38.0,
            "the program keeps the setpoint it was given"
        );
    }
}

#[test]
fn lesson20_startup_design_uses_the_relay_rule_and_inhibits_invalid_cycles() {
    let ku = 4.0 * 10.0 / (std::f64::consts::PI * 0.45);
    let (kp, ki) = (0.45 * ku, 0.45 * ku / (4.5 / 1.2));
    for (before, after, kp, ki) in [
        (
            "CycleAmplitudeC : LREAL := 0.45;",
            "CycleAmplitudeC : LREAL := 0.0;",
            0.0,
            0.0,
        ),
        (
            "CyclePeriodS : LREAL := 4.5;",
            "CyclePeriodS : LREAL := 0.0;",
            0.0,
            0.0,
        ),
        (
            "CyclePeriodS : LREAL := 4.5;",
            "CyclePeriodS : LREAL := 4.5;",
            kp,
            ki,
        ),
    ] {
        let (mut runtime, mut simulation) =
            setup_with("06-pid-tuning-methods/29-relay-tuning", &[(before, after)]);
        let debug = runtime.enable_debug();
        let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
            panic!("Lesson20 must execute its P1 program");
        };
        lesson19_scan(&mut runtime, &mut simulation, 0);
        assert!(
            (lesson19_gain(&runtime, program, "ProportionalGain") - kp).abs() < 1e-9,
            "{after}: relay rule gives Kp = .45*4d/(pi*a) = {kp}"
        );
        assert!(
            (lesson19_gain(&runtime, program, "IntegralGain") - ki).abs() < 1e-9,
            "{after}: relay rule gives Ki = Kp/(Pu/1.2) = {ki}"
        );
        debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
        debug.enqueue_io_write(
            IoAddress::parse("%ML0").unwrap(),
            Value::LWord(38.0_f64.to_bits()),
        );
        lesson19_scan(&mut runtime, &mut simulation, 1);
        if kp == 0.0 {
            assert_eq!(
                number(&runtime, "%QL0"),
                75.0,
                "{after}: an unusable relay record keeps the heater in Manual"
            );
        } else {
            assert_eq!(
                number(&runtime, "%QL0"),
                100.0,
                "{after}: the relay candidate's first request exceeds 100 % and is limited"
            );
        }
    }
}

#[test]
fn lesson20_hmi_only_binds_signals_of_the_executed_heater_relay() {
    let (runtime, _) = setup("06-pid-tuning-methods/29-relay-tuning");
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/06-pid-tuning-methods/29-relay-tuning");
    let descriptor = trust_runtime::hmi::load_hmi_dir(&project).unwrap();
    let diagnostics = trust_runtime::hmi::validate_hmi_bindings(
        "Res",
        &runtime.metadata_snapshot(),
        None,
        &descriptor,
    );
    assert!(
        diagnostics.is_empty(),
        "The relay HMI must bind the executed one-heater project: {diagnostics:?}"
    );
}

/// The heater PI of Lessons 21 and 22 runs the actual `Control.PID` with the gains Control Studio applies.
fn executes_the_pi_control_studio_applies(project: &str, kp: f64, ki: f64) {
    let (mut runtime, mut simulation) = setup(project);
    let debug = runtime.enable_debug();
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("{project} must execute its P1 program");
    };
    for scan in 0..10 {
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            number(&runtime, "%QL0"),
            75.0,
            "Manual holds the hot-reset heater command"
        );
        assert!(
            (number(&runtime, "%IL8") - 35.0).abs() <= 0.020001,
            "the heater holds 35°C within its ±.02°C noise"
        );
    }
    let Some(Value::Instance(controller)) = runtime
        .storage()
        .get_instance_var(program, "Controller")
        .cloned()
    else {
        panic!("{project} must instantiate the real Control.PID block");
    };
    assert_eq!(
        runtime
            .storage()
            .get_instance(controller)
            .unwrap()
            .type_name
            .as_str(),
        "Control.PID"
    );
    let Some(mut proposed) = runtime
        .storage()
        .get_instance_var(program, "Parameters")
        .cloned()
    else {
        panic!("{project} must keep the complete Control.PID parameter record that Control Studio applies");
    };
    let Value::Struct(ref mut record) = proposed else {
        panic!("complete Control.PID parameter record");
    };
    for (field, value) in [("ProportionalGain", kp), ("IntegralGain", ki)] {
        *std::sync::Arc::make_mut(record).field_mut(field).unwrap() = Value::LReal(value);
    }
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "Parameters", proposed.clone()));
    for scan in 10..13 {
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            runtime.storage().get_instance_var(program, "Parameters"),
            Some(&proposed),
            "{project}: cyclic code must preserve the gains Control Studio applied"
        );
    }
    let (setpoint_before, pv_before) = {
        let Some(Value::Struct(state)) =
            runtime.storage().get_instance_var(controller, "StateValue")
        else {
            panic!("Control.PID must retain its executed state");
        };
        let field = |name: &str| match state.field(name) {
            Some(Value::LReal(value)) => *value,
            other => panic!("{name}: {other:?}"),
        };
        (field("Setpoint"), field("ProcessValue"))
    };
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    debug.enqueue_io_write(
        IoAddress::parse("%ML0").unwrap(),
        Value::LWord(38.0_f64.to_bits()),
    );
    lesson19_scan(&mut runtime, &mut simulation, 13);
    let pv = number(&runtime, "%IL8");
    let expected =
        75.0 + kp * ((38.0 - setpoint_before) - (pv - pv_before)) + ki * 0.1 * (38.0 - pv);
    assert!((number(&runtime, "%QL0") - expected).abs() < 1e-9,
        "{project}: the applied Kp {kp} and Ki {ki} drive the first automatic request, got {} (expected {expected})", number(&runtime, "%QL0"));
}

#[test]
fn lesson21_executes_each_candidate_control_studio_applies_without_overwriting_it() {
    executes_the_pi_control_studio_applies(
        "06-pid-tuning-methods/31-tuning-method-comparison",
        4.9,
        1.0,
    );
}

#[test]
fn robustness_margins_executes_the_pi_whose_margins_control_studio_shows() {
    // Lesson 58 runs the balanced PI of Lesson 22 on the same heater.
    executes_the_pi_control_studio_applies(
        "07-classical-stability-and-frequency-design/88-robustness-margins",
        5.769231,
        1.153846,
    );
}

#[test]
fn tuning_to_requirements_executes_the_pi_control_studio_designs_without_overwriting_it() {
    // Set B's balanced response: lambda = 10/3 s gives Kp = 5/(0.2 (10/3 + 1)) and Ki = Kp/5.
    executes_the_pi_control_studio_applies(
        "06-pid-tuning-methods/85-tuning-to-requirements",
        5.769231,
        1.153846,
    );
}

#[test]
fn lesson21_counts_command_activity_from_the_initial_holding_command() {
    let (mut runtime, mut simulation) = setup("06-pid-tuning-methods/31-tuning-method-comparison");
    let debug = runtime.enable_debug();
    lesson19_scan(&mut runtime, &mut simulation, 0);
    assert_eq!(
        number(&runtime, "%ML64"),
        0.0,
        "holding 75% has not moved the command"
    );
    // The lesson's worked example: 75% → 80% → 78% moves the command 5 + 2 = 7 points.
    for (scan, command, activity) in [(1, 80.0, 5.0), (2, 78.0, 7.0), (3, 78.0, 7.0)] {
        debug.enqueue_io_write(
            IoAddress::parse("%ML72").unwrap(),
            Value::LWord(f64::to_bits(command)),
        );
        lesson19_scan(&mut runtime, &mut simulation, scan);
        assert_eq!(
            number(&runtime, "%QL0"),
            command,
            "Manual applies the commanded heater power"
        );
        assert!(
            (number(&runtime, "%ML64") - activity).abs() < 1e-9,
            "after {command}%: activity {} (expected {activity})",
            number(&runtime, "%ML64")
        );
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
            "%MX96.1",
            "%MX96.3",
        ),
        (
            "06-pid-tuning-methods/85-tuning-to-requirements",
            "%MX96.1",
            "%MX96.3",
        ),
        (
            "07-classical-stability-and-frequency-design/88-robustness-margins",
            "%MX96.1",
            "%MX96.3",
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
            || project.ends_with("31-tuning-method-comparison")
            || project.ends_with("85-tuning-to-requirements")
            || project.ends_with("88-robustness-margins")
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
