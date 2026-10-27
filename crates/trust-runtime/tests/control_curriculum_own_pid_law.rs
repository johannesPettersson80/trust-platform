//! Lesson 40, Your own PID law: the program's own positional PID drives the drum axis that holds the weight.
use trust_runtime::debug::DebugControl;
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::SimulationController;
use trust_runtime::value::Value;
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::*;

const PROJECT: &str = "03-production-quality-digital-pid/86-own-pid-law";
const HOLDING_TORQUE: f64 = 1.962;

fn own_law(runtime: &CompiledRuntime, program: &str) -> String {
    let own = format!("{program}.Own");
    assert!(
        exists(runtime, &own),
        "the program's own PositionalPid must drive the axis"
    );
    own
}

fn own_number(runtime: &CompiledRuntime, own: &str, name: &str) -> f64 {
    match value(runtime, &format!("{own}.{name}")) {
        Some(Value::LReal(value)) => value,
        other => panic!("Own.{name}: {other:?}"),
    }
}

fn step(
    runtime: &mut CompiledRuntime,
    simulation: &mut SimulationController,
    debug: &DebugControl,
    update: u64,
) {
    scan_every(runtime, simulation, debug, update * 20, 20, &[]);
}

/// Control Studio's start of the actual ST at the holding operating point, as its validation run and Apply do:
/// the designed gains, the integral term at the present torque, the previous measurement at the present angle,
/// and Auto, all at one task boundary. `integral` is `None` when the setup did not name the integral term.
fn start_in_auto(
    runtime: &mut CompiledRuntime,
    debug: &DebugControl,
    program: &str,
    gains: (f64, f64, f64),
    integral: Option<f64>,
) {
    set_program_record(
        runtime,
        program,
        "OwnParameters",
        &[
            ("ProportionalGain", gains.0),
            ("IntegralGain", gains.1),
            ("DerivativeGain", gains.2),
        ],
    );
    let own = own_law(runtime, program);
    if let Some(integral) = integral {
        enqueue_instance_write(debug, &own, "IntegralNm", Value::LReal(integral));
    }
    let angle = number(runtime, "%IL0");
    enqueue_instance_write(debug, &own, "PreviousAngleRad", Value::LReal(angle));
    enqueue_instance_write(debug, program, "AutoMode", Value::Bool(true));
}

#[test]
fn own_pid_law_holds_the_weight_in_manual_at_the_hot_reset() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    for update in 0..50 {
        step(&mut runtime, &mut simulation, &debug, update);
        assert_eq!(
            number(&runtime, "%QL8"),
            HOLDING_TORQUE,
            "the weight pulls with 1.962 N·m"
        );
        assert_eq!(
            number(&runtime, "%QL0"),
            HOLDING_TORQUE,
            "Manual holds the weight with 1.962 N·m"
        );
        assert!(
            number(&runtime, "%IL0").abs() <= 0.001 + 1e-12,
            "the axis stays at rest: only the ±0.001 rad noise remains"
        );
    }
}

#[test]
fn own_pid_law_continues_from_the_holding_torque_with_the_state_control_studio_sets() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for update in 0..10 {
        step(&mut runtime, &mut simulation, &debug, update);
    }
    let previous_angle = number(&runtime, "%IL0");
    let own = own_law(&runtime, program);
    // Lesson 39's gain-revision rule still runs: it adds (Kp,old − Kp,new)·e and (Kd,new − Kd,old)·ω of the
    // previous update, which at rest are only measurement noise.
    let revision = (16.0 - 30.0) * own_number(&runtime, &own, "PreviousErrorRad")
        - (7.0 - 10.0) * own_number(&runtime, &own, "PreviousVelocityRadS");
    let (kp, ki, kd) = (30.0, 20.0, 10.0);
    start_in_auto(
        &mut runtime,
        &debug,
        program,
        (kp, ki, kd),
        Some(HOLDING_TORQUE),
    );
    step(&mut runtime, &mut simulation, &debug, 10);
    // The filter restarts at the Auto entry, so the law sees the measured angle itself.
    let angle = number(&runtime, "%ML32");
    let error = number(&runtime, "%ML0") - angle;
    let expected = kp * error + (HOLDING_TORQUE + revision + ki * error * 0.02)
        - kd * (angle - previous_angle) / 0.02;
    assert!(
        (number(&runtime, "%QL0") - expected).abs() < 1e-9,
        "the first Auto torque is P + I + D from the state Control Studio set: {} (expected {expected})",
        number(&runtime, "%QL0")
    );
    let mut lowest = f64::INFINITY;
    for update in 11..110 {
        step(&mut runtime, &mut simulation, &debug, update);
        assert_eq!(
            program_record_number(&runtime, program, "OwnParameters", "ProportionalGain"),
            kp,
            "cyclic code keeps the applied gains"
        );
        lowest = lowest.min(number(&runtime, "%IL0"));
    }
    assert!(
        lowest > -0.005,
        "the weight does not sag after the switch: lowest angle {lowest} rad"
    );
    let integral = own_number(&runtime, &own_law(&runtime, program), "IntegralNm");
    assert!(
        (integral - HOLDING_TORQUE).abs() < 0.05,
        "the integral term still carries the weight: {integral} N·m"
    );
}

#[test]
fn own_pid_law_started_in_auto_without_its_integral_term_drops_the_weight() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    // A start in Auto from the compiled state: the integral term is still zero.
    start_in_auto(&mut runtime, &debug, program, (30.0, 20.0, 10.0), None);
    step(&mut runtime, &mut simulation, &debug, 0);
    assert!(
        number(&runtime, "%QL0").abs() < 0.5,
        "without the integral term the first torque is only P + D: {} N·m",
        number(&runtime, "%QL0")
    );
    let mut lowest = f64::INFINITY;
    for update in 1..100 {
        step(&mut runtime, &mut simulation, &debug, update);
        lowest = lowest.min(number(&runtime, "%IL0"));
    }
    assert!(
        lowest < -0.02,
        "the weight pulls the axis down until P and I catch up: lowest angle {lowest} rad"
    );
}

#[test]
fn own_pid_law_brakes_the_drum_and_removes_torque_at_the_travel_limit() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    step(&mut runtime, &mut simulation, &debug, 0);
    // After the hot reset, Manual drives the axis up with 10 N·m against the weight.
    debug.enqueue_io_write(
        IoAddress::parse("%ML8").unwrap(),
        Value::LWord(10.0_f64.to_bits()),
    );
    let mut aborted_at = None;
    for update in 1..200 {
        step(&mut runtime, &mut simulation, &debug, update);
        if value(&runtime, &format!("{program}.SafetyAbort")) == Some(Value::Bool(true)) {
            aborted_at.get_or_insert(update);
            assert_eq!(number(&runtime, "%QL0"), 0.0, "an abort removes the torque");
            assert_eq!(
                number(&runtime, "%QL8"),
                0.0,
                "the brake carries the weight"
            );
        } else {
            assert_eq!(
                number(&runtime, "%QL8"),
                HOLDING_TORQUE,
                "until the abort the motor carries the weight"
            );
        }
    }
    let aborted_at = aborted_at.expect("leaving the ±2 rad travel range must abort");
    assert!(
        number(&runtime, "%IL0") > 2.0,
        "the abort follows the angle leaving the travel range (at update {aborted_at})"
    );
}
