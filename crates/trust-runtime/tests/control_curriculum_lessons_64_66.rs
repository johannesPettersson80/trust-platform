use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;

mod control_heater_support;
use control_heater_support::*;

const LESSON64: &str = "10-multivariable-control/63-decentralized-pi";

/// One 0.1 s update of the coupled vessels: both temperatures and both heater commands.
#[derive(Clone, Copy, Debug)]
struct Paired {
    vessel: [f64; 2],
    heater: [f64; 2],
}

/// What starts together with Auto: a 35 → 37 °C setpoint for one vessel, or 10 kW taken from one vessel.
#[derive(Clone, Copy, Debug)]
enum Event {
    Setpoint(usize),
    Load(usize),
}

/// One update at rest with both PIs in Manual; then, as Control Studio's Apply writes them, the gains (Kp, Ki) of both
/// members and Auto, together with `event`, for `updates`.
/// Control Studio's PID-group setup binds these program variables; `set_instance_var` would silently add a missing one.
fn lesson64_assert_interface(runtime: &trust_runtime::Runtime) {
    let p1 = program(runtime);
    for name in [
        "VesselsAuto",
        "Vessel1Parameters",
        "Vessel2Parameters",
        "Vessel1ManualPct",
        "Vessel2ManualPct",
        "Load1Kw",
        "Load2Kw",
    ] {
        assert!(
            runtime.storage().get_instance_var(p1, name).is_some(),
            "Control Studio's PID-group setup binds P1.{name}"
        );
    }
}

fn lesson64_run(gains: (f64, f64), event: Event, updates: u64) -> Vec<Paired> {
    let (mut runtime, mut simulation) = setup(LESSON64);
    lesson64_assert_interface(&runtime);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    let mut record = Vec::new();
    for step in 0..1 + updates {
        let mut writes = Vec::new();
        if step == 1 {
            assert!(runtime
                .write_instance(p1, "VesselsAuto", Value::Bool(true))
                .is_ok());
            for parameters in ["Vessel1Parameters", "Vessel2Parameters"] {
                set_program_record(
                    &mut runtime,
                    p1,
                    parameters,
                    &[("ProportionalGain", gains.0), ("IntegralGain", gains.1)],
                );
            }
            match event {
                Event::Setpoint(k) => {
                    writes.push((["%ML0", "%ML8"][k], Value::LWord(37.0_f64.to_bits())))
                }
                Event::Load(k) => assert!(
                    runtime
                        .write_instance(p1, ["Load1Kw", "Load2Kw"][k], Value::LReal(10.0))
                        .is_ok(),
                    "load input {k}"
                ),
            }
        }
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            step * 100,
            100,
            &writes,
        );
        record.push(Paired {
            vessel: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            heater: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
        });
    }
    record
}

/// (own vessel peak, minimum, |error| from 30 s, largest deviation of the other vessel from 35 °C).
fn lesson64_metrics(record: &[Paired], event: Event) -> (f64, f64, f64, f64) {
    let run = &record[1..];
    let (own, target) = match event {
        Event::Setpoint(k) => (k, 37.0),
        Event::Load(k) => (k, 35.0),
    };
    let series = run.iter().map(|u| u.vessel[own]);
    let late = run[300..]
        .iter()
        .map(|u| (u.vessel[own] - target).abs())
        .fold(0.0, f64::max);
    let other = run
        .iter()
        .map(|u| (u.vessel[1 - own] - 35.0).abs())
        .fold(0.0, f64::max);
    (
        series.clone().fold(f64::MIN, f64::max),
        series.fold(f64::MAX, f64::min),
        late,
        other,
    )
}

