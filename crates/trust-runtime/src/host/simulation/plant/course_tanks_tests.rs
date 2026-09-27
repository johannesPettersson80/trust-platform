use crate::harness::{CompileSession, SourceFile};
use crate::io::IoAddress;
use crate::simulation::{SimulationConfig, SimulationController};
use crate::value::{Duration, Value};
use crate::Runtime;
use std::path::Path;

const LESSON67: &str = "02-derivative-action-and-dynamic-response/20-state-equations-from-stores";

fn number(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL {address}, got {other:?}"),
    }
}
fn setup(project: &str) -> (Runtime, SimulationController) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(project);
    let source = std::fs::read_to_string(path.join("src/Main.st")).unwrap();
    let session = CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", source)]);
    let mut runtime = session.build_runtime().unwrap();
    runtime
        .apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
        .unwrap();
    let simulation =
        SimulationController::new(SimulationConfig::load(&path.join("simulation.toml")).unwrap());
    (runtime, simulation)
}
#[test]
fn lesson67_observes_nonlinear_cascade_and_one_second_local_predictions() {
    for deviation in [0.0_f64, 0.001] {
        let (mut runtime, mut simulation) = setup(LESSON67);
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        assert!(
            (number(&runtime, "%IL0") - 0.27).abs() < 1e-12,
            "lesson67 requires upstream .27m physical initial state"
        );
        assert!((number(&runtime, "%IL8") - 0.25).abs() < 1e-12);
        for step in 0..=120 {
            simulation
                .apply_pre_cycle(Duration::from_millis(step * 1000), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(true));
            debug.enqueue_io_write(
                IoAddress::parse("%ML72").unwrap(),
                Value::LWord(deviation.to_bits()),
            );
            runtime.advance_time(Duration::from_millis(1000));
            runtime.execute_cycle().unwrap();
            let expected_flow = 0.010 + if step < 20 { deviation } else { 0.0 };
            assert!(
                (number(&runtime, "%QL0") - expected_flow).abs() < 1e-12,
                "wrong pump flow at experiment second {step}"
            );
            if step <= 20 {
                assert!(
                    (number(&runtime, "%ML0") - number(&runtime, "%IL0")).abs() < 0.001,
                    "upstream local prediction mismatch at {step}"
                );
                assert!(
                    (number(&runtime, "%ML8") - number(&runtime, "%IL8")).abs() < 0.001,
                    "downstream local prediction mismatch at {step}"
                );
            }
            if step == 0 {
                assert!((number(&runtime, "%ML16") - (-0.0008 + 2.0 * deviation)).abs() < 1e-12);
                assert!((number(&runtime, "%ML24") - 0.0008).abs() < 1e-12);
            }
            simulation
                .apply_post_cycle(Duration::from_millis((step + 1) * 1000), &runtime)
                .unwrap();
        }
    }
}

#[test]
fn lesson67_early_abort_commands_zero_flow_without_clamping_levels() {
    let (mut runtime, mut simulation) = setup(LESSON67);
    let debug = runtime.enable_debug();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(true));
    runtime.advance_time(Duration::from_millis(1000));
    runtime.execute_cycle().unwrap();
    simulation
        .apply_post_cycle(Duration::from_millis(1000), &runtime)
        .unwrap();
    simulation
        .apply_pre_cycle(Duration::from_millis(1000), &mut runtime)
        .unwrap();
    let level = number(&runtime, "%IL0");
    debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(false));
    runtime.advance_time(Duration::from_millis(1000));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "early project cancellation must command zero pump flow"
    );
    assert_eq!(
        number(&runtime, "%IL0"),
        level,
        "the lesson must never overwrite physical truth"
    );
}

