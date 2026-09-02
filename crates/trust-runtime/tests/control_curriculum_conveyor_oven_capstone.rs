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
const CYCLES: usize = 1_800;

fn project_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/81-conveyor-oven")
        .join(relative)
}

fn project_file(relative: &str) -> String {
    let path = project_path(relative);
    assert!(
        path.is_file(),
        "Lesson 81 executable conveyor-oven capstone is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 81 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 81 program before configuration")
        .replace(
            "\n    OperatingMode : DINT :=",
            "\n    OperatingMode AT %ID192 : DINT :=",
        )
        .replace(
            "\n    TemperatureSetpoint : LREAL :=",
            "\n    TemperatureSetpoint AT %IL200 : LREAL :=",
        )
        .replace(
            "\n    UpstreamTemperatureDeviation : LREAL :=",
            "\n    UpstreamTemperatureDeviation AT %IL208 : LREAL :=",
        )
        .replace(
            "\n    ManualHeaterPower : LREAL :=",
            "\n    ManualHeaterPower AT %IL216 : LREAL :=",
        )
        .replace(
            "\n    CycleTime : LREAL :=",
            "\n    CycleTime AT %IL224 : LREAL :=",
        )
        .replace(
            "\n    UpstreamQualityGood : BOOL :=",
            "\n    UpstreamQualityGood AT %IX240.0 : BOOL :=",
        )
        .replace(
            "\n    TemperatureQualityGood : BOOL :=",
            "\n    TemperatureQualityGood AT %IX240.1 : BOOL :=",
        )
        .replace(
            "\n    ModelValid : BOOL :=",
            "\n    ModelValid AT %IX240.2 : BOOL :=",
        )
        .replace(
            "\n    HeaterAvailable : BOOL :=",
            "\n    HeaterAvailable AT %IX240.3 : BOOL :=",
        )
        .replace(
            "OrdinaryMeasuredTemperature : LREAL :=",
            "OrdinaryMeasuredTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "FeedforwardMeasuredTemperature : LREAL :=",
            "FeedforwardMeasuredTemperature AT %IL8 : LREAL :=",
        )
        .replace(
            "IntegratedMeasuredTemperature : LREAL :=",
            "IntegratedMeasuredTemperature AT %IL16 : LREAL :=",
        )
        .replace(
            "OrdinaryRequestedHeater : LREAL :=",
            "OrdinaryRequestedHeater AT %ML0 : LREAL :=",
        )
        .replace(
            "FeedforwardRequestedHeater : LREAL :=",
            "FeedforwardRequestedHeater AT %ML8 : LREAL :=",
        )
        .replace(
            "IntegratedRequestedHeater : LREAL :=",
            "IntegratedRequestedHeater AT %ML16 : LREAL :=",
        )
        .replace(
            "IntegratedFeedforwardContribution : LREAL :=",
            "IntegratedFeedforwardContribution AT %ML32 : LREAL :=",
        )
        .replace(
            "FeedforwardContribution : LREAL :=",
            "FeedforwardContribution AT %ML24 : LREAL :=",
        )
        .replace(
            "IntegratedPredictedTemperature : LREAL :=",
            "IntegratedPredictedTemperature AT %ML40 : LREAL :=",
        )
        .replace(
            "IntegratedDelayFreeModel : LREAL :=",
            "IntegratedDelayFreeModel AT %ML48 : LREAL :=",
        )
        .replace(
            "IntegratedDelayedModel : LREAL :=",
            "IntegratedDelayedModel AT %ML56 : LREAL :=",
        )
        .replace(
            "IntegratedModelCorrection : LREAL :=",
            "IntegratedModelCorrection AT %ML64 : LREAL :=",
        )
        .replace(
            "OrdinaryAppliedHeater : LREAL :=",
            "OrdinaryAppliedHeater AT %QL0 : LREAL :=",
        )
        .replace(
            "FeedforwardAppliedHeater : LREAL :=",
            "FeedforwardAppliedHeater AT %QL8 : LREAL :=",
        )
        .replace(
            "IntegratedAppliedHeater : LREAL :=",
            "IntegratedAppliedHeater AT %QL16 : LREAL :=",
        )
        .replace(
            "OrdinaryThermalInput : LREAL :=",
            "OrdinaryThermalInput AT %QL24 : LREAL :=",
        )
        .replace(
            "FeedforwardThermalInput : LREAL :=",
            "FeedforwardThermalInput AT %QL32 : LREAL :=",
        )
        .replace(
            "IntegratedThermalInput : LREAL :=",
            "IntegratedThermalInput AT %QL40 : LREAL :=",
        )
        .replace(
            "PredictorHistoryComplete : BOOL :=",
            "PredictorHistoryComplete AT %MX72.0 : BOOL :=",
        )
        .replace(
            "PredictorActive : BOOL :=",
            "PredictorActive AT %MX72.1 : BOOL :=",
        )
        .replace(
            "PredictorFallback : BOOL :=",
            "PredictorFallback AT %MX72.2 : BOOL :=",
        )
        .replace(
            "IntegratedTrackingActive : BOOL :=",
            "IntegratedTrackingActive AT %MX72.3 : BOOL :=",
        )
        .replace(
            "IntegratedFaultActive : BOOL :=",
            "IntegratedFaultActive AT %MX72.4 : BOOL :=",
        )
        .replace(
            "ControllerRevision : DINT :=",
            "ControllerRevision AT %MD76 : DINT :=",
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
            "control/14-integrated-capstones/81-conveyor-oven/src/OvenController.st",
            project_file("src/OvenController.st"),
        ),
        SourceFile::with_path(
            "control/14-integrated-capstones/81-conveyor-oven/src/Main.st",
            mapped_source(),
        ),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 81 Structured Text and Control library")
}

