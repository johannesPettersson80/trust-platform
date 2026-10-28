use std::path::Path;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::{bound_address, compile, exists, type_name, value, LessonRuntime};

fn process_value(runtime: &CompiledRuntime) -> f64 {
    let program = "P1";
    assert!(exists(runtime, program), "P1 program");
    let Some(Value::LReal(value)) = value(runtime, &format!("{program}.ProcessValue")) else {
        panic!("measured ProcessValue");
    };
    value
}
fn number(r: &CompiledRuntime, a: &str) -> f64 {
    match r.read_io(&IoAddress::parse(a).unwrap()).unwrap() {
        Value::LWord(b) => f64::from_bits(b),
        Value::LReal(v) => v,
        v => panic!("{a}: {v:?}"),
    }
}
fn setup(folder: &str) -> (CompiledRuntime, SimulationController) {
    setup_source(folder, |source| source)
}
fn setup_source(
    folder: &str,
    source: impl FnOnce(String) -> String,
) -> (CompiledRuntime, SimulationController) {
    let p = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control")
        .join(folder);
    let session = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "src/Main.st",
            source(std::fs::read_to_string(p.join("src/Main.st")).unwrap()),
        ),
        SourceFile::with_path(
            "Control/control.st",
            std::fs::read_to_string(p.join("libraries/control/src/control.st")).unwrap(),
        ),
    ]);
    let r = compile(session);
    (
        r,
        SimulationController::new(SimulationConfig::load(p.join("simulation.toml")).unwrap()),
    )
}

#[test]
fn heater_p_and_pi_read_the_single_native_sensor_input() {
    for folder in ["04-water-heater-p", "05-water-heater-pi"] {
        let (runtime, _) = setup(folder);
        let binding = bound_address(&runtime, "P1.ProcessValue")
            .expect("the actual ProcessValue must have a compiled I/O binding");
        assert_eq!(binding, IoAddress::parse("%IL0").unwrap(),
            "{folder}: ProcessValue must read the normal sensor input, not an ST-composed simulation value");
    }
}

#[test]
fn proportional_heater_output_is_owned_by_the_actual_control_pid_instance() {
    let (mut runtime, _) = setup("04-water-heater-p");
    let debug = runtime.enable_debug();
    for (enabled, temperature, expected_output, expected_nominal) in [
        (true, 20.0_f64, 30.0, 30.0),
        (true, 30.0, 10.0, 10.0),
        (true, 35.0, 0.0, 0.0),
        (true, 36.0, 0.0, -2.0),
        (false, 20.0, 0.0, 0.0),
    ] {
        debug.enqueue_io_write(IoAddress::parse("%MX24.1").unwrap(), Value::Bool(enabled));
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(temperature.to_bits()),
        );
        debug.enqueue_io_write(IoAddress::parse("%IL8").unwrap(), Value::LWord(0));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();

        let program = "P1";
        assert!(
            exists(&runtime, program),
            "the delivered lesson must execute its P1 program"
        );
        let controller = format!("{program}.Controller");
        assert!(
            exists(&runtime, &controller),
            "the delivered P lesson must execute a real Control.PID instance"
        );
        assert_eq!(
            type_name(&runtime, &controller).unwrap().as_str(),
            "Control.PID"
        );
        let Some(Value::Struct(state)) = value(&runtime, &format!("{controller}.StateValue"))
        else {
            panic!("the real PID must retain its executed controller history");
        };
        assert_eq!(
            state.field("ProcessValue"),
            Some(&Value::LReal(temperature))
        );
        assert_eq!(
            state.field("NominalOutput"),
            Some(&Value::LReal(expected_nominal))
        );
        assert_eq!(
            state.field("SaturatedOutput"),
            Some(&Value::LReal(expected_output))
        );
        assert_eq!(number(&runtime, "%QL0"), expected_output);
    }

    // Change only the actual block's configured limit. A parallel handwritten
    // output assignment would still apply its 30% diagnostic request here.
    let program = "P1";
    let mut parameters = value(&runtime, &format!("{program}.Parameters")).unwrap();
    let Value::Struct(ref mut record) = parameters else {
        panic!("the application must configure the complete PID parameter record");
    };
    for (field, expected) in [
        ("ProportionalGain", 2.0),
        ("IntegralGain", 0.0),
        ("DerivativeGain", 0.0),
        ("Bias", 0.0),
    ] {
        assert_eq!(record.field(field), Some(&Value::LReal(expected)));
    }
    *std::sync::Arc::make_mut(record)
        .field_mut("OutputMaximum")
        .unwrap() = Value::LReal(10.0);
    assert!(runtime
        .write_instance(program, "Parameters", parameters)
        .is_ok());
    debug.enqueue_io_write(IoAddress::parse("%MX24.1").unwrap(), Value::Bool(true));
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        number(&runtime, "%ML40"),
        30.0,
        "retain the unlimited diagnostic request"
    );
    assert_eq!(
        number(&runtime, "%QL0"),
        10.0,
        "Control.PID's returned limit must own the actuator output"
    );
}

