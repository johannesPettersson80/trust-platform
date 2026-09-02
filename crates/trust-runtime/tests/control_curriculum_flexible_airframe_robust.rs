use std::fmt::Write as _;
use std::ops::{Add, Div, Mul, Neg, Sub};
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const DT: f64 = 0.02;
const FREQUENCY_POINTS: usize = 20_000;
const OUTPUT_LIMIT: f64 = 15.0;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/12-nonlinear-adaptive-and-robust-control/72-flexible-airframe-robust")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 72 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 72 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 72 program before configuration")
        .replace(
            "ControllerEnabled : BOOL :=",
            "ControllerEnabled AT %IX64.0 : BOOL :=",
        )
        .replace("DataHealthy : BOOL :=", "DataHealthy AT %IX64.1 : BOOL :=")
        .replace(
            "NominalSaturated : BOOL :=",
            "NominalSaturated AT %MX65.0 : BOOL :=",
        )
        .replace(
            "RobustSaturated : BOOL :=",
            "RobustSaturated AT %MX65.1 : BOOL :=",
        )
        .replace(
            "NominalRequested : LREAL :=",
            "NominalRequested AT %ML72 : LREAL :=",
        )
        .replace(
            "RobustRequested : LREAL :=",
            "RobustRequested AT %ML80 : LREAL :=",
        )
        .replace(
            "FilteredRobustBending : LREAL :=",
            "FilteredRobustBending AT %ML88 : LREAL :=",
        )
        .replace(
            "NominalApplied : LREAL :=",
            "NominalApplied AT %QL0 : LREAL :=",
        )
        .replace("NominalGust : LREAL :=", "NominalGust AT %QL8 : LREAL :=")
        .replace(
            "RobustApplied : LREAL :=",
            "RobustApplied AT %QL16 : LREAL :=",
        )
        .replace("RobustGust : LREAL :=", "RobustGust AT %QL24 : LREAL :=")
        .replace(
            "NominalBending : LREAL :=",
            "NominalBending AT %IL0 : LREAL :=",
        )
        .replace(
            "RobustBending : LREAL :=",
            "RobustBending AT %IL8 : LREAL :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path("flexible-airframe-robust/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 72 Structured Text and Control library")
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-flexible-airframe-{label}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn simulation(omega: f64, damping: f64, effectiveness: f64, label: &str) -> SimulationController {
    let omega_squared = omega * omega;
    let config_text = format!(
        r#"[simulation]
enabled = true
seed = 20260829
time_scale = 1
plant_step_ms = 5

[[plants]]
id = "{label}-p-loop"
model = "state_space"
command_sources = ["%QL0", "%QL8"]
feedback_targets = ["%IL0"]
initial_state = [0.0, 0.0]
[plants.parameters]
a = [[0.0, 1.0], [{a21}, {a22}]]
b = [[0.0, 0.0], [{control_b}, {gust_b}]]
c = [[1.0, 0.0]]
d = [[0.0, 0.0]]

[[plants]]
id = "{label}-robust-loop"
model = "state_space"
command_sources = ["%QL16", "%QL24"]
feedback_targets = ["%IL8"]
initial_state = [0.0, 0.0]
[plants.parameters]
a = [[0.0, 1.0], [{a21}, {a22}]]
b = [[0.0, 0.0], [{control_b}, {gust_b}]]
c = [[1.0, 0.0]]
d = [[0.0, 0.0]]
"#,
        a21 = -omega_squared,
        a22 = -2.0 * damping * omega,
        control_b = 0.1 * effectiveness * omega_squared,
        gust_b = omega_squared,
    );
    let path = temporary_config_path(label);
    std::fs::write(&path, config_text).expect("write Lesson 72 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 72 simulation config");
    let _ = std::fs::remove_file(path);
    SimulationController::new(config)
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid BOOL address"),
            Value::Bool(value),
        )
        .expect("write Lesson 72 BOOL input");
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid LREAL address"))
        .expect("read Lesson 72 LREAL signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid BOOL address"))
        .expect("read Lesson 72 BOOL signal")
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    fn new(re: f64, im: f64) -> Self {
        Self { re, im }
    }

    fn polar(radius: f64, angle: f64) -> Self {
        Self::new(radius * angle.cos(), radius * angle.sin())
    }

    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
}

impl Add for Complex {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self::new(self.re + rhs.re, self.im + rhs.im)
    }
}

