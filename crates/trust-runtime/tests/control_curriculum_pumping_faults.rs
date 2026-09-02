use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const DT: f64 = 0.1;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn lesson_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/13-networked-fault-aware-and-diagnostic-control/74-pumping-fault-aware")
}

fn project_file(relative: &str) -> String {
    let path = lesson_root().join(relative);
    assert!(
        path.is_file(),
        "Lesson 74 must ship the executable project file {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 74 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 74 program before configuration")
        .replace(
            "PressureASampleReceived : BOOL :=",
            "PressureASampleReceived AT %IX240.0 : BOOL :=",
        )
        .replace(
            "PressureBSampleReceived : BOOL :=",
            "PressureBSampleReceived AT %IX240.1 : BOOL :=",
        )
        .replace(
            "LevelSampleReceived : BOOL :=",
            "LevelSampleReceived AT %IX240.2 : BOOL :=",
        )
        .replace(
            "DriveFeedbackReceived : BOOL :=",
            "DriveFeedbackReceived AT %IX240.3 : BOOL :=",
        )
        .replace(
            "PressureAQualityGood : BOOL :=",
            "PressureAQualityGood AT %IX240.4 : BOOL :=",
        )
        .replace(
            "PressureBQualityGood : BOOL :=",
            "PressureBQualityGood AT %IX240.5 : BOOL :=",
        )
        .replace(
            "LevelQualityGood : BOOL :=",
            "LevelQualityGood AT %IX240.6 : BOOL :=",
        )
        .replace(
            "DriveFeedbackQualityGood : BOOL :=",
            "DriveFeedbackQualityGood AT %IX240.7 : BOOL :=",
        )
        .replace(
            "AcknowledgeRequest : BOOL :=",
            "AcknowledgeRequest AT %IX241.0 : BOOL :=",
        )
        .replace(
            "ResetApproved : BOOL :=",
            "ResetApproved AT %IX241.1 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL88 : LREAL :=")
        .replace(
            "LocalTimeSeconds : LREAL :=",
            "LocalTimeSeconds AT %IL96 : LREAL :=",
        )
        .replace(
            "PressureASourceTime : LREAL :=",
            "PressureASourceTime AT %IL104 : LREAL :=",
        )
        .replace(
            "PressureBSourceTime : LREAL :=",
            "PressureBSourceTime AT %IL112 : LREAL :=",
        )
        .replace(
            "LevelSourceTime : LREAL :=",
            "LevelSourceTime AT %IL120 : LREAL :=",
        )
        .replace(
            "DriveFeedbackSourceTime : LREAL :=",
            "DriveFeedbackSourceTime AT %IL128 : LREAL :=",
        )
        .replace("LevelRaw : LREAL :=", "LevelRaw AT %IL136 : LREAL :=")
        .replace(
            "NaivePressureA : LREAL :=",
            "NaivePressureA AT %IL0 : LREAL :=",
        )
        .replace("PressureARaw : LREAL :=", "PressureARaw AT %IL8 : LREAL :=")
        .replace(
            "PressureBRaw : LREAL :=",
            "PressureBRaw AT %IL16 : LREAL :=",
        )
        .replace(
            "DriveFeedbackRaw : LREAL :=",
            "DriveFeedbackRaw AT %IL24 : LREAL :=",
        )
        .replace(
            "NaiveEvidencePressure : LREAL :=",
            "NaiveEvidencePressure AT %IL40 : LREAL :=",
        )
        .replace(
            "NaiveRequested : LREAL :=",
            "NaiveRequested AT %ML144 : LREAL :=",
        )
        .replace(
            "ProtectedRequested : LREAL :=",
            "ProtectedRequested AT %ML152 : LREAL :=",
        )
        .replace("NaiveApplied : LREAL :=", "NaiveApplied AT %QL0 : LREAL :=")
        .replace(
            "ProtectedApplied : LREAL :=",
            "ProtectedApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "PressureAResidual : LREAL :=",
            "PressureAResidual AT %ML160 : LREAL :=",
        )
        .replace(
            "PressureBResidual : LREAL :=",
            "PressureBResidual AT %ML168 : LREAL :=",
        )
        .replace(
            "PressureDiscrepancy : LREAL :=",
            "PressureDiscrepancy AT %ML176 : LREAL :=",
        )
        .replace(
            "PredictedPressure : LREAL :=",
            "PredictedPressure AT %ML184 : LREAL :=",
        )
        .replace(
            "ConservativeLevel : LREAL :=",
            "ConservativeLevel AT %ML192 : LREAL :=",
        )
        .replace(
            "ActuatorTrackingResidual : LREAL :=",
            "ActuatorTrackingResidual AT %ML200 : LREAL :=",
        )
        .replace(
            "SelectedPressure : LREAL :=",
            "SelectedPressure AT %ML208 : LREAL :=",
        )
        .replace(
            "SelectedPressureSource : INT :=",
            "SelectedPressureSource AT %MW116 : INT :=",
        )
        .replace("OperatingMode : INT :=", "OperatingMode AT %MW108 : INT :=")
        .replace("FaultCode : INT :=", "FaultCode AT %MW110 : INT :=")
        .replace(
            "RecoveryHealthyCount : INT :=",
            "RecoveryHealthyCount AT %MW112 : INT :=",
        )
        .replace(
            "PressureAIsolated : BOOL :=",
            "PressureAIsolated AT %MX114.0 : BOOL :=",
        )
        .replace(
            "PressureBIsolated : BOOL :=",
            "PressureBIsolated AT %MX114.1 : BOOL :=",
        )
        .replace(
            "FaultLatched : BOOL :=",
            "FaultLatched AT %MX114.2 : BOOL :=",
        )
        .replace(
            "FaultAcknowledged : BOOL :=",
            "FaultAcknowledged AT %MX114.3 : BOOL :=",
        )
        .replace(
            "RecoveryReady : BOOL :=",
            "RecoveryReady AT %MX114.4 : BOOL :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "pumping-fault-aware/src/FaultAwarePumpSupervisor.st",
            project_file("src/FaultAwarePumpSupervisor.st"),
        ),
        SourceFile::with_path("pumping-fault-aware/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 74 Structured Text and Control library")
}

