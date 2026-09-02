use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

#[path = "control_curriculum_motion_progression/evidence.rs"]
#[allow(dead_code)]
mod evidence;

fn project_file(relative: &str) -> String {
    curriculum_project_file("heat-exchanger-tuning-methods", relative)
}

fn tuning_project(project: &str) -> &str {
    match project {
        "heat-exchanger-tuning-methods" => "42-heat-exchanger-tuning-methods",
        "relay-autotuning" => "43-relay-autotuning",
        "model-based-tuning" => "44-model-based-tuning",
        _ => panic!("unknown PID-tuning lesson: {project}"),
    }
}

fn curriculum_project_file(project: &str, relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/06-pid-tuning-methods")
        .join(tuning_project(project))
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn curriculum_artifact_root(project: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/06-pid-tuning-methods")
        .join(tuning_project(project))
        .join("figures")
}

fn write_or_compare_curriculum_artifact(path: &Path, expected: &str) {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("curriculum artifact parent"))
            .expect("create curriculum artifact directory");
        std::fs::write(path, expected).expect("write curriculum artifact");
        return;
    }
    let actual = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read retained artifact {}: {error}", path.display()));
    assert_eq!(
        actual,
        expected,
        "retained artifact drift: {}",
        path.display()
    );
}

fn relative_variation(values: &[f64]) -> f64 {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values
        .iter()
        .map(|value| (value - mean).abs())
        .fold(0.0, f64::max)
        / mean.abs()
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-tuning-progression-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn read_lreal(runtime: &trust_runtime::Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn write_lreal(runtime: &mut trust_runtime::Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL value");
}

fn moving_average(values: &[f64], width: usize) -> Vec<f64> {
    values
        .windows(width)
        .map(|window| window.iter().sum::<f64>() / width as f64)
        .collect()
}

fn cumulative_movement(values: &[f64]) -> f64 {
    values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum()
}

fn time_at_limits(values: &[f64], minimum: f64, maximum: f64, sample_seconds: f64) -> f64 {
    values
        .iter()
        .filter(|value| (**value - minimum).abs() <= 1e-9 || (**value - maximum).abs() <= 1e-9)
        .count() as f64
        * sample_seconds
}

fn settling_time(
    values: &[f64],
    setpoint: f64,
    start: usize,
    end: usize,
    tolerance: f64,
    dwell_samples: usize,
) -> Option<f64> {
    (start..end.saturating_sub(dwell_samples)).find_map(|index| {
        values[index..index + dwell_samples]
            .iter()
            .all(|value| (value - setpoint).abs() <= tolerance)
            .then_some((index - start) as f64 * 0.1)
    })
}

fn semicolon_values(values: impl IntoIterator<Item = f64>) -> String {
    values
        .into_iter()
        .map(|value| format!("{value:.3}"))
        .collect::<Vec<_>>()
        .join(";")
}

#[test]
fn control_curriculum_manual_and_historical_pi_expose_their_tradeoffs() {
    let shipped_source = project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        // Legacy names keep the pre-change tuning draft executable for the red run.
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX64.0 : BOOL :=")
        .replace("ManipulatedValue : LREAL :=", "ManipulatedValue AT %QL0 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        // Lesson 42 exports four equivalent experiments and its configured gains.
        .replace("Setpoint : LREAL :=", "Setpoint AT %ML24 : LREAL :=")
        .replace("HistoricalKp : LREAL :=", "HistoricalKp AT %ML32 : LREAL :=")
        .replace("HistoricalKi : LREAL :=", "HistoricalKi AT %ML40 : LREAL :=")
        .replace("ManualPRequested : LREAL :=", "ManualPRequested AT %ML48 : LREAL :=")
        .replace("CriticalPRequested : LREAL :=", "CriticalPRequested AT %ML56 : LREAL :=")
        .replace("ManualPApplied : LREAL :=", "ManualPApplied AT %QL0 : LREAL :=")
        .replace("ManualPiApplied : LREAL :=", "ManualPiApplied AT %QL8 : LREAL :=")
        .replace("CriticalPApplied : LREAL :=", "CriticalPApplied AT %QL16 : LREAL :=")
        .replace("HistoricalPiApplied : LREAL :=", "HistoricalPiApplied AT %QL24 : LREAL :=")
        .replace("ManualPTemperature : LREAL :=", "ManualPTemperature AT %IL0 : LREAL :=")
        .replace("ManualPiTemperature : LREAL :=", "ManualPiTemperature AT %IL8 : LREAL :=")
        .replace("CriticalPTemperature : LREAL :=", "CriticalPTemperature AT %IL16 : LREAL :=")
        .replace("HistoricalPiTemperature : LREAL :=", "HistoricalPiTemperature AT %IL24 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("heat-exchanger-tuning-methods/src/Main.st", source.clone()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped manual and historical tuning lesson");

    let config_source = project_file("simulation.toml");
    let config_path = temporary_config_path();
    std::fs::write(&config_path, &config_source).expect("write tuning simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load four heat-exchanger tuning cases"),
    );
    let setpoint = IoAddress::parse("%ML24").unwrap();
    let applied = ["%QL0", "%QL8", "%QL16", "%QL24"].map(|value| IoAddress::parse(value).unwrap());
    let measured = ["%IL0", "%IL8", "%IL16", "%IL24"].map(|value| IoAddress::parse(value).unwrap());
    let historical_kp = IoAddress::parse("%ML32").unwrap();
    let historical_ki = IoAddress::parse("%ML40").unwrap();
    let requested =
        ["%ML48", "%QL8", "%ML56", "%QL24"].map(|value| IoAddress::parse(value).unwrap());

    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &setpoint, 50.0);
    let mut temperatures: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::with_capacity(1_850));
    let mut commands: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::with_capacity(1_850));
    let mut requests: [Vec<f64>; 4] = std::array::from_fn(|_| Vec::with_capacity(1_850));
    for cycle in 1usize..=1_850 {
        if cycle == 101 {
            write_lreal(&mut runtime, &setpoint, 52.5);
        }
        if cycle == 1_801 {
            write_lreal(&mut runtime, &setpoint, 50.0);
        }
        runtime
            .execute_cycle()
            .expect("execute four tuning candidates");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance four tuning plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish four temperatures");
        for index in 0..4 {
            temperatures[index].push(read_lreal(&runtime, &measured[index]));
            commands[index].push(read_lreal(&runtime, &applied[index]));
            requests[index].push(read_lreal(&runtime, &requested[index]));
        }
    }

    let input_spans = commands.each_ref().map(|trace| {
        trace.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - trace.iter().copied().fold(f64::INFINITY, f64::min)
    });
    println!(
        "configured historical gains={:.6}/{:.6}, first/last commands={:?}/{:?}",
        read_lreal(&runtime, &historical_kp),
        read_lreal(&runtime, &historical_ki),
        commands.each_ref().map(|trace| trace[0]),
        commands.each_ref().map(|trace| trace[1_799]),
    );
    assert!(
        input_spans.iter().all(|span| *span > 0.0),
        "all four executable controller cases must move their valve: {input_spans:?}"
    );

    let manual_p_late_error = temperatures[0][800..1_000]
        .iter()
        .map(|pv| (52.5 - pv).abs())
        .sum::<f64>()
        / 200.0;
    let critical_smoothed = moving_average(&temperatures[2][200..1_000], 9);
    let critical_crossings = critical_smoothed
        .windows(2)
        .enumerate()
        .filter_map(|(index, pair)| {
            if (pair[0] - 52.5) * (pair[1] - 52.5) < 0.0 {
                Some(index)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let full_periods = critical_crossings
        .windows(3)
        .map(|crossings| (crossings[2] - crossings[0]) as f64 * 0.1)
        .collect::<Vec<_>>();
    let measured_pu = full_periods.iter().sum::<f64>() / full_periods.len() as f64;

    let final_error =
        |trace: &[f64]| (52.5 - trace[1_700..1_800].iter().sum::<f64>() / 100.0).abs();
    let rise_time = |trace: &[f64]| {
        let ten = trace
            .iter()
            .position(|pv| *pv >= 50.25)
            .expect("10 percent crossing");
        let ninety = trace
            .iter()
            .position(|pv| *pv >= 52.25)
            .expect("90 percent crossing");
        (ninety - ten) as f64 * 0.1
    };
    let overshoot_percent = |trace: &[f64]| {
        ((trace[..1_000]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            - 52.5)
            / 2.5
            * 100.0)
            .max(0.0)
    };
    let travel = |trace: &[f64]| {
        trace
            .windows(2)
            .map(|pair| (pair[1] - pair[0]).abs())
            .sum::<f64>()
    };
    let manual_final_error = final_error(&temperatures[1]);
    let historical_final_error = final_error(&temperatures[3]);
    let manual_rise = rise_time(&temperatures[1]);
    let historical_rise = rise_time(&temperatures[3]);
    let manual_overshoot = overshoot_percent(&temperatures[1]);
    let manual_travel = travel(&commands[1]);
    let historical_travel = travel(&commands[3]);

    let slow_source = source.replace(
        "ManualPiParameters.IntegralGain := LREAL#0.10;",
        "ManualPiParameters.IntegralGain := LREAL#0.05;",
    );
    let mut slow_runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("heat-exchanger-tuning-methods/src/Main.st", slow_source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile retained first manual PI trial");
    let mut slow_simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("reload identical tuning conditions"),
    );
    slow_simulation
        .apply_pre_cycle(Duration::ZERO, &mut slow_runtime)
        .unwrap();
    write_lreal(&mut slow_runtime, &setpoint, 50.0);
    let mut slow_manual_pi = Vec::with_capacity(1_850);
    let mut slow_manual_command = Vec::with_capacity(1_850);
    for cycle in 1usize..=1_850 {
        if cycle == 101 {
            write_lreal(&mut slow_runtime, &setpoint, 52.5);
        }
        if cycle == 1_801 {
            write_lreal(&mut slow_runtime, &setpoint, 50.0);
        }
        slow_runtime
            .execute_cycle()
            .expect("execute first manual PI trial");
        let now = Duration::from_millis((cycle * 100) as i64);
        slow_simulation
            .apply_post_cycle(now, &slow_runtime)
            .unwrap();
        slow_simulation
            .apply_pre_cycle(now, &mut slow_runtime)
            .unwrap();
        slow_manual_pi.push(read_lreal(&slow_runtime, &measured[1]));
        slow_manual_command.push(read_lreal(&slow_runtime, &applied[1]));
    }
    let slow_manual_final_error = final_error(&slow_manual_pi);

    let ku = 22.235_014_648_160_227;
    let pu = 11.349_231_501_873_16;
    let expected_kp = 0.45 * ku;
    let expected_ki = expected_kp / (pu / 1.2);
    assert!((read_lreal(&runtime, &historical_kp) - expected_kp).abs() <= 0.02);
    assert!((read_lreal(&runtime, &historical_ki) - expected_ki).abs() <= 0.02);
    assert!(
        manual_p_late_error >= 1.1 && manual_p_late_error <= 1.4,
        "manual P late error={manual_p_late_error}"
    );
    assert!(
        slow_manual_final_error >= 0.25,
        "first manual PI trial must miss recovery requirement, error={slow_manual_final_error}"
    );
    assert!(
        critical_crossings.len() >= 12,
        "critical loop needs at least six alternating cycles, crossings={}",
        critical_crossings.len()
    );
    assert!(
        measured_pu >= 9.0 && measured_pu <= 14.0,
        "critical period={measured_pu}"
    );
    assert!(
        manual_final_error <= 0.10,
        "manual PI final error={manual_final_error}"
    );
    assert!(
        historical_final_error <= 0.10,
        "historical PI final error={historical_final_error}"
    );
    assert!(
        historical_rise < manual_rise,
        "historical/manual rise time={historical_rise}/{manual_rise}"
    );
    assert!(
        manual_overshoot <= 5.0,
        "manual PI overshoot={manual_overshoot}%"
    );
    assert!(
        historical_travel >= 2.0 * manual_travel,
        "historical/manual valve travel={historical_travel}/{manual_travel}"
    );
    assert!(commands
        .iter()
        .flatten()
        .all(|mv| *mv >= 0.0 && *mv <= 100.0));
    assert_eq!(config_source.matches("[[plants]]").count(), 4);
    for forbidden in ["RiseTime", "Overshoot", "ValveTravel", "AcceptedCandidate"] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds result '{forbidden}'"
        );
    }

    let cases = [
        (
            "manual_p",
            2.0,
            0.0,
            "rejected_offset",
            &temperatures[0],
            &requests[0],
            &commands[0],
            Some(manual_p_late_error),
            None,
            None,
        ),
        (
            "manual_pi_rejected",
            2.0,
            0.05,
            "rejected_recovery",
            &slow_manual_pi,
            &slow_manual_command,
            &slow_manual_command,
            Some(slow_manual_final_error),
            None,
            None,
        ),
        (
            "manual_pi",
            2.0,
            0.10,
            "preferred_for_lesson",
            &temperatures[1],
            &requests[1],
            &commands[1],
            Some(manual_final_error),
            Some(manual_rise),
            Some(manual_overshoot),
        ),
        (
            "historical_pi",
            expected_kp,
            expected_ki,
            "retained_not_default",
            &temperatures[3],
            &requests[3],
            &commands[3],
            Some(historical_final_error),
            Some(historical_rise),
            None,
        ),
        (
            "critical_p",
            ku,
            0.0,
            "no_candidate",
            &temperatures[2],
            &requests[2],
            &commands[2],
            None,
            None,
            None,
        ),
    ];
    let mut summary = String::from(
        "case,kp_percent_per_degC,ki_percent_per_degC_s,decision,model,operating_region,controller_form,bias_percent,output_minimum_percent,output_maximum_percent,sample_time_s,seed,applied,settling_status,settling_time_s,late_or_final_error_degC,load_iae_degC_s,rise_time_s,overshoot_percent,cumulative_applied_mv_movement_pp,peak_requested_mv_percent,peak_applied_mv_percent,saturation_duration_s,critical_peak_count,invalid_sample_count,abort_reason,restoration_status,restored_sp_degC,source\n",
    );
    for (case, kp, ki, decision, pv, requested_mv, applied_mv, error, rise, overshoot) in &cases {
        let settled = settling_time(pv, 52.5, 100, 1_000, 0.1, 50);
        let load_iae = pv[1_000..1_800]
            .iter()
            .map(|value| (52.5 - value).abs())
            .sum::<f64>()
            * 0.1;
        writeln!(
            summary,
            "{case},{kp:.12},{ki:.12},{decision},first_order_dead_time,PV_48_to_56_degC__MV_0_to_100_percent,parallel_positional,20.0,0.0,100.0,0.1,20260829,false,{},{},{},{load_iae:.12},{},{},{:.12},{:.12},{:.12},{:.12},{},0,none,confirmed,{:.12},configured_simulation",
            if settled.is_some() { "settled" } else { "not_settled" },
            settled.map(|value| format!("{value:.12}")).unwrap_or_default(),
            error.map(|value| format!("{value:.12}")).unwrap_or_default(),
            rise.map(|value| format!("{value:.12}")).unwrap_or_default(),
            overshoot.map(|value| format!("{value:.12}")).unwrap_or_default(),
            cumulative_movement(applied_mv),
            requested_mv.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            applied_mv.iter().copied().fold(f64::NEG_INFINITY, f64::max),
            time_at_limits(applied_mv, 0.0, 100.0, 0.1),
            if *case == "critical_p" { critical_crossings.len() } else { 0 },
            50.0,
        )
        .unwrap();
    }
    let mut trace = String::from(
        "sample,time_s,case,setpoint_degC,pv_degC,requested_mv_percent,applied_mv_percent,mode,quality,saturated,event\n",
    );
    for (case, _, _, _, pv, requested_mv, applied_mv, _, _, _) in &cases {
        for index in 0..pv.len() {
            let event = match index {
                100 => "setpoint_step",
                999 => "load_disturbance",
                1_800 => "restoration",
                _ => "none",
            };
            writeln!(
                trace,
                "{},{:.3},{case},{:.12},{:.12},{:.12},{:.12},auto,good,{},{event}",
                index + 1,
                (index + 1) as f64 * 0.1,
                if (100..1_800).contains(&index) {
                    52.5
                } else {
                    50.0
                },
                pv[index],
                requested_mv[index],
                applied_mv[index],
                (requested_mv[index] - applied_mv[index]).abs() > 1e-9,
            )
            .unwrap();
        }
    }
    evidence::retain_with_trace(
        &curriculum_artifact_root("heat-exchanger-tuning-methods"),
        "manual-historical-tuning-comparison",
        "Manual and historical heat-exchanger tuning",
        "Same plant and events; the rejected manual trial and critical experiment remain visible",
        0.1,
        &[
            evidence::Panel {
                title: "Outlet temperature",
                unit: "degC",
                minimum: 48.0,
                maximum: 56.0,
                series: vec![
                    evidence::Series {
                        name: "Manual P",
                        color: "#0072b2",
                        dashed: false,
                        values: &temperatures[0],
                    },
                    evidence::Series {
                        name: "Manual PI",
                        color: "#009e73",
                        dashed: false,
                        values: &temperatures[1],
                    },
                    evidence::Series {
                        name: "Rejected Ki=0.05",
                        color: "#cc79a7",
                        dashed: true,
                        values: &slow_manual_pi,
                    },
                    evidence::Series {
                        name: "Critical P",
                        color: "#d55e00",
                        dashed: true,
                        values: &temperatures[2],
                    },
                    evidence::Series {
                        name: "Historical PI",
                        color: "#e69f00",
                        dashed: false,
                        values: &temperatures[3],
                    },
                ],
            },
            evidence::Panel {
                title: "Applied valve position",
                unit: "%",
                minimum: 0.0,
                maximum: 100.0,
                series: vec![
                    evidence::Series {
                        name: "Manual P MV",
                        color: "#0072b2",
                        dashed: false,
                        values: &commands[0],
                    },
                    evidence::Series {
                        name: "Manual PI MV",
                        color: "#009e73",
                        dashed: false,
                        values: &commands[1],
                    },
                    evidence::Series {
                        name: "Rejected MV",
                        color: "#cc79a7",
                        dashed: true,
                        values: &slow_manual_command,
                    },
                    evidence::Series {
                        name: "Critical P MV",
                        color: "#d55e00",
                        dashed: true,
                        values: &commands[2],
                    },
                    evidence::Series {
                        name: "Historical PI MV",
                        color: "#e69f00",
                        dashed: false,
                        values: &commands[3],
                    },
                ],
            },
        ],
        &[
            evidence::Marker {
                at_seconds: 10.0,
                label: "setpoint step",
            },
            evidence::Marker {
                at_seconds: 100.0,
                label: "load disturbance",
            },
            evidence::Marker {
                at_seconds: 180.0,
                label: "restoration",
            },
        ],
        &trace,
        &summary,
    );
    let _ = std::fs::remove_file(config_path);

    println!(
        "manual/historical PI: P offset={manual_p_late_error:.3}degC, rejected Ki=0.05 final error={slow_manual_final_error:.3}degC, critical crossings/Pu={}/{measured_pu:.3}s, accepted/historical final error={manual_final_error:.3}/{historical_final_error:.3}degC, rise={manual_rise:.2}/{historical_rise:.2}s, manual overshoot={manual_overshoot:.2}%, travel={manual_travel:.2}/{historical_travel:.2}pp",
        critical_crossings.len()
    );
}

