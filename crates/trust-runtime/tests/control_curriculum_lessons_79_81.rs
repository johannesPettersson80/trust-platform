use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

fn flag(runtime: &trust_runtime::Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Bool(value) => value,
        other => panic!("expected BOOL at {address}, got {other:?}"),
    }
}

const LESSON79: &str = "12-nonlinear-adaptive-and-robust-control/77-robust-control";

/// Lesson 86: the nominal heater, 10 kW/°C with 1.0 s delivery delay, under the real Control.PID whose gains Control Studio
/// tunes, validates on the corner heaters of the range and applies. The program starts from candidate A, Kp = 2.5 %/°C and
/// Ki = 0.5 %/(°C·s).
#[test]
fn lesson79_the_nominal_heater_rests_at_35_c_with_75_percent_under_candidate_a() {
    let (mut runtime, mut simulation) = setup(LESSON79);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    for step in 0..10 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
        assert_eq!(
            number(&runtime, "%QL0"),
            75.0,
            "Manual holds the holding command 7.5 · 10 = 75 %"
        );
        assert!(
            (number(&runtime, "%IL8") - 35.0).abs() < 1e-9,
            "the nominal heater rests at 35 °C"
        );
    }
    assert_eq!(
        program_record_number(&runtime, program, "Parameters", "ProportionalGain"),
        2.5
    );
    assert_eq!(
        program_record_number(&runtime, program, "Parameters", "IntegralGain"),
        0.5
    );
    assert!(
        (controller_state(&runtime, program, "SaturatedOutput") - 75.0).abs() < 1e-9,
        "Control.PID holds 75 %"
    );
}

/// Runs the nominal heater in Auto for 60 s after the event and returns the highest and lowest temperature and the largest
/// distance from the setpoint from 30 s to 60 s.
fn lesson79_event(kp: f64, ki: f64, setpoint: f64, extraction_kw: f64) -> (f64, f64, f64) {
    let (mut runtime, mut simulation) = setup(LESSON79);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    scan(&mut runtime, &mut simulation, &debug, 0, &[]);
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("ProportionalGain", kp), ("IntegralGain", ki)],
    );
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    debug.enqueue_instance_write(program, "DisturbanceKw", Value::LReal(extraction_kw));
    let (mut highest, mut lowest, mut late) = (f64::MIN, f64::MAX, 0.0_f64);
    for step in 1..=600 {
        let writes = if step == 1 {
            vec![("%ML0", Value::LWord(setpoint.to_bits()))]
        } else {
            vec![]
        };
        scan(&mut runtime, &mut simulation, &debug, step * 100, &writes);
        let temperature = number(&runtime, "%IL8");
        highest = highest.max(temperature);
        lowest = lowest.min(temperature);
        if step >= 300 {
            late = late.max((temperature - setpoint).abs());
        }
    }
    (highest, lowest, late)
}

#[test]
fn lesson79_candidate_a_meets_both_requirements_on_the_nominal_heater() {
    let (highest, _, late) = lesson79_event(2.5, 0.5, 36.0, 0.0);
    assert!(
        highest <= 37.0,
        "setpoint step: peak {highest} °C must stay at or below 37 °C"
    );
    assert!(
        late <= 0.2,
        "setpoint step: {late} °C from 30 s to 60 s must stay within 0.2 °C"
    );
    let (_, lowest, late) = lesson79_event(2.5, 0.5, 35.0, 10.0);
    assert!(
        lowest >= 33.0,
        "10 kW extraction: lowest {lowest} °C must stay at or above 33 °C"
    );
    assert!(
        late <= 0.2,
        "10 kW extraction: {late} °C from 30 s to 60 s must stay within 0.2 °C"
    );
}

#[test]
fn lesson79_candidate_b_applied_by_control_studio_asks_for_more_than_the_heater_has() {
    let (mut runtime, mut simulation) = setup(LESSON79);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    scan(&mut runtime, &mut simulation, &debug, 0, &[]);
    set_program_record(
        &mut runtime,
        program,
        "Parameters",
        &[("ProportionalGain", 30.0), ("IntegralGain", 6.0)],
    );
    for step in 1..4 {
        scan(&mut runtime, &mut simulation, &debug, step * 100, &[]);
        assert_eq!(
            program_record_number(&runtime, program, "Parameters", "ProportionalGain"),
            30.0,
            "cyclic code must preserve the gains Control Studio applied"
        );
    }
    debug.enqueue_instance_write(program, "AutoMode", Value::Bool(true));
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        400,
        &[("%ML0", Value::LWord(36.0_f64.to_bits()))],
    );
    assert_eq!(
        number(&runtime, "%QL0"),
        100.0,
        "75 % + 30 · 1 °C exceeds the heater's 100 %: the command saturates"
    );
}

#[test]
fn lesson79_an_invalid_measurement_latches_zero_heat_and_ends_the_extraction() {
    let (mut runtime, mut simulation) = setup(LESSON79);
    let debug = runtime.enable_debug();
    let program = program(&runtime);
    scan(&mut runtime, &mut simulation, &debug, 0, &[]);
    debug.enqueue_instance_write(program, "DisturbanceKw", Value::LReal(10.0));
    scan(
        &mut runtime,
        &mut simulation,
        &debug,
        100,
        &[("%IL8", Value::LWord(1000.0_f64.to_bits()))],
    );
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "an invalid measurement removes heat"
    );
    assert_eq!(number(&runtime, "%QL8"), 0.0, "and ends the extraction");
    assert!(flag(&runtime, "%MX96.3"), "the abort latches");
    scan(&mut runtime, &mut simulation, &debug, 200, &[]);
    assert_eq!(
        number(&runtime, "%QL0"),
        0.0,
        "the latched abort keeps the heat off with a valid measurement"
    );
}

