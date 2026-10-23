use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON55: &str = "09-industrial-control-structures/51-feedforward";

/// One 0.1 s update of the three heaters, ordered [PI alone, PI + feedforward, PI + feedforward from 36 kW], with the
/// feedback part (command − feedforward) of the two feedforward heaters.
#[derive(Clone, Copy, Debug)]
struct Feedforward {
    temperature: [f64; 3],
    command: [f64; 3],
    feedback: [f64; 2],
}

/// One update at rest, then Control Studio's project experiment (activation at %MX48.0) for `updates`.
fn lesson55_run(updates: u64) -> Vec<Feedforward> {
    let (mut runtime, mut simulation) = setup(LESSON55);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX48.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Feedforward {
            temperature: std::array::from_fn(|k| number(&runtime, &format!("%IL{}", 8 * k))),
            command: std::array::from_fn(|k| number(&runtime, &format!("%QL{}", 16 * k))),
            feedback: [number(&runtime, "%ML32"), number(&runtime, "%ML40")],
        });
    }
    record
}

#[test]
fn lesson55_adds_the_measured_load_to_the_pi_request_at_the_first_update() {
    // The 30 kW extraction starts with the experiment. Before the water has changed, PI alone still asks for 75 %;
    // feedforward adds 30/2 = 15 points (90 %) and, from the 36 kW estimate, 18 points (93 %).
    let record = lesson55_run(1);
    assert_eq!(
        record[0].command,
        [75.0, 75.0, 75.0],
        "at rest: {:?}",
        record[0]
    );
    let first = record[1].command;
    assert!(
        (first[0] - 75.0).abs() < 1e-9
            && (first[1] - 90.0).abs() < 1e-9
            && (first[2] - 93.0).abs() < 1e-9,
        "first update: {first:?}"
    );
}

#[test]
fn lesson55_feedforward_leaves_only_the_delivery_delay_drop_and_pi_trims_the_estimate() {
    // For one second no extra heat arrives: the water falls by 3·(1 − e^(−1/5)) = 0.5438 °C with feedforward, and
    // PI alone lets it fall below 33.8 °C. All three return within 0.2 °C from 30 s. At 60 s the command is 90 % in each
    // case; the feedback part is 75 % with the exact load and 72 % with the 36 kW estimate.
    let record = lesson55_run(600);
    let run = &record[1..];
    let minimum = |k: usize| {
        run.iter()
            .map(|u| u.temperature[k])
            .fold(f64::MAX, f64::min)
    };
    assert!(minimum(0) < 33.8, "PI alone: {}", minimum(0));
    for k in 1..3 {
        assert!(
            (minimum(k) - (35.0 - 3.0 * (1.0 - (-0.2f64).exp()))).abs() < 1e-3,
            "feedforward {k}: {}",
            minimum(k)
        );
    }
    for k in 0..3 {
        let late = run[300..]
            .iter()
            .map(|u| (u.temperature[k] - 35.0).abs())
            .fold(0.0, f64::max);
        assert!(late <= 0.2, "heater {k}: late error {late}");
    }
    let last = run.last().unwrap();
    assert!(
        last.command.iter().all(|u| (u - 90.0).abs() < 0.05),
        "final commands: {:?}",
        last.command
    );
    assert!(
        (last.feedback[0] - 75.0).abs() < 0.05 && (last.feedback[1] - 72.0).abs() < 0.05,
        "feedback parts: {:?}",
        last.feedback
    );
}

const LESSON56: &str = "09-industrial-control-structures/52-cascade";

/// One 0.1 s update of the two jacketed vessels, ordered [cascade that Control Studio commissions, single loop], with the
/// cascade's jacket setpoint.
#[derive(Clone, Copy, Debug)]
struct Jacketed {
    jacket: [f64; 2],
    vessel: [f64; 2],
    command: [f64; 2],
    jacket_setpoint: f64,
}

fn lesson56_read(runtime: &trust_runtime::Runtime) -> Jacketed {
    Jacketed {
        jacket: [number(runtime, "%IL0"), number(runtime, "%IL16")],
        vessel: [number(runtime, "%IL8"), number(runtime, "%IL24")],
        command: [number(runtime, "%QL0"), number(runtime, "%QL16")],
        jacket_setpoint: number(runtime, "%ML8"),
    }
}

