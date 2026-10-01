use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON73: &str = "11-state-space-estimation-and-predictive-control/69-lqr";

/// Control Studio's 20-step LQR designs of the cascade sampled at 1 s, Q = diag(400, 400) m⁻² with the same terminal
/// weight: [K1, K2, N] for R = 40000 and R = 160000 s²/m⁶.
const LQR_R40000: [f64; 3] = [0.07841259, 0.02451445, 0.12292704];
const LQR_R160000: [f64; 3] = [0.03442132, 0.00979336, 0.06421468];

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

/// One 1 s update of the cascade: both levels, the requested and applied flows, the level and effort cost sums, the late
/// peak deviation and the clipping and fault flags.
#[derive(Clone, Copy, Debug)]
struct Lqr {
    tank: [f64; 2],
    requested: f64,
    pump: f64,
    level_cost: f64,
    effort_cost: f64,
    late_peak: f64,
    clipped: bool,
    fault: bool,
}

/// Applies `gains` [K1, K2, N] and the feedback mode as Control Studio's Apply would and runs `settle` + 1 updates with tank
/// 2's setpoint at `setpoint`, as Apply's live check leaves it. Then Control Studio's project experiment (activation at
/// %MX40.0) runs one update per entry of `periods_ms`, each after that interval. Each `(update, address, level)` replaces
/// that reading for one update, counted from 0.
fn lesson73_run(
    gains: [f64; 3],
    feedback: bool,
    setpoint: f64,
    settle: usize,
    periods_ms: &[u64],
    readings: &[(usize, &str, f64)],
) -> Vec<Lqr> {
    let (mut runtime, mut simulation) = setup(LESSON73);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    for (name, value) in ["K1", "K2", "N"].into_iter().zip(gains) {
        assert!(runtime
            .storage_mut()
            .set_instance_var(p1, name, Value::LReal(value)));
    }
    // The mode is written by symbol: the runtime refuses an Auto transfer whose mode flag is bound to I/O.
    assert!(runtime
        .storage_mut()
        .set_instance_var(p1, "FeedbackEnabled", Value::Bool(feedback)));
    debug.enqueue_io_write(
        IoAddress::parse("%ML0").unwrap(),
        Value::LWord(setpoint.to_bits()),
    );
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::repeat_n(&1000, settle + 1)
        .chain(periods_ms.iter())
        .enumerate()
    {
        if step == settle + 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX40.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, level)| (*address, Value::LWord(level.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Lqr {
            tank: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            requested: number(&runtime, "%ML8"),
            pump: number(&runtime, "%QL0"),
            level_cost: number(&runtime, "%ML16"),
            effort_cost: number(&runtime, "%ML24"),
            late_peak: number(&runtime, "%ML32"),
            clipped: flag(&runtime, "%MX40.1"),
            fault: flag(&runtime, "%MX40.2"),
        });
    }
    record
}

#[test]
fn lesson73_lqr_feedback_returns_both_levels_after_the_slug() {
    // Apply leaves tank 2's setpoint at 0.26 m. The experiment returns it to 0.25 m and holds it for 60 s, and both levels
    // are back within 1 mm: the design's slower pole is near −0.054 s⁻¹, an 18.5 s time constant. At 60 s the pump
    // runs one second at 0.020 m³/s, a slug that puts tank 1 about 0.02 m high. Then the R = 40000 design requests
    // q = 0.010 − K1(h1 − 0.25) − K2(h2 − 0.25), about 0.0085 m³/s at first, and both levels are within 2 mm of rest from
    // 50 s to 60 s after the slug.
    let [k1, k2, _] = LQR_R40000;
    let record = lesson73_run(LQR_R40000, true, 0.26, 80, &[1000; 122], &[]);
    assert!(
        (record[80].tank[1] - 0.26).abs() < 0.0005,
        "settled at the 0.26 m setpoint: {:?}",
        record[80]
    );
    let run = &record[81..];
    assert!(
        run[59]
            .tank
            .iter()
            .all(|level| (level - 0.25).abs() < 0.001),
        "back at 0.25 m: {:?}",
        run[59]
    );
    assert!(run[60].pump == 0.020, "slug: {:?}", run[60]);
    for (i, u) in run.iter().enumerate().skip(61) {
        let row = 0.010 - k1 * (u.tank[0] - 0.25) - k2 * (u.tank[1] - 0.25);
        assert!(
            (u.requested - row).abs() < 1e-12 && u.pump == u.requested && !u.clipped,
            "row product at {i} s: {u:?}"
        );
    }
    assert!(
        (run[61].requested - 0.00845).abs() < 0.0001,
        "first correction: {}",
        run[61].requested
    );
    assert!(
        run[110..=120]
            .iter()
            .all(|u| u.tank.iter().all(|level| (level - 0.25).abs() <= 0.002)),
        "late deviations: {:?}",
        &run[110..=120]
    );
}

