use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON37: &str = "03-production-quality-digital-pid/37-discrete-integral-derivative";

/// One recorded controller update, in the program's positional reading.
#[derive(Clone, Copy, Debug)]
struct Update {
    filtered: f64,
    error: f64,
    proportional: f64,
    trial: f64,
    derivative: f64,
    request: f64,
    torque: f64,
    integral: f64,
}

/// The axis rests for 0.5 s; then Control Studio's project experiment starts (activation at %MX88.0).
fn lesson37_experiment(seconds: f64) -> Vec<Update> {
    let (mut runtime, mut simulation) = setup(LESSON37);
    let debug = runtime.enable_debug();
    for step in 0..25 {
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
    }
    debug.enqueue_io_write(IoAddress::parse("%MX88.0").unwrap(), Value::Bool(true));
    let mut updates = Vec::new();
    for step in 25..(25 + (seconds / 0.02).round() as u64) {
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
        updates.push(Update {
            filtered: number(&runtime, "%ML32"),
            error: number(&runtime, "%ML40"),
            proportional: number(&runtime, "%ML48"),
            trial: number(&runtime, "%ML56"),
            derivative: number(&runtime, "%ML64"),
            request: number(&runtime, "%ML72"),
            torque: number(&runtime, "%QL0"),
            integral: number(&runtime, "%ML80"),
        });
    }
    updates
}

#[test]
fn lesson37_every_update_follows_the_discrete_order() {
    let updates = lesson37_experiment(3.0);
    // The angle carries ±0.001 rad of noise, so P differs from 16 · 1.5 by at most 0.016 N·m.
    assert!(
        (updates[0].proportional - 24.0).abs() < 0.02,
        "the first update sees the 1.5 rad target: P = 16 · 1.5 = 24 N·m, got {}",
        updates[0].proportional
    );
    assert_eq!(
        updates[0].torque, 20.0,
        "the request exceeds the drive: the applied torque is clamped at 20 N·m"
    );
    for pair in updates.windows(2) {
        let (previous, current) = (pair[0], pair[1]);
        assert!(
            (current.proportional - 16.0 * current.error).abs() < 1e-9,
            "P = Kp·e: {current:?}"
        );
        assert!(
            (current.trial - (previous.integral + 4.0 * current.error * 0.02)).abs() < 1e-9,
            "I* = I + Ki·e·Δt: {current:?}"
        );
        assert!(
            (current.derivative + 7.0 * (current.filtered - previous.filtered) / 0.02).abs() < 1e-9,
            "D = −Kd·ω_f: {current:?}"
        );
        assert!(
            (current.request - (current.proportional + current.trial + current.derivative)).abs()
                < 1e-9,
            "u_r = P + I* + D: {current:?}"
        );
        assert!(
            (current.torque - current.request.clamp(-20.0, 20.0)).abs() < 1e-9,
            "u_a = clamp(u_r): {current:?}"
        );
        assert!(
            (current.integral - (current.trial + 0.02 / 1.0 * (current.torque - current.request)))
                .abs()
                < 1e-9,
            "I = I* + (Δt/Tt)(u_a − u_r): {current:?}"
        );
    }
    assert!(
        updates
            .iter()
            .filter(|update| update.torque == 20.0)
            .count()
            >= 2,
        "the clamp and the tracking correction act for several updates"
    );
}

#[test]
fn lesson37_the_integral_takes_over_the_drum_load() {
    let updates = lesson37_experiment(10.0);
    let last = updates.last().unwrap();
    assert!(
        (last.filtered - 1.5).abs() < 0.02,
        "the axis holds 1.5 rad: {}",
        last.filtered
    );
    assert!(
        (last.integral - 1.962).abs() < 0.1,
        "the retained integral carries the 1.962 N·m load: {}",
        last.integral
    );
}

const LESSON38: &str = "03-production-quality-digital-pid/38-positional-incremental-pid";

/// One update of Lesson 39: Control.PID's torque, its retained request, and the forms calculated from the same inputs.
#[derive(Clone, Copy, Debug)]
struct Forms {
    torque: f64,
    retained: f64,
    positional: f64,
    retained_applied: f64,
    unpreloaded: f64,
    error: f64,
    proportional: f64,
    derivative: f64,
    integral: f64,
}