impl Sub for Complex {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self::new(self.re - rhs.re, self.im - rhs.im)
    }
}

impl Mul for Complex {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self::new(
            self.re * rhs.re - self.im * rhs.im,
            self.re * rhs.im + self.im * rhs.re,
        )
    }
}

impl Div for Complex {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        let denominator = rhs.re * rhs.re + rhs.im * rhs.im;
        Self::new(
            (self.re * rhs.re + self.im * rhs.im) / denominator,
            (self.im * rhs.re - self.re * rhs.im) / denominator,
        )
    }
}

impl Neg for Complex {
    type Output = Self;
    fn neg(self) -> Self::Output {
        Self::new(-self.re, -self.im)
    }
}

impl From<f64> for Complex {
    fn from(value: f64) -> Self {
        Self::new(value, 0.0)
    }
}

#[derive(Clone, Copy, Debug)]
struct PlantCase {
    omega: f64,
    damping: f64,
    effectiveness: f64,
}

fn plant_cases() -> Vec<PlantCase> {
    let mut cases = Vec::new();
    for omega in [3.2, 4.0, 4.8] {
        for damping in [0.03, 0.08, 0.15] {
            for effectiveness in [0.8, 1.0, 1.2] {
                cases.push(PlantCase {
                    omega,
                    damping,
                    effectiveness,
                });
            }
        }
    }
    cases
}

#[derive(Clone, Copy, Debug)]
struct DiscretePlant {
    a: [[f64; 2]; 2],
    b: [f64; 2],
}

fn discretize(case: PlantCase) -> DiscretePlant {
    let omega = case.omega;
    let damping = case.damping;
    let decaying = (-damping * omega * DT).exp();
    let damped_frequency = omega * (1.0 - damping * damping).sqrt();
    let sine = (damped_frequency * DT).sin();
    let cosine = (damped_frequency * DT).cos();
    let ratio = damping * omega / damped_frequency;
    let a = [
        [
            decaying * (cosine + ratio * sine),
            decaying * sine / damped_frequency,
        ],
        [
            -decaying * omega * omega * sine / damped_frequency,
            decaying * (cosine - ratio * sine),
        ],
    ];
    let continuous_b = 0.1 * case.effectiveness * omega * omega;
    let v0 = a[0][1] * continuous_b;
    let v1 = (a[1][1] - 1.0) * continuous_b;
    let b = [-2.0 * damping / omega * v0 - v1 / (omega * omega), v0];
    DiscretePlant { a, b }
}

fn plant_response(plant: DiscretePlant, z: Complex) -> Complex {
    let z_minus_a00 = z - Complex::from(plant.a[0][0]);
    let z_minus_a11 = z - Complex::from(plant.a[1][1]);
    let determinant = z_minus_a00 * z_minus_a11 - Complex::from(plant.a[0][1] * plant.a[1][0]);
    (z_minus_a11 * Complex::from(plant.b[0]) + Complex::from(plant.a[0][1] * plant.b[1]))
        / determinant
}

#[derive(Clone, Copy, Debug)]
struct Candidate {
    kp: f64,
    kd: f64,
    filter_time: f64,
}

fn controller_response(candidate: Candidate, z: Complex) -> Complex {
    let coefficient = DT / (candidate.filter_time + 0.5 * DT);
    let filter =
        Complex::from(coefficient) / (Complex::from(1.0) - Complex::from(1.0 - coefficient) / z);
    let difference = (Complex::from(1.0) - Complex::from(1.0) / z) / Complex::from(DT);
    (Complex::from(candidate.kp) + Complex::from(candidate.kd) * difference) * filter
}

#[derive(Clone, Copy, Debug)]
struct Peak {
    value: f64,
    frequency: f64,
    case: PlantCase,
}

impl Peak {
    fn empty() -> Self {
        Self {
            value: 0.0,
            frequency: 0.0,
            case: PlantCase {
                omega: 0.0,
                damping: 0.0,
                effectiveness: 0.0,
            },
        }
    }

    fn retain(&mut self, value: f64, frequency: f64, case: PlantCase) {
        if value > self.value {
            *self = Self {
                value,
                frequency,
                case,
            };
        }
    }
}

#[derive(Debug)]
struct FrequencyMetrics {
    weighted_sensitivity: Peak,
    weighted_uncertainty: Peak,
    weighted_effort: Peak,
    gamma: f64,
    maximum_pole_radius: f64,
}

