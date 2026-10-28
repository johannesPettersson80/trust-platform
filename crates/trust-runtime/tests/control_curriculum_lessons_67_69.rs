use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON67: &str = "02-derivative-action-and-dynamic-response/20-state-equations-from-stores";
const LESSON68: &str = "11-state-space-estimation-and-predictive-control/66-controllability";
const LESSON69: &str = "11-state-space-estimation-and-predictive-control/67-observability";

fn flag(runtime: &trust_runtime::CompiledRuntime, address: &str) -> bool {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Bool(value) => value,
        other => panic!("expected BOOL at {address}, got {other:?}"),
    }
}

/// One 1 s update of both cascades, ordered [raise, lower]: measured and model levels [tank 1, tank 2] and the pump flows.
#[derive(Clone, Copy, Debug)]
struct Cascade {
    tank: [[f64; 2]; 2],
    model: [[f64; 2]; 2],
    pump: [f64; 2],
    fault: bool,
}

/// One update at rest, then Control Studio's project experiment (activation at %MX32.0). `period_ms` gives each update's
/// clock step, so a test can hand the program a wrong interval.
fn lesson67_run(periods_ms: &[u64]) -> Vec<Cascade> {
    lesson67_run_with_readings(periods_ms, &[])
}

/// As `lesson67_run`, and each `(update, address, level)` replaces that level reading for one update. The process image
/// holds an LREAL input as its raw 64 bits.
fn lesson67_run_with_readings(periods_ms: &[u64], readings: &[(usize, &str, f64)]) -> Vec<Cascade> {
    let (mut runtime, mut simulation) = setup(LESSON67);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&1000).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX32.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, level)| (*address, Value::LWord(level.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Cascade {
            tank: [
                [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
                [number(&runtime, "%IL16"), number(&runtime, "%IL24")],
            ],
            model: [
                [number(&runtime, "%ML0"), number(&runtime, "%ML8")],
                [number(&runtime, "%ML16"), number(&runtime, "%ML24")],
            ],
            pump: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            fault: flag(&runtime, "%MX32.2"),
        });
    }
    record
}

#[test]
fn lesson67_the_state_space_model_follows_both_tanks_through_a_pump_change() {
    // From 0.25/0.25 m at 0.010 m³/s the pump runs 0.011 (raise) or 0.009 m³/s (lower) for 20 s, then 0.010 again. Tank 1
    // moves first; tank 2 keeps moving for more than 10 s after the pump has returned, fed by the water stored in tank 1.
    // The model x' = Ax + Bv, advanced from both old states once per second, stays within 0.001 m of both measured levels.
    let record = lesson67_run(&[1000; 120]);
    let run = &record[1..];
    for (i, u) in run.iter().enumerate() {
        let pulse = if i < 20 { 0.001 } else { 0.0 };
        assert!(
            (u.pump[0] - (0.010 + pulse)).abs() < 1e-12
                && (u.pump[1] - (0.010 - pulse)).abs() < 1e-12,
            "pumps at {i} s: {:?}",
            u.pump
        );
        for c in 0..2 {
            for t in 0..2 {
                assert!(
                    (u.model[c][t] - u.tank[c][t]).abs() <= 0.001,
                    "case {c} tank {t} at {i} s: model {} measured {}",
                    u.model[c][t],
                    u.tank[c][t]
                );
            }
        }
    }
    assert!(
        run[1].tank[0][0] - 0.25 > 10.0 * (run[1].tank[0][1] - 0.25),
        "tank 1 first: {:?}",
        run[1].tank[0]
    );
    let peak = (0..run.len())
        .max_by(|&a, &b| run[a].tank[0][1].total_cmp(&run[b].tank[0][1]))
        .unwrap();
    let low = (0..run.len())
        .min_by(|&a, &b| run[a].tank[1][1].total_cmp(&run[b].tank[1][1]))
        .unwrap();
    assert!(
        peak > 30 && low > 30,
        "tank 2 turns at {peak} s and {low} s"
    );
}