#[test]
fn lesson73_the_costs_are_scaled_sums_over_the_recovery() {
    // From the first update after the slug, 61 s, to 120 s the program adds 400(x1² + x2²) to the level cost and
    // 40000(q − 0.010)² of the applied flow to the effort cost, the design scales 0.05 m and 0.005 m³/s for both designs,
    // and records the largest level deviation from 110 s to 120 s, 50–60 s after the slug. The gentler R = 160000 design
    // spends less effort and leaves more level error, both meet the 2 mm late requirement, and each design has the smaller
    // cost by its own weights.
    let costs = [LQR_R40000, LQR_R160000].map(|gains| {
        let record = lesson73_run(gains, true, 0.26, 80, &[1000; 122], &[]);
        let run = &record[81..];
        let scored = &run[61..=120];
        let level: f64 = scored
            .iter()
            .map(|u| {
                400.0
                    * ((u.tank[0] - 0.25) * (u.tank[0] - 0.25)
                        + (u.tank[1] - 0.25) * (u.tank[1] - 0.25))
            })
            .sum();
        let effort: f64 = scored
            .iter()
            .map(|u| 40000.0 * (u.pump - 0.010) * (u.pump - 0.010))
            .sum();
        let late = run[110..=120]
            .iter()
            .flat_map(|u| u.tank)
            .map(|h| (h - 0.25).abs())
            .fold(0.0, f64::max);
        let end = run[run.len() - 1];
        assert!(
            (end.level_cost - level).abs() < 1e-12 && (end.effort_cost - effort).abs() < 1e-12,
            "costs {end:?} against {level} {effort}"
        );
        assert!(
            end.late_peak == late && late <= 0.002,
            "late peak {} against {}",
            end.late_peak,
            late
        );
        (level, effort)
    });
    let [(level_a, effort_a), (level_b, effort_b)] = costs;
    assert!(
        effort_b < effort_a && level_b > level_a,
        "gentler design: {costs:?}"
    );
    assert!(
        level_a + effort_a < level_b + effort_b,
        "cheaper by R = 40000: {costs:?}"
    );
    assert!(
        level_b + 4.0 * effort_b < level_a + 4.0 * effort_a,
        "cheaper by R = 160000: {costs:?}"
    );
}

#[test]
fn lesson73_a_clipped_flow_is_charged_as_applied() {
    // Gains ten times too strong request a negative flow after the slug: the pump delivers 0, Clipped reports it and the
    // effort cost charges the applied correction of 0.010 m³/s, 40000·0.010² = 4, not the request.
    let record = lesson73_run([0.8, 0.0, 0.82], true, 0.25, 0, &[1000; 63], &[]);
    let u = record[1 + 61];
    assert!(
        u.requested < 0.0 && u.pump == 0.0 && u.clipped,
        "clipped: {u:?}"
    );
    assert!(
        (u.effort_cost - 4.0).abs() < 1e-12,
        "charged as applied: {}",
        u.effort_cost
    );
}

