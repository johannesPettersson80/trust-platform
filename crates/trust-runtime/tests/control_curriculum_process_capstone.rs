use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;
use evidence::{Panel, RetainedSeries, Series};

const DT: f64 = 0.1;
const CYCLES: usize = 2_200;

fn project_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/77-water-heater")
        .join(relative)
}

fn project_file(relative: &str) -> String {
    let path = project_path(relative);
    assert!(
        path.is_file(),
        "Lesson 77 executable water-heater capstone is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 77 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 77 program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX192.0 : BOOL :=",
        )
        .replace("ManualMode : BOOL :=", "ManualMode AT %IX192.1 : BOOL :=")
        .replace(
            "TemperatureQualityGood : BOOL :=",
            "TemperatureQualityGood AT %IX192.2 : BOOL :=",
        )
        .replace(
            "TemperatureSetpoint : LREAL :=",
            "TemperatureSetpoint AT %IL64 : LREAL :=",
        )
        .replace(
            "ManualOutput : LREAL :=",
            "ManualOutput AT %IL72 : LREAL :=",
        )
        .replace("CycleTime : LREAL :=", "CycleTime AT %IL80 : LREAL :=")
        .replace("PTemperature : LREAL :=", "PTemperature AT %IL0 : LREAL :=")
        .replace(
            "HistoricalTemperature : LREAL :=",
            "HistoricalTemperature AT %IL8 : LREAL :=",
        )
        .replace(
            "CurrentTemperature : LREAL :=",
            "CurrentTemperature AT %IL16 : LREAL :=",
        )
        .replace(
            "CandidateTemperature : LREAL :=",
            "CandidateTemperature AT %IL24 : LREAL :=",
        )
        .replace("PRequested : LREAL :=", "PRequested AT %ML88 : LREAL :=")
        .replace(
            "HistoricalRequested : LREAL :=",
            "HistoricalRequested AT %ML96 : LREAL :=",
        )
        .replace(
            "CurrentRequested : LREAL :=",
            "CurrentRequested AT %ML104 : LREAL :=",
        )
        .replace(
            "CandidateRequested : LREAL :=",
            "CandidateRequested AT %ML112 : LREAL :=",
        )
        .replace("PApplied : LREAL :=", "PApplied AT %QL0 : LREAL :=")
        .replace(
            "HistoricalApplied : LREAL :=",
            "HistoricalApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "CurrentApplied : LREAL :=",
            "CurrentApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "CandidateApplied : LREAL :=",
            "CandidateApplied AT %QL24 : LREAL :=",
        )
        .replace(
            "ControllerRevision : DINT :=",
            "ControllerRevision AT %MD120 : DINT :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "control/14-integrated-capstones/77-water-heater/src/Main.st",
            mapped_source(),
        ),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 77 Structured Text and Control library")
}

fn write_lreal(runtime: &mut Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).unwrap(),
            Value::LWord(value.to_bits()),
        )
        .unwrap();
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(&IoAddress::parse(address).unwrap(), Value::Bool(value))
        .unwrap();
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL at {address}, got {value:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &str) -> i32 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::DWord(value) => value as i32,
        value => panic!("expected DINT at {address}, got {value:?}"),
    }
}

#[derive(Clone, Debug)]
struct Sample {
    time: f64,
    setpoint: f64,
    temperatures: [f64; 4],
    requested: [f64; 4],
    applied: [f64; 4],
    manual: bool,
    quality: bool,
    revision: i32,
}