#[test]
fn control_curriculum_feedback_relay_qualifies_cycles_before_producing_candidate() {
    let shipped_source = curriculum_project_file("relay-autotuning", "src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("relay lesson program before configuration")
        .replace(
            "RelayAmplitude : LREAL :=",
            "RelayAmplitude AT %ML0 : LREAL :=",
        )
        .replace(
            "QualifiedRelayApplied : LREAL :=",
            "QualifiedRelayApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "RejectedRelayApplied : LREAL :=",
            "RejectedRelayApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "QualifiedTemperature : LREAL :=",
            "QualifiedTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "RejectedTemperature : LREAL :=",
            "RejectedTemperature AT %IL8 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "relay-autotuning/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped feedback-relay lesson");

    let config_source = curriculum_project_file("relay-autotuning", "simulation.toml");
    let config_path = temporary_config_path();
    std::fs::write(&config_path, &config_source).expect("write relay simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load qualified and rejected relay plants"),
    );
    let relay_amplitude = IoAddress::parse("%ML0").unwrap();
    let applied = ["%QL0", "%QL8"].map(|value| IoAddress::parse(value).unwrap());
    let measured = ["%IL0", "%IL8"].map(|value| IoAddress::parse(value).unwrap());

    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &relay_amplitude, 0.0);
    let mut commands: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(1_920));
    let mut temperatures: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(1_920));
    for cycle in 1usize..=1_920 {
        if cycle == 101 {
            write_lreal(&mut runtime, &relay_amplitude, 18.0);
        }
        if cycle == 1_901 {
            write_lreal(&mut runtime, &relay_amplitude, 0.0);
        }
        runtime.execute_cycle().expect("execute feedback relays");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance relay plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish relay temperatures");
        for index in 0..2 {
            commands[index].push(read_lreal(&runtime, &applied[index]));
            temperatures[index].push(read_lreal(&runtime, &measured[index]));
        }
    }

    let transition_indices = commands[0][100..1_900]
        .windows(2)
        .enumerate()
        .filter_map(|(index, pair)| ((pair[1] - pair[0]).abs() > 1.0).then_some(index + 101))
        .collect::<Vec<_>>();
    assert!(
        transition_indices.len() >= 12,
        "qualified relay needs at least six complete alternating cycles: {transition_indices:?}"
    );
    let evaluation_transitions = &transition_indices[4..];
    let periods = evaluation_transitions
        .windows(3)
        .map(|window| (window[2] - window[0]) as f64 * 0.1)
        .collect::<Vec<_>>();
    let period = periods.iter().sum::<f64>() / periods.len() as f64;

    let smoothed = moving_average(&temperatures[0], 5);
    let mut upper_peaks = Vec::new();
    let mut lower_troughs = Vec::new();
    for window in evaluation_transitions.windows(2) {
        let start = window[0].min(smoothed.len() - 1);
        let end = window[1].min(smoothed.len());
        if end <= start {
            continue;
        }
        if commands[0][window[0]] < 20.0 {
            upper_peaks.push(
                smoothed[start..end]
                    .iter()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max),
            );
        } else {
            lower_troughs.push(
                smoothed[start..end]
                    .iter()
                    .copied()
                    .fold(f64::INFINITY, f64::min),
            );
        }
    }
    assert!(!upper_peaks.is_empty() && !lower_troughs.is_empty());
    let upper_mean = upper_peaks.iter().sum::<f64>() / upper_peaks.len() as f64;
    let lower_mean = lower_troughs.iter().sum::<f64>() / lower_troughs.len() as f64;
    let amplitude = (upper_mean - lower_mean) / 2.0;
    let amplitude_variation = relative_variation(
        &upper_peaks
            .iter()
            .zip(&lower_troughs)
            .map(|(high, low)| (high - low) / 2.0)
            .collect::<Vec<_>>(),
    );
    let period_variation = relative_variation(&periods);
    let snr = amplitude / 0.05;
    let estimated_ku = 4.0 * 18.0 / (std::f64::consts::PI * amplitude);
    let reference_ku = 22.235_014_648_160_227;
    let ku_relative_error = (estimated_ku - reference_ku).abs() / reference_ku;

    let mut rejected_sorted = temperatures[1][300..1_900].to_vec();
    rejected_sorted.sort_by(f64::total_cmp);
    let rejected_amplitude = (rejected_sorted[1_519] - rejected_sorted[80]) / 2.0;
    let rejected_snr = rejected_amplitude / 0.05;

    assert_eq!(
        commands[0][100], 38.0,
        "qualified relay must begin at its high level"
    );
    assert!(commands[0][100..1_900]
        .iter()
        .all(|value| *value == 2.0 || *value == 38.0));
    assert!(commands[1][100..1_900]
        .iter()
        .all(|value| *value == 19.8 || *value == 20.2));
    assert_eq!(
        commands[0][1_919], 20.0,
        "qualified relay must restore bias"
    );
    assert_eq!(commands[1][1_919], 20.0, "rejected relay must restore bias");
    assert!(
        period >= 9.0 && period <= 14.0,
        "qualified relay period={period}"
    );
    assert!(
        amplitude > 0.32,
        "qualified amplitude must exceed four hysteresis widths: {amplitude}"
    );
    assert!(snr >= 5.0, "qualified relay SNR={snr}");
    assert!(amplitude_variation <= 0.10,
        "qualified amplitude variation={amplitude_variation}, upper={upper_peaks:?}, lower={lower_troughs:?}");
    assert!(
        period_variation <= 0.10,
        "qualified period variation={period_variation}"
    );
    assert!(
        ku_relative_error <= 0.20 + 0.01,
        "relay/reference Ku={estimated_ku}/{reference_ku}, relative error={ku_relative_error}"
    );
    assert!(
        rejected_amplitude <= 0.32 || rejected_snr < 5.0,
        "low-amplitude relay must be rejected: amplitude/SNR={rejected_amplitude}/{rejected_snr}"
    );
    for forbidden in [
        "UltimateGain",
        "UltimatePeriod",
        "CandidateKp",
        "QualifiedRecord",
    ] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds derived result '{forbidden}'"
        );
    }
    assert_eq!(config_source.matches("[[plants]]").count(), 2);

    let rejected_transitions = commands[1][100..1_900]
        .windows(2)
        .filter(|pair| (pair[1] - pair[0]).abs() > 0.01)
        .count();
    let candidate_kp = 0.45 * estimated_ku;
    let candidate_ti = period / 1.2;
    let candidate_ki = candidate_kp / candidate_ti;
    let crossing_times =
        semicolon_values(transition_indices.iter().map(|index| *index as f64 * 0.1));
    let upper_extrema = semicolon_values(upper_peaks.iter().copied());
    let lower_extrema = semicolon_values(lower_troughs.iter().copied());
    let summary = format!(
        "case,relay_center_percent,relay_amplitude_pp,hysteresis_half_width_degC,model_gain_degC_per_percent,model_time_constant_s,model_dead_time_s,output_minimum_percent,output_maximum_percent,sample_time_s,seed,process_amplitude_degC,period_s,amplitude_variation,period_variation,snr,transition_count,crossing_times_s,upper_extrema_degC,lower_extrema_degC,saturation_duration_s,invalid_sample_count,stop_reason,failed_criteria,estimated_ku_percent_per_degC,candidate_kp_percent_per_degC,candidate_ti_s,candidate_ki_percent_per_degC_s,decision,candidate_applied,restoration_status,restored_percent,source\nqualified,20.0,18.0,0.08,0.5,20.0,3.0,0.0,100.0,0.1,20260829,{amplitude:.12},{period:.12},{amplitude_variation:.12},{period_variation:.12},{snr:.12},{},{crossing_times},{upper_extrema},{lower_extrema},0.0,0,duration_complete,none,{estimated_ku:.12},{candidate_kp:.12},{candidate_ti:.12},{candidate_ki:.12},candidate_unapplied,false,confirmed,{:.12},configured_simulation\nlow_amplitude,20.0,0.2,0.08,0.5,20.0,3.0,0.0,100.0,0.1,20260829,{rejected_amplitude:.12},,,, {rejected_snr:.12},{rejected_transitions},,,,0.0,0,insufficient_excitation,process_amplitude_below_4_hysteresis_and_snr_below_5,,,,,no_tuning_candidate,false,confirmed,{:.12},configured_simulation\n",
        transition_indices.len(), commands[0][1_919], commands[1][1_919]
    ).replace(", ", ",");
    let mut trace = String::from(
        "sample,time_s,case,pv_degC,requested_mv_percent,applied_mv_percent,mode,quality,saturated,event\n",
    );
    for (case, pv, mv) in [
        ("qualified", &temperatures[0], &commands[0]),
        ("low_amplitude", &temperatures[1], &commands[1]),
    ] {
        for index in 0..pv.len() {
            let transition = index > 0 && (mv[index] - mv[index - 1]).abs() > 0.01;
            let event = if index == 100 {
                "relay_enabled"
            } else if index == 1_900 {
                "restoration"
            } else if transition {
                "threshold_crossing"
            } else {
                "none"
            };
            writeln!(
                trace,
                "{},{:.3},{case},{:.12},{:.12},{:.12},relay,good,false,{event}",
                index + 1,
                (index + 1) as f64 * 0.1,
                pv[index],
                mv[index],
                mv[index],
            )
            .unwrap();
        }
    }
    evidence::retain_with_trace(
        &curriculum_artifact_root("relay-autotuning"),
        "relay-autotuning-comparison",
        "Qualified and rejected feedback-relay tests",
        "Feedback thresholds drive switching; the low-amplitude record remains rejected",
        0.1,
        &[
            evidence::Panel {
                title: "Outlet temperature",
                unit: "degC",
                minimum: 48.0,
                maximum: 52.0,
                series: vec![
                    evidence::Series {
                        name: "Qualified PV",
                        color: "#0072b2",
                        dashed: false,
                        values: &temperatures[0],
                    },
                    evidence::Series {
                        name: "Rejected PV",
                        color: "#d55e00",
                        dashed: true,
                        values: &temperatures[1],
                    },
                ],
            },
            evidence::Panel {
                title: "Applied valve position",
                unit: "%",
                minimum: 0.0,
                maximum: 40.0,
                series: vec![
                    evidence::Series {
                        name: "Qualified MV",
                        color: "#0072b2",
                        dashed: false,
                        values: &commands[0],
                    },
                    evidence::Series {
                        name: "Rejected MV",
                        color: "#d55e00",
                        dashed: true,
                        values: &commands[1],
                    },
                ],
            },
        ],
        &[
            evidence::Marker {
                at_seconds: 10.0,
                label: "relay enabled",
            },
            evidence::Marker {
                at_seconds: 190.0,
                label: "restoration",
            },
        ],
        &trace,
        &summary,
    );
    let _ = std::fs::remove_file(config_path);

    println!(
        "feedback relay: cycles={}, amplitude={amplitude:.3}degC, period={period:.3}s, variation={amplitude_variation:.3}/{period_variation:.3}, SNR={snr:.2}, Ku={estimated_ku:.2}; rejected amplitude/SNR={rejected_amplitude:.3}/{rejected_snr:.2}",
        transition_indices.len() / 2,
    );
}