#[test]
fn proportional_heater_has_zero_bias_and_retains_the_physical_steady_state_error() {
    let mut trace = String::from(
        "time_s,setpoint_degC,pv_degC,requested_percent,applied_percent,extraction_kw\n",
    );
    for preparation in [0, 100] {
        let (mut r, mut sim) = setup("04-water-heater-p");
        let debug = r.enable_debug();
        for scan in 0..preparation + 1202 {
            let elapsed_scan = scan - preparation;
            sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
                .unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%MX24.1").unwrap(),
                Value::Bool((0..1200).contains(&elapsed_scan)),
            );
            r.advance_time(Duration::from_millis(100));
            r.execute_cycle().unwrap();
            if preparation == 0 && (0..1200).contains(&elapsed_scan) {
                trace.push_str(&format!(
                    "{:.1},{},{:.12},{:.12},{:.12},{}\n",
                    elapsed_scan as f64 * 0.1,
                    number(&r, "%ML0"),
                    process_value(&r),
                    number(&r, "%ML40"),
                    number(&r, "%QL0"),
                    number(&r, "%QL8"),
                ));
            }
            if elapsed_scan == 0 {
                assert_eq!(
                    number(&r, "%IL0"),
                    20.0,
                    "P lesson must use the20°C heater, not a50cm tank"
                );
                assert_eq!(number(&r, "%QL0"), 30.0, "zero-biasKp2 times15°C error");
            }
            if elapsed_scan == 590 {
                assert!(
                    (process_value(&r) - 24.2857142857).abs() < 0.01,
                    "P retains the predicted heater error"
                );
            }
            if elapsed_scan == 1199 {
                assert!(
                    (process_value(&r) - 22.1428571429).abs() < 0.01,
                    "same30kW load at experiment60s"
                );
            }
            if elapsed_scan >= 1200 {
                assert_eq!(number(&r, "%QL0"), 0.0);
            }
            sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
                .unwrap();
        }
    }
    if let Ok(path) = std::env::var("TRUST_LESSON5_TRACE") {
        std::fs::write(path, trace).expect("retain executed simulator trace");
    }
}