#[test]
fn lesson64_both_pis_start_in_manual_at_the_resting_point() {
    // Before Control Studio applies a design, both PIs are in Manual: a vessel-1 setpoint change alone leaves both heaters
    // at 75 %. With one switch both go to Auto (Kp 5, Ki 1); Manual tracked the new setpoint, so heater 1's first update
    // adds only the integral part, 1·2·0.1 = 0.2 points, and heater 2 stays at 75 %.
    let (mut runtime, mut simulation) = setup(LESSON64);
    lesson64_assert_interface(&runtime);
    let debug = runtime.enable_debug();
    for step in 0..50 {
        let writes = if step == 1 {
            vec![("%ML0", Value::LWord(37.0_f64.to_bits()))]
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
        let heaters = [number(&runtime, "%QL0"), number(&runtime, "%QL8")];
        assert!(
            heaters.iter().all(|h| (h - 75.0).abs() < 1e-12),
            "Manual at update {step}: {heaters:?}"
        );
    }
    let p1 = program(&runtime);
    assert!(runtime
        .write_instance(p1, "VesselsAuto", Value::Bool(true))
        .is_ok());
    scan_every(&mut runtime, &mut simulation, &debug, 5000, 100, &[]);
    let heaters = [number(&runtime, "%QL0"), number(&runtime, "%QL8")];
    assert!(
        (heaters[0] - 75.2).abs() < 1e-9 && heaters[1] == 75.0,
        "first Auto update: {heaters:?}"
    );
}

#[test]
fn lesson64_each_vessel_has_its_own_controller_and_state() {
    // First Auto update with Kp 5, Ki 1: a vessel-1 setpoint step gives P = 5·2 = 10 and I = 75 + 1·2·0.1 = 75.2, so heater 1
    // requests 85.2 % while heater 2, with zero error, stays at 75 %. Mirrored for vessel 2; a load has not yet moved a
    // temperature, so both heaters still command 75 %.
    for (k, event) in [Event::Setpoint(0), Event::Setpoint(1)]
        .into_iter()
        .enumerate()
    {
        let first = lesson64_run((5.0, 1.0), event, 1)[1];
        assert!(
            (first.heater[k] - 85.2).abs() < 1e-9 && first.heater[1 - k] == 75.0,
            "{event:?}: {first:?}"
        );
    }
    for event in [Event::Load(0), Event::Load(1)] {
        let first = lesson64_run((5.0, 1.0), event, 1)[1];
        assert!(first.heater == [75.0, 75.0], "{event:?}: {first:?}");
    }
}

#[test]
fn lesson64_the_paired_loops_meet_every_own_vessel_requirement_but_not_the_other_vessel_limit() {
    // Kp 5, Ki 1 per loop. Setpoint 35 → 37 °C: peak ≤ 37.5 °C and |error| ≤ 0.2 °C from 30 s, in both directions, but the
    // unchanged vessel strays more than 0.2 °C. 10 kW load: loaded vessel ≥ 34 °C, |error| ≤ 0.2 °C from 30 s, other vessel
    // within 0.2 °C. The heaters head for the balance: 93/67 % after a setpoint step, 80/75 % under a load.
    for event in [Event::Setpoint(0), Event::Setpoint(1)] {
        let record = lesson64_run((5.0, 1.0), event, 600);
        let (peak, _, late, other) = lesson64_metrics(&record, event);
        assert!(
            peak <= 37.5 && late <= 0.2,
            "{event:?}: peak {peak} late {late}"
        );
        assert!(
            other > 0.2 && other < 0.35,
            "{event:?}: unchanged vessel {other}"
        );
        assert!(
            record
                .iter()
                .all(|u| u.heater.iter().all(|h| (0.0..=100.0).contains(h))),
            "heaters within 0–100 %"
        );
        let last = record.last().unwrap();
        let Event::Setpoint(k) = event else {
            unreachable!()
        };
        assert!(
            (last.heater[k] - 93.0).abs() < 0.5 && (last.heater[1 - k] - 67.0).abs() < 0.5,
            "setpoint balance {:?}",
            last.heater
        );
    }
    for event in [Event::Load(0), Event::Load(1)] {
        let record = lesson64_run((5.0, 1.0), event, 600);
        let (_, low, late, other) = lesson64_metrics(&record, event);
        assert!(
            low >= 34.0 && late <= 0.2 && other <= 0.2,
            "{event:?}: {low} {late} {other}"
        );
        let last = record.last().unwrap();
        let Event::Load(k) = event else {
            unreachable!()
        };
        assert!(
            (last.heater[k] - 80.0).abs() < 0.2 && (last.heater[1 - k] - 75.0).abs() < 0.2,
            "load balance {:?}",
            last.heater
        );
    }
}

const LESSON65: &str = "10-multivariable-control/64-decoupling";

/// Control Studio's decoupled PID group binds these program variables; `set_instance_var` would silently add a missing one.
fn lesson65_assert_interface(runtime: &trust_runtime::Runtime) {
    let p1 = program(runtime);
    for name in [
        "VesselsAuto",
        "Vessel1Parameters",
        "Vessel2Parameters",
        "Vessel1ManualPct",
        "Vessel2ManualPct",
        "Request1Pct",
        "Request2Pct",
        "D11",
        "D12",
        "D21",
        "D22",
    ] {
        assert!(
            runtime.storage().get_instance_var(p1, name).is_some(),
            "Control Studio's decoupled PID group binds P1.{name}"
        );
    }
}

/// Lesson 69's gain matrix through Control Studio's decoupler D = K⁻¹·diag(K) = (81/65)·[[1, −4/9], [−4/9, 1]].
const LESSON65_STUDIO_D: [[f64; 2]; 2] = [[81.0 / 65.0, -36.0 / 65.0], [-36.0 / 65.0, 81.0 / 65.0]];

/// As Control Studio's Apply does: the decoupler, both members' gains and the controllers' starting requests v = D⁻¹·u for
/// the present heater commands (Specification 35 §17.15), then Auto.
fn lesson65_apply(runtime: &mut trust_runtime::Runtime, d: [[f64; 2]; 2], gains: (f64, f64)) {
    let p1 = program(runtime);
    for (name, value) in [
        ("D11", d[0][0]),
        ("D12", d[0][1]),
        ("D21", d[1][0]),
        ("D22", d[1][1]),
    ] {
        assert!(runtime
            .write_instance(p1, name, Value::LReal(value))
            .is_ok());
    }
    for parameters in ["Vessel1Parameters", "Vessel2Parameters"] {
        set_program_record(
            runtime,
            p1,
            parameters,
            &[("ProportionalGain", gains.0), ("IntegralGain", gains.1)],
        );
    }
    let heaters = [number(runtime, "%QL0"), number(runtime, "%QL8")];
    let determinant = d[0][0] * d[1][1] - d[0][1] * d[1][0];
    let requests = [
        (d[1][1] * heaters[0] - d[0][1] * heaters[1]) / determinant,
        (-d[1][0] * heaters[0] + d[0][0] * heaters[1]) / determinant,
    ];
    for (member, request) in ["Vessel1", "Vessel2"].into_iter().zip(requests) {
        let Some(Value::Instance(pid)) = runtime.storage().get_instance_var(p1, member).cloned()
        else {
            panic!("{member} is a Control.PID instance")
        };
        let Some(Value::Struct(state)) = runtime
            .storage()
            .get_instance_var(pid, "StateValue")
            .cloned()
        else {
            panic!("{member} keeps its state record")
        };
        let mut state = (*state).clone();
        for field in ["NominalOutput", "SaturatedOutput"] {
            assert!(
                state.set_existing_field(field.into(), Value::LReal(request)),
                "{member}.{field}"
            );
        }
        assert!(runtime
            .write_instance(pid, "StateValue", Value::Struct(std::sync::Arc::new(state)))
            .is_ok());
    }
    assert!(runtime
        .write_instance(p1, "VesselsAuto", Value::Bool(true))
        .is_ok());
}

/// One 0.1 s update of the coupled vessels: both temperatures, both heater commands and both controller requests.
#[derive(Clone, Copy, Debug)]
struct Decoupled {
    vessel: [f64; 2],
    heater: [f64; 2],
    request: [f64; 2],
}

/// One update at rest in Manual; then `lesson65_apply` with `d` and `gains`, and vessel 1's setpoint 30 → 32 °C if `step`.
fn lesson65_run(d: [[f64; 2]; 2], gains: (f64, f64), step: bool, updates: u64) -> Vec<Decoupled> {
    let (mut runtime, mut simulation) = setup(LESSON65);
    lesson65_assert_interface(&runtime);
    let debug = runtime.enable_debug();
    let p1 = program(&runtime);
    let mut record = Vec::new();
    for count in 0..1 + updates {
        let mut writes = Vec::new();
        if count == 1 {
            lesson65_apply(&mut runtime, d, gains);
            if step {
                writes.push(("%ML0", Value::LWord(32.0_f64.to_bits())));
            }
        }
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            count * 100,
            100,
            &writes,
        );
        record.push(Decoupled {
            vessel: [number(&runtime, "%IL0"), number(&runtime, "%IL8")],
            heater: [number(&runtime, "%QL0"), number(&runtime, "%QL8")],
            request: [
                program_number(&runtime, p1, "Request1Pct"),
                program_number(&runtime, p1, "Request2Pct"),
            ],
        });
    }
    record
}