const LESSON80: &str = "13-networked-fault-aware-and-diagnostic-control/78-networked-control";

/// The six network cases in the program's order. Every case is the nominal heater, 10 kW/°C with 1 s delivery delay, under a
/// remote Control.PID whose commands travel as packets, one per 100 ms update, normally arriving one update later.
const DROPPED: usize = 0;
const REORDERED: usize = 1;
const LATE: usize = 2;
const OLD_SESSION: usize = 3;
const OUTAGE: usize = 4;
const RECOVERY: usize = 5;

/// One 100 ms update of the six heaters: temperatures, applied commands, what each case's remote controller sent and what its
/// receiver holds, the local Control.PID's output, and the published results.
#[derive(Clone, Debug)]
struct Networked {
    temperature: [f64; 6],
    command: [f64; 6],
    sent: [f64; 6],
    remote: [bool; 6],
    hold_ms: [u64; 6],
    sequence: [u64; 6],
    old_session: [u64; 6],
    not_newer: [u64; 6],
    by_age: [u64; 6],
    local_output: [f64; 6],
    setpoint: f64,
    dropped_hold_s: f64,
    late_hold_s: f64,
    outage_hold_s: f64,
    reordered_rejected: f64,
    late_rejected: f64,
    old_session_rejected: f64,
    step_passed: f64,
    largest_jump: f64,
    highest_peak: f64,
    largest_step_error: f64,
    recovery_fresh: f64,
    late_age_s: f64,
    at_rest: bool,
    fault: bool,
    outage_remote: bool,
    recovery_remote: bool,
}

fn member(
    runtime: &trust_runtime::Runtime,
    instance: trust_runtime::memory::InstanceId,
    name: &str,
) -> Value {
    runtime
        .storage()
        .get_instance_var(instance, name)
        .cloned()
        .unwrap_or_else(|| panic!("missing member {name}"))
}

fn member_instance(
    runtime: &trust_runtime::Runtime,
    instance: trust_runtime::memory::InstanceId,
    name: &str,
) -> trust_runtime::memory::InstanceId {
    match member(runtime, instance, name) {
        Value::Instance(id) => id,
        other => panic!("{name} is not an instance: {other:?}"),
    }
}

fn networked_case(
    runtime: &trust_runtime::Runtime,
    program: trust_runtime::memory::InstanceId,
    case: usize,
) -> trust_runtime::memory::InstanceId {
    let Some(Value::Array(cases)) = runtime.storage().get_instance_var(program, "Cases") else {
        panic!("the lesson must keep its six cases");
    };
    let Value::Instance(heater) = cases.elements()[case].clone() else {
        panic!("case {case} is not an instance")
    };
    heater
}

fn lreal(value: &Value) -> f64 {
    match value {
        Value::LReal(value) => *value,
        other => panic!("expected LREAL, got {other:?}"),
    }
}

fn udint(value: &Value) -> u64 {
    match value {
        Value::UDInt(value) => u64::from(*value),
        other => panic!("expected UDINT, got {other:?}"),
    }
}

fn boolean(value: &Value) -> bool {
    match value {
        Value::Bool(value) => *value,
        other => panic!("expected BOOL, got {other:?}"),
    }
}

fn record_field(value: &Value, field: &str) -> Value {
    match value {
        Value::Struct(record) => record
            .field(field)
            .cloned()
            .unwrap_or_else(|| panic!("missing field {field}")),
        other => panic!("expected a record, got {other:?}"),
    }
}