fn run_case(config_name: &str) -> Vec<Sample> {
    let mut runtime = build_runtime();
    let mut simulation = SimulationController::new(
        SimulationConfig::load(project_path(config_name))
            .unwrap_or_else(|error| panic!("load Lesson 77 {config_name}: {error}")),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heater temperatures");
    let mut trace = Vec::with_capacity(CYCLES);

    for cycle in 0..CYCLES {
        let time = cycle as f64 * DT;
        let setpoint = if (140.0..170.0).contains(&time) {
            45.0
        } else {
            35.0
        };
        let manual = (180.0..185.0).contains(&time);
        let quality = !(190.0..192.0).contains(&time);
        write_bool(&mut runtime, "%IX192.0", true);
        write_bool(&mut runtime, "%IX192.1", manual);
        write_bool(&mut runtime, "%IX192.2", quality);
        write_lreal(&mut runtime, "%IL64", setpoint);
        write_lreal(&mut runtime, "%IL72", 60.0);
        write_lreal(&mut runtime, "%IL80", DT);
        runtime
            .execute_cycle()
            .expect("execute Lesson 77 controller scan");

        let requested =
            ["%ML88", "%ML96", "%ML104", "%ML112"].map(|address| read_lreal(&runtime, address));
        let applied =
            ["%QL0", "%QL8", "%QL16", "%QL24"].map(|address| read_lreal(&runtime, address));
        assert!(requested
            .iter()
            .all(|value| value.is_finite() && (0.0..=100.0).contains(value)));
        assert!(applied
            .iter()
            .all(|value| value.is_finite() && (0.0..=100.0).contains(value)));
        if manual {
            assert!(applied.iter().all(|value| (*value - 60.0).abs() < 1.0e-9));
        }
        if !quality {
            assert!(applied.iter().all(|value| (*value - 60.0).abs() < 1.0e-9));
        }
        let revision = read_dint(&runtime, "%MD120");
        let now = Duration::from_millis(((cycle + 1) * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance heater copies");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish heater temperatures");
        trace.push(Sample {
            time,
            setpoint,
            temperatures: ["%IL0", "%IL8", "%IL16", "%IL24"]
                .map(|address| read_lreal(&runtime, address)),
            requested,
            applied,
            manual,
            quality,
            revision,
        });
    }
    trace
}

fn mean_absolute_error(trace: &[Sample], controller: usize, start: f64, end: f64) -> f64 {
    let eligible: Vec<_> = trace
        .iter()
        .filter(|sample| sample.time >= start && sample.time < end && sample.quality)
        .collect();
    eligible
        .iter()
        .map(|sample| (sample.setpoint - sample.temperatures[controller]).abs())
        .sum::<f64>()
        / eligible.len() as f64
}

fn iae(trace: &[Sample], controller: usize, start: f64, end: f64) -> f64 {
    trace
        .iter()
        .filter(|sample| sample.time >= start && sample.time < end && sample.quality)
        .map(|sample| (sample.setpoint - sample.temperatures[controller]).abs() * DT)
        .sum()
}

fn movement(trace: &[Sample], controller: usize, start: f64, end: f64) -> f64 {
    trace
        .windows(2)
        .filter(|pair| pair[1].time >= start && pair[1].time < end)
        .map(|pair| (pair[1].applied[controller] - pair[0].applied[controller]).abs())
        .sum()
}

fn identify_reference_fopdt() -> (f64, f64, f64) {
    let step_time = 10.0;
    let delay = 1.0;
    let time_constant = 5.0;
    let input_step = 10.0;
    let gain = 0.2;
    let samples: Vec<_> = (0..=400)
        .map(|index| {
            let time = index as f64 * DT;
            let output = if time <= step_time + delay {
                35.0
            } else {
                35.0 + gain
                    * input_step
                    * (1.0 - (-(time - step_time - delay) / time_constant).exp())
            };
            (time, output)
        })
        .collect();
    let final_change = samples.last().unwrap().1 - 35.0;
    let estimated_gain = final_change / input_step;
    let response_time = samples
        .iter()
        .find(|(_, output)| *output > 35.01)
        .unwrap()
        .0;
    let target = 35.0 + 0.632 * final_change;
    let sixty_three_time = samples
        .iter()
        .find(|(_, output)| *output >= target)
        .unwrap()
        .0;
    (
        estimated_gain,
        response_time - step_time,
        sixty_three_time - response_time,
    )
}

#[test]
fn control_curriculum_water_heater_capstone_commissions_unapplied_pi() {
    let root = project_path("");
    assert!(
        root.is_dir(),
        "Lesson 77 executable water-heater capstone is missing: {}",
        root.display()
    );
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 77 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 77);
    assert_eq!(lesson["prerequisites"][0], "control-capstone-cold-storage");
    assert_eq!(lesson["nextLesson"], "control-capstone-servo");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_water_heater_capstone_commissions_unapplied_pi"
    );
    for config in [
        "simulation.toml",
        "scenarios/gain-low.toml",
        "scenarios/gain-high.toml",
        "scenarios/delay-high.toml",
    ] {
        SimulationConfig::load(project_path(config))
            .unwrap_or_else(|error| panic!("{config} must be executable: {error}"));
    }

    let (identified_gain, identified_delay, identified_time_constant) = identify_reference_fopdt();
    assert!((identified_gain - 0.2).abs() < 0.001);
    assert!((identified_delay - 1.0).abs() <= 0.2);
    assert!((identified_time_constant - 5.0).abs() <= 0.3);
    let robust = (3.260_869_565_f64, 0.652_173_913_f64);
    let balanced = (5.769_230_769_f64, 1.153_846_154_f64);
    assert!((robust.0 - 15.0 / 4.6).abs() < 1.0e-8);
    assert!((robust.1 - robust.0 / 5.0).abs() < 1.0e-8);
    assert!((balanced.0 - 15.0 / 2.6).abs() < 1.0e-8);
    assert!((balanced.1 - balanced.0 / 5.0).abs() < 1.0e-8);

    let nominal = run_case("simulation.toml");
    let variants = [
        run_case("scenarios/gain-low.toml"),
        run_case("scenarios/gain-high.toml"),
        run_case("scenarios/delay-high.toml"),
    ];
    let p_load_error = mean_absolute_error(&nominal, 0, 130.0, 140.0);
    assert!(
        p_load_error > 1.0,
        "P-plus-bias load error {p_load_error:.4} degC"
    );
    for controller in 1..4 {
        let load_error = mean_absolute_error(&nominal, controller, 130.0, 140.0);
        assert!(
            load_error <= 0.25,
            "PI {controller} load error {load_error:.4} degC"
        );
    }
    let robust_iae = iae(&nominal, 2, 10.0, 140.0);
    let balanced_iae = iae(&nominal, 3, 10.0, 140.0);
    let robust_movement = movement(&nominal, 2, 10.0, 140.0);
    let balanced_movement = movement(&nominal, 3, 10.0, 140.0);
    assert!(nominal.iter().all(|sample| {
        sample
            .requested
            .iter()
            .zip(sample.applied.iter())
            .all(|(requested, applied)| (requested - applied).abs() < 1.0e-9)
    }));
    assert!(balanced_iae < robust_iae);
    assert!(balanced_movement > robust_movement);
    for (index, trace) in variants.iter().enumerate() {
        for controller in [2_usize, 3] {
            let final_error = mean_absolute_error(trace, controller, 130.0, 140.0);
            assert!(
                final_error <= 0.30,
                "variant {index}, PI {controller}: {final_error:.4}"
            );
        }
    }
    assert!(nominal.iter().all(|sample| sample.revision == 77));
    assert!(nominal.iter().any(|sample| sample.manual));
    let current_before_auto = nominal
        .iter()
        .find(|sample| sample.time >= 184.9)
        .unwrap()
        .applied[2];
    let current_after_auto = nominal
        .iter()
        .find(|sample| sample.time >= 185.0)
        .unwrap()
        .applied[2];
    assert!((current_after_auto - current_before_auto).abs() <= 5.0);

    let time: Vec<f64> = nominal.iter().map(|sample| sample.time).collect();
    let setpoint: Vec<f64> = nominal.iter().map(|sample| sample.setpoint).collect();
    let robust_temperature: Vec<f64> = nominal
        .iter()
        .map(|sample| sample.temperatures[2])
        .collect();
    let balanced_temperature: Vec<f64> = nominal
        .iter()
        .map(|sample| sample.temperatures[3])
        .collect();
    let robust_output: Vec<f64> = nominal.iter().map(|sample| sample.applied[2]).collect();
    let balanced_output: Vec<f64> = nominal.iter().map(|sample| sample.applied[3]).collect();
    let design = [0.0, 1.0];
    let design_iae = [robust_iae, balanced_iae];
    let design_movement = [robust_movement, balanced_movement];
    let mut summary = String::from("design,iae_degC_s,applied_movement_pp,recommendation\n");
    writeln!(
        summary,
        "robust,{robust_iae:.9},{robust_movement:.9},selected-unapplied"
    )
    .unwrap();
    writeln!(
        summary,
        "balanced,{balanced_iae:.9},{balanced_movement:.9},faster-alternative"
    )
    .unwrap();
    evidence::retain(
        &root.join("figures"),
        "water-heater-capstone-evidence",
        "Both PI designs remove offset; the robust design uses less heater movement",
        "Common Simulation record with requested and applied output retained separately; the recommendation remains unapplied.",
        &[
            Panel {
                title: "Water temperature",
                x_unit: "time (s)",
                y_unit: "degC",
                x_minimum: 0.0,
                x_maximum: 220.0,
                y_minimum: 30.0,
                y_maximum: 55.0,
                series: vec![
                    Series { name: "Setpoint", color: "#64748b", dashed: true, x: &time, y: &setpoint },
                    Series { name: "Robust PI", color: "#2563eb", dashed: false, x: &time, y: &robust_temperature },
                    Series { name: "Balanced PI", color: "#dc2626", dashed: false, x: &time, y: &balanced_temperature },
                ],
            },
            Panel {
                title: "Applied heater power",
                x_unit: "time (s)",
                y_unit: "%",
                x_minimum: 0.0,
                x_maximum: 220.0,
                y_minimum: 0.0,
                y_maximum: 100.0,
                series: vec![
                    Series { name: "Robust PI", color: "#2563eb", dashed: false, x: &time, y: &robust_output },
                    Series { name: "Balanced PI", color: "#dc2626", dashed: false, x: &time, y: &balanced_output },
                ],
            },
        ],
        &[
            RetainedSeries { panel: "Design comparison", name: "IAE", x: &design, y: &design_iae, x_unit: "design index", y_unit: "degC*s" },
            RetainedSeries { panel: "Design comparison", name: "Applied movement", x: &design, y: &design_movement, x_unit: "design index", y_unit: "percentage points" },
        ],
        &summary,
    );

    println!(
        "water-heater capstone: identified K/T/L={identified_gain:.4}/{identified_time_constant:.2}/{identified_delay:.2}; P load error={p_load_error:.3} degC; robust/balanced IAE={robust_iae:.2}/{balanced_iae:.2} degC*s; movement={robust_movement:.1}/{balanced_movement:.1} pp; recommendation remains unapplied"
    );
}
