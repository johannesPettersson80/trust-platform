//! Primary application contract: the declared mode drives the actual Control.PID.
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};

mod control_heater_support;
use control_heater_support::{enqueue_instance_write, program, value, LessonRuntime};

#[test]
fn control_library_readme_example_retains_integral_history_between_cycles() {
    let readme = include_str!("../../../libraries/control/README.md");
    let program = readme
        .split("```iecst\n")
        .nth(1)
        .unwrap()
        .split("```")
        .next()
        .unwrap();
    let source = format!("{program}\nCONFIGURATION Config\nRESOURCE Res ON PLC\nTASK MainTask(INTERVAL := T#10ms, PRIORITY := 1);\nPROGRAM P1 WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION");
    let mut runtime = TestHarness::from_session(CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("README-example.st", source),
    ]))
    .expect("compile the actual published library example")
    .into_runtime();
    let mut outputs = Vec::new();
    for _ in 0..2 {
        runtime.advance_time(Duration::from_millis(10));
        runtime
            .execute_cycle()
            .expect("execute published cyclic example");
        let program = control_heater_support::program(&runtime);
        let Some(Value::LReal(output)) = value(&runtime, &format!("{program}.Output")) else {
            panic!("actual PID output");
        };
        outputs.push(output);
    }
    assert!(
        outputs[0].is_finite() && outputs[0] > 0.0,
        "initial PI output: {outputs:?}"
    );
    assert!((outputs[1] - outputs[0] - 0.005).abs() < 1e-9,
        "the next cycle must retain and advance integral history, not initialize it again: {outputs:?}");
}

#[test]
fn primary_pid_example_starts_manual_with_reference_at_the_measured_process() {
    let mut runtime = TestHarness::from_session(CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "src/main.st",
            include_str!("../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ]))
    .expect("compile actual PID startup")
    .into_runtime();
    let debug = runtime.enable_debug();
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(20.0_f64.to_bits()),
    );
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    let program = program(&runtime);
    assert_eq!(
        value(&runtime, &format!("{program}.AutoMode")),
        Some(Value::Bool(false))
    );
    assert_eq!(
        value(&runtime, &format!("{program}.Setpoint")),
        Some(Value::LReal(20.0)),
        "the initial Manual reference must match the measured operating point, not zeroed memory"
    );
}

#[test]
fn primary_pid_example_manual_mode_controls_the_actual_block() {
    let mut runtime = TestHarness::from_session(CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "src/main.st",
            include_str!("../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ]))
    .expect("compile recovered actual Control.PID example")
    .into_runtime();
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for _ in 0..30 {
        enqueue_instance_write(&debug, program, "AutoMode", Value::Bool(false));
        debug.enqueue_io_write(
            IoAddress::parse("%ML0").unwrap(),
            Value::LWord(40.0_f64.to_bits()),
        );
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(20.0_f64.to_bits()),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime
            .execute_cycle()
            .expect("execute actual PID in declared Manual mode");
    }
    let output = match runtime.read_io(&IoAddress::parse("%QL0").unwrap()).unwrap() {
        Value::LWord(bits) => f64::from_bits(bits),
        Value::LReal(value) => value,
        value => panic!("expected LREAL output image, got {value:?}"),
    };
    assert!(
        output.abs() < 1e-9,
        "Manual must reach its zero command despite positive SP-PV; actual output {output}"
    );
}

#[test]
fn primary_pid_example_adopts_the_parameter_record_without_overwriting_it() {
    let mut runtime = TestHarness::from_session(CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "src/main.st",
            include_str!("../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ]))
    .expect("compile actual Control.PID example")
    .into_runtime();
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for _ in 0..10 {
        enqueue_instance_write(&debug, program, "AutoMode", Value::Bool(false));
        debug.enqueue_io_write(
            IoAddress::parse("%ML0").unwrap(),
            Value::LWord(20.0_f64.to_bits()),
        );
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(20.0_f64.to_bits()),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
    }
    let mut proposed = value(&runtime, &format!("{program}.Parameters")).unwrap();
    let Value::Struct(ref mut record) = proposed else {
        panic!("complete PID parameter record");
    };
    for (field, value) in [
        ("ProportionalGain", 2.0),
        ("IntegralGain", 0.0),
        ("DerivativeGain", 0.0),
        ("Bias", 0.0),
    ] {
        *std::sync::Arc::make_mut(record).field_mut(field).unwrap() = Value::LReal(value);
    }
    assert!(runtime
        .write_instance(program, "Parameters", proposed.clone())
        .is_ok());
    for _ in 0..3 {
        enqueue_instance_write(&debug, program, "AutoMode", Value::Bool(true));
        debug.enqueue_io_write(
            IoAddress::parse("%ML0").unwrap(),
            Value::LWord(21.0_f64.to_bits()),
        );
        debug.enqueue_io_write(
            IoAddress::parse("%IL0").unwrap(),
            Value::LWord(20.0_f64.to_bits()),
        );
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        assert_eq!(
            value(&runtime, &format!("{program}.Parameters")),
            Some(proposed.clone()),
            "cyclic code must preserve the adopted record"
        );
        let output = match runtime.read_io(&IoAddress::parse("%QL0").unwrap()).unwrap() {
            Value::LWord(bits) => f64::from_bits(bits),
            Value::LReal(value) => value,
            _ => panic!("LREAL output"),
        };
        assert!(
            (output - 2.0).abs() < 1e-9,
            "the actual block must execute the adopted gain, integral switch and bias"
        );
        assert_eq!(value(&runtime, &format!("{program}.ParameterRevision")), Some(Value::DInt(2)),
            "startup and one changed complete record must produce two revisions, without cyclic increments");
    }
}
