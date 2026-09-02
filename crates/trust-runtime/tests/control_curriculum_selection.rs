use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};

#[path = "control_curriculum_motion_progression/evidence.rs"]
#[allow(dead_code)]
mod evidence;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

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

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/31-controller-selection")
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn modelling_project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/32-plant-response-classes")
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn conservation_project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/33-tank-heater-conservation")
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
        "trust-control-selection-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut trust_runtime::Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).unwrap(),
            Value::LWord(value.to_bits()),
        )
        .expect("write LREAL");
}

fn read_lreal(runtime: &trust_runtime::Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .expect("read LREAL")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn read_int(runtime: &trust_runtime::Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .expect("read INT")
    {
        Value::Word(value) => value as i16,
        value => panic!("expected INT, got {value:?}"),
    }
}

fn selection_runtime() -> trust_runtime::Runtime {
    let source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "ColdRoomSetpoint : LREAL :=",
            "ColdRoomSetpoint AT %ML0 : LREAL :=",
        )
        .replace(
            "HeaterSetpoint : LREAL :=",
            "HeaterSetpoint AT %ML8 : LREAL :=",
        )
        .replace(
            "TankSetpoint : LREAL :=",
            "TankSetpoint AT %ML16 : LREAL :=",
        )
        .replace(
            "TableSetpoint : LREAL :=",
            "TableSetpoint AT %ML24 : LREAL :=",
        )
        .replace(
            "ColdRoomTemperature : LREAL :=",
            "ColdRoomTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "HeaterTemperature : LREAL :=",
            "HeaterTemperature AT %IL8 : LREAL :=",
        )
        .replace("TankLevel : LREAL :=", "TankLevel AT %IL16 : LREAL :=")
        .replace(
            "TablePosition : LREAL :=",
            "TablePosition AT %IL24 : LREAL :=",
        )
        .replace(
            "TableVelocity : LREAL :=",
            "TableVelocity AT %IL32 : LREAL :=",
        )
        .replace(
            "ColdRoomCompressor : LREAL :=",
            "ColdRoomCompressor AT %QL0 : LREAL :=",
        )
        .replace("HeaterPower : LREAL :=", "HeaterPower AT %QL8 : LREAL :=")
        .replace("TankPump : LREAL :=", "TankPump AT %QL16 : LREAL :=")
        .replace("TableTorque : LREAL :=", "TableTorque AT %QL24 : LREAL :=")
        .replace(
            "ColdRoomStructure : INT :=",
            "ColdRoomStructure AT %MW64 : INT :=",
        )
        .replace(
            "HeaterStructure : INT :=",
            "HeaterStructure AT %MW66 : INT :=",
        )
        .replace("TankStructure : INT :=", "TankStructure AT %MW68 : INT :=")
        .replace(
            "TableStructure : INT :=",
            "TableStructure AT %MW70 : INT :=",
        );
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("controller-selection/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped controller-selection lesson")
}

