use std::path::Path;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;
fn number(r: &Runtime, a: &str) -> f64 {
    match r.io().read(&IoAddress::parse(a).unwrap()).unwrap() {
        Value::LWord(b) => f64::from_bits(b),
        Value::DWord(b) => f32::from_bits(b) as f64,
        v => panic!("{a}: {v:?}"),
    }
}
fn setup(relative: &str, replacements: &[(&str, &str)]) -> (Runtime, SimulationController) {
    setup_with_events(relative, replacements, "")
}
fn setup_with_events(
    relative: &str,
    replacements: &[(&str, &str)],
    events: &str,
) -> (Runtime, SimulationController) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative);
    let mut source = std::fs::read_to_string(p.join("src/Main.st")).unwrap();
    for (a, b) in replacements {
        source = source.replace(a, b);
    }
    let mut sources = vec![SourceFile::with_path("src/Main.st", source)];
    let library = p.join("libraries/control/src/control.st");
    if library.is_file() {
        sources.push(SourceFile::with_path(
            "Control/control.st",
            std::fs::read_to_string(library).unwrap(),
        ));
    }
    let session = CompileSession::from_sources(sources);
    let mut r = session.build_runtime().unwrap();
    r.apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
        .unwrap();
    static NEXT_CONFIG: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT_CONFIG.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let config = std::env::temp_dir().join(format!(
        "trust-heater-intro-{}-{stamp}-{sequence}.toml",
        std::process::id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&config)
        .unwrap();
    use std::io::Write;
    write!(
        file,
        "{}\n{events}",
        std::fs::read_to_string(p.join("simulation.toml")).unwrap()
    )
    .unwrap();
    drop(file);
    let configuration = SimulationConfig::load(&config);
    std::fs::remove_file(config).unwrap();
    (r, SimulationController::new(configuration.unwrap()))
}

const HOT_TRIAL_PROJECTS: [&str; 3] = [
    "01-feedback-foundations-and-temperature-control/09-setpoint-and-disturbance-response",
    "09-industrial-control-structures/60-loop-diagnosis",
    "06-pid-tuning-methods/28-manual-pi-tuning",
];

#[test]
fn hot_trial_abort_removes_both_commands_and_only_a_fresh_runtime_restores_the_hot_start() {
    for project in HOT_TRIAL_PROJECTS {
        for trial in [0, 1] {
            let replacement = format!("TrialKind : INT := {trial};");
            let source = [("TrialKind : INT := 0;", replacement.as_str())];
            let (mut runtime, mut simulation) = setup(project, &source);
            let debug = runtime.enable_debug();
            for scan in 0..16 {
                simulation
                    .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                    .unwrap();
                debug.enqueue_io_write(
                    IoAddress::parse("%MX112.0").unwrap(),
                    Value::Bool((3..7).contains(&scan)),
                );
                runtime.advance_time(Duration::from_millis(100));
                runtime.execute_cycle().unwrap();
                if scan < 3 {
                    assert_eq!(
                        number(&runtime, "%QL0"),
                        75.0,
                        "{project}: prepared hot holding command"
                    );
                    assert_eq!(number(&runtime, "%IL0"), 35.0);
                } else if scan < 7 {
                    assert!(number(&runtime, "%QL0") > 0.0);
                    assert_eq!(
                        number(&runtime, "%QL8"),
                        if trial == 1 { 30.0 } else { 0.0 }
                    );
                } else {
                    assert_pid_stopped(&runtime, project);
                    if scan > 7 {
                        assert_eq!(
                            number(&runtime, "%IL8"),
                            0.0,
                            "{project}: next native input confirms the applied stop"
                        );
                    }
                }
                simulation
                    .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                    .unwrap();
            }
            // Recompile/recreate both owners, as a fresh Simulator start does.
            // Do not reset selected fields inside the stopped controller.
            let (mut fresh, mut plant) = setup(project, &source);
            for scan in 0..12 {
                plant
                    .apply_pre_cycle(Duration::from_millis(scan * 100), &mut fresh)
                    .unwrap();
                fresh.advance_time(Duration::from_millis(100));
                fresh.execute_cycle().unwrap();
                assert_eq!(
                    number(&fresh, "%IL0"),
                    35.0,
                    "{project}: restored native state and delay history"
                );
                assert_eq!(number(&fresh, "%QL0"), 75.0);
                assert_eq!(number(&fresh, "%IL8"), 75.0);
                assert_eq!(number(&fresh, "%QL8"), 0.0);
                plant
                    .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &fresh)
                    .unwrap();
            }
        }
    }
}

