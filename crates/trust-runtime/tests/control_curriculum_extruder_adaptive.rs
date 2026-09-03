use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const DT: f64 = 0.1;
const FORGETTING: f64 = 0.98;
const INFORMATION_FORGETTING: f64 = 0.98;
const INFORMATION_THRESHOLD: f64 = 0.10;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(
            "../../examples/control/12-nonlinear-adaptive-and-robust-control/71-extruder-adaptive",
        )
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 71 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 71 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 71 program before configuration")
        .replace(
            "RequestedArchitecture : INT :=",
            "RequestedArchitecture AT %IW64 : INT :=",
        )
        .replace(
            "ActiveArchitecture : INT :=",
            "ActiveArchitecture AT %MW2 : INT :=",
        )
        .replace("ResinGrade : INT :=", "ResinGrade AT %IW66 : INT :=")
        // Map the longer name first: a generic `Enabled` replacement would
        // otherwise also match the suffix of `ProbeEnabled`.
        .replace("ProbeEnabled : BOOL :=", "ProbeEnabled AT %IX68.3 : BOOL :=")
        .replace("Enabled : BOOL :=", "Enabled AT %IX68.0 : BOOL :=")
        .replace(
            "AdaptationRequested : BOOL :=",
            "AdaptationRequested AT %IX68.1 : BOOL :=",
        )
        .replace("DataHealthy : BOOL :=", "DataHealthy AT %IX68.2 : BOOL :=")
        .replace(
            "AdaptationActive : BOOL :=",
            "AdaptationActive AT %MX7.0 : BOOL :=",
        )
        .replace("EstimateFrozen : BOOL :=", "EstimateFrozen AT %MX7.1 : BOOL :=")
        .replace(
            "PersistentExcitation : BOOL :=",
            "PersistentExcitation AT %MX7.2 : BOOL :=",
        )
        .replace(
            "EstimateQualified : BOOL :=",
            "EstimateQualified AT %MX7.3 : BOOL :=",
        )
        .replace("FallbackActive : BOOL :=", "FallbackActive AT %MX7.4 : BOOL :=")
        .replace(
            "ProjectionActive : BOOL :=",
            "ProjectionActive AT %MX7.5 : BOOL :=",
        )
        .replace("OutputSaturated : BOOL :=", "OutputSaturated AT %MX7.6 : BOOL :=")
        .replace(
            "EstimatorInitialized : BOOL :=",
            "EstimatorInitialized AT %MX7.7 : BOOL :=",
        )
        .replace("PressureSetpoint : LREAL :=", "PressureSetpoint AT %IL72 : LREAL :=")
        .replace("EstimatedGain : LREAL :=", "EstimatedGain AT %ML16 : LREAL :=")
        .replace("EstimatedBias : LREAL :=", "EstimatedBias AT %ML24 : LREAL :=")
        .replace(
            "\n    Covariance11 : LREAL := 10.0;",
            "\n    Covariance11 AT %ML32 : LREAL := 10.0;",
        )
        .replace(
            "\n    Covariance12 : LREAL := 0.0;",
            "\n    Covariance12 AT %ML40 : LREAL := 0.0;",
        )
        .replace(
            "\n    Covariance22 : LREAL := 10.0;",
            "\n    Covariance22 AT %ML48 : LREAL := 10.0;",
        )
        .replace("Information11 : LREAL :=", "Information11 AT %ML56 : LREAL :=")
        .replace("Information12 : LREAL :=", "Information12 AT %ML64 : LREAL :=")
        .replace("Information22 : LREAL :=", "Information22 AT %ML72 : LREAL :=")
        .replace("CovarianceTrace : LREAL :=", "CovarianceTrace AT %ML80 : LREAL :=")
        .replace(
            "InformationDeterminant : LREAL :=",
            "InformationDeterminant AT %ML88 : LREAL :=",
        )
        .replace("Innovation : LREAL :=", "Innovation AT %ML96 : LREAL :=")
        .replace(
            "ActiveProportionalGain : LREAL :=",
            "ActiveProportionalGain AT %ML104 : LREAL :=",
        )
        .replace(
            "ActiveIntegralGain : LREAL :=",
            "ActiveIntegralGain AT %ML112 : LREAL :=",
        )
        .replace("Probe : LREAL :=", "Probe AT %ML120 : LREAL :=")
        .replace(
            "ScrewSpeedRequested : LREAL :=",
            "ScrewSpeedRequested AT %ML128 : LREAL :=",
        )
        .replace(
            "ScrewSpeedApplied : LREAL :=",
            "ScrewSpeedApplied AT %QL0 : LREAL :=",
        )
        .replace("DiePressure : LREAL :=", "DiePressure AT %IL0 : LREAL :=")
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "extruder-adaptive/src/AdaptiveExtruderController.st",
            project_file("src/AdaptiveExtruderController.st"),
        ),
        SourceFile::with_path("extruder-adaptive/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 71 Structured Text and Control library")
}