/// Sets the acknowledgement time as Control Studio's declared experiment setting does, runs one update at rest, then the
/// project experiment (activation at %MX112.0), one update per entry of `periods_ms`. Each `(update, address, value)` replaces
/// that input for one update, counted from 0.
fn lesson80_run(
    acknowledge_s: f64,
    periods_ms: &[u64],
    readings: &[(usize, &str, f64)],
) -> Vec<Networked> {
    let (mut runtime, mut simulation) = setup(LESSON80);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    assert!(runtime.storage_mut().set_instance_var(
        p1,
        "AcknowledgementTime",
        Value::LReal(acknowledge_s)
    ));
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&100).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX112.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, value)| (*address, Value::LWord(value.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        let heaters: Vec<_> = (0..6)
            .map(|case| networked_case(&runtime, p1, case))
            .collect();
        let receiver = |case: usize, name: &str| {
            member(
                &runtime,
                member_instance(&runtime, heaters[case], "Receiver"),
                name,
            )
        };
        let local = |case: usize| {
            let state = member(
                &runtime,
                member_instance(&runtime, heaters[case], "LocalLoop"),
                "StateValue",
            );
            lreal(&record_field(&state, "SaturatedOutput"))
        };
        record.push(Networked {
            temperature: std::array::from_fn(|case| number(&runtime, &format!("%IL{}", 8 * case))),
            command: std::array::from_fn(|case| number(&runtime, &format!("%QL{}", 8 * case))),
            sent: std::array::from_fn(|case| {
                lreal(&record_field(
                    &member(&runtime, heaters[case], "Sent"),
                    "CommandPct",
                ))
            }),
            remote: std::array::from_fn(|case| boolean(&receiver(case, "RemoteAuthority"))),
            hold_ms: std::array::from_fn(|case| udint(&receiver(case, "HoldMs"))),
            sequence: std::array::from_fn(|case| udint(&receiver(case, "LastSequence"))),
            old_session: std::array::from_fn(|case| udint(&receiver(case, "RejectedOldSession"))),
            not_newer: std::array::from_fn(|case| udint(&receiver(case, "RejectedNotNewer"))),
            by_age: std::array::from_fn(|case| udint(&receiver(case, "RejectedByAge"))),
            local_output: std::array::from_fn(local),
            setpoint: number(&runtime, "%ML0"),
            dropped_hold_s: number(&runtime, "%ML8"),
            late_hold_s: number(&runtime, "%ML16"),
            outage_hold_s: number(&runtime, "%ML24"),
            reordered_rejected: number(&runtime, "%ML32"),
            late_rejected: number(&runtime, "%ML40"),
            old_session_rejected: number(&runtime, "%ML48"),
            step_passed: number(&runtime, "%ML56"),
            largest_jump: number(&runtime, "%ML64"),
            highest_peak: number(&runtime, "%ML72"),
            largest_step_error: number(&runtime, "%ML80"),
            recovery_fresh: number(&runtime, "%ML88"),
            late_age_s: number(&runtime, "%ML96"),
            at_rest: flag(&runtime, "%MX112.1"),
            fault: flag(&runtime, "%MX112.2"),
            outage_remote: flag(&runtime, "%MX112.4"),
            recovery_remote: flag(&runtime, "%MX112.5"),
        });
    }
    record
}

#[test]
fn lesson80_every_heater_rests_under_remote_control_and_the_step_starts_together() {
    // At rest every heater holds 35 °C with 75 %. The experiment's first update establishes connection 7 at 0 ms, starting
    // from that command, gives the remote controller authority and steps the setpoint to 36 °C. The remote's first packet,
    // sequence 0, arrives 100 ms later and is accepted in every case.
    let record = lesson80_run(8.0, &[100; 3], &[]);
    let rest = &record[0];
    assert!(
        rest.temperature.iter().all(|t| (t - 35.0).abs() < 1e-9)
            && rest.command.iter().all(|u| *u == 75.0)
            && rest.setpoint == 35.0,
        "rest: {rest:?}"
    );
    let run = &record[1..];
    let first = &run[0];
    assert!(
        first.setpoint == 36.0 && first.at_rest && first.remote.iter().all(|r| *r),
        "start: {first:?}"
    );
    assert!(
        first.command.iter().all(|u| *u == 75.0) && first.hold_ms.iter().all(|h| *h == 0),
        "the connection starts from the rest command: {first:?}"
    );
    assert!(
        (first.sent[0] - 77.55).abs() < 1e-9,
        "the remote PI's first command: {first:?}"
    );
    for case in 0..6 {
        assert!(
            run[1].sequence[case] == 0
                && run[1].command[case] == first.sent[case]
                && run[1].hold_ms[case] == 0,
            "case {case}: {:?}",
            run[1]
        );
    }
}

#[test]
fn lesson80_every_case_meets_the_step_requirement_through_its_network_faults() {
    // Every heater peaks at most 37 °C and stays within 0.2 °C of 36 °C from 30 s to 60 s, whatever its network did at 5 s.
    // The results appear at 60 s and equal those recalculated from the temperatures.
    let record = lesson80_run(8.0, &[100; 620], &[]);
    let run = &record[1..];
    let peaks: Vec<f64> = (0..6)
        .map(|case| {
            run[..=600]
                .iter()
                .map(|u| u.temperature[case])
                .fold(f64::MIN, f64::max)
        })
        .collect();
    let late: Vec<f64> = (0..6)
        .map(|case| {
            run[300..=600]
                .iter()
                .map(|u| (36.0 - u.temperature[case]).abs())
                .fold(0.0, f64::max)
        })
        .collect();
    assert!(
        peaks.iter().all(|p| *p <= 37.0) && late.iter().all(|e| *e <= 0.2),
        "peaks {peaks:?} late {late:?}"
    );
    let end = &run[600];
    assert!(
        run[599].step_passed == 0.0 && end.step_passed == 6.0,
        "{end:?}"
    );
    assert_eq!(
        end.highest_peak,
        peaks.iter().copied().fold(f64::MIN, f64::max),
        "{end:?}"
    );
    assert_eq!(
        end.largest_step_error,
        late.iter().copied().fold(0.0, f64::max),
        "{end:?}"
    );
}

