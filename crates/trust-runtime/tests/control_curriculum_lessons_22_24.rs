use std::path::Path;

use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};

mod control_heater_support;
use control_heater_support::*;

#[test]
fn lesson22_executes_the_setpoint_weight_control_studio_applies() {
    for (weight, saturates) in [(0.5, false), (1.0, true)] {
        let (mut runtime, mut simulation) = setup(
            "01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting",
        );
        let debug = runtime.enable_debug();
        let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
            panic!("Lesson22 must execute its P1 program");
        };
        for step in 0..10 {
            scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
            assert_eq!(
                number(&runtime, "%QL0"),
                75.0,
                "Manual holds the hot-reset heater command"
            );
            assert!(
                (number(&runtime, "%IL8") - 35.0).abs() <= 0.020001,
                "the Lesson 21 heater holds 35 °C within its ±0.02 °C noise"
            );
        }
        let Some(Value::Instance(controller)) = runtime
            .storage()
            .get_instance_var(program, "Controller")
            .cloned()
        else {
            panic!("Lesson22 must instantiate the real Control.PID block");
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
            panic!("Lesson22 must keep the complete Control.PID parameter record that Control Studio applies");
        };
        let Value::Struct(ref mut record) = proposed else {
            panic!("complete Control.PID parameter record")
        };
        for (field, value) in [
            ("ProportionalGain", 13.0),
            ("IntegralGain", 3.5),
            ("ProportionalSetpointWeight", weight),
        ] {
            *std::sync::Arc::make_mut(record).field_mut(field).unwrap() = Value::LReal(value);
        }
        assert!(runtime
            .storage_mut()
            .set_instance_var(program, "Parameters", proposed.clone()));
        for step in 10..13 {
            scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
            assert_eq!(
                runtime.storage().get_instance_var(program, "Parameters"),
                Some(&proposed),
                "cyclic code must preserve the weight Control Studio applied"
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
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            1300,
            &[("%ML0", Value::LWord(38.0_f64.to_bits()))],
        );
        let pv = number(&runtime, "%IL8");
        let request = 75.0
            + 13.0 * (weight * (38.0 - setpoint_before) - (pv - pv_before))
            + 3.5 * 0.1 * (38.0 - pv);
        let expected = request.clamp(0.0, 100.0);
        assert!((number(&runtime, "%QL0") - expected).abs() < 1e-9,
            "β {weight}: the weighted setpoint kick drives the first request, got {} (expected {expected})", number(&runtime, "%QL0"));
        assert_eq!(number(&runtime, "%QL0") >= 100.0, saturates,
            "β {weight}: a 3 °C step with Kp 13 saturates the heater only with the full setpoint weight");
    }
}

#[test]
fn lesson22_hmi_only_binds_signals_of_the_executed_heater() {
    let relative =
        "01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting";
    let (runtime, _) = setup(relative);
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative);
    let descriptor = trust_runtime::hmi::load_hmi_dir(&project).unwrap();
    let diagnostics = trust_runtime::hmi::validate_hmi_bindings(
        "Res",
        &runtime.metadata_snapshot(),
        None,
        &descriptor,
    );
    assert!(
        diagnostics.is_empty(),
        "The Lesson 23 HMI must bind the executed heater project: {diagnostics:?}"
    );
}