#[test]
fn integral_action_removes_the_same_heater_error_with_an_experiment_relative_load() {
    let mut trace = String::from(
        "time_s,setpoint_degC,pv_degC,proportional_percent,integral_percent,applied_percent,extraction_kw\n",
    );
    for preparation in [0, 100] {
        let (mut r, mut sim) = setup("05-water-heater-pi");
        let debug = r.enable_debug();
        for scan in 0..preparation + 1202 {
            let elapsed_scan = scan - preparation;
            sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
                .unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%MX80.0").unwrap(),
                Value::Bool((0..1200).contains(&elapsed_scan)),
            );
            r.advance_time(Duration::from_millis(100));
            r.execute_cycle().unwrap();
            if preparation == 0 && (0..1200).contains(&elapsed_scan) {
                trace.push_str(&format!(
                    "{:.1},{},{:.12},{:.12},{:.12},{:.12},{}\n",
                    elapsed_scan as f64 * 0.1,
                    number(&r, "%ML0"),
                    process_value(&r),
                    number(&r, "%ML40"),
                    number(&r, "%ML56"),
                    number(&r, "%QL0"),
                    number(&r, "%QL8"),
                ));
            }
            if elapsed_scan == 0 {
                assert_eq!(number(&r, "%IL0"), 20.0);
                assert_eq!(number(&r, "%QL0"), 30.75);
            }
            if elapsed_scan == 610 {
                assert_eq!(
                    number(&r, "%QL8"),
                    30.0,
                    "the actual heat-extraction event must be published after relative60s"
                );
            }
            if (0..1200).contains(&elapsed_scan) {
                let nominal = number(&r, "%ML32");
                assert!(
                    (0.0..=100.0).contains(&nominal),
                    "the PI warming/load comparison must keep controller limits inactive"
                );
                assert_eq!(
                    nominal,
                    number(&r, "%QL0"),
                    "retained nominal and applied output agree throughout this comparison"
                );
            }
            if (500..600).contains(&elapsed_scan) || (1100..1200).contains(&elapsed_scan) {
                assert!(
                    (34.8..=35.2).contains(&process_value(&r)),
                    "PI must remove the same P error in both final10s windows"
                );
            }
            if elapsed_scan >= 1200 {
                assert_eq!(number(&r, "%QL0"), 0.0);
                assert_eq!(number(&r, "%ML56"), 0.0);
            }
            sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
                .unwrap();
        }
    }
    if let Ok(path) = std::env::var("TRUST_LESSON6_TRACE") {
        std::fs::write(path, trace).expect("retain executed simulator trace");
    }
}

#[test]
fn integral_action_uses_actual_control_pid_and_retains_its_output_state() {
    let (mut runtime, _) = setup("05-water-heater-pi");
    let debug = runtime.enable_debug();
    for (enabled, temperature, expected_output, expected_integral) in [
        (false, 20.0_f64, 0.0, 0.0),
        (false, 20.0, 0.0, 0.0),
        (true, 20.0, 30.75, 0.75),
        (true, 20.0, 31.5, 1.5),
        (true, 35.0, 1.5, 1.5),
    ] {
        debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(enabled));
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(temperature.to_bits()),
        );
        debug.enqueue_io_write(IoAddress::parse("%IL8").unwrap(), Value::LWord(0));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        if !enabled {
            assert_eq!(number(&runtime, "%QL0"), expected_output);
            assert_eq!(number(&runtime, "%ML56"), expected_integral);
            continue;
        }
        let program = "P1";
        assert!(
            exists(&runtime, program),
            "the delivered PI lesson must execute its P1 program"
        );
        let controller = format!("{program}.Controller");
        assert!(
            exists(&runtime, &controller),
            "the delivered PI lesson must execute a real Control.PID instance"
        );
        assert_eq!(
            type_name(&runtime, &controller).unwrap().as_str(),
            "Control.PID"
        );
        let Some(Value::Struct(state)) = value(&runtime, &format!("{controller}.StateValue"))
        else {
            panic!("the real PI block must retain its nominal output history");
        };
        assert_eq!(
            state.field("ProcessValue"),
            Some(&Value::LReal(temperature))
        );
        assert_eq!(
            state.field("NominalOutput"),
            Some(&Value::LReal(expected_output))
        );
        assert_eq!(
            state.field("SaturatedOutput"),
            Some(&Value::LReal(expected_output))
        );
        assert_eq!(number(&runtime, "%QL0"), expected_output);
        assert_eq!(number(&runtime, "%ML56"), expected_integral);
    }

    let program = "P1";
    let mut parameters = value(&runtime, &format!("{program}.Parameters")).unwrap();
    let Value::Struct(ref mut record) = parameters else {
        panic!("the PI application must configure the complete PID parameter record");
    };
    for (field, expected) in [
        ("ProportionalGain", 2.0),
        ("IntegralGain", 0.5),
        ("DerivativeGain", 0.0),
        ("ProportionalSetpointWeight", 1.0),
        ("Bias", 0.0),
    ] {
        assert_eq!(record.field(field), Some(&Value::LReal(expected)));
    }
    *std::sync::Arc::make_mut(record)
        .field_mut("OutputMaximum")
        .unwrap() = Value::LReal(10.0);
    assert!(runtime
        .write_instance(program, "Parameters", parameters)
        .is_ok());
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(20.0_f64.to_bits()),
    );
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    let controller = format!("{program}.Controller");
    let Some(Value::Struct(state)) = value(&runtime, &format!("{controller}.StateValue")) else {
        unreachable!();
    };
    let Some(Value::LReal(nominal)) = state.field("NominalOutput") else {
        unreachable!();
    };
    assert!(
        (*nominal - 30.025).abs() < 1e-9,
        "the real block must back-calculate from its own output limit"
    );
    assert!(
        (number(&runtime, "%ML56") - (*nominal - 30.0)).abs() < 1e-9,
        "plotted I must derive from the actual retained nominal state"
    );
    assert_eq!(
        number(&runtime, "%QL0"),
        10.0,
        "the real PI return value must own the actuator output"
    );
}

