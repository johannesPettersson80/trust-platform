use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/57-compressor-override/figures")
}

fn time_axis(samples: usize) -> Vec<f64> {
    (1..=samples).map(|sample| sample as f64 * 0.1).collect()
}

fn plot_bounds<'a>(series: impl IntoIterator<Item = &'a [f64]>) -> (f64, f64) {
    let mut minimum = f64::INFINITY;
    let mut maximum = f64::NEG_INFINITY;
    for values in series {
        for value in values {
            minimum = minimum.min(*value);
            maximum = maximum.max(*value);
        }
    }
    let padding = ((maximum - minimum) * 0.08).max(0.01);
    (minimum - padding, maximum + padding)
}

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/57-compressor-override")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 57 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 57 file {}: {error}", path.display()))
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-override-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL input");
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL input");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime
        .io()
        .read(address)
        .expect("read LREAL lesson signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL lesson signal") {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

#[derive(Debug)]
struct OverrideRun {
    pressures: [Vec<f64>; 3],
    margins: [Vec<f64>; 3],
    valves: [Vec<f64>; 3],
    tracked_normal_demand: Vec<f64>,
    tracked_protect_demand: Vec<f64>,
    untracked_active: Vec<bool>,
    tracked_active: Vec<bool>,
    protection_available: Vec<bool>,
}

fn run_override(margin_valid: bool) -> OverrideRun {
    let source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("override lesson program before configuration")
        .replace("MarginValid : BOOL :=", "MarginValid AT %MX88.5 : BOOL :=")
        .replace(
            "PressureSetpoint : LREAL :=",
            "PressureSetpoint AT %ML192 : LREAL :=",
        )
        .replace(
            "TrackedNormalDemand : LREAL :=",
            "TrackedNormalDemand AT %ML200 : LREAL :=",
        )
        .replace(
            "TrackedProtectDemand : LREAL :=",
            "TrackedProtectDemand AT %ML208 : LREAL :=",
        )
        .replace(
            "BaselinePressure : LREAL :=",
            "BaselinePressure AT %IL88 : LREAL :=",
        )
        .replace(
            "BaselineMargin : LREAL :=",
            "BaselineMargin AT %IL96 : LREAL :=",
        )
        .replace(
            "UntrackedPressure : LREAL :=",
            "UntrackedPressure AT %IL104 : LREAL :=",
        )
        .replace(
            "UntrackedMargin : LREAL :=",
            "UntrackedMargin AT %IL112 : LREAL :=",
        )
        .replace(
            "TrackedPressure : LREAL :=",
            "TrackedPressure AT %IL120 : LREAL :=",
        )
        .replace(
            "TrackedMargin : LREAL :=",
            "TrackedMargin AT %IL128 : LREAL :=",
        )
        .replace(
            "BaselineValveRequest : LREAL :=",
            "BaselineValveRequest AT %QL128 : LREAL :=",
        )
        .replace(
            "UntrackedValveRequest : LREAL :=",
            "UntrackedValveRequest AT %QL136 : LREAL :=",
        )
        .replace(
            "TrackedValveRequest : LREAL :=",
            "TrackedValveRequest AT %QL144 : LREAL :=",
        )
        .replace(
            "UntrackedProtectionActive : BOOL :=",
            "UntrackedProtectionActive AT %MX88.6 : BOOL :=",
        )
        .replace(
            "TrackedProtectionActive : BOOL :=",
            "TrackedProtectionActive AT %MX88.7 : BOOL :=",
        )
        .replace(
            "TrackedProtectionAvailable : BOOL :=",
            "TrackedProtectionAvailable AT %MX89.0 : BOOL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("compressor-override/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped override lesson");
    let config_path = temporary_config_path();
    std::fs::write(&config_path, project_file("simulation.toml"))
        .expect("write compressor simulation config");
    let config = SimulationConfig::load(&config_path).expect("load three equivalent compressors");
    assert_eq!(config.plant_system.plants.len(), 3);
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial compressor measurements");

    let valid_address = IoAddress::parse("%MX88.5").unwrap();
    let setpoint_address = IoAddress::parse("%ML192").unwrap();
    let pressure_addresses =
        ["%IL88", "%IL104", "%IL120"].map(|address| IoAddress::parse(address).unwrap());
    let margin_addresses =
        ["%IL96", "%IL112", "%IL128"].map(|address| IoAddress::parse(address).unwrap());
    let valve_addresses =
        ["%QL128", "%QL136", "%QL144"].map(|address| IoAddress::parse(address).unwrap());
    let tracked_normal_address = IoAddress::parse("%ML200").unwrap();
    let tracked_protect_address = IoAddress::parse("%ML208").unwrap();
    let untracked_active_address = IoAddress::parse("%MX88.6").unwrap();
    let tracked_active_address = IoAddress::parse("%MX88.7").unwrap();
    let protection_available_address = IoAddress::parse("%MX89.0").unwrap();
    let mut result = OverrideRun {
        pressures: std::array::from_fn(|_| Vec::with_capacity(1_400)),
        margins: std::array::from_fn(|_| Vec::with_capacity(1_400)),
        valves: std::array::from_fn(|_| Vec::with_capacity(1_400)),
        tracked_normal_demand: Vec::with_capacity(1_400),
        tracked_protect_demand: Vec::with_capacity(1_400),
        untracked_active: Vec::with_capacity(1_400),
        tracked_active: Vec::with_capacity(1_400),
        protection_available: Vec::with_capacity(1_400),
    };
    for cycle in 1usize..=1_400 {
        write_bool(&mut runtime, &valid_address, margin_valid);
        write_lreal(
            &mut runtime,
            &setpoint_address,
            if cycle <= 200 || cycle > 800 {
                8.0
            } else {
                9.0
            },
        );
        runtime
            .execute_cycle()
            .expect("execute compressor override controllers");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance equivalent compressors");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish compressor measurements");
        for index in 0..3 {
            result.pressures[index].push(read_lreal(&runtime, &pressure_addresses[index]));
            result.margins[index].push(read_lreal(&runtime, &margin_addresses[index]));
            result.valves[index].push(read_lreal(&runtime, &valve_addresses[index]));
        }
        result
            .tracked_normal_demand
            .push(read_lreal(&runtime, &tracked_normal_address));
        result
            .tracked_protect_demand
            .push(read_lreal(&runtime, &tracked_protect_address));
        result
            .untracked_active
            .push(read_bool(&runtime, &untracked_active_address));
        result
            .tracked_active
            .push(read_bool(&runtime, &tracked_active_address));
        result
            .protection_available
            .push(read_bool(&runtime, &protection_available_address));
    }
    let _ = std::fs::remove_file(config_path);
    result
}

fn transitions(values: &[bool]) -> Vec<usize> {
    (1..values.len())
        .filter(|index| values[*index] != values[*index - 1])
        .collect()
}

#[test]
fn control_curriculum_override_tracks_inactive_compressor_controllers_and_releases_once() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 57 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 57 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 57);
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_override_tracks_inactive_compressor_controllers_and_releases_once"
    );
    assert_eq!(
        project_file("src/Main.st").matches("Control.PID;").count(),
        5
    );
    assert_eq!(
        project_file("simulation.toml")
            .matches("model = \"state_space\"")
            .count(),
        3
    );

    let run = run_override(true);
    for values in run
        .pressures
        .iter()
        .chain(run.margins.iter())
        .chain(run.valves.iter())
    {
        assert!(values.iter().all(|value| value.is_finite()));
    }
    assert!(run
        .valves
        .iter()
        .flatten()
        .all(|value| (0.0..=100.0).contains(value)));
    let minimum_margin = run.margins.each_ref().map(|values| {
        values[200..800]
            .iter()
            .copied()
            .fold(f64::INFINITY, f64::min)
    });
    let tracked_transitions = transitions(&run.tracked_active);
    let untracked_transitions = transitions(&run.untracked_active);
    let tracked_steps = tracked_transitions
        .iter()
        .map(|index| (run.valves[2][*index] - run.valves[2][index - 1]).abs())
        .collect::<Vec<_>>();
    let untracked_steps = untracked_transitions
        .iter()
        .map(|index| (run.valves[1][*index] - run.valves[1][index - 1]).abs())
        .collect::<Vec<_>>();
    println!(
        "compressor minimum margins={minimum_margin:?}, tracked transitions={tracked_transitions:?} steps={tracked_steps:?}, untracked transitions={untracked_transitions:?} steps={untracked_steps:?}"
    );
    assert!(
        minimum_margin[0] < 12.0,
        "normal pressure control must expose the constraint conflict"
    );
    assert!(
        minimum_margin[2] >= 11.5,
        "tracked override must protect the declared margin"
    );
    assert_eq!(
        tracked_transitions.len(),
        2,
        "tracked selector must take over and release exactly once"
    );
    assert!(tracked_steps.iter().all(|step| *step <= 1.0));
    assert!(
        minimum_margin[1] < minimum_margin[2]
            || untracked_steps.iter().copied().fold(0.0, f64::max) > 1.0,
        "untracked inactive state must expose late takeover or a transition bump"
    );

    let invalid = run_override(false);
    assert!(invalid
        .protection_available
        .iter()
        .all(|available| !available));
    assert!(invalid.tracked_active.iter().all(|active| !active));
    assert!(invalid.valves[2]
        .iter()
        .all(|value| (*value - 30.0).abs() <= 1e-12));

    let time = time_axis(run.pressures[0].len());
    let (pressure_min, pressure_max) = plot_bounds(run.pressures.iter().map(Vec::as_slice));
    let (margin_min, margin_max) = plot_bounds(run.margins.iter().map(Vec::as_slice));
    let summary = format!(
        "case,status,minimum_margin_percent,selector_transitions,maximum_takeover_step_percent\nnormal_only,rejected,{:.6},0,0\nuntracked_override,rejected,{:.6},{},{:.6}\ntracked_override,qualified,{:.6},{},{:.6}\ninvalid_margin_fallback,unprotected_metrics_ineligible,,,\n",
        minimum_margin[0], minimum_margin[1], untracked_transitions.len(),
        untracked_steps.iter().copied().fold(0.0, f64::max), minimum_margin[2],
        tracked_transitions.len(), tracked_steps.iter().copied().fold(0.0, f64::max),
    );
    let tracked_authority = run
        .tracked_active
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let protection_available = run
        .protection_available
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let invalid_available = invalid
        .protection_available
        .iter()
        .map(|value| f64::from(*value))
        .collect::<Vec<_>>();
    let tracking_error = run.valves[2]
        .iter()
        .zip(&run.tracked_normal_demand)
        .map(|(applied, inactive)| applied - inactive)
        .collect::<Vec<_>>();
    let invalid_time = time_axis(invalid.valves[2].len());
    evidence::retain(
        &evidence_root(),
        "compressor-override-evidence",
        "Override control protects the compressor constraint",
        "Tracking the inactive controllers prevents a bump when the protective selector takes authority.",
        &[
            evidence::Panel {
                title: "Discharge pressure", x_unit: "time (s)", y_unit: "pressure (bar)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(),
                y_minimum: pressure_min, y_maximum: pressure_max,
                series: vec![
                    evidence::Series { name: "Normal only", color: "#6b7280", dashed: false, x: &time, y: &run.pressures[0] },
                    evidence::Series { name: "Untracked override", color: "#dc2626", dashed: false, x: &time, y: &run.pressures[1] },
                    evidence::Series { name: "Tracked override", color: "#2563eb", dashed: false, x: &time, y: &run.pressures[2] },
                ],
            },
            evidence::Panel {
                title: "Protection margin", x_unit: "time (s)", y_unit: "margin (%)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(),
                y_minimum: margin_min, y_maximum: margin_max,
                series: vec![
                    evidence::Series { name: "Normal only", color: "#6b7280", dashed: false, x: &time, y: &run.margins[0] },
                    evidence::Series { name: "Untracked override", color: "#dc2626", dashed: false, x: &time, y: &run.margins[1] },
                    evidence::Series { name: "Tracked override", color: "#2563eb", dashed: false, x: &time, y: &run.margins[2] },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Controller candidates", name: "Normal demand", x: &time, y: &run.tracked_normal_demand, x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Controller candidates", name: "Protect demand", x: &time, y: &run.tracked_protect_demand, x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Final element", name: "Selected and applied valve", x: &time, y: &run.valves[2], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Authority", name: "Protect controller active", x: &time, y: &tracked_authority, x_unit: "time (s)", y_unit: "active (0/1)" },
            evidence::RetainedSeries { panel: "Authority", name: "Protection available", x: &time, y: &protection_available, x_unit: "time (s)", y_unit: "available (0/1)" },
            evidence::RetainedSeries { panel: "Tracking", name: "Inactive normal tracking error", x: &time, y: &tracking_error, x_unit: "time (s)", y_unit: "error (percentage points)" },
            evidence::RetainedSeries { panel: "Invalid margin quality", name: "Protection available", x: &invalid_time, y: &invalid_available, x_unit: "time (s)", y_unit: "available (0/1)" },
            evidence::RetainedSeries { panel: "Invalid margin quality", name: "Safe applied valve", x: &invalid_time, y: &invalid.valves[2], x_unit: "time (s)", y_unit: "valve (%)" },
        ],
        &summary,
    );
}