#[test]
fn lesson80_a_dropped_packet_holds_the_last_command_for_one_update() {
    // Case 0: packet 50 is lost. At 5.1 s nothing arrives: the command accepted at 5.0 s is held for 100 ms, and packet 51 is
    // accepted at 5.2 s.
    let record = lesson80_run(8.0, &[100; 60], &[]);
    let run = &record[1..];
    let c = DROPPED;
    assert!(
        run[51].hold_ms[c] == 100
            && run[51].remote[c]
            && run[51].command[c] == run[50].command[c]
            && run[51].sequence[c] == 49,
        "{:?}",
        run[51]
    );
    assert!(
        run[52].sequence[c] == 51
            && run[52].hold_ms[c] == 0
            && run[52].command[c] == run[51].sent[c],
        "{:?}",
        run[52]
    );
    assert!(
        run.iter()
            .all(|u| u.remote[c] && u.old_session[c] + u.not_newer[c] + u.by_age[c] == 0),
        "no rejection"
    );
    assert!(
        run[51].dropped_hold_s == 0.1 && run[52].dropped_hold_s == 0.0,
        "published: {:?}",
        run[51]
    );
}

#[test]
fn lesson80_a_reordered_packet_is_rejected_and_never_applied() {
    // Case 1: packet 51 arrives at 5.1 s, before packet 50 at 5.2 s. Packet 51 is accepted; packet 50 is not newer and is
    // rejected, so the command from packet 51 is held one update and packet 50's older command is never applied.
    let record = lesson80_run(8.0, &[100; 60], &[]);
    let run = &record[1..];
    let c = REORDERED;
    assert!(
        run[51].sequence[c] == 51 && run[51].command[c] == run[51].sent[c],
        "{:?}",
        run[51]
    );
    assert!(
        run[52].not_newer[c] == 1
            && run[52].sequence[c] == 51
            && run[52].command[c] == run[51].command[c],
        "{:?}",
        run[52]
    );
    assert!(
        run[50].sent[c] != run[51].sent[c],
        "packet 50 carries a different command"
    );
    assert!(
        run.iter()
            .all(|u| u.remote[c] && u.old_session[c] == 0 && u.by_age[c] == 0),
        "remote throughout"
    );
    assert!(
        run[51].reordered_rejected == 0.0 && run[52].reordered_rejected == 1.0,
        "published: {:?}",
        run[52]
    );
}

#[test]
fn lesson80_a_late_packet_is_rejected_by_age_and_the_hold_ends_exactly_at_0_3_s() {
    // Case 2: packet 50, sent at 5.0 s, arrives at 5.3 s after packets 51 and 52 were lost. Its sequence is newer, but its
    // age, 300 ms, exceeds 200 ms: rejected. The command accepted at 5.0 s is held for 100, 200 and 300 ms; 300 ms is not
    // more than 300 ms, so the remote controller keeps authority, and packet 53 is accepted at 5.4 s.
    let record = lesson80_run(8.0, &[100; 60], &[]);
    let run = &record[1..];
    let c = LATE;
    for (update, hold) in [(51, 100), (52, 200), (53, 300)] {
        let u = &run[update];
        assert!(
            u.hold_ms[c] == hold
                && u.remote[c]
                && u.command[c] == run[50].command[c]
                && u.sequence[c] == 49,
            "update {update}: {u:?}"
        );
    }
    assert!(
        run[52].by_age[c] == 0 && run[53].by_age[c] == 1 && run[53].not_newer[c] == 0,
        "{:?}",
        run[53]
    );
    assert!(
        run[54].sequence[c] == 53 && run[54].hold_ms[c] == 0 && run[54].remote[c],
        "{:?}",
        run[54]
    );
    assert!(
        run[53].late_hold_s == 0.3 && run[53].late_rejected == 1.0,
        "published: {:?}",
        run[53]
    );
    assert!(
        run[50].late_age_s == 0.1 && run[51].late_age_s == 0.0 && run[53].late_age_s == 0.3,
        "packet age: {:?} {:?}",
        run[50],
        run[53]
    );
}

#[test]
fn lesson80_an_old_session_packet_is_rejected() {
    // Case 3: at 5.1 s a packet of connection 6 arrives in place of packet 50: sequence 4000, sent at 5.0 s, 0 %. Its age is
    // 100 ms and its sequence larger than any so far; only the connection identity rejects it, and 0 % is never applied.
    let record = lesson80_run(8.0, &[100; 60], &[]);
    let run = &record[1..];
    let c = OLD_SESSION;
    assert!(
        run[51].old_session[c] == 1
            && run[51].sequence[c] == 49
            && run[51].command[c] == run[50].command[c],
        "{:?}",
        run[51]
    );
    assert!(
        run[52].sequence[c] == 51 && run.iter().all(|u| u.command[c] > 70.0),
        "{:?}",
        run[52]
    );
    assert!(
        run[51].old_session_rejected == 1.0,
        "published: {:?}",
        run[51]
    );
}