#[test]
fn lesson73_in_manual_the_pump_follows_the_manual_flow() {
    // With FeedbackEnabled FALSE the pump delivers ManualFlowM3PerS, 0.010 m³/s, apart from the slug.
    let record = lesson73_run(LQR_R40000, false, 0.25, 0, &[1000; 64], &[]);
    let run = &record[1..];
    assert!(
        run[59].pump == 0.010 && run[60].pump == 0.020 && run[61..].iter().all(|u| u.pump == 0.010),
        "manual: {:?}",
        &run[59..]
    );
}

#[test]
fn lesson73_a_wrong_interval_or_level_latches_zero_flow() {
    // A 2 s gap between updates or a level outside 0.05–0.45 m latches zero flow.
    let gap = lesson73_run(LQR_R40000, true, 0.25, 0, &[1000, 1000, 2000, 1000], &[]);
    assert!(
        !gap[2].fault && gap[3..].iter().all(|u| u.fault && u.pump == 0.0),
        "latched after the gap: {gap:?}"
    );
    let level = lesson73_run(LQR_R40000, true, 0.25, 0, &[1000; 5], &[(3, "%IL8", 0.46)]);
    assert!(
        !level[2].fault && level[3..].iter().all(|u| u.fault && u.pump == 0.0),
        "latched on the level: {level:?}"
    );
}

const LESSON74: &str = "11-state-space-estimation-and-predictive-control/72-lqg";

/// Control Studio's LQG design of the one-second cascade model with only tank 2 measured: the 20-step LQR gain [K1, K2, N]
/// (Q = diag(400, 400), terminal weight Q, R = 40000) and the steady Kalman gains [L1, L2] as it shows them, for the
/// reading variance 2.5e-5 m² with the process covariance diag(1e-8, 1e-8), the model's error only, or diag(4e-6, 1e-8),
/// which lets tank 1 change by about 2 mm per update unexplained.
const LQG_GAINS: [f64; 3] = [0.07874026, 0.0238743, 0.12261456];
const KALMAN_MODEL_ONLY: [f64; 2] = [0.002255, 0.006778];
const KALMAN_UPSETS: [f64; 2] = [0.2393, 0.1056];

/// One 1 s update of both cascades, [full-state A, LQG B]: the levels [tank 1, tank 2], B's tank-2 reading, B's command
/// (what its estimator is told), B's estimated levels, the pump flows, the level and effort costs, the late peak
/// deviations, B's estimation RMS error and the fault flags.
#[derive(Clone, Copy, Debug)]
struct Lqg {
    tank: [[f64; 2]; 2],
    reading: f64,
    command: f64,
    estimate: [f64; 2],
    pump: [f64; 2],
    level_cost: [f64; 2],
    effort_cost: [f64; 2],
    late_peak: [f64; 2],
    estimation_rms: f64,
    fault: [bool; 2],
}