#[test]
fn hot_trial_bad_measurement_latches_stop_and_cannot_be_cleared_by_reenabling_the_experiment() {
    for project in HOT_TRIAL_PROJECTS {
        for bad in [-1.0_f64, 81.0] {
            let (mut runtime, mut simulation) = setup(
                project,
                &[("TrialKind : INT := 0;", "TrialKind : INT := 1;")],
            );
            let debug = runtime.enable_debug();
            simulation
                .apply_pre_cycle(Duration::ZERO, &mut runtime)
                .unwrap();
            debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(true));
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            assert_eq!(number(&runtime, "%QL8"), 30.0);
            for (measured, enabled) in [(bad, true), (35.0, false), (35.0, true)] {
                debug.enqueue_io_write(
                    IoAddress::parse("%IL0").unwrap(),
                    Value::LWord(measured.to_bits()),
                );
                debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(enabled));
                runtime.advance_time(Duration::from_millis(100));
                runtime.execute_cycle().unwrap();
                assert_pid_stopped(&runtime, project);
                let Value::Instance(program) = runtime.storage().get_global("P1").unwrap() else {
                    panic!("P1");
                };
                assert_eq!(
                    runtime.storage().get_instance_var(*program, "SafetyAbort"),
                    Some(&Value::Bool(true)),
                    "{project}: latched input fault"
                );
                let Some(Value::Struct(input)) =
                    runtime.storage().get_instance_var(*program, "Input")
                else {
                    panic!("PID input record");
                };
                assert_eq!(
                    input.field("ProcessValue"),
                    Some(&Value::LReal(35.0)),
                    "{project}: preserve last usable measurement outside bad-input arithmetic"
                );
            }
        }
    }
}

fn assert_pid_stopped(runtime: &Runtime, project: &str) {
    assert_eq!(number(runtime, "%QL0"), 0.0, "{project}: heating stops");
    assert_eq!(number(runtime, "%QL8"), 0.0, "{project}: extraction stops");
    let Value::Instance(program) = runtime.storage().get_global("P1").unwrap() else {
        panic!("P1");
    };
    assert_eq!(
        runtime.storage().get_instance_var(*program, "AutoMode"),
        Some(&Value::Bool(false))
    );
    let Some(Value::Instance(controller)) =
        runtime.storage().get_instance_var(*program, "Controller")
    else {
        panic!("actual Control.PID");
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
        panic!("PID state");
    };
    for field in [
        "Setpoint",
        "SetpointRate",
        "ProcessValue",
        "ProcessValueRate",
        "FeedForward",
        "NominalOutput",
        "SaturatedOutput",
    ] {
        let Some(Value::LReal(value)) = state.field(field) else {
            panic!("{field}: real PID history");
        };
        assert!(value.is_finite(), "{project}: {field} stays finite");
    }
    assert_eq!(state.field("NominalOutput"), Some(&Value::LReal(0.0)));
    assert_eq!(state.field("SaturatedOutput"), Some(&Value::LReal(0.0)));
}

#[test]
fn diagnosis_repeats_the_heater_case_and_keeps_sensor_and_output_evidence_separate() {
    let (mut r, mut sim) = setup_with_events(
        "09-industrial-control-structures/60-loop-diagnosis",
        &[],
        r#"
[[plant_events]]
at_ms = 0
plant = "heater-loop-diagnosis"
kind = "set_sensor_bias"
channel = 0
value = 0.3
"#,
    );
    let debug = r.enable_debug();
    for scan in 0..2 {
        sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(scan > 0));
        r.advance_time(Duration::from_millis(100));
        r.execute_cycle().unwrap();
        if scan == 0 {
            assert_eq!(
                number(&r, "%IL0"),
                35.0,
                "diagnosis must keep the heater, not switch to50t/h flow"
            );
        }
        // The native event reaches the next input snapshot after the plant step.
        if scan > 0 {
            assert!(
                (number(&r, "%ML72") - 0.3).abs() < 1e-9,
                "temperature-reference disagreement remains a separate observation"
            );
        }
        assert_eq!(
            number(&r, "%ML88"),
            0.0,
            "within-range request has no unavailable command"
        );
        if scan == 1 {
            assert!(number(&r, "%ML64") > 0.0);
            assert!(
                number(&r, "%QL0") < 100.0,
                "positive error alone does not mean a capacity limit"
            );
        }
        sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
            .unwrap();
    }
}