fn hot_windup_trial(
    folder: &str,
    activation: &str,
    declared_limits_match: bool,
    preparation: i64,
) -> f64 {
    let (mut r, mut sim) = setup(folder);
    let debug = r.enable_debug();
    let mut largest_request = 0.0_f64;
    let mut return_integral = 0.0;
    for scan in 0..preparation + 702 {
        let experiment_scan = scan - preparation;
        sim.apply_pre_cycle(Duration::from_millis(scan * 100), &mut r)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse(activation).unwrap(),
            Value::Bool((0..700).contains(&experiment_scan)),
        );
        r.advance_time(Duration::from_millis(100));
        r.execute_cycle().unwrap();
        if experiment_scan < 0 {
            assert!(
                (number(&r, "%IL0") - 35.0).abs() < 1e-9,
                "hot preparation must hold35°C"
            );
            assert_eq!(number(&r, "%QL0"), 75.0);
        }
        if experiment_scan == 0 {
            assert!(
                (number(&r, "%ML8") - 95.5).abs() < 1e-9,
                "I75 plus P20 plus first.5point increment"
            );
        }
        if (0..700).contains(&experiment_scan) {
            assert_eq!(
                number(&r, "%ML0"),
                if experiment_scan < 100 { 45.0 } else { 35.0 }
            );
            let requested = number(&r, "%ML8");
            let applied = number(&r, "%IL8");
            assert!((0.0..=100.0).contains(&applied));
            largest_request = largest_request.max(requested);
            if declared_limits_match {
                let controller_output = number(&r, "%QL0");
                assert!(
                    (number(&r, "%ML80") - 0.1 * (controller_output - requested)).abs() < 1e-9,
                    "the PID correction uses its current declared limit, not the preceding applied readback"
                );
            }
            if experiment_scan == 99 {
                return_integral = number(&r, "%ML64");
            }
        }
        if experiment_scan >= 700 {
            assert_eq!(number(&r, "%QL0"), 0.0);
        }
        if experiment_scan >= 701 {
            assert_eq!(number(&r, "%IL8"), 0.0);
        }
        sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
            .unwrap();
        assert!((0.0..=100.0).contains(&sim.status().plants[0].applied_input[0]));
    }
    assert!(
        largest_request > 100.0,
        "the declared infeasible target must reveal request saturation"
    );
    return_integral
}

#[test]
fn windup_lesson_starts_hot_and_runs_the_declared_seventy_second_experiment() {
    for preparation in [0, 100] {
        assert!(
            hot_windup_trial(
                "06-water-heater-saturation-windup",
                "%MX96.0",
                false,
                preparation
            ) > 100.0
        );
    }
}

