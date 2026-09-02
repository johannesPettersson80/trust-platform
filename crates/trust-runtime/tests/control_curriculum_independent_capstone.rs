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

const NOMINAL_SECONDS: usize = 300;

fn project_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/84-independent")
        .join(relative)
}

fn project_file(relative: &str) -> String {
    let path = project_path(relative);
    assert!(
        path.is_file(),
        "required Lesson 84 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 84 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 84 program before configuration")
        .replace("OperatingMode : DINT :=", "OperatingMode AT %MD0 : DINT :=")
        .replace("Enabled : BOOL :=", "Enabled AT %MX8.0 : BOOL :=")
        .replace(
            "PressureQualityGood : BOOL :=",
            "PressureQualityGood AT %MX8.1 : BOOL :=",
        )
        .replace(
            "DemandQualityGood : BOOL :=",
            "DemandQualityGood AT %MX8.2 : BOOL :=",
        )
        .replace(
            "DeliveryQualityGood : BOOL :=",
            "DeliveryQualityGood AT %MX8.3 : BOOL :=",
        )
        .replace(
            "CompressorAvailable : BOOL :=",
            "CompressorAvailable AT %MX8.4 : BOOL :=",
        )
        .replace(
            "FeedforwardQualified : BOOL :=",
            "FeedforwardQualified AT %MX8.5 : BOOL :=",
        )
        .replace(
            "TrackingActive : BOOL :=",
            "TrackingActive AT %MX8.6 : BOOL :=",
        )
        .replace("FaultActive : BOOL :=", "FaultActive AT %MX8.7 : BOOL :=")
        .replace("Initialized : BOOL :=", "Initialized AT %MX9.0 : BOOL :=")
        .replace("TimingValid : BOOL :=", "TimingValid AT %MX9.1 : BOOL :=")
        .replace(
            "PressureSetpoint : LREAL :=",
            "PressureSetpoint AT %ML16 : LREAL :=",
        )
        .replace(
            "MeasuredDemand : LREAL :=",
            "MeasuredDemand AT %ML24 : LREAL :=",
        )
        .replace(
            "ManualFeedbackRequest : LREAL :=",
            "ManualFeedbackRequest AT %ML32 : LREAL :=",
        )
        .replace(
            "ManualIntegratedRequest : LREAL :=",
            "ManualIntegratedRequest AT %ML40 : LREAL :=",
        )
        .replace(
            "FeedbackRequested : LREAL :=",
            "FeedbackRequested AT %ML48 : LREAL :=",
        )
        .replace(
            "IntegratedFeedback : LREAL :=",
            "IntegratedFeedback AT %ML56 : LREAL :=",
        )
        .replace(
            "IntegratedFeedforward : LREAL :=",
            "IntegratedFeedforward AT %ML64 : LREAL :=",
        )
        .replace(
            "IntegratedRequested : LREAL :=",
            "IntegratedRequested AT %ML72 : LREAL :=",
        )
        .replace(
            "PreviousFeedbackApplied : LREAL :=",
            "PreviousFeedbackApplied AT %ML80 : LREAL :=",
        )
        .replace(
            "PreviousIntegratedApplied : LREAL :=",
            "PreviousIntegratedApplied AT %ML88 : LREAL :=",
        )
        .replace(
            "ControllerRevision : DINT :=",
            "ControllerRevision AT %MD96 : DINT :=",
        )
        .replace("CycleTime : LREAL :=", "CycleTime AT %ML104 : LREAL :=")
        .replace(
            "FeedbackApplied : LREAL :=",
            "FeedbackApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "FeedbackPlantDemand : LREAL :=",
            "FeedbackPlantDemand AT %QL8 : LREAL :=",
        )
        .replace(
            "IntegratedApplied : LREAL :=",
            "IntegratedApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "IntegratedPlantDemand : LREAL :=",
            "IntegratedPlantDemand AT %QL24 : LREAL :=",
        )
        .replace(
            "FeedbackPressure : LREAL :=",
            "FeedbackPressure AT %IL0 : LREAL :=",
        )
        .replace(
            "FeedbackActualDelivery : LREAL :=",
            "FeedbackActualDelivery AT %IL8 : LREAL :=",
        )
        .replace(
            "IntegratedPressure : LREAL :=",
            "IntegratedPressure AT %IL16 : LREAL :=",
        )
        .replace(
            "IntegratedActualDelivery : LREAL :=",
            "IntegratedActualDelivery AT %IL24 : LREAL :=",
        )
}