fn simulation() -> SimulationController {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-control-pumping-faults-{}-{stamp}.toml",
        std::process::id()
    ));
    std::fs::write(&path, project_file("simulation.toml"))
        .expect("write Lesson 74 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 74 simulation config");
    let _ = std::fs::remove_file(path);
    SimulationController::new(config)
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

fn read_int(runtime: &Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Word(value) => value as i16,
        value => panic!("expected INT at {address}, got {value:?}"),
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

#[test]
fn control_curriculum_pumping_faults_isolate_degrade_and_recover() {
    assert!(
        lesson_root().is_dir(),
        "Lesson 74 must ship one executable multirate fault-aware pumping project"
    );
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 74 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 74);
    assert_eq!(lesson["prerequisites"][0], "control-networked-pumping");
    assert_eq!(lesson["nextLesson"], "control-loop-diagnosis");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_pumping_faults_isolate_degrade_and_recover"
    );

    let mut runtime = build_runtime();
    let mut plant = simulation();
    plant.apply_pre_cycle(Duration::ZERO, &mut runtime).unwrap();
    let mut previous_protected_applied = 50.0;
    let mut predictor_oracle = 6.0;
    let mut naive_state_oracle = 6.0;
    let decay = (-DT / 4.0).exp();
    let mut maximum_restoration_rate = 0.0_f64;
    let mut false_trip_count = 0_usize;
    let mut wrong_sensor_selection_count = 0_usize;
    let mut bias_isolation_scan = None;
    let mut actuator_fault_scan = None;
    let mut safe_cases_seen = [false; 4];
    let mut safe_case_scans = [None; 4];
    let mut time_trace = Vec::with_capacity(280);
    let mut fault_trace = Vec::with_capacity(280);
    let mut mode_trace = Vec::with_capacity(280);
    let mut protected_command_trace = Vec::with_capacity(280);

    for scan in 0..280 {
        let time = scan as f64 * DT;
        let pressure_a_received = true;
        let pressure_b_received = true;
        let level_received = scan % 5 == 0 && !(150..170).contains(&scan);
        let drive_received = scan % 2 == 0;
        let pressure_a_good = !(115..130).contains(&scan);
        let pressure_b_good = !(120..130).contains(&scan);
        let level_value = if (190..200).contains(&scan) {
            19.0
        } else {
            35.0
        };
        let setpoint = if (215..240).contains(&scan) { 7.0 } else { 6.0 };
        let acknowledge = matches!(scan, 100 | 130 | 170 | 200 | 241);
        let reset = matches!(scan, 110 | 140 | 180 | 210 | 252);

        write_bool(&mut runtime, "%IX240.0", pressure_a_received);
        write_bool(&mut runtime, "%IX240.1", pressure_b_received);
        write_bool(&mut runtime, "%IX240.2", level_received);
        write_bool(&mut runtime, "%IX240.3", drive_received);
        write_bool(&mut runtime, "%IX240.4", pressure_a_good);
        write_bool(&mut runtime, "%IX240.5", pressure_b_good);
        write_bool(&mut runtime, "%IX240.6", true);
        write_bool(&mut runtime, "%IX240.7", true);
        write_bool(&mut runtime, "%IX241.0", acknowledge);
        write_bool(&mut runtime, "%IX241.1", reset);
        write_lreal(&mut runtime, "%IL88", setpoint);
        write_lreal(&mut runtime, "%IL96", time);
        write_lreal(&mut runtime, "%IL104", time);
        write_lreal(&mut runtime, "%IL112", time);
        if level_received {
            write_lreal(&mut runtime, "%IL120", time);
        }
        if drive_received {
            write_lreal(&mut runtime, "%IL128", time);
        }
        write_lreal(&mut runtime, "%IL136", level_value);

        predictor_oracle += 0.025 * (4.0 + 0.04 * previous_protected_applied - predictor_oracle);
        runtime
            .execute_cycle()
            .expect("execute Lesson 74 station scan");
        let protected_applied = read_lreal(&runtime, "%QL8");
        let naive_applied = read_lreal(&runtime, "%QL0");
        let predicted = read_lreal(&runtime, "%ML184");
        assert!(
            (predicted - predictor_oracle).abs() <= 2e-9,
            "scan {scan}: ST predictor {predicted} != oracle {predictor_oracle}"
        );
        assert!(protected_applied.is_finite() && (0.0..=100.0).contains(&protected_applied));
        assert!(naive_applied.is_finite() && (0.0..=100.0).contains(&naive_applied));
        if scan >= 252 {
            maximum_restoration_rate = maximum_restoration_rate
                .max((protected_applied - previous_protected_applied).abs() / DT);
        }

        let mode = read_int(&runtime, "%MW108");
        let fault = read_int(&runtime, "%MW110");
        time_trace.push(time);
        mode_trace.push(mode as f64);
        fault_trace.push(fault as f64);
        protected_command_trace.push(protected_applied);
        if scan < 50 && (mode != 0 || fault != 0 || read_bool(&runtime, "%MX114.2")) {
            false_trip_count += 1;
        }
        if (50..100).contains(&scan) && read_bool(&runtime, "%MX114.0") {
            bias_isolation_scan.get_or_insert(scan);
            assert!(!read_bool(&runtime, "%MX114.1"));
            assert!((read_lreal(&runtime, "%ML208") - read_lreal(&runtime, "%IL16")).abs() <= 1e-9);
        }
        if read_bool(&runtime, "%MX114.0") && read_int(&runtime, "%MW116") == 1 {
            wrong_sensor_selection_count += 1;
        }
        if (120..130).contains(&scan) && fault == 3 {
            safe_cases_seen[0] = true;
            safe_case_scans[0].get_or_insert(scan);
            assert_eq!(protected_applied, 0.0);
        }
        if (156..170).contains(&scan) && fault == 4 {
            safe_cases_seen[1] = true;
            safe_case_scans[1].get_or_insert(scan);
            assert_eq!(protected_applied, 0.0);
        }
        if (190..200).contains(&scan) && fault == 5 {
            safe_cases_seen[2] = true;
            safe_case_scans[2].get_or_insert(scan);
            assert_eq!(protected_applied, 0.0);
        }
        if (220..241).contains(&scan) && fault == 6 {
            actuator_fault_scan.get_or_insert(scan);
            safe_cases_seen[3] = true;
            safe_case_scans[3].get_or_insert(scan);
            assert_eq!(protected_applied, 0.0);
        }

        naive_state_oracle = 4.0
            + 0.04 * naive_applied
            + (naive_state_oracle - (4.0 + 0.04 * naive_applied)) * decay;
        previous_protected_applied = protected_applied;
        let now = Duration::from_millis((scan as i64 + 1) * 100);
        plant.apply_post_cycle(now, &runtime).unwrap();
        plant.apply_pre_cycle(now, &mut runtime).unwrap();
        assert!((read_lreal(&runtime, "%IL40") - naive_state_oracle).abs() <= 2e-8);
        if (50..100).contains(&(scan + 1)) {
            assert!(
                (read_lreal(&runtime, "%IL8") - read_lreal(&runtime, "%IL16") - 1.0).abs() <= 2e-8
            );
        }
    }

    assert_eq!(false_trip_count, 0);
    assert_eq!(wrong_sensor_selection_count, 0);
    assert_eq!(
        bias_isolation_scan,
        Some(52),
        "A bias must isolate on its third new sample"
    );
    let actuator_fault_scan = actuator_fault_scan.expect("stuck drive must be confirmed");
    assert!(
        actuator_fault_scan <= 232,
        "actuator fault was late at scan {actuator_fault_scan}"
    );
    assert!(safe_cases_seen.into_iter().all(|seen| seen));
    assert!(maximum_restoration_rate <= 10.0 + 1e-9);
    assert!(!read_bool(&runtime, "%MX114.2"));
    assert_eq!(read_int(&runtime, "%MW108"), 0);
    println!(
        "pumping faults: A isolated at {:.1}s; safe scans ambiguous/stale-level/low-level/actuator={:?}; actuator confirmed at {:.1}s; false trips={false_trip_count}; wrong selections={wrong_sensor_selection_count}; maximum restoration rate={maximum_restoration_rate:.1}%/s",
        bias_isolation_scan.unwrap() as f64 * DT,
        safe_case_scans,
        actuator_fault_scan as f64 * DT,
    );

    let mut summary = String::from("event,time_s\n");
    writeln!(
        summary,
        "pressure_a_isolated,{:.3}",
        bias_isolation_scan.unwrap() as f64 * DT
    )
    .unwrap();
    for (label, scan) in [
        "ambiguous_pressure",
        "stale_level",
        "low_level",
        "stuck_actuator",
    ]
    .iter()
    .zip(safe_case_scans)
    {
        writeln!(
            summary,
            "{label},{:.3}",
            scan.expect("safe event") as f64 * DT
        )
        .unwrap();
    }
    writeln!(
        summary,
        "actuator_confirmed,{:.3}",
        actuator_fault_scan as f64 * DT
    )
    .unwrap();
    writeln!(
        summary,
        "maximum_restoration_rate_percent_per_s,{maximum_restoration_rate:.9}"
    )
    .unwrap();
    let figure_root = lesson_root().join("figures");
    evidence::retain(
        &figure_root,
        "pumping-fault-aware-evidence",
        "Fault-aware control isolates, degrades, and recovers explicitly",
        "Fault codes: 0 healthy, 1 pressure A, 3 ambiguous, 4 stale level, 5 low level, 6 actuator. Modes: 0 automatic, 1 degraded, 2 safe.",
        &[
            evidence::Panel {
                title: "Diagnostic and operating state codes",
                x_unit: "time (s)",
                y_unit: "code",
                x_minimum: 0.0,
                x_maximum: 27.9,
                y_minimum: 0.0,
                y_maximum: 6.0,
                series: vec![
                    evidence::Series { name: "Fault", color: "#dc2626", dashed: false, x: &time_trace, y: &fault_trace },
                    evidence::Series { name: "Mode", color: "#2563eb", dashed: true, x: &time_trace, y: &mode_trace },
                ],
            },
            evidence::Panel {
                title: "Protected applied pump command",
                x_unit: "time (s)",
                y_unit: "%",
                x_minimum: 0.0,
                x_maximum: 27.9,
                y_minimum: 0.0,
                y_maximum: 100.0,
                series: vec![evidence::Series { name: "Applied command", color: "#16a34a", dashed: false, x: &time_trace, y: &protected_command_trace }],
            },
        ],
        &[],
        &summary,
    );
}