#[test]
fn control_curriculum_selects_the_minimum_sufficient_structure_for_four_real_loops() {
    let mut runtime = selection_runtime();
    let config_path = temporary_config_path();
    std::fs::write(&config_path, project_file("simulation.toml"))
        .expect("write controller-selection simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load four-plant selection simulation"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, "%ML0", 2.0);
    write_lreal(&mut runtime, "%ML8", 30.0);
    write_lreal(&mut runtime, "%ML16", 60.0);
    write_lreal(&mut runtime, "%ML24", 1.0);

    let mut cold_tail = Vec::new();
    let mut heater_tail = Vec::new();
    let mut tank_tail = Vec::new();
    let mut table_tail = Vec::new();
    let mut compressor_transitions = 0usize;
    let mut minimum_transition_gap = usize::MAX;
    let mut last_transition_cycle = None;
    let mut previous_compressor = 0.0;
    let mut cold_trace = Vec::with_capacity(1_200);
    let mut heater_trace = Vec::with_capacity(1_200);
    let mut tank_trace = Vec::with_capacity(1_200);
    let mut table_trace = Vec::with_capacity(1_200);
    let mut compressor_trace = Vec::with_capacity(1_200);
    let mut heater_output_trace = Vec::with_capacity(1_200);
    let mut tank_output_trace = Vec::with_capacity(1_200);
    let mut table_output_trace = Vec::with_capacity(1_200);

    for cycle in 1usize..=1_200 {
        runtime
            .execute_cycle()
            .expect("execute controller-selection workshop");
        let compressor = read_lreal(&runtime, "%QL0");
        assert!(compressor == 0.0 || compressor == 100.0);
        if cycle > 1 && compressor != previous_compressor {
            compressor_transitions += 1;
            if let Some(previous_cycle) = last_transition_cycle {
                minimum_transition_gap = minimum_transition_gap.min(cycle - previous_cycle);
            }
            last_transition_cycle = Some(cycle);
        }
        previous_compressor = compressor;

        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance four plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish four plant states");
        cold_trace.push(read_lreal(&runtime, "%IL0"));
        heater_trace.push(read_lreal(&runtime, "%IL8"));
        tank_trace.push(read_lreal(&runtime, "%IL16"));
        table_trace.push(read_lreal(&runtime, "%IL24"));
        compressor_trace.push(compressor);
        heater_output_trace.push(read_lreal(&runtime, "%QL8"));
        tank_output_trace.push(read_lreal(&runtime, "%QL16"));
        table_output_trace.push(read_lreal(&runtime, "%QL24"));
        if cycle > 1_000 {
            cold_tail.push(read_lreal(&runtime, "%IL0"));
            heater_tail.push(read_lreal(&runtime, "%IL8"));
            tank_tail.push(read_lreal(&runtime, "%IL16"));
            table_tail.push(read_lreal(&runtime, "%IL24"));
        }
    }
    let _ = std::fs::remove_file(config_path);

    assert_eq!(
        read_int(&runtime, "%MW64"),
        1,
        "cold room must select ON/OFF"
    );
    assert_eq!(read_int(&runtime, "%MW66"), 3, "heater must select PI");
    assert_eq!(
        read_int(&runtime, "%MW68"),
        2,
        "integrating tank must select P"
    );
    assert_eq!(
        read_int(&runtime, "%MW70"),
        5,
        "loaded table must select PID"
    );

    let cold_min = cold_tail.iter().copied().fold(f64::INFINITY, f64::min);
    let cold_max = cold_tail.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let heater_error = (heater_tail.iter().sum::<f64>() / heater_tail.len() as f64 - 30.0).abs();
    let tank_error = (tank_tail.iter().sum::<f64>() / tank_tail.len() as f64 - 60.0).abs();
    let table_error = (table_tail.iter().sum::<f64>() / table_tail.len() as f64 - 1.0).abs();

    assert!(
        cold_min >= 1.35 && cold_max <= 2.65,
        "cold-room tail left protected band tolerance: {cold_min:.3}..{cold_max:.3}"
    );
    assert!(compressor_transitions > 0);
    assert!(
        minimum_transition_gap >= 50,
        "compressor dwell was {} ms",
        minimum_transition_gap * 100
    );
    assert!(
        heater_error <= 0.10,
        "PI heater final-window error={heater_error:.4} degC"
    );
    assert!(
        tank_error <= 0.05,
        "P tank final-window error={tank_error:.4} cm"
    );
    assert!(
        table_error <= 0.02,
        "PID table final-window error={table_error:.4} rad"
    );

    println!(
        "selected structures cold/heater/tank/table=ON-OFF/PI/P/PID; cold={cold_min:.3}..{cold_max:.3}degC, final errors heater/tank/table={heater_error:.4}degC/{tank_error:.4}cm/{table_error:.4}rad"
    );

    let normalized_pv = [
        cold_trace
            .iter()
            .map(|value| value / 2.0 * 100.0)
            .collect::<Vec<_>>(),
        heater_trace
            .iter()
            .map(|value| value / 30.0 * 100.0)
            .collect::<Vec<_>>(),
        tank_trace
            .iter()
            .map(|value| value / 60.0 * 100.0)
            .collect::<Vec<_>>(),
        table_trace
            .iter()
            .map(|value| value * 100.0)
            .collect::<Vec<_>>(),
    ];
    let table_utilization = table_output_trace
        .iter()
        .map(|value| value / 10.0 * 100.0)
        .collect::<Vec<_>>();
    let mut run = String::from("sample,time_s,cold_room_sp_degC,cold_room_pv_degC,compressor_percent,heater_sp_degC,heater_pv_degC,heater_output_percent,tank_sp_cm,tank_pv_cm,tank_pump_percent,table_sp_rad,table_pv_rad,table_torque_Nm,evidence_class\n");
    for index in 0..cold_trace.len() {
        run.push_str(&format!("{},{:.1},2.000000,{:.12},{:.12},30.000000,{:.12},{:.12},60.000000,{:.12},{:.12},1.000000,{:.12},{:.12},configured_simulation\n", index + 1, (index + 1) as f64 * 0.1, cold_trace[index], compressor_trace[index], heater_trace[index], heater_output_trace[index], tank_trace[index], tank_output_trace[index], table_trace[index], table_output_trace[index]));
    }
    let summary = format!("plant,selected_structure,accepted,decision_measure,measured_value,unit\ncold room,ON/OFF,true,tail minimum,{cold_min:.12},degC\ncold room,ON/OFF,true,tail maximum,{cold_max:.12},degC\nwater heater,PI,true,final-window absolute error,{heater_error:.12},degC\nbuffer tank,P,true,final-window absolute error,{tank_error:.12},cm\npositioning table,PID,true,final-window absolute error,{table_error:.12},rad\n");
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/31-controller-selection/figures");
    write_or_compare_curriculum_artifact(&artifact_root.join("controller-selection-run.csv"), &run);
    let candidates = [
        (
            "cold room",
            "ON/OFF",
            true,
            "selected",
            "",
            "ON/OFF",
            "binary actuator; cycling allowed; dwell required",
        ),
        (
            "cold room",
            "P",
            false,
            "rejected",
            "continuous output cannot be applied by binary compressor",
            "ON/OFF",
            "binary actuator",
        ),
        (
            "cold room",
            "PI",
            false,
            "rejected",
            "continuous output cannot be applied by binary compressor",
            "ON/OFF",
            "binary actuator",
        ),
        (
            "cold room",
            "PD",
            false,
            "rejected",
            "continuous output cannot be applied by binary compressor",
            "ON/OFF",
            "binary actuator",
        ),
        (
            "cold room",
            "PID",
            false,
            "rejected",
            "continuous output cannot be applied by binary compressor",
            "ON/OFF",
            "binary actuator",
        ),
        (
            "water heater",
            "ON/OFF",
            false,
            "rejected",
            "cycling violates continuous-temperature requirement",
            "PI",
            "zero load error; continuous actuator",
        ),
        (
            "water heater",
            "P",
            false,
            "rejected",
            "cannot retain required heater power at zero error",
            "PI",
            "zero load error",
        ),
        (
            "water heater",
            "PI",
            true,
            "selected",
            "",
            "PI",
            "zero load error; no demonstrated damping need",
        ),
        (
            "water heater",
            "PD",
            false,
            "rejected",
            "derivative does not remove constant-load error",
            "PI",
            "zero load error",
        ),
        (
            "water heater",
            "PID",
            false,
            "rejected",
            "derivative adds noise sensitivity without demonstrated benefit",
            "PI",
            "minimum sufficient structure",
        ),
        (
            "buffer tank",
            "ON/OFF",
            false,
            "rejected",
            "binary cycling violates level-accuracy requirement",
            "P",
            "integrating plant; continuous pump",
        ),
        (
            "buffer tank",
            "P",
            true,
            "selected",
            "",
            "P",
            "plant integration supplies zero setpoint error",
        ),
        (
            "buffer tank",
            "PI",
            false,
            "rejected",
            "second integrator is unnecessary for declared setpoint case",
            "P",
            "minimum sufficient structure",
        ),
        (
            "buffer tank",
            "PD",
            false,
            "rejected",
            "derivative adds noise sensitivity without demonstrated damping need",
            "P",
            "minimum sufficient structure",
        ),
        (
            "buffer tank",
            "PID",
            false,
            "rejected",
            "integral and derivative add mechanisms not required",
            "P",
            "minimum sufficient structure",
        ),
        (
            "positioning table",
            "ON/OFF",
            false,
            "rejected",
            "binary torque cannot meet position and damping requirements",
            "PID",
            "continuous reversible actuator",
        ),
        (
            "positioning table",
            "P",
            false,
            "rejected",
            "constant load leaves offset and high gain reduces damping",
            "PID",
            "zero load error; damping",
        ),
        (
            "positioning table",
            "PI",
            false,
            "rejected",
            "integral removes offset but does not supply damping",
            "PID",
            "damping required",
        ),
        (
            "positioning table",
            "PD",
            false,
            "rejected",
            "derivative supplies damping but constant load leaves offset",
            "PID",
            "zero load error required",
        ),
        (
            "positioning table",
            "PID",
            true,
            "selected",
            "",
            "PID",
            "stiffness; damping; zero load error",
        ),
    ];
    for plant in [
        "cold room",
        "water heater",
        "buffer tank",
        "positioning table",
    ] {
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| candidate.0 == plant)
                .count(),
            5
        );
        assert_eq!(
            candidates
                .iter()
                .filter(|candidate| candidate.0 == plant && candidate.2)
                .count(),
            1
        );
    }
    let mut candidate_csv = String::from("plant,candidate_structure,eligible,decision,rejection_reason,selected_structure,requirements,evidence_basis\n");
    for candidate in candidates {
        writeln!(
            candidate_csv,
            "{},{},{},{},{},{},{},structural pre-screen plus executed selected controller",
            candidate.0,
            candidate.1,
            candidate.2,
            candidate.3,
            candidate.4,
            candidate.5,
            candidate.6
        )
        .unwrap();
    }
    write_or_compare_curriculum_artifact(
        &artifact_root.join("controller-selection-candidates.csv"),
        &candidate_csv,
    );
    evidence::retain(&artifact_root, "controller-selection",
        "Choose the simplest controller that meets requirements",
        "Configured Simulation evidence · normalized PV and actuator use; physical values remain in the retained run",
        0.1,
        &[
            evidence::Panel { title: "Process value relative to setpoint", unit: "%", minimum: 0.0, maximum: 220.0, series: vec![
                evidence::Series { name: "cold room", color: "#0072b2", dashed: false, values: &normalized_pv[0] },
                evidence::Series { name: "heater", color: "#d55e00", dashed: false, values: &normalized_pv[1] },
                evidence::Series { name: "tank", color: "#009e73", dashed: false, values: &normalized_pv[2] },
                evidence::Series { name: "table", color: "#cc79a7", dashed: false, values: &normalized_pv[3] },
            ]},
            evidence::Panel { title: "Actuator use relative to declared range", unit: "%", minimum: -100.0, maximum: 100.0, series: vec![
                evidence::Series { name: "compressor", color: "#0072b2", dashed: false, values: &compressor_trace },
                evidence::Series { name: "heater", color: "#d55e00", dashed: false, values: &heater_output_trace },
                evidence::Series { name: "tank pump", color: "#009e73", dashed: false, values: &tank_output_trace },
                evidence::Series { name: "table torque", color: "#cc79a7", dashed: false, values: &table_utilization },
            ]},
        ], &[], &[], &summary);
}

