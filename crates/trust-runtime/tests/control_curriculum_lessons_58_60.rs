use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON58: &str = "09-industrial-control-structures/54-split-range";

/// One 0.1 s update of the three jacketed vessels, ordered [heater-only cascade with 60 kW entering the jacket,
/// split-range cascade with the same inflow, split-range cascade with a 30 kW jacket loss], with the applied net heat of
/// the two split-range vessels.
#[derive(Clone, Copy, Debug)]
struct Split {
    vessel: [f64; 3],
    heater: [f64; 3],
    cooler: [f64; 3],
    net: [f64; 2],
}

/// One update at rest, then Control Studio's project experiment (activation at %MX72.0) for `updates`.
fn lesson58_run(updates: u64) -> Vec<Split> {
    let (mut runtime, mut simulation) = setup(LESSON58);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX72.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Split {
            vessel: std::array::from_fn(|k| number(&runtime, &format!("%IL{}", 16 * k + 8))),
            heater: std::array::from_fn(|k| number(&runtime, &format!("%QL{}", 24 * k))),
            cooler: std::array::from_fn(|k| number(&runtime, &format!("%QL{}", 24 * k + 8))),
            net: [number(&runtime, "%ML32"), number(&runtime, "%ML48")],
        });
    }
    record
}

#[test]
fn lesson58_maps_the_signed_demand_to_one_service_at_its_own_scale() {
    // Q = 2·uh − uc: at rest 30 kW from the heater at 15 %. With 60 kW entering the jacket the split range must remove
    // 30 kW, cooler 30 %; with a 30 kW jacket loss it must supply 60 kW, heater 30 %. The two services are never on
    // together, and the applied net heat is 2·uh − uc.
    let record = lesson58_run(600);
    assert!(
        record[0].heater[1..]
            .iter()
            .all(|u| (u - 15.0).abs() < 1e-9)
            && record[0].cooler[1..].iter().all(|u| *u == 0.0),
        "at rest: {:?}",
        record[0]
    );
    assert!(
        record
            .iter()
            .all(|u| (1..3).all(|k| u.heater[k] == 0.0 || u.cooler[k] == 0.0)),
        "one service at a time"
    );
    assert!(
        record
            .iter()
            .all(|u| (0..2)
                .all(|k| (u.net[k] - (2.0 * u.heater[k + 1] - u.cooler[k + 1])).abs() < 1e-9)),
        "net heat is 2·uh − uc"
    );
    let last = record.last().unwrap();
    assert!(
        last.heater[1] == 0.0 && (last.cooler[1] - 30.0).abs() < 0.05,
        "inflow: {last:?}"
    );
    assert!(
        (last.heater[2] - 30.0).abs() < 0.05 && last.cooler[2] == 0.0,
        "loss: {last:?}"
    );
}

#[test]
fn lesson58_the_split_range_holds_the_vessel_under_excess_heat_where_heating_alone_cannot() {
    // Excess heat: peak ≤ 36 °C and |error| ≤ 0.2 °C from 30 s. The heater-only cascade can only switch its heater off and
    // the vessel keeps rising above 40 °C; the split range cools and meets both. On the jacket loss it holds ≥ 34 °C.
    let record = lesson58_run(600);
    let run = &record[1..];
    let peak = |k: usize| run.iter().map(|u| u.vessel[k]).fold(f64::MIN, f64::max);
    let late = |k: usize| {
        run[300..]
            .iter()
            .map(|u| (u.vessel[k] - 35.0).abs())
            .fold(0.0, f64::max)
    };
    assert!(peak(0) > 40.0, "heater only: {}", peak(0));
    assert!(
        peak(1) <= 36.0 && late(1) <= 0.2,
        "split range, inflow: peak {} late {}",
        peak(1),
        late(1)
    );
    let minimum = run.iter().map(|u| u.vessel[2]).fold(f64::MAX, f64::min);
    assert!(
        minimum >= 34.0 && late(2) <= 0.2,
        "split range, loss: min {minimum} late {}",
        late(2)
    );
}

const LESSON59: &str = "09-industrial-control-structures/55-override-control";

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

/// One 0.1 s update of the three vessels, ordered [vessel PI alone, override with tracking, override without tracking],
/// with the two requests and the selection of the override vessels.
#[derive(Clone, Copy, Debug)]
struct Override {
    jacket: [f64; 3],
    vessel: [f64; 3],
    heater: [f64; 3],
    vessel_request: [f64; 2],
    jacket_request: [f64; 2],
    jacket_selected: [bool; 2],
}