#[test]
fn diagnosis_uses_actual_pid_output_and_preserves_accepted_parameters() {
    let (mut runtime, mut simulation) =
        setup("09-industrial-control-structures/60-loop-diagnosis", &[]);
    let debug = runtime.enable_debug();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(true));
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("the delivered diagnosis program");
    };
    let Some(Value::Instance(controller)) =
        runtime.storage().get_instance_var(program, "Controller")
    else {
        panic!("diagnosis must execute the existing Control.PID instance");
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
    assert!(
        (number(&runtime, "%QL0") - 81.15).abs() < 1e-9,
        "preserve the same hot target experiment"
    );
    let mut accepted = runtime
        .storage()
        .get_instance_var(program, "Parameters")
        .unwrap()
        .clone();
    let Value::Struct(ref mut parameters) = accepted else {
        panic!("complete PID parameter record");
    };
    *std::sync::Arc::make_mut(parameters)
        .field_mut("OutputMaximum")
        .unwrap() = Value::LReal(10.0);
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "Parameters", accepted.clone()));
    for _ in 0..3 {
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            runtime.storage().get_instance_var(program, "Parameters"),
            Some(&accepted)
        );
        assert_eq!(
            number(&runtime, "%QL0"),
            10.0,
            "a handwritten output owner must not bypass the actual PID limit"
        );
    }
}

#[test]
fn diagnosis_receives_measured_and_reference_temperatures_through_distinct_native_inputs() {
    let (runtime, _) = setup("09-industrial-control-structures/60-loop-diagnosis", &[]);
    for (symbol, address) in [
        ("P1.TemperatureC", "%IL0"),
        ("P1.WaterTemperatureC", "%IL16"),
        ("P1.AppliedPowerPct", "%IL8"),
    ] {
        let binding = runtime
            .io()
            .bindings()
            .iter()
            .find(|binding| {
                binding
                    .display_name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(symbol))
            })
            .expect("diagnostic observation must have a compiled I/O binding");
        assert_eq!(binding.address, IoAddress::parse(address).unwrap(),
            "the application must observe native measurements instead of summing or biasing simulated paths in ST");
    }
}

#[test]
fn diagnosis_native_sensor_bias_does_not_change_the_reference_or_claim_missing_capacity() {
    let (mut runtime, mut simulation) = setup_with_events(
        "09-industrial-control-structures/60-loop-diagnosis",
        &[],
        r#"
[[plant_events]]
at_ms = 0
plant = "heater-loop-diagnosis"
kind = "set_sensor_bias"
channel = 0
value = 0.3
"#,
    );
    let debug = runtime.enable_debug();
    for scan in 0..3 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(scan > 0));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        if scan > 0 {
            assert!((number(&runtime, "%ML72") - 0.3).abs() < 1e-9,
                "bias on the measured native channel must leave the independent reference unchanged");
            assert!(number(&runtime, "%ML88").abs() < 1e-9,
                "one-scan command/readback timing must not masquerade as unavailable heater capacity");
            assert!(number(&runtime, "%QL0") < 100.0);
        }
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
}