#[test]
fn antiwindup_repeats_the_same_hot_trial_and_reduces_excess_integral() {
    for preparation in [0, 100] {
        let untracked = hot_windup_trial(
            "06-water-heater-saturation-windup",
            "%MX96.0",
            false,
            preparation,
        );
        let tracked =
            hot_windup_trial("07-water-heater-anti-windup", "%MX112.0", true, preparation);
        assert!(
            tracked < untracked,
            "the accurate declared limit must reduce retained excess at the same 10 s return"
        );
    }
}

#[test]
fn setpoint_and_load_lesson_uses_two_independent_sixty_second_heater_trials() {
    let mut trace = String::from(
        "trial,time_s,setpoint_degC,pv_degC,proportional_percent,integral_percent,requested_percent,applied_percent,extraction_kw\n",
    );
    for trial in [0_u16, 1] {
        let (mut r, mut sim) = setup_source("09-setpoint-and-disturbance-response", |source| {
            source.replace(
                "TrialKind : INT := 0;",
                &format!("TrialKind : INT := {trial};"),
            )
        });
        let debug = r.enable_debug();
        let mut maximum_late_error = 0.0_f64;
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
            if elapsed_scan < 0 {
                assert!((number(&r, "%IL0") - 35.0).abs() < 1e-9, "common hot reset");
                assert_eq!(number(&r, "%QL0"), 75.0);
            }
            if elapsed_scan == 0 {
                assert!(
                    (number(&r, "%QL0") - if trial == 0 { 81.15 } else { 75.0 }).abs() < 1e-9,
                    "target changes error immediately; unmeasured extraction does not"
                );
            }
            if (0..600).contains(&elapsed_scan) {
                trace.push_str(&format!(
                    "{},{:.1},{},{:.12},{:.12},{:.12},{:.12},{:.12},{}\n",
                    if trial == 0 { "setpoint" } else { "load" },
                    elapsed_scan as f64 * 0.1,
                    number(&r, "%ML0"),
                    number(&r, "%IL0"),
                    number(&r, "%ML56"),
                    number(&r, "%ML16"),
                    number(&r, "%ML32"),
                    number(&r, "%QL0"),
                    number(&r, "%QL8"),
                ));
                assert_eq!(number(&r, "%ML0"), if trial == 0 { 38.0 } else { 35.0 });
                assert_eq!(number(&r, "%QL8"), if trial == 0 { 0.0 } else { 30.0 });
                let pv = number(&r, "%IL0");
                assert!(if trial == 0 { pv <= 39.0 } else { pv >= 33.0 });
                if elapsed_scan >= 300 {
                    maximum_late_error =
                        maximum_late_error.max((pv - if trial == 0 { 38.0 } else { 35.0 }).abs());
                }
            }
            if elapsed_scan >= 600 {
                assert_eq!(number(&r, "%QL0"), 0.0);
                assert_eq!(number(&r, "%QL8"), 0.0);
            }
            sim.apply_post_cycle(Duration::from_millis((scan + 1) * 100), &r)
                .unwrap();
        }
        eprintln!(
            "trial={trial}: maximum30to60sError={maximum_late_error:.8} degC, meets0.2Criterion={}",
            maximum_late_error <= 0.2
        );
        assert_eq!(maximum_late_error<=0.2,trial==0,"retain the measured failed load requirement for diagnosis without retuning the baseline");
    }
    if let Ok(path) = std::env::var("TRUST_LESSON9_TRACE") {
        std::fs::write(path, trace).expect("retain executed simulator trace");
    }
}

