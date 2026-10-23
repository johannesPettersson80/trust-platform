use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON70: &str = "11-state-space-estimation-and-predictive-control/68-state-feedback";
const LESSON71: &str = "11-state-space-estimation-and-predictive-control/70-state-observer";
const LESSON72: &str = "11-state-space-estimation-and-predictive-control/71-kalman-filter";

/// Control Studio's sampled pole placement for z = e^(−0.08), e^(−0.12) at 1 s: K = [K1, K2] and the reference gain N.
const STUDIO_GAINS: [f64; 3] = [0.05616014, 0.0369345, 0.11309464];

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

/// One 1 s update: both cascades' levels [controlled, uncorrected][tank 1, tank 2], the requested and both applied flows,
/// and the clipping and fault flags.
#[derive(Clone, Copy, Debug)]
struct Update {
    tank: [[f64; 2]; 2],
    requested: f64,
    pump: [f64; 2],
    clipped: bool,
    fault: [bool; 2],
}

/// Applies `gains` [K1, K2, N] and the feedback mode as Control Studio's Apply would and runs `settle` updates with tank 2's
/// setpoint at `setpoint`, as Apply's live check leaves it. Then Control Studio's project experiment (activation at
/// %MX16.0) runs `updates` updates. Each `(update, address, level)` replaces that reading for one update, counted from 0.
fn lesson70_run_after(
    gains: [f64; 3],
    feedback: bool,
    setpoint: f64,
    settle: usize,
    updates: usize,
    readings: &[(usize, &str, f64)],
) -> Vec<Update> {
    let (mut runtime, mut simulation) = setup(LESSON70);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    for (name, value) in ["K1", "K2", "N"].into_iter().zip(gains) {
        assert!(runtime
            .write_instance(p1, name, Value::LReal(value))
            .is_ok());
    }
    // The mode is written by symbol: the runtime refuses an Auto transfer whose mode flag is bound to I/O.
    assert!(runtime
        .write_instance(p1, "FeedbackEnabled", Value::Bool(feedback))
        .is_ok());
    debug.enqueue_io_write(
        IoAddress::parse("%ML0").unwrap(),
        Value::LWord(setpoint.to_bits()),
    );
    let mut record = Vec::new();
    for step in 0..=settle + updates {
        if step == settle + 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX16.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, level)| (*address, Value::LWord(level.to_bits())))
            .collect();
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step as u64 * 1000,
            1000,
            &writes,
        );
        record.push(Update {
            tank: [
                [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
                [number(&runtime, "%IL16"), number(&runtime, "%IL24")],
            ],
            requested: number(&runtime, "%ML8"),
            pump: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            clipped: flag(&runtime, "%MX16.2"),
            fault: [flag(&runtime, "%MX16.3"), flag(&runtime, "%MX16.4")],
        });
    }
    record
}

/// As `lesson70_run_after` from rest: no settling phase, the setpoint left unwritten.
fn lesson70_run(
    gains: [f64; 3],
    feedback: bool,
    updates: usize,
    readings: &[(usize, &str, f64)],
) -> Vec<Update> {
    lesson70_run_after(gains, feedback, 0.0, 0, updates, readings)
}

#[test]
fn lesson70_state_feedback_returns_both_levels_after_the_slug() {
    // Apply leaves tank 2's setpoint at 0.26 m. The experiment returns it to 0.25 m and both controlled levels are back
    // within 0.5 mm after 59 s. At 60 s both pumps run 0.020 m³/s for one second: a slug of 0.01 m³ puts tank 1 about
    // 0.02 m high. The controlled pump then requests q = 0.010 − K1(h1 − 0.25) − K2(h2 − 0.25), about 0.0089 m³/s at first,
    // and both levels are within 2 mm of rest 50 s after the slug. The uncorrected pump returns to 0.010 m³/s, and its
    // tank 2 is still more than 4 mm high then.
    let [k1, k2, _] = STUDIO_GAINS;
    let record = lesson70_run_after(STUDIO_GAINS, true, 0.26, 80, 181, &[]);
    assert!(
        (record[80].tank[0][1] - 0.26).abs() < 0.0005,
        "settled at the 0.26 m setpoint: {:?}",
        record[80].tank[0]
    );
    let run = &record[81..];
    assert!(
        run[59].tank[0]
            .iter()
            .all(|level| (level - 0.25).abs() < 0.0005),
        "back at 0.25 m: {:?}",
        run[59].tank[0]
    );
    assert!(run[60].pump == [0.020, 0.020], "slug: {:?}", run[60]);
    for (i, u) in run.iter().enumerate().skip(61) {
        let row = 0.010 - k1 * (u.tank[0][0] - 0.25) - k2 * (u.tank[0][1] - 0.25);
        assert!(
            (u.requested - row).abs() < 1e-12 && u.pump[0] == u.requested && !u.clipped,
            "row product at {i} s: {u:?}"
        );
        assert!(
            u.pump[1] == 0.010,
            "uncorrected pump at {i} s: {}",
            u.pump[1]
        );
    }
    assert!(
        (run[61].requested - 0.0089).abs() < 0.0001,
        "first correction: {}",
        run[61].requested
    );
    for u in &run[110..] {
        assert!(
            u.tank[0].iter().all(|level| (level - 0.25).abs() < 0.002),
            "controlled from 50 s after the slug: {:?}",
            u.tank[0]
        );
    }
    assert!(
        run[110].tank[1][1] - 0.25 > 0.004,
        "uncorrected tank 2 50 s after the slug: {}",
        run[110].tank[1][1]
    );
}