/// Applies [K1, K2, N], the estimator gains and the feedback mode as Control Studio's Apply would and runs `settle` + 1
/// updates with tank 2's setpoint at `setpoint`. Then the project experiment (activation at %MX96.0) runs one update per
/// entry of `periods_ms`. Each `(update, address, level)` replaces that reading for one update, counted from 0.
fn lesson74_run(
    kalman: [f64; 2],
    feedback: bool,
    setpoint: f64,
    settle: usize,
    periods_ms: &[u64],
    readings: &[(usize, &str, f64)],
) -> Vec<Lqg> {
    let (mut runtime, mut simulation) = setup(LESSON74);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    for (name, value) in ["K1", "K2", "N", "L1", "L2"]
        .into_iter()
        .zip(LQG_GAINS.into_iter().chain(kalman))
    {
        assert!(runtime
            .storage_mut()
            .set_instance_var(p1, name, Value::LReal(value)));
    }
    assert!(runtime
        .storage_mut()
        .set_instance_var(p1, "FeedbackEnabled", Value::Bool(feedback)));
    debug.enqueue_io_write(
        IoAddress::parse("%ML0").unwrap(),
        Value::LWord(setpoint.to_bits()),
    );
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::repeat_n(&1000, settle + 1)
        .chain(periods_ms.iter())
        .enumerate()
    {
        if step == settle + 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX96.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, level)| (*address, Value::LWord(level.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Lqg {
            tank: [
                [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
                [number(&runtime, "%IL16"), number(&runtime, "%IL24")],
            ],
            reading: number(&runtime, "%ML8"),
            command: number(&runtime, "%ML16"),
            estimate: [number(&runtime, "%ML24"), number(&runtime, "%ML32")],
            pump: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            level_cost: [number(&runtime, "%ML40"), number(&runtime, "%ML56")],
            effort_cost: [number(&runtime, "%ML48"), number(&runtime, "%ML64")],
            late_peak: [number(&runtime, "%ML72"), number(&runtime, "%ML80")],
            estimation_rms: number(&runtime, "%ML88"),
            fault: [flag(&runtime, "%MX96.3"), flag(&runtime, "%MX96.4")],
        });
    }
    record
}

#[test]
fn lesson74_the_estimate_predicts_from_the_previous_command_and_corrects_with_the_reading() {
    // Every update B's estimator first predicts with the one-second model from the old estimate and the command its
    // controller gave last update, then corrects with the new reading, and only then is the command calculated from the
    // corrected estimate. At the slug the pump delivers 0.020 m³/s but the estimator is told the controller's command:
    // the slug is water the model does not know about.
    let [l1, l2] = KALMAN_UPSETS;
    let [k1, k2, _] = LQG_GAINS;
    let record = lesson74_run(KALMAN_UPSETS, true, 0.26, 20, &[1000; 70], &[]);
    let run = &record[21..];
    for i in 1..run.len() {
        let (old, u) = (&run[i - 1], &run[i]);
        let x = [old.estimate[0] - 0.25, old.estimate[1] - 0.25];
        let predicted = [
            0.96 * x[0] + 2.0 * (old.command - 0.010),
            0.04 * x[0] + 0.96 * x[1],
        ];
        let innovation = (u.reading - 0.25) - predicted[1];
        let corrected = [
            predicted[0] + l1 * innovation,
            predicted[1] + l2 * innovation,
        ];
        assert!(
            (u.estimate[0] - 0.25 - corrected[0]).abs() < 1e-12
                && (u.estimate[1] - 0.25 - corrected[1]).abs() < 1e-12,
            "estimate at {i} s: {u:?}"
        );
        let command = (0.010 - k1 * corrected[0] - k2 * corrected[1]).clamp(0.0, 0.020);
        assert!(
            (u.command - command).abs() < 1e-12,
            "command at {i} s: {u:?}"
        );
        assert!(
            u.pump[1] == if i == 60 { 0.020 } else { u.command },
            "pump at {i} s: {u:?}"
        );
    }
    assert!(
        run[60].command < 0.0105,
        "the estimator is told the command, not the slug: {:?}",
        run[60]
    );
}

#[test]
fn lesson74_at_rest_the_estimate_stays_at_the_operating_point_from_the_first_update() {
    // The estimator's previous command starts at the operating flow: at rest, in Manual or in Auto, both estimated
    // deviations are zero in every update, the first included.
    for feedback in [false, true] {
        let record = lesson74_run(KALMAN_MODEL_ONLY, feedback, 0.25, 5, &[], &[]);
        assert!(
            record
                .iter()
                .all(|u| u.estimate == [0.25, 0.25] && u.command == 0.010),
            "at rest: {record:?}"
        );
    }
}

#[test]
fn lesson74_the_full_state_reference_reads_tank_2_with_the_same_noise() {
    // The experiment adds the reading noise; before it both cascades read tank 2 exactly. During it cascade A, which
    // measures both levels, reads tank 2 with the same noise sample as B, so the two loops differ only in how they know
    // tank 1.
    let [k1, k2, _] = LQG_GAINS;
    let before = lesson74_run(KALMAN_UPSETS, true, 0.25, 5, &[], &[]);
    assert!(
        before.iter().all(|u| u.reading == u.tank[1][1]),
        "exact before the experiment: {before:?}"
    );
    let record = lesson74_run(KALMAN_UPSETS, true, 0.25, 0, &[1000; 70], &[]);
    for (i, u) in record.iter().enumerate().skip(2) {
        if i == 61 {
            continue;
        }
        let noise = u.reading - u.tank[1][1];
        let command = (0.010 - k1 * (u.tank[0][0] - 0.25) - k2 * (u.tank[0][1] + noise - 0.25))
            .clamp(0.0, 0.020);
        assert!(
            noise != 0.0 && (u.pump[0] - command).abs() < 1e-12,
            "full-state command at {i} s: {u:?}"
        );
    }
}

#[test]
fn lesson74_lqg_sees_the_slug_only_if_its_process_covariance_allows_upsets() {
    // After the same slug, the full-state loop keeps both levels within 2 mm 50–60 s later. B's estimator with the model's
    // error alone hardly moves, so B's pump barely reacts and its levels are still more than 2 mm high then; with a process
    // covariance that lets tank 1 change unexplained, B reacts and meets the 2 mm requirement, but with more level
    // deviation than the full-state loop.
    let blind = lesson74_run(KALMAN_MODEL_ONLY, true, 0.26, 80, &[1000; 122], &[]);
    let upsets = lesson74_run(KALMAN_UPSETS, true, 0.26, 80, &[1000; 122], &[]);
    let [blind_end, upsets_end] = [blind[blind.len() - 1], upsets[upsets.len() - 1]];
    for end in [blind_end, upsets_end] {
        assert!(end.late_peak[0] <= 0.002, "full-state late peak: {end:?}");
    }
    assert!(
        blind_end.late_peak[1] > 0.002 && blind_end.effort_cost[1] < 0.01,
        "model error only: {blind_end:?}"
    );
    assert!(
        upsets_end.late_peak[1] <= 0.002 && upsets_end.level_cost[1] > upsets_end.level_cost[0],
        "upsets allowed: {upsets_end:?}"
    );
    assert!(
        upsets_end.estimation_rms < blind_end.estimation_rms,
        "estimation RMS: {} {}",
        upsets_end.estimation_rms,
        blind_end.estimation_rms
    );
}

#[test]
fn lesson74_the_costs_and_the_estimation_error_are_scored_over_the_recovery() {
    // From 61 s to 120 s each cascade adds 400(x1² + x2²) and 40000(q − 0.010)² of its applied flow, as in Lesson 80, and
    // records its largest level deviation from 110 s; B also scores its tank-1 estimate against the true level.
    let record = lesson74_run(KALMAN_UPSETS, true, 0.26, 80, &[1000; 122], &[]);
    let run = &record[81..];
    let scored = &run[61..=120];
    let end = run[run.len() - 1];
    for c in 0..2 {
        let level: f64 = scored
            .iter()
            .map(|u| 400.0 * ((u.tank[c][0] - 0.25).powi(2) + (u.tank[c][1] - 0.25).powi(2)))
            .sum();
        let effort: f64 = scored
            .iter()
            .map(|u| 40000.0 * (u.pump[c] - 0.010).powi(2))
            .sum();
        let late = run[110..=120]
            .iter()
            .flat_map(|u| u.tank[c])
            .map(|h| (h - 0.25).abs())
            .fold(0.0, f64::max);
        assert!(
            (end.level_cost[c] - level).abs() < 1e-12
                && (end.effort_cost[c] - effort).abs() < 1e-12
                && end.late_peak[c] == late,
            "cascade {c}: {end:?}"
        );
    }
    let squares: f64 = scored
        .iter()
        .map(|u| (u.estimate[0] - u.tank[1][0]).powi(2))
        .sum();
    assert!(
        (end.estimation_rms - (squares / scored.len() as f64).sqrt()).abs() < 1e-12,
        "estimation RMS: {end:?}"
    );
}

#[test]
fn lesson74_the_estimator_never_reads_tank_1() {
    // B's tank 1 is read only to score the estimate: wrong readings of it change no estimate, command or pump flow.
    let plain = lesson74_run(KALMAN_UPSETS, true, 0.25, 0, &[1000; 8], &[]);
    let misread = lesson74_run(
        KALMAN_UPSETS,
        true,
        0.25,
        0,
        &[1000; 8],
        &[(2, "%IL16", 0.30), (5, "%IL16", 0.30)],
    );
    for (a, b) in plain.iter().zip(&misread) {
        assert!(
            a.estimate == b.estimate && a.command == b.command && a.pump[1] == b.pump[1],
            "B with tank 1 misread: {a:?} {b:?}"
        );
    }
}

#[test]
fn lesson74_each_cascade_latches_on_its_own_measurements() {
    // A latches on its levels, B on its tank-2 reading; a wrong update interval latches both. A latched cascade holds zero
    // flow and B's estimate freezes.
    let a = lesson74_run(
        KALMAN_UPSETS,
        true,
        0.25,
        0,
        &[1000; 5],
        &[(3, "%IL0", 0.46)],
    );
    assert!(
        !a[2].fault[0]
            && a[3..]
                .iter()
                .all(|u| u.fault[0] && u.pump[0] == 0.0 && !u.fault[1]),
        "A alone: {a:?}"
    );
    let b = lesson74_run(
        KALMAN_UPSETS,
        true,
        0.25,
        0,
        &[1000; 5],
        &[(3, "%IL24", 0.46)],
    );
    assert!(
        !b[2].fault[1]
            && b[3..].iter().all(|u| u.fault[1]
                && u.pump[1] == 0.0
                && !u.fault[0]
                && u.estimate == b[3].estimate),
        "B alone: {b:?}"
    );
    let truth = lesson74_run(
        KALMAN_UPSETS,
        true,
        0.25,
        0,
        &[1000; 5],
        &[(3, "%IL16", 0.46)],
    );
    assert!(
        truth.iter().all(|u| !u.fault[1]),
        "B's tank 1 is not a measurement: {truth:?}"
    );
    let gap = lesson74_run(KALMAN_UPSETS, true, 0.25, 0, &[1000, 1000, 2000, 1000], &[]);
    assert!(
        !gap[2].fault[0]
            && gap[3..]
                .iter()
                .all(|u| u.fault == [true, true] && u.pump == [0.0, 0.0]),
        "both after the gap: {gap:?}"
    );
}

#[test]
fn lesson74_in_manual_both_pumps_follow_the_manual_flow_and_the_estimate_follows_it() {
    // With FeedbackEnabled FALSE both pumps deliver ManualFlowM3PerS, apart from the slug, and B's estimator is told that
    // flow as its command.
    let record = lesson74_run(KALMAN_UPSETS, false, 0.25, 0, &[1000; 64], &[]);
    let run = &record[1..];
    assert!(
        run[60].pump == [0.020, 0.020]
            && run[61..]
                .iter()
                .all(|u| u.pump == [0.010, 0.010] && u.command == 0.010),
        "manual: {:?}",
        &run[59..]
    );
}

const LESSON75: &str = "11-state-space-estimation-and-predictive-control/73-mpc";

/// One 1 s update of the two heaters: temperatures, commands, setpoints, heater 2's extra load, the supply used, the
/// selected plan's predicted temperatures after three seconds and its cost, the number of plans rejected by a limit, and
/// the planning-failure and fault flags.
#[derive(Clone, Copy, Debug)]
struct Mpc {
    temperature: [f64; 2],
    command: [f64; 2],
    setpoint: [f64; 2],
    load: f64,
    supply: f64,
    predicted: [f64; 2],
    cost: f64,
    rejected: f64,
    planning_failed: bool,
    fault: bool,
}

/// One heater's part of the lesson's plan for command `u`: one second with the previous command, which is still being
/// delivered, then two with `u`. `None` when `u` leaves 0–100 points or a predicted temperature leaves 26–33 °C; otherwise
/// the cost Σ(T − r)² + 0.002·(u − previous)² and the final temperature, in the program's order of operations.
fn lesson75_heater(
    temperature: f64,
    previous: f64,
    u: f64,
    setpoint: f64,
    load: f64,
) -> Option<(f64, f64)> {
    let first = temperature + (2.0 * previous - 10.0 * (temperature - 20.0) - load) / 50.0;
    let second = first + (2.0 * u - 10.0 * (first - 20.0) - load) / 50.0;
    let end = second + (2.0 * u - 10.0 * (second - 20.0) - load) / 50.0;
    if !(0.0..=100.0).contains(&u)
        || [first, second, end]
            .iter()
            .any(|t| !(26.0..=33.0).contains(t))
    {
        return None;
    }
    let cost = (first - setpoint) * (first - setpoint)
        + (second - setpoint) * (second - setpoint)
        + (end - setpoint) * (end - setpoint)
        + 0.002 * (u - previous) * (u - previous);
    Some((cost, end))
}

/// A pair of commands on the grid: its cost, the two commands and the two heaters' final temperatures.
type Plan = (f64, [f64; 2], [f64; 2]);

/// The cheapest of the 231 pairs on the 5-point grid with u1 + u2 ≤ 100, searched in the program's order: a pair's cost is
/// the sum of the two heaters' costs and it is rejected when either heater's part is. Also the number rejected.
fn lesson75_best(
    temperature: [f64; 2],
    previous: [f64; 2],
    setpoint: [f64; 2],
    load: [f64; 2],
) -> (Option<Plan>, usize) {
    let part = |h: usize, i: usize| {
        lesson75_heater(
            temperature[h],
            previous[h],
            5.0 * i as f64,
            setpoint[h],
            load[h],
        )
    };
    let (mut best, mut rejected) = (None::<Plan>, 0);
    for i in 0..=20 {
        for j in 0..=(20 - i) {
            match (part(0, i), part(1, j)) {
                (Some((c1, e1)), Some((c2, e2))) => {
                    if best.is_none_or(|(lowest, _, _)| c1 + c2 < lowest) {
                        best = Some((c1 + c2, [5.0 * i as f64, 5.0 * j as f64], [e1, e2]))
                    }
                }
                _ => rejected += 1,
            }
        }
    }
    (best, rejected)
}

/// Runs `settle` + 1 updates at rest, then the project experiment (activation at %MX56.0), one update per entry of
/// `periods_ms`. Each `(update, address, value)` replaces that input for one update, counted from 0.
fn lesson75_run(settle: usize, periods_ms: &[u64], readings: &[(usize, &str, f64)]) -> Vec<Mpc> {
    let (mut runtime, mut simulation) = setup(LESSON75);
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    let mut now = 0;
    for (step, period) in std::iter::repeat_n(&1000, settle + 1)
        .chain(periods_ms.iter())
        .enumerate()
    {
        if step == settle + 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX56.0").unwrap(), Value::Bool(true));
        }
        let writes: Vec<(&str, Value)> = readings
            .iter()
            .filter(|(at, _, _)| *at == step)
            .map(|(_, address, value)| (*address, Value::LWord(value.to_bits())))
            .collect();
        scan_every(&mut runtime, &mut simulation, &debug, now, *period, &writes);
        now += period;
        record.push(Mpc {
            temperature: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            command: [number(&runtime, "%QL0"), number(&runtime, "%QL16")],
            setpoint: [number(&runtime, "%ML0"), number(&runtime, "%ML8")],
            load: number(&runtime, "%QL24"),
            supply: number(&runtime, "%ML16"),
            predicted: [number(&runtime, "%ML24"), number(&runtime, "%ML32")],
            cost: number(&runtime, "%ML40"),
            rejected: number(&runtime, "%ML48"),
            planning_failed: flag(&runtime, "%MX56.1"),
            fault: flag(&runtime, "%MX56.2"),
        });
    }
    record
}