#[test]
fn lesson80_an_outage_transfers_to_local_control_pid_without_a_jump_and_stays_local() {
    // Case 4: packets 50–58 are lost. The command is held for 100, 200 and 300 ms; at 5.4 s, 400 ms after the last accepted
    // packet, the receiver drops remote authority and the local Control.PID, which tracked the applied command, takes over
    // within 0.1 point. Packets arrive again from 6.0 s and are accepted, but without an acknowledgement control stays local.
    let record = lesson80_run(8.0, &[100; 620], &[]);
    let run = &record[1..];
    let c = OUTAGE;
    assert!(
        run[53].remote[c] && run[53].hold_ms[c] == 300 && run[53].outage_remote,
        "{:?}",
        run[53]
    );
    assert!(
        run[54..=600]
            .iter()
            .all(|u| !u.remote[c] && !u.outage_remote),
        "local from 5.4 s"
    );
    assert!(
        (run[54].command[c] - run[53].command[c]).abs() <= 0.1
            && run[54].command[c] == run[54].local_output[c],
        "{:?} {:?}",
        run[53],
        run[54]
    );
    assert!(
        run[54].outage_hold_s == 0.4 && run[60].sequence[c] == 59 && run[60].hold_ms[c] == 0,
        "{:?}",
        run[60]
    );
    assert!(
        run[600].largest_jump >= (run[54].command[c] - run[53].command[c]).abs()
            && run[600].largest_jump <= 0.1,
        "{:?}",
        run[600]
    );
}

#[test]
fn lesson80_remote_control_returns_only_after_acknowledgement_three_fresh_packets_and_alignment() {
    // Case 5: the same outage; the operator acknowledges at 8.0 s. Packets accepted at 8.0, 8.1 and 8.2 s make three fresh
    // packets, and the remote controller, which tracked the applied command, sends within 0.1 point of it: remote control
    // returns at 8.2 s, not before, without a jump.
    let record = lesson80_run(8.0, &[100; 620], &[]);
    let run = &record[1..];
    let c = RECOVERY;
    assert!(
        run[54..=81]
            .iter()
            .all(|u| !u.remote[c] && !u.recovery_remote),
        "local until 8.2 s"
    );
    assert!(
        run[82..=600]
            .iter()
            .all(|u| u.remote[c] && u.recovery_remote),
        "remote from 8.2 s"
    );
    assert!(
        run[79].recovery_fresh == 0.0
            && run[81].recovery_fresh == 2.0
            && run[82].recovery_fresh == 3.0,
        "{:?} {:?}",
        run[81],
        run[82]
    );
    assert!(
        (run[82].command[c] - run[81].command[c]).abs() <= 0.1,
        "{:?} {:?}",
        run[81],
        run[82]
    );
}

#[test]
fn lesson80_the_acknowledgement_time_is_an_experiment_setting_frozen_at_the_start() {
    // With the acknowledgement at 20 s, control returns at 20.2 s. The setting is read at the start only.
    let record = lesson80_run(20.0, &[100; 250], &[]);
    let run = &record[1..];
    let c = RECOVERY;
    assert!(
        run[54..=201].iter().all(|u| !u.remote[c]) && run[202..].iter().all(|u| u.remote[c]),
        "return at 20.2 s"
    );
}

#[test]
fn lesson80_both_loops_run_control_pid() {
    // Each case's remote and local loops are Control.PID with Lesson 85's gains: Kp 2.5 %/°C, Ki 0.5 %/(°C·s), β = 1,
    // back-calculation with Tt = 1 s, 0–100 %.
    let (mut runtime, mut simulation) = setup(LESSON80);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    scan_every(&mut runtime, &mut simulation, &debug, 0, 100, &[]);
    for case in 0..6 {
        let heater = networked_case(&runtime, p1, case);
        for name in ["RemoteLoop", "LocalLoop"] {
            let parameters = member(
                &runtime,
                member_instance(&runtime, heater, name),
                "ParametersValue",
            );
            let gain = |field: &str| lreal(&record_field(&parameters, field));
            assert!(
                gain("ProportionalGain") == 2.5
                    && gain("IntegralGain") == 0.5
                    && gain("ProportionalSetpointWeight") == 1.0,
                "case {case} {name}"
            );
            assert!(
                gain("TrackingTime") == 1.0
                    && gain("OutputMinimum") == 0.0
                    && gain("OutputMaximum") == 100.0,
                "case {case} {name}"
            );
        }
    }
}

#[test]
fn lesson80_leaving_the_bounds_or_a_wrong_interval_latches_zero_heat() {
    // A temperature outside 0–80 °C or an update interval outside 100 ms ± 10 ms latches zero heat on every heater.
    let stopped = |u: &Networked| u.fault && u.command.iter().all(|pct| *pct == 0.0);
    let hot = lesson80_run(8.0, &[100; 10], &[(6, "%IL8", 81.0)]);
    assert!(
        !hot[5].fault && hot[6..].iter().all(stopped),
        "temperature: {hot:?}"
    );
    let gap = lesson80_run(8.0, &[100, 100, 150, 100], &[]);
    assert!(
        !gap[2].fault && gap[3..].iter().all(stopped),
        "interval: {gap:?}"
    );
}

const LESSON81: &str = "13-networked-fault-aware-and-diagnostic-control/79-fault-aware-control";

/// The six sensor cases in the program's order. Every case is the nominal heater, 10 kW/°C with 1 s delivery delay, measured
/// by two sensors A (primary) and B (monitor) that read its temperature every 100 ms; after 5 s each case's sensors fail.
const STALE: usize = 0;
const RANGE: usize = 1;
const DISAGREE: usize = 2;
const SHORT: usize = 3;
const BOTH: usize = 4;
const RESTORED: usize = 5;