#[test]
fn response_trial_is_owned_by_actual_control_pid_and_preserves_accepted_limits() {
    let (mut runtime, mut simulation) = setup("09-setpoint-and-disturbance-response");
    let debug = runtime.enable_debug();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(true));
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    let program = "P1";
    assert!(exists(&runtime, program), "delivered lesson program");
    let controller = format!("{program}.Controller");
    assert!(
        exists(&runtime, &controller),
        "lesson 9 must execute the actual Control.PID instance"
    );
    assert_eq!(
        type_name(&runtime, &controller).unwrap().as_str(),
        "Control.PID"
    );
    let Some(Value::Struct(state)) = value(&runtime, &format!("{controller}.StateValue")) else {
        panic!("actual PID histories");
    };
    assert_eq!(state.field("ProcessValue"), Some(&Value::LReal(35.0)));
    assert!((number(&runtime, "%QL0") - 81.15).abs() < 1e-9);
    let Some(Value::LReal(saturated)) = state.field("SaturatedOutput") else {
        panic!("actual output state");
    };
    assert_eq!(number(&runtime, "%QL0"), *saturated);

    // Only the accepted library parameter changes. A handwritten parallel
    // output owner would ignore this limit and continue applying about 81%.
    let mut accepted = value(&runtime, &format!("{program}.Parameters")).unwrap();
    let Value::Struct(ref mut fields) = accepted else {
        panic!("complete accepted PID record");
    };
    *std::sync::Arc::make_mut(fields)
        .field_mut("OutputMaximum")
        .unwrap() = Value::LReal(10.0);
    assert!(runtime
        .write_instance(program, "Parameters", accepted.clone())
        .is_ok());
    for _ in 0..3 {
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            value(&runtime, &format!("{program}.Parameters")),
            Some(accepted.clone())
        );
        assert_eq!(
            number(&runtime, "%QL0"),
            10.0,
            "only actual PID output owns the heater command"
        );
    }
    debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(false));
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "termination uses the actual Manual branch"
    );
    assert_eq!(
        number(&runtime, "%QL8"),
        0.0,
        "termination removes extraction too"
    );
}

#[test]
fn response_trial_reads_one_native_temperature_sensor_with_independent_input_delays() {
    let (mut runtime, mut simulation) = setup("09-setpoint-and-disturbance-response");
    let binding = bound_address(&runtime, "P1.TemperatureC")
        .expect("actual temperature must have a compiled I/O binding");
    assert_eq!(
        binding,
        IoAddress::parse("%IL0").unwrap(),
        "lesson 9 must receive one physical sensor value, not sum simulated paths inside ST"
    );
    for scan in 0..20 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert!(
            (number(&runtime, "%IL0") - 35.0).abs() < 1e-9,
            "native hot sensor history"
        );
        assert_eq!(number(&runtime, "%QL0"), 75.0);
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
}

#[test]
fn response_trial_uses_the_published_source_initializer_after_a_fresh_start() {
    let (mut runtime, mut simulation) =
        setup_source("09-setpoint-and-disturbance-response", |source| {
            source.replace("TrialKind : INT := 0;", "TrialKind : INT := 1;")
        });
    let debug = runtime.enable_debug();
    let program = "P1";
    assert!(exists(&runtime, program), "P1 program");
    assert_eq!(
        value(&runtime, &format!("{program}.TrialKind")),
        Some(Value::Int(1)),
        "compiled saved initializer"
    );
    for scan in 0..12 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%MX112.0").unwrap(),
            Value::Bool(scan >= 10),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(value(&runtime, &format!("{program}.TrialKind")), Some(Value::Int(1)),
            "fresh scans must preserve the published source-selected load trial without a hidden memory write");
        assert_eq!(number(&runtime, "%ML0"), 35.0);
        assert_eq!(
            number(&runtime, "%QL8"),
            if scan >= 10 { 30.0 } else { 0.0 }
        );
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
}