#[test]
fn manual_tuning_updates_one_actual_pid_parameter_without_replacing_the_accepted_record() {
    for (field, value, expected) in [
        ("ProportionalGain", 2.4, 82.35),
        ("IntegralGain", 0.6, 81.18),
    ] {
        let (mut runtime, mut simulation) = setup("06-pid-tuning-methods/28-manual-pi-tuning", &[]);
        let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
            panic!("delivered manual tuning program");
        };
        let Some(Value::Instance(controller)) =
            runtime.storage().get_instance_var(program, "Controller")
        else {
            panic!("manual PI tuning must use the existing Control.PID instance");
        };
        let controller = *controller;
        assert_eq!(
            runtime
                .storage()
                .get_instance(controller)
                .unwrap()
                .type_name
                .as_str(),
            "Control.PID"
        );
        let mut accepted = runtime
            .storage()
            .get_instance_var(program, "Parameters")
            .unwrap()
            .clone();
        let Value::Struct(ref mut record) = accepted else {
            panic!("complete actual PID parameter record");
        };
        *std::sync::Arc::make_mut(record).field_mut(field).unwrap() = Value::LReal(value);
        assert!(runtime
            .storage_mut()
            .set_instance_var(program, "Parameters", accepted.clone()));
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(true));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert!(
            (number(&runtime, "%QL0") - expected).abs() < 1e-8,
            "one-gain prediction through the actual block"
        );
        let Some(Value::Struct(state)) =
            runtime.storage().get_instance_var(controller, "StateValue")
        else {
            panic!("actual PID histories");
        };
        assert_eq!(
            state.field("SaturatedOutput"),
            Some(&Value::LReal(number(&runtime, "%QL0")))
        );
        for _ in 0..3 {
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            assert_eq!(
                runtime.storage().get_instance_var(program, "Parameters"),
                Some(&accepted),
                "cyclic application code must preserve all accepted settings"
            );
        }
    }
}

#[test]
fn manual_tuning_observes_the_native_sensor_and_applied_actuator() {
    let (runtime, _) = setup("06-pid-tuning-methods/28-manual-pi-tuning", &[]);
    for (symbol, address) in [("P1.TemperatureC", "%IL0"), ("P1.AppliedPowerPct", "%IL8")] {
        let binding = runtime
            .io()
            .bindings()
            .iter()
            .find(|binding| {
                binding
                    .display_name
                    .as_deref()
                    .is_some_and(|name| name.eq_ignore_ascii_case(symbol))
            })
            .expect("compiled manual tuning observation");
        assert_eq!(
            binding.address,
            IoAddress::parse(address).unwrap(),
            "normal native plant feedback, without ST plant composition"
        );
    }
}

#[test]
fn manual_tuning_uses_actual_record_gains_one_change_at_a_time_on_the_same_heater() {
    for (kp, ki, expected) in [(2.0, 0.5, 81.15), (2.4, 0.5, 82.35), (2.0, 0.6, 81.18)] {
        let kp_line = format!("ProportionalGain := LREAL#{kp},");
        let ki_line = format!("IntegralGain := LREAL#{ki},");
        let (mut r, mut sim) = setup(
            "06-pid-tuning-methods/28-manual-pi-tuning",
            &[
                ("ProportionalGain := LREAL#2.0,", &kp_line),
                ("IntegralGain := LREAL#0.5,", &ki_line),
            ],
        );
        let debug = r.enable_debug();
        sim.apply_pre_cycle(Duration::ZERO, &mut r).unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(true));
        r.advance_time(Duration::from_millis(100));
        r.execute_cycle().unwrap();
        assert_eq!(
            number(&r, "%IL0"),
            35.0,
            "manual tuning preserves the same hot heater"
        );
        assert!(
            (number(&r, "%QL0") - expected).abs() < 1e-5,
            "declared one-gain change through actual LREAL PID parameters"
        );
        assert!((number(&r, "%ML96") - kp).abs() < 1e-8);
        assert!((number(&r, "%ML104") - ki).abs() < 1e-8);
    }
}

#[test]
fn manual_tuning_inhibits_a_candidate_outside_the_declared_practice_bounds() {
    let (mut r, mut sim) = setup(
        "06-pid-tuning-methods/28-manual-pi-tuning",
        &[(
            "ProportionalGain := LREAL#2.0,",
            "ProportionalGain := LREAL#4.1,",
        )],
    );
    let debug = r.enable_debug();
    sim.apply_pre_cycle(Duration::ZERO, &mut r).unwrap();
    debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(true));
    r.advance_time(Duration::from_millis(100));
    r.execute_cycle().unwrap();
    assert_eq!(
        number(&r, "%QL0"),
        0.0,
        "outside1–4 proportional-gain practice bounds"
    );
}