fn temporary_config(case: &str, gain: f64, time_constant: f64, delay: f64) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-control-oven-capstone-{case}-{}-{stamp}.toml",
        std::process::id()
    ));
    let source = project_file("simulation.toml")
        .replace("gain = 0.8", &format!("gain = {gain}"))
        .replace(
            "time_constant_s = 12.0",
            &format!("time_constant_s = {time_constant}"),
        )
        .replace("dead_time_s = 16.0", &format!("dead_time_s = {delay}"));
    std::fs::write(&path, source).expect("write temporary oven mismatch configuration");
    path
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
    temperature: [f64; 3],
    requested: [f64; 3],
    applied: [f64; 3],
    feedforward: [f64; 2],
    predicted: f64,
    correction: f64,
    history_complete: bool,
    predictor_active: bool,
    fallback: bool,
    tracking: bool,
    fault: bool,
    revision: i32,
}

#[derive(Clone, Copy)]
struct RunInputs {
    mode: i32,
    upstream_quality: bool,
    temperature_quality: bool,
    model_valid: bool,
    heater_available: bool,
}

fn run_case<F>(case: &str, gain: f64, time_constant: f64, delay: f64, mut inputs: F) -> Vec<Sample>
where
    F: FnMut(usize, Option<&Sample>) -> RunInputs,
{
    let mut runtime = build_runtime();
    let config_path = temporary_config(case, gain, time_constant, delay);
    let config = SimulationConfig::load(&config_path).expect("load delayed oven copies");
    assert_eq!(config.plant_system.plants.len(), 3);
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let mut trace = Vec::with_capacity(CYCLES);
    let mut manual = 0.0;
    for cycle in 0..CYCLES {
        let input = inputs(cycle, trace.last());
        if cycle == 1_200 {
            manual = trace
                .last()
                .map_or(0.0, |sample: &Sample| sample.applied[2]);
        }
        write_dint(&mut runtime, "%ID192", input.mode);
        write_lreal(
            &mut runtime,
            "%IL200",
            if cycle < 100 { 20.0 } else { 35.0 },
        );
        write_lreal(&mut runtime, "%IL208", if cycle < 540 { 0.0 } else { -2.0 });
        write_lreal(&mut runtime, "%IL216", manual);
        write_lreal(&mut runtime, "%IL224", DT);
        write_bool(&mut runtime, "%IX240.0", input.upstream_quality);
        write_bool(&mut runtime, "%IX240.1", input.temperature_quality);
        write_bool(&mut runtime, "%IX240.2", input.model_valid);
        write_bool(&mut runtime, "%IX240.3", input.heater_available);
        runtime
            .execute_cycle()
            .expect("execute Lesson 81 oven controllers");
        let applied = ["%QL0", "%QL8", "%QL16"].map(|address| read_lreal(&runtime, address));
        assert!(applied
            .iter()
            .all(|value| value.is_finite() && (0.0..=100.0).contains(value)));
        // Capture controller inputs and outputs at one scan boundary. Advancing
        // the plants first would compare current controller state with the next
        // scan's temperature and corrupt fallback and performance evidence.
        let sample = Sample {
            temperature: ["%IL0", "%IL8", "%IL16"].map(|address| read_lreal(&runtime, address)),
            requested: ["%ML0", "%ML8", "%ML16"].map(|address| read_lreal(&runtime, address)),
            applied,
            feedforward: ["%ML24", "%ML32"].map(|address| read_lreal(&runtime, address)),
            predicted: read_lreal(&runtime, "%ML40"),
            correction: read_lreal(&runtime, "%ML64"),
            history_complete: read_bool(&runtime, "%MX72.0"),
            predictor_active: read_bool(&runtime, "%MX72.1"),
            fallback: read_bool(&runtime, "%MX72.2"),
            tracking: read_bool(&runtime, "%MX72.3"),
            fault: read_bool(&runtime, "%MX72.4"),
            revision: read_dint(&runtime, "%MD76"),
        };
        let now = Duration::from_millis(((cycle + 1) * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance delayed ovens");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish product temperatures");
        trace.push(sample);
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

fn iae(trace: &[Sample], train: usize, start: usize, end: usize) -> f64 {
    trace[start..end]
        .iter()
        .map(|sample| (35.0 - sample.temperature[train]).abs() * DT)
        .sum()
}

fn rise_time(trace: &[Sample], train: usize) -> f64 {
    trace[100..700]
        .iter()
        .position(|sample| sample.temperature[train] >= 33.5)
        .expect("temperature reaches 90 percent before disturbance") as f64
        * DT
}

fn overshoot(trace: &[Sample], train: usize) -> f64 {
    trace[100..]
        .iter()
        .map(|sample| sample.temperature[train])
        .fold(35.0, f64::max)
        - 35.0
}

fn final_error(trace: &[Sample], train: usize) -> f64 {
    trace[1_700..]
        .iter()
        .map(|sample| (35.0 - sample.temperature[train]).abs())
        .sum::<f64>()
        / 100.0
}

fn total_variation(trace: &[Sample], train: usize) -> f64 {
    trace
        .windows(2)
        .map(|pair| (pair[1].applied[train] - pair[0].applied[train]).abs())
        .sum()
}

fn loop_response(omega: f64, kp: f64, ki: f64, gain: f64, tau: f64, delay: f64) -> (f64, f64) {
    let controller_re = kp;
    let controller_im = -ki / omega;
    let denominator = 1.0 + (omega * tau).powi(2);
    let plant_re = gain / denominator;
    let plant_im = -gain * omega * tau / denominator;
    let undelayed_re = controller_re * plant_re - controller_im * plant_im;
    let undelayed_im = controller_re * plant_im + controller_im * plant_re;
    let angle = -omega * delay;
    (
        undelayed_re * angle.cos() - undelayed_im * angle.sin(),
        undelayed_re * angle.sin() + undelayed_im * angle.cos(),
    )
}

fn frequency_margins(kp: f64, ki: f64) -> (f64, f64, f64, f64) {
    let mut previous_w = 1.0e-4_f64;
    let mut previous_mag = f64::INFINITY;
    let mut crossover = 0.0;
    let mut crossover_phase = 0.0;
    let mut phase_crossing = 0.0;
    let mut phase_crossing_mag = 0.0;
    for index in 1..=200_000 {
        let exponent = -4.0 + 6.0 * index as f64 / 200_000.0;
        let omega = 10.0_f64.powf(exponent);
        let (re, im) = loop_response(omega, kp, ki, 0.8, 12.0, 16.0);
        let magnitude = re.hypot(im);
        let phase = (-ki / omega).atan2(kp) - (omega * 12.0).atan() - omega * 16.0;
        if crossover == 0.0 && previous_mag >= 1.0 && magnitude <= 1.0 {
            crossover = omega;
            crossover_phase = phase;
        }
        if phase_crossing == 0.0 && phase <= -std::f64::consts::PI {
            phase_crossing = omega;
            phase_crossing_mag = magnitude;
        }
        previous_w = omega;
        previous_mag = magnitude;
    }
    assert!(previous_w > 0.0 && crossover > 0.0 && phase_crossing > 0.0);
    let phase_margin = std::f64::consts::PI + crossover_phase;
    let gain_margin = 1.0 / phase_crossing_mag;
    let delay_margin = phase_margin / crossover;
    (
        crossover,
        phase_margin.to_degrees(),
        gain_margin,
        delay_margin,
    )
}

fn loop_response_at_s(sr: f64, si: f64, kp: f64, ki: f64) -> (f64, f64) {
    let s_norm = sr * sr + si * si;
    let controller = (kp + ki * sr / s_norm, -ki * si / s_norm);
    let plant_denominator = (1.0 + 12.0 * sr).powi(2) + (12.0 * si).powi(2);
    let plant = (
        0.8 * (1.0 + 12.0 * sr) / plant_denominator,
        -0.8 * 12.0 * si / plant_denominator,
    );
    let undelayed = (
        controller.0 * plant.0 - controller.1 * plant.1,
        controller.0 * plant.1 + controller.1 * plant.0,
    );
    let delay_magnitude = (-16.0 * sr).exp();
    let delay = (
        delay_magnitude * (-16.0 * si).cos(),
        delay_magnitude * (-16.0 * si).sin(),
    );
    (
        undelayed.0 * delay.0 - undelayed.1 * delay.1,
        undelayed.0 * delay.1 + undelayed.1 * delay.0,
    )
}

fn nyquist_winding(kp: f64, ki: f64) -> (i32, f64) {
    let mut previous_angle = None;
    let mut accumulated_angle = 0.0;
    let mut minimum_distance = f64::INFINITY;
    let mut sample = |sr: f64, si: f64| {
        let response = loop_response_at_s(sr, si, kp, ki);
        let shifted = (1.0 + response.0, response.1);
        minimum_distance = minimum_distance.min(shifted.0.hypot(shifted.1));
        let angle = shifted.1.atan2(shifted.0);
        if let Some(previous) = previous_angle {
            let mut increment = angle - previous;
            while increment > std::f64::consts::PI {
                increment -= 2.0 * std::f64::consts::PI;
            }
            while increment < -std::f64::consts::PI {
                increment += 2.0 * std::f64::consts::PI;
            }
            accumulated_angle += increment;
        }
        previous_angle = Some(angle);
    };

    // Traverse the complete right-half-plane contour. The small semicircle
    // indents around the PI pole at the origin; the large semicircle closes the
    // contour instead of treating a wrapped positive-frequency trace as Nyquist proof.
    let maximum_frequency = 100.0;
    let indentation_radius = 1.0e-5;
    for index in 0..=20_000 {
        let omega =
            -maximum_frequency + (maximum_frequency - indentation_radius) * index as f64 / 20_000.0;
        sample(0.0, omega);
    }
    for index in 1..=2_000 {
        let angle = -std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * index as f64 / 2_000.0;
        sample(
            indentation_radius * angle.cos(),
            indentation_radius * angle.sin(),
        );
    }
    for index in 1..=20_000 {
        let omega =
            indentation_radius + (maximum_frequency - indentation_radius) * index as f64 / 20_000.0;
        sample(0.0, omega);
    }
    for index in 1..=10_000 {
        let angle = std::f64::consts::FRAC_PI_2 - std::f64::consts::PI * index as f64 / 10_000.0;
        sample(
            maximum_frequency * angle.cos(),
            maximum_frequency * angle.sin(),
        );
    }
    (
        (accumulated_angle / (2.0 * std::f64::consts::PI)).round() as i32,
        minimum_distance,
    )
}

#[test]
fn control_curriculum_conveyor_oven_capstone_selects_delay_architecture() {
    let root = project_path("");
    assert!(
        root.is_dir(),
        "Lesson 81 executable conveyor-oven capstone is missing: {}",
        root.display()
    );
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 81 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 81);
    assert_eq!(lesson["prerequisites"][0], "control-capstone-gravity-tank");
    assert_eq!(lesson["nextLesson"], "control-capstone-distillation");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_conveyor_oven_capstone_selects_delay_architecture"
    );
    SimulationConfig::load(project_path("simulation.toml"))
        .expect("Lesson 81 configuration must be executable");

    let (crossover, phase_margin, gain_margin, delay_margin) =
        frequency_margins(0.46875, 0.0390625);
    let (_, aggressive_phase_margin, _, _) = frequency_margins(0.9375, 0.078125);
    let (nyquist_encirclements, nyquist_distance) = nyquist_winding(0.46875, 0.0390625);
    assert!(
        phase_margin >= 35.0,
        "ordinary PI phase margin={phase_margin:.2} deg"
    );
    assert!(
        delay_margin >= 2.0,
        "ordinary PI delay margin={delay_margin:.2} s"
    );
    assert!(gain_margin > 1.0);
    assert!(
        aggressive_phase_margin < 35.0,
        "next aggressive PI must be rejected, phase margin={aggressive_phase_margin:.2} deg"
    );
    assert_eq!(
        nyquist_encirclements, 0,
        "stable open-loop plant has P=0, so accepted feedback requires N=0 and Z=0"
    );
    assert!(
        nyquist_distance > 0.0,
        "Nyquist contour must not cross the critical point"
    );

    let healthy = |_: usize, _: Option<&Sample>| RunInputs {
        mode: 1,
        upstream_quality: true,
        temperature_quality: true,
        model_valid: true,
        heater_available: true,
    };
    let nominal = run_case("nominal", 0.8, 12.0, 16.0, healthy);
    let moderate = run_case("moderate", 0.76, 13.0, 17.0, healthy);
    let severe = run_case("severe", 0.60, 18.0, 22.0, healthy);
    for trace in [&nominal, &moderate, &severe] {
        assert!(trace.iter().all(|sample| sample.revision == 81));
        assert!(trace.iter().all(|sample| sample
            .temperature
            .iter()
            .all(|value| value.is_finite() && (15.0..=60.0).contains(value))));
        assert!(trace.iter().all(|sample| sample
            .requested
            .iter()
            .all(|value| value.is_finite() && (0.0..=100.0).contains(value))));
    }
    assert!(nominal[540..]
        .iter()
        .all(|sample| (sample.feedforward[0] - 2.5).abs() <= 1.0e-9
            && (sample.feedforward[1] - 2.5).abs() <= 1.0e-9));
    let ordinary_setpoint_iae = iae(&nominal, 0, 100, 700);
    let integrated_setpoint_iae = iae(&nominal, 2, 100, 700);
    let ordinary_regulatory_iae = iae(&nominal, 0, 700, 1_800);
    let feedforward_regulatory_iae = iae(&nominal, 1, 700, 1_800);
    let ordinary_rise = rise_time(&nominal, 0);
    let integrated_rise = rise_time(&nominal, 2);
    let ordinary_overshoot = overshoot(&nominal, 0);
    let ordinary_final_error = final_error(&nominal, 0);
    assert!(ordinary_overshoot <= 2.0);
    assert!(ordinary_final_error <= 0.5);
    assert!(
        integrated_rise <= 0.85 * ordinary_rise,
        "integrated rise {integrated_rise:.1}s versus ordinary {ordinary_rise:.1}s"
    );
    assert!(integrated_setpoint_iae < ordinary_setpoint_iae);
    assert!(feedforward_regulatory_iae <= 0.80 * ordinary_regulatory_iae,
        "feedforward regulatory IAE {feedforward_regulatory_iae:.3} versus ordinary {ordinary_regulatory_iae:.3}");
    assert!(nominal[100..]
        .iter()
        .all(|sample| sample.history_complete && sample.predictor_active && !sample.fallback));
    let nominal_correction = nominal
        .iter()
        .map(|sample| sample.correction.abs())
        .fold(0.0, f64::max);
    let moderate_correction = moderate
        .iter()
        .map(|sample| sample.correction.abs())
        .fold(0.0, f64::max);
    assert!(
        moderate_correction > nominal_correction
            || iae(&moderate, 2, 100, 1_800) > iae(&nominal, 2, 100, 1_800)
    );
    assert!(moderate[100..].iter().all(|sample| !sample.fallback));
    assert!(severe[100..].iter().any(|sample| sample.fallback));
    assert!(severe
        .iter()
        .filter(|sample| sample.fallback)
        .all(|sample| (sample.predicted - sample.temperature[2]).abs() <= 1.0e-9));
    let severe_fallback_count = severe.iter().filter(|sample| sample.fallback).count();
    let feedforward_improvement =
        100.0 * (1.0 - feedforward_regulatory_iae / ordinary_regulatory_iae);
    let rise_improvement = 100.0 * (1.0 - integrated_rise / ordinary_rise);

    let abnormal = run_case("abnormal", 0.8, 12.0, 16.0, |cycle, _| RunInputs {
        mode: if (1_200..1_250).contains(&cycle) {
            2
        } else if cycle >= 1_600 {
            3
        } else {
            1
        },
        upstream_quality: !(800..850).contains(&cycle),
        temperature_quality: !(1_000..1_020).contains(&cycle),
        model_valid: !(900..950).contains(&cycle),
        heater_available: !(1_400..1_420).contains(&cycle),
    });
    assert!(abnormal[800..850]
        .iter()
        .all(|sample| sample.feedforward[0].abs() < 1.0e-12
            && sample.feedforward[1].abs() < 1.0e-12
            && !sample.fault));
    assert!(abnormal[900..950]
        .iter()
        .all(|sample| sample.fallback && !sample.fault));
    for (start, end) in [(1_000, 1_020), (1_400, 1_420)] {
        assert!(abnormal[start..end].iter().all(|sample| sample
            .applied
            .iter()
            .all(|value| value.abs() < 1.0e-12)
            && sample.fault));
    }
    assert!(abnormal[1_200..1_250].iter().all(|sample| sample.tracking));
    assert!(abnormal[1_750..]
        .iter()
        .all(|sample| sample.applied[2].abs() < 1.0e-12));

    let design = [0.0, 1.0];
    let setpoint_iae = [ordinary_setpoint_iae, integrated_setpoint_iae];
    let regulatory_iae = [ordinary_regulatory_iae, feedforward_regulatory_iae];
    let margins = [phase_margin, aggressive_phase_margin];
    let mut summary = String::from("metric,ordinary,improved,unit,decision\n");
    writeln!(summary, "setpoint_iae,{ordinary_setpoint_iae:.9},{integrated_setpoint_iae:.9},degC*s,Smith-selected").unwrap();
    writeln!(summary, "regulatory_iae,{ordinary_regulatory_iae:.9},{feedforward_regulatory_iae:.9},degC*s,feedforward-selected").unwrap();
    writeln!(
        summary,
        "phase_margin,{phase_margin:.9},{aggressive_phase_margin:.9},deg,aggressive-rejected"
    )
    .unwrap();
    writeln!(
        summary,
        "delay_margin,{delay_margin:.9},,s,ordinary-qualified"
    )
    .unwrap();
    writeln!(
        summary,
        "severe_fallback_scans,{severe_fallback_count},,count,fallback-required"
    )
    .unwrap();
    evidence::retain(
        &root.join("figures"),
        "conveyor-oven-capstone-evidence",
        "Delay compensation earns authority only on qualified model evidence",
        "Index 0 is ordinary PI; index 1 is Smith prediction for setpoint response and measured feedforward for disturbance rejection.",
        &[
            Panel {
                title: "Setpoint integrated absolute error",
                x_unit: "design index",
                y_unit: "degC*s",
                x_minimum: -0.05,
                x_maximum: 1.05,
                y_minimum: 0.0,
                y_maximum: 600.0,
                series: vec![Series { name: "Setpoint IAE", color: "#2563eb", dashed: false, x: &design, y: &setpoint_iae }],
            },
            Panel {
                title: "Regulatory integrated absolute error",
                x_unit: "design index",
                y_unit: "degC*s",
                x_minimum: -0.05,
                x_maximum: 1.05,
                y_minimum: 0.0,
                y_maximum: 60.0,
                series: vec![Series { name: "Regulatory IAE", color: "#16a34a", dashed: false, x: &design, y: &regulatory_iae }],
            },
        ],
        &[RetainedSeries { panel: "Phase-margin comparison", name: "Phase margin", x: &design, y: &margins, x_unit: "design index", y_unit: "deg" }],
        &summary,
    );

    println!(
        "conveyor-oven capstone: ordinary crossover={crossover:.5} rad/s, phase margin={phase_margin:.2} deg, gain margin={gain_margin:.2}, delay margin={delay_margin:.2} s; aggressive phase margin={aggressive_phase_margin:.2} deg; Nyquist N={nyquist_encirclements}, min distance={nyquist_distance:.3}; ordinary overshoot={ordinary_overshoot:.3} degC, final error={ordinary_final_error:.3} degC; setpoint IAE ordinary/integrated={ordinary_setpoint_iae:.2}/{integrated_setpoint_iae:.2} degC*s; regulatory IAE ordinary/feedforward={ordinary_regulatory_iae:.2}/{feedforward_regulatory_iae:.2} degC*s ({feedforward_improvement:.1}% better); rise ordinary/integrated={ordinary_rise:.1}/{integrated_rise:.1} s ({rise_improvement:.1}% faster); heater travel={:.2}/{:.2}/{:.2} pp; correction nominal/moderate={nominal_correction:.3}/{moderate_correction:.3} degC; severe fallback scans={severe_fallback_count}",
        total_variation(&nominal, 0), total_variation(&nominal, 1), total_variation(&nominal, 2),
    );
}
