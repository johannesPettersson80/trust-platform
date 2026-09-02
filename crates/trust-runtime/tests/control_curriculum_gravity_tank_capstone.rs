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
const NOMINAL_CYCLES: usize = 4_200;

fn project_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/80-gravity-tank")
        .join(relative)
}

fn project_file(relative: &str) -> String {
    let path = project_path(relative);
    assert!(
        path.is_file(),
        "Lesson 80 executable gravity-tank capstone is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 80 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 80 program before configuration")
        .replace(
            "\n    OperatingMode : DINT :=",
            "\n    OperatingMode AT %ID192 : DINT :=",
        )
        .replace(
            "\n    LevelSetpoint : LREAL :=",
            "\n    LevelSetpoint AT %IL200 : LREAL :=",
        )
        .replace(
            "\n    ManualPumpFlow : LREAL :=",
            "\n    ManualPumpFlow AT %IL208 : LREAL :=",
        )
        .replace(
            "\n    CycleTime : LREAL :=",
            "\n    CycleTime AT %IL216 : LREAL :=",
        )
        .replace(
            "\n    LevelQualityGood : BOOL :=",
            "\n    LevelQualityGood AT %IX240.0 : BOOL :=",
        )
        .replace(
            "\n    SchedulingAvailable : BOOL :=",
            "\n    SchedulingAvailable AT %IX240.1 : BOOL :=",
        )
        .replace(
            "\n    PumpAvailable : BOOL :=",
            "\n    PumpAvailable AT %IX240.2 : BOOL :=",
        )
        .replace(
            "FixedMeasuredLevel : LREAL :=",
            "FixedMeasuredLevel AT %IL0 : LREAL :=",
        )
        .replace(
            "ScheduledMeasuredLevel : LREAL :=",
            "ScheduledMeasuredLevel AT %IL8 : LREAL :=",
        )
        .replace(
            "FixedRequestedPump : LREAL :=",
            "FixedRequestedPump AT %ML0 : LREAL :=",
        )
        .replace(
            "ScheduledRequestedPump : LREAL :=",
            "ScheduledRequestedPump AT %ML8 : LREAL :=",
        )
        .replace(
            "ScheduledFilteredLevel : LREAL :=",
            "ScheduledFilteredLevel AT %ML16 : LREAL :=",
        )
        .replace(
            "ScheduledFraction : LREAL :=",
            "ScheduledFraction AT %ML24 : LREAL :=",
        )
        .replace(
            "ScheduledActiveKp : LREAL :=",
            "ScheduledActiveKp AT %ML32 : LREAL :=",
        )
        .replace(
            "ScheduledActiveKi : LREAL :=",
            "ScheduledActiveKi AT %ML40 : LREAL :=",
        )
        .replace(
            "FixedAppliedPump : LREAL :=",
            "FixedAppliedPump AT %QL0 : LREAL :=",
        )
        .replace(
            "ScheduledAppliedPump : LREAL :=",
            "ScheduledAppliedPump AT %QL8 : LREAL :=",
        )
        .replace(
            "FixedTrackingActive : BOOL :=",
            "FixedTrackingActive AT %MX48.0 : BOOL :=",
        )
        .replace(
            "ScheduledTrackingActive : BOOL :=",
            "ScheduledTrackingActive AT %MX48.1 : BOOL :=",
        )
        .replace(
            "ScheduledScheduleValid : BOOL :=",
            "ScheduledScheduleValid AT %MX48.2 : BOOL :=",
        )
        .replace(
            "ScheduledScheduleClamped : BOOL :=",
            "ScheduledScheduleClamped AT %MX48.3 : BOOL :=",
        )
        .replace(
            "FixedFaultActive : BOOL :=",
            "FixedFaultActive AT %MX48.4 : BOOL :=",
        )
        .replace(
            "ScheduledFaultActive : BOOL :=",
            "ScheduledFaultActive AT %MX48.5 : BOOL :=",
        )
        .replace(
            "ControllerRevision : DINT :=",
            "ControllerRevision AT %MD52 : DINT :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "Control/advanced_control.st",
            project_file("libraries/control/src/advanced_control.st"),
        ),
        SourceFile::with_path(
            "control/14-integrated-capstones/80-gravity-tank/src/TankController.st",
            project_file("src/TankController.st"),
        ),
        SourceFile::with_path(
            "control/14-integrated-capstones/80-gravity-tank/src/Main.st",
            mapped_source(),
        ),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 80 Structured Text and Control library")
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

fn write_dint(runtime: &mut Runtime, address: &str, value: i32) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).unwrap(),
            Value::DWord(value as u32),
        )
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
    setpoint: f64,
    level: [f64; 2],
    requested: [f64; 2],
    applied: [f64; 2],
    filtered_level: f64,
    fraction: f64,
    active_kp: f64,
    active_ki: f64,
    tracking: [bool; 2],
    schedule_valid: bool,
    schedule_clamped: bool,
    fault: [bool; 2],
    revision: i32,
}