#[test]
fn lesson68_one_upstream_pump_produces_bounded_opposite_transients() {
    for deviation in [-0.001_f64, 0.001] {
        let (mut runtime, mut simulation) =
            setup("11-state-space-estimation-and-predictive-control/66-controllability");
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        assert!(
            (number(&runtime, "%IL0") - 0.25).abs() < 1e-12,
            "lesson68 starts in the .25m equilibrium"
        );
        assert!((number(&runtime, "%IL8") - 0.25).abs() < 1e-12);
        for step in 0..=20 {
            simulation
                .apply_pre_cycle(Duration::from_millis(step * 1000), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(true));
            debug.enqueue_io_write(
                IoAddress::parse("%ML72").unwrap(),
                Value::LWord(deviation.to_bits()),
            );
            runtime.advance_time(Duration::from_millis(1000));
            runtime.execute_cycle().unwrap();
            assert!(
                (number(&runtime, "%QL0") - (0.010 + if step < 20 { deviation } else { 0.0 }))
                    .abs()
                    < 1e-12
            );
            if step == 20 {
                let up = number(&runtime, "%IL0") - 0.25;
                let down = number(&runtime, "%IL8") - 0.25;
                assert!(up * deviation > 0.0 && down * deviation > 0.0);
                assert!(up.abs() > down.abs(), "upstream responds before downstream");
                assert!(up.abs() < 0.04 && down.abs() < 0.02);
            }
            simulation
                .apply_post_cycle(Duration::from_millis((step + 1) * 1000), &runtime)
                .unwrap();
        }
    }
}

#[test]
fn lesson69_downstream_history_reconstructs_upstream_and_exposes_noise_amplification() {
    for amplitude in [0.0_f64, 0.005] {
        let (mut runtime, mut simulation) =
            setup("11-state-space-estimation-and-predictive-control/67-observability");
        let debug = runtime.enable_debug();
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .unwrap();
        assert!(
            (number(&runtime, "%IL0") - 0.27).abs() < 1e-12,
            "lesson69 requires the perturbed upstream physical initial state"
        );
        assert!((number(&runtime, "%IL8") - 0.25).abs() < 1e-12);
        let mut squared_error = 0.0_f64;
        let mut previous_measurement: Option<f64> = None;
        for step in 0..=20 {
            simulation
                .apply_pre_cycle(Duration::from_millis(step * 1000), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(true));
            debug.enqueue_io_write(
                IoAddress::parse("%ML72").unwrap(),
                Value::LWord(amplitude.to_bits()),
            );
            runtime.advance_time(Duration::from_millis(1000));
            runtime.execute_cycle().unwrap();
            assert!((number(&runtime, "%QL0") - 0.010).abs() < 1e-12);
            if step == 0 {
                assert_eq!(
                    number(&runtime, "%ML40"),
                    0.0,
                    "no estimate before the second sample"
                );
            }
            let measurement = number(&runtime, "%ML8");
            if let Some(previous) = previous_measurement {
                let expected_slope = measurement - previous;
                assert!((number(&runtime, "%ML16") - expected_slope).abs() < 1e-12);
                let estimate = number(&runtime, "%ML0");
                assert!((estimate - (measurement + 25.0 * expected_slope)).abs() < 1e-12);
                let error = estimate - (number(&runtime, "%IL0") - 0.25);
                assert!((number(&runtime, "%ML24") - error).abs() < 1e-12);
                squared_error += error * error;
            }
            previous_measurement = Some(measurement);
            if step == 20 {
                let rmse = (squared_error / 20.0).sqrt();
                if amplitude == 0.0 {
                    assert!(rmse < 0.001, "clean reconstruction RMSE {rmse}");
                } else {
                    assert!(
                        (0.24..0.27).contains(&rmse),
                        "noise must expose about .25m amplification, got {rmse}"
                    );
                }
                assert_eq!(number(&runtime, "%ML40"), 20.0);
            }
            simulation
                .apply_post_cycle(Duration::from_millis((step + 1) * 1000), &runtime)
                .unwrap();
        }
    }
}

#[test]
fn lesson67_rejects_a_local_model_region_failure_before_hiding_it() {
    let (mut runtime, mut simulation) = setup(LESSON67);
    let debug = runtime.enable_debug();
    for step in 0..=14 {
        simulation
            .apply_pre_cycle(Duration::from_millis(step * 1000), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX80.0").unwrap(), Value::Bool(true));
        debug.enqueue_io_write(
            IoAddress::parse("%ML72").unwrap(),
            Value::LWord((-0.010_f64).to_bits()),
        );
        runtime.advance_time(Duration::from_millis(1000));
        runtime.execute_cycle().unwrap();
        simulation
            .apply_post_cycle(Duration::from_millis((step + 1) * 1000), &runtime)
            .unwrap();
    }
    assert!(
        number(&runtime, "%IL0") > 0.05,
        "physical level has not yet crossed its bound"
    );
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX80.4").unwrap())
            .unwrap(),
        Value::Bool(true),
        "a rejected local prediction must be reported instead of clipping the model level"
    );
}