fn determinant3(matrix: [[f64; 3]; 3]) -> f64 {
    matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0])
}

fn cubic_root_radii(matrix: [[f64; 3]; 3]) -> [f64; 3] {
    let trace = matrix[0][0] + matrix[1][1] + matrix[2][2];
    let trace_square = matrix[0][0] * matrix[0][0]
        + matrix[1][1] * matrix[1][1]
        + matrix[2][2] * matrix[2][2]
        + 2.0
            * (matrix[0][1] * matrix[1][0]
                + matrix[0][2] * matrix[2][0]
                + matrix[1][2] * matrix[2][1]);
    let coefficients = [
        -trace,
        0.5 * (trace * trace - trace_square),
        -determinant3(matrix),
    ];
    let mut roots = [
        Complex::polar(1.0, 0.0),
        Complex::polar(1.0, 2.0 * std::f64::consts::PI / 3.0),
        Complex::polar(1.0, 4.0 * std::f64::consts::PI / 3.0),
    ];
    for _ in 0..100 {
        let previous = roots;
        for index in 0..3 {
            let z = previous[index];
            let polynomial = z * z * z
                + Complex::from(coefficients[0]) * z * z
                + Complex::from(coefficients[1]) * z
                + Complex::from(coefficients[2]);
            let denominator = (z - previous[(index + 1) % 3]) * (z - previous[(index + 2) % 3]);
            roots[index] = z - polynomial / denominator;
        }
    }
    [roots[0].abs(), roots[1].abs(), roots[2].abs()]
}

fn nominal_pole_radius(plant: DiscretePlant) -> f64 {
    let matrix = [
        [plant.a[0][0] - 40.0 * plant.b[0], plant.a[0][1]],
        [plant.a[1][0] - 40.0 * plant.b[1], plant.a[1][1]],
    ];
    let trace = matrix[0][0] + matrix[1][1];
    let determinant = matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
    let discriminant = Complex::new(trace * trace - 4.0 * determinant, 0.0);
    if discriminant.re >= 0.0 {
        let root = discriminant.re.sqrt();
        ((trace + root) * 0.5)
            .abs()
            .max(((trace - root) * 0.5).abs())
    } else {
        determinant.sqrt()
    }
}

fn robust_pole_radius(plant: DiscretePlant, candidate: Candidate) -> f64 {
    let coefficient = DT / (candidate.filter_time + 0.5 * DT);
    let retained = 1.0 - coefficient;
    let current_gain = (candidate.kp + candidate.kd / DT) * coefficient;
    let retained_gain = candidate.kd / DT - (candidate.kp + candidate.kd / DT) * retained;
    let matrix = [
        [
            plant.a[0][0] - plant.b[0] * current_gain,
            plant.a[0][1],
            plant.b[0] * retained_gain,
        ],
        [
            plant.a[1][0] - plant.b[1] * current_gain,
            plant.a[1][1],
            plant.b[1] * retained_gain,
        ],
        [coefficient, 0.0, retained],
    ];
    cubic_root_radii(matrix).into_iter().fold(0.0, f64::max)
}

fn frequency_metrics(candidate: Option<Candidate>) -> FrequencyMetrics {
    let mut weighted_sensitivity = Peak::empty();
    let mut weighted_uncertainty = Peak::empty();
    let mut weighted_effort = Peak::empty();
    let mut maximum_pole_radius = 0.0_f64;
    for case in plant_cases() {
        let plant = discretize(case);
        maximum_pole_radius = maximum_pole_radius.max(match candidate {
            Some(candidate) => robust_pole_radius(plant, candidate),
            None => nominal_pole_radius(plant),
        });
        for index in 0..FREQUENCY_POINTS {
            let exponent = -2.0 + 4.0 * index as f64 / (FREQUENCY_POINTS - 1) as f64;
            let frequency = 10.0_f64.powf(exponent);
            let z = Complex::polar(1.0, frequency * DT);
            let plant_value = plant_response(plant, z);
            let controller = candidate
                .map(|candidate| controller_response(candidate, z))
                .unwrap_or_else(|| Complex::from(40.0));
            let loop_value = plant_value * controller;
            let sensitivity = Complex::from(1.0) / (Complex::from(1.0) + loop_value);
            let complementary = loop_value * sensitivity;
            let control_sensitivity = controller * sensitivity;
            let s = Complex::new(0.0, frequency);
            let performance_weight = Complex::from(1.5) / (Complex::from(1.0) + s);
            let uncertainty_weight = Complex::from(0.15)
                * (Complex::from(1.0) + s / Complex::from(2.0))
                / (Complex::from(1.0) + s / Complex::from(10.0));
            weighted_sensitivity.retain((performance_weight * sensitivity).abs(), frequency, case);
            weighted_uncertainty.retain(
                (uncertainty_weight * complementary).abs(),
                frequency,
                case,
            );
            weighted_effort.retain(0.005 * control_sensitivity.abs(), frequency, case);
        }
    }
    let gamma = weighted_sensitivity
        .value
        .max(weighted_uncertainty.value)
        .max(weighted_effort.value);
    FrequencyMetrics {
        weighted_sensitivity,
        weighted_uncertainty,
        weighted_effort,
        gamma,
        maximum_pole_radius,
    }
}

