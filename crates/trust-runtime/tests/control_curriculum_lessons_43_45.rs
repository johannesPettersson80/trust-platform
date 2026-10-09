use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON43: &str = "09-industrial-control-structures/56-gain-scheduling";
const LEVELS: [f64; 3] = [0.09, 0.25, 0.49];

/// One 1 s update of the six tanks: level changes and inflows as [low, mid, high] for the fixed and the scheduled
/// loops, and the scheduled integral gains.
#[derive(Clone, Copy, Debug)]
struct Scheduling {
    fixed: [f64; 3],
    scheduled: [f64; 3],
    #[allow(dead_code)] // shown in failure messages
    fixed_inlet: [f64; 3],
    scheduled_inlet: [f64; 3],
    ki: [f64; 3],
}

/// `rest` updates at the holding flows, then Control Studio's project experiment (activation at %MX72.0), which
/// steps every setpoint by 0.02 m, for `updates` more.
fn lesson43_run(rest: u64, updates: u64) -> Vec<Scheduling> {
    let (mut runtime, mut simulation) = setup(LESSON43);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX72.0").unwrap(), Value::Bool(true));
        }
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 1000,
            1000,
            &[],
        );
        let read = |address: &str| number(&runtime, address);
        record.push(Scheduling {
            fixed: [read("%ML0"), read("%ML8"), read("%ML16")],
            scheduled: [read("%ML24"), read("%ML32"), read("%ML40")],
            fixed_inlet: [read("%QL0"), read("%QL16"), read("%QL32")],
            scheduled_inlet: [read("%QL8"), read("%QL24"), read("%QL40")],
            ki: [read("%ML48"), read("%ML56"), read("%ML64")],
        });
    }
    record
}

/// Seconds after the step until a level change stays within 0.002 m of the 0.02 m step.
fn settled_after(changes: &[f64]) -> Option<usize> {
    (0..changes.len()).find(|&start| {
        changes[start..]
            .iter()
            .all(|change| (change - 0.02).abs() <= 0.002)
    })
}

#[test]
fn lesson43_the_scheduled_integral_gain_follows_the_local_model() {
    // Ki(h) = c/(2λ√h) with c = 0.020 m^(5/2)/s and λ = 10 s: 0.00333, 0.00200 and 0.00143 m²/s².
    let record = lesson43_run(5, 1);
    for (index, level) in LEVELS.iter().enumerate() {
        let expected = 0.020 / (2.0 * 10.0 * level.sqrt());
        assert!(
            (record[4].ki[index] - expected).abs() <= 0.01 * expected,
            "Ki at {level} m: {:?}",
            record[4]
        );
    }
}

#[test]
fn lesson43_the_scheduled_loop_responds_alike_at_every_level() {
    let record = lesson43_run(5, 61);
    let run = &record[5..];
    for (index, level) in LEVELS.iter().enumerate() {
        let changes: Vec<f64> = run.iter().map(|update| update.scheduled[index]).collect();
        let settled = settled_after(&changes).expect("the scheduled loop settles");
        assert!(
            settled <= 30,
            "scheduled at {} m within 0.002 m by 30 s: {settled} s",
            level
        );
    }
    for update in run {
        let (low, high) = (
            update
                .scheduled
                .iter()
                .cloned()
                .fold(f64::INFINITY, f64::min),
            update
                .scheduled
                .iter()
                .cloned()
                .fold(f64::NEG_INFINITY, f64::max),
        );
        assert!(
            high - low <= 0.0015,
            "the three scheduled responses coincide within the sensor noise: {update:?}"
        );
    }
}