#[test]
fn lesson67_each_model_step_uses_both_old_states() {
    // Raise case: the model starts from the measured 0.25/0.25 m, x = 0. With v = 0.001 the first step gives x' = [0.002, 0],
    // so [0.252, 0.25] m after 1 s; the second, from the old [0.002, 0], gives x' = [0.002 − 0.04·0.002, 0.04·0.002] =
    // [0.00192, 0.00008], so [0.25392, 0.25008] m after 2 s.
    let record = lesson67_run(&[1000; 3]);
    assert!(
        (record[1].model[0][0] - 0.25).abs() < 1e-12
            && (record[1].model[0][1] - 0.25).abs() < 1e-12,
        "start {:?}",
        record[1].model[0]
    );
    assert!(
        (record[2].model[0][0] - 0.252).abs() < 1e-12
            && (record[2].model[0][1] - 0.25).abs() < 1e-12,
        "1 s {:?}",
        record[2].model[0]
    );
    assert!(
        (record[3].model[0][0] - 0.25392).abs() < 1e-12
            && (record[3].model[0][1] - 0.25008).abs() < 1e-12,
        "2 s {:?}",
        record[3].model[0]
    );
}

#[test]
fn lesson67_a_wrong_interval_latches_zero_flow() {
    // The model advances one second per update. When the clock shows 2 s between two updates, the program latches a fault
    // and stops both pumps, and they stay stopped.
    let record = lesson67_run(&[1000, 1000, 2000, 1000, 1000]);
    assert!(
        !record[2].fault && record[2].pump[0] > 0.0,
        "normal before the gap: {:?}",
        record[2]
    );
    assert!(
        record[3..].iter().all(|u| u.fault && u.pump == [0.0, 0.0]),
        "latched after the gap: {:?}",
        &record[3..]
    );
}

#[test]
fn lesson67_a_level_outside_its_range_latches_zero_flow() {
    // The model is local to 0.25 m. A tank-2 reading of 0.46 m in one case, or a tank-1 reading of 0.04 m in the other,
    // is outside 0.05–0.45 m: the program latches a fault and stops both pumps, and they stay stopped after the readings
    // return to the measured levels.
    for (address, level) in [("%IL8", 0.46), ("%IL16", 0.04)] {
        let record = lesson67_run_with_readings(&[1000; 5], &[(3, address, level)]);
        assert!(
            !record[2].fault && record[2].pump[0] > 0.0,
            "{address}: normal before the reading: {:?}",
            record[2]
        );
        assert!(
            record[3..].iter().all(|u| u.fault && u.pump == [0.0, 0.0]),
            "{address}: latched after the reading: {:?}",
            &record[3..]
        );
        assert!(
            record[5]
                .tank
                .iter()
                .flatten()
                .all(|level| (0.05..=0.45).contains(level)),
            "{address}: readings restored: {:?}",
            record[5].tank
        );
    }
}

/// One 1 s update of the Lesson 75 cascade: measured and model levels [tank 1, tank 2], the pump flow and the fault latch.
#[derive(Clone, Copy, Debug)]
struct Steered {
    tank: [f64; 2],
    model: [f64; 2],
    pump: f64,
    fault: bool,
}

/// One update at rest, then Control Studio's project experiment (activation at %MX16.0). `periods_ms` and `readings` as
/// in `lesson67_run_with_readings`.
fn lesson68_run(periods_ms: &[u64], readings: &[(usize, &str, f64)]) -> Vec<Steered> {
    let (mut runtime, mut simulation) = setup(LESSON68);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&1000).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX16.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, level)| (*address, Value::LWord(level.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Steered {
            tank: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            model: [number(&runtime, "%ML0"), number(&runtime, "%ML8")],
            pump: number(&runtime, "%QL0"),
            fault: flag(&runtime, "%MX16.2"),
        });
    }
    record
}