#[test]
fn lesson70_in_manual_the_pump_follows_the_manual_flow() {
    // Feedback disabled: after the slug the controlled pump delivers the Manual flow, 0.010 m³/s until Control Studio
    // writes another, like the uncorrected one. A written Manual flow of 0.012 m³/s is delivered as such.
    let record = lesson70_run(STUDIO_GAINS, false, 70, &[]);
    assert!(
        record[62..]
            .iter()
            .all(|u| u.pump == [0.010, 0.010] && u.requested == 0.010),
        "manual after the slug: {:?}",
        &record[62..]
    );
    let (mut runtime, mut simulation) = setup(LESSON70);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    assert!(runtime
        .write_instance(p1, "ManualFlowM3PerS", Value::LReal(0.012))
        .is_ok());
    scan_every(&mut runtime, &mut simulation, &debug, 0, 1000, &[]);
    assert_eq!(number(&runtime, "%QL0"), 0.012, "written Manual flow");
}

#[test]
fn lesson70_the_applied_flow_is_limited_and_clipping_reported() {
    // With K1 = 1 m²/s the first correction after the slug requests 0.010 − 0.0196 < 0: the pump applies 0 and reports
    // clipping instead of a negative flow.
    let record = lesson70_run([1.0, 0.0, 0.0], true, 62, &[]);
    assert!(
        record[62].requested < 0.0 && record[62].pump[0] == 0.0 && record[62].clipped,
        "limited: {:?}",
        record[62]
    );
}

#[test]
fn lesson70_each_cascade_latches_only_its_own_pump() {
    // A controlled tank-2 reading of 0.46 m latches zero flow on the controlled pump only; the uncorrected pump runs on.
    // The Validate trials simulate only the controlled cascade, so the other one must not stop it.
    let record = lesson70_run(STUDIO_GAINS, true, 5, &[(3, "%IL8", 0.46)]);
    assert!(
        record[3..]
            .iter()
            .all(|u| u.fault == [true, false] && u.pump == [0.0, 0.010]),
        "latched: {:?}",
        &record[3..]
    );
    let record = lesson70_run(STUDIO_GAINS, true, 5, &[(3, "%IL16", 0.04)]);
    assert!(
        record[3..]
            .iter()
            .all(|u| u.fault == [false, true] && u.pump[1] == 0.0 && u.pump[0] > 0.0),
        "latched: {:?}",
        &record[3..]
    );
}

#[test]
fn lesson70_a_setpoint_outside_the_levels_holds_the_operating_level() {
    // The setpoint is memory-bound and reads 0 before anything writes it. Outside 0.05–0.45 m it is replaced by 0.25 m,
    // so a reference gain cannot drive the tanks towards an empty target: at rest the pump stays at 0.010 m³/s.
    let record = lesson70_run(STUDIO_GAINS, true, 0, &[]);
    assert!(
        (record[0].requested - 0.010).abs() < 1e-12 && record[0].pump[0] == 0.010,
        "at rest: {:?}",
        record[0]
    );
}

/// One 1 s update of the Lesson 78 rig: the true tank levels, the noisy tank-2 reading, the four tank-1 estimates
/// [clean, clean fast, noisy, noisy fast], their RMS errors from 30 s after the observers start, the clean observer's
/// tank-2 estimate and innovation, the pump and the fault latch.
#[derive(Clone, Copy, Debug)]
struct Observer {
    tank: [f64; 2],
    reading: f64,
    estimate: [f64; 4],
    rms: [f64; 4],
    clean_tank2: f64,
    innovation: f64,
    pump: f64,
    fault: bool,
}

