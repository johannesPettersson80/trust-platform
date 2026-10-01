//! Primary application contract: the declared mode drives the actual Control.PID.
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};

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
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("README-example.st", source),
    ])
    .build_runtime()
    .expect("compile the actual published library example");
    let mut outputs = Vec::new();
    for _ in 0..2 {
        runtime.advance_time(Duration::from_millis(10));
        runtime
            .execute_cycle()
            .expect("execute published cyclic example");
        let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
            panic!("program instance");
        };
        let Some(Value::LReal(output)) = runtime.storage().get_instance_var(program, "Output")
        else {
            panic!("actual PID output");
        };
        outputs.push(*output);
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
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "src/main.st",
            include_str!("../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ])
    .build_runtime()
    .expect("compile actual PID startup");
    let debug = runtime.enable_debug();
    debug.enqueue_io_write(
        IoAddress::parse("%IL0").unwrap(),
        Value::LWord(20.0_f64.to_bits()),
    );
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    let Value::Instance(program) = runtime.storage().get_global("P1").unwrap() else {
        panic!("program instance");
    };
    assert_eq!(
        runtime.storage().get_instance_var(*program, "AutoMode"),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        runtime.storage().get_instance_var(*program, "Setpoint"),
        Some(&Value::LReal(20.0)),
        "the initial Manual reference must match the measured operating point, not zeroed memory"
    );
}

#[test]
fn primary_pid_example_manual_mode_controls_the_actual_block() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "src/main.st",
            include_str!("../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ])
    .build_runtime()
    .expect("compile recovered actual Control.PID example");
    let debug = runtime.enable_debug();
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("program instance");
    };
    for _ in 0..30 {
        debug.enqueue_instance_write(program, "AutoMode", Value::Bool(false));
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
    let output = match runtime
        .io()
        .read(&IoAddress::parse("%QL0").unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL output image, got {value:?}"),
    };
    assert!(
        output.abs() < 1e-9,
        "Manual must reach its zero command despite positive SP-PV; actual output {output}"
    );
}

#[test]
fn primary_pid_example_adopts_the_parameter_record_without_overwriting_it() {
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "src/main.st",
            include_str!("../../../examples/control-pid-commissioning/src/Main.st"),
        ),
    ])
    .build_runtime()
    .expect("compile actual Control.PID example");
    let debug = runtime.enable_debug();
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("program instance");
    };
    for _ in 0..10 {
        debug.enqueue_instance_write(program, "AutoMode", Value::Bool(false));
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
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("program instance");
    };
    let mut proposed = runtime
        .storage()
        .get_instance_var(program, "Parameters")
        .unwrap()
        .clone();
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
        .storage_mut()
        .set_instance_var(program, "Parameters", proposed.clone()));
    for _ in 0..3 {
        debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
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
            runtime.storage().get_instance_var(program, "Parameters"),
            Some(&proposed),
            "cyclic code must preserve the adopted record"
        );
        let Value::LWord(bits) = runtime
            .io()
            .read(&IoAddress::parse("%QL0").unwrap())
            .unwrap()
        else {
            panic!("LREAL output");
        };
        assert!(
            (f64::from_bits(bits) - 2.0).abs() < 1e-9,
            "the actual block must execute the adopted gain, integral switch and bias"
        );
        assert_eq!(runtime.storage().get_instance_var(program, "ParameterRevision"), Some(&Value::DInt(2)),
            "startup and one changed complete record must produce two revisions, without cyclic increments");
    }
}
