use std::sync::atomic::{AtomicUsize, Ordering};

use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::Value;
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::*;

const LESSON40: &str =
    "01-feedback-foundations-and-temperature-control/14-conservation-and-accumulation";

/// One 1 s update of the tank: measured level, model level, inlet and outlet flow.
#[derive(Clone, Copy, Debug)]
struct Tank {
    level: f64,
    model: f64,
    inlet: f64,
    outlet: f64,
}

/// `rest` updates with equal flows, then Control Studio's project experiment (activation at %MX16.0) for `updates`
/// more. Returns every update.
fn lesson40_run(rest: u64, updates: u64) -> Vec<Tank> {
    let (mut runtime, mut simulation) = setup(LESSON40);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX16.0").unwrap(), Value::Bool(true));
        }
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 1000,
            1000,
            &[],
        );
        record.push(Tank {
            level: number(&runtime, "%IL0"),
            model: number(&runtime, "%ML0"),
            inlet: number(&runtime, "%QL0"),
            outlet: number(&runtime, "%QL8"),
        });
    }
    record
}

#[test]
fn lesson40_equal_flows_hold_the_level() {
    let record = lesson40_run(10, 0);
    for update in &record {
        assert_eq!(
            (update.inlet, update.outlet),
            (0.010, 0.010),
            "equal 0.010 m³/s flows: {update:?}"
        );
        assert!(
            (update.level - 0.25).abs() <= 0.0005 + 1e-12,
            "the level stays at 0.25 m within the sensor's ±0.0005 m: {update:?}"
        );
    }
}

#[test]
fn lesson40_the_flow_step_raises_the_level_at_the_balance_rate() {
    // Step at update 5: the inlet delivers 0.012 m³/s for 10 s, so the level rises (0.012 − 0.010)/0.5 = 0.004 m/s.
    let record = lesson40_run(5, 25);
    let step = &record[5..];
    assert!(
        step[..10].iter().all(|update| update.inlet == 0.012),
        "0.012 m³/s for ten updates: {:?}",
        &step[..10]
    );
    assert!(
        step[10..].iter().all(|update| update.inlet == 0.010),
        "then 0.010 m³/s again"
    );
    let start = step[0].model;
    assert!(
        (start - 0.25).abs() <= 0.0005 + 1e-12,
        "the model starts from the measured level: {start}"
    );
    assert!(
        (step[10].model - (start + 0.04)).abs() < 1e-12,
        "ten updates add 0.004 m/s · 10 s = 0.04 m: {:?}",
        step[10]
    );
    assert!(
        (step[10].level - 0.29).abs() <= 0.001,
        "the tank reaches 0.29 m within 0.001 m after 10 s: {:?}",
        step[10]
    );
    assert!(
        (step[10].level - step[10].model).abs() <= 0.001,
        "measured and model level agree within 0.001 m: {:?}",
        step[10]
    );
}

#[test]
fn lesson40_equal_flows_again_hold_the_new_level() {
    let record = lesson40_run(5, 25);
    for update in &record[15..] {
        assert!(
            (update.level - 0.29).abs() <= 0.0005 + 1e-9,
            "equal flows hold the stored 0.29 m, not the old 0.25 m: {update:?}"
        );
        assert!(
            (update.model - record[15].model).abs() < 1e-12,
            "the model level holds too: {update:?}"
        );
    }
}

const LESSON41: &str =
    "01-feedback-foundations-and-temperature-control/16-gravity-tank-nonlinearity";

/// One 1 s update of the gravity-drained tank.
#[derive(Clone, Copy, Debug)]
struct Drained {
    level: f64,
    gravity_model: f64,
    gravity_outflow: f64,
    constant_model: f64,
    equilibrium: f64,
    inlet: f64,
}

/// `rest` updates at 0.010 m³/s, then Control Studio's project experiment (activation at %MX24.1) for `updates`.
fn lesson41_run(rest: u64, updates: u64) -> Vec<Drained> {
    let (runtime, simulation) = setup(LESSON41);
    lesson41_record(runtime, simulation, rest, updates)
}