/// One update at rest, then Control Studio's project experiment (activation at %MX88.0).
fn lesson71_run(periods_ms: &[u64], readings: &[(usize, &str, f64)]) -> Vec<Observer> {
    let (mut runtime, mut simulation) = setup(LESSON71);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::once(&1000).chain(periods_ms.iter()).enumerate() {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX88.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, level)| (*address, Value::LWord(level.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Observer {
            tank: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            reading: number(&runtime, "%ML0"),
            estimate: [
                number(&runtime, "%ML8"),
                number(&runtime, "%ML16"),
                number(&runtime, "%ML24"),
                number(&runtime, "%ML32"),
            ],
            rms: [
                number(&runtime, "%ML40"),
                number(&runtime, "%ML48"),
                number(&runtime, "%ML56"),
                number(&runtime, "%ML64"),
            ],
            clean_tank2: number(&runtime, "%ML72"),
            innovation: number(&runtime, "%ML80"),
            pump: number(&runtime, "%QL0"),
            fault: flag(&runtime, "%MX88.2"),
        });
    }
    record
}

#[test]
fn lesson71_observers_start_from_the_reading_and_converge_on_tank_1() {
    // The first experiment update pours a slug into tank 1: 0.020 m³/s for one second. The observers start one update
    // later with the hidden tank 1 guessed at rest, x̂1 = 0, and tank 2 taken from its reading, so the first innovation is
    // zero. With exact readings the estimate of tank 1 is within 2 mm of the true level from 25 s with L = [0.48, 0.28] and
    // from 15 s with the stronger L = [2.52, 0.64].
    let record = lesson71_run(&[1000; 122], &[]);
    let run = &record[1..];
    assert!(
        run[0].pump == 0.020 && run[1..].iter().all(|u| u.pump == 0.010),
        "slug then 0.010: {} {:?}",
        run[0].pump,
        run[1].pump
    );
    let start = run[1];
    assert!(
        start.estimate[0] == 0.25 && start.clean_tank2 == start.tank[1] && start.innovation == 0.0,
        "start: {start:?}"
    );
    assert!(
        start.tank[0] - 0.25 > 0.019,
        "the slug is in tank 1: {:?}",
        start.tank
    );
    for (i, u) in run.iter().enumerate() {
        if i >= 26 {
            assert!(
                (u.estimate[0] - u.tank[0]).abs() < 0.002,
                "clean L at {} s: {} against {}",
                i - 1,
                u.estimate[0],
                u.tank[0]
            );
        }
        if i >= 16 {
            assert!(
                (u.estimate[1] - u.tank[0]).abs() < 0.002,
                "clean fast at {} s: {} against {}",
                i - 1,
                u.estimate[1],
                u.tank[0]
            );
        }
    }
    assert!(
        (run[11].estimate[0] - run[11].tank[0]).abs() > 0.005,
        "L is still converging after 10 s: {:?}",
        run[11]
    );
}

#[test]
fn lesson71_the_stronger_gain_passes_more_reading_noise() {
    // Readings 5 mm off in alternate directions. From 30 s after the observers start, the RMS error of the tank-1 estimate
    // is below 0.5 mm with exact readings, below 2 mm with noisy readings and L = [0.48, 0.28], and above 6 mm with noisy
    // readings and the stronger L = [2.52, 0.64].
    let record = lesson71_run(&[1000; 122], &[]);
    let last = record[record.len() - 1];
    assert!(
        last.rms[0] < 0.0005 && last.rms[2] < 0.002 && last.rms[3] > 0.006,
        "RMS clean, clean fast, noisy, noisy fast: {:?}",
        last.rms
    );
    assert!(
        (last.reading - last.tank[1]).abs() > 0.004,
        "the reading is noisy: {} against {}",
        last.reading,
        last.tank[1]
    );
}

#[test]
fn lesson71_the_observers_never_read_tank_1() {
    // Tank 1's level is read only to score the estimates: wrong tank-1 readings at the observers' start and at update 5
    // change no estimate.
    let plain = lesson71_run(&[1000; 8], &[]);
    let misread = lesson71_run(&[1000; 8], &[(2, "%IL0", 0.30), (5, "%IL0", 0.30)]);
    for (a, b) in plain.iter().zip(&misread) {
        assert_eq!(
            a.estimate, b.estimate,
            "estimates with tank 1 misread: {a:?} {b:?}"
        );
    }
}