fn simulation(file: &str) -> SimulationController {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "trust-control-extruder-adaptive-{}-{stamp}.toml",
        std::process::id()
    ));
    std::fs::write(&path, project_file(file)).expect("write Lesson 71 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 71 simulation config");
    let _ = std::fs::remove_file(path);
    SimulationController::new(config)
}

fn write_lreal(runtime: &mut Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid LREAL address"),
            Value::LWord(value.to_bits()),
        )
        .expect("write LREAL lesson input");
}

fn write_int(runtime: &mut Runtime, address: &str, value: i16) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid INT address"),
            Value::Word(value as u16),
        )
        .expect("write INT lesson input");
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid BOOL address"),
            Value::Bool(value),
        )
        .expect("write BOOL lesson input");
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid LREAL address"))
        .expect("read LREAL lesson signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn read_int(runtime: &Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid INT address"))
        .expect("read INT lesson signal")
    {
        Value::Word(bits) => bits as i16,
        value => panic!("expected INT, got {value:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid BOOL address"))
        .expect("read BOOL lesson signal")
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

#[derive(Debug, Clone, Copy)]
struct RlsOracle {
    initialized: bool,
    gain: f64,
    bias_scaled: f64,
    p11: f64,
    p12: f64,
    p22: f64,
    r11: f64,
    r12: f64,
    r22: f64,
    previous_pressure: f64,
    previous_applied: f64,
    previous_saturated: bool,
    innovation: f64,
    active: bool,
}

impl RlsOracle {
    fn new() -> Self {
        Self {
            initialized: false,
            gain: 1.0,
            bias_scaled: 0.0,
            p11: 10.0,
            p12: 0.0,
            p22: 10.0,
            r11: 0.0,
            r12: 0.0,
            r22: 0.0,
            previous_pressure: 40.0,
            previous_applied: 40.0,
            previous_saturated: false,
            innovation: 0.0,
            active: false,
        }
    }

