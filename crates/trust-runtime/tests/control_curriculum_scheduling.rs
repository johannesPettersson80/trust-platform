use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const ADVANCED_CONTROL_SOURCE: &str =
    include_str!("../../../libraries/control/src/advanced_control.st");

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/58-gravity-tank-scheduling/figures")
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
    let padding = ((maximum - minimum) * 0.08).max(0.0001);
    (minimum - padding, maximum + padding)
}

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/58-gravity-tank-scheduling")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 58 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 58 file {}: {error}", path.display()))
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-scheduling-{}-{stamp}.toml",
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

#[derive(Debug)]
struct SchedulingRun {
    levels: [Vec<f64>; 3],
    pumps: [Vec<f64>; 3],
    active_kp: Vec<f64>,
    active_ki: Vec<f64>,
    parallel_region: Vec<f64>,
    transition_count: Vec<f64>,
}

fn run_scheduling(available: bool) -> SchedulingRun {
    let source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("scheduling lesson program before configuration")
        .replace(
            "SchedulingAvailable : BOOL :=",
            "SchedulingAvailable AT %MX89.1 : BOOL :=",
        )
        .replace(
            "LevelSetpoint : LREAL :=",
            "LevelSetpoint AT %ML216 : LREAL :=",
        )
        .replace(
            "FilteredSchedulingLevel : LREAL :=",
            "FilteredSchedulingLevel AT %ML224 : LREAL :=",
        )
        .replace(
            "InterpolationFraction : LREAL :=",
            "InterpolationFraction AT %ML232 : LREAL :=",
        )
        .replace("ActiveKp : LREAL :=", "ActiveKp AT %ML240 : LREAL :=")
        .replace("ActiveKi : LREAL :=", "ActiveKi AT %ML248 : LREAL :=")
        .replace(
            "ParallelRegionValue : LREAL :=",
            "ParallelRegionValue AT %ML256 : LREAL :=",
        )
        .replace(
            "ParallelTransitionCount : LREAL :=",
            "ParallelTransitionCount AT %ML264 : LREAL :=",
        )
        .replace("FixedLevel : LREAL :=", "FixedLevel AT %IL136 : LREAL :=")
        .replace(
            "InterpolatedLevel : LREAL :=",
            "InterpolatedLevel AT %IL144 : LREAL :=",
        )
        .replace(
            "ParallelLevel : LREAL :=",
            "ParallelLevel AT %IL152 : LREAL :=",
        )
        .replace(
            "FixedPumpRequest : LREAL :=",
            "FixedPumpRequest AT %QL152 : LREAL :=",
        )
        .replace(
            "InterpolatedPumpRequest : LREAL :=",
            "InterpolatedPumpRequest AT %QL160 : LREAL :=",
        )
        .replace(
            "ParallelPumpRequest : LREAL :=",
            "ParallelPumpRequest AT %QL168 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("Control/advanced_control.st", ADVANCED_CONTROL_SOURCE),
        SourceFile::with_path(
            "gravity-tank-scheduling/src/GainScheduledTankController.st",
            project_file("src/GainScheduledTankController.st"),
        ),
        SourceFile::with_path("gravity-tank-scheduling/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped gravity-tank scheduling lesson");
    let config_path = temporary_config_path();
    std::fs::write(&config_path, project_file("simulation.toml"))
        .expect("write scheduling simulation config");
    let config = SimulationConfig::load(&config_path).expect("load three nonlinear gravity tanks");
    assert_eq!(config.plant_system.plants.len(), 3);
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial tank levels");

    let available_address = IoAddress::parse("%MX89.1").unwrap();
    let setpoint_address = IoAddress::parse("%ML216").unwrap();
    let level_addresses =
        ["%IL136", "%IL144", "%IL152"].map(|address| IoAddress::parse(address).unwrap());
    let pump_addresses =
        ["%QL152", "%QL160", "%QL168"].map(|address| IoAddress::parse(address).unwrap());
    let kp_address = IoAddress::parse("%ML240").unwrap();
    let ki_address = IoAddress::parse("%ML248").unwrap();
    let region_address = IoAddress::parse("%ML256").unwrap();
    let count_address = IoAddress::parse("%ML264").unwrap();
    let mut result = SchedulingRun {
        levels: std::array::from_fn(|_| Vec::with_capacity(4_200)),
        pumps: std::array::from_fn(|_| Vec::with_capacity(4_200)),
        active_kp: Vec::with_capacity(4_200),
        active_ki: Vec::with_capacity(4_200),
        parallel_region: Vec::with_capacity(4_200),
        transition_count: Vec::with_capacity(4_200),
    };
    for cycle in 1usize..=4_200 {
        let setpoint = if cycle <= 300 {
            0.30
        } else if cycle <= 1_200 {
            0.55
        } else if cycle <= 2_400 {
            0.80
        } else if cycle <= 3_300 {
            0.55
        } else {
            0.30
        };
        write_bool(&mut runtime, &available_address, available);
        write_lreal(&mut runtime, &setpoint_address, setpoint);
        runtime
            .execute_cycle()
            .expect("execute fixed and scheduled tank controllers");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance nonlinear gravity tanks");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish tank levels");
        for index in 0..3 {
            result.levels[index].push(read_lreal(&runtime, &level_addresses[index]));
            result.pumps[index].push(read_lreal(&runtime, &pump_addresses[index]));
        }
        result.active_kp.push(read_lreal(&runtime, &kp_address));
        result.active_ki.push(read_lreal(&runtime, &ki_address));
        result
            .parallel_region
            .push(read_lreal(&runtime, &region_address));
        result
            .transition_count
            .push(read_lreal(&runtime, &count_address));
    }
    let _ = std::fs::remove_file(config_path);
    result
}