#[test]
fn control_curriculum_classifies_four_physical_plant_responses_before_modelling() {
    let shipped_source = modelling_project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace("ExperimentInput : LREAL :=", "ExperimentInput AT %ML0 : LREAL :=")
        .replace("HeaterCommand : LREAL :=", "HeaterCommand AT %QL0 : LREAL :=")
        .replace("TankInflowTrim : LREAL :=", "TankInflowTrim AT %QL8 : LREAL :=")
        .replace("OvenCommand : LREAL :=", "OvenCommand AT %QL16 : LREAL :=")
        .replace("DriveTorque : LREAL :=", "DriveTorque AT %QL24 : LREAL :=")
        .replace("HeaterTemperature : LREAL :=", "HeaterTemperature AT %IL0 : LREAL :=")
        .replace("TankLevel : LREAL :=", "TankLevel AT %IL8 : LREAL :=")
        .replace("OvenTemperature : LREAL :=", "OvenTemperature AT %IL16 : LREAL :=")
        .replace("DrivePosition : LREAL :=", "DrivePosition AT %IL24 : LREAL :=")
        // Legacy aliases keep the pre-change example executable so the red is
        // a behavior assertion rather than an unmapped-process-image failure.
        .replace("ManipulatedValue : LREAL :=", "ManipulatedValue AT %QL0 : LREAL :=")
        .replace("ManipulatedValue2 : LREAL :=", "ManipulatedValue2 AT %QL8 : LREAL :=")
        .replace("ManipulatedValue3 : LREAL :=", "ManipulatedValue3 AT %QL16 : LREAL :=")
        .replace("ManipulatedValue4 : LREAL :=", "ManipulatedValue4 AT %QL24 : LREAL :=")
        .replace("ManipulatedValue5 : LREAL :=", "ManipulatedValue5 AT %QL32 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace("IntegratingValue : LREAL :=", "IntegratingValue AT %IL8 : LREAL :=")
        .replace("OscillatoryValue : LREAL :=", "OscillatoryValue AT %IL16 : LREAL :=")
        .replace("DelayedValue : LREAL :=", "DelayedValue AT %IL24 : LREAL :=")
        .replace("UnstableValue : LREAL :=", "UnstableValue AT %IL32 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("plant-response-classes/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped physical plant-classification lesson");

    let config_path = temporary_config_path();
    std::fs::write(&config_path, modelling_project_file("simulation.toml"))
        .expect("write physical classification simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load physical classification plants"),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, "%ML0", 1.0);

    let mut heater = Vec::with_capacity(120);
    let mut tank = Vec::with_capacity(120);
    let mut oven = Vec::with_capacity(120);
    let mut drive = Vec::with_capacity(120);
    for cycle in 1..=120 {
        runtime
            .execute_cycle()
            .expect("fan out normalized open-loop experiment");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance plant-class skid");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish plant responses");
        heater.push(read_lreal(&runtime, "%IL0"));
        tank.push(read_lreal(&runtime, "%IL8"));
        oven.push(read_lreal(&runtime, "%IL16"));
        drive.push(read_lreal(&runtime, "%IL24"));
    }
    let _ = std::fs::remove_file(config_path);

    let heater_final = *heater.last().unwrap();
    let heater_late_slope = (heater[119] - heater[99]) / 2.0;
    let tank_final = *tank.last().unwrap();
    let tank_late_slope = (tank[119] - tank[99]) / 2.0;
    let oven_delay_samples = oven
        .iter()
        .position(|value| (*value - 80.0).abs() > 0.01)
        .unwrap_or(oven.len());
    let oven_final = *oven.last().unwrap();
    let drive_peak = drive.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let mut drive_crossings = 0usize;
    for pair in drive.windows(2) {
        if (pair[0] - 1.0) * (pair[1] - 1.0) < 0.0 {
            drive_crossings += 1;
        }
    }

    assert!(
        (heater_final - 53.93).abs() <= 0.10,
        "self-regulating vessel final temperature={heater_final:.3}"
    );
    assert!(
        heater_late_slope.abs() <= 0.08,
        "self-regulating vessel late slope={heater_late_slope:.4} degC/s"
    );
    assert!(
        (tank_final - 62.0).abs() <= 0.15,
        "integrating tank final level={tank_final:.3}"
    );
    assert!(
        (tank_late_slope - 1.0).abs() <= 0.03,
        "integrating tank late slope={tank_late_slope:.4} cm/s"
    );
    assert!(
        oven_delay_samples >= 39,
        "delayed oven first moved after {:.1}s",
        oven_delay_samples as f64 / 10.0
    );
    assert!(
        (oven_final - 84.91).abs() <= 0.15,
        "delayed oven final temperature={oven_final:.3}"
    );
    assert!(
        drive_peak >= 1.40,
        "flexible-drive peak={drive_peak:.3} rad"
    );
    assert!(
        drive_crossings >= 2,
        "flexible-drive crossings={drive_crossings}"
    );
    assert!(
        !shipped_source.contains("Control.PID"),
        "open-loop classification lesson must not hide a feedback controller"
    );

    println!(
        "classified vessel/tank/oven/drive=self-regulating/integrating/delayed/oscillatory; final={heater_final:.3}degC/{tank_final:.3}cm/{oven_final:.3}degC, delay={:.1}s, drive peak/crossings={drive_peak:.3}/{drive_crossings}",
        oven_delay_samples as f64 / 10.0
    );

    let normalized = [
        heater
            .iter()
            .map(|value| (value - 20.0) / 34.0 * 100.0)
            .collect::<Vec<_>>(),
        tank.iter()
            .map(|value| (value - 50.0) / 12.0 * 100.0)
            .collect::<Vec<_>>(),
        oven.iter()
            .map(|value| (value - 80.0) / 20.0 * 100.0)
            .collect::<Vec<_>>(),
        drive.iter().map(|value| value * 100.0).collect::<Vec<_>>(),
    ];
    let mut run = String::from("sample,time_s,normalized_command,heater_command_percent,heater_temperature_degC,tank_inflow_trim_percent,tank_level_cm,oven_command_percent,oven_temperature_degC,drive_torque_Nm,drive_position_rad,evidence_class\n");
    for index in 0..heater.len() {
        run.push_str(&format!("{},{:.1},1.000000,40.000000,{:.12},10.000000,{:.12},40.000000,{:.12},1.000000,{:.12},configured_simulation\n", index + 1, (index + 1) as f64 * 0.1, heater[index], tank[index], oven[index], drive[index]));
    }
    let summary = format!("plant,response_class,accepted,decision_measure,measured_value,unit\nheated vessel,self-regulating,true,final temperature,{heater_final:.12},degC\nbuffer tank,integrating,true,late slope,{tank_late_slope:.12},cm/s\nconveyor oven,delayed,true,response delay,{:.12},s\nflexible drive,oscillatory,true,peak,{drive_peak:.12},rad\nflexible drive,oscillatory,true,steady-value crossings,{drive_crossings},count\n", oven_delay_samples as f64 / 10.0);
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/32-plant-response-classes/figures");
    write_or_compare_curriculum_artifact(
        &artifact_root.join("plant-response-classes-run.csv"),
        &run,
    );
    evidence::retain(&artifact_root, "plant-response-classes",
        "Open-loop steps reveal four response classes",
        "Configured Simulation evidence · plotted responses are normalized; the retained run preserves physical units",
        0.1,
        &[
            evidence::Panel { title: "Stored thermal and material responses", unit: "% of declared change", minimum: 0.0, maximum: 120.0, series: vec![
                evidence::Series { name: "self-regulating vessel", color: "#0072b2", dashed: false, values: &normalized[0] },
                evidence::Series { name: "integrating tank", color: "#d55e00", dashed: false, values: &normalized[1] },
            ]},
            evidence::Panel { title: "Delay and oscillation signatures", unit: "% of declared change", minimum: -20.0, maximum: 180.0, series: vec![
                evidence::Series { name: "delayed oven", color: "#009e73", dashed: false, values: &normalized[2] },
                evidence::Series { name: "flexible drive", color: "#cc79a7", dashed: false, values: &normalized[3] },
            ]},
        ], &[], &[], &summary);
    let worksheet = "response_id,first_second_observation,classification_status,still_plausible_classes,learner_commitment,reveal_status\nA,small rise without enough history to prove settling,ambiguous,self-regulating|integrating|delayed,,not_revealed\nB,small continuing rise without enough history to prove integration,ambiguous,self-regulating|integrating,,not_revealed\nC,no visible movement in the first second,ambiguous,dead time|zero gain|bad signal,,not_revealed\nD,early rise without enough history to prove repeated crossings,ambiguous,self-regulating|oscillatory,,not_revealed\n";
    write_or_compare_curriculum_artifact(
        &artifact_root.join("plant-response-classes-worksheet.csv"),
        worksheet,
    );
    evidence::retain(
        &artifact_root,
        "plant-response-classes-first-second",
        "One second is not enough to classify every response",
        "Unlabeled prediction view · record what remains ambiguous before revealing more data",
        0.1,
        &[
            evidence::Panel {
                title: "Responses A and B during the first second",
                unit: "% of declared change",
                minimum: -5.0,
                maximum: 55.0,
                series: vec![
                    evidence::Series {
                        name: "Response A",
                        color: "#0072b2",
                        dashed: false,
                        values: &normalized[0][..10],
                    },
                    evidence::Series {
                        name: "Response B",
                        color: "#d55e00",
                        dashed: false,
                        values: &normalized[1][..10],
                    },
                ],
            },
            evidence::Panel {
                title: "Responses C and D during the first second",
                unit: "% of declared change",
                minimum: -5.0,
                maximum: 55.0,
                series: vec![
                    evidence::Series {
                        name: "Response C",
                        color: "#009e73",
                        dashed: false,
                        values: &normalized[2][..10],
                    },
                    evidence::Series {
                        name: "Response D",
                        color: "#cc79a7",
                        dashed: false,
                        values: &normalized[3][..10],
                    },
                ],
            },
        ],
        &[],
        &[],
        worksheet,
    );
}