#[test]
fn lesson23_filters_the_measurement_control_studio_applies() {
    for filter_seconds in [1.0, 0.0] {
        let (mut runtime, mut simulation) =
            setup("01-feedback-foundations-and-temperature-control/11-measurement-filtering");
        let debug = runtime.enable_debug();
        let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
            panic!("Lesson23 must execute its P1 program");
        };
        assert!(
            runtime.storage_mut().set_instance_var(
                program,
                "FilterTimeS",
                Value::LReal(filter_seconds)
            ),
            "Control Studio applies the filter time Tf to the program"
        );
        for step in 0..5 {
            scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
            assert_eq!(
                number(&runtime, "%QL0"),
                75.0,
                "Manual holds the hot-reset heater command"
            );
            assert_eq!(
                program_number(&runtime, program, "FilteredC"),
                number(&runtime, "%IL8"),
                "in Manual the filter starts from the fresh measurement, so Auto entry is bumpless"
            );
        }
        let Some(Value::Instance(controller)) = runtime
            .storage()
            .get_instance_var(program, "Controller")
            .cloned()
        else {
            panic!("Lesson23 must instantiate the real Control.PID block");
        };
        debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
        scan(&mut runtime, &mut simulation, &debug, 500, &[]);
        assert_eq!(program_number(&runtime, program, "FilteredC"), number(&runtime, "%IL8"),
            "the Auto-entry scan starts the filter from the fresh measurement, so the transfer is exactly bumpless");
        let alpha = 0.1 / (filter_seconds + 0.1);
        for step in 6..25 {
            let previous = program_number(&runtime, program, "FilteredC");
            scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
            let raw = number(&runtime, "%IL8");
            let filtered = program_number(&runtime, program, "FilteredC");
            assert!((filtered - (previous + alpha * (raw - previous))).abs() < 1e-12,
                "Tf {filter_seconds} s: yf_new = yf_old + α(yr − yf_old) with α = {alpha}, got {filtered} from {previous} and {raw}");
            let Some(Value::Struct(state)) =
                runtime.storage().get_instance_var(controller, "StateValue")
            else {
                panic!("Control.PID must retain its executed state");
            };
            assert_eq!(
                state.field("ProcessValue"),
                Some(&Value::LReal(filtered)),
                "Control.PID uses the filtered measurement"
            );
            if filter_seconds == 0.0 {
                assert_eq!(filtered, raw, "Tf = 0 bypasses the filter");
            }
        }
    }
}

#[test]
fn lesson23_hmi_only_binds_signals_of_the_executed_heater() {
    let relative = "01-feedback-foundations-and-temperature-control/11-measurement-filtering";
    let (runtime, _) = setup(relative);
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative);
    let descriptor = trust_runtime::hmi::load_hmi_dir(&project).unwrap();
    let diagnostics = trust_runtime::hmi::validate_hmi_bindings(
        "Res",
        &runtime.metadata_snapshot(),
        None,
        &descriptor,
    );
    assert!(
        diagnostics.is_empty(),
        "The Lesson 24 HMI must bind the executed heater project: {diagnostics:?}"
    );
}

#[test]
fn lesson24_updates_the_controller_on_the_measured_clock() {
    for (period, measured, delay) in [(0.5, true, 0.0), (0.5, false, 0.0), (0.1, true, 2.0)] {
        let (mut runtime, mut simulation) =
            setup("01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling");
        let debug = runtime.enable_debug();
        let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
            panic!("Lesson24 must execute its P1 program");
        };
        for step in 0..5 {
            scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
            assert_eq!(
                number(&runtime, "%QL0"),
                75.0,
                "Manual holds the hot-reset heater command"
            );
        }
        assert!(runtime.storage_mut().set_instance_var(
            program,
            "ControllerPeriodS",
            Value::LReal(period)
        ));
        assert!(runtime.storage_mut().set_instance_var(program, "AssumedCycleS", Value::LReal(if measured { 0.0 } else { 0.1 })),
            "AssumedCycleS 0 uses the measured interval; a positive value assumes that fixed interval");
        debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
        let mut updates = Vec::new();
        let mut last_update_ms = None;
        for step in 5..60 {
            if step == 20 && delay > 0.0 {
                assert!(runtime.storage_mut().set_instance_var(
                    program,
                    "DelayUpdateS",
                    Value::LReal(delay)
                ));
            }
            let before = controller_state(&runtime, program, "ProcessValue");
            scan(
                &mut runtime,
                &mut simulation,
                &debug,
                step * 1000 / 10,
                &[("%ML0", Value::LWord(36.0_f64.to_bits()))],
            );
            if controller_state(&runtime, program, "ProcessValue") != before {
                // Control.PID was updated in this scan.
                let elapsed = program_number(&runtime, program, "ElapsedS");
                if let Some(previous) = last_update_ms {
                    assert!(
                        (elapsed - (step * 100 - previous) as f64 / 1000.0).abs() < 1e-9,
                        "ElapsedS is the measured time since the previous update"
                    );
                }
                last_update_ms = Some(step * 100);
                let expected_cycle = if measured { elapsed } else { 0.1 };
                assert!(
                    (input_cycle_time(&runtime, program) - expected_cycle).abs() < 1e-9,
                    "Control.PID integrates over {} (measured {elapsed} s)",
                    if measured {
                        "the measured interval"
                    } else {
                        "the assumed 0.1 s"
                    }
                );
                updates.push(elapsed);
            }
        }
        let steady: Vec<f64> = updates.iter().skip(1).copied().collect();
        if delay > 0.0 {
            assert!(
                steady
                    .iter()
                    .any(|elapsed| (elapsed - (period + delay)).abs() < 1e-6),
                "a delayed update integrates the whole {} s interval: {steady:?}",
                period + delay
            );
        } else {
            assert!(
                steady.iter().all(|elapsed| (elapsed - period).abs() < 1e-6),
                "the controller updates every {period} s of measured time: {steady:?}"
            );
        }
    }
}