/// One 100 ms update of the six heaters: the true temperatures, commands, and what each case's sensor selection holds, and the
/// published results.
#[derive(Clone, Debug)]
struct Sensed {
    temperature: [f64; 6],
    command: [f64; 6],
    source: [i64; 6],
    selected: [f64; 6],
    faulted: [bool; 6],
    disagreement_ms: [u64; 6],
    agreeing: [u64; 6],
    qualified: [bool; 6],
    setpoint: f64,
    stale_source: f64,
    range_source: f64,
    both_source: f64,
    disagreement_s: f64,
    short_s: f64,
    restored_agreeing: f64,
    stale_selected: f64,
    largest_jump: f64,
    passed: f64,
    highest_peak: f64,
    largest_step_error: f64,
    at_rest: bool,
    disagree_fault: bool,
    both_fault: bool,
    restored_fault: bool,
    restored_qualified: bool,
    clock_fault: bool,
}

fn int(value: &Value) -> i64 {
    match value {
        Value::Int(value) => i64::from(*value),
        other => panic!("expected INT, got {other:?}"),
    }
}

fn sensed_case(
    runtime: &trust_runtime::Runtime,
    program: trust_runtime::memory::InstanceId,
    case: usize,
) -> trust_runtime::memory::InstanceId {
    let Some(Value::Array(cases)) = runtime.storage().get_instance_var(program, "Cases") else {
        panic!("the lesson must keep its six cases");
    };
    let Value::Instance(heater) = cases.elements()[case].clone() else {
        panic!("case {case} is not an instance")
    };
    heater
}

/// Sets the Auto request time as Control Studio's declared experiment setting does, runs one update at rest, then the project
/// experiment (activation at %MX96.0), one update per entry of `periods_ms`.
fn lesson81_run(auto_request_s: f64, periods_ms: &[u64]) -> Vec<Sensed> {
    let (mut runtime, mut simulation) = setup(LESSON81);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    assert!(runtime.storage_mut().set_instance_var(
        p1,
        "AutoRequestTime",
        Value::LReal(auto_request_s)
    ));
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&100).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX96.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &[]);
        now += period;
        let heaters: Vec<_> = (0..6).map(|case| sensed_case(&runtime, p1, case)).collect();
        let selector = |case: usize, name: &str| {
            member(
                &runtime,
                member_instance(&runtime, heaters[case], "Selector"),
                name,
            )
        };
        record.push(Sensed {
            temperature: std::array::from_fn(|case| number(&runtime, &format!("%IL{}", 8 * case))),
            command: std::array::from_fn(|case| number(&runtime, &format!("%QL{}", 8 * case))),
            source: std::array::from_fn(|case| int(&selector(case, "Source"))),
            selected: std::array::from_fn(|case| lreal(&selector(case, "SelectedC"))),
            faulted: std::array::from_fn(|case| boolean(&selector(case, "FaultLatched"))),
            disagreement_ms: std::array::from_fn(|case| udint(&selector(case, "DisagreementMs"))),
            agreeing: std::array::from_fn(|case| udint(&selector(case, "Agreeing"))),
            qualified: std::array::from_fn(|case| boolean(&selector(case, "Qualified"))),
            setpoint: number(&runtime, "%ML0"),
            stale_source: number(&runtime, "%ML8"),
            range_source: number(&runtime, "%ML16"),
            both_source: number(&runtime, "%ML24"),
            disagreement_s: number(&runtime, "%ML32"),
            short_s: number(&runtime, "%ML40"),
            restored_agreeing: number(&runtime, "%ML48"),
            stale_selected: number(&runtime, "%ML56"),
            largest_jump: number(&runtime, "%ML64"),
            passed: number(&runtime, "%ML72"),
            highest_peak: number(&runtime, "%ML80"),
            largest_step_error: number(&runtime, "%ML88"),
            at_rest: flag(&runtime, "%MX96.1"),
            clock_fault: flag(&runtime, "%MX96.2"),
            disagree_fault: flag(&runtime, "%MX96.3"),
            both_fault: flag(&runtime, "%MX96.4"),
            restored_fault: flag(&runtime, "%MX96.5"),
            restored_qualified: flag(&runtime, "%MX96.6"),
        });
    }
    record
}

#[test]
fn lesson81_every_heater_rests_on_its_primary_sensor_and_the_step_starts_together() {
    // At rest every heater holds 35 °C with 75 %, and its selector already reports sensor A with the heater's temperature. At
    // the start the setpoint steps to 36 °C; both sensors are valid and agree, so every case controls on sensor A with its
    // Control.PID.
    let record = lesson81_run(12.0, &[100; 3]);
    let rest = &record[0];
    assert!(
        rest.temperature.iter().all(|t| (t - 35.0).abs() < 1e-9)
            && rest.command.iter().all(|u| *u == 75.0)
            && rest.setpoint == 35.0,
        "rest: {rest:?}"
    );
    assert!(
        rest.source.iter().all(|s| *s == 1)
            && (0..6).all(|case| rest.selected[case] == rest.temperature[case])
            && rest.stale_selected == rest.temperature[0],
        "rest reading: {rest:?}"
    );
    let first = &record[1];
    assert!(
        first.setpoint == 36.0
            && first.at_rest
            && first.source.iter().all(|s| *s == 1)
            && first.faulted.iter().all(|f| !f),
        "start: {first:?}"
    );
    assert!(
        (first.command[0] - 77.55).abs() < 1e-9,
        "Control.PID's first command: {first:?}"
    );
}

