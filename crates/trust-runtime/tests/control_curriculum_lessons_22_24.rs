use std::path::Path;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

fn number(runtime: &Runtime, address: &str) -> f64 {
    match runtime.io().read(&IoAddress::parse(address).unwrap()).unwrap() {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL at {address}, got {other:?}"),
    }
}

fn setup(relative: &str) -> (Runtime, SimulationController) {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(relative);
    let session = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "src/Main.st",
            std::fs::read_to_string(project.join("src/Main.st")).unwrap(),
        ),
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

fn scan(
    runtime: &mut Runtime,
    simulation: &mut SimulationController,
    debug: &trust_runtime::debug::DebugControl,
    now_ms: u64,
    writes: &[(&str, Value)],
) {
    simulation
        .apply_pre_cycle(Duration::from_millis(now_ms as i64), runtime)
        .unwrap();
    for (address, value) in writes {
        debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value.clone());
    }
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    simulation
        .apply_post_cycle(Duration::from_millis((now_ms + 100) as i64), runtime)
        .unwrap();
}

#[test]
fn lesson21_accepts_a_relay_candidate_and_runs_from_the_hot_heater_reset() {
    let (mut runtime, mut simulation) = setup("06-pid-tuning-methods/31-tuning-method-comparison");
    let debug = runtime.enable_debug();
    scan(&mut runtime, &mut simulation, &debug, 0, &[]);
    assert_eq!(number(&runtime, "%IL0"), 35.0);
    assert_eq!(number(&runtime, "%QL0"), 75.0);
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        100,
        &[
            ("%ML72", Value::LWord(10.397336997569598_f64.to_bits())),
            ("%ML88", Value::LWord(2.250325442359752_f64.to_bits())),
            ("%MW80", Value::Word(0)),
            ("%MX104.0", Value::Bool(true)),
        ],
    );
    assert_eq!(number(&runtime, "%ML72"), 10.397336997569598);
    assert!(number(&runtime, "%QL0") >= 99.0);
    assert_eq!(number(&runtime, "%ML0"), 38.0);
}

#[test]
fn lesson22_selects_beta_and_a_separate_load_trial_on_the_hot_heater() {
    let (mut runtime, mut simulation) = setup(
        "01-feedback-foundations-and-temperature-control/10-proportional-setpoint-weighting",
    );
    let debug = runtime.enable_debug();
    scan(&mut runtime, &mut simulation, &debug, 0, &[]);
    assert_eq!(number(&runtime, "%IL0"), 35.0);
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        100,
        &[
            ("%ML16", Value::LWord(1.0_f64.to_bits())),
            ("%MW80", Value::Word(0)),
            ("%MX128.0", Value::Bool(true)),
        ],
    );
    assert_eq!(number(&runtime, "%ML16"), 1.0);
    assert_eq!(number(&runtime, "%ML0"), 38.0);
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        200,
        &[("%MX128.0", Value::Bool(false))],
    );
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        300,
        &[
            ("%MW80", Value::Word(1)),
            ("%MX128.0", Value::Bool(true)),
        ],
    );
    assert_eq!(number(&runtime, "%QL8"), 30.0);
}

#[test]
fn lesson23_uses_a_selectable_filter_and_retains_raw_and_true_heater_signals() {
    let (mut runtime, mut simulation) = setup(
        "01-feedback-foundations-and-temperature-control/11-measurement-filtering",
    );
    let debug = runtime.enable_debug();
    scan(&mut runtime, &mut simulation, &debug, 0, &[]);
    assert_eq!(number(&runtime, "%IL8"), 35.0);
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        100,
        &[
            ("%ML24", Value::LWord(0.0_f64.to_bits())),
            ("%MW120", Value::Word(1)),
            ("%MX80.0", Value::Bool(true)),
        ],
    );
    assert_eq!(number(&runtime, "%ML24"), 0.0);
    assert_eq!(number(&runtime, "%QL8"), 30.0);
    assert!(number(&runtime, "%ML8") >= 0.0);
}

#[test]
fn lesson24_keeps_elapsed_time_arithmetic_with_the_real_hot_heater_io() {
    let (mut runtime, mut simulation) = setup(
        "01-feedback-foundations-and-temperature-control/12-elapsed-time-and-sampling",
    );
    let debug = runtime.enable_debug();
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        0,
        &[("%MX80.0", Value::Bool(true))],
    );
    assert_eq!(number(&runtime, "%IL0"), 35.0);
    assert_eq!(number(&runtime, "%QL0"), 75.0);
    let mut event_elapsed = 0.0;
    let mut event_actual = 0.0;
    let mut event_assumed = 0.0;
    for cycle in 1..=11 {
        scan(
            &mut runtime,
            &mut simulation,
            &debug,
            cycle * 100,
            &[("%MX80.0", Value::Bool(true))],
        );
        if cycle == 10 {
            event_elapsed = number(&runtime, "%ML0");
            event_actual = number(&runtime, "%ML16");
            event_assumed = number(&runtime, "%ML24");
        }
    }
    assert_eq!(event_elapsed, 0.2);
    assert!(event_actual > event_assumed);
}