#[test]
fn process_dynamics_records_the_held_input_after_hot_preparation() {
    for preparation in [0_i64, 100] {
        let (mut r, mut sim) = setup(
            "01-feedback-foundations-and-temperature-control/13-process-response-classes",
            &[],
        );
        let debug = r.enable_debug();
        for scan in 0..preparation + 303 {
            let elapsed = scan - preparation;
            sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
                .unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%MX80.0").unwrap(),
                Value::Bool((0..=300).contains(&elapsed)),
            );
            r.advance_time(Duration::from_millis(100));
            r.execute_cycle().unwrap();
            let pv = number(&r, "%IL0");
            let applied = number(&r, "%QL0");
            if elapsed < 0 {
                assert_eq!(
                    applied, 75.0,
                    "preparation must maintain the supplied hot operating point"
                );
                assert!((pv - 35.0).abs() < 1e-9);
            }
            if (0..300).contains(&elapsed) {
                assert_eq!(applied, 85.0, "Test must hold the prepared input for30s");
            }
            if (0..10).contains(&elapsed) {
                assert!(
                    (pv - 35.0).abs() < 1e-9,
                    "the existing75% history precedes the delayed input"
                );
            }
            if elapsed == 60 {
                assert!((pv - 36.264).abs() < 0.04);
            }
            if elapsed == 260 {
                assert!((pv - 36.986).abs() < 0.01);
            }
            if elapsed >= 300 {
                assert_eq!(
                    applied, 0.0,
                    "completion and subsequent disable remove heating"
                );
            }
            assert!((33.0..=39.0).contains(&pv));
            assert!((0.0..=100.0).contains(&applied));
            sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
                .unwrap();
        }
    }
    let (mut r, mut sim) = setup(
        "01-feedback-foundations-and-temperature-control/13-process-response-classes",
        &[],
    );
    let debug = r.enable_debug();
    sim.apply_pre_cycle(Duration::ZERO, &mut r).unwrap();
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(40.0_f64.to_bits()),
    );
    debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(true));
    r.advance_time(Duration::from_millis(100));
    r.execute_cycle().unwrap();
    assert_eq!(number(&r, "%QL0"), 0.0, "fault removes heating");
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(35.0_f64.to_bits()),
    );
    debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(false));
    r.advance_time(Duration::from_millis(100));
    r.execute_cycle().unwrap();
    assert_eq!(
        number(&r, "%QL0"),
        0.0,
        "disabling after a fault must not resume the75% holding command"
    );
}

#[test]
fn energy_balance_predicts_the_hot_step_with_separate_delayed_model_and_measurement() {
    let (mut r, mut sim) = setup(
        "01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance",
        &[],
    );
    let debug = r.enable_debug();
    for scan in 0..402 {
        let elapsed_scan = scan - 100;
        sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%MX40.2").unwrap(),
            Value::Bool((0..300).contains(&elapsed_scan)),
        );
        r.advance_time(Duration::from_millis(100));
        r.execute_cycle().unwrap();
        if elapsed_scan < 0 {
            assert!(
                (number(&r, "%IL0") - 35.0).abs() < 1e-9,
                "hot physical reset"
            );
            assert!(
                (number(&r, "%MD0") - 35.0).abs() < 1e-5,
                "separate model reset"
            );
        }
        if (0..300).contains(&elapsed_scan) {
            assert_eq!(number(&r, "%QL0"), 85.0);
            assert!(
                (number(&r, "%MD0") - number(&r, "%IL0")).abs() < 0.05,
                "Euler model stays separate from continuous simulated measurement"
            );
        }
        if elapsed_scan == 60 {
            assert!((number(&r, "%IL0") - 36.264).abs() < 0.04);
        }
        if elapsed_scan == 260 {
            assert!((number(&r, "%IL0") - 36.986).abs() < 0.01);
        }
        if elapsed_scan >= 300 {
            assert_eq!(number(&r, "%QL0"), 0.0);
        }
        sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
            .unwrap();
    }
}