#[test]
fn lesson81_single_sensor_faults_keep_the_step_requirement_and_disagreement_faults_remove_heat() {
    // The stale, range and short-disagreement cases keep a valid measurement and meet the requirement on the true temperature:
    // peak at most 37 °C and within 0.2 °C of 36 °C from 30 s to 60 s. The latched cases remove heat and do not count.
    let record = lesson81_run(12.0, &[100; 620]);
    let run = &record[1..];
    for case in [STALE, RANGE, SHORT] {
        let peak = run[..=600]
            .iter()
            .map(|u| u.temperature[case])
            .fold(f64::MIN, f64::max);
        let late = run[300..=600]
            .iter()
            .map(|u| (36.0 - u.temperature[case]).abs())
            .fold(0.0, f64::max);
        assert!(
            peak <= 37.0 && late <= 0.2,
            "case {case}: peak {peak} late {late}"
        );
    }
    let end = &run[600];
    assert!(run[599].passed == 0.0 && end.passed == 3.0, "{end:?}");
    let peaks = [STALE, RANGE, SHORT].map(|case| {
        run[..=600]
            .iter()
            .map(|u| u.temperature[case])
            .fold(f64::MIN, f64::max)
    });
    assert_eq!(
        end.highest_peak,
        peaks.iter().copied().fold(f64::MIN, f64::max),
        "{end:?}"
    );
    assert!(
        end.largest_jump <= 0.1 && end.largest_step_error <= 0.2,
        "{end:?}"
    );
}

#[test]
fn lesson81_a_stale_primary_is_used_until_its_reading_is_older_than_0_5_s_then_the_monitor_takes_over(
) {
    // Case 0: sensor A stops acquiring at 5.0 s. Its last reading stays usable while its age is at most 500 ms, through 5.5 s;
    // at 5.6 s it is 600 ms old and invalid, and control transfers to sensor B without a command jump.
    let record = lesson81_run(12.0, &[100; 70]);
    let run = &record[1..];
    let c = STALE;
    assert!(
        run[50..=55]
            .iter()
            .all(|u| u.source[c] == 1 && u.selected[c] == run[50].selected[c]),
        "A held through 5.5 s"
    );
    assert!(
        run[56].source[c] == 2
            && !run[56].faulted[c]
            && run[56].selected[c] == run[56].temperature[c],
        "{:?}",
        run[56]
    );
    assert!(
        (run[56].command[c] - run[55].command[c]).abs() <= 0.1,
        "{:?} {:?}",
        run[55],
        run[56]
    );
    assert!(
        run[55].stale_source == 1.0
            && run[56].stale_source == 2.0
            && run[53].stale_selected == run[50].selected[c],
        "published: {:?}",
        run[56]
    );
}

#[test]
fn lesson81_an_out_of_range_primary_hands_over_to_the_monitor_at_once() {
    // Case 1: sensor A reads 85 °C from 5.0 s, outside 0–80 °C: invalid at once, and control transfers to sensor B at 5.0 s
    // without a command jump.
    let record = lesson81_run(12.0, &[100; 70]);
    let run = &record[1..];
    let c = RANGE;
    assert!(
        run[49].source[c] == 1
            && run[50].source[c] == 2
            && run[60].source[c] == 2
            && !run[60].faulted[c],
        "{:?}",
        run[50]
    );
    assert!(
        (run[50].command[c] - run[49].command[c]).abs() <= 0.1,
        "{:?} {:?}",
        run[49],
        run[50]
    );
    assert!(run[50].range_source == 2.0, "published: {:?}", run[50]);
}

#[test]
fn lesson81_a_sustained_fresh_disagreement_latches_zero_heat_at_0_2_s() {
    // Case 2: from 5.0 s sensor B reads 0.6 °C higher than A. Both are fresh and in range, so neither can be chosen: the
    // disagreement is timed 0, 100 and 200 ms at 5.0, 5.1 and 5.2 s, and at 200 ms the fault latches and heat is removed.
    let record = lesson81_run(12.0, &[100; 620]);
    let run = &record[1..];
    let c = DISAGREE;
    assert!(
        run[50].disagreement_ms[c] == 0 && run[51].disagreement_ms[c] == 100 && !run[51].faulted[c],
        "{:?}",
        run[51]
    );
    assert!(
        run[52].disagreement_ms[c] == 200 && run[52].faulted[c] && run[52].source[c] == 0,
        "{:?}",
        run[52]
    );
    assert!(
        run[52..=600]
            .iter()
            .all(|u| u.command[c] == 0.0 && u.faulted[c] && u.disagree_fault),
        "zero heat stays latched"
    );
    assert!(
        run[51].disagreement_s == 0.1 && run[52].disagreement_s == 0.2,
        "published: {:?}",
        run[52]
    );
}

#[test]
fn lesson81_separate_short_disagreements_do_not_accumulate() {
    // Case 3: B reads 0.6 °C higher at 5.0 and 5.1 s, agrees at 5.2 s, and is 0.6 °C higher again at 5.3 and 5.4 s. Each
    // interval lasts 100 ms; they are separate, so the timer restarts and no fault latches.
    let record = lesson81_run(12.0, &[100; 70]);
    let run = &record[1..];
    let c = SHORT;
    let timer: Vec<u64> = run[50..=55].iter().map(|u| u.disagreement_ms[c]).collect();
    assert_eq!(timer, vec![0, 100, 0, 0, 100, 0], "timer");
    assert!(
        run.iter().all(|u| !u.faulted[c] && u.source[c] == 1),
        "no fault, sensor A throughout"
    );
    assert!(run[54].short_s == 0.1, "published: {:?}", run[54]);
}

