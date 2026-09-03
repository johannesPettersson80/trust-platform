use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;
use evidence::{Panel, RetainedSeries, Series};

const DT: f64 = 0.1;
const CYCLES: usize = 1_400;

fn lesson_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/76-cold-storage")
}

fn lesson_file(relative: &str) -> String {
    let path = lesson_root().join(relative);
    assert!(
        path.is_file(),
        "Lesson 76 executable cold-storage capstone is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 76 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    lesson_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 76 program before configuration")
        .replace("AutoMode : BOOL :=", "AutoMode AT %IX192.0 : BOOL :=")
        .replace(
            "TemperatureQualityGood : BOOL :=",
            "TemperatureQualityGood AT %IX192.1 : BOOL :=",
        )
        .replace(
            "StarterReady : BOOL :=",
            "StarterReady AT %IX192.2 : BOOL :=",
        )
        .replace(
            "CompressorRunning : LREAL :=",
            "CompressorRunning AT %IL8 : LREAL :=",
        )
        .replace(
            "CoolingRequested : BOOL :=",
            "CoolingRequested AT %MX200.0 : BOOL :=",
        )
        .replace(
            "DwellInhibit : BOOL :=",
            "DwellInhibit AT %MX200.1 : BOOL :=",
        )
        .replace(
            "QualityFaultLatched : BOOL :=",
            "QualityFaultLatched AT %MX200.2 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL64 : LREAL :=")
        .replace(
            "HysteresisWidth : LREAL :=",
            "HysteresisWidth AT %IL72 : LREAL :=",
        )
        .replace(
            "MinimumOnTime : LREAL :=",
            "MinimumOnTime AT %IL80 : LREAL :=",
        )
        .replace(
            "MinimumOffTime : LREAL :=",
            "MinimumOffTime AT %IL88 : LREAL :=",
        )
        .replace(
            "MeasuredCycleTime : LREAL :=",
            "MeasuredCycleTime AT %IL96 : LREAL :=",
        )
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "CompressorCommand : LREAL :=",
            "CompressorCommand AT %QL0 : LREAL :=",
        )
        .replace(
            "DwellElapsed : LREAL :=",
            "DwellElapsed AT %ML104 : LREAL :=",
        )
        .replace("SwitchCount : DINT :=", "SwitchCount AT %MD112 : DINT :=")
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![SourceFile::with_path(
        "control/14-integrated-capstones/76-cold-storage/src/Main.st",
        mapped_source(),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 76 Structured Text")
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

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL at {address}, got {value:?}"),
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Sample {
    time: f64,
    pv: f64,
    requested: bool,
    command: f64,
    running: bool,
    auto: bool,
    quality: bool,
    starter_ready: bool,
    dwell: f64,
}

fn run_candidate(hysteresis: f64, minimum_on: f64, minimum_off: f64, faults: bool) -> Vec<Sample> {
    let config_path = lesson_root().join("simulation.toml");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load shipped cold-room Simulation config"),
    );
    let mut runtime = build_runtime();
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial cold-room temperature");
    let mut running = false;
    let mut trace = Vec::with_capacity(CYCLES);

    for cycle in 0..CYCLES {
        if faults && cycle == 1_000 {
            runtime = build_runtime();
            simulation
                .apply_pre_cycle(Duration::from_millis(100_000), &mut runtime)
                .expect("publish live room state after controller restart");
            running = false;
        }
        let time = cycle as f64 * DT;
        let starter_ready = !faults || !(1_000..1_050).contains(&cycle);
        let quality = !faults || !(1_100..1_120).contains(&cycle);
        let auto =
            !faults || (!(1_120..1_130).contains(&cycle) && !(1_200..1_230).contains(&cycle));

        write_bool(&mut runtime, "%IX192.0", auto);
        write_bool(&mut runtime, "%IX192.1", quality);
        write_bool(&mut runtime, "%IX192.2", starter_ready);
        write_lreal(&mut runtime, "%IL8", if running { 100.0 } else { 0.0 });
        write_lreal(&mut runtime, "%IL64", 2.0);
        write_lreal(&mut runtime, "%IL72", hysteresis);
        write_lreal(&mut runtime, "%IL80", minimum_on);
        write_lreal(&mut runtime, "%IL88", minimum_off);
        write_lreal(&mut runtime, "%IL96", DT);
        runtime
            .execute_cycle()
            .expect("execute Lesson 76 thermostat scan");

        let command = read_lreal(&runtime, "%QL0");
        let requested = read_bool(&runtime, "%MX200.0");
        assert!(
            command == 0.0 || command == 100.0,
            "compressor command must be discrete"
        );
        if !auto || !quality || !starter_ready {
            assert_eq!(
                command, 0.0,
                "ineligible operation must command the safe state"
            );
        }
        running = command == 100.0;
        let now = Duration::from_millis(((cycle + 1) * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance shipped cold-room Simulation plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish cold-room measurement");
        trace.push(Sample {
            time,
            pv: read_lreal(&runtime, "%IL0"),
            requested,
            command,
            running,
            auto,
            quality,
            starter_ready,
            dwell: read_lreal(&runtime, "%ML104"),
        });
    }
    trace
}

fn transitions(trace: &[Sample], window: impl Fn(&Sample) -> bool) -> usize {
    trace
        .windows(2)
        .filter(|pair| window(&pair[1]) && pair[0].running != pair[1].running)
        .count()
}

fn minimum_completed_duration(trace: &[Sample], state: bool) -> f64 {
    let mut change_times = Vec::new();
    for pair in trace.windows(2) {
        if pair[0].running != pair[1].running && pair[1].time < 100.0 {
            change_times.push((pair[1].time, pair[1].running));
        }
    }
    change_times
        .windows(2)
        .filter(|pair| pair[0].1 == state)
        .map(|pair| pair[1].0 - pair[0].0)
        .fold(f64::INFINITY, f64::min)
}

fn assert_project_contract(root: &Path) {
    let lesson: serde_json::Value =
        serde_json::from_str(&lesson_file("lesson.json")).expect("parse Lesson 76 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 76);
    assert_eq!(lesson["prerequisites"][0], "control-loop-diagnosis");
    assert_eq!(lesson["nextLesson"], "control-capstone-water-heater");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_cold_storage_capstone_selects_safe_discrete_control"
    );
    assert!(root.join("simulation.toml").is_file());
    let source = lesson_file("src/Main.st");
    assert!(!source.contains("Control.PID") && !source.contains("PIDParameters"));
}

#[test]
fn control_curriculum_cold_storage_capstone_selects_safe_discrete_control() {
    let root = lesson_root();
    assert!(
        root.is_dir(),
        "Lesson 76 executable cold-storage capstone is missing: {}",
        root.display()
    );
    assert_project_contract(&root);

    let single_threshold = run_candidate(0.0, 0.0, 0.0, false);
    let hysteresis_only = run_candidate(1.0, 0.0, 0.0, false);
    let selected = run_candidate(1.0, 5.0, 5.0, true);
    let repeated = run_candidate(1.0, 5.0, 5.0, true);

    let quiet = |sample: &Sample| sample.time >= 20.0 && sample.time < 60.0;
    let single_switches = transitions(&single_threshold, quiet);
    let hysteresis_switches = transitions(&hysteresis_only, quiet);
    let selected_switches = transitions(&selected, quiet);
    assert!(
        single_switches > hysteresis_switches,
        "single threshold must expose more noise crossings: single={single_switches}, hysteresis={hysteresis_switches}"
    );
    assert!(selected_switches <= hysteresis_switches);

    let minimum_on = minimum_completed_duration(&selected, true);
    let minimum_off = minimum_completed_duration(&selected, false);
    assert!(
        minimum_on >= 5.0 - DT,
        "minimum on duration {minimum_on:.3} s"
    );
    assert!(
        minimum_off >= 5.0 - DT,
        "minimum off duration {minimum_off:.3} s"
    );
    let door_peak = selected
        .iter()
        .filter(|sample| (60.0..90.0).contains(&sample.time) && sample.quality)
        .map(|sample| sample.pv)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        door_peak <= 3.0,
        "door-event peak {door_peak:.4} degC exceeds product band"
    );
    assert!(selected
        .iter()
        .filter(|sample| (100.0..105.0).contains(&sample.time))
        .all(|sample| !sample.starter_ready && sample.command == 0.0));
    assert!(selected
        .iter()
        .filter(|sample| (110.0..113.0).contains(&sample.time))
        .all(|sample| !sample.quality || !sample.auto || sample.command == 0.0));
    assert!(selected
        .iter()
        .filter(|sample| (120.0..123.0).contains(&sample.time))
        .all(|sample| !sample.auto && sample.command == 0.0));
    assert_eq!(
        selected, repeated,
        "fixed seed and schedule must replay exactly"
    );

    let time: Vec<f64> = selected.iter().map(|sample| sample.time).collect();
    let temperature: Vec<f64> = selected.iter().map(|sample| sample.pv).collect();
    let setpoint = vec![2.0; selected.len()];
    let lower_product_limit = vec![1.0; selected.len()];
    let switch_off_threshold = vec![1.5; selected.len()];
    let switch_on_threshold = vec![2.5; selected.len()];
    let upper_product_limit = vec![3.0; selected.len()];
    let requested_command: Vec<f64> = selected
        .iter()
        .map(|sample| if sample.requested { 100.0 } else { 0.0 })
        .collect();
    let command: Vec<f64> = selected.iter().map(|sample| sample.command).collect();
    let candidate = [0.0, 1.0, 2.0];
    let transition_counts = [
        single_switches as f64,
        hysteresis_switches as f64,
        selected_switches as f64,
    ];
    let mut summary = String::from(
        "candidate,quiet_transitions,minimum_on_s,minimum_off_s,door_peak_degC,verdict\n",
    );
    writeln!(
        summary,
        "single-threshold,{single_switches},,,,reject-noise-switching"
    )
    .unwrap();
    writeln!(
        summary,
        "hysteresis-only,{hysteresis_switches},,,,eligible-without-dwell"
    )
    .unwrap();
    writeln!(summary, "hysteresis-plus-dwell,{selected_switches},{minimum_on:.6},{minimum_off:.6},{door_peak:.6},selected").unwrap();
    evidence::retain(
        &root.join("figures"),
        "cold-storage-capstone-evidence",
        "Hysteresis and dwell protect the cold-room compressor",
        "The upper panel shows the product band and 1.5/2.5 degC switching thresholds; the lower panel separates requested demand from the applied compressor state.",
        &[
            Panel {
                title: "Selected cold-room temperature",
                x_unit: "time (s)",
                y_unit: "degC",
                x_minimum: 0.0,
                x_maximum: 140.0,
                y_minimum: 0.0,
                y_maximum: 4.0,
                series: vec![
                    Series { name: "PV", color: "#2563eb", dashed: false, x: &time, y: &temperature },
                    Series { name: "SP", color: "#64748b", dashed: true, x: &time, y: &setpoint },
                    Series { name: "Lower limit", color: "#dc2626", dashed: true, x: &time, y: &lower_product_limit },
                    Series { name: "Off 1.5", color: "#7c3aed", dashed: true, x: &time, y: &switch_off_threshold },
                    Series { name: "On 2.5", color: "#d97706", dashed: true, x: &time, y: &switch_on_threshold },
                    Series { name: "Upper limit", color: "#dc2626", dashed: true, x: &time, y: &upper_product_limit },
                ],
            },
            Panel {
                title: "Requested (red dashed) and applied (green) compressor command",
                x_unit: "time (s)",
                y_unit: "%",
                x_minimum: 0.0,
                x_maximum: 140.0,
                y_minimum: 0.0,
                y_maximum: 105.0,
                series: vec![
                    Series { name: "Requested command", color: "#dc2626", dashed: true, x: &time, y: &requested_command },
                    Series { name: "Applied command", color: "#16a34a", dashed: false, x: &time, y: &command },
                ],
            },
        ],
        &[RetainedSeries {
            panel: "Candidate comparison",
            name: "Quiet-window transitions",
            x: &candidate,
            y: &transition_counts,
            x_unit: "candidate index",
            y_unit: "count",
        }],
        &summary,
    );

    println!(
        "cold-storage capstone: quiet transitions single/hysteresis/selected={single_switches}/{hysteresis_switches}/{selected_switches}; minimum on/off={minimum_on:.1}/{minimum_off:.1} s; door peak={door_peak:.3} degC"
    );
}
