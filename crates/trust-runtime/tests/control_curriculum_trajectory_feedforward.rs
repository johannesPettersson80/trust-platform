//! Lesson 61, Trajectory feedforward: `Control.PID` follows a planned motion profile on the drum axis and adds the
//! torque of the plan, Kv·ṙ + Ka·r̈, that Control Studio designs from the model.
use trust_runtime::debug::DebugControl;
use trust_runtime::memory::InstanceId;
use trust_runtime::simulation::SimulationController;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

mod control_heater_support;
use control_heater_support::*;

const PROJECT: &str = "09-industrial-control-structures/90-trajectory-feedforward";
const CYCLE_S: f64 = 0.02;

fn step(
    runtime: &mut Runtime,
    simulation: &mut SimulationController,
    debug: &DebugControl,
    update: u64,
) {
    scan_every(runtime, simulation, debug, update * 20, 20, &[]);
}

fn lreal(value: f64) -> Value {
    Value::LWord(value.to_bits())
}

fn aborted(runtime: &Runtime, program: InstanceId) -> bool {
    runtime.storage().get_instance_var(program, "SafetyAbort") == Some(&Value::Bool(true))
}

/// Auto at rest with the given PID and feedforward gains, then a 1 rad setpoint: per scan after the setpoint, the time,
/// the planned angle, rate and acceleration, the measured angle and the torque, over 8 s.
fn move_one_radian(gains: [f64; 3], feedforward: [f64; 2]) -> (Vec<[f64; 6]>, bool) {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for update in 0..10 {
        step(&mut runtime, &mut simulation, &debug, update);
    }
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[
            ("ProportionalGain", gains[0]),
            ("IntegralGain", gains[1]),
            ("DerivativeGain", gains[2]),
        ],
    );
    debug.enqueue_instance_write(program, "RateGain", Value::LReal(feedforward[0]));
    debug.enqueue_instance_write(program, "AccelerationGain", Value::LReal(feedforward[1]));
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    step(&mut runtime, &mut simulation, &debug, 10);
    let mut samples = Vec::new();
    let mut stopped = false;
    for index in 0..400u64 {
        let writes = if index == 0 {
            vec![("%ML0", lreal(1.0))]
        } else {
            Vec::new()
        };
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            (11 + index) * 20,
            20,
            &writes,
        );
        samples.push([
            index as f64 * CYCLE_S,
            program_number(&runtime, program, "ReferenceRad"),
            program_number(&runtime, program, "ReferenceVelocityRadS"),
            program_number(&runtime, program, "ReferenceAccelerationRadS2"),
            number(&runtime, "%IL0"),
            number(&runtime, "%QL0"),
        ]);
        stopped |= aborted(&runtime, program);
    }
    (samples, stopped)
}

const PID: [f64; 3] = [18.77, 17.46, 7.27];

#[test]
fn trajectory_feedforward_rests_at_the_hot_reset() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    for update in 0..100 {
        step(&mut runtime, &mut simulation, &debug, update);
        assert_eq!(number(&runtime, "%QL0"), 0.0, "Manual holds zero torque");
        assert!(
            number(&runtime, "%IL0").abs() <= 0.001 + 1e-12,
            "the axis rests at 0 rad within the encoder's ±0.001 rad"
        );
    }
}

#[test]
fn trajectory_feedforward_plans_a_trapezoidal_move() {
    let (samples, _) = move_one_radian(PID, [1.0, 1.0]);
    // 1 rad at 1 rad/s and 2 rad/s²: accelerating to 0.5 s, cruising to 1.0 s, braking to 1.5 s. The move starts from the
    // planned angle where the axis rested when Auto began, which includes that scan's encoder noise.
    let start = samples[0][1];
    let distance = 1.0 - start;
    // Braking lasts the final 0.5 s of D/V + V/A = D + 0.5 s: at 1.2 s, D − 0.7 s remain.
    let braking = distance - 0.7;
    for (index, travelled, rate, acceleration) in [
        (10, 0.04, 0.4, 2.0),
        (30, 0.35, 1.0, 0.0),
        (60, distance - braking * braking, 2.0 * braking, -2.0),
        (80, distance, 0.0, 0.0),
    ] {
        let [time, planned, planned_rate, planned_acceleration, ..] = samples[index];
        let angle = start + travelled;
        assert!((planned - angle).abs() < 1e-9 && (planned_rate - rate).abs() < 1e-9 && (planned_acceleration - acceleration).abs() < 1e-9,
            "at {time} s the plan is {planned} rad, {planned_rate} rad/s, {planned_acceleration} rad/s² (expected {angle}, {rate}, {acceleration})");
    }
}

#[test]
fn trajectory_feedforward_adds_the_torque_of_the_plan() {
    // Without feedback the torque is the feedforward alone: Kv·ṙ + Ka·r̈ = 0.4 + 2 at 0.2 s.
    let (samples, _) = move_one_radian([0.0, 0.0, 0.0], [1.0, 1.0]);
    let [time, _, rate, acceleration, _, torque] = samples[10];
    assert!(
        (torque - (rate + acceleration)).abs() < 1e-9 && (torque - 2.4).abs() < 1e-9,
        "torque {torque} N·m at {time} s"
    );
}

#[test]
fn trajectory_feedforward_cuts_the_following_error() {
    let measure = |feedforward: [f64; 2]| {
        let (samples, stopped) = move_one_radian(PID, feedforward);
        assert!(!stopped, "the move stays inside the travel");
        let following = samples
            .iter()
            .filter(|sample| sample[0] <= 1.5)
            .map(|sample| (sample[1] - sample[4]).abs())
            .fold(0.0, f64::max);
        let settled = samples
            .iter()
            .filter(|sample| (sample[4] - 1.0).abs() > 0.02)
            .map(|sample| sample[0])
            .fold(0.0, f64::max);
        (following, settled)
    };
    let (alone, alone_settled) = measure([0.0, 0.0]);
    let (with, with_settled) = measure([1.0, 1.0]);
    assert!(
        alone > 0.06 && alone_settled > 2.0,
        "the PID alone lags the plan by {alone} rad and settles from {alone_settled} s"
    );
    assert!(
        with < 0.03 && with_settled < 1.6,
        "with the feedforward the lag is {with} rad and the axis settles from {with_settled} s"
    );
}

#[test]
fn trajectory_feedforward_removes_torque_outside_the_travel() {
    let (mut runtime, mut simulation) = setup(PROJECT);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    step(&mut runtime, &mut simulation, &debug, 0);
    debug.enqueue_io_write(
        trust_runtime::io::IoAddress::parse("%ML8").unwrap(),
        lreal(5.0),
    );
    let mut stopped = false;
    for update in 1..200 {
        step(&mut runtime, &mut simulation, &debug, update);
        if aborted(&runtime, program) {
            stopped = true;
            assert_eq!(
                number(&runtime, "%QL0"),
                0.0,
                "the abort removes the torque"
            );
        }
    }
    assert!(stopped, "an angle beyond 1.8 rad aborts");
}