/// The same run with the tank started at `level`, as set under Starting conditions in the hardware canvas: the
/// project's simulation.toml with its starting level replaced, loaded from a collision-proof temporary file.
fn lesson41_run_from(level: f64, rest: u64, updates: u64) -> Vec<Drained> {
    static NEXT: AtomicUsize = AtomicUsize::new(0);
    let text = std::fs::read_to_string(project_dir(LESSON41).join("simulation.toml")).unwrap();
    assert!(
        text.contains("initial_state = [0.25]"),
        "the shipped tank starts at 0.25 m"
    );
    let path = std::env::temp_dir().join(format!(
        "trust-lesson45-start-{}-{}.toml",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(
        &path,
        text.replace(
            "initial_state = [0.25]",
            &format!("initial_state = [{level:?}]"),
        ),
    )
    .unwrap();
    let simulation = SimulationController::new(SimulationConfig::load(&path).unwrap());
    std::fs::remove_file(&path).unwrap();
    let (runtime, _) = setup(LESSON41);
    lesson41_record(runtime, simulation, rest, updates)
}

fn lesson41_record(
    mut runtime: CompiledRuntime,
    mut simulation: SimulationController,
    rest: u64,
    updates: u64,
) -> Vec<Drained> {
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX24.1").unwrap(), Value::Bool(true));
        }
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 1000,
            1000,
            &[],
        );
        record.push(Drained {
            level: number(&runtime, "%IL0"),
            gravity_model: number(&runtime, "%ML0"),
            gravity_outflow: number(&runtime, "%ML8"),
            constant_model: number(&runtime, "%ML16"),
            equilibrium: number(&runtime, "%ML32"),
            inlet: number(&runtime, "%QL0"),
        });
    }
    record
}

#[test]
fn lesson41_the_gravity_tank_settles_at_the_square_law_equilibrium() {
    // 0.011 m³/s through an outlet with c = 0.020 m^(5/2)/s: h_eq = (0.011/0.020)² = 0.3025 m.
    let record = lesson41_run(5, 200);
    let run = &record[5..];
    assert!(
        run.iter().all(|update| update.inlet == 0.011),
        "0.011 m³/s throughout the 200 s run"
    );
    let last = run[run.len() - 1];
    assert!(
        (last.level - 0.3025).abs() <= 0.001,
        "the tank settles at 0.3025 m: {last:?}"
    );
    assert!(
        (last.gravity_model - 0.3025).abs() <= 1e-4,
        "the gravity model settles there too: {last:?}"
    );
    let drift = (run[run.len() - 1].gravity_model - run[run.len() - 11].gravity_model).abs();
    assert!(
        drift <= 0.001,
        "no more than 0.001 m of drift in the final 10 s: {drift}"
    );
    for update in run {
        assert!(
            (update.level - update.gravity_model).abs() <= 0.0015,
            "the gravity model follows the tank: {update:?}"
        );
    }
}

#[test]
fn lesson41_the_same_step_from_one_metre_settles_twice_as_far() {
    // Started at 1.0 m, the program holds the inflow that balances it, c·√1.0 = 0.020 m³/s, and the project
    // experiment adds the same 0.001 m³/s as at 0.25 m: h_eq = (0.021/0.020)² = 1.1025 m, 0.1025 m higher
    // instead of 0.0525 m.
    let record = lesson41_run_from(1.0, 5, 200);
    let (rest, run) = record.split_at(5);
    for update in rest {
        assert!(
            update.inlet == 0.020,
            "0.020 m³/s holds 1.0 m before the step: {update:?}"
        );
        assert!(
            (update.level - 1.0).abs() <= 0.001,
            "the tank stays at 1.0 m before the step: {update:?}"
        );
    }
    assert!(
        run.iter().all(|update| update.inlet == 0.021),
        "0.021 m³/s throughout the 200 s run"
    );
    assert!(
        run.iter()
            .all(|update| (update.equilibrium - 1.1025).abs() < 1e-12),
        "h_eq = (0.021/0.020)² = 1.1025 m"
    );
    let last = run[run.len() - 1];
    assert!(
        (last.gravity_model - 1.1025).abs() <= 0.003,
        "the gravity model approaches 1.1025 m: {last:?}"
    );
    for update in run {
        assert!(
            (update.level - update.gravity_model).abs() <= 0.0015,
            "the gravity model follows the tank: {update:?}"
        );
    }
    assert!(
        (last.constant_model - run[0].constant_model - 0.002 * 199.0).abs() < 1e-9,
        "the constant-outflow model rises 0.002 m/s at 1.0 m too: {last:?}"
    );
}

#[test]
fn lesson41_the_outflow_follows_the_square_root_of_the_level() {
    let record = lesson41_run(5, 60);
    for update in &record[5..] {
        assert!(
            (update.gravity_outflow - 0.020 * update.gravity_model.sqrt()).abs() < 1e-12,
            "q_out = c·√h at the model level: {update:?}"
        );
    }
    assert!(
        (record[4].gravity_outflow - 0.010).abs() < 1e-3,
        "0.010 m³/s at 0.25 m: {:?}",
        record[4]
    );
}