const LESSON24: &str =
    "01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling";

#[test]
fn lesson24_holds_the_command_over_an_unusable_interval() {
    let (mut runtime, mut simulation) = setup(LESSON24);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..5 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    let setpoint = [("%ML0", Value::LWord(36.0_f64.to_bits()))];
    for step in 5..20 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &setpoint);
    }
    // One update 3 s late: its 3.1 s interval is longer than half the loop's 4.46 s ultimate period.
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "DelayUpdateS", Value::LReal(3.0)));
    let mut late = None;
    for step in 20..70 {
        let output = number(&runtime, "%QL0");
        let controller_pv = controller_state(&runtime, program, "ProcessValue");
        scan(&mut runtime, &mut simulation, &debug, step * 100, &setpoint);
        let elapsed = program_number(&runtime, program, "ElapsedS");
        match late {
            None if (elapsed - 3.1).abs() < 1e-6 => {
                assert_eq!(
                    number(&runtime, "%QL0"),
                    output,
                    "the command is held over the unusable interval"
                );
                assert_eq!(
                    controller_state(&runtime, program, "ProcessValue"),
                    controller_pv,
                    "Control.PID is not updated over the unusable interval"
                );
                late = Some(step);
            }
            Some(late_step) if step == late_step + 1 => {
                assert!(
                    (elapsed - 0.1).abs() < 1e-6,
                    "the next interval starts at the rejected acquisition: {elapsed}"
                );
                assert!((input_cycle_time(&runtime, program) - 0.1).abs() < 1e-9);
                assert_eq!(program_number(&runtime, program, "FilteredC"), program_number(&runtime, program, "SafeProcessValueC"),
                    "the first update after the rejection restarts the filter from the fresh measurement");
            }
            _ => {}
        }
    }
    assert!(late.is_some(), "the 3.1 s interval must be measured");
}

#[test]
fn lesson24_keeps_measuring_intervals_after_a_day() {
    let (mut runtime, mut simulation) = setup(LESSON24);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..5 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
    }
    // A day without a scan, then normal scans in Auto: the interval measurement must keep working.
    runtime.advance_time(Duration::from_secs(24 * 3600));
    runtime.execute_cycle().unwrap();
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    debug.enqueue_io_write(
        IoAddress::parse("%ML0").unwrap(),
        Value::LWord(36.0_f64.to_bits()),
    );
    let mut outputs = Vec::new();
    for scan_number in 0..30 {
        runtime.advance_time(Duration::from_millis(100));
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("scan {scan_number} after a day: {error:?}"));
        outputs.push(number(&runtime, "%QL0"));
        let elapsed = program_number(&runtime, program, "ElapsedS");
        assert!(
            (elapsed - 0.1).abs() < 1e-9,
            "scan {scan_number} after a day measures 0.1 s, got {elapsed}"
        );
    }
    assert!(
        outputs.windows(2).all(|pair| pair[1] > pair[0]),
        "the controller keeps raising the command toward 36 °C: {outputs:?}"
    );
}