fn build_runtime() -> Runtime {
    let source = project_file("src/Main.st");
    for required in ["CycleTime", "TimingValid"] {
        assert!(
            source.contains(required),
            "Lesson 84 must expose jitter-qualified {required}"
        );
    }
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            project_file("libraries/control/src/control.st"),
        ),
        SourceFile::with_path(
            "control/14-integrated-capstones/84-independent/src/CompressedAirController.st",
            project_file("src/CompressedAirController.st"),
        ),
        SourceFile::with_path("control/14-integrated-capstones/84-independent/src/Main.st", mapped_source()),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 84 Structured Text and Control library")
}

fn write_lreal(runtime: &mut Runtime, address: &str, value: f64) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid LREAL address"),
            Value::LWord(value.to_bits()),
        )
        .expect("write Lesson 84 LREAL");
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid BOOL address"),
            Value::Bool(value),
        )
        .expect("write Lesson 84 BOOL");
}

fn write_dint(runtime: &mut Runtime, address: &str, value: i32) {
    runtime
        .io_mut()
        .write(
            &IoAddress::parse(address).expect("valid DINT address"),
            Value::DWord(value as u32),
        )
        .expect("write Lesson 84 DINT");
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid LREAL address"))
        .expect("read Lesson 84 LREAL")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL at {address}, got {value:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid BOOL address"))
        .expect("read Lesson 84 BOOL")
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL at {address}, got {value:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &str) -> i32 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("valid DINT address"))
        .expect("read Lesson 84 DINT")
    {
        Value::DWord(value) => value as i32,
        value => panic!("expected DINT at {address}, got {value:?}"),
    }
}

fn configure(runtime: &mut Runtime, demand: f64, cycle_time: f64) {
    write_dint(runtime, "%MD0", 1);
    write_bool(runtime, "%MX8.0", true);
    write_bool(runtime, "%MX8.1", true);
    write_bool(runtime, "%MX8.2", true);
    write_bool(runtime, "%MX8.3", true);
    write_bool(runtime, "%MX8.4", true);
    write_lreal(runtime, "%ML16", 7.0);
    write_lreal(runtime, "%ML24", demand);
    write_lreal(runtime, "%ML104", cycle_time);
}

fn demand_at(time: f64) -> f64 {
    if time < 60.0 {
        50.0
    } else if time < 180.0 {
        65.0
    } else {
        55.0
    }
}

#[derive(Default, Debug)]
struct Metrics {
    iae: [f64; 2],
    maximum_departure: [f64; 2],
    normal_violation: [f64; 2],
    applied_movement: [f64; 2],
    peak_applied: [f64; 2],
    maximum_tracking_residual: [f64; 2],
}

