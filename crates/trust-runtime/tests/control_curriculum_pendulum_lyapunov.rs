use std::f64::consts::PI;
use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const CERTIFICATE_LEVEL: f64 = 0.025;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join(
            "../../examples/control/12-nonlinear-adaptive-and-robust-control/69-pendulum-lyapunov",
        )
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 69 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 69 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 69 program before configuration")
        .replace(
            "AngleSetpoint : LREAL :=",
            "AngleSetpoint AT %ML0 : LREAL :=",
        )
        .replace("AngleError : LREAL :=", "AngleError AT %ML8 : LREAL :=")
        .replace(
            "TorqueRequested : LREAL :=",
            "TorqueRequested AT %ML16 : LREAL :=",
        )
        .replace(
            "LyapunovValue : LREAL :=",
            "LyapunovValue AT %ML24 : LREAL :=",
        )
        .replace(
            "LyapunovDerivative : LREAL :=",
            "LyapunovDerivative AT %ML32 : LREAL :=",
        )
        .replace(
            "PreviousLyapunovValue : LREAL :=",
            "PreviousLyapunovValue AT %ML40 : LREAL :=",
        )
        .replace(
            "LyapunovDelta : LREAL :=",
            "LyapunovDelta AT %ML48 : LREAL :=",
        )
        .replace(
            "CertificateLevel : LREAL :=",
            "CertificateLevel AT %ML56 : LREAL :=",
        )
        .replace("Enabled : BOOL :=", "Enabled AT %MX64.0 : BOOL :=")
        .replace(
            "TorqueSaturated : BOOL :=",
            "TorqueSaturated AT %MX64.1 : BOOL :=",
        )
        .replace(
            "EvidenceInitialized : BOOL :=",
            "EvidenceInitialized AT %MX64.2 : BOOL :=",
        )
        .replace(
            "InsideCertifiedSet : BOOL :=",
            "InsideCertifiedSet AT %MX64.3 : BOOL :=",
        )
        .replace(
            "CertificateValid : BOOL :=",
            "CertificateValid AT %MX64.4 : BOOL :=",
        )
        .replace(
            "TorqueApplied : LREAL :=",
            "TorqueApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "AbsoluteAngle : LREAL :=",
            "AbsoluteAngle AT %IL0 : LREAL :=",
        )
        .replace("AngularRate : LREAL :=", "AngularRate AT %IL8 : LREAL :=")
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![SourceFile::with_path(
        "pendulum-lyapunov/src/Main.st",
        mapped_source(),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 69 Structured Text")
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-pendulum-{label}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn simulation(alpha: f64, omega: f64, label: &str) -> SimulationController {
    let source = project_file("simulation.toml");
    let configured = source.replace(
        "initial_state = [3.241592653589793,0]",
        &format!("initial_state = [{:.15},{omega:.15}]", PI + alpha),
    );
    assert_ne!(
        configured, source,
        "Lesson 69 initial-state fixture must be replaceable"
    );
    let path = temporary_config_path(label);
    std::fs::write(&path, configured).expect("write Lesson 69 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 69 simulation config");
    let _ = std::fs::remove_file(path);
    SimulationController::new(config)
}

fn write_bool(runtime: &mut Runtime, address: &str, value: bool) {
    runtime
        .io_mut()
        .write(&IoAddress::parse(address).unwrap(), Value::Bool(value))
        .unwrap();
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

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .expect("read BOOL lesson signal")
    {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .expect("read LREAL lesson signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

fn lyapunov(alpha: f64, omega: f64) -> f64 {
    0.5 * ((4.0 / 3.0) * alpha * alpha + (1.0 / 3.0) * alpha * omega + (1.0 / 3.0) * omega * omega)
}

fn lyapunov_derivative(alpha: f64, omega: f64) -> f64 {
    -0.5 * (alpha * alpha + omega * omega) + (alpha / 6.0 + omega / 3.0) * (alpha.sin() - alpha)
}

fn derivative(state: [f64; 2], torque: f64) -> [f64; 2] {
    [state[1], torque - state[0].sin()]
}

fn rk4_step(mut state: [f64; 2], torque: f64, step: f64, substeps: usize) -> [f64; 2] {
    let h = step / substeps as f64;
    for _ in 0..substeps {
        let k1 = derivative(state, torque);
        let k2 = derivative(
            [state[0] + 0.5 * h * k1[0], state[1] + 0.5 * h * k1[1]],
            torque,
        );
        let k3 = derivative(
            [state[0] + 0.5 * h * k2[0], state[1] + 0.5 * h * k2[1]],
            torque,
        );
        let k4 = derivative([state[0] + h * k3[0], state[1] + h * k3[1]], torque);
        state[0] += h * (k1[0] + 2.0 * k2[0] + 2.0 * k3[0] + k4[0]) / 6.0;
        state[1] += h * (k1[1] + 2.0 * k2[1] + 2.0 * k3[1] + k4[1]) / 6.0;
    }
    state
}

#[derive(Debug, Default)]
struct TrajectoryMetrics {
    initial_v: f64,
    maximum_v: f64,
    maximum_v_dot: f64,
    maximum_delta_v: f64,
    peak_requested_torque: f64,
    peak_applied_torque: f64,
    saturation_seconds: f64,
    sublevel_exits: usize,
    certificate_violations: usize,
    final_alpha: f64,
    final_omega: f64,
    settling_time: Option<f64>,
    initial_inside: bool,
    entered_certified_set: bool,
}

fn run_trajectory(alpha0: f64, omega0: f64, label: &str) -> TrajectoryMetrics {
    let mut runtime = build_runtime();
    let mut plant = simulation(alpha0, omega0, label);
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial pendulum state");
    write_lreal(&mut runtime, "%ML0", PI);
    write_lreal(&mut runtime, "%ML56", CERTIFICATE_LEVEL);
    write_bool(&mut runtime, "%MX64.0", true);

    let mut metrics = TrajectoryMetrics {
        maximum_v_dot: f64::NEG_INFINITY,
        maximum_delta_v: f64::NEG_INFINITY,
        ..TrajectoryMetrics::default()
    };
    let mut last_unsettled_sample = None;
    for sample in 0..=80 {
        runtime
            .execute_cycle()
            .expect("execute pendulum controller sample");
        let angle = read_lreal(&runtime, "%IL0");
        let alpha = angle - PI;
        let omega = read_lreal(&runtime, "%IL8");
        let requested = read_lreal(&runtime, "%ML16");
        let applied = read_lreal(&runtime, "%QL0");
        let value = read_lreal(&runtime, "%ML24");
        let derivative = read_lreal(&runtime, "%ML32");
        let delta = read_lreal(&runtime, "%ML48");
        let inside = read_bool(&runtime, "%MX64.3");
        let saturated = read_bool(&runtime, "%MX64.1");
        let certificate_valid = read_bool(&runtime, "%MX64.4");

        let published_error = read_lreal(&runtime, "%ML8");
        assert!(
            (published_error - alpha).abs() <= 1e-12,
            "{label} sample {sample}: angle={angle}, setpoint={}, expected alpha={alpha}, published={published_error}",
            read_lreal(&runtime, "%ML0")
        );
        assert!((requested - (-4.0 * alpha - 2.0 * omega)).abs() <= 1e-12);
        assert!((applied - requested.clamp(-2.0, 2.0)).abs() <= 1e-12);
        assert!((value - lyapunov(alpha, omega)).abs() <= 1e-12);
        assert!((derivative - lyapunov_derivative(alpha, omega)).abs() <= 1e-12);
        assert_eq!(inside, value <= CERTIFICATE_LEVEL + 1e-12);
        assert_eq!(saturated, (requested - applied).abs() > 1e-12);

        if sample == 0 {
            metrics.initial_v = value;
            metrics.initial_inside = inside;
        }
        metrics.maximum_v = metrics.maximum_v.max(value);
        metrics.maximum_v_dot = metrics.maximum_v_dot.max(derivative);
        metrics.maximum_delta_v = metrics.maximum_delta_v.max(delta);
        metrics.peak_requested_torque = metrics.peak_requested_torque.max(requested.abs());
        metrics.peak_applied_torque = metrics.peak_applied_torque.max(applied.abs());
        metrics.entered_certified_set |= inside;
        if saturated {
            metrics.saturation_seconds += 0.1;
        }
        if metrics.initial_inside && !inside {
            metrics.sublevel_exits += 1;
        }
        if inside && value > 1e-12 {
            let sample_valid = derivative < 0.0 && !saturated && (sample == 0 || delta <= 1e-12);
            if !sample_valid || !certificate_valid {
                metrics.certificate_violations += 1;
            }
        }

        metrics.final_alpha = alpha;
        metrics.final_omega = omega;
        if alpha.abs() > 0.01 || omega.abs() > 0.02 {
            last_unsettled_sample = Some(sample);
        }
        if sample < 80 {
            let now = Duration::from_millis((sample as i64 + 1) * 100);
            plant
                .apply_post_cycle(now, &runtime)
                .expect("advance nonlinear pendulum");
            plant
                .apply_pre_cycle(now, &mut runtime)
                .expect("publish nonlinear pendulum");
        }
    }
    metrics.settling_time = match last_unsettled_sample {
        Some(sample) if 80_usize.saturating_sub(sample) >= 5 => Some((sample + 1) as f64 * 0.1),
        None => Some(0.0),
        _ => None,
    };
    metrics
}

#[test]
fn control_curriculum_pendulum_lyapunov_certifies_only_declared_sublevel_set() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 69 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 69 must use schema 4");
    assert_eq!(lesson["sequence"], 69);
    assert_eq!(lesson["prerequisites"][0], "control-building-hvac-mpc");
    assert_eq!(lesson["nextLesson"], "control-robot-joint-nonlinear");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_pendulum_lyapunov_certifies_only_declared_sublevel_set"
    );

    let predicted_v = lyapunov(0.1, 0.0);
    let predicted_v_dot = lyapunov_derivative(0.1, 0.0);
    assert!((predicted_v - 0.006_666_666_666_666_667).abs() <= 1e-15);
    assert!((predicted_v_dot - -0.005_002_776_389_219_532).abs() <= 1e-15);
    assert!((lyapunov(0.25, 0.0) - 0.041_666_666_666_666_664).abs() <= 1e-15);

    // The sublevel implication is analytic: minimizing V on a fixed angle or
    // rate gives V >= 5/8*alpha^2 and V >= 5/32*omega^2 respectively.
    assert!(((5.0 / 8.0_f64) * 0.2_f64.powi(2) - CERTIFICATE_LEVEL).abs() <= 1e-15);
    assert!(((5.0 / 32.0_f64) * 0.4_f64.powi(2) - CERTIFICATE_LEVEL).abs() <= 1e-15);
    // Maximizing |[-4,-2]x| on x'Px<=2*rho gives sqrt(0.96), below 2 N*m.
    let maximum_certified_torque = (2.0 * CERTIFICATE_LEVEL * 19.2_f64).sqrt();
    assert!(maximum_certified_torque < 2.0);
    // |sin(a)-a| <= |a|^3/6 and |a|<=0.2 make the nonlinear
    // remainder too small to overturn either negative quadratic coefficient.
    assert!(0.5 - 1.0 / 450.0 > 0.49);
    assert!(0.5 - 1.0 / 900.0 > 0.49);

    // Match one held-torque plant interval to an independent ten-substep RK4 oracle.
    let mut runtime = build_runtime();
    let mut plant = simulation(0.1, 0.0, "rk4-oracle");
    plant.apply_pre_cycle(Duration::ZERO, &mut runtime).unwrap();
    write_lreal(&mut runtime, "%ML0", PI);
    write_lreal(&mut runtime, "%ML56", CERTIFICATE_LEVEL);
    write_bool(&mut runtime, "%MX64.0", true);
    runtime
        .execute_cycle()
        .expect("execute reference controller sample");
    let torque = read_lreal(&runtime, "%QL0");
    let expected = rk4_step([PI + 0.1, 0.0], torque, 0.1, 10);
    plant
        .apply_post_cycle(Duration::from_millis(100), &runtime)
        .expect("advance reference plant interval");
    plant
        .apply_pre_cycle(Duration::from_millis(100), &mut runtime)
        .expect("publish reference plant state");
    assert!((read_lreal(&runtime, "%IL0") - expected[0]).abs() <= 1e-12);
    assert!((read_lreal(&runtime, "%IL8") - expected[1]).abs() <= 1e-12);

    let certified_cases = [
        (0.05, 0.0, "p005"),
        (-0.05, 0.0, "m005"),
        (0.10, 0.0, "p010"),
        (-0.10, 0.0, "m010"),
        (0.15, -0.15, "p015-m015"),
        (-0.15, 0.15, "m015-p015"),
        (0.0, 0.35, "p035-rate"),
        (0.0, -0.35, "m035-rate"),
    ];
    let mut artifact_cases = Vec::new();
    for (alpha, omega, label) in certified_cases {
        let metrics = run_trajectory(alpha, omega, label);
        assert!(
            metrics.initial_inside,
            "{label} must start in the certified set"
        );
        assert_eq!(
            metrics.sublevel_exits, 0,
            "{label} escaped its invariant sublevel set"
        );
        assert_eq!(
            metrics.certificate_violations, 0,
            "{label} violated the certificate"
        );
        assert_eq!(
            metrics.saturation_seconds, 0.0,
            "{label} saturated the torque actuator"
        );
        assert!(
            metrics.final_alpha.abs() <= 0.01,
            "{label} final alpha={}",
            metrics.final_alpha
        );
        assert!(
            metrics.final_omega.abs() <= 0.02,
            "{label} final omega={}",
            metrics.final_omega
        );
        assert!(
            metrics.settling_time.is_some(),
            "{label} did not settle for the final 0.5 s"
        );
        eprintln!("certified {label}: V0={:.6}, max Vdot={:.6}, max DeltaV={:.6}, peak torque={:.3} N*m, settling={:.1}s, final=({:.5},{:.5})", metrics.initial_v, metrics.maximum_v_dot, metrics.maximum_delta_v, metrics.peak_applied_torque, metrics.settling_time.unwrap(), metrics.final_alpha, metrics.final_omega);
        artifact_cases.push((label, metrics));
    }

    for (alpha, label) in [(0.25, "outside-positive"), (-0.25, "outside-negative")] {
        let metrics = run_trajectory(alpha, 0.0, label);
        assert!(
            !metrics.initial_inside,
            "{label} must remain explicitly uncertified initially"
        );
        eprintln!(
            "observational {label}: V0={:.6}, entered certificate={}, final=({:.5},{:.5})",
            metrics.initial_v,
            metrics.entered_certified_set,
            metrics.final_alpha,
            metrics.final_omega
        );
        artifact_cases.push((label, metrics));
    }

    let saturated = run_trajectory(0.8, 0.0, "saturated-outside");
    assert!(!saturated.initial_inside);
    assert!(saturated.saturation_seconds > 0.0);
    eprintln!(
        "saturated outside: V0={:.6}, saturation={:.1}s, peak requested/applied={:.3}/{:.3} N*m",
        saturated.initial_v,
        saturated.saturation_seconds,
        saturated.peak_requested_torque,
        saturated.peak_applied_torque
    );
    artifact_cases.push(("saturated-outside", saturated));

    let mut disabled = build_runtime();
    let mut disabled_plant = simulation(0.1, 0.0, "disabled");
    disabled_plant
        .apply_pre_cycle(Duration::ZERO, &mut disabled)
        .unwrap();
    write_lreal(&mut disabled, "%ML0", PI);
    write_lreal(&mut disabled, "%ML56", CERTIFICATE_LEVEL);
    write_bool(&mut disabled, "%MX64.0", false);
    disabled
        .execute_cycle()
        .expect("execute disabled pendulum controller");
    assert_eq!(read_lreal(&disabled, "%QL0"), 0.0);

    let case_number: Vec<f64> = (1..=artifact_cases.len())
        .map(|value| value as f64)
        .collect();
    let initial_v: Vec<f64> = artifact_cases
        .iter()
        .map(|(_, metrics)| metrics.initial_v)
        .collect();
    let certificate = vec![CERTIFICATE_LEVEL; artifact_cases.len()];
    let peak_torque: Vec<f64> = artifact_cases
        .iter()
        .map(|(_, metrics)| metrics.peak_applied_torque)
        .collect();
    let mut summary = String::from(
        "case,initial_v,inside_certificate,settling_s,peak_applied_torque_nm,saturation_s\n",
    );
    for (label, metrics) in &artifact_cases {
        writeln!(
            summary,
            "{label},{:.9},{},{},{:.9},{:.3}",
            metrics.initial_v,
            metrics.initial_inside,
            metrics
                .settling_time
                .map_or_else(|| "not_settled".to_string(), |value| format!("{value:.3}")),
            metrics.peak_applied_torque,
            metrics.saturation_seconds,
        )
        .unwrap();
    }
    let figure_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/12-nonlinear-adaptive-and-robust-control/69-pendulum-lyapunov/figures");
    evidence::retain(
        &figure_root,
        "pendulum-lyapunov-evidence",
        "Certified region is smaller than simulated recovery",
        "Cases 1-8 start inside the certificate; 9-11 are observations only.",
        &[
            evidence::Panel {
                title: "Initial Lyapunov value",
                x_unit: "case",
                y_unit: "V",
                x_minimum: 1.0,
                x_maximum: artifact_cases.len() as f64,
                y_minimum: 0.0,
                y_maximum: initial_v.iter().copied().fold(CERTIFICATE_LEVEL, f64::max) * 1.1,
                series: vec![
                    evidence::Series {
                        name: "Initial V",
                        color: "#2563eb",
                        dashed: false,
                        x: &case_number,
                        y: &initial_v,
                    },
                    evidence::Series {
                        name: "Certificate rho",
                        color: "#dc2626",
                        dashed: true,
                        x: &case_number,
                        y: &certificate,
                    },
                ],
            },
            evidence::Panel {
                title: "Peak applied torque",
                x_unit: "case",
                y_unit: "N m",
                x_minimum: 1.0,
                x_maximum: artifact_cases.len() as f64,
                y_minimum: 0.0,
                y_maximum: 2.2,
                series: vec![evidence::Series {
                    name: "Applied torque",
                    color: "#16a34a",
                    dashed: false,
                    x: &case_number,
                    y: &peak_torque,
                }],
            },
        ],
        &[],
        &summary,
    );
}
