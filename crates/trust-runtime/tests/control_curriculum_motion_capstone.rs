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

const DT: f64 = 0.01;
const CYCLES: usize = 3_000;

fn project_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/78-servo")
        .join(relative)
}

fn project_file(relative: &str) -> String {
    let path = project_path(relative);
    assert!(
        path.is_file(),
        "Lesson 78 executable servo capstone is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 78 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 78 program before configuration")
        .replace(
            "AutomaticEnabled : BOOL :=",
            "AutomaticEnabled AT %IX192.0 : BOOL :=",
        )
        .replace(
            "MeasurementQualityGood : BOOL :=",
            "MeasurementQualityGood AT %IX192.1 : BOOL :=",
        )
        .replace(
            "PositionSetpoint : LREAL :=",
            "PositionSetpoint AT %IL200 : LREAL :=",
        )
        .replace(
            "\n    LoadTorque : LREAL :=",
            "\n    LoadTorque AT %IL208 : LREAL :=",
        )
        .replace(
            "ManualTorque : LREAL :=",
            "ManualTorque AT %IL216 : LREAL :=",
        )
        .replace(
            "FineCycleTime : LREAL :=",
            "FineCycleTime AT %IL224 : LREAL :=",
        )
        .replace("PRawPosition : LREAL :=", "PRawPosition AT %IL0 : LREAL :=")
        .replace(
            "PTruePosition : LREAL :=",
            "PTruePosition AT %IL8 : LREAL :=",
        )
        .replace("PVelocity : LREAL :=", "PVelocity AT %IL16 : LREAL :=")
        .replace(
            "PDRawPosition : LREAL :=",
            "PDRawPosition AT %IL24 : LREAL :=",
        )
        .replace(
            "PDTruePosition : LREAL :=",
            "PDTruePosition AT %IL32 : LREAL :=",
        )
        .replace("PDVelocity : LREAL :=", "PDVelocity AT %IL40 : LREAL :=")
        .replace(
            "PIDRawPosition : LREAL :=",
            "PIDRawPosition AT %IL48 : LREAL :=",
        )
        .replace(
            "PIDTruePosition : LREAL :=",
            "PIDTruePosition AT %IL56 : LREAL :=",
        )
        .replace("PIDVelocity : LREAL :=", "PIDVelocity AT %IL64 : LREAL :=")
        .replace(
            "CoarseRawPosition : LREAL :=",
            "CoarseRawPosition AT %IL72 : LREAL :=",
        )
        .replace(
            "CoarseTruePosition : LREAL :=",
            "CoarseTruePosition AT %IL80 : LREAL :=",
        )
        .replace(
            "CoarseVelocity : LREAL :=",
            "CoarseVelocity AT %IL88 : LREAL :=",
        )
        .replace(
            "PFilteredPosition : LREAL :=",
            "PFilteredPosition AT %ML96 : LREAL :=",
        )
        .replace(
            "PDFilteredPosition : LREAL :=",
            "PDFilteredPosition AT %ML104 : LREAL :=",
        )
        .replace(
            "PIDFilteredPosition : LREAL :=",
            "PIDFilteredPosition AT %ML112 : LREAL :=",
        )
        .replace(
            "CoarseFilteredPosition : LREAL :=",
            "CoarseFilteredPosition AT %ML120 : LREAL :=",
        )
        .replace(
            "PRequestedTorque : LREAL :=",
            "PRequestedTorque AT %ML128 : LREAL :=",
        )
        .replace(
            "PDRequestedTorque : LREAL :=",
            "PDRequestedTorque AT %ML136 : LREAL :=",
        )
        .replace(
            "PIDRequestedTorque : LREAL :=",
            "PIDRequestedTorque AT %ML144 : LREAL :=",
        )
        .replace(
            "CoarseRequestedTorque : LREAL :=",
            "CoarseRequestedTorque AT %ML152 : LREAL :=",
        )
        .replace(
            "CoarsePendingTorque : LREAL :=",
            "CoarsePendingTorque AT %ML160 : LREAL :=",
        )
        .replace(
            "PAppliedTorque : LREAL :=",
            "PAppliedTorque AT %QL0 : LREAL :=",
        )
        .replace("PLoadTorque : LREAL :=", "PLoadTorque AT %QL8 : LREAL :=")
        .replace(
            "PDAppliedTorque : LREAL :=",
            "PDAppliedTorque AT %QL16 : LREAL :=",
        )
        .replace(
            "PDLoadTorque : LREAL :=",
            "PDLoadTorque AT %QL24 : LREAL :=",
        )
        .replace(
            "PIDAppliedTorque : LREAL :=",
            "PIDAppliedTorque AT %QL32 : LREAL :=",
        )
        .replace(
            "PIDLoadTorque : LREAL :=",
            "PIDLoadTorque AT %QL40 : LREAL :=",
        )
        .replace(
            "CoarseAppliedTorque : LREAL :=",
            "CoarseAppliedTorque AT %QL48 : LREAL :=",
        )
        .replace(
            "CoarseLoadTorque : LREAL :=",
            "CoarseLoadTorque AT %QL56 : LREAL :=",
        )
        .replace(
            "FineUpdatePulse : BOOL :=",
            "FineUpdatePulse AT %MX172.0 : BOOL :=",
        )
        .replace(
            "CoarseUpdatePulse : BOOL :=",
            "CoarseUpdatePulse AT %MX172.1 : BOOL :=",
        )
        .replace(
            "ControllerRevision : DINT :=",
            "ControllerRevision AT %MD176 : DINT :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "control/14-integrated-capstones/78-servo/src/ServoCandidate.st",
            project_file("src/ServoCandidate.st"),
        ),
        SourceFile::with_path("control/14-integrated-capstones/78-servo/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 78 Structured Text and Control library")
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
    time: f64,
    setpoint: f64,
    raw_position: [f64; 4],
    true_position: [f64; 4],
    velocity: [f64; 4],
    filtered_position: [f64; 4],
    requested: [f64; 4],
    applied: [f64; 4],
    fine_update: bool,
    coarse_update: bool,
    quality: bool,
    automatic: bool,
    revision: i32,
}