/// 0.5 s at rest in Manual, then Control Studio's project experiment (activation at %MX88.0). Returns every update
/// and the index of the first experiment update.
fn lesson38_experiment(seconds: f64) -> (Vec<Forms>, usize) {
    let (mut runtime, mut simulation) = setup(LESSON38);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    let mut updates = Vec::new();
    for step in 0..(25 + (seconds / 0.02).round() as u64) {
        if step == 25 {
            debug.enqueue_io_write(IoAddress::parse("%MX88.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
        updates.push(Forms {
            torque: number(&runtime, "%QL0"),
            retained: controller_state(&runtime, program, "NominalOutput"),
            positional: number(&runtime, "%ML96"),
            retained_applied: number(&runtime, "%ML104"),
            unpreloaded: number(&runtime, "%ML112"),
            error: number(&runtime, "%ML40"),
            proportional: number(&runtime, "%ML48"),
            derivative: number(&runtime, "%ML64"),
            integral: number(&runtime, "%ML80"),
        });
    }
    (updates, 25)
}

#[test]
fn lesson38_the_positional_form_repeats_every_torque_of_control_pid() {
    let (updates, start) = lesson38_experiment(4.0);
    assert_eq!(
        updates[start].torque, 20.0,
        "the Auto entry with the 1.5 rad target requests more than the drive: clamped at 20 N·m"
    );
    assert!(
        updates[start..]
            .iter()
            .take_while(|update| update.torque == 20.0)
            .count()
            >= 2,
        "the clamp holds for several updates"
    );
    for (index, update) in updates.iter().enumerate() {
        assert!(
            (update.positional - update.torque).abs() < 1e-9,
            "update {index}: the positional form gives {} N·m, Control.PID {} N·m",
            update.positional,
            update.torque
        );
        assert!((update.proportional + update.integral + update.derivative - update.retained).abs() < 1e-9,
            "update {index}: P + I + D of the positional form is the request Control.PID retains: {update:?}");
    }
}

#[test]
fn lesson38_retaining_the_applied_torque_changes_the_controller() {
    let (updates, start) = lesson38_experiment(4.0);
    for (index, update) in updates[..=start].iter().enumerate() {
        assert!(
            (update.retained_applied - update.torque).abs() < 1e-9,
            "update {index}: identical until the first clamped update: {update:?}"
        );
    }
    // After the clamp both forms change by the same increments, so the excess lost at the limit stays lost.
    let clamped = updates[start..]
        .iter()
        .take_while(|update| update.torque == 20.0)
        .count();
    let after = &updates[start + clamped..];
    let offset = after[0].torque - after[0].retained_applied;
    assert!(
        offset > 1.0,
        "retaining the applied torque gives a lower torque after the clamp: {offset}"
    );
    for update in after {
        assert!(
            (update.torque - update.retained_applied - offset).abs() < 1e-9,
            "the offset stays {offset}: {update:?}"
        );
    }
}

#[test]
fn lesson38_a_gain_revision_moves_proportional_torque_into_the_integral() {
    let (updates, start) = lesson38_experiment(4.0);
    let revision = start + 50;
    let (before, at) = (updates[revision - 1], updates[revision]);
    assert!(
        (before.proportional - 16.0 * before.error).abs() < 1e-9,
        "Kp = 16 before the revision: {before:?}"
    );
    assert!(
        (at.proportional - 24.0 * at.error).abs() < 1e-9,
        "Kp = 24 from the revision on, 1 s after the step: {at:?}"
    );
    // The positional integral takes over (Kp,old − Kp,new)·e of the previous update; without that, the torque jumps.
    let jump = (24.0 - 16.0) * before.error;
    assert!(
        jump.abs() > 0.2,
        "the revision happens at a visible error: {jump}"
    );
    assert!(
        (at.integral - (before.integral - jump + 4.0 * at.error * 0.02)).abs() < 1e-9,
        "I = I − ΔKp·e + Ki·e·Δt: {at:?}"
    );
    for update in &updates[..revision] {
        assert!(
            (update.unpreloaded - update.torque).abs() < 1e-9,
            "identical before the revision: {update:?}"
        );
    }
    for update in &updates[revision..] {
        assert!(
            (update.unpreloaded - update.torque - jump).abs() < 1e-9,
            "without the preload the torque stays {jump} N·m off: {update:?}"
        );
    }
}

const LESSON39: &str = "14-integrated-capstones/82-servo";

fn lreal(value: f64) -> Value {
    Value::LWord(value.to_bits())
}

fn program_flag(
    runtime: &trust_runtime::Runtime,
    program: trust_runtime::memory::InstanceId,
    name: &str,
) -> bool {
    match runtime.storage().get_instance_var(program, name) {
        Some(Value::Bool(value)) => *value,
        other => panic!("{name}: {other:?}"),
    }
}

/// Control Studio's verification start: Control.PID preset at rest at 0 rad with zero torque, the program marked as
/// initialized, the commissioning candidate's integral gain and β = 0.3 written, and Auto as given.
fn lesson39_verification(
    auto: bool,
    integral_gain: f64,
) -> (
    trust_runtime::Runtime,
    trust_runtime::simulation::SimulationController,
    trust_runtime::debug::DebugControl,
    trust_runtime::memory::InstanceId,
) {
    let (mut runtime, simulation) = setup(LESSON39);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    set_controller_state(
        &mut runtime,
        program,
        &[
            ("Setpoint", 0.0),
            ("SetpointRate", 0.0),
            ("ProcessValue", 0.0),
            ("ProcessValueRate", 0.0),
            ("FeedForward", 0.0),
            ("NominalOutput", 0.0),
            ("SaturatedOutput", 0.0),
        ],
    );
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[
            ("IntegralGain", integral_gain),
            ("ProportionalSetpointWeight", 0.3),
        ],
    );
    for (name, value) in [("Initialized", true), ("AutoMode", auto)] {
        assert!(
            runtime
                .storage_mut()
                .set_instance_var(program, name, Value::Bool(value)),
            "{name}"
        );
    }
    (runtime, simulation, debug, program)
}

#[test]
fn lesson39_the_installed_axis_runs_the_lesson_38_controller() {
    let (mut runtime, mut simulation) = setup(LESSON39);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..5 {
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
    }
    for (field, expected) in [
        ("ProportionalGain", 16.0),
        ("IntegralGain", 4.0),
        ("DerivativeGain", 7.0),
        ("ProportionalSetpointWeight", 1.0),
    ] {
        assert_eq!(
            program_record_number(&runtime, program, "ConfiguredParameters", field),
            expected,
            "{field}"
        );
    }
    assert_eq!(
        program_number(&runtime, program, "ConfiguredFilterTimeS"),
        0.1,
        "the Lesson 37 angle filter is configured with the gains"
    );
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "the axis rests in Manual without torque"
    );
}

#[test]
fn lesson39_commissions_the_candidate_at_one_boundary() {
    let (mut runtime, mut simulation) = setup(LESSON39);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..100 {
        if step == 25 {
            debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
    }
    let revision = program_dint(&runtime, program, "ParameterRevision");
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("IntegralGain", -1.0)],
    );
    scan_every(&mut runtime, &mut simulation, &debug, 100 * 20, 20, &[]);
    assert!(
        program_flag(&runtime, program, "ParametersRejected"),
        "a negative integral gain rejects the whole set"
    );
    assert_eq!(
        program_dint(&runtime, program, "ParameterRevision"),
        revision,
        "an invalid set changes nothing"
    );
    assert_eq!(
        program_record_number(&runtime, program, "ConfiguredParameters", "IntegralGain"),
        4.0
    );
    let before = number(&runtime, "%QL0");
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("IntegralGain", 12.0), ("ProportionalSetpointWeight", 0.3)],
    );
    scan_every(&mut runtime, &mut simulation, &debug, 101 * 20, 20, &[]);
    assert!(!program_flag(&runtime, program, "ParametersRejected"));
    assert_eq!(
        program_dint(&runtime, program, "ParameterRevision"),
        revision + 1,
        "the candidate is adopted at one boundary"
    );
    for (field, expected) in [("IntegralGain", 12.0), ("ProportionalSetpointWeight", 0.3)] {
        assert_eq!(
            program_record_number(&runtime, program, "ConfiguredParameters", field),
            expected,
            "{field}"
        );
    }
    let after = number(&runtime, "%QL0");
    assert!(
        (after - before).abs() < 0.5,
        "the torque continues across the update: {before} → {after}"
    );
}

