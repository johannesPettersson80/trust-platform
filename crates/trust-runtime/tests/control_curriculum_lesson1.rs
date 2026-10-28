//! Specification 38 §3.1: the manual sequence starts with experiment enablement.
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::{compile, LessonRuntime};

const PROJECT: &str = "../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy";
const MAIN: &str = include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/src/Main.st");

#[test]
fn nonfinite_sensor_fault_restores_safe_commands() {
    use trust_runtime::config::{IoConfig, RuntimeConfig};
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(PROJECT);
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        for fault_scan in [100, 700] {
            let session =
                CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", MAIN)]);
            let mut runtime = compile(session);
            runtime.set_fault_policy(
                RuntimeConfig::load(project.join("runtime.toml"))
                    .unwrap()
                    .fault_policy,
            );
            runtime.set_io_safe_state(IoConfig::load(project.join("io.toml")).unwrap().safe_state);
            let debug = runtime.enable_debug();
            for _ in 0..fault_scan {
                debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
                debug.enqueue_io_write(
                    IoAddress::parse("%IL0").unwrap(),
                    Value::LWord(4.0_f64.to_bits()),
                );
                runtime.advance_time(Duration::from_millis(100));
                runtime.execute_cycle().unwrap();
            }
            assert_eq!(
                number(&runtime, if fault_scan == 100 { "%QL0" } else { "%QL16" }),
                if fault_scan == 100 { 100.0 } else { 1.0 }
            );
            debug.enqueue_io_write(
                IoAddress::parse("%IL0").unwrap(),
                Value::LWord(value.to_bits()),
            );
            runtime.advance_time(Duration::from_millis(100));
            let error = runtime.execute_cycle().unwrap_err();
            assert!(error.to_string().contains("must be finite"), "{error}");
            assert_eq!(
                number(&runtime, "%QL0"),
                0.0,
                "a rejected sensor value must not leave refrigeration ON"
            );
            assert_eq!(
                number(&runtime, "%QL16"),
                0.0,
                "a rejected sensor value must close the door disturbance"
            );
            assert_eq!(number(&runtime, "%QL8"), 4.0);
        }
    }
}

fn number(runtime: &CompiledRuntime, address: &str) -> f64 {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        Value::LReal(value) => value,
        value => panic!("expected LREAL at {address}, got {value:?}"),
    }
}

fn run_case(wait_scans: i64, initialize_validity: bool) -> Vec<[f64; 5]> {
    let session = CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", MAIN)]);
    let mut runtime = compile(session);
    let debug = runtime.enable_debug();
    let config = SimulationConfig::load(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(PROJECT)
            .join("simulation.toml"),
    )
    .expect("load actual Lesson 1 plant");
    let mut simulation = SimulationController::new(config);
    let mut rows = Vec::new();
    for scan in 0..wait_scans + 1202 {
        let now = Duration::from_millis(scan * 100);
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        if initialize_validity {
            debug.enqueue_io_write(IoAddress::parse("%MX24.2").unwrap(), Value::Bool(true));
        }
        if scan == wait_scans || scan == wait_scans + 1200 {
            debug.enqueue_io_write(
                IoAddress::parse("%MX24.0").unwrap(),
                Value::Bool(scan == wait_scans),
            );
        }
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().unwrap();
        if !initialize_validity {
            assert_eq!(
                number(&runtime, "%ML0"),
                2.0,
                "the delivered lesson must publish its fixed target without hidden setup writes"
            );
        }
        let pv = number(&runtime, "%IL0");
        let command = number(&runtime, "%QL0");
        let door = number(&runtime, "%IL8");
        let surroundings = number(&runtime, "%IL16");
        if scan < wait_scans {
            assert_eq!(
                door, 0.0,
                "door must remain closed while learner reviews the experiment"
            );
            assert_eq!(command, 0.0);
            assert!(
                (pv - 4.0).abs() < 1e-8,
                "disabled room must retain its starting equilibrium"
            );
        } else if scan < wait_scans + 1200 {
            let elapsed = scan - wait_scans;
            assert_eq!(command, if elapsed < 200 { 100.0 } else { 0.0 });
            // Feedback is published from the preceding plant step.
            if (601..899).contains(&elapsed) {
                assert_eq!(door, 1.0, "door must open during the enabled experiment");
                assert_eq!(surroundings, 6.0);
            }
            if !(599..=901).contains(&elapsed) {
                assert_eq!(door, 0.0);
            }
            rows.push([elapsed as f64 * 0.1, pv, command, door, surroundings]);
        } else {
            assert_eq!(
                command, 0.0,
                "disabling the experiment restores refrigeration OFF"
            );
        }
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 100), &runtime)
            .unwrap();
    }
    assert!(
        rows[200][1] < 1.0,
        "refrigeration must cool the actual plant"
    );
    assert!(
        rows[899][1] - rows[599][1] > 1.5,
        "open door must produce the promised warming"
    );
    rows
}