#[test]
fn response_trial_latches_out_of_range_temperature_and_stops_both_commands() {
    for bad_temperature in [-1.0_f64, 81.0] {
        let (mut runtime, mut simulation) =
            setup_source("09-setpoint-and-disturbance-response", |source| {
                source.replace("TrialKind : INT := 0;", "TrialKind : INT := 1;")
            });
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(true));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(number(&runtime, "%QL8"), 30.0);
        let program = "P1";
        assert!(exists(&runtime, program), "P1");
        for measured in [bad_temperature, 35.0] {
            debug.enqueue_io_write(
                IoAddress::parse("%IL0").unwrap(),
                Value::LWord(measured.to_bits()),
            );
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            assert_eq!(
                number(&runtime, "%QL0"),
                0.0,
                "bad measurement or latched fault removes heating"
            );
            assert_eq!(
                number(&runtime, "%QL8"),
                0.0,
                "bad measurement or latched fault removes extraction"
            );
            assert_eq!(
                value(&runtime, &format!("{program}.SafetyAbort")),
                Some(Value::Bool(true))
            );
            let controller = format!("{program}.Controller");
            assert!(exists(&runtime, &controller), "actual PID");
            let Some(Value::Struct(state)) = value(&runtime, &format!("{controller}.StateValue"))
            else {
                panic!("PID histories");
            };
            assert_eq!(
                state.field("ProcessValue"),
                Some(&Value::LReal(35.0)),
                "keep the last usable measurement out of the faulted arithmetic"
            );
            assert_eq!(state.field("SaturatedOutput"), Some(&Value::LReal(0.0)));
            assert_eq!(state.field("NominalOutput"), Some(&Value::LReal(0.0)));
        }
    }
}

fn configured_io_address(folder: &str, symbol: &str) -> IoAddress {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control")
        .join(folder);
    let source = std::fs::read_to_string(root.join("src/Main.st")).unwrap();
    let needle = format!("P1.{symbol} AT ");
    let line = source
        .lines()
        .find(|line| line.contains(&needle))
        .unwrap_or_else(|| panic!("missing {symbol} binding in {folder}"));
    IoAddress::parse(line.split_whitespace().nth(2).unwrap()).unwrap()
}

fn program_flag(runtime: &CompiledRuntime, name: &str) -> bool {
    let program = "P1";
    assert!(exists(runtime, program), "P1 program");
    match value(runtime, &format!("{program}.{name}")) {
        Some(Value::Bool(value)) => value,
        value => panic!("{name}: {value:?}"),
    }
}

#[test]
fn heater_lessons_five_to_eight_disable_and_abort_remove_command_and_feedback() {
    for (folder, feedback) in [
        ("04-water-heater-p", None),
        ("05-water-heater-pi", None),
        ("06-water-heater-saturation-windup", Some("%IL8")),
        ("07-water-heater-anti-windup", Some("%IL8")),
    ] {
        let activation = configured_io_address(folder, "ExperimentEnabled");
        let (mut runtime, mut simulation) = setup(folder);
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        debug.enqueue_io_write(activation.clone(), Value::Bool(true));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        simulation
            .apply_post_cycle(Duration::from_millis(100), &runtime)
            .unwrap();
        assert!(
            simulation.status().plants[0].actuators[0].applied > 0.0,
            "{folder}: heater was actually active before disable"
        );
        simulation
            .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(activation.clone(), Value::Bool(false));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            number(&runtime, "%QL0"),
            0.0,
            "{folder}: disable removes the command"
        );
        simulation
            .apply_post_cycle(Duration::from_millis(200), &runtime)
            .unwrap();
        assert_eq!(
            simulation.status().plants[0].actuators[0].applied,
            0.0,
            "{folder}: actual native actuator stops"
        );
        simulation
            .apply_pre_cycle(Duration::from_millis(200), &mut runtime)
            .unwrap();
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        if let Some(address) = feedback {
            assert_eq!(
                number(&runtime, address),
                0.0,
                "{folder}: native applied feedback follows zero command"
            );
        }

        let (mut runtime, mut simulation) = setup(folder);
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        debug.enqueue_io_write(activation, Value::Bool(true));
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        simulation
            .apply_post_cycle(Duration::from_millis(100), &runtime)
            .unwrap();
        assert!(
            simulation.status().plants[0].actuators[0].applied > 0.0,
            "{folder}: heater was actually active before bad input"
        );
        simulation
            .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord((-1.0_f64).to_bits()),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert!(
            program_flag(&runtime, "SafetyAbort"),
            "{folder}: bad PV latches SafetyAbort"
        );
        assert_eq!(
            number(&runtime, "%QL0"),
            0.0,
            "{folder}: abort removes command"
        );
        simulation
            .apply_post_cycle(Duration::from_millis(200), &runtime)
            .unwrap();
        assert_eq!(
            simulation.status().plants[0].actuators[0].applied,
            0.0,
            "{folder}: actual native actuator stops"
        );
        simulation
            .apply_pre_cycle(Duration::from_millis(200), &mut runtime)
            .unwrap();
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        if let Some(address) = feedback {
            assert_eq!(
                number(&runtime, address),
                0.0,
                "{folder}: abort reaches native applied feedback"
            );
        }
    }
}