#[derive(Clone, Copy, Debug)]
struct TimeSample {
    time: f64,
    nominal_bending: f64,
    robust_bending: f64,
    nominal_requested: f64,
    nominal_applied: f64,
    robust_requested: f64,
    robust_applied: f64,
    data_healthy: bool,
    enabled: bool,
}

#[derive(Debug)]
struct TimeRecord {
    samples: Vec<TimeSample>,
    nominal_movement: f64,
    robust_movement: f64,
    nominal_saturation_seconds: f64,
    robust_saturation_seconds: f64,
}

fn run_time_case(case: PlantCase, quality_and_disable: bool, label: &str) -> TimeRecord {
    let mut runtime = build_runtime();
    let mut plant = simulation(case.omega, case.damping, case.effectiveness, label);
    plant.apply_pre_cycle(Duration::ZERO, &mut runtime).unwrap();
    let sample_count = if quality_and_disable { 750 } else { 1250 };
    let mut samples = Vec::with_capacity(sample_count);
    let mut nominal_movement = 0.0;
    let mut robust_movement = 0.0;
    let mut nominal_saturation_seconds = 0.0;
    let mut robust_saturation_seconds = 0.0;
    let mut previous_nominal_applied = 0.0;
    let mut previous_robust_applied = 0.0;
    let mut previous_filtered = 0.0;
    let coefficient = DT / (0.05 + 0.5 * DT);

    for sample_index in 0..sample_count {
        let time = sample_index as f64 * DT;
        let data_healthy = !quality_and_disable || !(12.0..13.0).contains(&time);
        let enabled = !quality_and_disable || time < 14.0;
        write_bool(&mut runtime, "%IX64.0", enabled);
        write_bool(&mut runtime, "%IX64.1", data_healthy);
        let nominal_bending = read_lreal(&runtime, "%IL0");
        let robust_bending = read_lreal(&runtime, "%IL8");
        let expected_filtered = if sample_index == 0 {
            robust_bending
        } else {
            previous_filtered + coefficient * (robust_bending - previous_filtered)
        };
        let expected_nominal_requested = -40.0 * nominal_bending;
        let expected_robust_requested = if sample_index == 0 {
            -10.0 * expected_filtered
        } else {
            -10.0 * expected_filtered - 3.0 * (expected_filtered - previous_filtered) / DT
        };

        runtime
            .execute_cycle()
            .expect("execute Lesson 72 controller interval");
        let nominal_requested = read_lreal(&runtime, "%ML72");
        let robust_requested = read_lreal(&runtime, "%ML80");
        let filtered = read_lreal(&runtime, "%ML88");
        let nominal_applied = read_lreal(&runtime, "%QL0");
        let robust_applied = read_lreal(&runtime, "%QL16");
        let expected_gust = if (250..300).contains(&sample_index) {
            0.10
        } else {
            0.0
        };
        assert!(
            (read_lreal(&runtime, "%QL8") - expected_gust).abs() <= 1e-12,
            "sample {sample_index} time={time:.2}: nominal gust was {} instead of {expected_gust}",
            read_lreal(&runtime, "%QL8"),
        );
        assert!((read_lreal(&runtime, "%QL24") - expected_gust).abs() <= 1e-12);
        assert!((filtered - expected_filtered).abs() <= 2e-9);
        assert!((nominal_requested - expected_nominal_requested).abs() <= 2e-9);
        assert!((robust_requested - expected_robust_requested).abs() <= 2e-8);
        let expected_nominal_applied = if enabled && data_healthy {
            expected_nominal_requested.clamp(-OUTPUT_LIMIT, OUTPUT_LIMIT)
        } else {
            0.0
        };
        let expected_robust_applied = if enabled && data_healthy {
            expected_robust_requested.clamp(-OUTPUT_LIMIT, OUTPUT_LIMIT)
        } else {
            0.0
        };
        assert!((nominal_applied - expected_nominal_applied).abs() <= 2e-9);
        assert!((robust_applied - expected_robust_applied).abs() <= 2e-8);
        assert_eq!(
            read_bool(&runtime, "%MX65.0"),
            enabled && data_healthy && (nominal_requested - nominal_applied).abs() > 1e-9,
        );
        assert_eq!(
            read_bool(&runtime, "%MX65.1"),
            enabled && data_healthy && (robust_requested - robust_applied).abs() > 1e-9,
        );
        assert!(nominal_applied.abs() <= OUTPUT_LIMIT + 1e-12);
        assert!(robust_applied.abs() <= OUTPUT_LIMIT + 1e-12);

        nominal_movement += (nominal_applied - previous_nominal_applied).abs();
        robust_movement += (robust_applied - previous_robust_applied).abs();
        if read_bool(&runtime, "%MX65.0") {
            nominal_saturation_seconds += DT;
        }
        if read_bool(&runtime, "%MX65.1") {
            robust_saturation_seconds += DT;
        }
        samples.push(TimeSample {
            time,
            nominal_bending,
            robust_bending,
            nominal_requested,
            nominal_applied,
            robust_requested,
            robust_applied,
            data_healthy,
            enabled,
        });
        previous_filtered = filtered;
        previous_nominal_applied = nominal_applied;
        previous_robust_applied = robust_applied;
        let now = Duration::from_millis((sample_index as i64 + 1) * 20);
        plant.apply_post_cycle(now, &runtime).unwrap();
        plant.apply_pre_cycle(now, &mut runtime).unwrap();
    }
    TimeRecord {
        samples,
        nominal_movement,
        robust_movement,
        nominal_saturation_seconds,
        robust_saturation_seconds,
    }
}