#[test]
fn energy_balance_fault_latches_and_fresh_runtime_restores_both_temperature_owners() {
    const PROJECT: &str =
        "01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance";
    for bad_temperature in [31.0_f64, 40.0_f64] {
        let (mut runtime, mut simulation) = setup(PROJECT, &[]);
        let debug = runtime.enable_debug();
        for scan in 0..18 {
            simulation
                .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%MX40.2").unwrap(),
                Value::Bool((3..10).contains(&scan) || scan >= 12),
            );
            if scan >= 8 {
                // A sensor fault reaches the actual input. Restoring its value
                // and toggling authority must not silently reset the ST latch.
                debug.enqueue_io_write(
                    IoAddress::parse("%IL0").unwrap(),
                    Value::LWord(if scan == 8 {
                        bad_temperature.to_bits()
                    } else {
                        35.0_f64.to_bits()
                    }),
                );
            }
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            if scan < 3 {
                assert_eq!(number(&runtime, "%IL0"), 35.0);
                assert_eq!(number(&runtime, "%MD0"), 35.0);
                assert_eq!(number(&runtime, "%QL0"), 75.0);
                assert_eq!(number(&runtime, "%MD16"), 75.0);
            } else if scan < 8 {
                assert_eq!(number(&runtime, "%QL0"), 85.0);
            } else {
                assert_eq!(number(&runtime, "%QL0"), 0.0, "fault removes heat");
                assert_eq!(
                    runtime
                        .io()
                        .read(&IoAddress::parse("%MX40.1").unwrap())
                        .unwrap(),
                    Value::Bool(true),
                    "valid PV and a new enable do not clear a latched fault"
                );
                if scan == 8 {
                    assert_eq!(number(&runtime, "%IL0"), bad_temperature);
                    assert_eq!(
                        number(&runtime, "%MD0"),
                        35.0,
                        "the separate model must not overwrite or copy the sensor fault"
                    );
                }
            }
            simulation
                .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
                .unwrap();
        }

        let (mut fresh, mut plant) = setup(PROJECT, &[]);
        for scan in 0..12 {
            plant
                .apply_pre_cycle(Duration::from_millis(scan * 100), &mut fresh)
                .unwrap();
            fresh.advance_time(Duration::from_millis(100));
            fresh.execute_cycle().unwrap();
            assert_eq!(number(&fresh, "%IL0"), 35.0, "native plant resets");
            assert_eq!(number(&fresh, "%MD0"), 35.0, "ST model resets");
            assert_eq!(number(&fresh, "%QL0"), 75.0, "hot holding command");
            assert_eq!(number(&fresh, "%MD16"), 75.0, "full delay history resets");
            assert_eq!(
                fresh
                    .io()
                    .read(&IoAddress::parse("%MX40.1").unwrap())
                    .unwrap(),
                Value::Bool(false)
            );
            assert_eq!(
                fresh
                    .io()
                    .read(&IoAddress::parse("%MX40.2").unwrap())
                    .unwrap(),
                Value::Bool(false)
            );
            plant
                .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &fresh)
                .unwrap();
        }
    }
}

#[test]
fn identification_experiment_requires_a_real_five_second_baseline_before_motion() {
    let (mut r, mut sim) = setup(
        "05-experiments-identification-and-validation/21-safe-informative-experiment",
        &[],
    );
    let debug = r.enable_debug();
    for scan in 0..362 {
        sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%MX64.0").unwrap(),
            Value::Bool(scan == 0 || scan >= 60),
        );
        r.advance_time(Duration::from_millis(100));
        r.execute_cycle().unwrap();
        if scan == 0 {
            assert_eq!(number(&r, "%IL0"), 35.0, "same hot heater");
            assert_eq!(
                number(&r, "%QL0"),
                75.0,
                "early confirmation cannot skip the baseline"
            );
            assert_eq!(
                r.io().read(&IoAddress::parse("%MX64.4").unwrap()).unwrap(),
                Value::Bool(true)
            );
        }
        if scan == 59 {
            assert_eq!(
                r.io().read(&IoAddress::parse("%MX64.2").unwrap()).unwrap(),
                Value::Bool(true)
            );
        }
        if (60..360).contains(&scan) {
            assert_eq!(number(&r, "%QL0"), 85.0, "qualified10point step for30s");
        }
        if scan >= 360 {
            assert_eq!(
                number(&r, "%QL0"),
                75.0,
                "normal completion restores qualified baseline command"
            );
        }
        sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
            .unwrap();
    }
}