fn run(config_name: &str, leak: f64, jitter: bool, noise: bool) -> Metrics {
    let mut runtime = build_runtime();
    let mut plant = SimulationController::new(
        SimulationConfig::load(project_path(config_name))
            .unwrap_or_else(|error| panic!("load Lesson 84 {config_name}: {error}")),
    );
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial compressed-air state");
    let mut metrics = Metrics::default();
    let mut previous = [50.0, 50.0];
    let mut elapsed_ms = 0i64;

    for cycle in 0..NOMINAL_SECONDS {
        let cycle_time = if jitter {
            if cycle % 2 == 0 {
                0.9
            } else {
                1.1
            }
        } else {
            1.0
        };
        let time = elapsed_ms as f64 / 1000.0;
        let demand = demand_at(time);
        let true_pressures = [read_lreal(&runtime, "%IL0"), read_lreal(&runtime, "%IL16")];
        if noise {
            let noise_value = if cycle % 2 == 0 { 0.005 } else { -0.005 };
            write_lreal(&mut runtime, "%IL0", true_pressures[0] + noise_value);
            write_lreal(&mut runtime, "%IL16", true_pressures[1] - noise_value);
        }
        configure(&mut runtime, demand, cycle_time);
        runtime
            .execute_cycle()
            .expect("execute compressed-air control interval");

        let applied = [read_lreal(&runtime, "%QL0"), read_lreal(&runtime, "%QL16")];
        let actual = [read_lreal(&runtime, "%IL8"), read_lreal(&runtime, "%IL24")];
        for index in 0..2 {
            assert!((0.0..=100.0).contains(&applied[index]));
            assert!(
                (applied[index] - previous[index]).abs() <= 10.0 * cycle_time + 1.0e-9,
                "applied slew violation at cycle {cycle}: {previous:?} -> {applied:?}"
            );
            let error = (7.0 - true_pressures[index]).abs();
            metrics.iae[index] += error * cycle_time;
            metrics.maximum_departure[index] = metrics.maximum_departure[index].max(error);
            metrics.normal_violation[index] += ((6.5 - true_pressures[index]).max(0.0)
                + (true_pressures[index] - 7.5).max(0.0))
                * cycle_time;
            metrics.applied_movement[index] += (applied[index] - previous[index]).abs();
            metrics.peak_applied[index] = metrics.peak_applied[index].max(applied[index]);
            metrics.maximum_tracking_residual[index] = metrics.maximum_tracking_residual[index]
                .max((applied[index] - actual[index]).abs());
        }
        if time == 60.0 {
            assert!((read_lreal(&runtime, "%ML64") - 15.0).abs() <= 1.0e-9);
            assert!(applied[1] > applied[0]);
        }
        assert_eq!(read_dint(&runtime, "%MD96"), 84);
        previous = applied;

        write_lreal(&mut runtime, "%QL8", demand + leak);
        write_lreal(&mut runtime, "%QL24", demand + leak);
        elapsed_ms += (cycle_time * 1000.0).round() as i64;
        let now = Duration::from_millis(elapsed_ms);
        plant
            .apply_post_cycle(now, &runtime)
            .expect("advance compressed-air plants");
        plant
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish compressed-air state");
    }
    metrics
}

#[derive(Clone, Copy, Debug)]
struct ComplexValue {
    re: f64,
    im: f64,
}