#[derive(Clone, Copy, Debug)]
struct ResponseMetrics {
    peak: f64,
    iae: f64,
    settling: Option<f64>,
    peak_requested: f64,
    peak_applied: f64,
}

fn response_metrics(record: &TimeRecord, robust: bool) -> ResponseMetrics {
    let event: Vec<_> = record
        .samples
        .iter()
        .filter(|sample| sample.time >= 5.0)
        .collect();
    let peak = event
        .iter()
        .map(|sample| {
            if robust {
                sample.robust_bending.abs()
            } else {
                sample.nominal_bending.abs()
            }
        })
        .fold(0.0_f64, f64::max);
    let iae = event
        .iter()
        .map(|sample| {
            if robust {
                sample.robust_bending.abs()
            } else {
                sample.nominal_bending.abs()
            }
        })
        .sum::<f64>()
        * DT;
    let post_gust: Vec<_> = event
        .iter()
        .copied()
        .filter(|sample| sample.time >= 6.0)
        .collect();
    let required_samples = (2.0 / DT) as usize;
    let settling = post_gust
        .windows(required_samples)
        .position(|window| {
            window.iter().all(|sample| {
                (if robust {
                    sample.robust_bending.abs()
                } else {
                    sample.nominal_bending.abs()
                }) <= 0.005
            })
        })
        .map(|index| post_gust[index].time - 6.0);
    let peak_requested = event
        .iter()
        .map(|sample| {
            if robust {
                sample.robust_requested.abs()
            } else {
                sample.nominal_requested.abs()
            }
        })
        .fold(0.0_f64, f64::max);
    let peak_applied = event
        .iter()
        .map(|sample| {
            if robust {
                sample.robust_applied.abs()
            } else {
                sample.nominal_applied.abs()
            }
        })
        .fold(0.0_f64, f64::max);
    ResponseMetrics {
        peak,
        iae,
        settling,
        peak_requested,
        peak_applied,
    }
}