#[test]
fn control_curriculum_conservation_balances_predict_tank_and_heater() {
    let shipped_source = conservation_project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "TankInletFlow : LREAL :=",
            "TankInletFlow AT %ML0 : LREAL :=",
        )
        .replace(
            "TankOutletFlow : LREAL :=",
            "TankOutletFlow AT %ML8 : LREAL :=",
        )
        .replace(
            "HeaterCommandRequest : LREAL :=",
            "HeaterCommandRequest AT %ML16 : LREAL :=",
        )
        .replace(
            "TankNetFlowCommand : LREAL :=",
            "TankNetFlowCommand AT %QL0 : LREAL :=",
        )
        .replace(
            "HeaterAppliedCommand : LREAL :=",
            "HeaterAppliedCommand AT %QL8 : LREAL :=",
        )
        .replace("TankLevel : LREAL :=", "TankLevel AT %IL0 : LREAL :=")
        .replace(
            "HeaterTemperature : LREAL :=",
            "HeaterTemperature AT %IL8 : LREAL :=",
        )
        .replace(
            "TankPredictedRate : LREAL :=",
            "TankPredictedRate AT %ML32 : LREAL :=",
        )
        .replace(
            "TankPredictedLevelAfter20Seconds : LREAL :=",
            "TankPredictedLevelAfter20Seconds AT %ML40 : LREAL :=",
        )
        .replace(
            "HeaterPredictedEquilibrium : LREAL :=",
            "HeaterPredictedEquilibrium AT %ML48 : LREAL :=",
        )
        .replace(
            "HeaterPredictedAtDelayPlusTimeConstant : LREAL :=",
            "HeaterPredictedAtDelayPlusTimeConstant AT %ML56 : LREAL :=",
        )
        .replace(
            "HeaterTimeConstant : LREAL :=",
            "HeaterTimeConstant AT %ML64 : LREAL :=",
        )
        .replace("ModelValid : BOOL :=", "ModelValid AT %MX80.0 : BOOL :=");
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "tank-heater-conservation/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped conservation-law lesson");

    let config_path = temporary_config_path();
    std::fs::write(&config_path, conservation_project_file("simulation.toml"))
        .expect("write tank and heater simulation config");
    let config = SimulationConfig::load(&config_path).expect("load conservation-law plants");

    let mut simulation = SimulationController::new(config.clone());
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, "%ML0", 12.0);
    write_lreal(&mut runtime, "%ML8", 10.0);
    write_lreal(&mut runtime, "%ML16", 20.0);

    let mut tank_at_20_seconds = 0.0;
    let mut heater_at_six_seconds = 0.0;
    let mut heater_during_delay = Vec::new();
    let mut heater_tail = Vec::new();
    let mut tank_tail = Vec::new();
    let mut tank_trace = Vec::with_capacity(300);
    let mut heater_trace = Vec::with_capacity(300);
    for cycle in 1usize..=300 {
        runtime
            .execute_cycle()
            .expect("execute conservation-law calculations");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance tank and heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish tank and heater states");
        tank_trace.push(read_lreal(&runtime, "%IL0"));
        heater_trace.push(read_lreal(&runtime, "%IL8"));
        if cycle <= 10 {
            heater_during_delay.push(read_lreal(&runtime, "%IL8"));
        }
        if cycle == 60 {
            heater_at_six_seconds = read_lreal(&runtime, "%IL8");
        }
        if cycle == 200 {
            tank_at_20_seconds = read_lreal(&runtime, "%IL0");
        }
        if cycle > 280 {
            tank_tail.push(read_lreal(&runtime, "%IL0"));
            heater_tail.push(read_lreal(&runtime, "%IL8"));
        }
    }

    assert!(
        (read_lreal(&runtime, "%ML32") - 0.4).abs() <= 1e-12,
        "derived tank rate must be 0.4 cm/s"
    );
    assert!(
        (read_lreal(&runtime, "%ML40") - 58.0).abs() <= 1e-12,
        "derived tank level at 20 seconds must be 58 cm"
    );
    assert!(
        (tank_at_20_seconds - 58.0).abs() <= 0.05,
        "simulated tank level at 20 seconds={tank_at_20_seconds:.4} cm"
    );
    let tank_late_slope = (tank_tail.last().unwrap() - tank_tail.first().unwrap()) / 1.9;
    assert!(
        (tank_late_slope - 0.4).abs() <= 0.01,
        "simulated tank late slope={tank_late_slope:.5} cm/s"
    );

    assert!(
        (read_lreal(&runtime, "%ML64") - 5.0).abs() <= 1e-12,
        "derived heater time constant must be 5 seconds"
    );
    assert!(
        (read_lreal(&runtime, "%ML48") - 24.0).abs() <= 1e-12,
        "derived heater equilibrium must be 24 degrees Celsius"
    );
    let heater_prediction = read_lreal(&runtime, "%ML56");
    assert!(
        (heater_prediction - 22.528482235).abs() <= 1e-6,
        "derived one-time-constant prediction={heater_prediction:.9} degC"
    );
    assert!(
        heater_during_delay
            .iter()
            .all(|value| (*value - 20.0).abs() <= 1e-9),
        "heater changed before the declared one-second delay: {heater_during_delay:?}"
    );
    assert!(
        (heater_at_six_seconds - heater_prediction).abs() <= 0.08,
        "heater at delay plus one time constant={heater_at_six_seconds:.5} degC"
    );
    let heater_final = *heater_tail.last().unwrap();
    let heater_late_slope = (heater_tail.last().unwrap() - heater_tail.first().unwrap()) / 1.9;
    assert!(
        (heater_final - 24.0).abs() <= 0.03,
        "heater final temperature={heater_final:.5} degC"
    );
    assert!(
        heater_late_slope.abs() <= 0.01,
        "heater late slope={heater_late_slope:.6} degC/s"
    );

    let mut equilibrium_runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "tank-heater-conservation/src/Main.st",
        shipped_source
            .split("CONFIGURATION Config")
            .next()
            .unwrap()
            .replace(
                "TankInletFlow : LREAL :=",
                "TankInletFlow AT %ML0 : LREAL :=",
            )
            .replace(
                "TankOutletFlow : LREAL :=",
                "TankOutletFlow AT %ML8 : LREAL :=",
            )
            .replace(
                "HeaterCommandRequest : LREAL :=",
                "HeaterCommandRequest AT %ML16 : LREAL :=",
            )
            .replace(
                "TankNetFlowCommand : LREAL :=",
                "TankNetFlowCommand AT %QL0 : LREAL :=",
            )
            .replace(
                "HeaterAppliedCommand : LREAL :=",
                "HeaterAppliedCommand AT %QL8 : LREAL :=",
            )
            .replace("TankLevel : LREAL :=", "TankLevel AT %IL0 : LREAL :=")
            .replace(
                "HeaterTemperature : LREAL :=",
                "HeaterTemperature AT %IL8 : LREAL :=",
            ),
    )])
    .build_runtime()
    .expect("compile equal-flow conservation case");
    let mut equilibrium_simulation = SimulationController::new(config);
    equilibrium_simulation
        .apply_pre_cycle(Duration::ZERO, &mut equilibrium_runtime)
        .unwrap();
    write_lreal(&mut equilibrium_runtime, "%ML0", 10.0);
    write_lreal(&mut equilibrium_runtime, "%ML8", 10.0);
    let initial_level = read_lreal(&equilibrium_runtime, "%IL0");
    let mut equilibrium_trace = Vec::with_capacity(200);
    for cycle in 1usize..=200 {
        equilibrium_runtime
            .execute_cycle()
            .expect("execute equal-flow tank case");
        let now = Duration::from_millis((cycle * 100) as i64);
        equilibrium_simulation
            .apply_post_cycle(now, &equilibrium_runtime)
            .unwrap();
        equilibrium_simulation
            .apply_pre_cycle(now, &mut equilibrium_runtime)
            .unwrap();
        equilibrium_trace.push(read_lreal(&equilibrium_runtime, "%IL0"));
    }
    let _ = std::fs::remove_file(config_path);
    let equilibrium_level = read_lreal(&equilibrium_runtime, "%IL0");
    assert!(
        (equilibrium_level - initial_level).abs() <= 1e-9,
        "equal tank flows must hold level: {initial_level:.4} -> {equilibrium_level:.4} cm"
    );
    assert!(
        !shipped_source.contains("Control.PID"),
        "conservation-law lesson must not hide a feedback controller"
    );

    println!(
        "conservation predictions matched: tank rate/20s={tank_late_slope:.4}cm/s/{tank_at_20_seconds:.3}cm, equal-flow drift={:.6}cm, heater 6s/final={heater_at_six_seconds:.3}/{heater_final:.3}degC",
        (equilibrium_level - initial_level).abs()
    );

    let tank_prediction = (1..=300)
        .map(|sample| 50.0 + 0.4 * sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let heater_prediction_trace = (1..=300)
        .map(|sample| {
            let time = sample as f64 * 0.1;
            if time <= 1.0 {
                20.0
            } else {
                24.0 - 4.0 * (-(time - 1.0) / 5.0).exp()
            }
        })
        .collect::<Vec<_>>();
    let mut run = String::from("phase,sample,time_s,tank_inlet_L_s,tank_outlet_L_s,tank_level_cm,tank_predicted_cm,heater_request_percent,heater_temperature_degC,heater_predicted_degC,units,quality,model_parameters,evidence_class\n");
    for index in 0..tank_trace.len() {
        run.push_str(&format!("net_flow,{},{:.1},12.000000,10.000000,{:.12},{:.12},20.000000,{:.12},{:.12},tank_flow=L/s;tank_level=cm;heater_command=percent;heater_temperature=degC,good,A=0.5m2;C=50kJ/degC;H=10kW/degC;Pmax=200kW;dead_time=1s,configured_simulation\n", index + 1, (index + 1) as f64 * 0.1, tank_trace[index], tank_prediction[index], heater_trace[index], heater_prediction_trace[index]));
    }
    for (index, level) in equilibrium_trace.iter().enumerate() {
        run.push_str(&format!("equal_flow,{},{:.1},10.000000,10.000000,{level:.12},50.000000,0.000000,20.000000,20.000000,tank_flow=L/s;tank_level=cm;heater_command=percent;heater_temperature=degC,good,A=0.5m2;C=50kJ/degC;H=10kW/degC;Pmax=200kW;dead_time=1s,configured_simulation\n", index + 1, (index + 1) as f64 * 0.1));
    }
    let equal_flow_drift = (equilibrium_level - initial_level).abs();
    let summary = format!("case,accepted,decision_measure,measured_value,unit\ntank net flow,true,late-window level rate,{tank_late_slope:.12},cm/s\ntank net flow,true,level at 20 seconds,{tank_at_20_seconds:.12},cm\ntank equal flow,true,level drift,{equal_flow_drift:.12},cm\nheater,true,temperature at delay plus one time constant,{heater_at_six_seconds:.12},degC\nheater,true,final temperature,{heater_final:.12},degC\n");
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/33-tank-heater-conservation/figures");
    write_or_compare_curriculum_artifact(
        &artifact_root.join("tank-heater-conservation-run.csv"),
        &run,
    );
    evidence::retain(&artifact_root, "tank-heater-conservation",
        "Balances predict stored level and heat",
        "Configured Simulation evidence · analytical predictions are compared with independently executed plants",
        0.1,
        &[
            evidence::Panel { title: "Tank level", unit: "cm", minimum: 48.0, maximum: 64.0, series: vec![
                evidence::Series { name: "simulated level", color: "#0072b2", dashed: false, values: &tank_trace },
                evidence::Series { name: "mass-balance prediction", color: "#d55e00", dashed: true, values: &tank_prediction },
            ]},
            evidence::Panel { title: "Heater temperature", unit: "degC", minimum: 19.5, maximum: 24.5, series: vec![
                evidence::Series { name: "simulated temperature", color: "#009e73", dashed: false, values: &heater_trace },
                evidence::Series { name: "energy-balance prediction", color: "#cc79a7", dashed: true, values: &heater_prediction_trace },
            ]},
        ], &[], &[evidence::Marker { at_seconds: 1.0, label: "heater delay ends" }], &summary);
}