impl ComplexValue {
    fn subtract(self, other: Self) -> Self {
        Self {
            re: self.re - other.re,
            im: self.im - other.im,
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
}

fn cubic_roots(a: f64, b: f64, c: f64) -> [ComplexValue; 3] {
    let mut roots = [
        ComplexValue { re: 1.0, im: 0.0 },
        ComplexValue { re: -0.4, im: 0.9 },
        ComplexValue { re: -0.4, im: -0.9 },
    ];
    for _ in 0..100 {
        let previous = roots;
        for index in 0..3 {
            let x = previous[index];
            let x2 = x.multiply(x);
            let polynomial = x2
                .multiply(x)
                .subtract(ComplexValue {
                    re: -a * x2.re,
                    im: -a * x2.im,
                })
                .subtract(ComplexValue {
                    re: -b * x.re,
                    im: -b * x.im,
                })
                .subtract(ComplexValue { re: -c, im: 0.0 });
            let denominator = (0..3)
                .filter(|candidate| *candidate != index)
                .map(|candidate| x.subtract(previous[candidate]))
                .reduce(|left, right| left.multiply(right))
                .expect("two other roots");
            roots[index] = x.subtract(polynomial.divide(denominator));
        }
    }
    roots
}

#[derive(Debug)]
struct FrequencyEvidence {
    crossover: f64,
    phase_margin: f64,
    delay_margin: f64,
    sensitivity_peak: f64,
}

fn frequency_evidence(kp: f64, ki: f64, rate_gain: f64, lag: f64) -> FrequencyEvidence {
    let mut prior_magnitude = f64::NAN;
    let mut crossover = f64::NAN;
    let mut phase_margin = f64::NAN;
    let mut sensitivity_peak: f64 = 0.0;
    for index in 0..=200_000 {
        let exponent = -5.0 + 7.0 * index as f64 / 200_000.0;
        let omega = 10.0_f64.powf(exponent);
        let denominator = 1.0 + (omega * lag).powi(2);
        let plant_re = -rate_gain * lag / denominator;
        let plant_im = -rate_gain / omega / denominator;
        let controller_re = kp;
        let controller_im = -ki / omega;
        let loop_re = controller_re * plant_re - controller_im * plant_im;
        let loop_im = controller_re * plant_im + controller_im * plant_re;
        let magnitude = loop_re.hypot(loop_im);
        sensitivity_peak = sensitivity_peak.max(1.0 / (1.0 + loop_re).hypot(loop_im));
        if crossover.is_nan()
            && prior_magnitude.is_finite()
            && (prior_magnitude - 1.0) * (magnitude - 1.0) <= 0.0
        {
            crossover = omega;
            phase_margin = 180.0 + loop_im.atan2(loop_re).to_degrees();
        }
        prior_magnitude = magnitude;
    }
    FrequencyEvidence {
        crossover,
        phase_margin,
        delay_margin: phase_margin.to_radians() / crossover,
        sensitivity_peak,
    }
}

fn verify_abnormal_authority() {
    let mut runtime = build_runtime();
    write_lreal(&mut runtime, "%IL0", 7.0);
    write_lreal(&mut runtime, "%IL8", 50.0);
    write_lreal(&mut runtime, "%IL16", 7.0);
    write_lreal(&mut runtime, "%IL24", 50.0);
    configure(&mut runtime, 50.0, 1.0);
    runtime
        .execute_cycle()
        .expect("initialize Lesson 84 authority");

    write_bool(&mut runtime, "%MX8.2", false);
    runtime
        .execute_cycle()
        .expect("remove invalid demand feedforward");
    assert!(!read_bool(&runtime, "%MX8.5"));
    assert!(!read_bool(&runtime, "%MX8.7"));
    assert_eq!(read_lreal(&runtime, "%ML64"), 0.0);
    assert!(read_lreal(&runtime, "%QL16") > 0.0);

    write_bool(&mut runtime, "%MX8.2", true);
    write_bool(&mut runtime, "%MX8.1", false);
    runtime
        .execute_cycle()
        .expect("fail closed on invalid pressure");
    assert!(read_bool(&runtime, "%MX8.7"));
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL16"), 0.0);

    write_bool(&mut runtime, "%MX8.1", true);
    write_bool(&mut runtime, "%MX8.3", false);
    runtime
        .execute_cycle()
        .expect("fail closed on invalid delivery");
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL16"), 0.0);

    write_bool(&mut runtime, "%MX8.3", true);
    write_bool(&mut runtime, "%MX8.4", false);
    runtime
        .execute_cycle()
        .expect("fail closed on unavailable compressor");
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL16"), 0.0);

    write_bool(&mut runtime, "%MX8.4", true);
    write_dint(&mut runtime, "%MD0", 2);
    write_lreal(&mut runtime, "%ML32", 70.0);
    write_lreal(&mut runtime, "%ML40", 60.0);
    runtime.execute_cycle().expect("enter bounded Manual");
    assert!(read_bool(&runtime, "%MX8.6"));
    assert_eq!(read_lreal(&runtime, "%QL0"), 10.0);
    assert_eq!(read_lreal(&runtime, "%QL16"), 10.0);

    write_dint(&mut runtime, "%MD0", 3);
    runtime.execute_cycle().expect("execute ControlledStop");
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL16"), 0.0);

    write_dint(&mut runtime, "%MD0", 1);
    runtime
        .execute_cycle()
        .expect("restore Automatic with bounded request");
    assert!(read_lreal(&runtime, "%QL0") <= 10.0 + 1.0e-9);
    assert!(read_lreal(&runtime, "%QL16") <= 10.0 + 1.0e-9);
    assert_eq!(read_dint(&runtime, "%MD96"), 84);
}

fn manual_scan(runtime: &mut Runtime, request: f64, demand: f64, cycle_time: f64) {
    configure(runtime, demand, cycle_time);
    write_dint(runtime, "%MD0", 2);
    write_lreal(runtime, "%ML32", request);
    write_lreal(runtime, "%ML40", request);
    runtime
        .execute_cycle()
        .expect("execute bounded identification scan");
}

fn initialized_manual_plant() -> (Runtime, SimulationController) {
    let mut runtime = build_runtime();
    let mut plant = SimulationController::new(
        SimulationConfig::load(project_path("simulation.toml"))
            .expect("load Lesson 84 identification plant"),
    );
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish identification initial state");
    manual_scan(&mut runtime, 50.0, 50.0, 1.0);
    (runtime, plant)
}

#[test]
fn control_curriculum_independent_capstone_identifies_and_validates_receiver_model() {
    let (mut lag_runtime, mut lag_plant) = initialized_manual_plant();
    let mut delivery_at_five_seconds = f64::NAN;
    for second in 0..10 {
        manual_scan(&mut lag_runtime, 60.0, 50.0, 1.0);
        let now = Duration::from_millis(((second + 1) * 1000) as i64);
        lag_plant
            .apply_post_cycle(now, &lag_runtime)
            .expect("advance delivery-pulse identification");
        lag_plant
            .apply_pre_cycle(now, &mut lag_runtime)
            .expect("publish delivery-pulse response");
        if second == 4 {
            delivery_at_five_seconds = read_lreal(&lag_runtime, "%IL8");
        }
    }
    let identified_lag = -5.0 / ((60.0 - delivery_at_five_seconds) / 10.0).ln();
    assert!(
        (identified_lag - 5.0).abs() <= 0.15,
        "identified lag={identified_lag}"
    );

    let (mut rate_runtime, mut rate_plant) = initialized_manual_plant();
    let initial_pressure = read_lreal(&rate_runtime, "%IL0");
    manual_scan(&mut rate_runtime, 50.0, 55.0, 1.0);
    rate_plant
        .apply_post_cycle(Duration::from_millis(1000), &rate_runtime)
        .expect("advance receiver-rate identification");
    rate_plant
        .apply_pre_cycle(Duration::from_millis(1000), &mut rate_runtime)
        .expect("publish receiver-rate response");
    let identified_rate = (read_lreal(&rate_runtime, "%IL0") - initial_pressure) / -5.0;
    assert!((identified_rate - 0.005).abs() <= 0.0001);

    let (mut validation_runtime, mut validation_plant) = initialized_manual_plant();
    for second in 0..10 {
        manual_scan(&mut validation_runtime, 55.0, 52.0, 1.0);
        let now = Duration::from_millis(((second + 1) * 1000) as i64);
        validation_plant
            .apply_post_cycle(now, &validation_runtime)
            .expect("advance unused validation sequence");
        validation_plant
            .apply_pre_cycle(now, &mut validation_runtime)
            .expect("publish unused validation response");
    }
    let expected_delivery = 55.0 + (50.0 - 55.0) * (-10.0_f64 / identified_lag).exp();
    let expected_pressure = 7.0
        + identified_rate
            * ((55.0 - 52.0) * 10.0
                + (50.0 - 55.0) * identified_lag * (1.0 - (-10.0_f64 / identified_lag).exp()));
    let measured_delivery = read_lreal(&validation_runtime, "%IL8");
    let measured_pressure = read_lreal(&validation_runtime, "%IL0");
    assert!((measured_delivery - expected_delivery).abs() <= 0.03);
    assert!((measured_pressure - expected_pressure).abs() <= 0.003);

    eprintln!(
        "compressed-air identification: Tc={identified_lag:.3}s, Kr={identified_rate:.6}bar/(%*s), unused validation delivery={measured_delivery:.3}/{expected_delivery:.3}%, pressure={measured_pressure:.4}/{expected_pressure:.4}bar"
    );
}

#[test]
fn control_curriculum_independent_capstone_defends_selected_architecture() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 84 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 84);
    assert_eq!(lesson["prerequisites"][0], "control-capstone-building-hvac");
    assert!(lesson["nextLesson"].is_null());
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_independent_capstone_defends_selected_architecture"
    );
    let source = project_file("src/Main.st");
    for forbidden in [
        "PressureIae",
        "AcceptedCandidate",
        "SelectedWinner",
        "PrecomputedTrace",
    ] {
        assert!(
            !source.contains(forbidden),
            "ST embeds forbidden shortcut {forbidden}"
        );
    }

    let roots = cubic_roots(0.2, 0.04, 0.001);
    assert!(
        roots.iter().all(|root| root.re < -0.02),
        "unstable reference poles: {roots:?}"
    );
    let frequency = frequency_evidence(40.0, 1.0, 0.005, 5.0);
    assert!((frequency.crossover - 0.1586).abs() <= 0.002);
    assert!(frequency.phase_margin >= 40.0);
    assert!(frequency.delay_margin >= 4.5);
    assert!(frequency.sensitivity_peak <= 1.7);

    let nominal = run("simulation.toml", 0.0, false, false);
    let uncertainty = run("simulation-uncertainty.toml", 5.0, true, true);
    for metrics in [&nominal, &uncertainty] {
        assert_eq!(metrics.normal_violation[1], 0.0);
        assert!(metrics.maximum_departure[1] <= 0.5);
        let improvement = 100.0 * (metrics.iae[0] - metrics.iae[1]) / metrics.iae[0];
        let movement_increase = 100.0 * (metrics.applied_movement[1] - metrics.applied_movement[0])
            / metrics.applied_movement[0];
        assert!(
            improvement >= 30.0,
            "feedforward IAE improvement {improvement:.1}%"
        );
        assert!(
            movement_increase <= 25.0,
            "movement increase {movement_increase:.1}%"
        );
    }
    verify_abnormal_authority();

    let architecture = [0.0, 1.0];
    let frequency_point = [0.0];
    let crossover = [frequency.crossover];
    let phase_margin = [frequency.phase_margin];
    let delay_margin = [frequency.delay_margin];
    let sensitivity_peak = [frequency.sensitivity_peak];
    let mut summary = String::from("plant,architecture,iae_bar_s,maximum_departure_bar,applied_movement_pp,normal_violation_bar_s,verdict\n");
    for (plant, metrics) in [("nominal", &nominal), ("uncertainty", &uncertainty)] {
        writeln!(
            summary,
            "{plant},feedback,{:.9},{:.9},{:.9},{:.9},baseline",
            metrics.iae[0],
            metrics.maximum_departure[0],
            metrics.applied_movement[0],
            metrics.normal_violation[0]
        )
        .unwrap();
        writeln!(
            summary,
            "{plant},PI-plus-feedforward,{:.9},{:.9},{:.9},{:.9},selected-unapplied",
            metrics.iae[1],
            metrics.maximum_departure[1],
            metrics.applied_movement[1],
            metrics.normal_violation[1]
        )
        .unwrap();
    }
    evidence::retain(
        &project_path("figures"),
        "independent-capstone-evidence",
        "Qualified demand feedforward improves the compressed-air receiver under uncertainty",
        "Index 0 is feedback only and index 1 is PI plus qualified demand feedforward; the same decision is tested on nominal and uncertainty plants.",
        &[
            Panel {
                title: "Pressure integrated absolute error",
                x_unit: "architecture index",
                y_unit: "bar*s",
                x_minimum: -0.05,
                x_maximum: 1.05,
                y_minimum: 0.0,
                y_maximum: 32.0,
                series: vec![
                    Series { name: "Nominal plant", color: "#2563eb", dashed: false, x: &architecture, y: &nominal.iae },
                    Series { name: "Uncertainty plant", color: "#dc2626", dashed: true, x: &architecture, y: &uncertainty.iae },
                ],
            },
            Panel {
                title: "Applied-compressor movement",
                x_unit: "architecture index",
                y_unit: "percentage points",
                x_minimum: -0.05,
                x_maximum: 1.05,
                y_minimum: 0.0,
                y_maximum: 180.0,
                series: vec![
                    Series { name: "Nominal plant", color: "#2563eb", dashed: false, x: &architecture, y: &nominal.applied_movement },
                    Series { name: "Uncertainty plant", color: "#dc2626", dashed: true, x: &architecture, y: &uncertainty.applied_movement },
                ],
            },
        ],
        &[
            RetainedSeries { panel: "Frequency qualification", name: "Crossover", x: &frequency_point, y: &crossover, x_unit: "qualified design", y_unit: "rad/s" },
            RetainedSeries { panel: "Frequency qualification", name: "Phase margin", x: &frequency_point, y: &phase_margin, x_unit: "qualified design", y_unit: "deg" },
            RetainedSeries { panel: "Frequency qualification", name: "Delay margin", x: &frequency_point, y: &delay_margin, x_unit: "qualified design", y_unit: "s" },
            RetainedSeries { panel: "Frequency qualification", name: "Maximum sensitivity", x: &frequency_point, y: &sensitivity_peak, x_unit: "qualified design", y_unit: "ratio" },
        ],
        &summary,
    );

    eprintln!(
        "independent compressed-air capstone: poles={roots:?}, frequency={frequency:?}, nominal={nominal:?}, uncertainty={uncertainty:?}"
    );
}