    fn update(
        &mut self,
        pressure: f64,
        enabled: bool,
        requested: bool,
        healthy: bool,
        probe_authorized: bool,
    ) {
        if !self.initialized {
            self.initialized = true;
            self.previous_pressure = pressure;
            self.active = false;
            return;
        }

        let phi1 = self.previous_applied / 100.0;
        let phi2 = 1.0;
        if enabled && requested && healthy && !self.previous_saturated {
            self.r11 = INFORMATION_FORGETTING * self.r11 + phi1 * phi1;
            self.r12 = INFORMATION_FORGETTING * self.r12 + phi1 * phi2;
            self.r22 = INFORMATION_FORGETTING * self.r22 + phi2 * phi2;
        }
        let determinant = self.r11 * self.r22 - self.r12 * self.r12;
        self.active = enabled
            && requested
            && healthy
            && !self.previous_saturated
            && probe_authorized
            && determinant >= INFORMATION_THRESHOLD;
        self.innovation = 0.0;
        if self.active {
            let observation =
                (self.previous_pressure + 5.0 * (pressure - self.previous_pressure) / DT) / 100.0;
            let p_phi1 = self.p11 * phi1 + self.p12 * phi2;
            let p_phi2 = self.p12 * phi1 + self.p22 * phi2;
            let denominator = FORGETTING + phi1 * p_phi1 + phi2 * p_phi2;
            let l1 = p_phi1 / denominator;
            let l2 = p_phi2 / denominator;
            self.innovation = observation - (self.gain * phi1 + self.bias_scaled * phi2);
            self.gain = (self.gain + l1 * self.innovation).clamp(0.5, 1.5);
            self.bias_scaled = (self.bias_scaled + l2 * self.innovation).clamp(-0.2, 0.2);
            self.p11 = (self.p11 - l1 * p_phi1) / FORGETTING;
            self.p12 = (self.p12 - l1 * p_phi2) / FORGETTING;
            self.p22 = (self.p22 - l2 * p_phi2) / FORGETTING;
        }
        self.previous_pressure = pressure;
    }

    fn finish_cycle(&mut self, applied: f64, saturated: bool) {
        self.previous_applied = applied;
        self.previous_saturated = saturated;
    }