#[test]
fn lesson41_the_constant_outflow_model_keeps_rising() {
    // Lesson 43's model keeps 0.010 m³/s out: (0.011 − 0.010)/0.5 = 0.002 m/s for all 200 s.
    let record = lesson41_run(5, 200);
    let (start, last) = (record[5], record[record.len() - 1]);
    assert!(
        (last.constant_model - start.constant_model - 0.002 * 199.0).abs() < 1e-9,
        "0.002 m/s for 199 updates after the step: {start:?} → {last:?}"
    );
    assert!(
        last.constant_model - last.level > 0.3,
        "it overshoots the real tank by more than 0.3 m: {last:?}"
    );
}

const LESSON42: &str =
    "01-feedback-foundations-and-temperature-control/17-gravity-tank-linearization";

/// One 1 s update of the two tanks: measured and tangent-model levels for the small and the large step.
#[derive(Clone, Copy, Debug)]
struct Steps {
    small: f64,
    small_tangent: f64,
    large: f64,
    large_tangent: f64,
    small_inlet: f64,
    large_inlet: f64,
}

/// `rest` updates at 0.010 m³/s, then Control Studio's project experiment (activation at %MX16.0) for `updates`.
fn lesson42_run(rest: u64, updates: u64) -> Vec<Steps> {
    let (mut runtime, mut simulation) = setup(LESSON42);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX16.0").unwrap(), Value::Bool(true));
        }
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 1000,
            1000,
            &[],
        );
        record.push(Steps {
            small: number(&runtime, "%IL0"),
            small_tangent: number(&runtime, "%ML0"),
            large: number(&runtime, "%IL8"),
            large_tangent: number(&runtime, "%ML8"),
            small_inlet: number(&runtime, "%QL0"),
            large_inlet: number(&runtime, "%QL8"),
        });
    }
    record
}

#[test]
fn lesson42_the_tangent_model_rises_with_the_local_time_constant() {
    // g = c/(2√h0) = 0.020 m²/s, τ = A/g = 25 s: one time constant covers 1 − e^−1 of the tangent's 0.05 m rise
    // (the 1 s Euler update gives 1 − 0.96^25).
    let record = lesson42_run(5, 30);
    let run = &record[5..];
    assert!(
        run.iter()
            .all(|update| update.small_inlet == 0.011 && update.large_inlet == 0.015),
        "0.011 and 0.015 m³/s: {:?}",
        run[0]
    );
    // The model starts from the measured deviation x0: x25 − x0 = (0.05 − x0)·(1 − 0.96^25).
    let start = run[0].small_tangent - 0.25;
    let risen = run[25].small_tangent - run[0].small_tangent;
    assert!(
        (risen - (0.05 - start) * (1.0 - 0.96_f64.powi(25))).abs() < 1e-9,
        "the tangent covers 64 % of its rise in 25 s: {risen}"
    );
}

#[test]
fn lesson42_the_small_step_stays_within_the_local_error() {
    // 0.001 m³/s more: the tangent settles at 0.25 + 0.001/0.020 = 0.3000 m, the tank at (0.011/0.020)² = 0.3025 m.
    let record = lesson42_run(5, 200);
    let last = record[record.len() - 1];
    assert!(
        (last.small_tangent - 0.3000).abs() <= 0.0006,
        "tangent equilibrium 0.3000 m: {last:?}"
    );
    assert!(
        (last.small - 0.3025).abs() <= 0.001,
        "the tank settles at 0.3025 m: {last:?}"
    );
    assert!(
        (last.small - last.small_tangent).abs() <= 0.005,
        "within the declared 0.005 m: {last:?}"
    );
}

#[test]
fn lesson42_the_large_step_leaves_the_local_error() {
    // 0.005 m³/s more: the tangent predicts 0.5000 m, the tank reaches 0.5625 m, 0.0625 m apart.
    let record = lesson42_run(5, 200);
    let last = record[record.len() - 1];
    assert!(
        (last.large_tangent - 0.5000).abs() <= 0.0006,
        "tangent equilibrium 0.5000 m: {last:?}"
    );
    assert!(
        (last.large - 0.5625).abs() <= 0.003,
        "the tank approaches 0.5625 m: {last:?}"
    );
    assert!(
        last.large - last.large_tangent > 0.05,
        "far outside the declared 0.005 m: {last:?}"
    );
}