#[derive(Clone, Copy)]
struct Inputs {
    mode: i32,
    setpoint: f64,
    manual_flow: f64,
    quality_good: bool,
    scheduling_available: bool,
    pump_available: bool,
}

fn execute_sequence<F>(cycles: usize, mut inputs_for: F) -> Vec<Sample>
where
    F: FnMut(usize, Option<&Sample>) -> Inputs,
{
    let mut runtime = build_runtime();
    let mut simulation = SimulationController::new(
        SimulationConfig::load(project_path("simulation.toml"))
            .expect("load Lesson 80 gravity-tank configuration"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::with_capacity(cycles);
    for cycle in 0..cycles {
        let inputs = inputs_for(cycle, trace.last());
        write_dint(&mut runtime, "%ID192", inputs.mode);
        write_lreal(&mut runtime, "%IL200", inputs.setpoint);
        write_lreal(&mut runtime, "%IL208", inputs.manual_flow);
        write_lreal(&mut runtime, "%IL216", DT);
        write_bool(&mut runtime, "%IX240.0", inputs.quality_good);
        write_bool(&mut runtime, "%IX240.1", inputs.scheduling_available);
        write_bool(&mut runtime, "%IX240.2", inputs.pump_available);
        runtime
            .execute_cycle()
            .expect("execute Lesson 80 tank scan");
        let applied = ["%QL0", "%QL8"].map(|address| read_lreal(&runtime, address));
        assert!(applied
            .iter()
            .all(|value| value.is_finite() && (0.0..=0.060).contains(value)));
        let now = Duration::from_millis(((cycle + 1) * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance nonlinear tanks");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish tank levels");
        trace.push(Sample {
            setpoint: inputs.setpoint,
            level: ["%IL0", "%IL8"].map(|address| read_lreal(&runtime, address)),
            requested: ["%ML0", "%ML8"].map(|address| read_lreal(&runtime, address)),
            applied,
            filtered_level: read_lreal(&runtime, "%ML16"),
            fraction: read_lreal(&runtime, "%ML24"),
            active_kp: read_lreal(&runtime, "%ML32"),
            active_ki: read_lreal(&runtime, "%ML40"),
            tracking: ["%MX48.0", "%MX48.1"].map(|address| read_bool(&runtime, address)),
            schedule_valid: read_bool(&runtime, "%MX48.2"),
            schedule_clamped: read_bool(&runtime, "%MX48.3"),
            fault: ["%MX48.4", "%MX48.5"].map(|address| read_bool(&runtime, address)),
            revision: read_dint(&runtime, "%MD52"),
        });
    }
    trace
}

fn nominal_setpoint(cycle: usize) -> f64 {
    if cycle < 300 {
        0.30
    } else if cycle < 1_200 {
        0.55
    } else if cycle < 2_400 {
        0.80
    } else if cycle < 3_300 {
        0.55
    } else {
        0.30
    }
}

fn regional_iae(trace: &[Sample], train: usize) -> [f64; 4] {
    [(300, 1_200), (1_200, 2_400), (2_400, 3_300), (3_300, 4_200)].map(|(start, end)| {
        trace[start..end]
            .iter()
            .map(|sample| (sample.setpoint - sample.level[train]).abs() * DT)
            .sum()
    })
}

fn maximum_jump(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .fold(0.0, f64::max)
}

fn total_variation(values: impl Iterator<Item = f64>) -> f64 {
    let values = values.collect::<Vec<_>>();
    values
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum()
}

#[test]
fn control_curriculum_gravity_tank_capstone_qualifies_full_range_schedule() {
    let root = project_path("");
    assert!(
        root.is_dir(),
        "Lesson 80 executable gravity-tank capstone is missing: {}",
        root.display()
    );
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 80 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 80);
    assert_eq!(lesson["prerequisites"][0], "control-capstone-reactor");
    assert_eq!(lesson["nextLesson"], "control-capstone-conveyor-oven");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_gravity_tank_capstone_qualifies_full_range_schedule"
    );
    SimulationConfig::load(project_path("simulation.toml"))
        .expect("Lesson 80 configuration must be executable");

    let levels = [0.30_f64, 0.55, 0.80];
    let equilibrium_flows = levels.map(|level| 0.050 * level.sqrt());
    let local_gain = levels.map(|level| 2.0 * level.sqrt() / 0.050);
    let local_time_constant = levels.map(|level| 2.0 * 1.0 * level.sqrt() / 0.050);
    for (actual, expected) in
        equilibrium_flows
            .iter()
            .zip([0.027_386_127_875, 0.037_080_992_436, 0.044_721_359_550])
    {
        assert!((*actual - expected).abs() < 1.0e-10);
    }
    for index in 0..3 {
        assert!((local_gain[index] - local_time_constant[index]).abs() < 1.0e-12);
    }

    let nominal = execute_sequence(NOMINAL_CYCLES, |cycle, _| Inputs {
        mode: 1,
        setpoint: nominal_setpoint(cycle),
        manual_flow: equilibrium_flows[0],
        quality_good: true,
        scheduling_available: true,
        pump_available: true,
    });
    assert!(nominal.iter().all(|sample| sample.revision == 80));
    assert!(nominal
        .iter()
        .all(|sample| sample.schedule_valid && !sample.schedule_clamped));
    assert!(nominal
        .iter()
        .all(|sample| !sample.fault[0] && !sample.fault[1]));
    assert!(nominal.iter().all(|sample| {
        sample
            .level
            .iter()
            .all(|value| value.is_finite() && (0.20..=0.90).contains(value))
            && sample
                .requested
                .iter()
                .all(|value| value.is_finite() && (0.0..=0.060).contains(value))
            && (0.30..=0.80).contains(&sample.filtered_level)
            && (0.0..=1.0).contains(&sample.fraction)
            && (sample.active_kp - 0.100).abs() < 1.0e-12
            && (0.00280..=0.00456).contains(&sample.active_ki)
    }));
    let iae = [regional_iae(&nominal, 0), regional_iae(&nominal, 1)];
    let worst = iae.map(|regions| regions.into_iter().fold(0.0, f64::max));
    assert!(
        worst[1] <= 0.95 * worst[0],
        "scheduled worst IAE {:.4} versus fixed {:.4}",
        worst[1],
        worst[0]
    );
    let departure = [0, 1].map(|train| {
        nominal
            .iter()
            .map(|sample| (sample.setpoint - sample.level[train]).abs())
            .fold(0.0, f64::max)
    });
    assert!(departure[1] <= departure[0] + 1.0e-9);
    let pump_travel =
        [0, 1].map(|train| total_variation(nominal.iter().map(|sample| sample.applied[train])));
    let ki_jump = maximum_jump(nominal.iter().map(|sample| sample.active_ki));
    assert!(ki_jump <= 0.000_06, "atomic filtered Ki jump={ki_jump:.8}");

    let abnormal = execute_sequence(2_200, |cycle, previous| {
        let last_applied = previous.map_or(equilibrium_flows[0], |sample| sample.applied[1]);
        Inputs {
            mode: if (800..850).contains(&cycle) {
                2
            } else if (1_800..1_900).contains(&cycle) {
                3
            } else {
                1
            },
            setpoint: if (1_500..1_550).contains(&cycle) {
                0.85
            } else {
                0.55
            },
            manual_flow: last_applied,
            quality_good: !(1_000..1_020).contains(&cycle),
            scheduling_available: !(1_200..1_220).contains(&cycle),
            pump_available: !(1_400..1_420).contains(&cycle),
        }
    });
    assert!(abnormal[800..850]
        .iter()
        .all(|sample| sample.tracking[0] && sample.tracking[1]));
    for (start, end) in [(1_000, 1_020), (1_400, 1_420), (1_500, 1_550)] {
        assert!(abnormal[start..end].iter().all(|sample| sample
            .applied
            .iter()
            .all(|value| value.abs() < 1.0e-12)
            && sample.fault[1]));
    }
    assert!(abnormal[1_200..1_220]
        .iter()
        .all(|sample| sample.applied[1].abs() < 1.0e-12 && sample.fault[1]));
    assert!(abnormal[1_500..1_550]
        .iter()
        .any(|sample| sample.schedule_clamped));
    assert!(abnormal[1_800..1_900]
        .iter()
        .all(|sample| sample.tracking[0] && sample.tracking[1]));
    assert!(abnormal[1_890..1_900]
        .iter()
        .all(|sample| sample.applied.iter().all(|value| value.abs() < 1.0e-12)));
    let recovery_bump = [850, 1_020, 1_220, 1_420, 1_550, 1_900]
        .into_iter()
        .map(|recovery| (abnormal[recovery].applied[1] - abnormal[recovery - 1].applied[1]).abs())
        .fold(0.0, f64::max);
    assert!(recovery_bump <= 0.0005 + 1.0e-12);

    let architecture = [0.0, 1.0];
    let mut summary = String::from(
        "architecture,worst_regional_iae_m_s,maximum_departure_m,pump_travel_m3_s,verdict\n",
    );
    writeln!(
        summary,
        "fixed,{:.9},{:.9},{:.9},reject-full-range-performance",
        worst[0], departure[0], pump_travel[0]
    )
    .unwrap();
    writeln!(
        summary,
        "scheduled,{:.9},{:.9},{:.9},selected-unapplied",
        worst[1], departure[1], pump_travel[1]
    )
    .unwrap();
    evidence::retain(
        &root.join("figures"),
        "gravity-tank-capstone-evidence",
        "The scheduled controller improves the gravity tank across its full range",
        "Index 0 is fixed gain and index 1 is the atomically updated schedule; abnormal authority and recovery remain in the retained summary.",
        &[
            Panel {
                title: "Worst regional integrated absolute error",
                x_unit: "architecture index",
                y_unit: "m*s",
                x_minimum: -0.05,
                x_maximum: 1.05,
                y_minimum: 0.0,
                y_maximum: 12.0,
                series: vec![Series { name: "Worst regional IAE", color: "#2563eb", dashed: false, x: &architecture, y: &worst }],
            },
            Panel {
                title: "Maximum level departure",
                x_unit: "architecture index",
                y_unit: "m",
                x_minimum: -0.05,
                x_maximum: 1.05,
                y_minimum: 0.0,
                y_maximum: 0.30,
                series: vec![Series { name: "Maximum departure", color: "#dc2626", dashed: false, x: &architecture, y: &departure }],
            },
        ],
        &[
            RetainedSeries { panel: "Regional IAE", name: "Fixed", x: &[0.0, 1.0, 2.0, 3.0], y: &iae[0], x_unit: "operating region", y_unit: "m*s" },
            RetainedSeries { panel: "Regional IAE", name: "Scheduled", x: &[0.0, 1.0, 2.0, 3.0], y: &iae[1], x_unit: "operating region", y_unit: "m*s" },
        ],
        &summary,
    );

    println!(
        "gravity-tank capstone: equilibrium flows={equilibrium_flows:?} m3/s; local gain/time constants={local_gain:?}; fixed/scheduled regional IAE={iae:?} m*s; worst={worst:?}; departure={departure:?} m; pump travel={pump_travel:?} m3/s; max Ki jump={ki_jump:.8}; max authority recovery bump={recovery_bump:.8} m3/s"
    );
}