#[test]
fn lesson68_two_pulses_return_tank_1_to_rest_with_tank_2_raised_and_it_cannot_stay() {
    // The pump runs 0.011 m³/s for 20 s and 0.009 m³/s for 11 s, the whole updates nearest the model's 10.97 s, then
    // 0.010 m³/s. At 31 s tank 1 is back within 0.5 mm of 0.25 m while tank 2 stands more than 9 mm above it: one pump
    // moved the two levels apart. No flow can hold them apart, because tank 2's balance is steady only at equal levels:
    // by 60 s tank 2 has drained below 4 mm while tank 1 stayed at rest. The model follows both levels within 0.001 m.
    let record = lesson68_run(&[1000; 120], &[]);
    let run = &record[1..];
    for (i, u) in run.iter().enumerate() {
        let deviation = if i < 20 {
            0.001
        } else if i < 31 {
            -0.001
        } else {
            0.0
        };
        assert!(
            (u.pump - (0.010 + deviation)).abs() < 1e-12,
            "pump at {i} s: {}",
            u.pump
        );
        for t in 0..2 {
            assert!(
                (u.model[t] - u.tank[t]).abs() <= 0.001,
                "tank {} at {i} s: model {} measured {}",
                t + 1,
                u.model[t],
                u.tank[t]
            );
        }
    }
    assert!(
        (run[31].tank[0] - 0.25).abs() < 0.0005 && run[31].tank[1] - 0.25 > 0.009,
        "steered at 31 s: {:?}",
        run[31].tank
    );
    assert!(
        (run[60].tank[0] - 0.25).abs() < 0.0005 && run[60].tank[1] - 0.25 < 0.004,
        "drained by 60 s: {:?}",
        run[60].tank
    );
}

#[test]
fn lesson68_a_wrong_interval_latches_zero_flow() {
    // As in Lesson 74: 2 s between two updates latches a fault and stops the pump, and it stays stopped.
    let record = lesson68_run(&[1000, 1000, 2000, 1000, 1000], &[]);
    assert!(
        !record[2].fault && record[2].pump > 0.0,
        "normal before the gap: {:?}",
        record[2]
    );
    assert!(
        record[3..].iter().all(|u| u.fault && u.pump == 0.0),
        "latched after the gap: {:?}",
        &record[3..]
    );
}

#[test]
fn lesson68_a_level_outside_its_range_latches_zero_flow() {
    // As in Lesson 74: a reading outside 0.05–0.45 m latches zero flow, which stays after the reading returns.
    for (address, level) in [("%IL8", 0.46), ("%IL0", 0.04)] {
        let record = lesson68_run(&[1000; 5], &[(3, address, level)]);
        assert!(
            !record[2].fault && record[2].pump > 0.0,
            "{address}: normal before the reading: {:?}",
            record[2]
        );
        assert!(
            record[3..].iter().all(|u| u.fault && u.pump == 0.0),
            "{address}: latched after the reading: {:?}",
            &record[3..]
        );
    }
}

/// One 1 s update of the Lesson 76 rig: both true tank-1 levels [clean, noisy], the noisy tank-2 reading, both
/// reconstructions of tank 1 [clean, noisy], the error statistics [clean RMS, clean peak, noisy RMS, noisy peak], the
/// pumps and the fault latch.
#[derive(Clone, Copy, Debug)]
struct Observed {
    tank1: [f64; 2],
    noisy_reading: f64,
    reconstruction: [f64; 2],
    statistics: [f64; 4],
    pump: [f64; 2],
    fault: bool,
}

/// One update at rest, then Control Studio's project experiment (activation at %MX56.0). `periods_ms` and `readings` as
/// in `lesson67_run_with_readings`.
fn lesson69_run(periods_ms: &[u64], readings: &[(usize, &str, f64)]) -> Vec<Observed> {
    let (mut runtime, mut simulation) = setup(LESSON69);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&1000).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX56.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, level)| (*address, Value::LWord(level.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Observed {
            tank1: [number(&runtime, "%IL0"), number(&runtime, "%IL16")],
            noisy_reading: number(&runtime, "%ML8"),
            reconstruction: [number(&runtime, "%ML0"), number(&runtime, "%ML16")],
            statistics: [
                number(&runtime, "%ML24"),
                number(&runtime, "%ML32"),
                number(&runtime, "%ML40"),
                number(&runtime, "%ML48"),
            ],
            pump: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            fault: flag(&runtime, "%MX56.2"),
        });
    }
    record
}

