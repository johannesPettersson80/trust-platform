use std::collections::VecDeque;
use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::SimulationConfig;
use trust_runtime::value::Value;
use trust_runtime::Runtime;

const DT: f64 = 0.1;
const SAMPLES: usize = 600;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn project_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/13-networked-fault-aware-and-diagnostic-control/75-loop-diagnosis")
        .join(relative)
}

fn project_file(relative: &str) -> String {
    let path = project_path(relative);
    assert!(
        path.is_file(),
        "required Lesson 75 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 75 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 75 program before configuration")
        .replace(
            "ControllerEnabled : BOOL :=",
            "ControllerEnabled AT %IX192.0 : BOOL :=",
        )
        .replace(
            "FlowQualityGood : BOOL :=",
            "FlowQualityGood AT %IX192.1 : BOOL :=",
        )
        .replace(
            "ReferenceQualityGood : BOOL :=",
            "ReferenceQualityGood AT %IX192.2 : BOOL :=",
        )
        .replace(
            "ScheduleTransition : BOOL :=",
            "ScheduleTransition AT %IX192.3 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL64 : LREAL :=")
        .replace(
            "ProcessValue : LREAL :=",
            "ProcessValue AT %IL72 : LREAL :=",
        )
        .replace(
            "ReferenceFlow : LREAL :=",
            "ReferenceFlow AT %IL80 : LREAL :=",
        )
        .replace(
            "ValveStemPosition : LREAL :=",
            "ValveStemPosition AT %IL88 : LREAL :=",
        )
        .replace(
            "UpstreamPressure : LREAL :=",
            "UpstreamPressure AT %IL96 : LREAL :=",
        )
        .replace(
            "AdjacentValveDemand : LREAL :=",
            "AdjacentValveDemand AT %IL104 : LREAL :=",
        )
        .replace(
            "MeasuredCycleTime : LREAL :=",
            "MeasuredCycleTime AT %IL112 : LREAL :=",
        )
        .replace(
            "ConfiguredProportionalGain : LREAL :=",
            "ConfiguredProportionalGain AT %IL120 : LREAL :=",
        )
        .replace(
            "ConfiguredIntegralGain : LREAL :=",
            "ConfiguredIntegralGain AT %IL128 : LREAL :=",
        )
        .replace(
            "OperatingRegion : INT :=",
            "OperatingRegion AT %IW160 : INT :=",
        )
        .replace(
            "ControllerMode : INT :=",
            "ControllerMode AT %MW70 : INT :=",
        )
        .replace(
            "ValveRequested : LREAL :=",
            "ValveRequested AT %QL0 : LREAL :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path("loop-diagnosis/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 75 Structured Text and Control library")
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

fn write_int(runtime: &mut Runtime, address: &str, value: i16) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).unwrap(),
            Value::Word(value as u16),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CaseKind {
    Healthy,
    PoorTuning,
    Stiction,
    SensorDrift,
    Saturation,
    UpstreamDisturbance,
    Interaction,
    ModelScheduleMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Diagnosis {
    Healthy,
    PoorTuning,
    Stiction,
    SensorDrift,
    Saturation,
    UpstreamDisturbance,
    Interaction,
    ModelScheduleMismatch,
    Inconclusive,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Remedy {
    ContinueMonitoring,
    ControlledRetuningTest,
    ValveInspection,
    InstrumentVerification,
    CapacityReview,
    DisturbanceInvestigation,
    InteractionStudy,
    ModelScheduleValidation,
    AcquireStemFeedback,
}

#[derive(Clone, Debug)]
struct Sample {
    time: f64,
    setpoint: f64,
    process_value: f64,
    reference_flow: f64,
    requested: f64,
    stem: Option<f64>,
    header: f64,
    adjacent: f64,
    region: i16,
    schedule_transition: bool,
    model_prediction: f64,
}

fn controller_gains(case: CaseKind) -> (f64, f64) {
    if case == CaseKind::PoorTuning {
        (18.0, 3.0)
    } else {
        (2.0, 0.2)
    }
}

fn run_case(case: CaseKind) -> Vec<Sample> {
    let mut runtime = build_runtime();
    let (kp, ki) = controller_gains(case);
    let mut flow = 50.0_f64;
    let mut model_prediction = 50.0_f64;
    let mut stem = 50.0_f64;
    let mut delayed_stem = VecDeque::from(vec![50.0; 10]);
    let mut trace = Vec::with_capacity(SAMPLES);

    for index in 0..SAMPLES {
        let time = index as f64 * DT;
        let setpoint = if case == CaseKind::Saturation && time >= 10.0 {
            120.0
        } else if case == CaseKind::Stiction && time >= 10.0 {
            if (((time - 10.0) / 10.0).floor() as usize) % 2 == 0 {
                55.0
            } else {
                45.0
            }
        } else if time >= 10.0 {
            55.0
        } else {
            50.0
        };
        let header = if case == CaseKind::UpstreamDisturbance && (20.0..25.0).contains(&time) {
            7.0
        } else {
            8.0
        };
        let adjacent = if case == CaseKind::Interaction
            && ((15.0..16.0).contains(&time)
                || (25.0..26.0).contains(&time)
                || (35.0..36.0).contains(&time))
        {
            20.0
        } else {
            0.0
        };
        let region = if case == CaseKind::ModelScheduleMismatch && time >= 20.0 {
            1
        } else {
            0
        };
        let schedule_transition = case == CaseKind::ModelScheduleMismatch && index == 200;
        let sensor_offset = if case == CaseKind::SensorDrift && time >= 10.0 {
            0.02 * (time - 10.0)
        } else {
            0.0
        };
        let process_value = flow + sensor_offset;

        write_bool(&mut runtime, "%IX192.0", true);
        write_bool(&mut runtime, "%IX192.1", true);
        write_bool(&mut runtime, "%IX192.2", true);
        write_bool(&mut runtime, "%IX192.3", schedule_transition);
        write_lreal(&mut runtime, "%IL64", setpoint);
        write_lreal(&mut runtime, "%IL72", process_value);
        write_lreal(&mut runtime, "%IL80", flow);
        write_lreal(&mut runtime, "%IL88", stem);
        write_lreal(&mut runtime, "%IL96", header);
        write_lreal(&mut runtime, "%IL104", adjacent);
        write_lreal(&mut runtime, "%IL112", DT);
        write_lreal(&mut runtime, "%IL120", kp);
        write_lreal(&mut runtime, "%IL128", ki);
        write_int(&mut runtime, "%MW70", 1);
        write_int(&mut runtime, "%IW160", region);
        runtime
            .execute_cycle()
            .expect("execute Lesson 75 controller scan");
        let requested = read_lreal(&runtime, "%QL0");
        assert!(requested.is_finite() && (0.0..=100.0).contains(&requested));

        trace.push(Sample {
            time,
            setpoint,
            process_value,
            reference_flow: flow,
            requested,
            stem: Some(stem),
            header,
            adjacent,
            region,
            schedule_transition,
            model_prediction,
        });

        if case == CaseKind::Stiction {
            // The independent valve oracle holds position for ten seconds and
            // then breaks away to the accumulated demand. This makes the
            // hold/ramp/jump signature observable without exposing a case
            // label to the Structured Text controller.
            if index >= 150 && (index - 150) % 100 == 0 {
                stem = requested;
            }
        } else {
            stem = requested;
        }
        stem = stem.clamp(0.0, 100.0);
        delayed_stem.push_back(stem);
        let delayed = delayed_stem.pop_front().unwrap();
        let nominal_target = delayed + 10.0 * (header - 8.0) - 0.4 * adjacent;
        let target = if region == 1 {
            50.0 + 0.7 * (delayed - 50.0)
        } else {
            nominal_target
        };
        flow += DT / 5.0 * (target - flow);
        model_prediction += DT / 5.0 * (delayed - model_prediction);
    }
    trace
}

fn slope(first: f64, last: f64, seconds: f64) -> f64 {
    (last - first) / seconds
}

fn classify(trace: &[Sample]) -> (Diagnosis, Remedy) {
    let event: Vec<_> = trace.iter().filter(|sample| sample.time >= 10.0).collect();
    let disagreement_slope = slope(
        event.first().unwrap().process_value - event.first().unwrap().reference_flow,
        event.last().unwrap().process_value - event.last().unwrap().reference_flow,
        event.last().unwrap().time - event.first().unwrap().time,
    );
    if disagreement_slope.abs() > 0.01 {
        return (Diagnosis::SensorDrift, Remedy::InstrumentVerification);
    }

    let saturation_window: Vec<_> = event
        .iter()
        .copied()
        .filter(|sample| sample.time >= 20.0)
        .collect();
    let saturated = saturation_window
        .iter()
        .filter(|sample| sample.stem.is_some_and(|stem| stem >= 99.9 || stem <= 0.1))
        .count() as f64
        / saturation_window.len() as f64;
    let mean_positive_error = saturation_window
        .iter()
        .map(|sample| (sample.setpoint - sample.process_value).max(0.0))
        .sum::<f64>()
        / saturation_window.len() as f64;
    if saturated > 0.90 && mean_positive_error > 2.0 {
        return (Diagnosis::Saturation, Remedy::CapacityReview);
    }

    let mut held_travel = 0.0_f64;
    let mut maximum_held_gap = 0.0_f64;
    let mut held_samples = 0_usize;
    let mut request_move_samples = 0_usize;
    let mut stiction_cycles = 0_usize;
    let mut previous_requested = trace[0].requested;
    let mut held_stem = trace[0].stem;
    for sample in trace.iter().skip(1) {
        let Some(stem) = sample.stem else {
            if sample.time >= 10.0 {
                return (Diagnosis::Inconclusive, Remedy::AcquireStemFeedback);
            }
            continue;
        };
        let hold_origin = held_stem.unwrap_or(stem);
        if (stem - hold_origin).abs() <= 0.2 {
            held_travel += (sample.requested - previous_requested).abs();
            maximum_held_gap = maximum_held_gap.max((sample.requested - stem).abs());
            held_samples += 1;
            if (sample.requested - previous_requested).abs() > 0.01 {
                request_move_samples += 1;
            }
        } else {
            if held_travel > 2.0
                && maximum_held_gap > 2.0
                && held_samples >= 5
                && request_move_samples >= 2
                && (stem - hold_origin).abs() > 1.0
            {
                stiction_cycles += 1;
            }
            held_travel = 0.0;
            maximum_held_gap = 0.0;
            held_samples = 0;
            request_move_samples = 0;
            held_stem = Some(stem);
        }
        previous_requested = sample.requested;
    }
    if stiction_cycles >= 3 {
        return (Diagnosis::Stiction, Remedy::ValveInspection);
    }

    if event.iter().any(|sample| sample.adjacent.abs() > 5.0) {
        return (Diagnosis::Interaction, Remedy::InteractionStudy);
    }
    if event.iter().any(|sample| (sample.header - 8.0).abs() > 0.2) {
        return (
            Diagnosis::UpstreamDisturbance,
            Remedy::DisturbanceInvestigation,
        );
    }
    if event.iter().any(|sample| sample.schedule_transition) {
        let residual = event
            .iter()
            .filter(|sample| sample.region == 1)
            .map(|sample| (sample.process_value - sample.model_prediction).abs())
            .sum::<f64>()
            / event.iter().filter(|sample| sample.region == 1).count() as f64;
        if residual > 0.5 {
            return (
                Diagnosis::ModelScheduleMismatch,
                Remedy::ModelScheduleValidation,
            );
        }
    }

    let errors: Vec<_> = event
        .iter()
        .map(|sample| sample.setpoint - sample.process_value)
        .collect();
    let zero_crossings = errors
        .windows(2)
        .filter(|pair| pair[0].signum() != pair[1].signum())
        .count();
    let rms = (errors.iter().map(|error| error * error).sum::<f64>() / errors.len() as f64).sqrt();
    if zero_crossings >= 6 && rms > 0.5 {
        return (Diagnosis::PoorTuning, Remedy::ControlledRetuningTest);
    }
    (Diagnosis::Healthy, Remedy::ContinueMonitoring)
}

#[test]
fn control_curriculum_loop_diagnosis_classifies_blinded_evidence() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 75 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 75 must use schema 4");
    assert_eq!(lesson["sequence"], 75);
    assert_eq!(lesson["prerequisites"][0], "control-pumping-fault-aware");
    assert_eq!(lesson["nextLesson"], "control-capstone-cold-storage");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_loop_diagnosis_classifies_blinded_evidence"
    );
    for index in 0..8 {
        let path = format!("scenarios/case-{index}.toml");
        SimulationConfig::load(project_path(&path)).unwrap_or_else(|error| {
            panic!("{path} must be an executable Simulation case: {error}")
        });
    }
    let answer_key: serde_json::Value =
        serde_json::from_str(&project_file("scenarios/answer-key.json"))
            .expect("parse sealed Lesson 75 answer key");
    assert_eq!(answer_key["cases"].as_array().map(Vec::len), Some(8));

    let cases = [
        (
            CaseKind::Healthy,
            Diagnosis::Healthy,
            Remedy::ContinueMonitoring,
        ),
        (
            CaseKind::PoorTuning,
            Diagnosis::PoorTuning,
            Remedy::ControlledRetuningTest,
        ),
        (
            CaseKind::Stiction,
            Diagnosis::Stiction,
            Remedy::ValveInspection,
        ),
        (
            CaseKind::SensorDrift,
            Diagnosis::SensorDrift,
            Remedy::InstrumentVerification,
        ),
        (
            CaseKind::Saturation,
            Diagnosis::Saturation,
            Remedy::CapacityReview,
        ),
        (
            CaseKind::UpstreamDisturbance,
            Diagnosis::UpstreamDisturbance,
            Remedy::DisturbanceInvestigation,
        ),
        (
            CaseKind::Interaction,
            Diagnosis::Interaction,
            Remedy::InteractionStudy,
        ),
        (
            CaseKind::ModelScheduleMismatch,
            Diagnosis::ModelScheduleMismatch,
            Remedy::ModelScheduleValidation,
        ),
    ];
    let mut summary = String::from("case,diagnosis,remedy\n");
    let mut stiction_trace = None;
    let mut drift_trace = None;
    for (case, expected_diagnosis, expected_remedy) in cases {
        let trace = run_case(case);
        let (diagnosis, remedy) = classify(&trace);
        assert_eq!(
            diagnosis, expected_diagnosis,
            "wrong diagnosis for {case:?}"
        );
        assert_eq!(remedy, expected_remedy, "wrong remedy for {case:?}");
        writeln!(summary, "{case:?},{diagnosis:?},{remedy:?}").unwrap();
        if case == CaseKind::Stiction {
            stiction_trace = Some(trace.clone());
        } else if case == CaseKind::SensorDrift {
            drift_trace = Some(trace.clone());
        }
    }

    let mut missing_stem = run_case(CaseKind::Stiction);
    for sample in &mut missing_stem {
        sample.stem = None;
    }
    assert_eq!(
        classify(&missing_stem),
        (Diagnosis::Inconclusive, Remedy::AcquireStemFeedback),
        "stiction versus tuning must remain inconclusive without stem feedback"
    );
    println!(
        "loop diagnosis: 8/8 blinded records and remedies correct; missing-stem case inconclusive"
    );

    let stiction = stiction_trace.expect("retain stiction evidence");
    let drift = drift_trace.expect("retain sensor-drift evidence");
    let stiction_time: Vec<f64> = stiction.iter().map(|sample| sample.time).collect();
    let stiction_request: Vec<f64> = stiction.iter().map(|sample| sample.requested).collect();
    let stiction_stem: Vec<f64> = stiction
        .iter()
        .map(|sample| sample.stem.expect("stiction stem feedback"))
        .collect();
    let drift_time: Vec<f64> = drift.iter().map(|sample| sample.time).collect();
    let drift_pv: Vec<f64> = drift.iter().map(|sample| sample.process_value).collect();
    let drift_reference: Vec<f64> = drift.iter().map(|sample| sample.reference_flow).collect();
    let figure_root = project_path("figures");
    evidence::retain(
        &figure_root,
        "loop-diagnosis-evidence",
        "Diagnosis follows physical evidence before retuning",
        "Valve stem feedback exposes stiction; the independent flow reference exposes sensor drift.",
        &[
            evidence::Panel {
                title: "Stiction fingerprint: demand moves while stem holds",
                x_unit: "time (s)",
                y_unit: "%",
                x_minimum: 0.0,
                x_maximum: 59.9,
                y_minimum: 0.0,
                y_maximum: 100.0,
                series: vec![
                    evidence::Series { name: "Requested valve", color: "#2563eb", dashed: false, x: &stiction_time, y: &stiction_request },
                    evidence::Series { name: "Measured stem", color: "#dc2626", dashed: true, x: &stiction_time, y: &stiction_stem },
                ],
            },
            evidence::Panel {
                title: "Sensor drift: measured PV (red), reference (green dashed)",
                x_unit: "time (s)",
                y_unit: "% flow",
                x_minimum: 0.0,
                x_maximum: 59.9,
                y_minimum: 45.0,
                y_maximum: 60.0,
                series: vec![
                    evidence::Series { name: "Measured PV", color: "#dc2626", dashed: false, x: &drift_time, y: &drift_pv },
                    evidence::Series { name: "Independent reference", color: "#16a34a", dashed: true, x: &drift_time, y: &drift_reference },
                ],
            },
        ],
        &[],
        &summary,
    );
}