fn regional_iae(values: &[f64]) -> [f64; 4] {
    [
        (300, 1_200, 0.55),
        (1_200, 2_400, 0.80),
        (2_400, 3_300, 0.55),
        (3_300, 4_200, 0.30),
    ]
    .map(|(start, end, target)| {
        values[start..end]
            .iter()
            .map(|value| (target - value).abs() * 0.1)
            .sum()
    })
}

#[test]
fn control_curriculum_gain_scheduling_crosses_gravity_tank_regions_without_chatter_or_bumps() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 58 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 58 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 58);
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_gain_scheduling_crosses_gravity_tank_regions_without_chatter_or_bumps"
    );
    let controller = project_file("src/GainScheduledTankController.st");
    assert_eq!(controller.matches("Control.PID;").count(), 5);
    assert_eq!(controller.matches("Control.PIDGainScheduler;").count(), 1);
    assert_eq!(
        project_file("simulation.toml")
            .matches("model = \"gravity_tank\"")
            .count(),
        3
    );

    let run = run_scheduling(true);
    for values in run.levels.iter().chain(run.pumps.iter()) {
        assert!(values.iter().all(|value| value.is_finite()));
    }
    assert!(run
        .levels
        .iter()
        .flatten()
        .all(|value| (0.20..=0.90).contains(value)));
    assert!(run
        .pumps
        .iter()
        .flatten()
        .all(|value| (0.0..=0.060).contains(value)));
    assert!(run
        .active_kp
        .iter()
        .all(|value| (*value - 0.100).abs() <= 1e-12));
    assert!(run
        .active_ki
        .iter()
        .all(|value| (0.00280..=0.00456).contains(value)));
    assert_eq!(*run.transition_count.last().unwrap(), 4.0);
    assert!(run
        .parallel_region
        .iter()
        .all(|value| [0.0, 1.0, 2.0].contains(value)));

    let iae = run.levels.each_ref().map(|values| regional_iae(values));
    let worst = iae.map(|regions| regions.into_iter().fold(0.0, f64::max));
    let transition_indices = (1..run.parallel_region.len())
        .filter(|index| run.parallel_region[*index] != run.parallel_region[index - 1])
        .collect::<Vec<_>>();
    let transition_steps = transition_indices
        .iter()
        .map(|index| (run.pumps[2][*index] - run.pumps[2][index - 1]).abs())
        .collect::<Vec<_>>();
    println!("gravity tank regional IAE={iae:?}, worst={worst:?}, transitions={transition_indices:?}, pump steps={transition_steps:?}");
    assert!(worst[1] < worst[0] && worst[2] < worst[0]);
    assert_eq!(transition_indices.len(), 4);
    assert!(transition_steps.iter().all(|step| *step <= 0.0005));

    let unavailable = run_scheduling(false);
    assert!(unavailable
        .pumps
        .iter()
        .flatten()
        .all(|value| value.abs() <= 1e-12));

    let time = time_axis(run.levels[0].len());
    let (level_min, level_max) = plot_bounds(run.levels.iter().map(Vec::as_slice));
    let (pump_min, pump_max) = plot_bounds(run.pumps.iter().map(Vec::as_slice));
    let summary = format!(
        "case,status,worst_regional_iae_m_s,schedule_transitions,maximum_transition_step_m3_s\nfixed_gain,rejected,{:.6},0,0\ninterpolated_gain,qualified,{:.6},0,0\nparallel_bumpless,qualified,{:.6},{},{:.6}\nunavailable_authority,unavailable_metrics_ineligible,,,\n",
        worst[0], worst[1], worst[2], transition_indices.len(),
        transition_steps.iter().copied().fold(0.0, f64::max),
    );
    let unavailable_time = time_axis(unavailable.levels[0].len());
    let schedule_available = vec![1.0; time.len()];
    let schedule_unavailable = vec![0.0; unavailable_time.len()];
    evidence::retain(
        &evidence_root(),
        "gravity-tank-scheduling-evidence",
        "Gain scheduling follows the nonlinear gravity tank",
        "Scheduled gains improve the response across low, middle, and high operating regions without transition bumps.",
        &[
            evidence::Panel {
                title: "Tank level", x_unit: "time (s)", y_unit: "level (m)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(),
                y_minimum: level_min, y_maximum: level_max,
                series: vec![
                    evidence::Series { name: "Fixed gain", color: "#6b7280", dashed: false, x: &time, y: &run.levels[0] },
                    evidence::Series { name: "Interpolated gain", color: "#059669", dashed: false, x: &time, y: &run.levels[1] },
                    evidence::Series { name: "Parallel bumpless", color: "#2563eb", dashed: false, x: &time, y: &run.levels[2] },
                ],
            },
            evidence::Panel {
                title: "Pump flow", x_unit: "time (s)", y_unit: "flow (m3/s)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(),
                y_minimum: pump_min, y_maximum: pump_max,
                series: vec![
                    evidence::Series { name: "Fixed gain", color: "#6b7280", dashed: false, x: &time, y: &run.pumps[0] },
                    evidence::Series { name: "Interpolated gain", color: "#059669", dashed: false, x: &time, y: &run.pumps[1] },
                    evidence::Series { name: "Parallel bumpless", color: "#2563eb", dashed: false, x: &time, y: &run.pumps[2] },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Applied pump", name: "Parallel scheduled applied flow", x: &time, y: &run.pumps[2], x_unit: "time (s)", y_unit: "flow (m3/s)" },
            evidence::RetainedSeries { panel: "Active schedule", name: "Integral gain", x: &time, y: &run.active_ki, x_unit: "time (s)", y_unit: "1/s" },
            evidence::RetainedSeries { panel: "Active schedule", name: "Region", x: &time, y: &run.parallel_region, x_unit: "time (s)", y_unit: "region index" },
            evidence::RetainedSeries { panel: "Active schedule", name: "Schedule valid", x: &time, y: &schedule_available, x_unit: "time (s)", y_unit: "valid (0/1)" },
            evidence::RetainedSeries { panel: "Unavailable schedule", name: "Schedule valid", x: &unavailable_time, y: &schedule_unavailable, x_unit: "time (s)", y_unit: "valid (0/1)" },
            evidence::RetainedSeries { panel: "Unavailable schedule", name: "Tracked pump request", x: &unavailable_time, y: &unavailable.pumps[2], x_unit: "time (s)", y_unit: "flow (m3/s)" },
        ],
        &summary,
    );
}