#[derive(Clone, Copy)]
struct ModelBasedCandidate {
    name: &'static str,
    gains: [(f64, f64); 3],
}

struct ModelBasedRun {
    process_values: [Vec<f64>; 3],
    requested_outputs: [Vec<f64>; 3],
    applied_outputs: [Vec<f64>; 3],
}

fn run_model_based_candidate(candidate: ModelBasedCandidate, variation: &str) -> ModelBasedRun {
    let shipped_source = curriculum_project_file("model-based-tuning", "src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("model-based lesson program before configuration")
        .replace(
            "HeaterSetpoint : LREAL :=",
            "HeaterSetpoint AT %ML0 : LREAL :=",
        )
        .replace("HeaterKp : LREAL :=", "HeaterKp AT %ML24 : LREAL :=")
        .replace("HeaterKi : LREAL :=", "HeaterKi AT %ML32 : LREAL :=")
        .replace("TankKp : LREAL :=", "TankKp AT %ML40 : LREAL :=")
        .replace("TankKi : LREAL :=", "TankKi AT %ML48 : LREAL :=")
        .replace("DryerKp : LREAL :=", "DryerKp AT %ML56 : LREAL :=")
        .replace("DryerKi : LREAL :=", "DryerKi AT %ML64 : LREAL :=")
        .replace(
            "HeaterRequested : LREAL :=",
            "HeaterRequested AT %ML72 : LREAL :=",
        )
        .replace(
            "TankRequested : LREAL :=",
            "TankRequested AT %ML80 : LREAL :=",
        )
        .replace(
            "DryerRequested : LREAL :=",
            "DryerRequested AT %ML88 : LREAL :=",
        )
        .replace(
            "HeaterApplied : LREAL :=",
            "HeaterApplied AT %QL0 : LREAL :=",
        )
        .replace("TankApplied : LREAL :=", "TankApplied AT %QL8 : LREAL :=")
        .replace(
            "DryerApplied : LREAL :=",
            "DryerApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "HeaterTemperature : LREAL :=",
            "HeaterTemperature AT %IL0 : LREAL :=",
        )
        .replace("TankLevel : LREAL :=", "TankLevel AT %IL8 : LREAL :=")
        .replace(
            "DryerDryness : LREAL :=",
            "DryerDryness AT %IL16 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("model-based-tuning/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped model-based tuning lesson");

    let nominal_config = curriculum_project_file("model-based-tuning", "simulation.toml");
    let config_source = match variation {
        "nominal" => nominal_config,
        "gain+20%" => nominal_config
            .replace("gain = 0.2", "gain = 0.24")
            .replace("gain = 0.05", "gain = 0.06")
            .replace("gain = 0.4", "gain = 0.48"),
        "delay+50%" => nominal_config
            .replace("dead_time_s = 1.0", "dead_time_s = 1.5")
            .replace("lag_s = 1.0", "lag_s = 1.5")
            .replace("dead_time_s = 8.0", "dead_time_s = 12.0"),
        other => panic!("unsupported model variation {other}"),
    };
    let config_path = temporary_config_path();
    std::fs::write(&config_path, config_source).expect("write model-based simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load heater, tank, and dryer models"),
    );
    let setpoint = IoAddress::parse("%ML0").unwrap();
    let gain_addresses = [("%ML24", "%ML32"), ("%ML40", "%ML48"), ("%ML56", "%ML64")]
        .map(|(kp, ki)| (IoAddress::parse(kp).unwrap(), IoAddress::parse(ki).unwrap()));
    let applied = ["%QL0", "%QL8", "%QL16"].map(|value| IoAddress::parse(value).unwrap());
    let requested = ["%ML72", "%ML80", "%ML88"].map(|value| IoAddress::parse(value).unwrap());
    let measured = ["%IL0", "%IL8", "%IL16"].map(|value| IoAddress::parse(value).unwrap());

    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish model-based initial states");
    write_lreal(&mut runtime, &setpoint, 20.0);
    for (index, (kp, ki)) in gain_addresses.iter().enumerate() {
        write_lreal(&mut runtime, kp, candidate.gains[index].0);
        write_lreal(&mut runtime, ki, candidate.gains[index].1);
    }
    let mut process_values: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    let mut requested_outputs: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    let mut applied_outputs: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    for cycle in 1usize..=2_200 {
        if cycle == 101 {
            write_lreal(&mut runtime, &setpoint, 25.0);
        }
        runtime
            .execute_cycle()
            .expect("execute model-based PI controllers");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance model-based plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish model-based process values");
        for index in 0..3 {
            process_values[index].push(read_lreal(&runtime, &measured[index]));
            requested_outputs[index].push(read_lreal(&runtime, &requested[index]));
            applied_outputs[index].push(read_lreal(&runtime, &applied[index]));
        }
    }
    let _ = std::fs::remove_file(config_path);
    ModelBasedRun {
        process_values,
        requested_outputs,
        applied_outputs,
    }
}

fn ten_to_ninety_rise(trace: &[f64], baseline: f64, change: f64) -> f64 {
    let start = 100;
    let ten = trace[start..1_000]
        .iter()
        .position(|value| *value >= baseline + 0.1 * change)
        .expect("10 percent response crossing")
        + start;
    let ninety = trace[start..1_000]
        .iter()
        .position(|value| *value >= baseline + 0.9 * change)
        .expect("90 percent response crossing")
        + start;
    (ninety - ten) as f64 * 0.1
}

#[test]
fn control_curriculum_simc_targets_are_model_qualified_and_remain_unapplied() {
    let lesson = curriculum_project_file("model-based-tuning", "lesson.json");
    let lesson_json: serde_json::Value =
        serde_json::from_str(&lesson).expect("parse lesson metadata");
    assert_eq!(lesson_json["change"]["applied"], false);
    assert_eq!(lesson_json["sequence"], 44);

    let candidates = [
        ModelBasedCandidate {
            name: "Fast",
            gains: [(9.375, 1.875), (20.0, 5.0), (3.125, 0.15625)],
        },
        ModelBasedCandidate {
            name: "Balanced",
            gains: [
                (5.769_230_769, 1.153_846_154),
                (13.333_333_333, 2.222_222_222),
                (2.083_333_333, 0.104_166_667),
            ],
        },
        ModelBasedCandidate {
            name: "Robust",
            gains: [(3.260_869_565, 0.652_173_913), (8.0, 0.8), (1.25, 0.0625)],
        },
    ];
    for plant in 0..3 {
        assert!(candidates[0].gains[plant].0 > candidates[1].gains[plant].0);
        assert!(candidates[1].gains[plant].0 > candidates[2].gains[plant].0);
    }

    let nominal = candidates.map(|candidate| run_model_based_candidate(candidate, "nominal"));
    let baselines = [20.0, 50.0, 40.0];
    let changes = [5.0, 2.0, 5.0];
    let setpoints = [25.0, 52.0, 45.0];
    let mut rise_times = [[0.0; 3]; 3];
    let mut settling_times = [[None; 3]; 3];
    let mut overshoots = [[0.0; 3]; 3];
    let mut final_errors = [[0.0; 3]; 3];
    let mut movements = [[0.0; 3]; 3];
    let mut peak_requested = [[0.0; 3]; 3];
    let mut peak_applied = [[0.0; 3]; 3];
    let mut limit_times = [[0.0; 3]; 3];
    for candidate in 0..3 {
        for plant in 0..3 {
            rise_times[candidate][plant] = ten_to_ninety_rise(
                &nominal[candidate].process_values[plant],
                baselines[plant],
                changes[plant],
            );
            let late_error = nominal[candidate].process_values[plant][2_100..2_200]
                .iter()
                .map(|pv| (setpoints[plant] - pv).abs())
                .sum::<f64>()
                / 100.0;
            final_errors[candidate][plant] = late_error;
            settling_times[candidate][plant] = settling_time(
                &nominal[candidate].process_values[plant],
                setpoints[plant],
                100,
                1_000,
                0.02 * changes[plant],
                50,
            );
            overshoots[candidate][plant] = ((nominal[candidate].process_values[plant][100..1_000]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max)
                - setpoints[plant])
                / changes[plant]
                * 100.0)
                .max(0.0);
            movements[candidate][plant] =
                cumulative_movement(&nominal[candidate].applied_outputs[plant]);
            peak_requested[candidate][plant] = nominal[candidate].requested_outputs[plant]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            peak_applied[candidate][plant] = nominal[candidate].applied_outputs[plant]
                .iter()
                .copied()
                .fold(f64::NEG_INFINITY, f64::max);
            limit_times[candidate][plant] =
                time_at_limits(&nominal[candidate].applied_outputs[plant], 0.0, 100.0, 0.1);
            assert!(
                late_error <= 0.25,
                "{} plant {plant} late absolute error={late_error}",
                candidates[candidate].name
            );
            assert!(nominal[candidate].process_values[plant]
                .iter()
                .all(|value| value.is_finite()));
            assert!(nominal[candidate].applied_outputs[plant]
                .iter()
                .all(|value| value.is_finite() && (0.0..=100.0).contains(value)));
        }
    }
    for plant in 0..3 {
        assert!(
            rise_times[0][plant] <= rise_times[1][plant] + 0.1
                && rise_times[1][plant] <= rise_times[2][plant] + 0.1,
            "Fast/Balanced/Robust rise times for plant {plant}: {:?}; first post-step samples: {:?}",
            rise_times.map(|times| times[plant]),
            nominal.each_ref().map(|run| &run.process_values[plant][95..125])
        );
    }

    let plant_names = ["water_heater", "buffer_tank", "belt_dryer"];
    let model_kinds = [
        "first_order_dead_time",
        "integrating_rate",
        "first_order_dead_time",
    ];
    let model_gains = [0.2, 0.05, 0.4];
    let model_time_constants = ["5.0", "", "20.0"];
    let model_dead_times = [1.0, 0.5, 8.0];
    let lambdas = [
        [1.666_666_667, 0.5, 8.0],
        [3.333_333_333, 1.0, 16.0],
        [6.666_666_667, 2.0, 32.0],
    ];
    let biases = [20.0, 50.0, 30.0];
    let pv_units = ["degC", "level_percent", "dryness_percent"];
    let operating_regions = [
        "PV_20_to_25_degC__MV_0_to_100_percent",
        "PV_50_to_52_level_percent__MV_0_to_100_percent",
        "PV_40_to_45_dryness_percent__MV_0_to_100_percent",
    ];
    let mut summary = String::from(
        "record_type,variation,candidate,plant,model_kind,model_gain,model_time_constant_s,model_dead_time_s,lambda_s,kp,ti_s,ki,controller_form,bias_percent,pv_unit,mv_unit,operating_region,sample_time_s,seed,settling_status,settling_time_s,rise_time_s,overshoot_percent,final_window_error,cumulative_mv_movement_pp,peak_requested_mv_percent,peak_applied_mv_percent,time_at_limit_s,minimum_pv,maximum_pv,qualification,applied,quality,event,source\n",
    );
    for candidate in 0..3 {
        for plant in 0..3 {
            let settling = settling_times[candidate][plant]
                .map(|value| format!("{value:.12}"))
                .unwrap_or_default();
            writeln!(
                summary,
                "candidate,nominal,{},{},{},{:.12},{},{:.12},{:.12},{:.12},{:.12},{:.12},parallel_positional,{:.12},{},percent,{},0.1,20260829,{},{},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},qualified_nominal,false,good,setpoint_and_load_validation,configured_simulation",
                candidates[candidate].name,
                plant_names[plant],
                model_kinds[plant],
                model_gains[plant],
                model_time_constants[plant],
                model_dead_times[plant],
                lambdas[candidate][plant],
                candidates[candidate].gains[plant].0,
                candidates[candidate].gains[plant].0 / candidates[candidate].gains[plant].1,
                candidates[candidate].gains[plant].1,
                biases[plant],
                pv_units[plant],
                operating_regions[plant],
                if settling_times[candidate][plant].is_some() { "settled" } else { "not_settled" },
                settling,
                rise_times[candidate][plant],
                overshoots[candidate][plant],
                final_errors[candidate][plant],
                movements[candidate][plant],
                peak_requested[candidate][plant],
                peak_applied[candidate][plant],
                limit_times[candidate][plant],
                nominal[candidate].process_values[plant].iter().copied().fold(f64::INFINITY, f64::min),
                nominal[candidate].process_values[plant].iter().copied().fold(f64::NEG_INFINITY, f64::max),
            )
            .unwrap();
        }
    }
    for variation in ["gain+20%", "delay+50%"] {
        for (candidate_index, candidate) in candidates.iter().copied().enumerate() {
            let run = run_model_based_candidate(candidate, variation);
            for plant in 0..3 {
                assert!(run.process_values[plant]
                    .iter()
                    .all(|value| value.is_finite()));
                assert!(run.applied_outputs[plant]
                    .iter()
                    .all(|value| value.is_finite() && (0.0..=100.0).contains(value)));
                writeln!(
                    summary,
                    "variation,{variation},{},{},{},{:.12},{},{:.12},{:.12},{:.12},{:.12},{:.12},parallel_positional,{:.12},{},percent,{},0.1,20260829,not_evaluated,,,,,,{:.12},{:.12},{:.12},{:.12},{:.12},finite_bounded_variation,false,good,uncertainty_validation,configured_simulation",
                    candidate.name,
                    plant_names[plant],
                    model_kinds[plant],
                    model_gains[plant],
                    model_time_constants[plant],
                    model_dead_times[plant],
                    lambdas[candidate_index][plant],
                    candidate.gains[plant].0,
                    candidate.gains[plant].0 / candidate.gains[plant].1,
                    candidate.gains[plant].1,
                    biases[plant],
                    pv_units[plant],
                    operating_regions[plant],
                    run.requested_outputs[plant].iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    run.applied_outputs[plant].iter().copied().fold(f64::NEG_INFINITY, f64::max),
                    time_at_limits(&run.applied_outputs[plant], 0.0, 100.0, 0.1),
                    run.process_values[plant].iter().copied().fold(f64::INFINITY, f64::min),
                    run.process_values[plant].iter().copied().fold(f64::NEG_INFINITY, f64::max),
                )
                .unwrap();
            }
        }
    }

    let shipped_source = curriculum_project_file("model-based-tuning", "src/Main.st");
    for forbidden in ["RiseTime", "SettlingTime", "AcceptedCandidate", "Winner"] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds result '{forbidden}'"
        );
    }

    let normalized: [[Vec<f64>; 3]; 3] = std::array::from_fn(|candidate| {
        std::array::from_fn(|plant| {
            nominal[candidate].process_values[plant]
                .iter()
                .map(|value| (value - baselines[plant]) / changes[plant])
                .collect()
        })
    });
    let mut trace = String::from(
        "sample,time_s,candidate,plant,model_kind,pv,pv_unit,normalized_pv,requested_mv_percent,applied_mv_percent,quality,event\n",
    );
    for candidate in 0..3 {
        for plant in 0..3 {
            for index in 0..nominal[candidate].process_values[plant].len() {
                let event = match index {
                    100 => "setpoint_step",
                    999 => "load_disturbance",
                    _ => "none",
                };
                writeln!(
                    trace,
                    "{},{:.3},{},{},{},{:.12},{},{:.12},{:.12},{:.12},good,{event}",
                    index + 1,
                    (index + 1) as f64 * 0.1,
                    candidates[candidate].name,
                    plant_names[plant],
                    model_kinds[plant],
                    nominal[candidate].process_values[plant][index],
                    pv_units[plant],
                    normalized[candidate][plant][index],
                    nominal[candidate].requested_outputs[plant][index],
                    nominal[candidate].applied_outputs[plant][index],
                )
                .unwrap();
            }
        }
    }
    evidence::retain_with_trace(
        &curriculum_artifact_root("model-based-tuning"),
        "model-based-tuning-comparison",
        "SIMC response targets on three process types",
        "The same color and dash key applies to normalized PV and applied MV",
        0.1,
        &[
            evidence::Panel {
                title: "Normalized process response",
                unit: "fraction of setpoint change",
                minimum: 0.0,
                maximum: 1.25,
                series: vec![
                    evidence::Series {
                        name: "Heater Fast",
                        color: "#0072b2",
                        dashed: false,
                        values: &normalized[0][0],
                    },
                    evidence::Series {
                        name: "Heater Balanced",
                        color: "#0072b2",
                        dashed: true,
                        values: &normalized[1][0],
                    },
                    evidence::Series {
                        name: "Heater Robust",
                        color: "#56b4e9",
                        dashed: true,
                        values: &normalized[2][0],
                    },
                    evidence::Series {
                        name: "Tank Fast",
                        color: "#009e73",
                        dashed: false,
                        values: &normalized[0][1],
                    },
                    evidence::Series {
                        name: "Tank Balanced",
                        color: "#009e73",
                        dashed: true,
                        values: &normalized[1][1],
                    },
                    evidence::Series {
                        name: "Tank Robust",
                        color: "#66c2a5",
                        dashed: true,
                        values: &normalized[2][1],
                    },
                    evidence::Series {
                        name: "Dryer Fast",
                        color: "#d55e00",
                        dashed: false,
                        values: &normalized[0][2],
                    },
                    evidence::Series {
                        name: "Dryer Balanced",
                        color: "#d55e00",
                        dashed: true,
                        values: &normalized[1][2],
                    },
                    evidence::Series {
                        name: "Dryer Robust",
                        color: "#e69f00",
                        dashed: true,
                        values: &normalized[2][2],
                    },
                ],
            },
            evidence::Panel {
                title: "Applied actuator output",
                unit: "%",
                minimum: 0.0,
                maximum: 100.0,
                series: vec![
                    evidence::Series {
                        name: "Heater Fast MV",
                        color: "#0072b2",
                        dashed: false,
                        values: &nominal[0].applied_outputs[0],
                    },
                    evidence::Series {
                        name: "Heater Balanced MV",
                        color: "#0072b2",
                        dashed: true,
                        values: &nominal[1].applied_outputs[0],
                    },
                    evidence::Series {
                        name: "Heater Robust MV",
                        color: "#56b4e9",
                        dashed: true,
                        values: &nominal[2].applied_outputs[0],
                    },
                    evidence::Series {
                        name: "Tank Fast MV",
                        color: "#009e73",
                        dashed: false,
                        values: &nominal[0].applied_outputs[1],
                    },
                    evidence::Series {
                        name: "Tank Balanced MV",
                        color: "#009e73",
                        dashed: true,
                        values: &nominal[1].applied_outputs[1],
                    },
                    evidence::Series {
                        name: "Tank Robust MV",
                        color: "#66c2a5",
                        dashed: true,
                        values: &nominal[2].applied_outputs[1],
                    },
                    evidence::Series {
                        name: "Dryer Fast MV",
                        color: "#d55e00",
                        dashed: false,
                        values: &nominal[0].applied_outputs[2],
                    },
                    evidence::Series {
                        name: "Dryer Balanced MV",
                        color: "#d55e00",
                        dashed: true,
                        values: &nominal[1].applied_outputs[2],
                    },
                    evidence::Series {
                        name: "Dryer Robust MV",
                        color: "#e69f00",
                        dashed: true,
                        values: &nominal[2].applied_outputs[2],
                    },
                ],
            },
        ],
        &[evidence::Marker {
            at_seconds: 10.0,
            label: "setpoint step",
        }],
        &trace,
        &summary,
    );
    println!(
        "SIMC targets remain unapplied; Fast/Balanced/Robust rise times heater={:?} tank={:?} dryer={:?}",
        rise_times.map(|times| times[0]),
        rise_times.map(|times| times[1]),
        rise_times.map(|times| times[2]),
    );
}