#[test]
fn lesson71_a_wrong_interval_or_level_latches_zero_flow() {
    // As in Lessons 74 to 77: the observers advance one second per update, so a 2 s gap latches zero flow; so does a
    // reading outside 0.05–0.45 m.
    let gap = lesson71_run(&[1000, 1000, 2000, 1000, 1000], &[]);
    assert!(
        !gap[2].fault && gap[3..].iter().all(|u| u.fault && u.pump == 0.0),
        "latched after the gap: {:?}",
        &gap[2..]
    );
    let level = lesson71_run(&[1000; 5], &[(3, "%IL8", 0.46)]);
    assert!(
        level[3..].iter().all(|u| u.fault && u.pump == 0.0),
        "latched after the reading: {:?}",
        &level[3..]
    );
}

/// One 1 s update of the Lesson 79 rig: the true levels, the noisy tank-2 reading, the Kalman filter's predicted and
/// corrected tank-1 estimates, its gain on tank 1, the innovation, its own standard deviation of tank 1's estimate, the
/// RMS and peak errors from 31 s, the pump and the fault latch.
#[derive(Clone, Copy, Debug)]
struct Filtered {
    tank: [f64; 2],
    reading: f64,
    predicted: f64,
    corrected: f64,
    gain: f64,
    innovation: f64,
    filter_sd: f64,
    rms: f64,
    claimed: f64,
    #[allow(dead_code)] // shown in failure messages
    peak: f64,
    pump: f64,
    fault: bool,
}