#[test]
fn lesson39_automatic_control_needs_integral_action() {
    let (mut runtime, mut simulation, debug, _program) = lesson39_verification(true, 0.0);
    for step in 0..10 {
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 20,
            20,
            &[("%ML0", lreal(1.0))],
        );
    }
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "without integral action the axis stays in Manual at zero torque"
    );
}

#[test]
fn lesson39_the_transfer_from_manual_keeps_the_holding_torque() {
    let (mut runtime, mut simulation, debug, program) = lesson39_verification(false, 12.0);
    debug.enqueue_instance_write(program, "LoadTorqueNm", Value::LReal(1.962));
    let mut record = Vec::new();
    for step in 0..300 {
        if step == 100 {
            debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
        }
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 20,
            20,
            &[("%ML8", lreal(1.962))],
        );
        record.push((number(&runtime, "%QL0"), number(&runtime, "%IL0")));
    }
    assert_eq!(record[99].0, 1.962, "Manual holds the load");
    assert_eq!(
        record[100].0, 1.962,
        "the transfer scan reconstructs the applied torque"
    );
    assert!(
        record[100..]
            .iter()
            .all(|(torque, _)| (torque - 1.962).abs() < 0.5),
        "no bump after the transfer"
    );
    assert!(
        record.iter().all(|(_, angle)| angle.abs() < 0.01),
        "the axis stays at its target"
    );
}