#[test]
fn heater_lessons_five_to_eight_latched_fault_ignores_valid_pv_and_disable_reenable() {
    for folder in [
        "04-water-heater-p",
        "05-water-heater-pi",
        "06-water-heater-saturation-windup",
        "07-water-heater-anti-windup",
    ] {
        let activation = configured_io_address(folder, "ExperimentEnabled");
        for bad in [-1.0_f64, 81.0] {
            let (mut runtime, mut simulation) = setup(folder);
            simulation
                .apply_pre_cycle(Duration::ZERO, &mut runtime)
                .unwrap();
            let debug = runtime.enable_debug();
            debug.enqueue_io_write(activation.clone(), Value::Bool(true));
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            debug.enqueue_io_write(
                IoAddress::parse("%IL0").unwrap(),
                Value::LWord(bad.to_bits()),
            );
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            assert!(
                program_flag(&runtime, "SafetyAbort"),
                "{folder}: {bad} latches the fault"
            );
            debug.enqueue_io_write(
                IoAddress::parse("%IL0").unwrap(),
                Value::LWord(35.0_f64.to_bits()),
            );
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            assert!(
                program_flag(&runtime, "SafetyAbort"),
                "{folder}: valid PV cannot clear a latched fault"
            );
            assert_eq!(
                number(&runtime, "%QL0"),
                0.0,
                "{folder}: valid PV cannot resume output"
            );
            debug.enqueue_io_write(activation.clone(), Value::Bool(false));
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            debug.enqueue_io_write(activation.clone(), Value::Bool(true));
            runtime.advance_time(Duration::from_millis(100));
            runtime.execute_cycle().unwrap();
            assert!(
                program_flag(&runtime, "SafetyAbort"),
                "{folder}: disable/re-enable cannot clear the latch"
            );
            assert_eq!(
                number(&runtime, "%QL0"),
                0.0,
                "{folder}: re-enable remains blocked"
            );
        }
    }
}

#[test]
fn heater_lessons_five_to_eight_fresh_runtime_restores_declared_initial_state() {
    for (folder, temperature, initial_command, feedback) in [
        ("04-water-heater-p", 20.0, 0.0, None),
        ("05-water-heater-pi", 20.0, 0.0, None),
        (
            "06-water-heater-saturation-windup",
            35.0,
            75.0,
            Some("%IL8"),
        ),
        ("07-water-heater-anti-windup", 35.0, 75.0, Some("%IL8")),
    ] {
        let (mut runtime, mut simulation) = setup(folder);
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert!(
            (number(&runtime, "%IL0") - temperature).abs() < 1e-9,
            "{folder}: declared fresh PV"
        );
        assert_eq!(
            number(&runtime, "%QL0"),
            initial_command,
            "{folder}: declared fresh command"
        );
        if let Some(address) = feedback {
            assert_eq!(
                number(&runtime, address),
                initial_command,
                "{folder}: declared fresh applied feedback"
            );
        }
    }
}