#[test]
fn lesson43_the_fixed_loop_is_slow_at_the_low_level() {
    // Ki = 0.002 is designed for 0.25 m; at 0.09 m the tank drains faster than that design assumes.
    let record = lesson43_run(5, 61);
    let run = &record[5..];
    let changes: Vec<f64> = run.iter().map(|update| update.fixed[0]).collect();
    assert!(
        (changes[30] - 0.02).abs() > 0.002,
        "fixed at 0.09 m is still more than 0.002 m away at 30 s: {}",
        changes[30]
    );
    let middle: Vec<f64> = run.iter().map(|update| update.fixed[1]).collect();
    assert!(
        settled_after(&middle).expect("settles") <= 30,
        "fixed at its 0.25 m design level meets the requirement"
    );
}

#[test]
fn lesson43_a_new_integral_gain_keeps_the_holding_flow() {
    // Control.PID adds increments to its retained request: the inflow changes by Kp·(Δr − Δy) + Ki·e·Δt with the
    // present Ki, and the accumulated holding flow stays as it is when Ki changes.
    let record = lesson43_run(5, 40);
    for pair in record[6..].windows(2) {
        let (previous, current) = (pair[0], pair[1]);
        let (y_previous, y) = (previous.scheduled[0] + 0.09, current.scheduled[0] + 0.09);
        let expected = 0.05 * -(y - y_previous) + current.ki[0] * (0.11 - y) * 1.0;
        assert!(
            (current.scheduled_inlet[0] - previous.scheduled_inlet[0] - expected).abs() < 1e-12,
            "increment with the present Ki only: {previous:?} → {current:?}"
        );
    }
    assert!(
        record[39].ki[0] < record[5].ki[0],
        "Ki fell as the level rose: {} → {}",
        record[5].ki[0],
        record[39].ki[0]
    );
}

const LESSON44: &str =
    "01-feedback-foundations-and-temperature-control/18-transfer-function-from-physics";

/// One 0.1 s update of the P-controlled heater with the two predictions.
#[derive(Clone, Copy, Debug)]
struct Heater {
    temperature: f64,
    setpoint: f64,
    power: f64,
    no_delay: f64,
    delayed: f64,
}

/// `rest` updates at 35 °C, then Control Studio's project experiment (activation at %MX24.0), which steps the
/// setpoint to 36 °C, for `updates` more.
fn lesson44_run(rest: u64, updates: u64) -> Vec<Heater> {
    let (mut runtime, mut simulation) = setup(LESSON44);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Heater {
            temperature: number(&runtime, "%IL0"),
            setpoint: number(&runtime, "%ML0"),
            power: number(&runtime, "%QL0"),
            no_delay: number(&runtime, "%ML8"),
            delayed: number(&runtime, "%ML16"),
        });
    }
    record
}

#[test]
fn lesson44_p_control_settles_at_the_closed_loop_gain() {
    // H(s) = 0.2·Kp/(5s + 1 + 0.2·Kp) with Kp = 5: gain 0.5, so the heater settles at 35.5 °C with 77.5 %.
    let record = lesson44_run(20, 400);
    let last = record[record.len() - 1];
    assert_eq!(last.setpoint, 36.0, "the setpoint steps to 36 °C");
    assert!(
        (last.temperature - 35.5).abs() <= 0.05,
        "P control leaves the water at 35.5 °C: {last:?}"
    );
    assert!((last.power - 77.5).abs() <= 0.2, "75 % + 5 · 0.5: {last:?}");
    assert!(
        record[..20]
            .iter()
            .all(|update| (update.power - 75.0).abs() <= 0.2),
        "at rest before the step: {:?}",
        &record[..20]
    );
}

#[test]
fn lesson44_the_transfer_function_predicts_the_delay_free_response() {
    // x(t) = 0.5·(1 − e^(−t/2.5)): 35.316 °C after 2.5 s.
    let record = lesson44_run(20, 60);
    assert!(
        (record[20].no_delay - 35.0).abs() < 1e-12,
        "the prediction starts at the step: {:?}",
        record[20]
    );
    assert!(
        (record[45].no_delay - (35.0 + 0.5 * (1.0 - (-1.0_f64).exp()))).abs() < 1e-9,
        "one closed-loop time constant: {:?}",
        record[45]
    );
}