#[test]
fn lesson75_each_update_applies_the_first_move_of_the_cheapest_feasible_plan() {
    // Every update the program searches the 231 permitted pairs, predicts three seconds from the measured temperatures,
    // the command still being delivered and each pair, rejects plans that leave 26–33 °C, and applies the cheapest
    // plan's pair; the next update plans again from new measurements. The setpoints step to 31 °C and 28 °C at the start,
    // and heater 2 loses 30 kW more from 60 s.
    let record = lesson75_run(2, &[1000; 124], &[]);
    let run = &record[3..];
    for i in 1..run.len() {
        let (old, u) = (&run[i - 1], &run[i]);
        let (best, rejected) = lesson75_best(u.temperature, old.command, u.setpoint, [0.0, u.load]);
        let (cost, pair, end) = best.expect("a feasible plan");
        assert!(
            u.command == pair && (u.cost - cost).abs() < 1e-9 && u.rejected == rejected as f64,
            "plan at {i} s: {u:?} against {pair:?} {cost} {rejected}"
        );
        assert!(
            (u.predicted[0] - end[0]).abs() < 1e-9
                && (u.predicted[1] - end[1]).abs() < 1e-9
                && !u.planning_failed,
            "prediction at {i} s: {u:?}"
        );
    }
    assert!(
        run[0].setpoint == [31.0, 28.0] && run[59].load == 0.0 && run[60].load == 30.0,
        "schedule: {:?} {:?}",
        run[0],
        run[60]
    );
}