    fn information_determinant(&self) -> f64 {
        self.r11 * self.r22 - self.r12 * self.r12
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    time: f64,
    setpoint: f64,
    pressure: f64,
    applied: f64,
    estimated_gain: f64,
    estimated_bias: f64,
    qualified: bool,
    persistent_excitation: bool,
    adaptation_active: bool,
    data_healthy: bool,
    information_determinant: f64,
}

#[derive(Debug)]
struct Record {
    samples: Vec<Sample>,
    movement: f64,
    saturation_seconds: f64,
    projection_count: usize,
}

fn setpoint(time: f64) -> f64 {
    if time < 30.0 {
        40.0
    } else if time < 90.0 {
        50.0
    } else {
        60.0
    }
}

fn effective_gain(time: f64) -> f64 {
    let grade_gain = if time < 60.0 { 1.0 } else { 0.75 };
    grade_gain * (1.0 + 0.15 * (2.0 * std::f64::consts::PI * time / 120.0).sin())
}

fn run(mode: i16, supervision: bool) -> Record {
    let mut runtime = build_runtime();
    let mut plant = simulation(if supervision {
        "simulation-no-drift.toml"
    } else {
        "simulation.toml"
    });
    plant.apply_pre_cycle(Duration::ZERO, &mut runtime).unwrap();
    write_lreal(&mut runtime, "%ML16", 1.0);
    write_lreal(&mut runtime, "%ML24", 0.0);
    write_int(&mut runtime, "%IW64", mode);
    write_bool(&mut runtime, "%IX68.0", true);
    write_bool(&mut runtime, "%IX68.1", mode == 2);
    write_bool(&mut runtime, "%IX68.2", true);
    write_bool(&mut runtime, "%IX68.3", true);

    let sample_count = if supervision { 1200 } else { 1100 };
    let mut oracle = RlsOracle::new();
    let mut samples = Vec::with_capacity(sample_count);
    let mut movement = 0.0;
    let mut saturation_seconds = 0.0;
    let mut projection_count = 0;
    let mut previous_applied = 40.0;
    for sample_index in 0..sample_count {
        let time = sample_index as f64 * DT;
        let healthy = !supervision || !(90.0..95.0).contains(&time);
        let probe = !supervision || time < 60.0 || time >= 95.0;
        let reference = if supervision { 40.0 } else { setpoint(time) };
        write_lreal(&mut runtime, "%IL72", reference);
        write_int(
            &mut runtime,
            "%IW66",
            if supervision || time < 60.0 { 0 } else { 1 },
        );
        write_bool(&mut runtime, "%IX68.2", healthy);
        write_bool(&mut runtime, "%IX68.3", probe);
        assert_eq!(
            read_bool(&runtime, "%IX68.3"),
            probe,
            "sample {sample_index}: probe authorization write must reach mapped input",
        );

        let pressure = read_lreal(&runtime, "%IL0");
        if mode == 2 {
            oracle.update(pressure, true, true, healthy, probe);
        }
        runtime
            .execute_cycle()
            .expect("execute extruder controller interval");

        let applied = read_lreal(&runtime, "%QL0");
        let saturated = read_bool(&runtime, "%MX7.6");
        assert!((0.0..=100.0).contains(&applied));
        assert_eq!(
            read_int(&runtime, "%MW2"),
            if mode == 2 && !read_bool(&runtime, "%MX7.3") {
                1
            } else {
                mode
            }
        );
        if mode == 2 {
            let tolerance = 2e-9;
            let actual_gain = read_lreal(&runtime, "%ML16");
            let actual_bias = read_lreal(&runtime, "%ML24");
            assert!(
                (actual_gain - oracle.gain).abs() <= tolerance,
                "sample {sample_index} time={time:.1}: gain actual={actual_gain:.15} oracle={:.15}, probe={probe}, probe after cycle={}, generated probe={}, frozen reason={}, healthy={healthy}, active actual={} oracle={}",
                oracle.gain,
                read_bool(&runtime, "%IX68.3"),
                read_lreal(&runtime, "%ML120"),
                read_int(&runtime, "%MW140"),
                read_bool(&runtime, "%MX7.0"),
                oracle.active,
            );
            assert!(
                (actual_bias - 100.0 * oracle.bias_scaled).abs() <= tolerance,
                "sample {sample_index} time={time:.1}: bias actual={actual_bias:.15} oracle={:.15}, probe={probe}, healthy={healthy}, active actual={} oracle={}",
                100.0 * oracle.bias_scaled,
                read_bool(&runtime, "%MX7.0"),
                oracle.active,
            );
            let actual_p11 = read_lreal(&runtime, "%ML32");
            let actual_p12 = read_lreal(&runtime, "%ML40");
            let actual_p22 = read_lreal(&runtime, "%ML48");
            assert!(
                (actual_p11 - oracle.p11).abs() <= tolerance,
                "sample {sample_index}: P11 actual={actual_p11:.15} oracle={:.15}, initialized={}, trace={:.15}",
                oracle.p11,
                read_bool(&runtime, "%MX7.7"),
                read_lreal(&runtime, "%ML80")
            );
            assert!(
                (actual_p12 - oracle.p12).abs() <= tolerance,
                "sample {sample_index}: P12 actual={actual_p12:.15} oracle={:.15}",
                oracle.p12
            );
            assert!(
                (actual_p22 - oracle.p22).abs() <= tolerance,
                "sample {sample_index}: P22 actual={actual_p22:.15} oracle={:.15}",
                oracle.p22
            );
            assert!((read_lreal(&runtime, "%ML56") - oracle.r11).abs() <= tolerance);
            assert!((read_lreal(&runtime, "%ML64") - oracle.r12).abs() <= tolerance);
            assert!((read_lreal(&runtime, "%ML72") - oracle.r22).abs() <= tolerance);
            assert!(
                (read_lreal(&runtime, "%ML88") - oracle.information_determinant()).abs()
                    <= tolerance
            );
            assert!((read_lreal(&runtime, "%ML96") - oracle.innovation).abs() <= tolerance);
            assert_eq!(read_bool(&runtime, "%MX7.0"), oracle.active);
            oracle.finish_cycle(applied, saturated);
        }

        movement += (applied - previous_applied).abs();
        if saturated {
            saturation_seconds += DT;
        }
        if read_bool(&runtime, "%MX7.5") {
            projection_count += 1;
        }
        samples.push(Sample {
            time,
            setpoint: reference,
            pressure,
            applied,
            estimated_gain: read_lreal(&runtime, "%ML16"),
            estimated_bias: read_lreal(&runtime, "%ML24"),
            qualified: read_bool(&runtime, "%MX7.3"),
            persistent_excitation: read_bool(&runtime, "%MX7.2"),
            adaptation_active: read_bool(&runtime, "%MX7.0"),
            data_healthy: healthy,
            information_determinant: read_lreal(&runtime, "%ML88"),
        });
        previous_applied = applied;
        let now = Duration::from_millis((sample_index as i64 + 1) * 100);
        plant.apply_post_cycle(now, &runtime).unwrap();
        plant.apply_pre_cycle(now, &mut runtime).unwrap();
    }
    Record {
        samples,
        movement,
        saturation_seconds,
        projection_count,
    }
}

fn iae(record: &Record, start: f64, end: f64) -> f64 {
    record
        .samples
        .iter()
        .filter(|sample| sample.time >= start && sample.time < end)
        .map(|sample| (sample.setpoint - sample.pressure).abs() * DT)
        .sum()
}

fn settling_after(record: &Record, start: f64, end: f64) -> Option<f64> {
    let window = 50;
    let first = record
        .samples
        .iter()
        .position(|sample| sample.time >= start)?;
    let last = record
        .samples
        .iter()
        .position(|sample| sample.time >= end)?;
    (first..last.saturating_sub(window)).find_map(|index| {
        record.samples[index..index + window]
            .iter()
            .all(|sample| (sample.setpoint - sample.pressure).abs() <= 0.5)
            .then_some(record.samples[index].time - start)
    })
}

#[test]
fn control_curriculum_extruder_adaptation_requires_information_and_bounds() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 71 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 71 must use schema 4");
    assert_eq!(lesson["sequence"], 71);
    assert_eq!(lesson["prerequisites"][0], "control-robot-joint-nonlinear");
    assert_eq!(lesson["nextLesson"], "control-flexible-airframe-robust");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_extruder_adaptation_requires_information_and_bounds"
    );

    assert!((5.0_f64 / (1.0 * 3.0) - 1.666_666_666_666_666_7).abs() <= 1e-12);
    assert!((1.0_f64 / (0.75 * 3.0) - 0.444_444_444_444_444_4).abs() <= 1e-12);
    assert!((5.0_f64 / (0.65 * 3.0) - 2.564_102_564_102_564).abs() <= 1e-12);
    assert!((effective_gain(30.0) - 1.15).abs() <= 1e-12);
    assert!((effective_gain(90.0) - 0.6375).abs() <= 1e-12);

    let fixed = run(0, false);
    let scheduled = run(1, false);
    let adaptive = run(2, false);
    let supervised = run(2, true);

    let fixed_grade_b_iae = iae(&fixed, 60.0, 110.0);
    let scheduled_grade_b_iae = iae(&scheduled, 60.0, 110.0);
    let adaptive_grade_b_iae = iae(&adaptive, 60.0, 110.0);
    assert!(adaptive.samples.iter().all(|sample| {
        sample.pressure.is_finite()
            && sample.applied.is_finite()
            && sample.estimated_gain.is_finite()
            && sample.estimated_bias.is_finite()
            && (0.5..=1.5).contains(&sample.estimated_gain)
            && (-20.0..=20.0).contains(&sample.estimated_bias)
    }));
    assert!(
        adaptive.projection_count > 0,
        "the grade transition must exercise visible bounded projection"
    );
    assert!(
        adaptive
            .samples
            .iter()
            .any(|sample| { (sample.estimated_gain - 0.5).abs() <= 1e-12 && !sample.qualified }),
        "a projected boundary estimate must not receive adaptive authority"
    );
    assert!(scheduled_grade_b_iae < fixed_grade_b_iae);
    assert!(adaptive_grade_b_iae <= scheduled_grade_b_iae);

    let qualified_estimation: Vec<_> = adaptive
        .samples
        .iter()
        .filter(|sample| sample.time >= 30.0 && sample.time < 110.0 && sample.qualified)
        .collect();
    assert!(
        !qualified_estimation.is_empty(),
        "adaptive estimate must qualify"
    );
    let estimate_rmse = (qualified_estimation
        .iter()
        .map(|sample| (sample.estimated_gain - effective_gain(sample.time)).powi(2))
        .sum::<f64>()
        / qualified_estimation.len() as f64)
        .sqrt();
    for requested_time in [30.0, 59.9, 65.0, 90.0, 109.9] {
        if let Some(sample) = adaptive
            .samples
            .iter()
            .find(|sample| (sample.time - requested_time).abs() < 0.05)
        {
            eprintln!(
                "extruder estimator t={:.1}: Khat={:.5}, Ktrue={:.5}, Bhat={:.3}, qualified={}, PE={}",
                sample.time,
                sample.estimated_gain,
                effective_gain(sample.time),
                sample.estimated_bias,
                sample.qualified,
                sample.persistent_excitation
            );
        }
    }
    assert!(
        estimate_rmse <= 0.08,
        "gain-estimate RMSE was {estimate_rmse}"
    );

    let authorization_frozen: Vec<_> = supervised
        .samples
        .iter()
        .filter(|sample| sample.time >= 60.0 && sample.time < 90.0)
        .collect();
    assert!(!authorization_frozen.is_empty());
    let authorization_frozen_gain = authorization_frozen[0].estimated_gain;
    let authorization_frozen_bias = authorization_frozen[0].estimated_bias;
    assert!(
        authorization_frozen.iter().all(|sample| {
            !sample.adaptation_active
                && (sample.estimated_gain - authorization_frozen_gain).abs() <= 1e-12
                && (sample.estimated_bias - authorization_frozen_bias).abs() <= 1e-12
        }),
        "removing probe authorization must freeze both estimates immediately"
    );

    let information_lost: Vec<_> = supervised
        .samples
        .iter()
        .filter(|sample| {
            sample.time >= 85.0
                && sample.time < 90.0
                && !sample.persistent_excitation
                && !sample.adaptation_active
        })
        .collect();
    for requested_time in [60.0, 75.0, 89.9] {
        if let Some(sample) = supervised
            .samples
            .iter()
            .find(|sample| (sample.time - requested_time).abs() < 0.05)
        {
            eprintln!(
                "extruder information t={:.1}: determinant={:.6}, active={}",
                sample.time, sample.information_determinant, sample.adaptation_active
            );
        }
    }
    assert!(
        !information_lost.is_empty(),
        "the constant-grade record must eventually lose persistent excitation"
    );
    assert!(supervised
        .samples
        .iter()
        .filter(|sample| !sample.data_healthy)
        .all(|sample| { sample.applied == 0.0 && !sample.adaptation_active }));
    assert!(supervised
        .samples
        .iter()
        .any(|sample| { sample.time >= 105.0 && sample.adaptation_active && sample.data_healthy }));
    let frozen_start = authorization_frozen[0];
    let frozen_end = authorization_frozen[authorization_frozen.len() - 1];
    let recovered = supervised
        .samples
        .iter()
        .find(|sample| sample.time >= 105.0 && sample.adaptation_active)
        .expect("authorized healthy excitation must recover adaptation");
    eprintln!(
        "extruder supervision freeze Khat={:.6} Bhat={:.6} bar, determinant={:.6}->{:.6}, recovery active at {:.1} s",
        frozen_start.estimated_gain,
        frozen_start.estimated_bias,
        frozen_start.information_determinant,
        frozen_end.information_determinant,
        recovered.time,
    );

    let mut projection = build_runtime();
    let mut projection_plant = simulation("simulation.toml");
    projection_plant
        .apply_pre_cycle(Duration::ZERO, &mut projection)
        .unwrap();
    write_lreal(&mut projection, "%ML16", 2.3);
    write_lreal(&mut projection, "%ML24", 0.0);
    write_lreal(&mut projection, "%IL72", 40.0);
    write_int(&mut projection, "%IW64", 2);
    write_int(&mut projection, "%IW66", 0);
    write_bool(&mut projection, "%IX68.0", true);
    write_bool(&mut projection, "%IX68.1", false);
    write_bool(&mut projection, "%IX68.2", true);
    write_bool(&mut projection, "%IX68.3", false);
    projection
        .execute_cycle()
        .expect("execute projected-estimate case");
    assert_eq!(read_lreal(&projection, "%ML16"), 1.5);
    assert!(read_bool(&projection, "%MX7.5"));

    write_bool(&mut projection, "%IX68.0", false);
    projection
        .execute_cycle()
        .expect("disable projected-estimate case");
    assert_eq!(read_lreal(&projection, "%QL0"), 0.0);

    eprintln!(
        "extruder fixed/scheduled/adaptive grade-B IAE={fixed_grade_b_iae:.3}/{scheduled_grade_b_iae:.3}/{adaptive_grade_b_iae:.3} bar*s, movement={:.1}/{:.1}/{:.1} pp, settling={:?}/{:?}/{:?}, estimate RMSE={estimate_rmse:.5} bar/%, saturation={:.1}/{:.1}/{:.1} s, projections={}",
        fixed.movement,
        scheduled.movement,
        adaptive.movement,
        settling_after(&fixed, 90.0, 110.0),
        settling_after(&scheduled, 90.0, 110.0),
        settling_after(&adaptive, 90.0, 110.0),
        fixed.saturation_seconds,
        scheduled.saturation_seconds,
        adaptive.saturation_seconds,
        adaptive.projection_count,
    );

    let architecture = [0.0, 1.0, 2.0];
    let grade_b_iae = [
        fixed_grade_b_iae,
        scheduled_grade_b_iae,
        adaptive_grade_b_iae,
    ];
    let movement = [fixed.movement, scheduled.movement, adaptive.movement];
    let mut summary =
        String::from("architecture,grade_b_iae_bar_s,cumulative_output_movement_pp,saturation_s\n");
    for (index, name) in ["fixed", "scheduled", "adaptive"].iter().enumerate() {
        let saturation = [
            fixed.saturation_seconds,
            scheduled.saturation_seconds,
            adaptive.saturation_seconds,
        ][index];
        writeln!(
            summary,
            "{name},{:.9},{:.9},{saturation:.3}",
            grade_b_iae[index], movement[index]
        )
        .unwrap();
    }
    writeln!(summary, "adaptive_gain_estimate_rmse,{estimate_rmse:.9},,").unwrap();
    let figure_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/12-nonlinear-adaptive-and-robust-control/71-extruder-adaptive/figures");
    evidence::retain(
        &figure_root,
        "extruder-adaptive-evidence",
        "Adaptation earns authority only from informative healthy data",
        "Index 0 is fixed, 1 is scheduled, and 2 is bounded supervised adaptation.",
        &[
            evidence::Panel {
                title: "Grade-B tracking error",
                x_unit: "architecture index",
                y_unit: "bar s",
                x_minimum: -0.1,
                x_maximum: 2.1,
                y_minimum: 0.0,
                y_maximum: grade_b_iae.iter().copied().fold(0.0_f64, f64::max) * 1.1,
                series: vec![evidence::Series {
                    name: "IAE",
                    color: "#2563eb",
                    dashed: false,
                    x: &architecture,
                    y: &grade_b_iae,
                }],
            },
            evidence::Panel {
                title: "Cumulative valve-command movement",
                x_unit: "architecture index",
                y_unit: "percentage points",
                x_minimum: -0.1,
                x_maximum: 2.1,
                y_minimum: 0.0,
                y_maximum: movement.iter().copied().fold(0.0_f64, f64::max) * 1.1,
                series: vec![evidence::Series {
                    name: "Movement",
                    color: "#16a34a",
                    dashed: false,
                    x: &architecture,
                    y: &movement,
                }],
            },
        ],
        &[],
        &summary,
    );
}