#[test]
fn lesson44_the_delayed_model_follows_the_heater_and_the_delay_free_one_does_not() {
    let record = lesson44_run(20, 200);
    let run = &record[20..];
    for update in run {
        assert!(
            (update.temperature - update.delayed).abs() <= 0.03,
            "the loop recomputed with the 1 s delay: {update:?}"
        );
    }
    let miss = run[..50]
        .iter()
        .map(|update| (update.temperature - update.no_delay).abs())
        .fold(0.0, f64::max);
    assert!(
        miss >= 0.1,
        "without the delay the transfer function misses the heater by {miss} °C"
    );
}

const LESSON45: &str = "02-derivative-action-and-dynamic-response/19-poles-zeros-and-response";

/// One 20 ms update of the heater and the shaft with their pole predictions.
#[derive(Clone, Copy, Debug)]
struct Poles {
    temperature: f64,
    heater_prediction: f64,
    angle: f64,
    shaft_prediction: f64,
}

/// `rest` updates, then Control Studio's project experiment (activation at %MX24.0): the heater command returns from
/// 80 % to 75 % and the shaft target steps to 1 rad.
fn lesson45_run(rest: u64, updates: u64) -> Vec<Poles> {
    let (mut runtime, mut simulation) = setup(LESSON45);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..rest + updates {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 20, 20, &[]);
        record.push(Poles {
            temperature: number(&runtime, "%IL0"),
            heater_prediction: number(&runtime, "%ML8"),
            angle: number(&runtime, "%IL8"),
            shaft_prediction: number(&runtime, "%ML16"),
        });
    }
    record
}

#[test]
fn lesson45_the_heater_decays_with_its_real_pole() {
    // Pole −0.2 s⁻¹: after the 1 s delivery delay the extra degree decays as e^(−0.2·(t − 1)), to 35.368 °C at 6 s.
    let record = lesson45_run(25, 1000);
    let run = &record[25..];
    for update in run {
        assert!(
            (update.temperature - update.heater_prediction).abs() <= 0.03,
            "the pole's decay: {update:?}"
        );
    }
    assert!(
        (run[300].heater_prediction - (35.0 + (-1.0_f64).exp())).abs() < 1e-9,
        "one time constant after the delay: {:?}",
        run[300]
    );
}

#[test]
fn lesson45_the_shaft_oscillates_with_its_complex_poles() {
    // Kp = 4 on J = 1, B = 1: s² + s + 4, poles −0.5 ± j1.936. Peaks every 2π/1.936 = 3.245 s, the first at
    // π/1.936 = 1.62 s with 1 + e^(−0.5·1.62) = 1.444 rad.
    let record = lesson45_run(25, 500);
    let angles: Vec<f64> = record[25..].iter().map(|update| update.angle).collect();
    let peaks: Vec<usize> = (1..angles.len() - 1)
        .filter(|&k| angles[k] > angles[k - 1] && angles[k] >= angles[k + 1] && angles[k] > 1.0)
        .collect();
    assert!(
        peaks.len() >= 2,
        "at least two peaks above the target: {peaks:?}"
    );
    let first = peaks[0] as f64 * 0.02;
    assert!(
        (first - 1.62).abs() <= 0.06,
        "first peak at 1.62 s: {first}"
    );
    assert!(
        (angles[peaks[0]] - 1.444).abs() <= 0.03,
        "first peak 1.444 rad: {}",
        angles[peaks[0]]
    );
    let period = (peaks[1] - peaks[0]) as f64 * 0.02;
    assert!((period - 3.245).abs() <= 0.06, "period 3.245 s: {period}");
    for update in &record[25..] {
        assert!(
            (update.angle - update.shaft_prediction).abs() <= 0.03,
            "the poles' response: {update:?}"
        );
    }
}