/// One update at rest, then Control Studio's project experiment (activation at %MX48.0) for `updates`.
fn lesson59_run(updates: u64) -> Vec<Override> {
    let (mut runtime, mut simulation) = setup(LESSON59);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX48.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Override {
            jacket: std::array::from_fn(|k| number(&runtime, &format!("%IL{}", 16 * k))),
            vessel: std::array::from_fn(|k| number(&runtime, &format!("%IL{}", 16 * k + 8))),
            heater: std::array::from_fn(|k| number(&runtime, &format!("%QL{}", 8 * k))),
            vessel_request: [number(&runtime, "%ML16"), number(&runtime, "%ML32")],
            jacket_request: [number(&runtime, "%ML24"), number(&runtime, "%ML40")],
            jacket_selected: [flag(&runtime, "%MX48.1"), flag(&runtime, "%MX48.2")],
        });
    }
    record
}

#[test]
fn lesson59_applies_the_smaller_request_and_the_tracked_loser_stays_at_the_applied_command() {
    // Low select: the heater gets min(vessel request, jacket request) in both override vessels. With tracking, the
    // controller that is not selected restarts each update from the applied command, so its request stays within its own
    // single-update increment of it; without tracking the jacket controller's request climbs far above the applied command
    // before it is needed.
    let record = lesson59_run(600);
    for u in &record[1..] {
        for k in 0..2 {
            assert!(
                (u.heater[k + 1] - u.vessel_request[k].min(u.jacket_request[k])).abs() < 1e-9,
                "low select: {u:?}"
            );
        }
    }
    assert!(
        record[1..]
            .windows(2)
            .all(|w| (w[1].vessel_request[0].max(w[1].jacket_request[0]) - w[0].heater[1]) <= 5.0),
        "the tracked loser stays near the applied command"
    );
    let windup = record[1..100]
        .iter()
        .map(|u| u.jacket_request[1] - u.heater[2])
        .fold(f64::MIN, f64::max);
    assert!(
        windup >= 20.0,
        "the untracked jacket controller winds up above the applied command: {windup}"
    );
}

#[test]
fn lesson59_only_the_tracked_override_holds_the_jacket_limit_and_returns_at_once() {
    // Setpoint 40 °C is infeasible with the jacket at 41.8 °C: the tracked override holds the vessel at (41.8 + 4)/1.2 =
    // 38.167 °C with the jacket ≤ 42 °C. The vessel PI alone overheats the jacket to 44 °C; the untracked override lets it
    // pass 42 °C. When the setpoint drops to 37 °C at 40 s, the tracked vessel controller takes over in the same update;
    // the untracked one stays stuck behind its wound-up integral.
    let record = lesson59_run(600);
    let run = &record[1..];
    let jacket_peak = |k: usize| run.iter().map(|u| u.jacket[k]).fold(f64::MIN, f64::max);
    assert!(jacket_peak(0) > 43.5, "PI alone: {}", jacket_peak(0));
    assert!(
        jacket_peak(1) <= 42.0,
        "tracked override: {}",
        jacket_peak(1)
    );
    assert!(
        jacket_peak(2) > 42.0,
        "untracked override: {}",
        jacket_peak(2)
    );
    assert!(
        (run[399].vessel[1] - 38.1667).abs() < 0.02,
        "held vessel at 39.9 s: {:?}",
        run[399]
    );
    assert!(
        run[399].jacket_selected[0] && !run[400].jacket_selected[0],
        "tracked: back to the vessel controller at 40 s"
    );
    assert!(
        run[400..500].iter().all(|u| u.jacket_selected[1]),
        "untracked: the jacket controller keeps authority"
    );
}

const LESSON60: &str = "09-industrial-control-structures/57-smith-predictor";

/// One 0.1 s update of the heater: the water temperature, the heater command and the predictor's correction c.
#[derive(Clone, Copy, Debug)]
struct Smith {
    temperature: f64,
    #[allow(dead_code)] // shown in failure messages
    command: f64,
    #[allow(dead_code)] // shown in failure messages
    correction: f64,
}

/// One update at rest in Manual; then the PI gains (Kp, Ki) and the model (K, T, θ) are written as Control Studio's Apply
/// writes them, the controller goes to Auto and the setpoint steps 35 → 35.5 °C (at %ML0), for `updates`.
/// Control Studio's Smith setup binds these program variables; `set_instance_var` would silently add a missing one.
fn lesson60_assert_interface(runtime: &trust_runtime::Runtime) {
    let p1 = program(runtime);
    for name in [
        "Auto",
        "Parameters",
        "ModelGain",
        "ModelTimeConstantS",
        "ModelDelayS",
        "CorrectionC",
    ] {
        assert!(
            runtime.storage().get_instance_var(p1, name).is_some(),
            "Control Studio's Smith setup binds P1.{name}"
        );
    }
}

