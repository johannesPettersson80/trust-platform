use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON61: &str = "14-integrated-capstones/83-reactor-control";

fn flag(runtime: &trust_runtime::CompiledRuntime, address: &str) -> bool {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Bool(value) => value,
        other => panic!("expected BOOL at {address}, got {other:?}"),
    }
}

/// One 0.1 s update of the five jacketed vessels, ordered [single loop and cascade on the 35 → 37 °C step, single loop
/// and cascade on the 30 kW jacket loss, lifecycle vessel], with the lifecycle vessel's mode and fault latch.
#[derive(Clone, Copy, Debug)]
struct Vessel {
    #[allow(dead_code)] // shown in failure messages
    jacket: [f64; 5],
    vessel: [f64; 5],
    heater: [f64; 5],
    auto: bool,
    fault: bool,
}

/// One update at rest, then Control Studio's project experiment (activation at %MX48.0) for `updates`.
fn lesson61_run(updates: u64) -> Vec<Vessel> {
    let (mut runtime, mut simulation) = setup(LESSON61);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX48.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Vessel {
            jacket: std::array::from_fn(|k| number(&runtime, &format!("%IL{}", 16 * k))),
            vessel: std::array::from_fn(|k| number(&runtime, &format!("%IL{}", 16 * k + 8))),
            heater: std::array::from_fn(|k| number(&runtime, &format!("%QL{}", 16 * k))),
            auto: flag(&runtime, "%MX48.1"),
            fault: flag(&runtime, "%MX48.2"),
        });
    }
    record
}

#[test]
fn lesson61_both_arrangements_meet_the_step_and_only_the_cascade_meets_the_jacket_loss() {
    // Rest: 35 °C vessel, 38 °C jacket, 15 %. Step 35 → 37 °C: peak ≤ 38 °C and |error| ≤ 0.2 °C from 30 s for both, holding
    // the balance's 17 %. 30 kW jacket loss: the cascade keeps ≥ 34 °C and |error| ≤ 0.2 °C from 30 s and holds the
    // balance's 30 %; the single loop falls below 34 °C.
    let record = lesson61_run(600);
    assert!(
        record[0].heater.iter().all(|u| (u - 15.0).abs() < 1e-9)
            && record[0].vessel.iter().all(|t| (t - 35.0).abs() < 1e-9),
        "rest: {:?}",
        record[0]
    );
    let run = &record[1..];
    let peak = |k: usize| run.iter().map(|u| u.vessel[k]).fold(f64::MIN, f64::max);
    let minimum = |k: usize| run.iter().map(|u| u.vessel[k]).fold(f64::MAX, f64::min);
    let late = |k: usize, target: f64| {
        run[300..]
            .iter()
            .map(|u| (u.vessel[k] - target).abs())
            .fold(0.0, f64::max)
    };
    for k in 0..2 {
        assert!(
            peak(k) <= 38.0 && late(k, 37.0) <= 0.2,
            "step {k}: peak {} late {}",
            peak(k),
            late(k, 37.0)
        );
    }
    assert!(
        minimum(3) >= 34.0 && late(3, 35.0) <= 0.2,
        "cascade, jacket loss: {} {}",
        minimum(3),
        late(3, 35.0)
    );
    assert!(
        minimum(2) < 34.0,
        "single loop, jacket loss: {}",
        minimum(2)
    );
    let last = run.last().unwrap();
    assert!(
        (last.heater[1] - 17.0).abs() < 0.1 && (last.heater[3] - 30.0).abs() < 0.1,
        "holding commands: {last:?}"
    );
}