/// `updates` after one update at rest; `start` runs in that first update (Auto switches and gains, as Control Studio's
/// Apply writes them) and `writes` are queued with it.
fn lesson56_run(
    updates: u64,
    start: impl Fn(&mut trust_runtime::Runtime),
    writes: &[(&str, Value)],
) -> Vec<Jacketed> {
    let (mut runtime, mut simulation) = setup(LESSON56);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            start(&mut runtime);
        }
        let queued: &[(&str, Value)] = if step == 1 { writes } else { &[] };
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            100,
            queued,
        );
        record.push(lesson56_read(&runtime));
    }
    record
}

/// Control Studio's cascade setup binds these program variables; `set_instance_var` would silently add a missing one.
fn lesson56_auto(runtime: &mut trust_runtime::Runtime, outer: bool, inner: bool) {
    let p1 = program(runtime);
    for name in [
        "OuterAuto",
        "InnerAuto",
        "OuterParameters",
        "InnerParameters",
        "OuterManualC",
        "InnerManualPct",
    ] {
        assert!(
            runtime.storage().get_instance_var(p1, name).is_some(),
            "Control Studio's cascade setup binds P1.{name}"
        );
    }
    for (name, value) in [("OuterAuto", outer), ("InnerAuto", inner)] {
        assert!(runtime.write_instance(p1, name, Value::Bool(value)).is_ok());
    }
}

#[test]
fn lesson56_the_cascade_starts_in_manual_at_the_resting_point() {
    // Before Control Studio commissions it, both PIDs of the cascade are in Manual: the heater holds 15 % and the jacket
    // setpoint 38 °C, and a vessel setpoint change alone moves neither. The single loop holds its vessel at 35 °C.
    let record = lesson56_run(50, |_| {}, &[("%ML0", Value::LWord(37.0_f64.to_bits()))]);
    for (step, update) in record.iter().enumerate() {
        assert!(
            (update.command[0] - 15.0).abs() < 1e-12
                && (update.jacket_setpoint - 38.0).abs() < 1e-12,
            "Manual at update {step}: {update:?}"
        );
        assert!(
            (update.vessel[1] - 35.0).abs() < 1e-9,
            "single loop at rest: {update:?}"
        );
    }
}

#[test]
fn lesson56_with_the_outer_pid_in_manual_the_inner_loop_follows_its_jacket_setpoint() {
    // Inner PID in Auto with Kp 5 %/°C and Ki 10 %/(°C·s), outer PID in Manual; its manual output, the jacket setpoint,
    // steps 38 → 39 °C. The first inner request is 15 + 5·1 + 10·1·0.1 = 21 %, and the jacket reaches 63.2 % of the step
    // within 0.6 s.
    let record = lesson56_run(
        100,
        |runtime| {
            lesson56_auto(runtime, false, true);
            let p1 = program(runtime);
            set_program_record(
                runtime,
                p1,
                "InnerParameters",
                &[("ProportionalGain", 5.0), ("IntegralGain", 10.0)],
            );
        },
        &[("%ML16", Value::LWord(39.0_f64.to_bits()))],
    );
    let run = &record[1..];
    assert!(
        (run[0].command[0] - 21.0).abs() < 1e-9,
        "first inner request: {:?}",
        run[0]
    );
    let reached = run
        .iter()
        .position(|u| u.jacket[0] >= 38.632)
        .expect("the jacket reaches 63.2 %") as f64
        * 0.1;
    assert!(reached <= 0.6, "63.2 % after {reached} s");
}