fn run_case(config_name: &str, abnormalities: bool) -> Vec<Sample> {
    let mut runtime = build_runtime();
    let mut simulation = SimulationController::new(
        SimulationConfig::load(project_path(config_name))
            .unwrap_or_else(|error| panic!("load Lesson 78 {config_name}: {error}")),
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial servo state");
    let mut trace = Vec::with_capacity(CYCLES);

    for cycle in 0..CYCLES {
        let time = cycle as f64 * DT;
        let setpoint = if time >= 1.0 { 0.5 } else { 0.0 };
        let load = if time >= 10.0 { 1.5 } else { 0.0 };
        let automatic = !abnormalities || !(26.0..26.5).contains(&time);
        let quality = !abnormalities || !(27.0..27.2).contains(&time);
        write_bool(&mut runtime, "%IX192.0", automatic);
        write_bool(&mut runtime, "%IX192.1", quality);
        write_lreal(&mut runtime, "%IL200", setpoint);
        write_lreal(&mut runtime, "%IL208", load);
        write_lreal(&mut runtime, "%IL216", 0.0);
        write_lreal(&mut runtime, "%IL224", DT);
        runtime
            .execute_cycle()
            .expect("execute Lesson 78 controller scan");

        let requested =
            ["%ML128", "%ML136", "%ML144", "%ML152"].map(|address| read_lreal(&runtime, address));
        let applied =
            ["%QL0", "%QL16", "%QL32", "%QL48"].map(|address| read_lreal(&runtime, address));
        assert!(requested
            .iter()
            .all(|value| value.is_finite() && value.abs() <= 10.0));
        assert!(applied
            .iter()
            .all(|value| value.is_finite() && value.abs() <= 10.0));
        let now = Duration::from_millis(((cycle + 1) * 10) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance flexible tables");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish flexible-table feedback");
        trace.push(Sample {
            time,
            setpoint,
            raw_position: ["%IL0", "%IL24", "%IL48", "%IL72"]
                .map(|address| read_lreal(&runtime, address)),
            true_position: ["%IL8", "%IL32", "%IL56", "%IL80"]
                .map(|address| read_lreal(&runtime, address)),
            velocity: ["%IL16", "%IL40", "%IL64", "%IL88"]
                .map(|address| read_lreal(&runtime, address)),
            filtered_position: ["%ML96", "%ML104", "%ML112", "%ML120"]
                .map(|address| read_lreal(&runtime, address)),
            requested,
            applied,
            fine_update: read_bool(&runtime, "%MX172.0"),
            coarse_update: read_bool(&runtime, "%MX172.1"),
            quality,
            automatic,
            revision: read_dint(&runtime, "%MD176"),
        });
    }
    trace
}

fn settling_time(trace: &[Sample], candidate: usize) -> Option<f64> {
    let start = trace.iter().position(|sample| sample.time >= 1.0).unwrap();
    let end = trace.iter().position(|sample| sample.time >= 10.0).unwrap();
    let dwell = (1.0 / DT) as usize;
    (start..end.saturating_sub(dwell)).find_map(|index| {
        trace[index..index + dwell]
            .iter()
            .all(|sample| (sample.setpoint - sample.true_position[candidate]).abs() <= 0.01)
            .then_some(trace[index].time - 1.0)
    })
}

fn final_window_error(trace: &[Sample], candidate: usize) -> f64 {
    let samples: Vec<_> = trace
        .iter()
        .filter(|sample| sample.time >= 25.0 && sample.quality && sample.automatic)
        .collect();
    samples
        .iter()
        .map(|sample| (sample.setpoint - sample.true_position[candidate]).abs())
        .sum::<f64>()
        / samples.len() as f64
}

fn overshoot_percent(trace: &[Sample], candidate: usize) -> f64 {
    let peak = trace
        .iter()
        .filter(|sample| (1.0..10.0).contains(&sample.time))
        .map(|sample| sample.true_position[candidate])
        .fold(f64::NEG_INFINITY, f64::max);
    100.0 * (peak - 0.5) / 0.5
}

fn torque_total_variation(trace: &[Sample], candidate: usize) -> f64 {
    trace
        .windows(2)
        .map(|pair| (pair[1].applied[candidate] - pair[0].applied[candidate]).abs())
        .sum()
}

fn quiet_torque_rms(trace: &[Sample], candidate: usize) -> f64 {
    let values: Vec<_> = trace
        .iter()
        .filter(|sample| (7.0..9.0).contains(&sample.time))
        .map(|sample| sample.applied[candidate])
        .collect();
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    (values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / values.len() as f64)
        .sqrt()
}

#[derive(Clone, Copy, Debug)]
struct Complex {
    re: f64,
    im: f64,
}

impl Complex {
    fn add(self, other: Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }

    fn multiply(self, other: Self) -> Self {
        Self {
            re: self.re * other.re - self.im * other.im,
            im: self.re * other.im + self.im * other.re,
        }
    }

    fn divide(self, other: Self) -> Self {
        let denominator = other.re * other.re + other.im * other.im;
        Self {
            re: (self.re * other.re + self.im * other.im) / denominator,
            im: (self.im * other.re - self.re * other.im) / denominator,
        }
    }

    fn abs(self) -> f64 {
        self.re.hypot(self.im)
    }
}

fn polynomial(coefficients: &[f64], value: Complex) -> Complex {
    coefficients
        .iter()
        .fold(Complex { re: 0.0, im: 0.0 }, |sum, coefficient| {
            sum.multiply(value).add(Complex {
                re: *coefficient,
                im: 0.0,
            })
        })
}

fn loop_value(frequency: f64) -> Complex {
    let s = Complex {
        re: 0.0,
        im: frequency,
    };
    let plant =
        polynomial(&[3.125, 125.0], s).divide(polynomial(&[1.0, 4.5, 126.9375, 62.5, 0.0], s));
    let controller = polynomial(&[2.0, 7.0, 1.2], s).divide(s.multiply(Complex {
        re: 1.0,
        im: 0.05 * frequency,
    }));
    plant.multiply(controller)
}

fn frequency_metrics() -> (f64, f64, f64, f64) {
    let mut samples = Vec::with_capacity(40_001);
    for index in 0..=40_000 {
        let exponent = -3.0 + 6.0 * index as f64 / 40_000.0;
        let frequency = 10_f64.powf(exponent);
        let value = loop_value(frequency);
        samples.push((
            frequency,
            value,
            value.abs(),
            value.im.atan2(value.re).to_degrees(),
        ));
    }
    let crossover = samples
        .iter()
        .min_by(|left, right| left.2.ln().abs().total_cmp(&right.2.ln().abs()))
        .unwrap();
    let phase_margin = 180.0 + crossover.3;
    let phase_crossing = samples
        .iter()
        .filter(|sample| sample.0 > crossover.0)
        .min_by(|left, right| (left.3 + 180.0).abs().total_cmp(&(right.3 + 180.0).abs()))
        .unwrap();
    let gain_margin = 1.0 / phase_crossing.2;
    let maximum_sensitivity = samples
        .iter()
        .map(|sample| {
            Complex { re: 1.0, im: 0.0 }
                .divide(Complex { re: 1.0, im: 0.0 }.add(sample.1))
                .abs()
        })
        .fold(0.0, f64::max);
    (crossover.0, phase_margin, gain_margin, maximum_sensitivity)
}

fn plant_derivative(state: [f64; 4], torque: f64) -> [f64; 4] {
    [
        state[1],
        -100.0 * state[0] - 3.5 * state[1] + 100.0 * state[2] + 2.5 * state[3] + 5.0 * torque,
        state[3],
        25.0 * state[0] + 0.625 * state[1] - 25.0 * state[2] - state[3],
    ]
}

fn fine_linear_step(state: [f64; 8]) -> [f64; 8] {
    let plant = [state[0], state[1], state[2], state[3]];
    let filter_coefficient = DT / (0.05 + 0.5 * DT);
    let filtered = state[4] + filter_coefficient * (plant[2] - state[4]);
    let process_rate = (filtered - state[6]) / DT;
    let requested = state[5]
        - 7.0 * (filtered - state[6])
        - 1.2 * DT * filtered
        - 2.0 * (process_rate - state[7]);

    let k1 = plant_derivative(plant, requested);
    let k2_state = std::array::from_fn(|index| plant[index] + 0.5 * DT * k1[index]);
    let k2 = plant_derivative(k2_state, requested);
    let k3_state = std::array::from_fn(|index| plant[index] + 0.5 * DT * k2[index]);
    let k3 = plant_derivative(k3_state, requested);
    let k4_state = std::array::from_fn(|index| plant[index] + DT * k3[index]);
    let k4 = plant_derivative(k4_state, requested);
    let next_plant: [f64; 4] = std::array::from_fn(|index| {
        plant[index] + DT * (k1[index] + 2.0 * k2[index] + 2.0 * k3[index] + k4[index]) / 6.0
    });
    [
        next_plant[0],
        next_plant[1],
        next_plant[2],
        next_plant[3],
        filtered,
        requested,
        filtered,
        process_rate,
    ]
}

fn fine_discrete_dominant_pole_radius() -> f64 {
    let mut vector = [1.0, -0.3, 0.7, 0.2, -0.1, 0.05, 0.4, -0.2];
    let mut radius = 0.0;
    for _ in 0..20_000 {
        let next = fine_linear_step(vector);
        radius = next.iter().map(|value| value * value).sum::<f64>().sqrt();
        vector = next.map(|value| value / radius);
    }
    radius
}

#[test]
fn control_curriculum_servo_capstone_qualifies_filtered_pid_digitally() {
    let root = project_path("");
    assert!(
        root.is_dir(),
        "Lesson 78 executable servo capstone is missing: {}",
        root.display()
    );
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 78 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 78);
    assert_eq!(lesson["prerequisites"][0], "control-capstone-water-heater");
    assert_eq!(lesson["nextLesson"], "control-capstone-reactor");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_servo_capstone_qualifies_filtered_pid_digitally"
    );
    for config in [
        "simulation.toml",
        "scenarios/heavy-payload.toml",
        "scenarios/reduced-damping.toml",
        "scenarios/doubled-noise.toml",
    ] {
        SimulationConfig::load(project_path(config))
            .unwrap_or_else(|error| panic!("{config} must be executable: {error}"));
    }

    let resonance = (20.0_f64 * (1.0 / 0.2 + 1.0 / 0.8)).sqrt();
    assert!((resonance - 11.180_339_887).abs() < 1.0e-6);
    assert!((std::f64::consts::PI / 0.01 - 314.159_265_359).abs() < 1.0e-6);
    assert!((std::f64::consts::PI / 0.1 - 31.415_926_536).abs() < 1.0e-6);
    let (crossover, phase_margin, gain_margin, maximum_sensitivity) = frequency_metrics();
    assert!((crossover - 3.10).abs() <= 0.08, "crossover={crossover:.4}");
    assert!(phase_margin >= 35.0, "phase margin={phase_margin:.3} deg");
    assert!(gain_margin >= 1.7, "gain margin={gain_margin:.3}");
    assert!(maximum_sensitivity <= 2.5, "Ms={maximum_sensitivity:.3}");
    let fine_pole_radius = fine_discrete_dominant_pole_radius();
    assert!((fine_pole_radius - 0.998_179).abs() <= 0.000_1);
    assert!(
        fine_pole_radius < 1.0,
        "fine discrete pole radius={fine_pole_radius:.7}"
    );

    let nominal = run_case("simulation.toml", false);
    let p_settling = settling_time(&nominal, 0);
    let pd_settling = settling_time(&nominal, 1);
    let pid_settling = settling_time(&nominal, 2);
    let coarse_settling = settling_time(&nominal, 3);
    assert!(
        p_settling.is_none(),
        "P unexpectedly settled: {p_settling:?}"
    );
    assert!(pd_settling.is_some(), "PD must damp the commanded move");
    assert!(
        final_window_error(&nominal, 1) >= 0.15,
        "PD must retain load offset"
    );
    assert!(
        pid_settling.is_some_and(|time| time <= 5.0),
        "PID settling={pid_settling:?}"
    );
    assert!(overshoot_percent(&nominal, 2) <= 35.0);
    assert!(final_window_error(&nominal, 2) <= 0.02);
    assert!(quiet_torque_rms(&nominal, 2) <= 0.10);
    assert!(coarse_settling.is_none() || overshoot_percent(&nominal, 3) > 35.0);
    assert!(nominal.iter().all(|sample| sample.revision == 78));
    assert!(nominal.iter().all(|sample| sample.fine_update));
    assert_eq!(
        nominal.iter().filter(|sample| sample.coarse_update).count(),
        CYCLES / 10
    );
    assert!(nominal.iter().all(|sample| {
        sample
            .raw_position
            .iter()
            .chain(sample.true_position.iter())
            .chain(sample.velocity.iter())
            .chain(sample.filtered_position.iter())
            .all(|value| value.is_finite())
    }));
    assert!(nominal.iter().all(|sample| {
        sample.requested[..3]
            .iter()
            .zip(sample.applied[..3].iter())
            .all(|(requested, applied)| (requested - applied).abs() < 1.0e-9)
    }));
    assert!(nominal
        .iter()
        .any(|sample| (sample.requested[3] - sample.applied[3]).abs() > 0.1));

    let mut variant_errors = Vec::new();
    for config in [
        "scenarios/heavy-payload.toml",
        "scenarios/reduced-damping.toml",
    ] {
        let trace = run_case(config, false);
        let error = final_window_error(&trace, 2);
        assert!(error <= 0.03, "{config} selected PID error");
        assert!(trace.iter().all(|sample| sample.velocity[2].abs() <= 5.0));
        variant_errors.push(error);
    }
    let noisy = run_case("scenarios/doubled-noise.toml", false);
    let doubled_noise_rms = quiet_torque_rms(&noisy, 2);
    assert!(doubled_noise_rms > quiet_torque_rms(&nominal, 2));
    assert!(doubled_noise_rms <= 0.20);
    let abnormal = run_case("simulation.toml", true);
    assert!(abnormal.iter().any(|sample| !sample.automatic));
    assert!(abnormal.iter().any(|sample| !sample.quality));
    assert!(abnormal.windows(2).all(|pair| {
        (0..3).all(|candidate| {
            (pair[1].applied[candidate] - pair[0].applied[candidate]).abs() <= 2.0 + 1.0e-9
        })
    }));
    for end_time in [26.5_f64, 27.2] {
        let stopped = abnormal
            .iter()
            .filter(|sample| sample.time < end_time)
            .last()
            .unwrap();
        assert!(
            stopped.applied.iter().all(|value| value.abs() < 1.0e-9),
            "controlled stop must reach zero torque before {end_time:.1}s: {:?}",
            stopped.applied
        );
    }

    let time: Vec<f64> = nominal.iter().map(|sample| sample.time).collect();
    let setpoint: Vec<f64> = nominal.iter().map(|sample| sample.setpoint).collect();
    let pid_position: Vec<f64> = nominal
        .iter()
        .map(|sample| sample.true_position[2])
        .collect();
    let delayed_position: Vec<f64> = nominal
        .iter()
        .map(|sample| sample.true_position[3])
        .collect();
    let pid_torque: Vec<f64> = nominal.iter().map(|sample| sample.applied[2]).collect();
    let delayed_torque: Vec<f64> = nominal.iter().map(|sample| sample.applied[3]).collect();
    let architecture = [0.0, 1.0, 2.0, 3.0];
    let final_error = [
        final_window_error(&nominal, 0),
        final_window_error(&nominal, 1),
        final_window_error(&nominal, 2),
        final_window_error(&nominal, 3),
    ];
    let mut summary =
        String::from("architecture,settling_s,overshoot_percent,final_error_rad,verdict\n");
    writeln!(
        summary,
        "P,not-settled,,{:.9},reject-no-settling",
        final_window_error(&nominal, 0)
    )
    .unwrap();
    writeln!(
        summary,
        "PD,{:.9},{:.9},{:.9},reject-load-offset",
        pd_settling.unwrap(),
        overshoot_percent(&nominal, 1),
        final_window_error(&nominal, 1)
    )
    .unwrap();
    writeln!(
        summary,
        "filtered-PID,{:.9},{:.9},{:.9},selected-unapplied",
        pid_settling.unwrap(),
        overshoot_percent(&nominal, 2),
        final_window_error(&nominal, 2)
    )
    .unwrap();
    writeln!(
        summary,
        "delayed-PID,{},{:.9},{:.9},reject-digital-response",
        coarse_settling.map_or_else(|| "not-settled".to_string(), |value| format!("{value:.9}")),
        overshoot_percent(&nominal, 3),
        final_window_error(&nominal, 3)
    )
    .unwrap();
    evidence::retain(
        &project_path("figures"),
        "servo-capstone-evidence",
        "The filtered PID survives the flexible mode and digital implementation",
        "The delayed candidate exposes why a continuous design must be qualified with the executed sample, hold, filter, and actuator limits.",
        &[
            Panel {
                title: "Indexing-table position",
                x_unit: "time (s)",
                y_unit: "rad",
                x_minimum: 0.0,
                x_maximum: 30.0,
                y_minimum: -0.25,
                y_maximum: 1.4,
                series: vec![
                    Series { name: "Setpoint", color: "#64748b", dashed: true, x: &time, y: &setpoint },
                    Series { name: "Filtered PID", color: "#2563eb", dashed: false, x: &time, y: &pid_position },
                    Series { name: "Delayed PID", color: "#dc2626", dashed: false, x: &time, y: &delayed_position },
                ],
            },
            Panel {
                title: "Applied motor torque",
                x_unit: "time (s)",
                y_unit: "N*m",
                x_minimum: 0.0,
                x_maximum: 30.0,
                y_minimum: -10.0,
                y_maximum: 10.0,
                series: vec![
                    Series { name: "Filtered PID", color: "#2563eb", dashed: false, x: &time, y: &pid_torque },
                    Series { name: "Delayed PID", color: "#dc2626", dashed: false, x: &time, y: &delayed_torque },
                ],
            },
        ],
        &[RetainedSeries { panel: "Architecture comparison", name: "Final load error", x: &architecture, y: &final_error, x_unit: "architecture index", y_unit: "rad" }],
        &summary,
    );

    println!(
        "servo capstone: resonance={resonance:.3} rad/s; crossover={crossover:.3} rad/s; PM={phase_margin:.2} deg; GM={gain_margin:.2}; Ms={maximum_sensitivity:.2}; fine pole radius={fine_pole_radius:.6}; P/PD/PID/coarse settling={p_settling:?}/{pd_settling:?}/{pid_settling:?}/{coarse_settling:?}; PID/coarse overshoot={:.1}/{:.1}%; PID final error={:.4} rad; PID torque RMS={:.4} N*m; PID/coarse movement={:.1}/{:.1} N*m; heavy/reduced-damping errors={:.4}/{:.4} rad; doubled-noise RMS={doubled_noise_rms:.4} N*m",
        overshoot_percent(&nominal, 2),
        overshoot_percent(&nominal, 3),
        final_window_error(&nominal, 2),
        quiet_torque_rms(&nominal, 2),
        torque_total_variation(&nominal, 2),
        torque_total_variation(&nominal, 3),
        variant_errors[0],
        variant_errors[1],
    );
}