#[test]
fn lesson65_both_pis_start_in_manual_at_the_lower_holding_point() {
    // Lesson 72's vessels rest at 30 °C, where the heaters hold at 50 %, so that Control Studio's decoupled controllers
    // can start at v = D⁻¹·u inside 0–100 %. In Manual a setpoint change alone leaves both heaters at 50 %.
    let (mut runtime, mut simulation) = setup(LESSON65);
    lesson65_assert_interface(&runtime);
    let debug = runtime.enable_debug();
    for count in 0..50 {
        let writes = if count == 1 {
            vec![("%ML0", Value::LWord(32.0_f64.to_bits()))]
        } else {
            Vec::new()
        };
        scan_every(
            &mut runtime,
            &mut simulation,
            &debug,
            count * 100,
            100,
            &writes,
        );
        let heaters = [number(&runtime, "%QL0"), number(&runtime, "%QL8")];
        assert!(
            heaters.iter().all(|h| (h - 50.0).abs() < 1e-12),
            "Manual at update {count}: {heaters:?}"
        );
    }
    let vessels = [number(&runtime, "%IL0"), number(&runtime, "%IL8")];
    assert!(
        vessels.iter().all(|t| (t - 30.0).abs() < 1e-9),
        "rest: {vessels:?}"
    );
}