#[test]
fn control_curriculum_flexible_airframe_qualifies_sampled_robust_design() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 72 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 72 must use schema 4");
    assert_eq!(lesson["sequence"], 72);
    assert_eq!(lesson["prerequisites"][0], "control-extruder-adaptive");
    assert_eq!(lesson["nextLesson"], "control-networked-pumping");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_flexible_airframe_qualifies_sampled_robust_design"
    );

    let nominal_frequency = frequency_metrics(None);
    let candidates = [
        Candidate {
            kp: 8.0,
            kd: 3.0,
            filter_time: 0.05,
        },
        Candidate {
            kp: 10.0,
            kd: 2.0,
            filter_time: 0.05,
        },
        Candidate {
            kp: 10.0,
            kd: 3.0,
            filter_time: 0.05,
        },
        Candidate {
            kp: 12.0,
            kd: 3.0,
            filter_time: 0.05,
        },
    ];
    let candidate_metrics: Vec<_> = candidates
        .into_iter()
        .map(|candidate| frequency_metrics(Some(candidate)))
        .collect();
    assert!(nominal_frequency.gamma >= 1.0);
    assert!(nominal_frequency.maximum_pole_radius >= 1.0);
    let selected_index = candidate_metrics
        .iter()
        .enumerate()
        .min_by(|left, right| left.1.gamma.total_cmp(&right.1.gamma))
        .map(|(index, _)| index)
        .unwrap();
    assert_eq!(
        selected_index, 2,
        "the common oracle must select (10,3,0.05)"
    );
    let selected = &candidate_metrics[selected_index];
    assert!(selected.weighted_sensitivity.value < 1.0);
    assert!(selected.weighted_uncertainty.value < 1.0);
    assert!(selected.weighted_effort.value < 1.0);
    assert!(selected.maximum_pole_radius < 1.0 - 1e-8);
    assert!((selected.gamma - 0.8333).abs() <= 0.002);

    let mut worst_nominal_iae = 0.0_f64;
    let mut worst_robust_iae = 0.0_f64;
    let mut worst_nominal_peak = 0.0_f64;
    let mut worst_robust_peak = 0.0_f64;
    let mut worst_robust_settling = 0.0_f64;
    let mut nominal_nonsettling = 0_usize;
    let mut worst_nominal_movement = 0.0_f64;
    let mut worst_robust_movement = 0.0_f64;
    let mut worst_nominal_saturation = 0.0_f64;
    let mut worst_robust_saturation = 0.0_f64;
    for (index, case) in plant_cases().into_iter().enumerate() {
        let record = run_time_case(case, false, &format!("grid-{index}"));
        let nominal = response_metrics(&record, false);
        let robust = response_metrics(&record, true);
        worst_nominal_iae = worst_nominal_iae.max(nominal.iae);
        worst_robust_iae = worst_robust_iae.max(robust.iae);
        worst_nominal_peak = worst_nominal_peak.max(nominal.peak);
        worst_robust_peak = worst_robust_peak.max(robust.peak);
        worst_nominal_movement = worst_nominal_movement.max(record.nominal_movement);
        worst_robust_movement = worst_robust_movement.max(record.robust_movement);
        worst_nominal_saturation = worst_nominal_saturation.max(record.nominal_saturation_seconds);
        worst_robust_saturation = worst_robust_saturation.max(record.robust_saturation_seconds);
        if let Some(settling) = robust.settling {
            worst_robust_settling = worst_robust_settling.max(settling);
        } else {
            panic!("robust case {case:?} did not meet the declared settling rule");
        }
        if nominal.settling.is_none() {
            nominal_nonsettling += 1;
        }
        assert!(robust.peak_requested.is_finite() && robust.peak_applied.is_finite());
        assert!(robust.peak_applied <= OUTPUT_LIMIT + 1e-12);
    }
    assert!(nominal_nonsettling > 0);
    assert!(worst_robust_iae < worst_nominal_iae);
    assert!(worst_robust_peak < worst_nominal_peak);
    assert_eq!(worst_robust_saturation, 0.0);

    let quality = run_time_case(
        PlantCase {
            omega: 4.0,
            damping: 0.08,
            effectiveness: 1.0,
        },
        true,
        "quality",
    );
    assert!(quality
        .samples
        .iter()
        .filter(|sample| !sample.data_healthy)
        .all(|sample| { sample.nominal_applied == 0.0 && sample.robust_applied == 0.0 }));
    assert!(quality
        .samples
        .iter()
        .filter(|sample| !sample.enabled)
        .all(|sample| { sample.nominal_applied == 0.0 && sample.robust_applied == 0.0 }));
    assert!(quality.samples.iter().any(|sample| {
        sample.time >= 13.0
            && sample.time < 14.0
            && sample.data_healthy
            && sample.enabled
            && sample.robust_applied.abs() <= OUTPUT_LIMIT
    }));

    eprintln!(
        "airframe nominal gamma={:.4}, pole radius={:.6}; robust peaks WpS/WdT/WuKS={:.4}/{:.4}/{:.4}, gamma={:.4}, pole radius={:.6}; worst time IAE nominal/robust={:.4}/{:.4} g*s, peak={:.4}/{:.4} g, movement={:.1}/{:.1} pp, saturation={:.2}/{:.2} s, robust settling={:.2} s, nominal nonsettling={}/27",
        nominal_frequency.gamma,
        nominal_frequency.maximum_pole_radius,
        selected.weighted_sensitivity.value,
        selected.weighted_uncertainty.value,
        selected.weighted_effort.value,
        selected.gamma,
        selected.maximum_pole_radius,
        worst_nominal_iae,
        worst_robust_iae,
        worst_nominal_peak,
        worst_robust_peak,
        worst_nominal_movement,
        worst_robust_movement,
        worst_nominal_saturation,
        worst_robust_saturation,
        worst_robust_settling,
        nominal_nonsettling,
    );
    eprintln!(
        "airframe robust worst weighted peaks: WpS at {:.4} rad/s {:?}, WdT at {:.4} rad/s {:?}, WuKS at {:.4} rad/s {:?}",
        selected.weighted_sensitivity.frequency,
        selected.weighted_sensitivity.case,
        selected.weighted_uncertainty.frequency,
        selected.weighted_uncertainty.case,
        selected.weighted_effort.frequency,
        selected.weighted_effort.case,
    );

    let design = [0.0, 1.0];
    let worst_iae = [worst_nominal_iae, worst_robust_iae];
    let worst_peak = [worst_nominal_peak, worst_robust_peak];
    let mut summary = String::from(
        "design,worst_iae_g_s,worst_peak_g,worst_movement_pp,saturation_s,gamma,maximum_pole_radius\n",
    );
    writeln!(
        summary,
        "nominal,{worst_nominal_iae:.9},{worst_nominal_peak:.9},{worst_nominal_movement:.9},{worst_nominal_saturation:.3},{:.9},{:.9}",
        nominal_frequency.gamma, nominal_frequency.maximum_pole_radius
    )
    .unwrap();
    writeln!(
        summary,
        "robust,{worst_robust_iae:.9},{worst_robust_peak:.9},{worst_robust_movement:.9},{worst_robust_saturation:.3},{:.9},{:.9}",
        selected.gamma, selected.maximum_pole_radius
    )
    .unwrap();
    let figure_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/12-nonlinear-adaptive-and-robust-control/72-flexible-airframe-robust/figures");
    evidence::retain(
        &figure_root,
        "flexible-airframe-robust-evidence",
        "Robust design improves the worst declared airframe case",
        "Index 0 is the nominal controller and index 1 is the qualified robust candidate.",
        &[
            evidence::Panel {
                title: "Worst integrated bending acceleration",
                x_unit: "design index",
                y_unit: "g s",
                x_minimum: -0.05,
                x_maximum: 1.05,
                y_minimum: 0.0,
                y_maximum: worst_iae.iter().copied().fold(0.0_f64, f64::max) * 1.1,
                series: vec![evidence::Series {
                    name: "Worst IAE",
                    color: "#2563eb",
                    dashed: false,
                    x: &design,
                    y: &worst_iae,
                }],
            },
            evidence::Panel {
                title: "Worst peak bending acceleration",
                x_unit: "design index",
                y_unit: "g",
                x_minimum: -0.05,
                x_maximum: 1.05,
                y_minimum: 0.0,
                y_maximum: worst_peak.iter().copied().fold(0.0_f64, f64::max) * 1.1,
                series: vec![evidence::Series {
                    name: "Worst peak",
                    color: "#dc2626",
                    dashed: false,
                    x: &design,
                    y: &worst_peak,
                }],
            },
        ],
        &[],
        &summary,
    );
}