#[test]
fn lesson61_the_recommended_cascade_transfers_without_a_bump_and_latches_a_sensor_fault() {
    // Lifecycle vessel, setpoint 37 °C from the start: Manual holds 15 % until 10 s. The transfer update keeps 15 %, and the
    // next adds only the integral steps of Lesson 63's cascade design (outer Ki 0.21, inner Kp 25.25, Ki 13.59): the jacket
    // setpoint rises 0.21·2·0.1 = 0.042 °C and the heater 25.25·0.042 + 13.59·0.042·0.1 = 1.118 points. The vessel then heats
    // toward 37 °C. From 48 to 50 s the vessel reading is 0 °C: in that update the heater goes to 0 %, and it stays there
    // with the latch set after the reading recovers.
    let record = lesson61_run(600);
    let run = &record[1..];
    assert!(
        run[..100]
            .iter()
            .all(|u| !u.auto && (u.heater[4] - 15.0).abs() < 1e-9),
        "manual holding"
    );
    assert!(
        run[100].auto && (run[100].heater[4] - 15.0).abs() < 1e-9,
        "transfer update: {:?}",
        run[100]
    );
    assert!(
        (run[101].heater[4] - (15.0 + 25.25 * 0.042 + 13.59 * 0.042 * 0.1)).abs() < 1e-6,
        "only the integral steps of Lesson 63's design after the transfer: {:?}",
        run[101]
    );
    assert!(
        run[479].auto && !run[479].fault && run[479].vessel[4] > 36.8,
        "Auto heats toward 37 °C: {:?}",
        run[479]
    );
    assert!(
        run[480..]
            .iter()
            .all(|u| u.fault && !u.auto && u.heater[4] == 0.0),
        "latched zero heat from 48 s"
    );
}

const LESSON62: &str = "10-multivariable-control/61-mimo-paths";

/// One 0.1 s update of the two coupled vessels: measured and model temperatures, commands and the model's wall flow.
#[derive(Clone, Copy, Debug)]
struct Paths {
    measured: [f64; 2],
    model: [f64; 2],
    heater: [f64; 2],
    transfer: f64,
}

/// One update at rest, then Control Studio's project experiment (activation at %MX24.0) for `updates`.
fn lesson62_run(updates: u64) -> Vec<Paths> {
    let (mut runtime, mut simulation) = setup(LESSON62);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX24.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Paths {
            measured: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            model: [number(&runtime, "%ML0"), number(&runtime, "%ML8")],
            heater: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            transfer: number(&runtime, "%ML16"),
        });
    }
    record
}

fn mean(window: &[Paths], value: impl Fn(&Paths) -> f64) -> f64 {
    window.iter().map(value).sum::<f64>() / window.len() as f64
}

#[test]
fn lesson62_each_heater_moves_both_vessels_by_its_column_of_the_gain_matrix() {
    // K = [[9/65, 4/65], [4/65, 9/65]] °C per point. Heater 1 at 80 % for 30 s with heater 2 fixed, back to 75 % for 30 s,
    // then heater 2 at 80 %: the last 5 s of each hold, less its baseline, match 5·K's columns within ±0.02 °C.
    let record = lesson62_run(900);
    assert!(
        record[0].measured.iter().all(|t| (t - 35.0).abs() < 1e-9)
            && record[0].heater == [75.0, 75.0],
        "rest: {:?}",
        record[0]
    );
    let run = &record[1..];
    for (i, u) in run.iter().enumerate() {
        let expected = if i < 300 {
            [80.0, 75.0]
        } else if i < 600 {
            [75.0, 75.0]
        } else {
            [75.0, 80.0]
        };
        assert_eq!(
            u.heater,
            expected,
            "open-loop commands at {:.1} s",
            i as f64 * 0.1
        );
    }
    let (own, cross) = (5.0 * 9.0 / 65.0, 5.0 * 4.0 / 65.0);
    let first = [
        mean(&run[250..300], |u| u.measured[0]) - 35.0,
        mean(&run[250..300], |u| u.measured[1]) - 35.0,
    ];
    let base = [
        mean(&run[550..600], |u| u.measured[0]),
        mean(&run[550..600], |u| u.measured[1]),
    ];
    let second = [
        mean(&run[850..900], |u| u.measured[0]) - base[0],
        mean(&run[850..900], |u| u.measured[1]) - base[1],
    ];
    assert!(
        (first[0] - own).abs() <= 0.02 && (first[1] - cross).abs() <= 0.02,
        "heater 1 paths: {first:?}"
    );
    assert!(
        (second[0] - cross).abs() <= 0.02 && (second[1] - own).abs() <= 0.02,
        "heater 2 paths: {second:?}"
    );
}

