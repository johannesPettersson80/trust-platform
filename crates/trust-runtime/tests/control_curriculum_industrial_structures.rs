use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root(level: &str, project: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(level)
        .join(project)
        .join("figures")
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
        .join(
            "../../examples/control/09-industrial-control-structures/53-conveyor-oven-feedforward",
        )
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 53 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 53 file {}: {error}", path.display()))
}

fn cascade_project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/54-reactor-cascade")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 54 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 54 file {}: {error}", path.display()))
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-feedforward-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn temporary_cascade_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-cascade-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL lesson input");
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL lesson input");
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
struct FeedforwardRun {
    temperatures: [Vec<f64>; 3],
    heater_requests: [Vec<f64>; 3],
    actual_speed: Vec<f64>,
    measured_speed: Vec<f64>,
    speed_quality: Vec<f64>,
    static_feedforward: Vec<f64>,
    dynamic_feedforward: Vec<f64>,
}

fn run_feedforward(scale: f64, speed_valid: bool) -> FeedforwardRun {
    let shipped_source = project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("lesson program before configuration")
        .replace("SpeedValid : BOOL :=", "SpeedValid AT %MX88.0 : BOOL :=")
        .replace(
            "ActualLineSpeed : LREAL :=",
            "ActualLineSpeed AT %ML96 : LREAL :=",
        )
        .replace(
            "MeasuredLineSpeed : LREAL :=",
            "MeasuredLineSpeed AT %ML104 : LREAL :=",
        )
        .replace(
            "DynamicFeedforwardScale : LREAL :=",
            "DynamicFeedforwardScale AT %ML112 : LREAL :=",
        )
        .replace(
            "StaticFeedforward : LREAL :=",
            "StaticFeedforward AT %ML120 : LREAL :=",
        )
        .replace(
            "DynamicFeedforward : LREAL :=",
            "DynamicFeedforward AT %ML128 : LREAL :=",
        )
        .replace(
            "FeedbackTemperature : LREAL :=",
            "FeedbackTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "StaticTemperature : LREAL :=",
            "StaticTemperature AT %IL8 : LREAL :=",
        )
        .replace(
            "DynamicTemperature : LREAL :=",
            "DynamicTemperature AT %IL16 : LREAL :=",
        )
        .replace(
            "FeedbackHeaterRequest : LREAL :=",
            "FeedbackHeaterRequest AT %QL0 : LREAL :=",
        )
        .replace(
            "FeedbackSpeedInput : LREAL :=",
            "FeedbackSpeedInput AT %QL8 : LREAL :=",
        )
        .replace(
            "StaticHeaterRequest : LREAL :=",
            "StaticHeaterRequest AT %QL16 : LREAL :=",
        )
        .replace(
            "StaticSpeedInput : LREAL :=",
            "StaticSpeedInput AT %QL24 : LREAL :=",
        )
        .replace(
            "DynamicHeaterRequest : LREAL :=",
            "DynamicHeaterRequest AT %QL32 : LREAL :=",
        )
        .replace(
            "DynamicSpeedInput : LREAL :=",
            "DynamicSpeedInput AT %QL40 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("conveyor-oven-feedforward/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped feedforward lesson");

    let config_path = temporary_config_path();
    std::fs::write(&config_path, project_file("simulation.toml"))
        .expect("write feedforward simulation config");
    let config = SimulationConfig::load(&config_path).expect("load three equivalent ovens");
    assert_eq!(config.plant_system.plants.len(), 3);
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial oven temperatures");

    let valid = IoAddress::parse("%MX88.0").unwrap();
    let actual_speed = IoAddress::parse("%ML96").unwrap();
    let measured_speed = IoAddress::parse("%ML104").unwrap();
    let dynamic_scale = IoAddress::parse("%ML112").unwrap();
    let static_ff = IoAddress::parse("%ML120").unwrap();
    let dynamic_ff = IoAddress::parse("%ML128").unwrap();
    let temperatures = ["%IL0", "%IL8", "%IL16"].map(|value| IoAddress::parse(value).unwrap());
    let heaters = ["%QL0", "%QL16", "%QL32"].map(|value| IoAddress::parse(value).unwrap());
    let mut result = FeedforwardRun {
        temperatures: std::array::from_fn(|_| Vec::with_capacity(900)),
        heater_requests: std::array::from_fn(|_| Vec::with_capacity(900)),
        actual_speed: Vec::with_capacity(900),
        measured_speed: Vec::with_capacity(900),
        speed_quality: Vec::with_capacity(900),
        static_feedforward: Vec::with_capacity(900),
        dynamic_feedforward: Vec::with_capacity(900),
    };
    for cycle in 1usize..=900 {
        let speed = if cycle <= 300 { 1.0 } else { 2.0 };
        write_bool(&mut runtime, &valid, speed_valid);
        write_lreal(&mut runtime, &actual_speed, speed);
        write_lreal(&mut runtime, &measured_speed, speed);
        write_lreal(&mut runtime, &dynamic_scale, scale);
        runtime
            .execute_cycle()
            .expect("execute feedforward controllers");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance equivalent ovens");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish oven temperatures");
        for index in 0..3 {
            result.temperatures[index].push(read_lreal(&runtime, &temperatures[index]));
            result.heater_requests[index].push(read_lreal(&runtime, &heaters[index]));
        }
        result.actual_speed.push(speed);
        result.measured_speed.push(speed);
        result.speed_quality.push(f64::from(speed_valid));
        result
            .static_feedforward
            .push(read_lreal(&runtime, &static_ff));
        result
            .dynamic_feedforward
            .push(read_lreal(&runtime, &dynamic_ff));
    }
    let _ = std::fs::remove_file(config_path);
    result
}

fn event_iae(values: &[f64]) -> f64 {
    values[300..600]
        .iter()
        .map(|value| (120.0 - value).abs() * 0.1)
        .sum()
}

fn event_peak(values: &[f64]) -> f64 {
    values[300..600]
        .iter()
        .map(|value| (120.0 - value).abs())
        .fold(0.0, f64::max)
}

fn event_travel(values: &[f64]) -> f64 {
    (300..600)
        .map(|index| (values[index] - values[index - 1]).abs())
        .sum()
}

fn span(values: &[f64]) -> f64 {
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    maximum - minimum
}

fn mean_absolute_temperature_error(values: &[f64]) -> f64 {
    values
        .iter()
        .map(|value| (120.0 - value).abs())
        .sum::<f64>()
        / values.len() as f64
}