#[test]
fn lesson24_verifies_from_the_state_control_studio_presets() {
    let (mut runtime, mut simulation) = setup(LESSON24);
    let program = program(&runtime);
    // control-loop.json pidInitialization: Control Studio presets the Control.PID states and
    // P1.Initialized instead of running the program's reset, and starts its own clock at 0.
    let Some(Value::Instance(controller)) = runtime
        .storage()
        .get_instance_var(program, "Controller")
        .cloned()
    else {
        panic!("Lesson24 must execute Control.PID");
    };
    let Some(Value::Struct(state)) = runtime
        .storage()
        .get_instance_var(controller, "StateValue")
        .cloned()
    else {
        panic!("Control.PID must keep its state record");
    };
    let mut state = (*state).clone();
    for (field, value) in [
        ("Setpoint", 35.0),
        ("SetpointRate", 0.0),
        ("ProcessValue", 35.0),
        ("ProcessValueRate", 0.0),
        ("FeedForward", 0.0),
        ("NominalOutput", 75.0),
        ("SaturatedOutput", 75.0),
    ] {
        assert!(state.set_existing_field(field.into(), Value::LReal(value)));
    }
    assert!(runtime.storage_mut().set_instance_var(
        controller,
        "StateValue",
        Value::Struct(std::sync::Arc::new(state))
    ));
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "Initialized", Value::Bool(true)));
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "AutoMode", Value::Bool(true)));
    // SetpointC is bound to %ML0: like Control Studio, write the address and the variable.
    runtime
        .io_mut()
        .write(
            &IoAddress::parse("%ML0").unwrap(),
            Value::LWord(38.0_f64.to_bits()),
        )
        .unwrap();
    assert!(runtime
        .storage_mut()
        .set_instance_var(program, "SetpointC", Value::LReal(38.0)));
    let mut outputs = Vec::new();
    let mut intervals = Vec::new();
    for sequence in 0..30_i64 {
        let now = Duration::from_millis(sequence * 100);
        runtime.set_current_time(now);
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("cycle {sequence}: {error:?}"));
        outputs.push(number(&runtime, "%QL0"));
        intervals.push(program_number(&runtime, program, "ElapsedS"));
        if sequence > 1 {
            assert!(
                (input_cycle_time(&runtime, program) - 0.1).abs() < 1e-9,
                "cycle {sequence}: Control.PID integrates over 0.1 s"
            );
        }
        simulation.apply_post_cycle(now, &runtime).unwrap();
    }
    // The 100 ms task first runs at 0.1 s. That scan has no previous acquisition, hence no interval.
    assert_eq!(
        intervals[1], 0.0,
        "the first executed scan measures no interval"
    );
    assert_eq!(
        outputs[1], 75.0,
        "the preset command is held over the unusable first interval"
    );
    assert!(
        intervals[2..]
            .iter()
            .all(|elapsed| (elapsed - 0.1).abs() < 1e-9),
        "each later update measures the verification clock's 0.1 s: {intervals:?}"
    );
    assert!(
        outputs[2] > 75.0,
        "the controller acts on the 3 °C setpoint step from the next scan: {outputs:?}"
    );
}

#[test]
fn lesson24_hmi_only_binds_signals_of_the_executed_heater() {
    let relative = "01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling";
    let (runtime, _) = setup(relative);
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative);
    let descriptor = trust_runtime::hmi::load_hmi_dir(&project).unwrap();
    let diagnostics = trust_runtime::hmi::validate_hmi_bindings(
        "Res",
        &runtime.metadata_snapshot(),
        None,
        &descriptor,
    );
    assert!(
        diagnostics.is_empty(),
        "The Lesson 25 HMI must bind the executed heater project: {diagnostics:?}"
    );
}
