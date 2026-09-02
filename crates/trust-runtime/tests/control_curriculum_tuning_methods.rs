use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

fn evidence_root(project: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/08-tuning-qualification-and-automation")
        .join(match project {
            "method-comparison" => "51-method-comparison",
            "automatic-water-heater" => "52-automatic-water-heater",
            _ => panic!("unknown tuning-qualification lesson: {project}"),
        })
        .join("figures")
}

fn time_axis(samples: usize, sample_seconds: f64) -> Vec<f64> {
    (1..=samples)
        .map(|sample| sample as f64 * sample_seconds)
        .collect()
}

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/08-tuning-qualification-and-automation/51-method-comparison")
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn automatic_project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/08-tuning-qualification-and-automation/52-automatic-water-heater")
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-tuning-methods-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_bool(runtime: &mut trust_runtime::Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL value");
}

fn read_lreal(runtime: &trust_runtime::Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

#[derive(Debug)]
struct Metrics {
    iae: f64,
    ise: f64,
    itae: f64,
    travel: f64,
    setpoint_overshoot_percent: f64,
    final_mean_absolute_error: f64,
}

fn metrics(trace: &[f64], output: &[f64], baseline: f64, setpoint: f64) -> Metrics {
    assert_eq!(trace.len(), 2_200);
    assert_eq!(output.len(), trace.len());
    let mut iae = 0.0;
    let mut ise = 0.0;
    let mut itae = 0.0;
    let mut travel = 0.0;
    for index in 100..trace.len() {
        let error = setpoint - trace[index];
        let elapsed = (index - 100) as f64 * 0.1;
        iae += error.abs() * 0.1;
        ise += error * error * 0.1;
        itae += elapsed * error.abs() * 0.1;
        travel += (output[index] - output[index - 1]).abs();
    }
    let change = setpoint - baseline;
    let setpoint_peak = trace[100..1_000]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let setpoint_overshoot_percent = ((setpoint_peak - setpoint) / change * 100.0).max(0.0);
    let final_mean_absolute_error = trace[2_100..2_200]
        .iter()
        .map(|value| (setpoint - value).abs())
        .sum::<f64>()
        / 100.0;
    Metrics {
        iae,
        ise,
        itae,
        travel,
        setpoint_overshoot_percent,
        final_mean_absolute_error,
    }
}

#[test]
fn control_curriculum_tuning_methods_separate_eligibility_from_tradeoffs() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse tuning lesson metadata");
    assert_eq!(lesson["sequence"], 51);
    assert_eq!(lesson["change"]["applied"], false);
    assert_eq!(lesson["methods"]["tank"]["AMIGO"]["status"], "Unavailable");
    assert!(lesson["methods"]["tank"]["AMIGO"]["reason"]
        .as_str()
        .expect("tank AMIGO reason")
        .contains("integrating"));

    let shipped_source = project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("lesson program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX96.0 : BOOL :=",
        )
        .replace(
            "HeaterSimcRequested : LREAL :=",
            "HeaterSimcRequested AT %ML128 : LREAL :=",
        )
        .replace(
            "HeaterAmigoRequested : LREAL :=",
            "HeaterAmigoRequested AT %ML136 : LREAL :=",
        )
        .replace(
            "TankSimcRequested : LREAL :=",
            "TankSimcRequested AT %ML144 : LREAL :=",
        )
        .replace(
            "DryerSimcRequested : LREAL :=",
            "DryerSimcRequested AT %ML152 : LREAL :=",
        )
        .replace(
            "DryerAmigoRequested : LREAL :=",
            "DryerAmigoRequested AT %ML160 : LREAL :=",
        )
        .replace(
            "HeaterSimcApplied : LREAL :=",
            "HeaterSimcApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "HeaterAmigoApplied : LREAL :=",
            "HeaterAmigoApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "TankSimcApplied : LREAL :=",
            "TankSimcApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "DryerSimcApplied : LREAL :=",
            "DryerSimcApplied AT %QL24 : LREAL :=",
        )
        .replace(
            "DryerAmigoApplied : LREAL :=",
            "DryerAmigoApplied AT %QL32 : LREAL :=",
        )
        .replace(
            "HeaterSimcTemperature : LREAL :=",
            "HeaterSimcTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "HeaterAmigoTemperature : LREAL :=",
            "HeaterAmigoTemperature AT %IL8 : LREAL :=",
        )
        .replace(
            "TankSimcLevel : LREAL :=",
            "TankSimcLevel AT %IL16 : LREAL :=",
        )
        .replace(
            "DryerSimcDryness : LREAL :=",
            "DryerSimcDryness AT %IL24 : LREAL :=",
        )
        .replace(
            "DryerAmigoDryness : LREAL :=",
            "DryerAmigoDryness AT %IL32 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("method-comparison/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped tuning-method lesson");

    let config_path = temporary_config_path();
    std::fs::write(&config_path, project_file("simulation.toml"))
        .expect("write tuning-method simulation config");
    let config = SimulationConfig::load(&config_path).expect("load five equivalent plant copies");
    assert_eq!(config.plant_system.plants.len(), 5);
    let mut simulation = SimulationController::new(config);
    let enabled = IoAddress::parse("%MX96.0").unwrap();
    let applied =
        ["%QL0", "%QL8", "%QL16", "%QL24", "%QL32"].map(|value| IoAddress::parse(value).unwrap());
    let requested = ["%ML128", "%ML136", "%ML144", "%ML152", "%ML160"]
        .map(|value| IoAddress::parse(value).unwrap());
    let measured =
        ["%IL0", "%IL8", "%IL16", "%IL24", "%IL32"].map(|value| IoAddress::parse(value).unwrap());
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial plant states");
    write_bool(&mut runtime, &enabled, false);
    let mut traces: [Vec<f64>; 5] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    let mut requests: [Vec<f64>; 5] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    let mut outputs: [Vec<f64>; 5] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    for cycle in 1usize..=2_200 {
        if cycle == 101 {
            write_bool(&mut runtime, &enabled, true);
        }
        runtime
            .execute_cycle()
            .expect("execute PI method candidates");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance method-comparison plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish method-comparison measurements");
        for index in 0..5 {
            traces[index].push(read_lreal(&runtime, &measured[index]));
            requests[index].push(read_lreal(&runtime, &requested[index]));
            outputs[index].push(read_lreal(&runtime, &applied[index]));
        }
    }
    let _ = std::fs::remove_file(config_path);

    for values in &traces {
        assert!(values.iter().all(|value| value.is_finite()));
    }
    for values in &outputs {
        assert!(values
            .iter()
            .all(|value| value.is_finite() && (0.0..=100.0).contains(value)));
    }
    let cases = [
        metrics(&traces[0], &outputs[0], 20.0, 25.0),
        metrics(&traces[1], &outputs[1], 20.0, 25.0),
        metrics(&traces[2], &outputs[2], 50.0, 52.0),
        metrics(&traces[3], &outputs[3], 40.0, 45.0),
        metrics(&traces[4], &outputs[4], 40.0, 45.0),
    ];
    println!("tuning-method comparison: {cases:#?}");
    assert!(cases
        .iter()
        .all(|item| item.final_mean_absolute_error <= 0.25));
    assert!(cases[..2]
        .iter()
        .chain(cases[3..].iter())
        .all(|item| item.setpoint_overshoot_percent <= 10.0));
    assert!(
        cases[2].setpoint_overshoot_percent * (52.0 - 50.0) / 100.0 <= 0.8,
        "tank peak excursion must satisfy its absolute level requirement: {cases:?}"
    );
    assert!(
        cases[1].iae < cases[0].iae,
        "heater AMIGO should reject error sooner: {cases:?}"
    );
    assert!(
        cases[1].travel > cases[0].travel,
        "heater AMIGO should move more: {cases:?}"
    );
    assert!(
        cases[4].iae > cases[3].iae,
        "dryer AMIGO should trade speed for robustness: {cases:?}"
    );
    assert!(
        cases[4].travel < cases[3].travel,
        "dryer AMIGO should move less: {cases:?}"
    );
    assert!(cases
        .iter()
        .all(|item| item.ise.is_finite() && item.itae.is_finite()));

    for forbidden in ["Winner", "ParetoRank", "PerformanceScore", "AcceptedMethod"] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds method-comparison result '{forbidden}'"
        );
    }

    let time = time_axis(2_200, 0.1);
    let heater_setpoint: Vec<f64> = (0..2_200)
        .map(|index| if index < 100 { 20.0 } else { 25.0 })
        .collect();
    let dryer_setpoint: Vec<f64> = (0..2_200)
        .map(|index| if index < 100 { 40.0 } else { 45.0 })
        .collect();
    let summary = format!(
        "case,plant,method,status,iae,ise,itae,overshoot_percent,final_mean_absolute_error,applied_movement_pp,applied\n\
heater_simc,water_heater,SIMC,qualified,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},false\n\
heater_amigo,water_heater,AMIGO,qualified,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},false\n\
tank_simc,buffer_tank,SIMC,qualified,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},false\n\
tank_amigo,buffer_tank,AMIGO,unavailable,,,,,,,false\n\
dryer_simc,belt_dryer,SIMC,qualified,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},false\n\
dryer_amigo,belt_dryer,AMIGO,qualified,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},false\n",
        cases[0].iae, cases[0].ise, cases[0].itae, cases[0].setpoint_overshoot_percent,
        cases[0].final_mean_absolute_error, cases[0].travel,
        cases[1].iae, cases[1].ise, cases[1].itae, cases[1].setpoint_overshoot_percent,
        cases[1].final_mean_absolute_error, cases[1].travel,
        cases[2].iae, cases[2].ise, cases[2].itae, cases[2].setpoint_overshoot_percent,
        cases[2].final_mean_absolute_error, cases[2].travel,
        cases[3].iae, cases[3].ise, cases[3].itae, cases[3].setpoint_overshoot_percent,
        cases[3].final_mean_absolute_error, cases[3].travel,
        cases[4].iae, cases[4].ise, cases[4].itae, cases[4].setpoint_overshoot_percent,
        cases[4].final_mean_absolute_error, cases[4].travel,
    );
    evidence::retain(
        &evidence_root("method-comparison"),
        "tuning-method-comparison-evidence",
        "One plan exposes different tuning trade-offs",
        "Equivalent plants separate method effects from plant and experiment changes.",
        &[
            evidence::Panel {
                title: "Water-heater response",
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
                x_minimum: 0.0,
                x_maximum: 220.0,
                y_minimum: 19.0,
                y_maximum: 26.0,
                series: vec![
                    evidence::Series {
                        name: "Heater setpoint",
                        color: "#111827",
                        dashed: true,
                        x: &time,
                        y: &heater_setpoint,
                    },
                    evidence::Series {
                        name: "Heater SIMC",
                        color: "#2673b8",
                        dashed: false,
                        x: &time,
                        y: &traces[0],
                    },
                    evidence::Series {
                        name: "Heater AMIGO",
                        color: "#b43c32",
                        dashed: false,
                        x: &time,
                        y: &traces[1],
                    },
                ],
            },
            evidence::Panel {
                title: "Belt-dryer response",
                x_unit: "time (s)",
                y_unit: "dryness (%)",
                x_minimum: 0.0,
                x_maximum: 220.0,
                y_minimum: 39.0,
                y_maximum: 46.0,
                series: vec![
                    evidence::Series {
                        name: "Dryer setpoint",
                        color: "#111827",
                        dashed: true,
                        x: &time,
                        y: &dryer_setpoint,
                    },
                    evidence::Series {
                        name: "Dryer SIMC",
                        color: "#2673b8",
                        dashed: false,
                        x: &time,
                        y: &traces[3],
                    },
                    evidence::Series {
                        name: "Dryer AMIGO",
                        color: "#b43c32",
                        dashed: false,
                        x: &time,
                        y: &traces[4],
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Buffer-tank response",
                name: "Tank SIMC level",
                x: &time,
                y: &traces[2],
                x_unit: "time (s)",
                y_unit: "level (%)",
            },
            evidence::RetainedSeries {
                panel: "Controller request",
                name: "Heater SIMC requested",
                x: &time,
                y: &requests[0],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Controller request",
                name: "Heater AMIGO requested",
                x: &time,
                y: &requests[1],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Controller request",
                name: "Tank SIMC requested",
                x: &time,
                y: &requests[2],
                x_unit: "time (s)",
                y_unit: "pump (%)",
            },
            evidence::RetainedSeries {
                panel: "Controller request",
                name: "Dryer SIMC requested",
                x: &time,
                y: &requests[3],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Controller request",
                name: "Dryer AMIGO requested",
                x: &time,
                y: &requests[4],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Applied output",
                name: "Heater SIMC applied",
                x: &time,
                y: &outputs[0],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Applied output",
                name: "Heater AMIGO applied",
                x: &time,
                y: &outputs[1],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Applied output",
                name: "Tank SIMC applied",
                x: &time,
                y: &outputs[2],
                x_unit: "time (s)",
                y_unit: "pump (%)",
            },
            evidence::RetainedSeries {
                panel: "Applied output",
                name: "Dryer SIMC applied",
                x: &time,
                y: &outputs[3],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Applied output",
                name: "Dryer AMIGO applied",
                x: &time,
                y: &outputs[4],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
        ],
        &summary,
    );
}

#[test]
fn control_curriculum_automatic_tune_validates_without_replacing_the_running_heater() {
    let lesson: serde_json::Value = serde_json::from_str(&automatic_project_file("lesson.json"))
        .expect("parse Automatic Tune lesson metadata");
    assert_eq!(lesson["sequence"], 52);
    assert_eq!(lesson["change"]["applied"], false);

    let shipped_source = automatic_project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("lesson program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX96.0 : BOOL :=",
        )
        .replace(
            "CurrentRequested : LREAL :=",
            "CurrentRequested AT %ML128 : LREAL :=",
        )
        .replace(
            "CandidateRequested : LREAL :=",
            "CandidateRequested AT %ML136 : LREAL :=",
        )
        .replace(
            "CurrentApplied : LREAL :=",
            "CurrentApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "CandidateApplied : LREAL :=",
            "CandidateApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "CurrentTemperature : LREAL :=",
            "CurrentTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "CandidateTemperature : LREAL :=",
            "CandidateTemperature AT %IL8 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("automatic-water-heater/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Automatic Tune heater lesson");

    let config_path = temporary_config_path();
    std::fs::write(&config_path, automatic_project_file("simulation.toml"))
        .expect("write Automatic Tune heater simulation config");
    let config = SimulationConfig::load(&config_path).expect("load equivalent heater copies");
    assert_eq!(config.plant_system.plants.len(), 2);
    let mut simulation = SimulationController::new(config);
    let enabled = IoAddress::parse("%MX96.0").unwrap();
    let applied = ["%QL0", "%QL8"].map(|value| IoAddress::parse(value).unwrap());
    let requested = ["%ML128", "%ML136"].map(|value| IoAddress::parse(value).unwrap());
    let measured = ["%IL0", "%IL8"].map(|value| IoAddress::parse(value).unwrap());
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heater states");
    write_bool(&mut runtime, &enabled, false);
    let mut traces: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    let mut requests: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    let mut outputs: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(2_200));
    for cycle in 1usize..=2_200 {
        if cycle == 101 {
            write_bool(&mut runtime, &enabled, true);
        }
        runtime
            .execute_cycle()
            .expect("execute current and candidate PI controllers");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance Automatic Tune heater copies");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish Automatic Tune heater measurements");
        for index in 0..2 {
            traces[index].push(read_lreal(&runtime, &measured[index]));
            requests[index].push(read_lreal(&runtime, &requested[index]));
            outputs[index].push(read_lreal(&runtime, &applied[index]));
        }
    }
    let _ = std::fs::remove_file(config_path);
    let current = metrics(&traces[0], &outputs[0], 20.0, 25.0);
    let candidate = metrics(&traces[1], &outputs[1], 20.0, 25.0);
    assert!(current.final_mean_absolute_error <= 0.25);
    assert!(candidate.final_mean_absolute_error <= 0.25);
    assert!(
        candidate.iae < current.iae,
        "Balanced candidate must improve accumulated error"
    );
    assert!(
        candidate.travel > current.travel,
        "faster candidate must expose its movement cost"
    );
    assert!(outputs
        .iter()
        .flatten()
        .all(|value| (0.0..=100.0).contains(value)));
    for forbidden in [
        "AutoTuneWinner",
        "StagePassed",
        "ModelFit",
        "AcceptedCandidate",
        "ApplyCandidate",
    ] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds orchestration result '{forbidden}'"
        );
    }
    println!("Automatic Tune validation current={current:#?}, candidate={candidate:#?}");

    let time = time_axis(2_200, 0.1);
    let setpoint: Vec<f64> = (0..2_200)
        .map(|index| if index < 100 { 20.0 } else { 25.0 })
        .collect();
    let summary = format!(
        "case,controller,status,iae,ise,itae,overshoot_percent,final_mean_absolute_error,applied_movement_pp,completed_stages,candidate_applied,active_controller_changed,evidence_class\n\
current,Robust_PI,running,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},Test|Identify|Tune|Validate,false,false,simulation\n\
candidate,Balanced_PI,validated_unapplied,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},Test|Identify|Tune|Validate,false,false,simulation\n",
        current.iae, current.ise, current.itae, current.setpoint_overshoot_percent,
        current.final_mean_absolute_error, current.travel,
        candidate.iae, candidate.ise, candidate.itae, candidate.setpoint_overshoot_percent,
        candidate.final_mean_absolute_error, candidate.travel,
    );
    evidence::retain(
        &evidence_root("automatic-water-heater"),
        "automatic-water-heater-evidence",
        "Automatic Tune validates without silent application",
        "The candidate improves accumulated error but remains explicitly unapplied.",
        &[
            evidence::Panel {
                title: "Heater temperature",
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
                x_minimum: 0.0,
                x_maximum: 220.0,
                y_minimum: 19.0,
                y_maximum: 26.0,
                series: vec![
                    evidence::Series {
                        name: "Setpoint",
                        color: "#111827",
                        dashed: true,
                        x: &time,
                        y: &setpoint,
                    },
                    evidence::Series {
                        name: "Running Robust PI",
                        color: "#2673b8",
                        dashed: false,
                        x: &time,
                        y: &traces[0],
                    },
                    evidence::Series {
                        name: "Unapplied Balanced PI",
                        color: "#b43c32",
                        dashed: false,
                        x: &time,
                        y: &traces[1],
                    },
                ],
            },
            evidence::Panel {
                title: "Applied heater command",
                x_unit: "time (s)",
                y_unit: "heater (%)",
                x_minimum: 0.0,
                x_maximum: 220.0,
                y_minimum: 0.0,
                y_maximum: 100.0,
                series: vec![
                    evidence::Series {
                        name: "Running applied",
                        color: "#2673b8",
                        dashed: false,
                        x: &time,
                        y: &outputs[0],
                    },
                    evidence::Series {
                        name: "Candidate applied in isolated validation",
                        color: "#b43c32",
                        dashed: false,
                        x: &time,
                        y: &outputs[1],
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Controller request",
                name: "Running requested",
                x: &time,
                y: &requests[0],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Controller request",
                name: "Candidate requested",
                x: &time,
                y: &requests[1],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
        ],
        &summary,
    );
}