fn lesson60_run(gains: (f64, f64), model: (f64, f64, f64), updates: u64) -> Vec<Smith> {
    let (mut runtime, mut simulation) = setup(LESSON60);
    lesson60_assert_interface(&runtime);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    let mut record = Vec::new();
    for step in 0..1 + updates {
        let mut writes = Vec::new();
        if step == 1 {
            assert!(runtime
                .storage_mut()
                .set_instance_var(p1, "Auto", Value::Bool(true)));
            set_program_record(
                &mut runtime,
                p1,
                "Parameters",
                &[("ProportionalGain", gains.0), ("IntegralGain", gains.1)],
            );
            for (name, value) in [
                ("ModelGain", model.0),
                ("ModelTimeConstantS", model.1),
                ("ModelDelayS", model.2),
            ] {
                assert!(
                    runtime
                        .storage_mut()
                        .set_instance_var(p1, name, Value::LReal(value)),
                    "{name}"
                );
            }
            writes.push(("%ML0", Value::LWord(35.5_f64.to_bits())));
        }
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            100,
            &writes,
        );
        record.push(Smith {
            temperature: number(&runtime, "%IL0"),
            command: number(&runtime, "%QL0"),
            correction: program_number(&runtime, p1, "CorrectionC"),
        });
    }
    record
}

#[test]
fn lesson60_the_controller_starts_in_manual_and_holds_its_output() {
    // Before Control Studio applies a design the controller is in Manual: a setpoint change alone leaves the heater at
    // its 75 % manual output and the water at 35 °C. Switched to Auto with the slow PI (Kp 5, Ki 1) and no predictor, its
    // first update adds only the integral part, 1·0.5·0.1 = 0.05 points: Manual tracked the new setpoint.
    let (mut runtime, mut simulation) = setup(LESSON60);
    lesson60_assert_interface(&runtime);
    let debug = runtime.enable_debug();
    for step in 0..50 {
        let writes = if step == 1 {
            vec![("%ML0", Value::LWord(35.5_f64.to_bits()))]
        } else {
            Vec::new()
        };
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            100,
            &writes,
        );
        let command = number(&runtime, "%QL0");
        assert!(
            (command - 75.0).abs() < 1e-12,
            "Manual output at update {step}: {command}"
        );
    }
    assert!((number(&runtime, "%IL0") - 35.0).abs() < 1e-9);
    let p1 = program(&runtime);
    assert!(runtime
        .storage_mut()
        .set_instance_var(p1, "Auto", Value::Bool(true)));
    scan_every(&mut runtime, &mut simulation, &debug, 5000, 100, &[]);
    let command = number(&runtime, "%QL0");
    assert!(
        (command - 75.05).abs() < 1e-9,
        "first Auto update: {command}"
    );
}

#[test]
fn lesson60_the_predictor_removes_the_delay_from_the_loop_but_not_from_the_heat() {
    // Fast gains Kp 25, Ki 5 (λ = 1 s). With the heater's own model, K 0.2 °C/%, T 5 s and θ 1 s, the controller acts on
    // the delay-free prediction: the water reaches 63 % of the 0.5 °C step after 1 s delay + 1 s and never overshoots.
    // The same gains with model gain 0, on the measured temperature, overshoot by more than 40 %.
    let peak = |run: &[Smith]| {
        run[1..]
            .iter()
            .map(|u| u.temperature)
            .fold(f64::MIN, f64::max)
    };
    let smith = lesson60_run((25.0, 5.0), (0.2, 5.0, 1.0), 300);
    let plain = lesson60_run((25.0, 5.0), (0.0, 5.0, 1.0), 300);
    assert!(
        peak(&plain) > 35.7,
        "PI on the measurement: {}",
        peak(&plain)
    );
    assert!(
        peak(&smith) <= 35.5 + 1e-3,
        "Smith predictor: {}",
        peak(&smith)
    );
    let reached = smith[1..]
        .iter()
        .position(|u| u.temperature >= 35.316)
        .expect("the water reaches 63 %") as f64
        * 0.1;
    assert!((1.8..=2.2).contains(&reached), "63 % after {reached} s");
}

#[test]
fn lesson60_a_wrong_model_delay_brings_the_overshoot_back() {
    // The same predictor with θ = 0.5 s instead of the heater's 1 s: the correction no longer cancels the delay, and the
    // water overshoots the 0.5 °C step by more than 10 %.
    let wrong = lesson60_run((25.0, 5.0), (0.2, 5.0, 0.5), 300);
    let peak = wrong[1..]
        .iter()
        .map(|u| u.temperature)
        .fold(f64::MIN, f64::max);
    assert!(peak > 35.55, "wrong model delay: {peak}");
}