#[test]
fn lesson75_the_supply_holds_and_both_targets_are_met_before_the_load() {
    // The applied pair never exceeds the 100-point supply. From 30 s to 59 s both temperatures are within 0.2 °C of 31 °C
    // and 28 °C, with 55 and 40 points: 95 of the 100.
    let record = lesson75_run(2, &[1000; 124], &[]);
    let run = &record[3..];
    assert!(
        record.iter().all(
            |u| u.command[0] + u.command[1] <= 100.0 && u.supply == u.command[0] + u.command[1]
        ),
        "supply: {record:?}"
    );
    assert!(
        run[30..60]
            .iter()
            .all(|u| (u.temperature[0] - 31.0).abs() <= 0.2
                && (u.temperature[1] - 28.0).abs() <= 0.2),
        "targets: {:?}",
        &run[30..60]
    );
    assert!(
        run[59].command == [55.0, 40.0],
        "steady commands: {:?}",
        run[59]
    );
}

#[test]
fn lesson75_with_the_extra_load_both_heaters_share_the_shortfall() {
    // With 30 kW more taken from heater 2 the targets need 55 + 55 = 110 points. The plans share the whole supply, 50 and
    // 50 points, and from 100 s both heaters are about 1 °C below their setpoints.
    let record = lesson75_run(2, &[1000; 124], &[]);
    let run = &record[3..];
    assert!(
        run[100..=120].iter().all(|u| u.command == [50.0, 50.0]
            && (u.temperature[0] - 30.0).abs() <= 0.1
            && (u.temperature[1] - 27.0).abs() <= 0.1),
        "shared shortfall: {:?}",
        &run[100..=120]
    );
}