#[test]
fn manual_experiment_is_independent_of_time_spent_reviewing() {
    let immediate = run_case(0, true);
    let delayed = run_case(1000, true);
    assert_eq!(
        immediate, delayed,
        "100 seconds of review must not change the experiment"
    );
    if let Ok(path) = std::env::var("TRUST_LESSON1_TRACE") {
        let mut csv = String::from("time_s,pv_degC,requested_mv_percent,applied_mv_percent,door_open,effective_surroundings_degC,evidence_class\n");
        for [time, pv, command, door, surroundings] in immediate {
            csv.push_str(&format!(
                "{time:.1},{pv:.12},{command},{command},{door},{surroundings},simulation\n"
            ));
        }
        std::fs::write(path, csv).expect("retain executed simulator trace");
    }
}

#[test]
fn manual_experiment_starts_without_hidden_memory_writes() {
    run_case(0, false);
}

#[test]
fn abort_and_invalid_measurements_clear_both_commands() {
    // Exercise both active refrigeration and the later open-door interval.
    for fault_scan in [100, 700] {
        for invalid_value in [None, Some(-10.1), Some(10.1)] {
            let session =
                CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", MAIN)]);
            let mut runtime = compile(session);
            let debug = runtime.enable_debug();
            for scan in 0..=fault_scan + 2 {
                debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
                debug.enqueue_io_write(
                    IoAddress::parse("%IL0").unwrap(),
                    Value::LWord(
                        (if scan >= fault_scan {
                            invalid_value.unwrap_or(4.0_f64)
                        } else {
                            4.0_f64
                        })
                        .to_bits(),
                    ),
                );
                debug.enqueue_io_write(
                    IoAddress::parse("%MX24.1").unwrap(),
                    Value::Bool(scan >= fault_scan && invalid_value.is_none()),
                );
                runtime.advance_time(Duration::from_millis(100));
                runtime.execute_cycle().unwrap();
                if scan == fault_scan - 1 {
                    assert_eq!(
                        number(&runtime, "%QL0"),
                        if fault_scan == 100 { 100.0 } else { 0.0 }
                    );
                    assert_eq!(
                        number(&runtime, "%QL16"),
                        if fault_scan == 700 { 1.0 } else { 0.0 }
                    );
                }
                if scan >= fault_scan {
                    assert_eq!(
                        number(&runtime, "%ML8"),
                        0.0,
                        "requested refrigeration must clear"
                    );
                    assert_eq!(
                        number(&runtime, "%QL0"),
                        0.0,
                        "applied refrigeration must clear"
                    );
                    assert_eq!(number(&runtime, "%QL16"), 0.0, "door command must clear");
                    assert_eq!(
                        number(&runtime, "%QL8"),
                        4.0,
                        "closed-door surroundings must return"
                    );
                    assert_eq!(
                        number(&runtime, "%ML32"),
                        0.0,
                        "experiment clock must reset"
                    );
                }
            }
        }
    }
}