/// Sets the assumed reading standard deviation as Control Studio's declared experiment setting does, runs `rest` updates at
/// rest, then the project experiment (activation at %MX72.0).
fn lesson72_run_after(
    rest: usize,
    sd_m: f64,
    periods_ms: &[u64],
    readings: &[(usize, &str, f64)],
) -> Vec<Filtered> {
    let (mut runtime, mut simulation) = setup(LESSON72);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    assert!(runtime
        .write_instance(p1, "AssumedReadingSdM", Value::LReal(sd_m))
        .is_ok());
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::repeat_n(&1000, rest)
        .chain(periods_ms.iter())
        .enumerate()
    {
        if step == rest {
            debug.enqueue_io_write(IoAddress::parse("%MX72.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, level)| (*address, Value::LWord(level.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Filtered {
            tank: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            reading: number(&runtime, "%ML0"),
            predicted: number(&runtime, "%ML8"),
            corrected: number(&runtime, "%ML16"),
            gain: number(&runtime, "%ML56"),
            innovation: number(&runtime, "%ML64"),
            filter_sd: number(&runtime, "%ML32"),
            rms: number(&runtime, "%ML40"),
            claimed: number(&runtime, "%ML80"),
            peak: number(&runtime, "%ML48"),
            pump: number(&runtime, "%QL0"),
            fault: flag(&runtime, "%MX72.2"),
        });
    }
    record
}

/// As `lesson72_run_after` with one update at rest.
fn lesson72_run(sd_m: f64, periods_ms: &[u64], readings: &[(usize, &str, f64)]) -> Vec<Filtered> {
    lesson72_run_after(1, sd_m, periods_ms, readings)
}

#[test]
fn lesson72_the_filter_predicts_corrects_and_converges_on_tank_1() {
    // The reading of tank 2 carries noise with a standard deviation of 5 mm, and the filter assumes exactly that. After the
    // unseen slug the filter starts with tank 1 at rest and tank 2 from its reading. Every correction is the prediction plus
    // the gain times the innovation, and tank 1's estimate is within 2 mm of the true level from 30 s, with an RMS error
    // below 0.5 mm from 31 s.
    let record = lesson72_run(0.005, &[1000; 122], &[]);
    let run = &record[1..];
    assert!(
        run[0].pump == 0.020 && run[1..].iter().all(|u| u.pump == 0.010),
        "slug then 0.010"
    );
    assert!(
        run[1].corrected == 0.25 && run[1].innovation == 0.0 && run[1].tank[0] - 0.25 > 0.019,
        "start: {:?}",
        run[1]
    );
    for (i, u) in run.iter().enumerate().skip(2) {
        assert!(
            (u.corrected - (u.predicted + u.gain * u.innovation)).abs() < 1e-12,
            "correction at {i} s: {u:?}"
        );
        if i >= 30 {
            assert!(
                (u.corrected - u.tank[0]).abs() < 0.002,
                "estimate at {i} s: {} against {}",
                u.corrected,
                u.tank[0]
            );
        }
    }
    assert!(
        run[run.len() - 1].rms < 0.0005,
        "RMS from 31 s: {}",
        run[run.len() - 1].rms
    );
}

#[test]
fn lesson72_a_wrong_assumed_uncertainty_costs_accuracy() {
    // The same readings with the assumed standard deviation 20 times too small, 0.25 mm, and 20 times too large, 100 mm.
    // Both estimates have a larger RMS error from 31 s than with the correct 5 mm. The filter's claimed RMS error is the
    // root mean square of its own standard deviation of tank 1 over the same updates. The correct filter's actual error
    // stays within its claim; the overconfident filter claims less than its actual error: the covariance is a belief.
    let records = [0.005, 0.00025, 0.1].map(|sd| lesson72_run(sd, &[1000; 122], &[]));
    for record in &records {
        // The edge update is record 1 at 0 s; the updates from 31 s are scored.
        let scored = &record[32..];
        let claimed = (scored
            .iter()
            .map(|u| u.filter_sd * u.filter_sd)
            .sum::<f64>()
            / scored.len() as f64)
            .sqrt();
        assert!(
            (record[record.len() - 1].claimed - claimed).abs() < 1e-12,
            "claimed RMS {} against {}",
            record[record.len() - 1].claimed,
            claimed
        );
    }
    let [right, small, large] = records.map(|record| record[record.len() - 1]);
    assert!(
        small.rms > right.rms && large.rms > right.rms,
        "RMS right, small, large: {} {} {}",
        right.rms,
        small.rms,
        large.rms
    );
    assert!(
        right.rms < right.claimed,
        "within its claim: {} against {}",
        right.rms,
        right.claimed
    );
    assert!(
        small.claimed < small.rms,
        "overconfident: claims {} against {}",
        small.claimed,
        small.rms
    );
    assert!(
        right.reading != 0.0 && (right.reading - small.reading).abs() < 1e-15,
        "same readings: {} {}",
        right.reading,
        small.reading
    );
}

#[test]
fn lesson72_every_experiment_reads_the_same_noise() {
    // The reading noise restarts when the experiment starts: an experiment started after 1 or after 7 updates at rest reads
    // the same noise and gives the same estimates, so runs with different assumed uncertainties compare the same readings.
    let early = lesson72_run_after(1, 0.005, &[1000; 12], &[]);
    let late = lesson72_run_after(7, 0.005, &[1000; 12], &[]);
    for (a, b) in early[1..].iter().zip(&late[7..]) {
        assert!(
            a.tank == b.tank && a.reading == b.reading && a.corrected == b.corrected,
            "same noise and estimates: {a:?} {b:?}"
        );
    }
}

#[test]
fn lesson72_the_filter_never_reads_tank_1() {
    // Tank 1's level is read only to score the estimate: wrong tank-1 readings at the filter's start and at update 5 change
    // no estimate.
    let plain = lesson72_run(0.005, &[1000; 8], &[]);
    let misread = lesson72_run(0.005, &[1000; 8], &[(2, "%IL0", 0.30), (5, "%IL0", 0.30)]);
    for (a, b) in plain.iter().zip(&misread) {
        assert!(
            a.predicted == b.predicted && a.corrected == b.corrected,
            "estimates with tank 1 misread: {a:?} {b:?}"
        );
    }
}

#[test]
fn lesson72_a_wrong_interval_or_level_latches_zero_flow_and_freezes_the_filter() {
    // A 2 s gap makes the one-second prediction stale: the pump latches at zero and the filter holds its estimate. A
    // reading outside 0.05–0.45 m does the same.
    let gap = lesson72_run(0.005, &[1000, 1000, 1000, 2000, 1000, 1000], &[]);
    assert!(
        !gap[3].fault && gap[4..].iter().all(|u| u.fault && u.pump == 0.0),
        "latched after the gap: {:?}",
        &gap[3..]
    );
    assert!(
        gap[4..].iter().all(|u| u.corrected == gap[3].corrected),
        "frozen after the gap: {:?}",
        &gap[3..]
    );
    let level = lesson72_run(0.005, &[1000; 6], &[(4, "%IL8", 0.46)]);
    assert!(
        level[4..]
            .iter()
            .all(|u| u.fault && u.pump == 0.0 && u.corrected == level[3].corrected),
        "latched and frozen: {:?}",
        &level[3..]
    );
}