#[test]
fn control_curriculum_feedforward_compensates_measured_oven_speed_before_feedback_error() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 53 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 53 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 53);
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_feedforward_compensates_measured_oven_speed_before_feedback_error"
    );
    let source = project_file("src/Main.st");
    assert_eq!(source.matches("Control.PID;").count(), 3);
    assert_eq!(source.matches("Control.LowPass;").count(), 1);
    let simulation = project_file("simulation.toml");
    assert_eq!(simulation.matches("model = \"state_space\"").count(), 3);

    let nominal = run_feedforward(1.0, true);
    for values in nominal
        .temperatures
        .iter()
        .chain(nominal.heater_requests.iter())
    {
        assert!(values.iter().all(|value| value.is_finite()));
    }
    assert!(nominal
        .heater_requests
        .iter()
        .flatten()
        .all(|value| (0.0..=100.0).contains(value)));

    let iae = nominal
        .temperatures
        .each_ref()
        .map(|values| event_iae(values));
    let peak = nominal
        .temperatures
        .each_ref()
        .map(|values| event_peak(values));
    let travel = nominal
        .heater_requests
        .each_ref()
        .map(|values| event_travel(values));
    let final_temperature_span = nominal
        .temperatures
        .each_ref()
        .map(|values| span(&values[800..900]));
    let final_heater_span = nominal
        .heater_requests
        .each_ref()
        .map(|values| span(&values[800..900]));
    let final_error = nominal
        .temperatures
        .each_ref()
        .map(|values| mean_absolute_temperature_error(&values[800..900]));
    println!("oven feedforward nominal IAE={iae:?}, peak={peak:?}, heater travel={travel:?}, final PV span={final_temperature_span:?}, final heater span={final_heater_span:?}, final error={final_error:?}");
    assert!(
        iae[1] < iae[0],
        "static feedforward must improve on feedback-only: {iae:?}"
    );
    assert!(
        iae[2] < iae[1],
        "dynamic feedforward must improve on static: {iae:?}"
    );
    assert!(
        peak[2] < peak[1] && peak[1] < peak[0],
        "peak ordering must follow path compensation: {peak:?}"
    );
    assert!((nominal.static_feedforward[300] - 10.0).abs() <= 1e-9);
    assert!((nominal.dynamic_feedforward[300] - 25.0).abs() <= 0.2);
    assert!((nominal.dynamic_feedforward[899] - 10.0).abs() <= 0.05);
    assert!(final_temperature_span[0] <= 0.05 && final_temperature_span[2] <= 0.05);
    assert!(
        final_temperature_span[1] > 0.05,
        "static case must be reported Not settled in the declared final window"
    );
    assert!(final_heater_span.iter().all(|value| *value <= 0.10));
    assert!(
        nominal.heater_requests[2]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max)
            < 100.0,
        "nominal dynamic result may not rely on saturation"
    );

    let mismatch = run_feedforward(0.8, true);
    let mismatch_iae = event_iae(&mismatch.temperatures[2]);
    let mismatch_final_error = mean_absolute_temperature_error(&mismatch.temperatures[2][850..900]);
    println!("oven dynamic-feedforward mismatch IAE={mismatch_iae:.6}, final error={mismatch_final_error:.6}");
    assert!(
        mismatch_iae > iae[2],
        "declared mismatch must leave visible residual error"
    );
    assert!(
        mismatch_iae < iae[0],
        "feedback trim plus mismatched feedforward must beat feedback-only"
    );
    assert!(mismatch_final_error <= 0.1);

    let invalid = run_feedforward(1.0, false);
    assert!(invalid
        .static_feedforward
        .iter()
        .all(|value| value.abs() <= 1e-12));
    assert!(invalid
        .dynamic_feedforward
        .iter()
        .all(|value| value.abs() <= 1e-12));
    assert!(invalid.temperatures[0]
        .iter()
        .zip(&invalid.temperatures[1])
        .chain(invalid.temperatures[0].iter().zip(&invalid.temperatures[2]))
        .all(|(left, right)| (left - right).abs() <= 1e-9));

    let time = time_axis(900);
    let (temperature_min, temperature_max) =
        plot_bounds(nominal.temperatures.iter().map(Vec::as_slice));
    let (heater_min, heater_max) = plot_bounds(nominal.heater_requests.iter().map(Vec::as_slice));
    let maximum_heater = nominal
        .heater_requests
        .each_ref()
        .map(|values| values.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    let saturation_duration = nominal
        .heater_requests
        .each_ref()
        .map(|values| values.iter().filter(|value| **value >= 99.999).count() as f64 * 0.1);
    let summary = format!("case,status,event_iae_degC_s,peak_deviation_degC,heater_travel_pp,final_error_degC,max_requested_heater_percent,max_applied_heater_percent,saturation_duration_s,authority,abort,restoration\nfeedback_only,qualified,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.3},automatic,false,qualified\nstatic_feedforward,not_settled,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.3},automatic,false,not_settled\ndynamic_feedforward,qualified,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.3},automatic,false,qualified\ndynamic_mismatch,qualified,{:.6},,,{:.6},,,,automatic,false,qualified\ninvalid_speed_fallback,invalid_quality_metrics_ineligible,,,,,,,feedback_only,false,tracked_fallback\n",
        iae[0], peak[0], travel[0], final_error[0], maximum_heater[0], maximum_heater[0], saturation_duration[0],
        iae[1], peak[1], travel[1], final_error[1], maximum_heater[1], maximum_heater[1], saturation_duration[1],
        iae[2], peak[2], travel[2], final_error[2], maximum_heater[2], maximum_heater[2], saturation_duration[2],
        mismatch_iae, mismatch_final_error);
    evidence::retain(
        &evidence_root(
            "09-industrial-control-structures",
            "53-conveyor-oven-feedforward",
        ),
        "conveyor-oven-feedforward-evidence",
        "Measured speed enables feedforward action",
        "Equal disturbances expose feedback delay, static mismatch, and dynamic path compensation.",
        &[
            evidence::Panel {
                title: "Product temperature",
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
                x_minimum: 0.0,
                x_maximum: 90.0,
                y_minimum: temperature_min,
                y_maximum: temperature_max,
                series: vec![
                    evidence::Series {
                        name: "Feedback only",
                        color: "#6b7280",
                        dashed: false,
                        x: &time,
                        y: &nominal.temperatures[0],
                    },
                    evidence::Series {
                        name: "Static feedforward",
                        color: "#b45309",
                        dashed: false,
                        x: &time,
                        y: &nominal.temperatures[1],
                    },
                    evidence::Series {
                        name: "Dynamic feedforward",
                        color: "#2563eb",
                        dashed: false,
                        x: &time,
                        y: &nominal.temperatures[2],
                    },
                ],
            },
            evidence::Panel {
                title: "Requested heater command",
                x_unit: "time (s)",
                y_unit: "heater (%)",
                x_minimum: 0.0,
                x_maximum: 90.0,
                y_minimum: heater_min,
                y_maximum: heater_max,
                series: vec![
                    evidence::Series {
                        name: "Feedback only",
                        color: "#6b7280",
                        dashed: false,
                        x: &time,
                        y: &nominal.heater_requests[0],
                    },
                    evidence::Series {
                        name: "Static feedforward",
                        color: "#b45309",
                        dashed: false,
                        x: &time,
                        y: &nominal.heater_requests[1],
                    },
                    evidence::Series {
                        name: "Dynamic feedforward",
                        color: "#2563eb",
                        dashed: false,
                        x: &time,
                        y: &nominal.heater_requests[2],
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Disturbance",
                name: "Actual conveyor speed",
                x: &time,
                y: &nominal.actual_speed,
                x_unit: "time (s)",
                y_unit: "speed (m/s)",
            },
            evidence::RetainedSeries {
                panel: "Disturbance",
                name: "Measured conveyor speed",
                x: &time,
                y: &nominal.measured_speed,
                x_unit: "time (s)",
                y_unit: "speed (m/s)",
            },
            evidence::RetainedSeries {
                panel: "Disturbance quality",
                name: "Speed valid",
                x: &time,
                y: &nominal.speed_quality,
                x_unit: "time (s)",
                y_unit: "valid (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Feedforward components",
                name: "Static component",
                x: &time,
                y: &nominal.static_feedforward,
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Feedforward components",
                name: "Dynamic component",
                x: &time,
                y: &nominal.dynamic_feedforward,
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Applied actuator",
                name: "Dynamic heater applied",
                x: &time,
                y: &nominal.heater_requests[2],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Mismatch temperature",
                name: "Dynamic scale 0.8",
                x: &time,
                y: &mismatch.temperatures[2],
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Invalid-quality fallback",
                name: "Dynamic feedforward",
                x: &time,
                y: &invalid.dynamic_feedforward,
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Invalid-quality fallback",
                name: "Speed valid",
                x: &time,
                y: &invalid.speed_quality,
                x_unit: "time (s)",
                y_unit: "valid (0/1)",
            },
        ],
        &summary,
    );
}

#[derive(Debug)]
struct CascadeRun {
    reactor_temperatures: [Vec<f64>; 3],
    jacket_temperatures: [Vec<f64>; 3],
    valve_requests: [Vec<f64>; 3],
    secondary_setpoints: [Vec<f64>; 2],
    commissioning_mode: Vec<f64>,
    inner_available: Vec<f64>,
    outer_auto: Vec<f64>,
    inner_auto: Vec<f64>,
    valve_saturated: Vec<f64>,
    aborted: Vec<f64>,
    restored: Vec<f64>,
}

fn run_cascade(commissioning: bool, exercise_authority_loss: bool) -> CascadeRun {
    let shipped_source = cascade_project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("cascade lesson program before configuration")
        .replace(
            "InnerCommissioning : BOOL :=",
            "InnerCommissioning AT %MX88.0 : BOOL :=",
        )
        .replace(
            "InnerAvailable : BOOL :=",
            "InnerAvailable AT %MX88.1 : BOOL :=",
        )
        .replace(
            "JacketTestSetpoint : LREAL :=",
            "JacketTestSetpoint AT %ML96 : LREAL :=",
        )
        .replace(
            "UtilityDisturbance : LREAL :=",
            "UtilityDisturbance AT %ML104 : LREAL :=",
        )
        .replace(
            "FastJacketSetpoint : LREAL :=",
            "FastJacketSetpoint AT %ML112 : LREAL :=",
        )
        .replace(
            "SlowJacketSetpoint : LREAL :=",
            "SlowJacketSetpoint AT %ML120 : LREAL :=",
        )
        .replace(
            "DirectReactorTemperature : LREAL :=",
            "DirectReactorTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "DirectJacketTemperature : LREAL :=",
            "DirectJacketTemperature AT %IL8 : LREAL :=",
        )
        .replace(
            "FastReactorTemperature : LREAL :=",
            "FastReactorTemperature AT %IL16 : LREAL :=",
        )
        .replace(
            "FastJacketTemperature : LREAL :=",
            "FastJacketTemperature AT %IL24 : LREAL :=",
        )
        .replace(
            "SlowReactorTemperature : LREAL :=",
            "SlowReactorTemperature AT %IL32 : LREAL :=",
        )
        .replace(
            "SlowJacketTemperature : LREAL :=",
            "SlowJacketTemperature AT %IL40 : LREAL :=",
        )
        .replace(
            "DirectValveRequest : LREAL :=",
            "DirectValveRequest AT %QL0 : LREAL :=",
        )
        .replace(
            "DirectUtilityInput : LREAL :=",
            "DirectUtilityInput AT %QL8 : LREAL :=",
        )
        .replace(
            "FastValveRequest : LREAL :=",
            "FastValveRequest AT %QL16 : LREAL :=",
        )
        .replace(
            "FastUtilityInput : LREAL :=",
            "FastUtilityInput AT %QL24 : LREAL :=",
        )
        .replace(
            "SlowValveRequest : LREAL :=",
            "SlowValveRequest AT %QL32 : LREAL :=",
        )
        .replace(
            "SlowUtilityInput : LREAL :=",
            "SlowUtilityInput AT %QL40 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("reactor-cascade/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped cascade lesson");
    let config_path = temporary_cascade_config_path();
    std::fs::write(&config_path, cascade_project_file("simulation.toml"))
        .expect("write cascade simulation config");
    let config = SimulationConfig::load(&config_path).expect("load three equivalent reactors");
    assert_eq!(config.plant_system.plants.len(), 3);
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial reactor states");

    let commissioning_address = IoAddress::parse("%MX88.0").unwrap();
    let available = IoAddress::parse("%MX88.1").unwrap();
    let jacket_test = IoAddress::parse("%ML96").unwrap();
    let utility = IoAddress::parse("%ML104").unwrap();
    let reactor_addresses =
        ["%IL0", "%IL16", "%IL32"].map(|value| IoAddress::parse(value).unwrap());
    let jacket_addresses = ["%IL8", "%IL24", "%IL40"].map(|value| IoAddress::parse(value).unwrap());
    let valve_addresses = ["%QL0", "%QL16", "%QL32"].map(|value| IoAddress::parse(value).unwrap());
    let secondary_addresses = ["%ML112", "%ML120"].map(|value| IoAddress::parse(value).unwrap());
    let cycles = if commissioning { 400 } else { 1_200 };
    let mut result = CascadeRun {
        reactor_temperatures: std::array::from_fn(|_| Vec::with_capacity(cycles)),
        jacket_temperatures: std::array::from_fn(|_| Vec::with_capacity(cycles)),
        valve_requests: std::array::from_fn(|_| Vec::with_capacity(cycles)),
        secondary_setpoints: std::array::from_fn(|_| Vec::with_capacity(cycles)),
        commissioning_mode: Vec::with_capacity(cycles),
        inner_available: Vec::with_capacity(cycles),
        outer_auto: Vec::with_capacity(cycles),
        inner_auto: Vec::with_capacity(cycles),
        valve_saturated: Vec::with_capacity(cycles),
        aborted: Vec::with_capacity(cycles),
        restored: Vec::with_capacity(cycles),
    };
    for cycle in 1usize..=cycles {
        let inner_available = !(exercise_authority_loss && (500..600).contains(&cycle));
        write_bool(&mut runtime, &commissioning_address, commissioning);
        write_bool(&mut runtime, &available, inner_available);
        write_lreal(
            &mut runtime,
            &jacket_test,
            if commissioning && cycle > 100 {
                75.0
            } else {
                70.0
            },
        );
        write_lreal(
            &mut runtime,
            &utility,
            if !commissioning && cycle > 400 {
                -10.0
            } else {
                0.0
            },
        );
        runtime
            .execute_cycle()
            .expect("execute direct and cascade controllers");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance equivalent reactors");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish reactor measurements");
        for index in 0..3 {
            result.reactor_temperatures[index]
                .push(read_lreal(&runtime, &reactor_addresses[index]));
            result.jacket_temperatures[index].push(read_lreal(&runtime, &jacket_addresses[index]));
            result.valve_requests[index].push(read_lreal(&runtime, &valve_addresses[index]));
        }
        for index in 0..2 {
            result.secondary_setpoints[index]
                .push(read_lreal(&runtime, &secondary_addresses[index]));
        }
        result.commissioning_mode.push(f64::from(commissioning));
        result.inner_available.push(f64::from(inner_available));
        result
            .outer_auto
            .push(f64::from(!commissioning && inner_available));
        result.inner_auto.push(f64::from(inner_available));
        result.valve_saturated.push(f64::from(
            result.valve_requests[1]
                .last()
                .is_some_and(|value| *value <= 0.001 || *value >= 99.999),
        ));
        result.aborted.push(0.0);
        result.restored.push(f64::from(
            exercise_authority_loss && cycle >= 600 && inner_available,
        ));
    }
    let _ = std::fs::remove_file(config_path);
    result
}

fn first_threshold(values: &[f64], baseline: f64, start: usize, threshold: f64) -> usize {
    values[start..]
        .iter()
        .position(|value| (value - baseline).abs() >= threshold)
        .map(|offset| start + offset)
        .expect("trace reaches declared threshold")
}

fn settling_time(values: &[f64], target: f64, start: usize, band: f64) -> f64 {
    (start..values.len())
        .find(|index| {
            values[*index..]
                .iter()
                .all(|value| (value - target).abs() <= band)
        })
        .map(|index| (index - start) as f64 * 0.1)
        .unwrap_or(f64::INFINITY)
}

#[test]
fn control_curriculum_cascade_commissions_inner_loop_before_rejecting_reactor_utility_disturbance()
{
    let lesson: serde_json::Value = serde_json::from_str(&cascade_project_file("lesson.json"))
        .expect("parse Lesson 54 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 54 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 54);
    assert_eq!(lesson["proof"]["nativeTest"],
        "control_curriculum_cascade_commissions_inner_loop_before_rejecting_reactor_utility_disturbance");
    let source = cascade_project_file("src/Main.st");
    assert_eq!(source.matches("Control.PID;").count(), 5);
    assert_eq!(
        cascade_project_file("simulation.toml")
            .matches("model = \"state_space\"")
            .count(),
        3
    );

    let commissioning = run_cascade(true, false);
    let fast_inner_settling = settling_time(&commissioning.jacket_temperatures[1], 75.0, 100, 0.1);
    let slow_inner_settling = settling_time(&commissioning.jacket_temperatures[2], 75.0, 100, 0.1);
    println!("reactor inner commissioning settling fast={fast_inner_settling:.3}s slow={slow_inner_settling:.3}s");
    assert!(fast_inner_settling.is_finite());
    assert!(
        slow_inner_settling >= fast_inner_settling * 3.0,
        "intentionally slow inner loop must fail the three-times separation rule"
    );

    let regulatory = run_cascade(false, false);
    for values in regulatory
        .reactor_temperatures
        .iter()
        .chain(regulatory.jacket_temperatures.iter())
        .chain(regulatory.valve_requests.iter())
    {
        assert!(values.iter().all(|value| value.is_finite()));
    }
    assert!(regulatory
        .valve_requests
        .iter()
        .flatten()
        .all(|value| (0.0..=100.0).contains(value)));
    let jacket_detect = first_threshold(&regulatory.jacket_temperatures[1], 70.0, 400, 0.1);
    let reactor_detect = first_threshold(&regulatory.reactor_temperatures[1], 80.0, 400, 0.1);
    let detection_lead = (reactor_detect - jacket_detect) as f64 * 0.1;
    let iae = regulatory.reactor_temperatures.each_ref().map(|values| {
        values[400..900]
            .iter()
            .map(|value| (80.0 - value).abs() * 0.1)
            .sum::<f64>()
    });
    let peak = regulatory.reactor_temperatures.each_ref().map(|values| {
        values[400..900]
            .iter()
            .map(|value| (80.0 - value).abs())
            .fold(0.0, f64::max)
    });
    let travel = regulatory.valve_requests.each_ref().map(|values| {
        (400..900)
            .map(|index| (values[index] - values[index - 1]).abs())
            .sum::<f64>()
    });
    println!("reactor utility event detection lead={detection_lead:.3}s, IAE={iae:?}, peak={peak:?}, valve travel={travel:?}");
    assert!(
        detection_lead > 0.0,
        "jacket must reveal utility loss before reactor product"
    );
    assert!(
        iae[1] < iae[0] * 0.70,
        "commissioned cascade must reject more than direct PI: {iae:?}"
    );
    assert!(peak[1] < peak[0]);
    assert!(
        iae[2] > iae[1],
        "slow inner loop must lose cascade benefit: {iae:?}"
    );
    assert!(regulatory
        .secondary_setpoints
        .iter()
        .flatten()
        .all(|value| (60.0..=85.0).contains(value)));

    let authority = run_cascade(false, true);
    let held_secondary_setpoint = authority.secondary_setpoints[0][498];
    let maximum_tracking_error = authority.secondary_setpoints[0][499..599]
        .iter()
        .map(|value| (value - held_secondary_setpoint).abs())
        .fold(0.0, f64::max);
    assert!(
        maximum_tracking_error <= 1e-9,
        "loss of inner-loop authority must hold the outer controller's secondary setpoint; tracking error={maximum_tracking_error}"
    );
    let held_valve = authority.valve_requests[1][498];
    let maximum_valve_tracking_error = authority.valve_requests[1][499..599]
        .iter()
        .map(|value| (value - held_valve).abs())
        .fold(0.0, f64::max);
    assert!(
        maximum_valve_tracking_error <= 1e-9,
        "loss of inner-loop authority must hold the last applied valve request; tracking error={maximum_valve_tracking_error}"
    );
    let restoration_bump =
        (authority.valve_requests[1][599] - authority.valve_requests[1][598]).abs();
    assert!(
        restoration_bump <= 1.0,
        "restored inner authority must be bumpless within 1 pp; bump={restoration_bump}"
    );

    let time = time_axis(regulatory.reactor_temperatures[0].len());
    let (reactor_min, reactor_max) =
        plot_bounds(regulatory.reactor_temperatures.iter().map(Vec::as_slice));
    let (valve_min, valve_max) = plot_bounds(regulatory.valve_requests.iter().map(Vec::as_slice));
    let secondary_min = regulatory.secondary_setpoints[0]
        .iter()
        .copied()
        .fold(f64::INFINITY, f64::min);
    let secondary_max = regulatory.secondary_setpoints[0]
        .iter()
        .copied()
        .fold(f64::NEG_INFINITY, f64::max);
    let saturation_duration = regulatory.valve_saturated.iter().sum::<f64>() * 0.1;
    let summary = format!("case,status,inner_settling_s,event_iae_degC_s,peak_deviation_degC,valve_travel_pp,secondary_setpoint_min_degC,secondary_setpoint_max_degC,valve_saturation_duration_s,authority,secondary_tracking_error_degC,valve_tracking_error_pp,restoration_bump_pp,abort,restoration\ndirect_pi,qualified,,{:.6},{:.6},{:.6},,,{:.3},automatic,,,,false,qualified\ncommissioned_cascade,qualified,,{:.6},{:.6},{:.6},{:.6},{:.6},{:.3},automatic,,,,false,qualified\nslow_inner_cascade,rejected,,{:.6},{:.6},{:.6},,,{:.3},automatic,,,,false,qualified\ninner_fast_settling,qualified,{:.6},,,,,,,commissioning,,,,false,qualified\ninner_slow_settling,rejected,{:.6},,,,,,,commissioning,,,,false,qualified\nauthority_loss,qualified,,,,,,,,tracking,{:.9},{:.9},{:.6},false,qualified\n",
        iae[0], peak[0], travel[0], saturation_duration,
        iae[1], peak[1], travel[1], secondary_min, secondary_max, saturation_duration,
        iae[2], peak[2], travel[2], saturation_duration,
        fast_inner_settling, slow_inner_settling, maximum_tracking_error,
        maximum_valve_tracking_error, restoration_bump);
    evidence::retain(
        &evidence_root("09-industrial-control-structures", "54-reactor-cascade"),
        "reactor-cascade-evidence",
        "A fast inner loop rejects utility disturbances first",
        "Inner-loop commissioning separates useful cascade action from a slow nested loop.",
        &[
            evidence::Panel {
                title: "Reactor temperature",
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: reactor_min,
                y_maximum: reactor_max,
                series: vec![
                    evidence::Series {
                        name: "Direct PI",
                        color: "#6b7280",
                        dashed: false,
                        x: &time,
                        y: &regulatory.reactor_temperatures[0],
                    },
                    evidence::Series {
                        name: "Commissioned cascade",
                        color: "#2563eb",
                        dashed: false,
                        x: &time,
                        y: &regulatory.reactor_temperatures[1],
                    },
                    evidence::Series {
                        name: "Slow inner loop",
                        color: "#b45309",
                        dashed: false,
                        x: &time,
                        y: &regulatory.reactor_temperatures[2],
                    },
                ],
            },
            evidence::Panel {
                title: "Valve request",
                x_unit: "time (s)",
                y_unit: "valve (%)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: valve_min,
                y_maximum: valve_max,
                series: vec![
                    evidence::Series {
                        name: "Direct PI",
                        color: "#6b7280",
                        dashed: false,
                        x: &time,
                        y: &regulatory.valve_requests[0],
                    },
                    evidence::Series {
                        name: "Commissioned cascade",
                        color: "#2563eb",
                        dashed: false,
                        x: &time,
                        y: &regulatory.valve_requests[1],
                    },
                    evidence::Series {
                        name: "Slow inner loop",
                        color: "#b45309",
                        dashed: false,
                        x: &time,
                        y: &regulatory.valve_requests[2],
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Commissioning",
                name: "Fast jacket temperature",
                x: &time_axis(commissioning.jacket_temperatures[1].len()),
                y: &commissioning.jacket_temperatures[1],
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Commissioning",
                name: "Slow jacket temperature",
                x: &time_axis(commissioning.jacket_temperatures[2].len()),
                y: &commissioning.jacket_temperatures[2],
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Cascade monitor",
                name: "Fast jacket temperature",
                x: &time,
                y: &regulatory.jacket_temperatures[1],
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Cascade monitor",
                name: "Fast jacket setpoint",
                x: &time,
                y: &regulatory.secondary_setpoints[0],
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Applied actuator",
                name: "Commissioned valve applied",
                x: &time,
                y: &regulatory.valve_requests[1],
                x_unit: "time (s)",
                y_unit: "valve (%)",
            },
            evidence::RetainedSeries {
                panel: "Operating state",
                name: "Commissioning mode",
                x: &time,
                y: &regulatory.commissioning_mode,
                x_unit: "time (s)",
                y_unit: "active (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Operating state",
                name: "Outer automatic",
                x: &time,
                y: &regulatory.outer_auto,
                x_unit: "time (s)",
                y_unit: "active (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Operating state",
                name: "Inner automatic",
                x: &time,
                y: &regulatory.inner_auto,
                x_unit: "time (s)",
                y_unit: "active (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Constraints",
                name: "Commissioned valve saturated",
                x: &time,
                y: &regulatory.valve_saturated,
                x_unit: "time (s)",
                y_unit: "active (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Qualification",
                name: "Aborted",
                x: &time,
                y: &regulatory.aborted,
                x_unit: "time (s)",
                y_unit: "active (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Authority-loss case",
                name: "Inner available",
                x: &time,
                y: &authority.inner_available,
                x_unit: "time (s)",
                y_unit: "available (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Authority-loss case",
                name: "Outer automatic",
                x: &time,
                y: &authority.outer_auto,
                x_unit: "time (s)",
                y_unit: "active (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Authority-loss case",
                name: "Fast secondary setpoint",
                x: &time,
                y: &authority.secondary_setpoints[0],
                x_unit: "time (s)",
                y_unit: "temperature (degC)",
            },
            evidence::RetainedSeries {
                panel: "Authority-loss case",
                name: "Fast valve applied",
                x: &time,
                y: &authority.valve_requests[1],
                x_unit: "time (s)",
                y_unit: "valve (%)",
            },
            evidence::RetainedSeries {
                panel: "Authority-loss case",
                name: "Restored",
                x: &time,
                y: &authority.restored,
                x_unit: "time (s)",
                y_unit: "qualified (0/1)",
            },
        ],
        &summary,
    );
}

fn ratio_project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/55-blending-ratio")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 55 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 55 file {}: {error}", path.display()))
}

#[derive(Debug)]
struct RatioRun {
    blend_ratios: [Vec<f64>; 2],
    additive_requests: [Vec<f64>; 2],
    ratio_demand: Vec<f64>,
    ratio_trim: Vec<f64>,
    actual_wild_flow: Vec<f64>,
    measured_wild_flow: Vec<f64>,
    quality_valid: Vec<f64>,
    flow_qualified: Vec<f64>,
}

fn run_ratio(measurement_bias: f64, valid: bool, low_flow: bool) -> RatioRun {
    let shipped_source = ratio_project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("ratio lesson program before configuration")
        .replace(
            "WildFlowValid : BOOL :=",
            "WildFlowValid AT %MX88.2 : BOOL :=",
        )
        .replace(
            "ActualWildFlow : LREAL :=",
            "ActualWildFlow AT %ML128 : LREAL :=",
        )
        .replace(
            "MeasuredWildFlow : LREAL :=",
            "MeasuredWildFlow AT %ML136 : LREAL :=",
        )
        .replace("RatioDemand : LREAL :=", "RatioDemand AT %ML144 : LREAL :=")
        .replace("RatioTrim : LREAL :=", "RatioTrim AT %ML152 : LREAL :=")
        .replace(
            "FeedbackRatio : LREAL :=",
            "FeedbackRatio AT %IL48 : LREAL :=",
        )
        .replace(
            "RatioControlledRatio : LREAL :=",
            "RatioControlledRatio AT %IL56 : LREAL :=",
        )
        .replace(
            "FeedbackAdditiveRequest : LREAL :=",
            "FeedbackAdditiveRequest AT %QL48 : LREAL :=",
        )
        .replace(
            "FeedbackWildFlowInput : LREAL :=",
            "FeedbackWildFlowInput AT %QL56 : LREAL :=",
        )
        .replace(
            "RatioAdditiveRequest : LREAL :=",
            "RatioAdditiveRequest AT %QL64 : LREAL :=",
        )
        .replace(
            "RatioWildFlowInput : LREAL :=",
            "RatioWildFlowInput AT %QL72 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("blending-ratio/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped ratio-control lesson");
    let config_path = temporary_cascade_config_path();
    std::fs::write(&config_path, ratio_project_file("simulation.toml"))
        .expect("write ratio-control simulation config");
    let config = SimulationConfig::load(&config_path).expect("load two equivalent blending plants");
    assert_eq!(config.plant_system.plants.len(), 2);
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial blend ratios");

    let quality_address = IoAddress::parse("%MX88.2").unwrap();
    let actual_flow_address = IoAddress::parse("%ML128").unwrap();
    let measured_flow_address = IoAddress::parse("%ML136").unwrap();
    let ratio_addresses = ["%IL48", "%IL56"].map(|value| IoAddress::parse(value).unwrap());
    let additive_addresses = ["%QL48", "%QL64"].map(|value| IoAddress::parse(value).unwrap());
    let ratio_demand_address = IoAddress::parse("%ML144").unwrap();
    let ratio_trim_address = IoAddress::parse("%ML152").unwrap();
    let mut result = RatioRun {
        blend_ratios: std::array::from_fn(|_| Vec::with_capacity(900)),
        additive_requests: std::array::from_fn(|_| Vec::with_capacity(900)),
        ratio_demand: Vec::with_capacity(900),
        ratio_trim: Vec::with_capacity(900),
        actual_wild_flow: Vec::with_capacity(900),
        measured_wild_flow: Vec::with_capacity(900),
        quality_valid: Vec::with_capacity(900),
        flow_qualified: Vec::with_capacity(900),
    };
    for cycle in 1usize..=900 {
        let event_flow = if low_flow { 5.0 } else { 120.0 };
        let actual_flow = if cycle > 300 { event_flow } else { 100.0 };
        let measured_flow = if cycle > 300 {
            event_flow + measurement_bias
        } else {
            100.0
        };
        write_bool(&mut runtime, &quality_address, valid);
        write_lreal(&mut runtime, &actual_flow_address, actual_flow);
        write_lreal(&mut runtime, &measured_flow_address, measured_flow);
        runtime
            .execute_cycle()
            .expect("execute feedback-only and ratio-trim controllers");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance equivalent blending plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish blend-ratio measurements");
        for index in 0..2 {
            result.blend_ratios[index].push(read_lreal(&runtime, &ratio_addresses[index]));
            result.additive_requests[index].push(read_lreal(&runtime, &additive_addresses[index]));
        }
        result
            .ratio_demand
            .push(read_lreal(&runtime, &ratio_demand_address));
        result
            .ratio_trim
            .push(read_lreal(&runtime, &ratio_trim_address));
        result.actual_wild_flow.push(actual_flow);
        result.measured_wild_flow.push(measured_flow);
        result.quality_valid.push(f64::from(valid));
        result
            .flow_qualified
            .push(f64::from(valid && measured_flow >= 10.0));
    }
    let _ = std::fs::remove_file(config_path);
    result
}

#[test]
fn control_curriculum_ratio_station_moves_with_wild_flow_and_fails_safe_at_low_flow() {
    let lesson: serde_json::Value = serde_json::from_str(&ratio_project_file("lesson.json"))
        .expect("parse Lesson 55 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 55 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 55);
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_ratio_station_moves_with_wild_flow_and_fails_safe_at_low_flow"
    );
    assert_eq!(
        ratio_project_file("src/Main.st")
            .matches("Control.PID;")
            .count(),
        2
    );
    assert_eq!(
        ratio_project_file("simulation.toml")
            .matches("model = \"state_space\"")
            .count(),
        2
    );

    let nominal = run_ratio(0.0, true, false);
    assert!((nominal.ratio_demand[300] - 36.0).abs() <= 1e-9);
    assert!(
        (nominal.additive_requests[1][300] - 36.0).abs() <= 0.05,
        "ratio demand must reach the valve before downstream composition changes"
    );
    let iae = nominal.blend_ratios.each_ref().map(|values| {
        values[300..600]
            .iter()
            .map(|value| (0.30 - value).abs() * 0.1)
            .sum::<f64>()
    });
    let peak = nominal.blend_ratios.each_ref().map(|values| {
        values[300..600]
            .iter()
            .map(|value| (0.30 - value).abs())
            .fold(0.0, f64::max)
    });
    println!("blend production event IAE={iae:?}, peak ratio deviation={peak:?}");
    assert!(
        iae[1] < iae[0] * 0.35,
        "ratio plus trim must act before feedback-only: {iae:?}"
    );
    assert!(peak[1] < peak[0]);
    assert!(nominal
        .additive_requests
        .iter()
        .flatten()
        .all(|value| value.is_finite() && (0.0..=60.0).contains(value)));

    let biased = run_ratio(5.0, true, false);
    let final_error = biased.blend_ratios[1][800..900]
        .iter()
        .map(|value| (0.30 - value).abs())
        .sum::<f64>()
        / 100.0;
    let final_trim = biased.ratio_trim[800..900].iter().sum::<f64>() / 100.0;
    println!("blend +5 L/min meter bias final error={final_error:.6}, trim={final_trim:.6} L/min");
    assert!(final_error < 0.002);
    assert!((-1.7..=-1.3).contains(&final_trim));

    let invalid = run_ratio(0.0, false, false);
    let low_flow = run_ratio(0.0, true, true);
    println!(
        "blend fallback invalid demand/request max={:.12}/{:.12}, low-flow demand/request max={:.12}/{:.12}",
        invalid.ratio_demand[300..].iter().copied().map(f64::abs).fold(0.0, f64::max),
        invalid.additive_requests[1][300..].iter().copied().map(f64::abs).fold(0.0, f64::max),
        low_flow.ratio_demand[300..].iter().copied().map(f64::abs).fold(0.0, f64::max),
        low_flow.additive_requests[1][300..].iter().copied().map(f64::abs).fold(0.0, f64::max),
    );
    assert!(invalid.ratio_demand[300..]
        .iter()
        .all(|value| value.abs() <= 1e-12));
    assert!(low_flow.ratio_demand[300..]
        .iter()
        .all(|value| value.abs() <= 1e-12));
    assert!(invalid.additive_requests[1][300..]
        .iter()
        .all(|value| value.abs() <= 1e-12));
    assert!(low_flow.additive_requests[1][300..]
        .iter()
        .all(|value| value.abs() <= 1e-12));

    let time = time_axis(nominal.blend_ratios[0].len());
    let (ratio_min, ratio_max) = plot_bounds(nominal.blend_ratios.iter().map(Vec::as_slice));
    let (flow_min, flow_max) = plot_bounds(nominal.additive_requests.iter().map(Vec::as_slice));
    let saturation_duration = nominal
        .additive_requests
        .each_ref()
        .map(|values| values.iter().filter(|value| **value >= 59.999).count() as f64 * 0.1);
    let summary = format!("case,status,event_iae_ratio_s,peak_ratio_deviation,first_scan_ratio_demand_L_min,final_error,final_trim_L_min,saturation_duration_s,authority,abort,restoration\nfeedback_only,qualified,{:.6},{:.6},0.000000,,,{:.3},automatic,false,qualified\nratio_plus_trim,qualified,{:.6},{:.6},{:.6},,,{:.3},automatic,false,qualified\nmeter_bias,qualified,,,,{:.6},{:.6},0.000,automatic,false,qualified\ninvalid_quality_fallback,invalid_quality_metrics_ineligible,,,,,,0.000,unavailable,false,tracked_zero\nlow_flow_fallback,low_flow_metrics_ineligible,,,,,,0.000,unavailable,false,tracked_zero\n",
        iae[0], peak[0], saturation_duration[0], iae[1], peak[1], nominal.ratio_demand[300],
        saturation_duration[1], final_error, final_trim);
    evidence::retain(
        &evidence_root("09-industrial-control-structures", "55-blending-ratio"),
        "blending-ratio-evidence",
        "Ratio action moves with the wild stream",
        "Feed-ratio action prevents the composition error that feedback must otherwise correct.",
        &[
            evidence::Panel {
                title: "Blend ratio",
                x_unit: "time (s)",
                y_unit: "additive/feed ratio",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: ratio_min,
                y_maximum: ratio_max,
                series: vec![
                    evidence::Series {
                        name: "Feedback only",
                        color: "#6b7280",
                        dashed: false,
                        x: &time,
                        y: &nominal.blend_ratios[0],
                    },
                    evidence::Series {
                        name: "Ratio plus trim",
                        color: "#2563eb",
                        dashed: false,
                        x: &time,
                        y: &nominal.blend_ratios[1],
                    },
                ],
            },
            evidence::Panel {
                title: "Additive request — ratio demand green dashed",
                x_unit: "time (s)",
                y_unit: "flow (L/min)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: flow_min,
                y_maximum: flow_max,
                series: vec![
                    evidence::Series {
                        name: "Feedback only",
                        color: "#6b7280",
                        dashed: false,
                        x: &time,
                        y: &nominal.additive_requests[0],
                    },
                    evidence::Series {
                        name: "Ratio plus trim",
                        color: "#2563eb",
                        dashed: false,
                        x: &time,
                        y: &nominal.additive_requests[1],
                    },
                    evidence::Series {
                        name: "Ratio demand",
                        color: "#059669",
                        dashed: true,
                        x: &time,
                        y: &nominal.ratio_demand,
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Wild stream",
                name: "Actual wild flow",
                x: &time,
                y: &nominal.actual_wild_flow,
                x_unit: "time (s)",
                y_unit: "flow (L/min)",
            },
            evidence::RetainedSeries {
                panel: "Wild stream",
                name: "Measured wild flow",
                x: &time,
                y: &nominal.measured_wild_flow,
                x_unit: "time (s)",
                y_unit: "flow (L/min)",
            },
            evidence::RetainedSeries {
                panel: "Ratio authority",
                name: "Quality valid",
                x: &time,
                y: &nominal.quality_valid,
                x_unit: "time (s)",
                y_unit: "valid (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Applied additive flow",
                name: "Ratio plus trim applied",
                x: &time,
                y: &nominal.additive_requests[1],
                x_unit: "time (s)",
                y_unit: "flow (L/min)",
            },
            evidence::RetainedSeries {
                panel: "Meter-bias trim",
                name: "Analyzer trim",
                x: &time,
                y: &biased.ratio_trim,
                x_unit: "time (s)",
                y_unit: "flow (L/min)",
            },
            evidence::RetainedSeries {
                panel: "Invalid-quality fallback",
                name: "Authority available",
                x: &time,
                y: &invalid.flow_qualified,
                x_unit: "time (s)",
                y_unit: "available (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Invalid-quality fallback",
                name: "Applied additive flow",
                x: &time,
                y: &invalid.additive_requests[1],
                x_unit: "time (s)",
                y_unit: "flow (L/min)",
            },
            evidence::RetainedSeries {
                panel: "Low-flow fallback",
                name: "Authority available",
                x: &time,
                y: &low_flow.flow_qualified,
                x_unit: "time (s)",
                y_unit: "available (0/1)",
            },
            evidence::RetainedSeries {
                panel: "Low-flow fallback",
                name: "Ratio demand",
                x: &time,
                y: &low_flow.ratio_demand,
                x_unit: "time (s)",
                y_unit: "flow (L/min)",
            },
        ],
        &summary,
    );
}

fn split_range_project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/09-industrial-control-structures/56-reactor-split-range")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 56 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 56 file {}: {error}", path.display()))
}

#[derive(Debug)]
struct SplitRangeRun {
    temperatures: [Vec<f64>; 3],
    demands: [Vec<f64>; 3],
    heating: [Vec<f64>; 3],
    cooling: [Vec<f64>; 3],
}

fn run_split_range(automatic_available: bool, cycles: usize) -> SplitRangeRun {
    let shipped_source = split_range_project_file("src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("split-range lesson program before configuration")
        .replace(
            "AutomaticAvailable : BOOL :=",
            "AutomaticAvailable AT %MX88.4 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %ML160 : LREAL :=")
        .replace(
            "DeadZoneDemand : LREAL :=",
            "DeadZoneDemand AT %ML168 : LREAL :=",
        )
        .replace(
            "OverlapDemand : LREAL :=",
            "OverlapDemand AT %ML176 : LREAL :=",
        )
        .replace(
            "CharacterizedDemand : LREAL :=",
            "CharacterizedDemand AT %ML184 : LREAL :=",
        )
        .replace(
            "DeadZoneTemperature : LREAL :=",
            "DeadZoneTemperature AT %IL64 : LREAL :=",
        )
        .replace(
            "OverlapTemperature : LREAL :=",
            "OverlapTemperature AT %IL72 : LREAL :=",
        )
        .replace(
            "CharacterizedTemperature : LREAL :=",
            "CharacterizedTemperature AT %IL80 : LREAL :=",
        )
        .replace(
            "DeadZoneHeatingRequest : LREAL :=",
            "DeadZoneHeatingRequest AT %QL80 : LREAL :=",
        )
        .replace(
            "DeadZoneCoolingRequest : LREAL :=",
            "DeadZoneCoolingRequest AT %QL88 : LREAL :=",
        )
        .replace(
            "OverlapHeatingRequest : LREAL :=",
            "OverlapHeatingRequest AT %QL96 : LREAL :=",
        )
        .replace(
            "OverlapCoolingRequest : LREAL :=",
            "OverlapCoolingRequest AT %QL104 : LREAL :=",
        )
        .replace(
            "CharacterizedHeatingRequest : LREAL :=",
            "CharacterizedHeatingRequest AT %QL112 : LREAL :=",
        )
        .replace(
            "CharacterizedCoolingRequest : LREAL :=",
            "CharacterizedCoolingRequest AT %QL120 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("reactor-split-range/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped split-range lesson");
    let config_path = temporary_cascade_config_path();
    std::fs::write(&config_path, split_range_project_file("simulation.toml"))
        .expect("write split-range simulation config");
    let config =
        SimulationConfig::load(&config_path).expect("load three equivalent split-range reactors");
    assert_eq!(config.plant_system.plants.len(), 3);
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial split-range temperatures");

    let available_address = IoAddress::parse("%MX88.4").unwrap();
    let setpoint_address = IoAddress::parse("%ML160").unwrap();
    let temperature_addresses =
        ["%IL64", "%IL72", "%IL80"].map(|value| IoAddress::parse(value).unwrap());
    let demand_addresses =
        ["%ML168", "%ML176", "%ML184"].map(|value| IoAddress::parse(value).unwrap());
    let heating_addresses =
        ["%QL80", "%QL96", "%QL112"].map(|value| IoAddress::parse(value).unwrap());
    let cooling_addresses =
        ["%QL88", "%QL104", "%QL120"].map(|value| IoAddress::parse(value).unwrap());
    let mut result = SplitRangeRun {
        temperatures: std::array::from_fn(|_| Vec::with_capacity(cycles)),
        demands: std::array::from_fn(|_| Vec::with_capacity(cycles)),
        heating: std::array::from_fn(|_| Vec::with_capacity(cycles)),
        cooling: std::array::from_fn(|_| Vec::with_capacity(cycles)),
    };
    for cycle in 1usize..=cycles {
        let setpoint = if cycle <= 200 {
            80.0
        } else if cycle <= 800 {
            84.0
        } else if cycle <= 1_400 {
            76.0
        } else {
            80.0
        };
        write_bool(&mut runtime, &available_address, automatic_available);
        write_lreal(&mut runtime, &setpoint_address, setpoint);
        runtime
            .execute_cycle()
            .expect("execute three signed PI and split-range mappings");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance equivalent split-range reactors");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish split-range temperatures");
        for index in 0..3 {
            result.temperatures[index].push(read_lreal(&runtime, &temperature_addresses[index]));
            result.demands[index].push(read_lreal(&runtime, &demand_addresses[index]));
            result.heating[index].push(read_lreal(&runtime, &heating_addresses[index]));
            result.cooling[index].push(read_lreal(&runtime, &cooling_addresses[index]));
        }
    }
    let _ = std::fs::remove_file(config_path);
    result
}

#[test]
fn control_curriculum_split_range_exposes_gap_overlap_and_characterized_crossover() {
    let lesson: serde_json::Value = serde_json::from_str(&split_range_project_file("lesson.json"))
        .expect("parse Lesson 56 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 56 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 56);
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_split_range_exposes_gap_overlap_and_characterized_crossover"
    );
    assert_eq!(
        split_range_project_file("src/Main.st")
            .matches("Control.PID;")
            .count(),
        3
    );
    assert_eq!(
        split_range_project_file("simulation.toml")
            .matches("model = \"state_space\"")
            .count(),
        3
    );

    let run = run_split_range(true, 2_000);
    for values in run
        .temperatures
        .iter()
        .chain(run.demands.iter())
        .chain(run.heating.iter())
        .chain(run.cooling.iter())
    {
        assert!(values.iter().all(|value| value.is_finite()));
    }
    assert!(run
        .heating
        .iter()
        .chain(run.cooling.iter())
        .flatten()
        .all(|value| (0.0..=100.0).contains(value)));
    assert!((run.heating[0][0]).abs() <= 1e-12 && (run.cooling[0][0]).abs() <= 1e-12);
    assert!((run.heating[1][0] - 4.0).abs() <= 1e-12 && (run.cooling[1][0] - 4.0).abs() <= 1e-12);
    assert!((run.heating[2][0]).abs() <= 1e-12 && (run.cooling[2][0]).abs() <= 1e-12);

    let simultaneous = run
        .heating
        .each_ref()
        .iter()
        .zip(run.cooling.each_ref())
        .map(|(heat, cool)| {
            heat.iter()
                .zip(cool)
                .filter(|(h, c)| **h > 0.1 && **c > 0.1)
                .count()
        })
        .collect::<Vec<_>>();
    let gap = run
        .demands
        .each_ref()
        .iter()
        .zip(run.heating.each_ref())
        .zip(run.cooling.each_ref())
        .map(|((demand, heat), cool)| {
            demand
                .iter()
                .zip(heat)
                .zip(cool)
                .filter(|((d, h), c)| d.abs() >= 0.25 && **h <= 0.1 && **c <= 0.1)
                .count()
        })
        .collect::<Vec<_>>();
    let throughput = run
        .heating
        .each_ref()
        .iter()
        .zip(run.cooling.each_ref())
        .map(|(heat, cool)| {
            heat.iter()
                .zip(cool)
                .map(|(h, c)| (h + c) * 0.1)
                .sum::<f64>()
        })
        .collect::<Vec<_>>();
    println!("split-range simultaneous samples={simultaneous:?}, gap samples={gap:?}, utility throughput={throughput:?} percent*s");
    assert!(gap[0] > 0 && gap[2] == 0);
    assert!(simultaneous[1] > 0 && simultaneous[2] == 0);
    assert!(throughput[1] > throughput[2]);
    assert!(run.demands[2]
        .iter()
        .zip(&run.cooling[2])
        .filter(|(demand, _)| **demand < -1.0)
        .all(|(demand, cooling)| (cooling - (-demand * (2.0 / 3.0))).abs() <= 1e-9));

    let unavailable = run_split_range(false, 10);
    assert!(unavailable
        .demands
        .iter()
        .chain(unavailable.heating.iter())
        .chain(unavailable.cooling.iter())
        .flatten()
        .all(|value| value.abs() <= 1e-12));

    let time = time_axis(run.temperatures[0].len());
    let (temperature_min, temperature_max) =
        plot_bounds(run.temperatures.iter().map(Vec::as_slice));
    let (demand_min, demand_max) = plot_bounds(run.demands.iter().map(Vec::as_slice));
    let peak_error = run.temperatures.each_ref().map(|values| {
        values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let setpoint = if index < 200 {
                    80.0
                } else if index < 800 {
                    84.0
                } else if index < 1_400 {
                    76.0
                } else {
                    80.0
                };
                (setpoint - value).abs()
            })
            .fold(0.0, f64::max)
    });
    let final_settling = run
        .temperatures
        .each_ref()
        .map(|values| settling_time(values, 80.0, 1_400, 0.1));
    let saturation_duration = run
        .heating
        .each_ref()
        .iter()
        .zip(run.cooling.each_ref())
        .map(|(heating, cooling)| {
            heating
                .iter()
                .zip(cooling)
                .filter(|(heat, cool)| **heat >= 99.999 || **cool >= 99.999)
                .count() as f64
                * 0.1
        })
        .collect::<Vec<_>>();
    let summary = format!("case,status,gap_samples,simultaneous_samples,utility_throughput_percent_s,peak_error_degC,final_settling_s,saturation_duration_s,authority,abort,restoration\ndead_zone,rejected,{},{},{:.6},{:.6},{:.3},{:.3},automatic,false,qualified\noverlap,rejected,{},{},{:.6},{:.6},{:.3},{:.3},automatic,false,qualified\ncharacterized,qualified,{},{},{:.6},{:.6},{:.3},{:.3},automatic,false,qualified\nunavailable_authority,unavailable_metrics_ineligible,,,,,,0.000,unavailable,false,tracked_zero_closed_valves\n",
        gap[0], simultaneous[0], throughput[0], peak_error[0], final_settling[0], saturation_duration[0],
        gap[1], simultaneous[1], throughput[1], peak_error[1], final_settling[1], saturation_duration[1],
        gap[2], simultaneous[2], throughput[2], peak_error[2], final_settling[2], saturation_duration[2]);
    evidence::retain(&evidence_root(
        "09-industrial-control-structures",
        "56-reactor-split-range",
    ),
        "reactor-split-range-evidence", "Characterized split range removes gap and overlap",
        "Equal signed demands expose dead zone, simultaneous utility use, and installed-gain correction.",
        &[
            evidence::Panel { title: "Reactor temperature", x_unit: "time (s)", y_unit: "temperature (degC)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(), y_minimum: temperature_min, y_maximum: temperature_max,
                series: vec![
                    evidence::Series { name: "Dead zone", color: "#b45309", dashed: false, x: &time, y: &run.temperatures[0] },
                    evidence::Series { name: "Overlap", color: "#dc2626", dashed: false, x: &time, y: &run.temperatures[1] },
                    evidence::Series { name: "Characterized", color: "#2563eb", dashed: false, x: &time, y: &run.temperatures[2] },
                ] },
            evidence::Panel { title: "Signed controller demand", x_unit: "time (s)", y_unit: "demand (%)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(), y_minimum: demand_min, y_maximum: demand_max,
                series: vec![
                    evidence::Series { name: "Dead zone", color: "#b45309", dashed: false, x: &time, y: &run.demands[0] },
                    evidence::Series { name: "Overlap", color: "#dc2626", dashed: false, x: &time, y: &run.demands[1] },
                    evidence::Series { name: "Characterized", color: "#2563eb", dashed: false, x: &time, y: &run.demands[2] },
                ] },
        ],
        &[
            evidence::RetainedSeries { panel: "Dead-zone mapping", name: "Heating", x: &time, y: &run.heating[0], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Dead-zone mapping", name: "Cooling", x: &time, y: &run.cooling[0], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Overlap mapping", name: "Heating", x: &time, y: &run.heating[1], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Overlap mapping", name: "Cooling", x: &time, y: &run.cooling[1], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Characterized mapping", name: "Heating", x: &time, y: &run.heating[2], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Characterized mapping", name: "Cooling", x: &time, y: &run.cooling[2], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Unavailable authority", name: "Signed demand", x: &time_axis(unavailable.demands[2].len()), y: &unavailable.demands[2], x_unit: "time (s)", y_unit: "demand (%)" },
            evidence::RetainedSeries { panel: "Unavailable authority", name: "Heating applied", x: &time_axis(unavailable.heating[2].len()), y: &unavailable.heating[2], x_unit: "time (s)", y_unit: "valve (%)" },
            evidence::RetainedSeries { panel: "Unavailable authority", name: "Cooling applied", x: &time_axis(unavailable.cooling[2].len()), y: &unavailable.cooling[2], x_unit: "time (s)", y_unit: "valve (%)" },
        ], &summary);
}