#[test]
fn identification_experiment_fault_aborts_to_zero_without_automatic_restore() {
    let (mut r, mut sim) = setup(
        "05-experiments-identification-and-validation/21-safe-informative-experiment",
        &[],
    );
    let debug = r.enable_debug();
    sim.apply_pre_cycle(Duration::ZERO, &mut r).unwrap();
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(40.0_f64.to_bits()),
    );
    r.advance_time(Duration::from_millis(100));
    r.execute_cycle().unwrap();
    assert_eq!(
        number(&r, "%QL0"),
        0.0,
        "PV outside33–39°C requires zero heat"
    );
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(35.0_f64.to_bits()),
    );
    r.advance_time(Duration::from_millis(100));
    r.execute_cycle().unwrap();
    assert_eq!(
        number(&r, "%QL0"),
        0.0,
        "valid PV alone cannot automatically restore75%"
    );
}

#[test]
fn identification_experiment_rejects_spread_and_bad_quality_in_actual_samples() {
    for bad_quality in [false, true] {
        let (mut r, mut sim) = setup(
            "05-experiments-identification-and-validation/21-safe-informative-experiment",
            &[],
        );
        let debug = r.enable_debug();
        for scan in 0..61 {
            sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
                .unwrap();
            let pv = if bad_quality {
                35.0_f64
            } else if scan % 2 == 0 {
                35.04
            } else {
                34.96
            };
            debug.enqueue_io_write(
                IoAddress::parse("%IL0").unwrap(),
                Value::LWord(pv.to_bits()),
            );
            if bad_quality {
                debug.enqueue_io_write(IoAddress::parse("%IW16").unwrap(), Value::Word(0));
            }
            debug.enqueue_io_write(
                IoAddress::parse("%MX64.0").unwrap(),
                Value::Bool(scan == 60),
            );
            r.advance_time(Duration::from_millis(100));
            r.execute_cycle().unwrap();
            if scan == 60 {
                assert_eq!(
                    r.io().read(&IoAddress::parse("%MX64.2").unwrap()).unwrap(),
                    Value::Bool(false)
                );
                assert_eq!(number(&r, "%QL0"), if bad_quality { 0.0 } else { 75.0 });
            }
            sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
                .unwrap();
        }
    }
}

#[test]
fn diagnosis_biased_target_trial_holds_the_reading_on_target_and_the_water_cold() {
    let mut trace = String::from(
        "case,time_s,pv_degC,reference_degC,requested_percent,applied_percent,integral_percent\n",
    );
    for bias in [0.0_f64, 0.3] {
        let (mut r, mut sim) = setup_with_events(
            "09-industrial-control-structures/60-loop-diagnosis",
            &[],
            &format!(
                "\n[[plant_events]]\nat_ms = 0\nplant = \"heater-loop-diagnosis\"\nkind = \"set_sensor_bias\"\nchannel = 0\nvalue = {bias:.1}\n"
            ),
        );
        let debug = r.enable_debug();
        let mut end = [0.0_f64; 3];
        for scan in 0..702 {
            let elapsed_scan = scan - 100;
            sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
                .unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%MX112.0").unwrap(),
                Value::Bool((0..600).contains(&elapsed_scan)),
            );
            r.advance_time(Duration::from_millis(100));
            r.execute_cycle().unwrap();
            if (0..600).contains(&elapsed_scan) {
                trace.push_str(&format!(
                    "{},{:.1},{:.12},{:.12},{:.12},{:.12},{:.12}\n",
                    if bias == 0.0 { "unbiased" } else { "biased" },
                    elapsed_scan as f64 * 0.1,
                    number(&r, "%IL0"),
                    number(&r, "%IL16"),
                    number(&r, "%ML32"),
                    number(&r, "%QL0"),
                    number(&r, "%ML16"),
                ));
                end = [number(&r, "%IL0"), number(&r, "%IL16"), number(&r, "%QL0")];
            }
            sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
                .unwrap();
        }
        eprintln!(
            "bias={bias}: end measured={:.4} reference={:.4} output={:.4}",
            end[0], end[1], end[2]
        );
        assert!(
            (end[0] - 38.0).abs() < 0.02,
            "the reading is held on target"
        );
        assert!(
            (end[1] - (38.0 - bias)).abs() < 0.02,
            "the water is cold by the bias"
        );
        assert!(
            (end[2] - (90.0 - 5.0 * bias)).abs() < 0.15,
            "the output supplies the power for the water's actual temperature"
        );
    }
    if let Ok(path) = std::env::var("TRUST_LESSON10_TRACE") {
        std::fs::write(path, trace).expect("retain executed simulator trace");
    }
}