#[test]
fn lesson39_verifies_the_candidate_from_the_state_control_studio_presets() {
    let (mut runtime, mut simulation, debug, _program) = lesson39_verification(true, 12.0);
    let mut torques = Vec::new();
    for step in 0..3 {
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 20,
            20,
            &[("%ML0", lreal(1.0))],
        );
        torques.push(number(&runtime, "%QL0"));
    }
    // The first update adds Kp·β·Δr + Ki·e·Δt = 16 · 0.3 · 1 + 12 · 1 · 0.02 = 5.04 N·m to the preset zero torque.
    assert!(
        (torques[0] - 5.04).abs() < 0.2,
        "the candidate's setpoint kick: {torques:?}"
    );
}

/// The loaded hold with Control Studio's measurement-fault events, by update index: the encoder freezes at `freeze`
/// and delivers again at `resume`, the fault is acknowledged at `acknowledge`, and Auto is requested at `restart`.
/// Returns torque, angle and the latched fault for every update.
fn lesson39_fault_trial(
    freeze: u64,
    resume: u64,
    acknowledge: u64,
    restart: u64,
    updates: u64,
) -> Vec<(f64, f64, bool)> {
    let (mut runtime, mut simulation, debug, program) = lesson39_verification(true, 12.0);
    debug.enqueue_instance_write(program, "LoadTorqueNm", Value::LReal(1.962));
    let mut record = Vec::new();
    for step in 0..updates {
        for (at, name, value) in [
            (freeze, "EncoderFrozen", true),
            (resume, "EncoderFrozen", false),
            (acknowledge, "FaultAcknowledge", true),
            (restart, "AutoMode", true),
        ] {
            if step == at {
                debug.enqueue_instance_write(program, name, Value::Bool(value));
            }
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
        record.push((
            number(&runtime, "%QL0"),
            number(&runtime, "%IL0"),
            program_flag(&runtime, program, "MeasurementFault"),
        ));
    }
    record
}

#[test]
fn lesson39_a_stale_encoder_removes_the_torque_until_a_deliberate_restart() {
    let record = lesson39_fault_trial(200, 205, 205, 210, 500);
    assert!(
        (record[199].0 - 1.962).abs() < 0.5,
        "the torque holds the load before the fault: {}",
        record[199].0
    );
    // Frozen from update 200, the reading stays usable while it is at most 0.04 s old.
    assert!(
        !record[201].2 && record[202].2,
        "the fault latches when the reading is older than 0.04 s: {:?}",
        &record[199..204]
    );
    for (index, update) in record[202..210].iter().enumerate() {
        assert_eq!(
            update.0,
            0.0,
            "update {}: no torque until Auto is requested again",
            202 + index
        );
    }
    assert!(
        record[206].2 && !record[207].2,
        "acknowledged, three fresh readings recover the measurement at update 207"
    );
    let lowest = record
        .iter()
        .map(|update| update.1)
        .fold(f64::INFINITY, f64::min);
    assert!(
        lowest < -0.005,
        "without torque the load pulls the axis down: {lowest}"
    );
    assert!(
        record[400..].iter().all(|update| update.1.abs() < 0.02),
        "the restarted controller holds within 0.02 rad from 8 s"
    );
}

#[test]
fn lesson39_fresh_readings_alone_do_not_restart_the_axis() {
    let record = lesson39_fault_trial(200, 205, 230, 240, 300);
    assert!(
        record[202..232]
            .iter()
            .all(|update| update.2 && update.0 == 0.0),
        "without an acknowledgement the fault stays latched"
    );
    assert!(
        !record[232].2,
        "acknowledged at update 230: recovered after three fresh readings"
    );
    assert!(
        record[232..240].iter().all(|update| update.0 == 0.0),
        "recovered in Manual at zero torque until Auto is requested"
    );
    assert!(
        record[241].0 > 0.0,
        "the new Auto request resumes control: {}",
        record[241].0
    );
}