#[test]
fn lesson69_tank_2_history_reconstructs_tank_1_and_noise_moves_it_by_a_quarter_metre() {
    // Both cascades get Lesson 75's pulse pair. Tank 1 is reconstructed from tank 2's readings alone as y + 25·Δy; the
    // first reading only primes Δy, so the first reconstruction appears one update after the start. The clean
    // reconstruction stays within 1.5 mm of the true tank 1. The noisy reading alternates −5 and +5 mm, so every Δy
    // carries ±10 mm and the noisy reconstruction differs from the clean one by ±(0.005 + 25·0.010) = ±0.255 m.
    let record = lesson69_run(&[1000; 120], &[]);
    // The sensor is noisy at every update, also before the experiment: its reading is tank 2 ± 5 mm, never empty.
    assert!(
        ((record[0].noisy_reading - 0.25).abs() - 0.005).abs() < 1e-12,
        "reading before the start: {}",
        record[0].noisy_reading
    );
    let run = &record[1..];
    for (i, u) in run.iter().enumerate() {
        let deviation = if i < 20 {
            0.001
        } else if i < 31 {
            -0.001
        } else {
            0.0
        };
        assert!(
            (u.pump[0] - (0.010 + deviation)).abs() < 1e-12 && u.pump[1] == u.pump[0],
            "pumps at {i} s: {:?}",
            u.pump
        );
    }
    assert!(
        run[0].reconstruction == [0.25, 0.25] && run[0].statistics == [0.0; 4],
        "no reconstruction from the priming reading: {:?}",
        run[0]
    );
    for (i, u) in run.iter().enumerate().skip(1) {
        assert!(
            (u.reconstruction[0] - u.tank1[0]).abs() <= 0.0015,
            "clean at {i} s: {} against {}",
            u.reconstruction[0],
            u.tank1[0]
        );
        let noise = (u.reconstruction[1] - u.reconstruction[0]).abs();
        assert!(
            (noise - 0.255).abs() < 1e-9,
            "noisy minus clean at {i} s: {noise}"
        );
    }
    let last = run[run.len() - 1].statistics;
    assert!(
        last[0] < 0.0006 && last[1] < 0.0015 && last[2] > 0.25 && last[3] > 0.25,
        "RMS and peak, clean then noisy: {last:?}"
    );
}

#[test]
fn lesson69_the_reconstruction_never_reads_tank_1() {
    // Tank 1's level is read only to score the reconstruction: a wrong tank-1 reading at update 5 changes that update's
    // error, never the reconstruction.
    let plain = lesson69_run(&[1000; 8], &[]);
    let misread = lesson69_run(&[1000; 8], &[(5, "%IL0", 0.30)]);
    for (a, b) in plain.iter().zip(&misread) {
        assert_eq!(
            a.reconstruction, b.reconstruction,
            "reconstruction with tank 1 misread: {a:?} {b:?}"
        );
    }
    assert!(
        misread[5].statistics[1] > 0.04 && plain[5].statistics[1] < 0.0015,
        "the misread tank 1 enters only the error: {:?} {:?}",
        plain[5],
        misread[5]
    );
}

#[test]
fn lesson69_a_wrong_interval_or_level_latches_zero_flow() {
    // As in Lessons 74 and 75: Δy needs one second between readings, so a 2 s gap latches zero flow; so does a reading
    // outside 0.05–0.45 m.
    let gap = lesson69_run(&[1000, 1000, 2000, 1000, 1000], &[]);
    assert!(
        !gap[2].fault && gap[2].pump[0] > 0.0,
        "normal before the gap: {:?}",
        gap[2]
    );
    assert!(
        gap[3..].iter().all(|u| u.fault && u.pump == [0.0, 0.0]),
        "latched after the gap: {:?}",
        &gap[3..]
    );
    let level = lesson69_run(&[1000; 5], &[(3, "%IL24", 0.46)]);
    assert!(
        level[3..].iter().all(|u| u.fault && u.pump == [0.0, 0.0]),
        "latched after the reading: {:?}",
        &level[3..]
    );
}