#[test]
fn lesson81_losing_both_sensors_latches_zero_heat() {
    // Case 4: sensor A reads 85 °C from 5.0 s, so control moves to B; B stops acquiring at 5.0 s and is 600 ms old at 5.6 s.
    // With no valid sensor left, the fault latches and heat is removed.
    let record = lesson81_run(12.0, &[100; 70]);
    let run = &record[1..];
    let c = BOTH;
    assert!(
        run[50..=55]
            .iter()
            .all(|u| u.source[c] == 2 && !u.faulted[c]),
        "B through 5.5 s"
    );
    assert!(
        run[56].faulted[c]
            && run[56].source[c] == 0
            && run[56..]
                .iter()
                .all(|u| u.command[c] == 0.0 && u.both_fault),
        "{:?}",
        run[56]
    );
    assert!(
        run[55].both_source == 2.0 && run[56].both_source == 0.0,
        "published: {:?}",
        run[56]
    );
}

#[test]
fn lesson81_recovery_needs_three_fresh_agreeing_readings_and_a_new_auto_request() {
    // Case 5: the disagreement of case 2 latches at 5.2 s and lasts until 9.9 s. The operator's Auto request at 8.0 s is
    // refused: the sensors still disagree. From 10.0 s they agree; three fresh agreeing readings at 10.0, 10.1 and 10.2 s
    // qualify the measurement, but control resumes only with the next Auto request, at 12.0 s, on sensor A. It starts from the
    // zero command: the first update adds only the integral step Ki·Δt·(36 − T), not a proportional jump Kp·(36 − T).
    let record = lesson81_run(12.0, &[100; 250]);
    let run = &record[1..];
    let c = RESTORED;
    assert!(
        run[52].faulted[c]
            && run[80].faulted[c]
            && run[80].command[c] == 0.0
            && !run[80].qualified[c],
        "{:?}",
        run[80]
    );
    let agreeing: Vec<u64> = run[99..=102].iter().map(|u| u.agreeing[c]).collect();
    assert_eq!(agreeing, vec![0, 1, 2, 3], "agreeing readings");
    assert!(
        run[102].qualified[c] && run[119].faulted[c] && run[119].command[c] == 0.0,
        "{:?}",
        run[119]
    );
    let integral_step = 0.5 * 0.1 * (36.0 - run[120].selected[c]);
    assert!(
        !run[120].faulted[c]
            && run[120].source[c] == 1
            && (run[120].command[c] - integral_step).abs() < 1e-9,
        "{:?}",
        run[120]
    );
    assert!(
        run[200].temperature[c] > run[120].temperature[c],
        "recovering: {:?}",
        run[200]
    );
    assert!(
        run[101].restored_agreeing == 2.0
            && run[102].restored_qualified
            && run[119].restored_fault
            && !run[120].restored_fault,
        "published: {:?}",
        run[120]
    );
}

#[test]
fn lesson81_the_auto_request_time_is_an_experiment_setting_frozen_at_the_start() {
    // With the Auto request at 20 s, control resumes at 20.0 s.
    let record = lesson81_run(20.0, &[100; 250]);
    let run = &record[1..];
    let c = RESTORED;
    assert!(
        run[199].faulted[c] && !run[200].faulted[c],
        "resume at 20.0 s: {:?}",
        run[200]
    );
}

#[test]
fn lesson81_every_heater_runs_control_pid() {
    // Each case's loop is Control.PID with Lesson 85's gains: Kp 2.5 %/°C, Ki 0.5 %/(°C·s), β = 1, back-calculation with
    // Tt = 1 s, 0–100 %.
    let (mut runtime, mut simulation) = setup(LESSON81);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    scan_every(&mut runtime, &mut simulation, &debug, 0, 100, &[]);
    for case in 0..6 {
        let parameters = member(
            &runtime,
            member_instance(&runtime, sensed_case(&runtime, p1, case), "Loop"),
            "ParametersValue",
        );
        let gain = |field: &str| lreal(&record_field(&parameters, field));
        assert!(
            gain("ProportionalGain") == 2.5
                && gain("IntegralGain") == 0.5
                && gain("ProportionalSetpointWeight") == 1.0,
            "case {case}"
        );
        assert!(
            gain("TrackingTime") == 1.0
                && gain("OutputMinimum") == 0.0
                && gain("OutputMaximum") == 100.0,
            "case {case}"
        );
    }
}

#[test]
fn lesson81_a_wrong_update_interval_latches_zero_heat() {
    // An update interval outside 100 ms ± 10 ms latches zero heat on every heater.
    let gap = lesson81_run(12.0, &[100, 100, 150, 100]);
    assert!(
        !gap[2].clock_fault
            && gap[3..]
                .iter()
                .all(|u| u.clock_fault && u.command.iter().all(|pct| *pct == 0.0)),
        "interval: {gap:?}"
    );
}