#[test]
fn lesson56_the_applied_cascade_corrects_the_jacket_loss_earlier_than_the_single_loop() {
    // Both PIDs of the cascade in Auto (outer Kp 0.5, Ki 0.12; inner Kp 5, Ki 10), then the project test (activation at
    // %MX32.0) takes 30 kW from both jackets. The cascade keeps its vessel above 34 °C and within 0.2 °C from 30 s, and
    // ends at the 30 % that replaces 30 kW of losses and 30 kW of extraction; the single loop (Kp 2.5, Ki 0.1) falls
    // below 34 °C.
    let record = lesson56_run(
        600,
        |runtime| {
            lesson56_auto(runtime, true, true);
            let p1 = program(runtime);
            set_program_record(
                runtime,
                p1,
                "OuterParameters",
                &[("ProportionalGain", 0.5), ("IntegralGain", 0.12)],
            );
            set_program_record(
                runtime,
                p1,
                "InnerParameters",
                &[("ProportionalGain", 5.0), ("IntegralGain", 10.0)],
            );
        },
        &[("%MX32.0", Value::Bool(true))],
    );
    let run = &record[1..];
    let minimum = |k: usize| run.iter().map(|u| u.vessel[k]).fold(f64::MAX, f64::min);
    let late = |k: usize| {
        run[300..]
            .iter()
            .map(|u| (u.vessel[k] - 35.0).abs())
            .fold(0.0, f64::max)
    };
    assert!(
        minimum(0) >= 34.0 && late(0) <= 0.2,
        "cascade: min {} late {}",
        minimum(0),
        late(0)
    );
    assert!(minimum(1) < 34.0, "single loop: min {}", minimum(1));
    assert!(
        (run.last().unwrap().command[0] - 30.0).abs() < 0.1,
        "final command: {:?}",
        run.last()
    );
}

const LESSON57: &str = "09-industrial-control-structures/53-ratio-control";

/// One 0.1 s update of the two additive pumps, ordered [carrier step 100 → 150 L/min, carrier stop 100 → 0 L/min].
#[derive(Clone, Copy, Debug)]
struct Ratio {
    additive: [f64; 2],
    command: [f64; 2],
    setpoint: [f64; 2],
    ratio: f64,
    tail: f64,
    evaluated: [bool; 2],
}

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

/// One update at rest, then Control Studio's project experiment (activation at %MX48.0) for `updates`.
fn lesson57_run(updates: u64) -> Vec<Ratio> {
    let (mut runtime, mut simulation) = setup(LESSON57);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX48.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Ratio {
            additive: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            command: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            setpoint: [number(&runtime, "%ML16"), number(&runtime, "%ML24")],
            ratio: number(&runtime, "%ML32"),
            tail: number(&runtime, "%ML40"),
            evaluated: [flag(&runtime, "%MX48.2"), flag(&runtime, "%MX48.1")],
        });
    }
    record
}

#[test]
fn lesson57_sets_the_additive_target_from_the_carrier_and_stops_at_once_without_a_ratio() {
    // Carrier 150 L/min: target 0.1·150 = 15 L/min; the first request 50 + 10·5·0.1 + 10·5 = 105 % is limited to 100 %.
    // Carrier 0 L/min: below 5 L/min the pump command is 0 % in the same update and the ratio is not evaluated.
    let record = lesson57_run(1);
    let first = record[1];
    assert!(
        (first.setpoint[0] - 15.0).abs() < 1e-12 && first.command[0] == 100.0 && first.evaluated[0],
        "step: {first:?}"
    );
    assert!(
        first.command[1] == 0.0 && !first.evaluated[1],
        "stop: {first:?}"
    );
}

#[test]
fn lesson57_holds_the_ratio_and_sums_the_stop_tail() {
    // The additive flow follows its target within ±0.005 of the ratio from 10 s to 60 s and ends at 75 %. After the
    // stop, the flow decays as 10·e^(−t) L/min: below 0.1 L/min by 5 s, with a tail of Σ q·Δt/60 = 0.175 L ≤ 0.20 L.
    let record = lesson57_run(600);
    let run = &record[1..];
    assert!(
        run[100..].iter().all(|u| (u.ratio - 0.1).abs() <= 0.005),
        "ratio after 10 s"
    );
    assert!(
        (run.last().unwrap().command[0] - 75.0).abs() < 0.1,
        "final command {:?}",
        run.last()
    );
    assert!(
        run.iter().all(|u| u.command[1] == 0.0 && !u.evaluated[1]),
        "the stopped pump stays at 0 % without a ratio"
    );
    assert!(
        run[50].additive[1] < 0.1,
        "flow at 5 s: {}",
        run[50].additive[1]
    );
    let tail = run[100].tail;
    assert!(
        (tail - 0.1751).abs() < 0.001 && tail <= 0.20,
        "tail after 10 s: {tail}"
    );
}