#[test]
fn lesson75_without_a_feasible_plan_the_commands_hold() {
    // A reading of 22 °C on heater 1 puts every plan's first second below 26 °C, whatever command is being delivered:
    // planning fails, the program reports it and holds the commands; the next update plans again.
    let record = lesson75_run(2, &[1000; 8], &[(6, "%IL0", 22.0)]);
    assert!(
        record[6].planning_failed && record[6].command == record[5].command,
        "held: {:?}",
        &record[5..7]
    );
    assert!(
        !record[7].planning_failed,
        "planning again: {:?}",
        record[7]
    );
}

#[test]
fn lesson75_an_invalid_temperature_or_interval_latches_zero_output() {
    // A temperature outside 0–100 °C or an update interval outside 1 s ± 50 ms latches both commands at zero.
    let reading = lesson75_run(2, &[1000; 5], &[(4, "%IL8", 150.0)]);
    assert!(
        !reading[3].fault
            && reading[4..]
                .iter()
                .all(|u| u.fault && u.command == [0.0, 0.0]),
        "invalid temperature: {reading:?}"
    );
    let gap = lesson75_run(0, &[1000, 1000, 2000, 1000], &[]);
    assert!(
        !gap[2].fault && gap[3..].iter().all(|u| u.fault && u.command == [0.0, 0.0]),
        "interval: {gap:?}"
    );
}