#[test]
fn lesson65_studio_s_decoupler_starts_from_the_present_heaters_without_a_bump() {
    // At 50 % the decoupled controllers start at v = D⁻¹·u = 50·(1 + 4/9) = 72.22 %, inside 0–100 % (at 75 % it would be
    // 108.3 %). Through D they give back exactly 50 %: switching to Auto does not move a heater.
    let record = lesson65_run(LESSON65_STUDIO_D, (6.75, 1.73), false, 20);
    for update in &record[1..] {
        assert!(
            update.heater.iter().all(|h| (h - 50.0).abs() < 1e-9)
                && update
                    .request
                    .iter()
                    .all(|v| (v - 650.0 / 9.0).abs() < 1e-6),
            "bumpless start: {update:?}"
        );
    }
}

#[test]
fn lesson65_the_decoupler_turns_heater_2_down_in_the_same_update() {
    // Vessel 1's setpoint 30 → 32 °C with Kp 6.75 %/°C and Ki 1.73 %/(°C·s): request 1 rises by 6.75·2 + 1.73·2·0.1 = 13.846
    // points, and D lowers heater 2 at once by (36/65)·13.846 = 7.67 points, to 42.33 %. With the identity map, as
    // paired PI, heater 2 still holds 50 %. In steady state the decoupled heaters hold the balance's 68 % and 42 %.
    let decoupled = lesson65_run(LESSON65_STUDIO_D, (6.75, 1.73), true, 600);
    let first = decoupled[1];
    assert!(
        (first.request[0] - (650.0 / 9.0 + 13.846)).abs() < 1e-9
            && (first.request[1] - 650.0 / 9.0).abs() < 1e-9,
        "requests: {first:?}"
    );
    assert!(
        (first.heater[1] - (50.0 - 36.0 / 65.0 * 13.846)).abs() < 1e-9,
        "heater 2 falls at once: {first:?}"
    );
    let paired = lesson65_run([[1.0, 0.0], [0.0, 1.0]], (6.75, 1.73), true, 1)[1];
    assert!(paired.heater[1] == 50.0, "paired: {paired:?}");
    let last = decoupled.last().unwrap();
    assert!(
        (last.heater[0] - 68.0).abs() < 0.5
            && (last.heater[1] - 42.0).abs() < 0.5
            && (last.vessel[1] - 30.0).abs() < 0.05,
        "steady state: {last:?}"
    );
}

const LESSON66: &str = "10-multivariable-control/65-robust-mimo-selection";

/// One 0.1 s update of the 24 pairs, ordered architecture (paired, decoupled) × coupling (6, 8, 10 kW/°C) × event
/// (vessel-1 target, vessel-2 target, vessel-1 load, vessel-2 load).
#[derive(Clone, Debug)]
struct Cases {
    vessel: Vec<[f64; 2]>,
    heater: Vec<[f64; 2]>,
}

/// One update at rest with Control Studio's coupling setting, then the project experiment (activation at %MX16.0).
fn lesson66_run(coupling: f64, updates: u64) -> Vec<Cases> {
    let (mut runtime, mut simulation) = setup(LESSON66);
    let Value::Instance(program) = runtime.storage().get_global("P1").cloned().unwrap() else {
        panic!("P1 program")
    };
    assert!(runtime
        .write_instance(program, "CouplingCase", Value::LReal(coupling))
        .is_ok());
    let debug = runtime.enable_debug();
    let mut record = Vec::new();
    for step in 0..1 + updates {
        if step == 1 {
            debug.enqueue_io_write(IoAddress::parse("%MX16.0").unwrap(), Value::Bool(true));
        }
        scan_every(&mut runtime, &mut simulation, &debug, step * 100, 100, &[]);
        record.push(Cases {
            vessel: (0..24)
                .map(|k| {
                    [
                        number(&runtime, &format!("%IL{}", 16 * k)),
                        number(&runtime, &format!("%IL{}", 16 * k + 8)),
                    ]
                })
                .collect(),
            heater: (0..24)
                .map(|k| {
                    [
                        number(&runtime, &format!("%QL{}", 16 * k)),
                        number(&runtime, &format!("%QL{}", 16 * k + 8)),
                    ]
                })
                .collect(),
        });
    }
    record
}