#[test]
fn lesson62_the_model_follows_both_vessels_and_its_wall_flow_reverses() {
    // The ST model integrates both balances from the commands; it stays within 0.01 °C of the measured vessels. Its wall
    // flow 8·(T1 − T2) carries heat from vessel 1 to vessel 2 while heater 1 is raised, and back while heater 2 is.
    let record = lesson62_run(900);
    let run = &record[1..];
    for (i, u) in run.iter().enumerate() {
        assert!(
            (u.model[0] - u.measured[0]).abs() <= 0.01
                && (u.model[1] - u.measured[1]).abs() <= 0.01,
            "model at {:.1} s: {u:?}",
            i as f64 * 0.1
        );
    }
    assert!(
        run[10..300].iter().all(|u| u.transfer > 0.0)
            && run[610..900].iter().all(|u| u.transfer < 0.0),
        "wall flow direction"
    );
    assert!(
        (run[299].transfer - 8.0 * (run[299].measured[0] - run[299].measured[1])).abs() < 0.05,
        "wall flow: {:?}",
        run[299]
    );
}

const LESSON63: &str = "10-multivariable-control/62-relative-gain-array";

/// One 0.1 s update of both vessel pairs, ordered [isolated 1, isolated 2, compensated 1, compensated 2].
#[derive(Clone, Copy, Debug)]
struct Pairs {
    vessel: [f64; 4],
    heater: [f64; 4],
}

/// One update at rest, then Control Studio's project experiment (activation at %MX8.0) for `updates`.
fn lesson63_run(updates: u64) -> Vec<Pairs> {
    let (mut runtime, mut simulation) = setup(LESSON63);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX8.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Pairs {
            vessel: std::array::from_fn(|k| number(&runtime, &format!("%IL{}", 8 * k))),
            heater: std::array::from_fn(|k| number(&runtime, &format!("%QL{}", 8 * k))),
        });
    }
    record
}

#[test]
fn lesson63_holding_the_other_vessel_lowers_the_path_gain_to_one_ninth() {
    // Isolated: heater 1 +5 points raises vessel 1 by 45/65 °C. Compensated: heater 3 +5 with heater 4 corrected by
    // −(4/9)·5 raises vessel 3 by 5/9 °C and returns vessel 4 to 35 °C, within ±0.02 °C in the last 5 s of each 30 s hold;
    // the same from 60 s for heater 4. The gain ratio (9/65)/(1/9) = 81/65 is the relative gain.
    let record = lesson63_run(900);
    let run = &record[1..];
    let correction = 75.0 - 20.0 / 9.0;
    for (i, u) in run.iter().enumerate() {
        let expected = if i < 300 {
            [80.0, 75.0, 80.0, correction]
        } else if i < 600 {
            [75.0; 4]
        } else {
            [75.0, 80.0, correction, 80.0]
        };
        assert!(
            u.heater
                .iter()
                .zip(expected)
                .all(|(a, b)| (a - b).abs() < 1e-9),
            "commands at {:.1} s: {:?}",
            i as f64 * 0.1,
            u.heater
        );
    }
    let window = |range: std::ops::Range<usize>, k: usize| {
        run[range].iter().map(|u| u.vessel[k]).sum::<f64>() / 50.0
    };
    let isolated = window(250..300, 0) - 35.0;
    let (held, other) = (window(250..300, 2) - 35.0, window(250..300, 3) - 35.0);
    assert!(
        (isolated - 45.0 / 65.0).abs() <= 0.02
            && (held - 5.0 / 9.0).abs() <= 0.02
            && other.abs() <= 0.02,
        "heater 1/3: {isolated} {held} {other}"
    );
    let (held2, other2) = (
        window(850..900, 3) - window(550..600, 3),
        window(850..900, 2) - window(550..600, 2),
    );
    assert!(
        (held2 - 5.0 / 9.0).abs() <= 0.02 && other2.abs() <= 0.02,
        "heater 4: {held2} {other2}"
    );
    assert!(
        (isolated / held - 81.0 / 65.0).abs() <= 0.05,
        "relative gain {}",
        isolated / held
    );
}

#[test]
fn lesson63_a_fixed_correction_holds_the_other_vessel_only_at_equilibrium() {
    // The correction lowers vessel 4 at once, while the heat from heater 3 reaches it only through the wall: vessel 4
    // dips by about 0.09 °C near 3 s before it returns to 35 °C.
    let record = lesson63_run(300);
    let run = &record[1..];
    let (at, low) = run
        .iter()
        .enumerate()
        .map(|(i, u)| (i, u.vessel[3]))
        .fold((0, f64::MAX), |a, b| if b.1 < a.1 { b } else { a });
    assert!(
        low < 34.95 && low > 34.85 && (20..45).contains(&at),
        "vessel 4 dip {low} at {:.1} s",
        at as f64 * 0.1
    );
}