/// For case `k`: whether it meets Lesson 71's requirements in the first 60 s, and the other vessel's largest deviation.
fn lesson66_case(record: &[Cases], k: usize) -> (bool, f64) {
    let run = &record[1..601];
    let event = k % 4;
    let own = event % 2;
    let target = if event < 2 { 37.0 } else { 35.0 };
    let series: Vec<f64> = run.iter().map(|u| u.vessel[k][own]).collect();
    let late = series[300..]
        .iter()
        .map(|t| (t - target).abs())
        .fold(0.0, f64::max);
    let other = run
        .iter()
        .map(|u| (u.vessel[k][1 - own] - 35.0).abs())
        .fold(0.0, f64::max);
    let heaters = run
        .iter()
        .all(|u| u.heater[k].iter().all(|h| (0.0..=100.0).contains(h)));
    let own_ok = if event < 2 {
        series.iter().cloned().fold(f64::MIN, f64::max) <= 37.5
    } else {
        series.iter().cloned().fold(f64::MAX, f64::min) >= 34.0
    };
    (own_ok && late <= 0.2 && other <= 0.2 && heaters, other)
}

#[test]
fn lesson66_studio_s_designs_are_frozen_and_only_the_decoupler_meets_the_unchanged_vessel_limit_from_8_kw(
) {
    // Both arrangements as Control Studio designed them for 8 kW/°C, frozen: Lesson 71's decentralized PI (Kp 6.75, Ki 1.73)
    // and Lesson 72's decoupled group (Kp 5.42, Ki 1.74, D = (81/65)·[[1, −4/9], [−4/9, 1]]) on deviations from 75 %. The
    // first update of a vessel-1 target step: paired 75 + 6.75·2 + 1.73·0.2 = 88.846 % and 75 %; decoupled 75 + (81/65)·11.188
    // = 88.942 % and 75 − (36/65)·11.188 = 68.804 %. Paired PI passes the loads but lets the unchanged vessel stray more than
    // 0.2 °C after a target step at every coupling. The frozen decoupler passes every case at 8 and 10 kW/°C, but at 6 kW/°C
    // it turns the other heater down more than the wall delivers and the unchanged vessel strays more than 0.2 °C; its dip
    // shrinks as the coupling grows. The vessel-1 target ends at 75 + (10 + Hc) and 75 − Hc %, the loads at 80/75 %.
    let mut dips = Vec::new();
    for (c, hc) in [6.0, 8.0, 10.0].into_iter().enumerate() {
        let record = lesson66_run(hc, 900);
        let first = &record[1];
        let (paired, decoupled) = (first.heater[4 * c], first.heater[12 + 4 * c]);
        assert!(
            (paired[0] - 88.846).abs() < 1e-9 && paired[1] == 75.0,
            "Hc {hc} paired first update: {paired:?}"
        );
        assert!(
            (decoupled[0] - (75.0 + 81.0 / 65.0 * 11.188)).abs() < 1e-9
                && (decoupled[1] - (75.0 - 36.0 / 65.0 * 11.188)).abs() < 1e-9,
            "Hc {hc} decoupled first update: {decoupled:?}"
        );
        for arch in 0..2 {
            for event in 0..4 {
                let k = 12 * arch + 4 * c + event;
                let (pass, other) = lesson66_case(&record, k);
                let expected = event >= 2 || (arch == 1 && hc > 7.0);
                assert_eq!(
                    pass, expected,
                    "Hc {hc} arch {arch} event {event}: other vessel {other}"
                );
            }
            let last = record.last().unwrap();
            let (target, load) = (
                last.heater[12 * arch + 4 * c],
                last.heater[12 * arch + 4 * c + 2],
            );
            assert!(
                (target[0] - (85.0 + hc)).abs() < 0.3 && (target[1] - (75.0 - hc)).abs() < 0.3,
                "Hc {hc} arch {arch}: {target:?}"
            );
            assert!(
                (load[0] - 80.0).abs() < 0.2 && (load[1] - 75.0).abs() < 0.2,
                "Hc {hc} arch {arch} load: {load:?}"
            );
        }
        let idle = if c == 0 { 4 } else { 0 };
        assert!(
            record.iter().all(|u| u.heater[idle] == [75.0, 75.0]),
            "Hc {hc}: other sets rest at 75 %"
        );
        dips.push(lesson66_case(&record, 12 + 4 * c).1);
    }
    assert!(
        dips[0] > dips[1] && dips[1] > dips[2],
        "decoupled dips {dips:?}"
    );
}
