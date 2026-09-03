use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");

fn st_executable_text(source: &str) -> String {
    let mut without_block_comments = String::with_capacity(source.len());
    let mut remainder = source;
    while let Some(start) = remainder.find("(*") {
        without_block_comments.push_str(&remainder[..start]);
        let comment = &remainder[start + 2..];
        let end = comment.find("*)").expect("closed ST block comment");
        remainder = &comment[end + 2..];
    }
    without_block_comments.push_str(remainder);
    without_block_comments
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect()
}

fn evidence_root(project: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/07-classical-stability-and-frequency-design")
        .join(classical_project(project))
        .join("figures")
}

fn classical_project(project: &str) -> &str {
    match project {
        "crane-routh-stability" => "45-crane-routh-stability",
        "servo-root-locus" => "46-servo-root-locus",
        "flexible-drive-bode" => "47-flexible-drive-bode",
        "delayed-thermal-nyquist" => "48-delayed-thermal-nyquist",
        "motor-sensitivity" => "49-motor-sensitivity",
        "position-z-plane" => "50-position-z-plane",
        _ => panic!("unknown classical-control lesson: {project}"),
    }
}

fn time_axis(samples: usize, sample_seconds: f64) -> Vec<f64> {
    (1..=samples)
        .map(|sample| sample as f64 * sample_seconds)
        .collect()
}

const PROJECTS: [(&str, &str, &str); 3] = [
    (
        "motor-sensitivity",
        include_str!("../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/49-motor-sensitivity/src/Main.st"),
        include_str!("../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/49-motor-sensitivity/simulation.toml"),
    ),
    (
        "crane-routh-stability",
        include_str!("../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/45-crane-routh-stability/src/Main.st"),
        include_str!("../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/45-crane-routh-stability/simulation.toml"),
    ),
    (
        "servo-root-locus",
        include_str!("../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/46-servo-root-locus/src/Main.st"),
        include_str!("../../../editors/vscode/media/examples/control/07-classical-stability-and-frequency-design/46-servo-root-locus/simulation.toml"),
    ),
];

fn temporary_config_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-classical-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL") {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL");
}

fn source_project_file(project: &str, relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/07-classical-stability-and-frequency-design")
        .join(classical_project(project))
        .join(relative);
    assert!(
        path.is_file(),
        "required curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read curriculum file {}: {error}", path.display()))
}

#[derive(Clone, Copy, Debug)]
struct ComplexValue {
    re: f64,
    im: f64,
}

impl ComplexValue {
    fn add(self, other: Self) -> Self {
        Self {
            re: self.re + other.re,
            im: self.im + other.im,
        }
    }

    fn scale(self, factor: f64) -> Self {
        Self {
            re: self.re * factor,
            im: self.im * factor,
        }
    }

    fn magnitude(self) -> f64 {
        self.re.hypot(self.im)
    }

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

fn crane_roots(gain: f64) -> [ComplexValue; 3] {
    cubic_roots(2.0, 3.0, gain)
}

fn peak_absolute(values: &[f64]) -> f64 {
    values.iter().map(|value| value.abs()).fold(0.0, f64::max)
}

fn fitted_sinusoid(
    samples: &[f64],
    start_index: usize,
    step_seconds: f64,
    frequency: f64,
) -> (f64, f64, f64) {
    let selected = &samples[start_index..];
    let count = selected.len() as f64;
    let mut sum_y = 0.0;
    let mut sum_sine = 0.0;
    let mut sum_cosine = 0.0;
    for (offset, value) in selected.iter().enumerate() {
        let time = (start_index + offset + 1) as f64 * step_seconds;
        sum_y += value;
        sum_sine += (frequency * time).sin();
        sum_cosine += (frequency * time).cos();
    }
    let mean_y = sum_y / count;
    let mean_sine = sum_sine / count;
    let mean_cosine = sum_cosine / count;
    let mut sine_square = 0.0;
    let mut cosine_square = 0.0;
    let mut sine_cosine = 0.0;
    let mut output_sine = 0.0;
    let mut output_cosine = 0.0;
    for (offset, value) in selected.iter().enumerate() {
        let time = (start_index + offset + 1) as f64 * step_seconds;
        let sine = (frequency * time).sin() - mean_sine;
        let cosine = (frequency * time).cos() - mean_cosine;
        let output = value - mean_y;
        sine_square += sine * sine;
        cosine_square += cosine * cosine;
        sine_cosine += sine * cosine;
        output_sine += output * sine;
        output_cosine += output * cosine;
    }
    let determinant = sine_square * cosine_square - sine_cosine * sine_cosine;
    assert!(determinant.abs() > 1.0e-12, "sinusoidal fit is singular");
    let sine_coefficient =
        (output_sine * cosine_square - output_cosine * sine_cosine) / determinant;
    let cosine_coefficient =
        (output_cosine * sine_square - output_sine * sine_cosine) / determinant;
    let mut residual_square = 0.0;
    let mut output_square = 0.0;
    for (offset, value) in selected.iter().enumerate() {
        let time = (start_index + offset + 1) as f64 * step_seconds;
        let sine = (frequency * time).sin() - mean_sine;
        let cosine = (frequency * time).cos() - mean_cosine;
        let predicted = mean_y + sine_coefficient * sine + cosine_coefficient * cosine;
        residual_square += (value - predicted).powi(2);
        output_square += (value - mean_y).powi(2);
    }
    (
        sine_coefficient.hypot(cosine_coefficient),
        cosine_coefficient.atan2(sine_coefficient),
        (1.0 - residual_square / output_square).clamp(0.0, 1.0),
    )
}

fn wrapped_degrees(radians: f64) -> f64 {
    let mut degrees = radians.to_degrees();
    while degrees > 180.0 {
        degrees -= 360.0;
    }
    while degrees <= -180.0 {
        degrees += 360.0;
    }
    degrees
}

fn second_order_gain_crossover(damping_coefficient: f64) -> (f64, f64) {
    // |5/(s^2 + a*s + 4)| = 1. Solving in x = omega^2 avoids
    // relying on the implementation that Control Studio uses for its sweep.
    let linear = damping_coefficient * damping_coefficient - 8.0;
    let frequency_squared = (-linear + (linear * linear + 36.0).sqrt()) / 2.0;
    let frequency = frequency_squared.sqrt();
    let phase = -f64::atan2(damping_coefficient * frequency, 4.0 - frequency_squared);
    (frequency, 180.0 + phase.to_degrees())
}

#[test]
fn control_curriculum_routh_range_predicts_crane_stable_marginal_and_unstable_runs() {
    let shipped_source = source_project_file("crane-routh-stability", "src/Main.st");
    let simulation_source = source_project_file("crane-routh-stability", "simulation.toml");
    assert_eq!(
        simulation_source.matches("[[plants]]").count(),
        3,
        "Lesson 45 requires equivalent K=4, K=6, and K=8 crane copies"
    );
    for required in [
        "StableGain : LREAL := 4.0",
        "MarginalGain : LREAL := 6.0",
        "UnstableGain : LREAL := 8.0",
    ] {
        assert!(
            shipped_source.contains(required),
            "missing executable feedback case: {required}"
        );
    }

    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("crane program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX96.0 : BOOL :=",
        )
        .replace("LowRequested : LREAL :=", "LowRequested AT %ML0 : LREAL :=")
        .replace(
            "ResonantRequested : LREAL :=",
            "ResonantRequested AT %ML8 : LREAL :=",
        )
        .replace(
            "HighRequested : LREAL :=",
            "HighRequested AT %ML16 : LREAL :=",
        )
        .replace(
            "ProportionalRequested : LREAL :=",
            "ProportionalRequested AT %ML0 : LREAL :=",
        )
        .replace(
            "LeadRequested : LREAL :=",
            "LeadRequested AT %ML8 : LREAL :=",
        )
        .replace(
            "StableAborted : BOOL :=",
            "StableAborted AT %MX97.0 : BOOL :=",
        )
        .replace(
            "MarginalAborted : BOOL :=",
            "MarginalAborted AT %MX97.1 : BOOL :=",
        )
        .replace(
            "UnstableAborted : BOOL :=",
            "UnstableAborted AT %MX97.2 : BOOL :=",
        )
        .replace(
            "StableRequested : LREAL :=",
            "StableRequested AT %ML0 : LREAL :=",
        )
        .replace(
            "MarginalRequested : LREAL :=",
            "MarginalRequested AT %ML8 : LREAL :=",
        )
        .replace(
            "UnstableRequested : LREAL :=",
            "UnstableRequested AT %ML16 : LREAL :=",
        )
        .replace(
            "StableApplied : LREAL :=",
            "StableApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "MarginalApplied : LREAL :=",
            "MarginalApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "UnstableApplied : LREAL :=",
            "UnstableApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "StablePosition : LREAL :=",
            "StablePosition AT %IL0 : LREAL :=",
        )
        .replace(
            "MarginalPosition : LREAL :=",
            "MarginalPosition AT %IL8 : LREAL :=",
        )
        .replace(
            "UnstablePosition : LREAL :=",
            "UnstablePosition AT %IL16 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "crane-routh-stability/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile three explicit crane feedback cases");
    let path = temporary_config_path("crane-routh-three-cases");
    std::fs::write(&path, simulation_source).expect("write crane simulation configuration");
    let mut plants = SimulationController::new(
        SimulationConfig::load(&path).expect("load three equivalent crane plants"),
    );
    let enabled = IoAddress::parse("%MX96.0").unwrap();
    let positions = ["%IL0", "%IL8", "%IL16"].map(|address| IoAddress::parse(address).unwrap());
    let requested_outputs =
        ["%ML0", "%ML8", "%ML16"].map(|address| IoAddress::parse(address).unwrap());
    let outputs = ["%QL0", "%QL8", "%QL16"].map(|address| IoAddress::parse(address).unwrap());
    let aborts =
        ["%MX97.0", "%MX97.1", "%MX97.2"].map(|address| IoAddress::parse(address).unwrap());
    plants
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &enabled, true);
    let mut traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(4_000));
    let mut requests: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(4_000));
    let mut commands: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(4_000));
    for cycle in 1..=4_000 {
        runtime
            .execute_cycle()
            .expect("execute crane feedback cases");
        let now = Duration::from_millis((cycle * 10) as i64);
        plants
            .apply_post_cycle(now, &runtime)
            .expect("advance crane plants");
        plants
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish crane positions");
        for index in 0..3 {
            traces[index].push(read_lreal(&runtime, &positions[index]));
            requests[index].push(read_lreal(&runtime, &requested_outputs[index]));
            commands[index].push(read_lreal(&runtime, &outputs[index]));
        }
    }

    let roots = [4.0, 6.0, 8.0].map(crane_roots);
    assert_eq!(roots[0].iter().filter(|root| root.re > 1.0e-8).count(), 0);
    assert!(roots[0].iter().all(|root| root.re < -1.0e-8));
    assert_eq!(
        roots[1]
            .iter()
            .filter(|root| root.re.abs() <= 1.0e-8)
            .count(),
        2
    );
    assert_eq!(roots[2].iter().filter(|root| root.re > 1.0e-8).count(), 2);

    let early_envelopes = traces
        .each_ref()
        .map(|trace| peak_absolute(&trace[200..1_200]));
    let late_envelopes = traces
        .each_ref()
        .map(|trace| peak_absolute(&trace[3_000..4_000]));
    let envelope_ratios =
        std::array::from_fn::<_, 3, _>(|index| late_envelopes[index] / early_envelopes[index]);
    assert!(
        envelope_ratios[0] < 0.25,
        "stable envelope ratio={}",
        envelope_ratios[0]
    );
    assert!(
        (0.75..=1.25).contains(&envelope_ratios[1]),
        "marginal envelope ratio={}",
        envelope_ratios[1]
    );
    assert!(
        read_bool(&runtime, &aborts[2]) || envelope_ratios[2] > 2.0,
        "unstable case neither grew nor aborted: ratio={}",
        envelope_ratios[2]
    );
    assert!(
        !read_bool(&runtime, &aborts[0]),
        "stable case must not abort"
    );
    assert!(
        !read_bool(&runtime, &aborts[1]),
        "marginal case must remain within its local range"
    );
    assert!(commands
        .iter()
        .flatten()
        .all(|value| value.is_finite() && (-100.0..=100.0).contains(value)));
    for forbidden in ["Routh", "RightHalfPlane", "StableLabel", "EnvelopeRatio"] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds analysis result '{forbidden}'"
        );
    }
    let time = time_axis(traces[0].len(), 0.01);
    let root_x = roots.map(|set| set.map(|root| root.re));
    let root_y = roots.map(|set| set.map(|root| root.im));
    let peak_positions = traces.each_ref().map(|trace| peak_absolute(trace));
    let peak_requests = requests.each_ref().map(|trace| peak_absolute(trace));
    let peak_commands = commands.each_ref().map(|trace| peak_absolute(trace));
    let force_travel = commands.each_ref().map(|trace| total_variation(trace));
    let abort_values = aborts
        .each_ref()
        .map(|address| read_bool(&runtime, address));
    write_bool(&mut runtime, &enabled, false);
    runtime.execute_cycle().expect("restore crane outputs");
    assert!(outputs
        .iter()
        .all(|address| read_lreal(&runtime, address).abs() < 1.0e-12));
    let summary = format!(
        "case,gain,polynomial,poles,right_half_plane_poles,predicted_class,sample_s,early_window_s,late_window_s,peak_position_m,early_envelope_m,late_envelope_m,late_early_ratio,peak_requested_force_percent,peak_applied_force_percent,applied_force_travel_pp,limit_contact,aborted,stop_reason,restoration,seed,applied,evidence_class\n\
stable,4.0,s^3+2s^2+3s+4,{:.4}{:+.4}j|{:.4}{:+.4}j|{:.4}{:+.4}j,0,asymptotically_stable,0.01,2.01..12.00,30.01..40.00,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},false,{},duration,safe_zero,20260829,false,simulation\n\
marginal,6.0,s^3+2s^2+3s+6,{:.4}{:+.4}j|{:.4}{:+.4}j|{:.4}{:+.4}j,0,marginal_not_settled,0.01,2.01..12.00,30.01..40.00,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},false,{},duration,safe_zero,20260829,false,simulation\n\
unstable,8.0,s^3+2s^2+3s+8,{:.4}{:+.4}j|{:.4}{:+.4}j|{:.4}{:+.4}j,2,unstable,0.01,2.01..12.00,30.01..40.00,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},false,{},local_range_abort,safe_zero,20260829,false,simulation\n",
        roots[0][0].re, roots[0][0].im, roots[0][1].re, roots[0][1].im, roots[0][2].re, roots[0][2].im,
        peak_positions[0], early_envelopes[0], late_envelopes[0], envelope_ratios[0], peak_requests[0], peak_commands[0], force_travel[0], abort_values[0],
        roots[1][0].re, roots[1][0].im, roots[1][1].re, roots[1][1].im, roots[1][2].re, roots[1][2].im,
        peak_positions[1], early_envelopes[1], late_envelopes[1], envelope_ratios[1], peak_requests[1], peak_commands[1], force_travel[1], abort_values[1],
        roots[2][0].re, roots[2][0].im, roots[2][1].re, roots[2][1].im, roots[2][2].re, roots[2][2].im,
        peak_positions[2], early_envelopes[2], late_envelopes[2], envelope_ratios[2], peak_requests[2], peak_commands[2], force_travel[2], abort_values[2],
    );
    evidence::retain(
        &evidence_root("crane-routh-stability"),
        "crane-routh-evidence",
        "Crane stability prediction and measured motion",
        "Routh-Hurwitz predicts the class; the simulator confirms the physical response.",
        &[
            evidence::Panel {
                title: "Closed-loop poles",
                x_unit: "real part (1/s)",
                y_unit: "imaginary part (rad/s)",
                x_minimum: -2.2,
                x_maximum: 0.4,
                y_minimum: -1.8,
                y_maximum: 1.8,
                series: vec![
                    evidence::Series {
                        name: "K = 4",
                        color: "#2673b8",
                        dashed: false,
                        x: &root_x[0],
                        y: &root_y[0],
                    },
                    evidence::Series {
                        name: "K = 6",
                        color: "#8a6d1d",
                        dashed: true,
                        x: &root_x[1],
                        y: &root_y[1],
                    },
                    evidence::Series {
                        name: "K = 8",
                        color: "#b43c32",
                        dashed: false,
                        x: &root_x[2],
                        y: &root_y[2],
                    },
                ],
            },
            evidence::Panel {
                title: "Crane position",
                x_unit: "time (s)",
                y_unit: "position (m)",
                x_minimum: 0.0,
                x_maximum: 40.0,
                y_minimum: -2.5,
                y_maximum: 2.5,
                series: vec![
                    evidence::Series {
                        name: "K = 4 position",
                        color: "#2673b8",
                        dashed: false,
                        x: &time,
                        y: &traces[0],
                    },
                    evidence::Series {
                        name: "K = 6 position",
                        color: "#8a6d1d",
                        dashed: true,
                        x: &time,
                        y: &traces[1],
                    },
                    evidence::Series {
                        name: "K = 8 position",
                        color: "#b43c32",
                        dashed: false,
                        x: &time,
                        y: &traces[2],
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Force",
                name: "K = 4 requested force",
                x: &time,
                y: &requests[0],
                x_unit: "time (s)",
                y_unit: "force (%)",
            },
            evidence::RetainedSeries {
                panel: "Force",
                name: "K = 4 applied force",
                x: &time,
                y: &commands[0],
                x_unit: "time (s)",
                y_unit: "force (%)",
            },
            evidence::RetainedSeries {
                panel: "Force",
                name: "K = 6 requested force",
                x: &time,
                y: &requests[1],
                x_unit: "time (s)",
                y_unit: "force (%)",
            },
            evidence::RetainedSeries {
                panel: "Force",
                name: "K = 6 applied force",
                x: &time,
                y: &commands[1],
                x_unit: "time (s)",
                y_unit: "force (%)",
            },
            evidence::RetainedSeries {
                panel: "Force",
                name: "K = 8 requested force",
                x: &time,
                y: &requests[2],
                x_unit: "time (s)",
                y_unit: "force (%)",
            },
            evidence::RetainedSeries {
                panel: "Force",
                name: "K = 8 applied force",
                x: &time,
                y: &commands[2],
                x_unit: "time (s)",
                y_unit: "force (%)",
            },
        ],
        &summary,
    );
    let _ = std::fs::remove_file(path);
    println!("crane K=4/6/8 envelope ratios={envelope_ratios:?}, roots={roots:?}");
}

fn settling_time(trace: &[f64], target: f64, tolerance: f64, step_seconds: f64) -> Option<f64> {
    (0..trace.len())
        .find(|start| {
            trace[*start..]
                .iter()
                .all(|value| (value - target).abs() <= tolerance)
        })
        .map(|index| index as f64 * step_seconds)
}

fn total_variation(trace: &[f64]) -> f64 {
    trace.windows(2).map(|pair| (pair[1] - pair[0]).abs()).sum()
}

#[test]
fn control_curriculum_root_locus_lead_design_meets_servo_damping_and_time_requirements() {
    let shipped_source = source_project_file("servo-root-locus", "src/Main.st");
    let simulation_source = source_project_file("servo-root-locus", "simulation.toml");
    assert_eq!(
        simulation_source.matches("[[plants]]").count(),
        2,
        "Lesson 46 requires equivalent proportional and lead-compensated servos"
    );
    for required in [
        "ProportionalGain : LREAL := 4.0",
        "LeadGain : LREAL := 50.0",
        "LeadZero : LREAL := 2.5",
        "LeadPole : LREAL := 15.0",
    ] {
        assert!(
            shipped_source.contains(required),
            "missing root-locus design value: {required}"
        );
    }
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("servo program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX96.0 : BOOL :=",
        )
        .replace(
            "ProportionalRequested : LREAL :=",
            "ProportionalRequested AT %ML0 : LREAL :=",
        )
        .replace(
            "LeadRequested : LREAL :=",
            "LeadRequested AT %ML8 : LREAL :=",
        )
        .replace(
            "ProportionalApplied : LREAL :=",
            "ProportionalApplied AT %QL0 : LREAL :=",
        )
        .replace("LeadApplied : LREAL :=", "LeadApplied AT %QL8 : LREAL :=")
        .replace(
            "ProportionalPosition : LREAL :=",
            "ProportionalPosition AT %IL0 : LREAL :=",
        )
        .replace("LeadPosition : LREAL :=", "LeadPosition AT %IL8 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "servo-root-locus/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile proportional and lead servo paths");
    let path = temporary_config_path("servo-root-locus-two-cases");
    std::fs::write(&path, simulation_source).expect("write servo simulation configuration");
    let mut plants = SimulationController::new(
        SimulationConfig::load(&path).expect("load equivalent servo plants"),
    );
    let enabled = IoAddress::parse("%MX96.0").unwrap();
    let positions = ["%IL0", "%IL8"].map(|address| IoAddress::parse(address).unwrap());
    let requested_outputs = ["%ML0", "%ML8"].map(|address| IoAddress::parse(address).unwrap());
    let outputs = ["%QL0", "%QL8"].map(|address| IoAddress::parse(address).unwrap());
    plants
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &enabled, true);
    let mut traces: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(800));
    let mut requests: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(800));
    let mut commands: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(800));
    for cycle in 1..=800 {
        runtime.execute_cycle().expect("execute both servo designs");
        let now = Duration::from_millis((cycle * 10) as i64);
        plants
            .apply_post_cycle(now, &runtime)
            .expect("advance servo plants");
        plants
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish servo positions");
        for index in 0..2 {
            traces[index].push(read_lreal(&runtime, &positions[index]));
            requests[index].push(read_lreal(&runtime, &requested_outputs[index]));
            commands[index].push(read_lreal(&runtime, &outputs[index]));
        }
    }

    let overshoots = traces.each_ref().map(|trace| {
        (trace.iter().copied().fold(f64::NEG_INFINITY, f64::max) - 1.0).max(0.0) * 100.0
    });
    let settling = traces
        .each_ref()
        .map(|trace| settling_time(trace, 1.0, 0.02, 0.01));
    let final_errors = traces.each_ref().map(|trace| {
        trace[700..800]
            .iter()
            .map(|value| (1.0 - value).abs())
            .sum::<f64>()
            / 100.0
    });
    assert!(
        (14.0..=19.0).contains(&overshoots[0]),
        "proportional overshoot={}",
        overshoots[0]
    );
    assert!(
        settling[0].is_some_and(|value| value > 3.0),
        "proportional settling={:?}",
        settling[0]
    );
    assert!(overshoots[1] < 5.0, "lead overshoot={}", overshoots[1]);
    assert!(
        settling[1].is_some_and(|value| value <= 2.0),
        "lead settling={:?}",
        settling[1]
    );
    assert!(
        final_errors[1] <= 0.01,
        "lead final-window error={}",
        final_errors[1]
    );
    assert!(
        commands
            .iter()
            .flatten()
            .all(|value| value.is_finite() && value.abs() < 100.0),
        "accepted servo designs must remain unsaturated"
    );

    let lead_roots = cubic_roots(17.0, 80.0, 125.0);
    let mut lead_sorted = lead_roots;
    lead_sorted.sort_by(|left, right| right.re.total_cmp(&left.re));
    assert!((lead_sorted[0].re + 3.235).abs() < 0.01 && (lead_sorted[1].re + 3.235).abs() < 0.01);
    assert!((lead_sorted[0].im.abs() - 1.186).abs() < 0.01);
    assert!((lead_sorted[2].re + 10.530).abs() < 0.02);
    for forbidden in [
        "Overshoot",
        "SettlingTime",
        "AcceptedDesign",
        "DominantPole",
    ] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds design result '{forbidden}'"
        );
    }
    let time = time_axis(traces[0].len(), 0.01);
    let proportional_pole_x = [-1.0, -1.0];
    let proportional_pole_y = [3.0_f64.sqrt(), -3.0_f64.sqrt()];
    let lead_pole_x = lead_sorted.map(|root| root.re);
    let lead_pole_y = lead_sorted.map(|root| root.im);
    let gain_sweep = (1..=100)
        .map(|index| index as f64 * 0.5)
        .collect::<Vec<_>>();
    let mut p_upper_x = Vec::with_capacity(gain_sweep.len());
    let mut p_upper_y = Vec::with_capacity(gain_sweep.len());
    let mut p_lower_x = Vec::with_capacity(gain_sweep.len());
    let mut p_lower_y = Vec::with_capacity(gain_sweep.len());
    let mut lead_branch_x: [Vec<f64>; 3] =
        std::array::from_fn(|_| Vec::with_capacity(gain_sweep.len()));
    let mut lead_branch_y: [Vec<f64>; 3] =
        std::array::from_fn(|_| Vec::with_capacity(gain_sweep.len()));
    for gain in &gain_sweep {
        if *gain <= 1.0 {
            p_upper_x.push(-1.0 + (1.0 - gain).sqrt());
            p_upper_y.push(0.0);
            p_lower_x.push(-1.0 - (1.0 - gain).sqrt());
            p_lower_y.push(0.0);
        } else {
            p_upper_x.push(-1.0);
            p_upper_y.push((gain - 1.0).sqrt());
            p_lower_x.push(-1.0);
            p_lower_y.push(-(gain - 1.0).sqrt());
        }
        let mut roots = cubic_roots(17.0, 30.0 + gain, 2.5 * gain);
        roots.sort_by(|left, right| {
            right
                .im
                .total_cmp(&left.im)
                .then(left.re.total_cmp(&right.re))
        });
        for branch in 0..3 {
            lead_branch_x[branch].push(roots[branch].re);
            lead_branch_y[branch].push(roots[branch].im);
        }
    }
    let damping_x = (0..=40)
        .map(|index| -12.0 + index as f64 * 12.0 / 40.0)
        .collect::<Vec<_>>();
    let damping_y = damping_x
        .iter()
        .map(|value| -value * (1.0_f64 - 0.7_f64.powi(2)).sqrt() / 0.7)
        .collect::<Vec<_>>();
    let damping_y_negative = damping_y.iter().map(|value| -value).collect::<Vec<_>>();
    let p_asymptote_x = [-1.0_f64, -1.0];
    let lead_asymptote_x = [-7.25_f64, -7.25];
    let asymptote_y = [-8.0_f64, 8.0];
    let p_open_loop_pole_x = [-2.0_f64, 0.0];
    let p_open_loop_pole_y = [0.0_f64, 0.0];
    let lead_open_loop_pole_x = [-15.0_f64, -2.0, 0.0];
    let lead_open_loop_pole_y = [0.0_f64, 0.0, 0.0];
    let lead_zero_x = [-2.5_f64];
    let lead_zero_y = [0.0_f64];
    let peak_requests = requests.each_ref().map(|trace| peak_absolute(trace));
    let peak_currents = commands.each_ref().map(|trace| peak_absolute(trace));
    let current_travel = commands.each_ref().map(|trace| total_variation(trace));
    let summary = format!(
        "case,controller,poles,damping_ratio,overshoot_percent,settling_s,final_window_s,final_error_mm,peak_requested_current_percent,peak_applied_current_percent,applied_current_travel_pp,time_at_limit_s,saturated,stop_reason,requirements_pass,seed,applied,evidence_class\n\
gain_only,P,-1.0000+1.7321j|-1.0000-1.7321j,0.5000,{:.4},{:.4},7.01..8.00,{:.6},{:.4},{:.4},{:.6},0.000,false,duration,false,20260829,false,simulation\n\
lead_compensated,lead,{:.4}{:+.4}j|{:.4}{:+.4}j|{:.4}{:+.4}j,0.9389,{:.4},{:.4},7.01..8.00,{:.6},{:.4},{:.4},{:.6},0.000,false,duration,true,20260829,false,simulation\n",
        overshoots[0], settling[0].unwrap(), final_errors[0], peak_requests[0], peak_currents[0], current_travel[0],
        lead_sorted[0].re, lead_sorted[0].im, lead_sorted[1].re, lead_sorted[1].im,
        lead_sorted[2].re, lead_sorted[2].im, overshoots[1], settling[1].unwrap(),
        final_errors[1], peak_requests[1], peak_currents[1], current_travel[1],
    );
    evidence::retain(
        &evidence_root("servo-root-locus"),
        "servo-root-locus-evidence",
        "Servo pole placement and measured position",
        "Gain alone misses the requirements; lead compensation moves the dominant poles.",
        &[
            evidence::Panel {
                title: "Root locus and requirement region",
                x_unit: "real part (1/s)",
                y_unit: "imaginary part (rad/s)",
                x_minimum: -16.0,
                x_maximum: 1.0,
                y_minimum: -8.0,
                y_maximum: 8.0,
                series: vec![
                    evidence::Series {
                        name: "P root-locus upper",
                        color: "#b43c32",
                        dashed: true,
                        x: &p_upper_x,
                        y: &p_upper_y,
                    },
                    evidence::Series {
                        name: "P root-locus lower",
                        color: "#b43c32",
                        dashed: true,
                        x: &p_lower_x,
                        y: &p_lower_y,
                    },
                    evidence::Series {
                        name: "Lead root-locus branch 1",
                        color: "#2673b8",
                        dashed: false,
                        x: &lead_branch_x[0],
                        y: &lead_branch_y[0],
                    },
                    evidence::Series {
                        name: "Lead root-locus branch 2",
                        color: "#2673b8",
                        dashed: false,
                        x: &lead_branch_x[1],
                        y: &lead_branch_y[1],
                    },
                    evidence::Series {
                        name: "Lead root-locus branch 3",
                        color: "#2673b8",
                        dashed: false,
                        x: &lead_branch_x[2],
                        y: &lead_branch_y[2],
                    },
                    evidence::Series {
                        name: "Damping ratio 0.7",
                        color: "#27865d",
                        dashed: true,
                        x: &damping_x,
                        y: &damping_y,
                    },
                    evidence::Series {
                        name: "Damping ratio -0.7",
                        color: "#27865d",
                        dashed: true,
                        x: &damping_x,
                        y: &damping_y_negative,
                    },
                    evidence::Series {
                        name: "P asymptote",
                        color: "#6b7280",
                        dashed: true,
                        x: &p_asymptote_x,
                        y: &asymptote_y,
                    },
                    evidence::Series {
                        name: "Lead asymptote",
                        color: "#9ca3af",
                        dashed: true,
                        x: &lead_asymptote_x,
                        y: &asymptote_y,
                    },
                    evidence::Series {
                        name: "P open-loop poles",
                        color: "#b43c32",
                        dashed: false,
                        x: &p_open_loop_pole_x,
                        y: &p_open_loop_pole_y,
                    },
                    evidence::Series {
                        name: "Lead open-loop poles",
                        color: "#2673b8",
                        dashed: false,
                        x: &lead_open_loop_pole_x,
                        y: &lead_open_loop_pole_y,
                    },
                    evidence::Series {
                        name: "Lead zero",
                        color: "#27865d",
                        dashed: false,
                        x: &lead_zero_x,
                        y: &lead_zero_y,
                    },
                    evidence::Series {
                        name: "Selected P poles",
                        color: "#b43c32",
                        dashed: false,
                        x: &proportional_pole_x,
                        y: &proportional_pole_y,
                    },
                    evidence::Series {
                        name: "Selected lead poles",
                        color: "#2673b8",
                        dashed: false,
                        x: &lead_pole_x,
                        y: &lead_pole_y,
                    },
                ],
            },
            evidence::Panel {
                title: "Table position",
                x_unit: "time (s)",
                y_unit: "position (mm)",
                x_minimum: 0.0,
                x_maximum: 8.0,
                y_minimum: 0.0,
                y_maximum: 1.25,
                series: vec![
                    evidence::Series {
                        name: "P position",
                        color: "#b43c32",
                        dashed: true,
                        x: &time,
                        y: &traces[0],
                    },
                    evidence::Series {
                        name: "Lead position",
                        color: "#2673b8",
                        dashed: false,
                        x: &time,
                        y: &traces[1],
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Current",
                name: "P requested current",
                x: &time,
                y: &requests[0],
                x_unit: "time (s)",
                y_unit: "current (%)",
            },
            evidence::RetainedSeries {
                panel: "Current",
                name: "P applied current",
                x: &time,
                y: &commands[0],
                x_unit: "time (s)",
                y_unit: "current (%)",
            },
            evidence::RetainedSeries {
                panel: "Current",
                name: "Lead requested current",
                x: &time,
                y: &requests[1],
                x_unit: "time (s)",
                y_unit: "current (%)",
            },
            evidence::RetainedSeries {
                panel: "Current",
                name: "Lead applied current",
                x: &time,
                y: &commands[1],
                x_unit: "time (s)",
                y_unit: "current (%)",
            },
        ],
        &summary,
    );
    let _ = std::fs::remove_file(path);
    println!("servo P/lead overshoot={overshoots:?}%, settling={settling:?}s, final error={final_errors:?}, lead roots={lead_sorted:?}");
}

#[test]
fn control_curriculum_bode_measurement_exposes_flexible_drive_resonance_and_margins() {
    let project = if PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/07-classical-stability-and-frequency-design/47-flexible-drive-bode")
        .is_dir()
    {
        "flexible-drive-bode"
    } else {
        "flexible-drive-nyquist"
    };
    let shipped_source = source_project_file(project, "src/Main.st");
    let simulation_source = source_project_file(project, "simulation.toml");
    assert_eq!(
        simulation_source.matches("[[plants]]").count(),
        3,
        "Lesson 47 requires equivalent low-, resonant-, and high-frequency drive copies"
    );
    for required in [
        "LowFrequency : LREAL := 0.5",
        "ResonantFrequency : LREAL := 2.0",
        "HighFrequency : LREAL := 8.0",
        "ProbeAmplitude : LREAL := 1.0",
    ] {
        assert!(
            shipped_source.contains(required),
            "missing executable Bode probe: {required}"
        );
    }

    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("flexible-drive program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX96.0 : BOOL :=",
        )
        .replace("LowRequested : LREAL :=", "LowRequested AT %ML0 : LREAL :=")
        .replace(
            "ResonantRequested : LREAL :=",
            "ResonantRequested AT %ML8 : LREAL :=",
        )
        .replace(
            "HighRequested : LREAL :=",
            "HighRequested AT %ML16 : LREAL :=",
        )
        .replace("LowApplied : LREAL :=", "LowApplied AT %QL0 : LREAL :=")
        .replace(
            "ResonantApplied : LREAL :=",
            "ResonantApplied AT %QL8 : LREAL :=",
        )
        .replace("HighApplied : LREAL :=", "HighApplied AT %QL16 : LREAL :=")
        .replace("LowSpeed : LREAL :=", "LowSpeed AT %IL0 : LREAL :=")
        .replace(
            "ResonantSpeed : LREAL :=",
            "ResonantSpeed AT %IL8 : LREAL :=",
        )
        .replace("HighSpeed : LREAL :=", "HighSpeed AT %IL16 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "flexible-drive-bode/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile three flexible-drive frequency probes");
    let path = temporary_config_path("flexible-drive-bode-three-frequencies");
    std::fs::write(&path, simulation_source)
        .expect("write flexible-drive simulation configuration");
    let mut plants = SimulationController::new(
        SimulationConfig::load(&path).expect("load flexible-drive simulation configuration"),
    );
    let enabled = IoAddress::parse("%MX96.0").unwrap();
    let requested_outputs =
        ["%ML0", "%ML8", "%ML16"].map(|address| IoAddress::parse(address).unwrap());
    let outputs = ["%QL0", "%QL8", "%QL16"].map(|address| IoAddress::parse(address).unwrap());
    let feedback = ["%IL0", "%IL8", "%IL16"].map(|address| IoAddress::parse(address).unwrap());
    plants
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &enabled, true);
    let mut requests: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(12_000));
    let mut commands: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(12_000));
    let mut speeds: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(12_000));
    for cycle in 1..=12_000 {
        runtime
            .execute_cycle()
            .expect("execute flexible-drive probes");
        let now = Duration::from_millis((cycle * 10) as i64);
        plants
            .apply_post_cycle(now, &runtime)
            .expect("advance flexible drives");
        plants
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish load speeds");
        for index in 0..3 {
            requests[index].push(read_lreal(&runtime, &requested_outputs[index]));
            commands[index].push(read_lreal(&runtime, &outputs[index]));
            speeds[index].push(read_lreal(&runtime, &feedback[index]) - 1_000.0);
        }
    }

    let frequencies = [0.5_f64, 2.0, 8.0];
    let expected_magnitudes = [0.2665_f64, 2.5, 0.01665];
    let expected_phases = [-1.53_f64, -90.0, -178.47];
    let mut measured = Vec::new();
    for index in 0..3 {
        let (input_amplitude, input_phase, input_quality) =
            fitted_sinusoid(&commands[index], 6_000, 0.01, frequencies[index]);
        let (output_amplitude, output_phase, output_quality) =
            fitted_sinusoid(&speeds[index], 6_000, 0.01, frequencies[index]);
        let magnitude = output_amplitude / input_amplitude;
        // A recorded command represents the value held across the completed
        // simulator step. Reference it to that interval's midpoint before
        // comparing the sampled estimate with the continuous-time model.
        let raw_phase = wrapped_degrees(output_phase - input_phase);
        let half_sample_phase = (0.5 * frequencies[index] * 0.01).to_degrees();
        let phase = wrapped_degrees((raw_phase - half_sample_phase).to_radians());
        assert!(
            (input_amplitude - 1.0).abs() < 1.0e-6,
            "probe amplitude={input_amplitude}"
        );
        assert!(
            (magnitude - expected_magnitudes[index]).abs() <= expected_magnitudes[index] * 0.03,
            "frequency={} magnitude={} expected={}",
            frequencies[index],
            magnitude,
            expected_magnitudes[index]
        );
        assert!(
            (phase - expected_phases[index]).abs() <= 2.0,
            "frequency={} phase={} expected={}",
            frequencies[index],
            phase,
            expected_phases[index]
        );
        let quality = input_quality.min(output_quality);
        assert!(
            quality > 0.99,
            "frequency={} fit quality={quality}",
            frequencies[index]
        );
        measured.push((
            input_amplitude,
            output_amplitude,
            magnitude,
            raw_phase,
            phase,
            quality,
        ));
    }

    let baseline = second_order_gain_crossover(0.2);
    let shaped = second_order_gain_crossover(3.2);
    assert!((baseline.0 - 2.994).abs() < 0.005 && (baseline.1 - 6.878).abs() < 0.05);
    assert!((shaped.0 - 1.443).abs() < 0.005 && (shaped.1 - 112.554).abs() < 0.05);
    let reduced_damping = second_order_gain_crossover(0.16);
    let shifted_resonance_denominator = 4.84_f64;
    assert!(
        reduced_damping.1 < baseline.1,
        "less shaft damping must reduce phase margin"
    );
    assert!(
        shifted_resonance_denominator > 4.0,
        "ten-percent frequency shift must perturb cancellation"
    );
    for forbidden in [
        "MagnitudeDb",
        "PhaseMargin",
        "AcceptedCandidate",
        "BodeResult",
    ] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds analysis result '{forbidden}'"
        );
    }
    let frequency_grid = (0..=80)
        .map(|index| 10.0_f64.powf(-1.0 + index as f64 * 2.5 / 80.0))
        .collect::<Vec<_>>();
    let log_frequency = frequency_grid
        .iter()
        .map(|value| value.log10())
        .collect::<Vec<_>>();
    let model_magnitude = frequency_grid
        .iter()
        .map(|omega| 1.0 / ((4.0 - omega * omega).powi(2) + (0.2 * omega).powi(2)).sqrt())
        .collect::<Vec<_>>();
    let model_phase = frequency_grid
        .iter()
        .map(|omega| -f64::atan2(0.2 * omega, 4.0 - omega * omega).to_degrees())
        .collect::<Vec<_>>();
    let measured_log_frequency = frequencies.map(f64::log10);
    let measured_magnitude = measured.iter().map(|value| value.2).collect::<Vec<_>>();
    let measured_phase = measured.iter().map(|value| value.4).collect::<Vec<_>>();
    let unshaped_magnitude = model_magnitude
        .iter()
        .map(|value| 5.0 * value)
        .collect::<Vec<_>>();
    let unshaped_phase = model_phase.clone();
    let shaped_magnitude = frequency_grid
        .iter()
        .map(|omega| 5.0 / ((4.0 - omega * omega).powi(2) + (3.2 * omega).powi(2)).sqrt())
        .collect::<Vec<_>>();
    let shaped_phase = frequency_grid
        .iter()
        .map(|omega| -f64::atan2(3.2 * omega, 4.0 - omega * omega).to_degrees())
        .collect::<Vec<_>>();
    let closed_loop_bandwidth = |damping: f64| {
        let values = frequency_grid
            .iter()
            .map(|omega| {
                let loop_response = ComplexValue { re: 5.0, im: 0.0 }.divide(ComplexValue {
                    re: 4.0 - omega * omega,
                    im: damping * omega,
                });
                loop_response
                    .divide(ComplexValue { re: 1.0, im: 0.0 }.add(loop_response))
                    .magnitude()
            })
            .collect::<Vec<_>>();
        let threshold = values[0] / 2.0_f64.sqrt();
        frequency_grid
            .iter()
            .zip(values)
            .find(|(_, value)| *value <= threshold)
            .map(|(frequency, _)| *frequency)
            .expect("closed-loop bandwidth in grid")
    };
    let unshaped_bandwidth = closed_loop_bandwidth(0.2);
    let shaped_bandwidth = closed_loop_bandwidth(3.2);
    let probe_time = time_axis(requests[0].len(), 0.01);
    let summary = format!(
        "case,record_type,controller,model_revision,controller_revision,sample_s,frequency_rad_s,input_amplitude_percent,output_amplitude_rpm,magnitude_rpm_per_percent,raw_phase_deg,corrected_phase_deg,fit_window_s,coherence,gain_crossovers_rad_s,phase_margins_deg,phase_crossover_rad_s,gain_margin,delay_margin_s,resonance_peak_rpm_per_percent,bandwidth_rad_s,perturbation,qualified,stop_reason,seed,applied,evidence_class\n\
low_probe,measured_probe,none,flexible-shaft-v1,probe-v1,0.01,{:.3},{:.6},{:.6},{:.6},{:.4},{:.4},60.01..120.00,{:.6},not_applicable,not_applicable,not_applicable,not_applicable,not_applicable,2.5,not_applicable,nominal,true,duration,20260829,false,simulation\n\
resonant_probe,measured_probe,none,flexible-shaft-v1,probe-v1,0.01,{:.3},{:.6},{:.6},{:.6},{:.4},{:.4},60.01..120.00,{:.6},not_applicable,not_applicable,not_applicable,not_applicable,not_applicable,2.5,not_applicable,nominal,true,duration,20260829,false,simulation\n\
high_probe,measured_probe,none,flexible-shaft-v1,probe-v1,0.01,{:.3},{:.6},{:.6},{:.6},{:.4},{:.4},60.01..120.00,{:.6},not_applicable,not_applicable,not_applicable,not_applicable,not_applicable,2.5,not_applicable,nominal,true,duration,20260829,false,simulation\n\
unshaped,controller_candidate,unshaped_gain_5,flexible-shaft-v1,unshaped-v1,0.01,{:.4},not_applicable,not_applicable,1.0,not_applicable,{:.4},not_applicable,not_applicable,{:.4},{:.4},unavailable,unbounded,{:.6},2.5,{:.4},nominal,true,analysis_complete,20260829,false,simulation\n\
resonance_shaped,controller_candidate,resonance_shaped,flexible-shaft-v1,shaped-v1,0.01,{:.4},not_applicable,not_applicable,1.0,not_applicable,{:.4},not_applicable,not_applicable,{:.4},{:.4},unavailable,unbounded,{:.6},2.5,{:.4},nominal,true,analysis_complete_unapplied,20260829,false,simulation\n\
reduced_damping,perturbation,resonance_shaped,flexible-shaft-damping-0.16,shaped-v1,0.01,{:.4},not_applicable,not_applicable,1.0,not_applicable,{:.4},not_applicable,not_applicable,{:.4},{:.4},unavailable,unbounded,{:.6},3.125,not_evaluated,reduced_damping,false,robustness_rejected,20260829,false,simulation\n",
        frequencies[0], measured[0].0, measured[0].1, measured[0].2, measured[0].3, measured[0].4, measured[0].5,
        frequencies[1], measured[1].0, measured[1].1, measured[1].2, measured[1].3, measured[1].4, measured[1].5,
        frequencies[2], measured[2].0, measured[2].1, measured[2].2, measured[2].3, measured[2].4, measured[2].5,
        baseline.0, -180.0 + baseline.1, baseline.0, baseline.1, baseline.1.to_radians() / baseline.0, unshaped_bandwidth,
        shaped.0, -180.0 + shaped.1, shaped.0, shaped.1, shaped.1.to_radians() / shaped.0, shaped_bandwidth,
        reduced_damping.0, -180.0 + reduced_damping.1, reduced_damping.0, reduced_damping.1,
        reduced_damping.1.to_radians() / reduced_damping.0,
    );
    evidence::retain(
        &evidence_root("flexible-drive-bode"),
        "flexible-drive-bode-evidence",
        "Flexible-drive frequency response",
        "Measured simulator points agree with the model and expose the shaft resonance.",
        &[
            evidence::Panel {
                title: "Magnitude",
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "speed / torque (rpm/%)",
                x_minimum: -1.0,
                x_maximum: 1.5,
                y_minimum: 0.0,
                y_maximum: 2.7,
                series: vec![
                    evidence::Series {
                        name: "Model",
                        color: "#2673b8",
                        dashed: false,
                        x: &log_frequency,
                        y: &model_magnitude,
                    },
                    evidence::Series {
                        name: "Measured",
                        color: "#b43c32",
                        dashed: true,
                        x: &measured_log_frequency,
                        y: &measured_magnitude,
                    },
                ],
            },
            evidence::Panel {
                title: "Phase",
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "phase (degrees)",
                x_minimum: -1.0,
                x_maximum: 1.5,
                y_minimum: -190.0,
                y_maximum: 10.0,
                series: vec![
                    evidence::Series {
                        name: "Model",
                        color: "#2673b8",
                        dashed: false,
                        x: &log_frequency,
                        y: &model_phase,
                    },
                    evidence::Series {
                        name: "Measured",
                        color: "#b43c32",
                        dashed: true,
                        x: &measured_log_frequency,
                        y: &measured_phase,
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "Low requested torque",
                x: &probe_time,
                y: &requests[0],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "Low applied torque",
                x: &probe_time,
                y: &commands[0],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "Low load speed",
                x: &probe_time,
                y: &speeds[0],
                x_unit: "time (s)",
                y_unit: "speed deviation (rpm)",
            },
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "Resonant requested torque",
                x: &probe_time,
                y: &requests[1],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "Resonant applied torque",
                x: &probe_time,
                y: &commands[1],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "Resonant load speed",
                x: &probe_time,
                y: &speeds[1],
                x_unit: "time (s)",
                y_unit: "speed deviation (rpm)",
            },
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "High requested torque",
                x: &probe_time,
                y: &requests[2],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "High applied torque",
                x: &probe_time,
                y: &commands[2],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Probe run",
                name: "High load speed",
                x: &probe_time,
                y: &speeds[2],
                x_unit: "time (s)",
                y_unit: "speed deviation (rpm)",
            },
            evidence::RetainedSeries {
                panel: "Controller Bode",
                name: "Unshaped magnitude",
                x: &log_frequency,
                y: &unshaped_magnitude,
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "magnitude",
            },
            evidence::RetainedSeries {
                panel: "Controller Bode",
                name: "Shaped magnitude",
                x: &log_frequency,
                y: &shaped_magnitude,
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "magnitude",
            },
            evidence::RetainedSeries {
                panel: "Controller Bode",
                name: "Unshaped phase",
                x: &log_frequency,
                y: &unshaped_phase,
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "phase (degrees)",
            },
            evidence::RetainedSeries {
                panel: "Controller Bode",
                name: "Shaped phase",
                x: &log_frequency,
                y: &shaped_phase,
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "phase (degrees)",
            },
        ],
        &summary,
    );
    evidence::retain(
        &evidence_root("flexible-drive-bode"),
        "flexible-drive-controller-bode-evidence",
        "Unshaped and resonance-shaped loop response",
        "The candidate moves crossover away from resonance but remains unapplied until validation.",
        &[
            evidence::Panel {
                title: "Open-loop magnitude",
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "magnitude",
                x_minimum: -1.0,
                x_maximum: 1.5,
                y_minimum: 0.0,
                y_maximum: 13.0,
                series: vec![
                    evidence::Series {
                        name: "Unshaped magnitude",
                        color: "#b43c32",
                        dashed: true,
                        x: &log_frequency,
                        y: &unshaped_magnitude,
                    },
                    evidence::Series {
                        name: "Shaped magnitude",
                        color: "#2673b8",
                        dashed: false,
                        x: &log_frequency,
                        y: &shaped_magnitude,
                    },
                ],
            },
            evidence::Panel {
                title: "Open-loop phase",
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "phase (degrees)",
                x_minimum: -1.0,
                x_maximum: 1.5,
                y_minimum: -190.0,
                y_maximum: 10.0,
                series: vec![
                    evidence::Series {
                        name: "Unshaped phase",
                        color: "#b43c32",
                        dashed: true,
                        x: &log_frequency,
                        y: &unshaped_phase,
                    },
                    evidence::Series {
                        name: "Shaped phase",
                        color: "#2673b8",
                        dashed: false,
                        x: &log_frequency,
                        y: &shaped_phase,
                    },
                ],
            },
        ],
        &[],
        &summary,
    );
    let _ = std::fs::remove_file(path);
    println!("flexible-drive measured magnitude/phase={measured:?}, baseline margin={baseline:?}, shaped margin={shaped:?}");
}

#[test]
fn control_curriculum_nyquist_count_predicts_delayed_oven_stability() {
    let project = if PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/07-classical-stability-and-frequency-design/48-delayed-thermal-nyquist")
        .is_dir()
    {
        "delayed-thermal-nyquist"
    } else {
        "boiler-drum-inverse-response"
    };
    let shipped_source = source_project_file(project, "src/Main.st");
    let simulation_source = source_project_file(project, "simulation.toml");
    assert_eq!(
        simulation_source.matches("[[plants]]").count(),
        3,
        "Lesson 48 requires equivalent 5-second, 25-second, and 60-second oven copies"
    );
    for required in [
        "ProportionalGain : LREAL := 2.0",
        "HoldingCommand : LREAL := 50.0",
        "Setpoint : LREAL := 101.0",
    ] {
        assert!(
            shipped_source.contains(required),
            "missing executable delayed-oven value: {required}"
        );
    }

    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("delayed-oven program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX96.0 : BOOL :=",
        )
        .replace(
            "ShortDelayAborted : BOOL :=",
            "ShortDelayAborted AT %MX97.0 : BOOL :=",
        )
        .replace(
            "MediumDelayAborted : BOOL :=",
            "MediumDelayAborted AT %MX97.1 : BOOL :=",
        )
        .replace(
            "LongDelayAborted : BOOL :=",
            "LongDelayAborted AT %MX97.2 : BOOL :=",
        )
        .replace(
            "ShortDelayRequested : LREAL :=",
            "ShortDelayRequested AT %ML0 : LREAL :=",
        )
        .replace(
            "MediumDelayRequested : LREAL :=",
            "MediumDelayRequested AT %ML8 : LREAL :=",
        )
        .replace(
            "LongDelayRequested : LREAL :=",
            "LongDelayRequested AT %ML16 : LREAL :=",
        )
        .replace(
            "ShortDelayApplied : LREAL :=",
            "ShortDelayApplied AT %QL0 : LREAL :=",
        )
        .replace(
            "MediumDelayApplied : LREAL :=",
            "MediumDelayApplied AT %QL8 : LREAL :=",
        )
        .replace(
            "LongDelayApplied : LREAL :=",
            "LongDelayApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "ShortDelayTemperature : LREAL :=",
            "ShortDelayTemperature AT %IL0 : LREAL :=",
        )
        .replace(
            "MediumDelayTemperature : LREAL :=",
            "MediumDelayTemperature AT %IL8 : LREAL :=",
        )
        .replace(
            "LongDelayTemperature : LREAL :=",
            "LongDelayTemperature AT %IL16 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "delayed-thermal-nyquist/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile three delayed-oven proportional loops");
    let path = temporary_config_path("delayed-thermal-nyquist-three-delays");
    std::fs::write(&path, simulation_source).expect("write delayed-oven simulation configuration");
    let mut plants = SimulationController::new(
        SimulationConfig::load(&path).expect("load delayed-oven simulation configuration"),
    );
    let enabled = IoAddress::parse("%MX96.0").unwrap();
    let aborts =
        ["%MX97.0", "%MX97.1", "%MX97.2"].map(|address| IoAddress::parse(address).unwrap());
    let requested_outputs =
        ["%ML0", "%ML8", "%ML16"].map(|address| IoAddress::parse(address).unwrap());
    let outputs = ["%QL0", "%QL8", "%QL16"].map(|address| IoAddress::parse(address).unwrap());
    let feedback = ["%IL0", "%IL8", "%IL16"].map(|address| IoAddress::parse(address).unwrap());
    plants
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &enabled, true);
    let mut errors: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(6_000));
    let mut requests: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(6_000));
    let mut commands: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(6_000));
    for cycle in 1..=6_000 {
        runtime.execute_cycle().expect("execute delayed-oven loops");
        let now = Duration::from_millis((cycle * 100) as i64);
        plants
            .apply_post_cycle(now, &runtime)
            .expect("advance delayed ovens");
        plants
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish oven temperatures");
        for index in 0..3 {
            errors[index].push((100.0 + 2.0 / 3.0 - read_lreal(&runtime, &feedback[index])).abs());
            requests[index].push(read_lreal(&runtime, &requested_outputs[index]));
            commands[index].push(read_lreal(&runtime, &outputs[index]));
        }
    }

    let early = errors
        .each_ref()
        .map(|trace| peak_absolute(&trace[1_000..2_000]));
    let late = errors
        .each_ref()
        .map(|trace| peak_absolute(&trace[5_000..6_000]));
    assert!(
        !read_bool(&runtime, &aborts[0]),
        "five-second-delay loop must remain inside its local range"
    );
    assert!(
        late[0] < 0.001 && late[0] < 0.2 * early[0],
        "stable envelope early={} late={}",
        early[0],
        late[0]
    );
    for index in 1..3 {
        assert!(
            read_bool(&runtime, &aborts[index]) || late[index] > 1.2 * early[index],
            "unstable delay case {index} neither grew nor aborted: early={} late={}",
            early[index],
            late[index]
        );
    }
    assert!(commands
        .iter()
        .flatten()
        .all(|value| value.is_finite() && (0.0..=100.0).contains(value)));

    let crossover = 3.0_f64.sqrt() / 20.0;
    let phases = [5.0_f64, 25.0, 60.0]
        .map(|delay| (-f64::atan(20.0 * crossover) - delay * crossover).to_degrees());
    assert!((phases[0] + 84.810).abs() < 0.01);
    assert!((phases[1] + 184.049).abs() < 0.01);
    assert!((phases[2] + 357.718).abs() < 0.01);
    let executable_source = st_executable_text(&shipped_source);
    for forbidden in [
        "Nyquist",
        "Encirclement",
        "RightHalfPlane",
        "StableLabel",
        "PhaseMargin",
    ] {
        assert!(
            !executable_source.contains(forbidden),
            "ST embeds analysis result '{forbidden}'"
        );
    }
    let contour_frequency = (0..=200)
        .map(|index| -0.5 + index as f64 * 1.0 / 200.0)
        .collect::<Vec<_>>();
    let contours = [5.0_f64, 25.0, 60.0].map(|delay| {
        let mut real = Vec::with_capacity(contour_frequency.len());
        let mut imaginary = Vec::with_capacity(contour_frequency.len());
        for omega in &contour_frequency {
            let angle = -omega * delay;
            let numerator_re = 2.0 * angle.cos();
            let numerator_im = 2.0 * angle.sin();
            let denominator = 1.0 + (20.0 * omega).powi(2);
            real.push((numerator_re + numerator_im * 20.0 * omega) / denominator);
            imaginary.push((numerator_im - numerator_re * 20.0 * omega) / denominator);
        }
        (real, imaginary)
    });
    let abort_values = aborts
        .each_ref()
        .map(|address| read_bool(&runtime, address));
    write_bool(&mut runtime, &enabled, false);
    runtime
        .execute_cycle()
        .expect("restore all delayed-oven paths to holding output");
    let restored = outputs
        .each_ref()
        .map(|address| read_lreal(&runtime, address));
    assert!(restored.iter().all(|value| (*value - 50.0).abs() < 1.0e-12));

    let time = time_axis(errors[0].len(), 0.1);
    let critical_x = [-1.0_f64];
    let critical_y = [0.0_f64];
    let delays = [5.0_f64, 25.0, 60.0];
    let minimum_distances = contours.each_ref().map(|contour| {
        contour
            .0
            .iter()
            .zip(&contour.1)
            .map(|(real, imaginary)| (real + 1.0).hypot(*imaginary))
            .fold(f64::INFINITY, f64::min)
    });
    let endpoint_magnitudes = contours.each_ref().map(|contour| {
        contour.0[0]
            .hypot(contour.1[0])
            .max(contour.0[contour.0.len() - 1].hypot(contour.1[contour.1.len() - 1]))
    });
    let peak_requests = requests.each_ref().map(|trace| peak_absolute(trace));
    let peak_commands = commands.each_ref().map(|trace| peak_absolute(trace));
    let limit_contacts = std::array::from_fn::<_, 3, _>(|index| {
        requests[index]
            .iter()
            .any(|value| !(0.0..=100.0).contains(value))
            || commands[index]
                .iter()
                .any(|value| *value <= 0.0 || *value >= 100.0)
    });
    let wrapped_phases = phases.map(|phase| wrapped_degrees(phase.to_radians()));
    let encirclements = [0_i32, -2, -2];
    let closed_loop_poles = [0_i32, 2, 2];
    let predicted_classes = ["stable", "unstable", "unstable"];
    let summary = format!(
        "case,delay_s,model_revision,controller_revision,sample_s,setpoint_deg_c,predicted_equilibrium_deg_c,crossover_rad_s,wrapped_phase_deg,unwrapped_phase_deg,open_loop_rhp_poles,ccw_encirclements,closed_loop_rhp_poles,sweep_min_rad_s,sweep_max_rad_s,points_per_decade,endpoint_magnitude,minimum_distance_to_minus_one,early_window_s,late_window_s,early_envelope_deg_c,late_envelope_deg_c,peak_requested_heater_percent,peak_applied_heater_percent,limit_contact,aborted,stop_reason,restoration,seed,predicted_class,applied,evidence_class\n{}",
        (0..3).map(|index| format!(
            "{}_delay,{:.0},oven-fopdt-v1,p-bias-v1,0.1,101.0,100.666667,{:.9},{:.6},{:.6},0,{},{},-0.5,0.5,linear_200,{:.9},{:.9},100.1..200.0,500.1..600.0,{:.9},{:.9},{:.6},{:.6},{},{},{},holding_50_percent,20260829,{},false,simulation\n",
            ["short", "medium", "long"][index], delays[index], crossover,
            wrapped_phases[index], phases[index], encirclements[index], closed_loop_poles[index],
            endpoint_magnitudes[index], minimum_distances[index], early[index], late[index],
            peak_requests[index], peak_commands[index], limit_contacts[index], abort_values[index],
            if abort_values[index] { "temperature_boundary" } else { "duration" },
            predicted_classes[index],
        )).collect::<String>(),
    );
    evidence::retain(
        &evidence_root("delayed-thermal-nyquist"),
        "delayed-thermal-nyquist-evidence",
        "Delay changes Nyquist stability",
        "The complete contour predicts which identical thermal loops remain stable.",
        &[
            evidence::Panel {
                title: "Open-loop Nyquist contour",
                x_unit: "real part",
                y_unit: "imaginary part",
                x_minimum: -2.2,
                x_maximum: 2.2,
                y_minimum: -2.2,
                y_maximum: 2.2,
                series: vec![
                    evidence::Series {
                        name: "5 s Nyquist contour",
                        color: "#2673b8",
                        dashed: false,
                        x: &contours[0].0,
                        y: &contours[0].1,
                    },
                    evidence::Series {
                        name: "25 s Nyquist contour",
                        color: "#8a6d1d",
                        dashed: true,
                        x: &contours[1].0,
                        y: &contours[1].1,
                    },
                    evidence::Series {
                        name: "60 s Nyquist contour",
                        color: "#b43c32",
                        dashed: false,
                        x: &contours[2].0,
                        y: &contours[2].1,
                    },
                    evidence::Series {
                        name: "Critical -1 point",
                        color: "#111827",
                        dashed: false,
                        x: &critical_x,
                        y: &critical_y,
                    },
                ],
            },
            evidence::Panel {
                title: "Distance from equilibrium",
                x_unit: "time (s)",
                y_unit: "absolute error (deg C)",
                x_minimum: 0.0,
                x_maximum: 600.0,
                y_minimum: 0.0,
                y_maximum: 12.0,
                series: vec![
                    evidence::Series {
                        name: "5 s equilibrium error",
                        color: "#2673b8",
                        dashed: false,
                        x: &time,
                        y: &errors[0],
                    },
                    evidence::Series {
                        name: "25 s equilibrium error",
                        color: "#8a6d1d",
                        dashed: true,
                        x: &time,
                        y: &errors[1],
                    },
                    evidence::Series {
                        name: "60 s equilibrium error",
                        color: "#b43c32",
                        dashed: false,
                        x: &time,
                        y: &errors[2],
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Heater command",
                name: "5 s requested heater",
                x: &time,
                y: &requests[0],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Heater command",
                name: "5 s applied heater",
                x: &time,
                y: &commands[0],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Heater command",
                name: "25 s requested heater",
                x: &time,
                y: &requests[1],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Heater command",
                name: "25 s applied heater",
                x: &time,
                y: &commands[1],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Heater command",
                name: "60 s requested heater",
                x: &time,
                y: &requests[2],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
            evidence::RetainedSeries {
                panel: "Heater command",
                name: "60 s applied heater",
                x: &time,
                y: &commands[2],
                x_unit: "time (s)",
                y_unit: "heater (%)",
            },
        ],
        &summary,
    );
    let _ = std::fs::remove_file(path);
    println!("delayed-oven phases={phases:?}, early envelopes={early:?}, late envelopes={late:?}");
}

#[test]
fn control_curriculum_loop_shaping_balances_motor_sensitivity_and_effort() {
    let shipped_source = source_project_file("motor-sensitivity", "src/Main.st");
    let simulation_source = source_project_file("motor-sensitivity", "simulation.toml");
    assert_eq!(
        simulation_source.matches("[[plants]]").count(),
        2,
        "Lesson 49 requires equivalent baseline and loop-shaped motor copies"
    );
    for required in [
        "ProportionalGain : LREAL := 0.4",
        "LeadZero : LREAL := 0.3",
        "LeadPole : LREAL := 3.0",
        "LagZero : LREAL := 0.8",
        "LagPole : LREAL := 0.08",
        "RollOffPole : LREAL := 8.0",
    ] {
        assert!(
            shipped_source.contains(required),
            "missing loop-shaping design value: {required}"
        );
    }

    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("motor-sensitivity program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX96.0 : BOOL :=",
        )
        .replace(
            "BaselineAborted : BOOL :=",
            "BaselineAborted AT %MX97.0 : BOOL :=",
        )
        .replace(
            "ShapedAborted : BOOL :=",
            "ShapedAborted AT %MX97.1 : BOOL :=",
        )
        .replace(
            "BaselineRequestedTorque : LREAL :=",
            "BaselineRequestedTorque AT %ML0 : LREAL :=",
        )
        .replace(
            "ShapedRequestedTorque : LREAL :=",
            "ShapedRequestedTorque AT %ML8 : LREAL :=",
        )
        .replace(
            "BaselineAppliedTorque : LREAL :=",
            "BaselineAppliedTorque AT %QL0 : LREAL :=",
        )
        .replace(
            "BaselineLoadTorque : LREAL :=",
            "BaselineLoadTorque AT %QL8 : LREAL :=",
        )
        .replace(
            "ShapedAppliedTorque : LREAL :=",
            "ShapedAppliedTorque AT %QL16 : LREAL :=",
        )
        .replace(
            "ShapedLoadTorque : LREAL :=",
            "ShapedLoadTorque AT %QL24 : LREAL :=",
        )
        .replace(
            "BaselineMeasuredSpeed : LREAL :=",
            "BaselineMeasuredSpeed AT %IL0 : LREAL :=",
        )
        .replace(
            "BaselineTrueSpeed : LREAL :=",
            "BaselineTrueSpeed AT %IL8 : LREAL :=",
        )
        .replace(
            "ShapedMeasuredSpeed : LREAL :=",
            "ShapedMeasuredSpeed AT %IL16 : LREAL :=",
        )
        .replace(
            "ShapedTrueSpeed : LREAL :=",
            "ShapedTrueSpeed AT %IL24 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "motor-sensitivity/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile proportional and loop-shaped motor paths");
    let path = temporary_config_path("motor-sensitivity-two-cases");
    std::fs::write(&path, simulation_source)
        .expect("write motor-sensitivity simulation configuration");
    let mut plants = SimulationController::new(
        SimulationConfig::load(&path).expect("load equivalent motor plants"),
    );
    let enabled = IoAddress::parse("%MX96.0").unwrap();
    let true_speeds = ["%IL8", "%IL24"].map(|address| IoAddress::parse(address).unwrap());
    let requested_torques = ["%ML0", "%ML8"].map(|address| IoAddress::parse(address).unwrap());
    let torques = ["%QL0", "%QL16"].map(|address| IoAddress::parse(address).unwrap());
    let aborts = ["%MX97.0", "%MX97.1"].map(|address| IoAddress::parse(address).unwrap());
    plants
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &enabled, true);
    let mut speed_traces: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(12_000));
    let mut requested_torque_traces: [Vec<f64>; 2] =
        std::array::from_fn(|_| Vec::with_capacity(12_000));
    let mut torque_traces: [Vec<f64>; 2] = std::array::from_fn(|_| Vec::with_capacity(12_000));
    for cycle in 1..=12_000 {
        runtime
            .execute_cycle()
            .expect("execute both motor controllers");
        let now = Duration::from_millis((cycle * 10) as i64);
        plants
            .apply_post_cycle(now, &runtime)
            .expect("advance both motor plants");
        plants
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish measured and true motor speeds");
        for index in 0..2 {
            speed_traces[index].push(read_lreal(&runtime, &true_speeds[index]));
            requested_torque_traces[index].push(read_lreal(&runtime, &requested_torques[index]));
            torque_traces[index].push(read_lreal(&runtime, &torques[index]));
        }
    }

    let overshoot_percent = speed_traces.each_ref().map(|trace| {
        let peak = trace[..6_000]
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        ((peak - 0.8) / 0.8 * 100.0).max(0.0)
    });
    let pre_load_settling = speed_traces
        .each_ref()
        .map(|trace| settling_time(&trace[..6_000], 0.8, 0.016, 0.01));
    let post_load_departure = speed_traces.each_ref().map(|trace| {
        peak_absolute(
            &trace[6_000..]
                .iter()
                .map(|value| value - 0.6)
                .collect::<Vec<_>>(),
        )
    });
    let final_errors = speed_traces.each_ref().map(|trace| {
        trace[11_000..12_000]
            .iter()
            .map(|value| (value - 0.6).abs())
            .sum::<f64>()
            / 1_000.0
    });
    let quiet_torque_travel = torque_traces
        .each_ref()
        .map(|trace| total_variation(&trace[10_000..12_000]));
    let requested_torque_travel = requested_torque_traces
        .each_ref()
        .map(|trace| total_variation(trace));
    let applied_torque_travel = torque_traces.each_ref().map(|trace| total_variation(trace));
    let quiet_requested_torque_travel = requested_torque_traces
        .each_ref()
        .map(|trace| total_variation(&trace[10_000..12_000]));

    assert!(
        overshoot_percent[1] < 0.65 * overshoot_percent[0],
        "shaping did not materially reduce overshoot: {overshoot_percent:?}"
    );
    assert!(
        pre_load_settling.iter().all(Option::is_some),
        "both cases must settle before the load step"
    );
    assert!(
        post_load_departure[1] > 1.25 * post_load_departure[0],
        "the bandwidth cost must remain visible: {post_load_departure:?}"
    );
    assert!(
        quiet_torque_travel[1] < 0.65 * quiet_torque_travel[0],
        "shaping did not reduce encoder-noise torque travel: {quiet_torque_travel:?}"
    );
    assert!(
        final_errors.iter().all(|error| *error < 0.01),
        "final equilibrium errors={final_errors:?}"
    );
    assert!(
        aborts.iter().all(|address| !read_bool(&runtime, address)),
        "qualified runs must not abort"
    );
    assert!(torque_traces
        .iter()
        .flatten()
        .all(|value| value.is_finite() && value.abs() <= 20.0));
    for forbidden in [
        "MaximumSensitivity",
        "Bandwidth",
        "AcceptedCandidate",
        "SensitivityResult",
    ] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds analysis result '{forbidden}'"
        );
    }
    let frequency = (0..=400)
        .map(|index| 10.0_f64.powf(-3.0 + index as f64 * 5.0 / 400.0))
        .collect::<Vec<_>>();
    let log_frequency = frequency
        .iter()
        .map(|value| value.log10())
        .collect::<Vec<_>>();
    let response = |shaped: bool| {
        let mut loop_magnitude = Vec::with_capacity(frequency.len());
        let mut sensitivity = Vec::with_capacity(frequency.len());
        let mut complementary = Vec::with_capacity(frequency.len());
        let mut disturbance = Vec::with_capacity(frequency.len());
        let mut control = Vec::with_capacity(frequency.len());
        let mut complementarity_residual = Vec::with_capacity(frequency.len());
        for omega in &frequency {
            let plant = ComplexValue { re: 10.0, im: 0.0 }.divide(ComplexValue {
                re: 1.0 - omega * omega,
                im: 0.6 * omega,
            });
            let controller = if shaped {
                let factor = |corner: f64| ComplexValue {
                    re: 1.0,
                    im: omega / corner,
                };
                factor(0.3)
                    .divide(factor(3.0))
                    .multiply(factor(0.8))
                    .divide(factor(0.08))
                    .divide(factor(8.0))
                    .scale(0.4)
            } else {
                ComplexValue { re: 0.4, im: 0.0 }
            };
            let loop_response = controller.multiply(plant);
            let denominator = ComplexValue { re: 1.0, im: 0.0 }.add(loop_response);
            let sensitivity_response = ComplexValue { re: 1.0, im: 0.0 }.divide(denominator);
            let complementary_response = loop_response.divide(denominator);
            loop_magnitude.push(loop_response.magnitude());
            sensitivity.push(sensitivity_response.magnitude());
            complementary.push(complementary_response.magnitude());
            disturbance.push(plant.multiply(sensitivity_response).magnitude());
            control.push(controller.multiply(sensitivity_response).magnitude());
            complementarity_residual.push(
                sensitivity_response
                    .add(complementary_response)
                    .subtract(ComplexValue { re: 1.0, im: 0.0 })
                    .magnitude(),
            );
        }
        (
            loop_magnitude,
            sensitivity,
            complementary,
            disturbance,
            control,
            complementarity_residual,
        )
    };
    let baseline_response = response(false);
    let shaped_response = response(true);
    let maximum = |values: &[f64]| values.iter().copied().fold(0.0, f64::max);
    assert!((maximum(&baseline_response.1) - 3.197).abs() < 0.03);
    assert!((maximum(&shaped_response.1) - 1.724).abs() < 0.03);
    assert!((maximum(&baseline_response.2) - 3.009).abs() < 0.03);
    assert!((maximum(&shaped_response.2) - 1.334).abs() < 0.03);
    assert!(maximum(&baseline_response.5) < 1.0e-12 && maximum(&shaped_response.5) < 1.0e-12);
    let time = time_axis(speed_traces[0].len(), 0.01);
    let limit_contacts = std::array::from_fn::<_, 2, _>(|index| {
        requested_torque_traces[index]
            .iter()
            .any(|value| value.abs() > 20.0)
            || torque_traces[index].iter().any(|value| value.abs() >= 20.0)
    });
    let abort_values = aborts
        .each_ref()
        .map(|address| read_bool(&runtime, address));
    let summary = format!(
        "case,model_revision,controller_revision,maximum_sensitivity,maximum_complementary_sensitivity,complementarity_residual,crossover_rad_s,phase_margin_deg,bandwidth_rad_s,s_at_0_01,gs_at_0_01,t_at_10,cs_at_10,overshoot_percent,settling_s,load_departure_rpm,requested_torque_travel_pp,applied_torque_travel_pp,quiet_requested_torque_travel_pp,limit_contact,aborted,stop_reason,qualified,seed,applied,evidence_class\n\
baseline,compliant-motor-v1,proportional-v1,{:.6},{:.6},{:.3e},2.187,19.13,3.43,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},duration,true,20260829,false,simulation\n\
loop_shaped,compliant-motor-v1,lead-lag-rolloff-v1,{:.6},{:.6},{:.3e},1.706,44.11,2.83,{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{},{},duration,true,20260829,false,simulation\n",
        maximum(&baseline_response.1), maximum(&baseline_response.2), maximum(&baseline_response.5),
        baseline_response.1[80], baseline_response.3[80], baseline_response.2[320], baseline_response.4[320],
        overshoot_percent[0], pre_load_settling[0].unwrap(), post_load_departure[0],
        requested_torque_travel[0], applied_torque_travel[0], quiet_requested_torque_travel[0],
        limit_contacts[0], abort_values[0],
        maximum(&shaped_response.1), maximum(&shaped_response.2), maximum(&shaped_response.5),
        shaped_response.1[80], shaped_response.3[80], shaped_response.2[320], shaped_response.4[320],
        overshoot_percent[1], pre_load_settling[1].unwrap(), post_load_departure[1],
        requested_torque_travel[1], applied_torque_travel[1], quiet_requested_torque_travel[1],
        limit_contacts[1], abort_values[1],
    );
    evidence::retain(
        &evidence_root("motor-sensitivity"),
        "motor-sensitivity-evidence",
        "Motor sensitivity and physical response",
        "Loop shaping reduces resonance and torque activity while exposing its bandwidth cost.",
        &[
            evidence::Panel {
                title: "Loop and sensitivity paths",
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "magnitude",
                x_minimum: -3.0,
                x_maximum: 2.0,
                y_minimum: 0.0,
                y_maximum: 3.5,
                series: vec![
                    evidence::Series {
                        name: "Baseline L",
                        color: "#6b7280",
                        dashed: true,
                        x: &log_frequency,
                        y: &baseline_response.0,
                    },
                    evidence::Series {
                        name: "Baseline S",
                        color: "#b43c32",
                        dashed: true,
                        x: &log_frequency,
                        y: &baseline_response.1,
                    },
                    evidence::Series {
                        name: "Baseline T",
                        color: "#b97a28",
                        dashed: true,
                        x: &log_frequency,
                        y: &baseline_response.2,
                    },
                    evidence::Series {
                        name: "Shaped L",
                        color: "#4b5563",
                        dashed: false,
                        x: &log_frequency,
                        y: &shaped_response.0,
                    },
                    evidence::Series {
                        name: "Shaped S",
                        color: "#2673b8",
                        dashed: false,
                        x: &log_frequency,
                        y: &shaped_response.1,
                    },
                    evidence::Series {
                        name: "Shaped T",
                        color: "#27865d",
                        dashed: false,
                        x: &log_frequency,
                        y: &shaped_response.2,
                    },
                ],
            },
            evidence::Panel {
                title: "Disturbance and control paths",
                x_unit: "log10 angular frequency (rad/s)",
                y_unit: "magnitude",
                x_minimum: -3.0,
                x_maximum: 2.0,
                y_minimum: 0.0,
                y_maximum: 18.0,
                series: vec![
                    evidence::Series {
                        name: "Baseline GS",
                        color: "#b43c32",
                        dashed: true,
                        x: &log_frequency,
                        y: &baseline_response.3,
                    },
                    evidence::Series {
                        name: "Baseline CS",
                        color: "#b97a28",
                        dashed: true,
                        x: &log_frequency,
                        y: &baseline_response.4,
                    },
                    evidence::Series {
                        name: "Shaped GS",
                        color: "#2673b8",
                        dashed: false,
                        x: &log_frequency,
                        y: &shaped_response.3,
                    },
                    evidence::Series {
                        name: "Shaped CS",
                        color: "#27865d",
                        dashed: false,
                        x: &log_frequency,
                        y: &shaped_response.4,
                    },
                ],
            },
        ],
        &[
            evidence::RetainedSeries {
                panel: "Physical response",
                name: "Baseline true speed",
                x: &time,
                y: &speed_traces[0],
                x_unit: "time (s)",
                y_unit: "speed deviation (rpm)",
            },
            evidence::RetainedSeries {
                panel: "Physical response",
                name: "Baseline requested torque",
                x: &time,
                y: &requested_torque_traces[0],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Physical response",
                name: "Baseline applied torque",
                x: &time,
                y: &torque_traces[0],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Physical response",
                name: "Shaped true speed",
                x: &time,
                y: &speed_traces[1],
                x_unit: "time (s)",
                y_unit: "speed deviation (rpm)",
            },
            evidence::RetainedSeries {
                panel: "Physical response",
                name: "Shaped requested torque",
                x: &time,
                y: &requested_torque_traces[1],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
            evidence::RetainedSeries {
                panel: "Physical response",
                name: "Shaped applied torque",
                x: &time,
                y: &torque_traces[1],
                x_unit: "time (s)",
                y_unit: "torque (%)",
            },
        ],
        &summary,
    );
    evidence::retain(
        &evidence_root("motor-sensitivity"),
        "motor-sensitivity-response-evidence",
        "Motor response and control effort",
        "The shaped loop reduces oscillation and quiet torque activity while recovering more slowly from load.",
        &[
            evidence::Panel {
                title: "True motor speed", x_unit: "time (s)", y_unit: "speed deviation (rpm)",
                x_minimum: 0.0, x_maximum: 120.0, y_minimum: 0.0, y_maximum: 1.2,
                series: vec![
                    evidence::Series { name: "Baseline true speed", color: "#b43c32", dashed: true, x: &time, y: &speed_traces[0] },
                    evidence::Series { name: "Shaped true speed", color: "#2673b8", dashed: false, x: &time, y: &speed_traces[1] },
                ],
            },
            evidence::Panel {
                title: "Applied torque", x_unit: "time (s)", y_unit: "torque (%)",
                x_minimum: 0.0, x_maximum: 120.0, y_minimum: -0.5, y_maximum: 0.5,
                series: vec![
                    evidence::Series { name: "Baseline applied torque", color: "#b43c32", dashed: true, x: &time, y: &torque_traces[0] },
                    evidence::Series { name: "Shaped applied torque", color: "#2673b8", dashed: false, x: &time, y: &torque_traces[1] },
                ],
            },
        ],
        &[],
        &summary,
    );
    let _ = std::fs::remove_file(path);
    println!(
        "motor sensitivity overshoot={overshoot_percent:?}%, settling={pre_load_settling:?}s, \
         load departure={post_load_departure:?} rpm, quiet torque travel={quiet_torque_travel:?} pp"
    );
}

#[test]
fn control_curriculum_z_plane_predicts_sampled_axis_failure_and_recovery() {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let preferred = repository
        .join("examples/control/07-classical-stability-and-frequency-design/50-position-z-plane");
    let project = if preferred.is_dir() {
        preferred
    } else {
        repository.join(
            "examples/control/07-classical-stability-and-frequency-design/50-position-z-plane",
        )
    };
    let shipped_source = std::fs::read_to_string(project.join("src/Main.st"))
        .expect("read position z-plane Structured Text");
    let simulation_source = std::fs::read_to_string(project.join("simulation.toml"))
        .expect("read position z-plane simulation");
    assert_eq!(
        simulation_source.matches("[[plants]]").count(),
        3,
        "Lesson 50 requires fine, coarse-baseline, and corrected-coarse axis copies"
    );
    for required in [
        "FineSampleSeconds : LREAL := 0.01",
        "CoarseSampleSeconds : LREAL := 0.1",
        "ContinuousProportionalGain : LREAL := 4.0",
        "ContinuousDerivativeGain : LREAL := 0.4",
        "CorrectedProportionalGain : LREAL := 1.0",
        "CorrectedDerivativeGain : LREAL := 0.4",
    ] {
        assert!(
            shipped_source.contains(required),
            "missing sampled-axis design value: {required}"
        );
    }

    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("sampled-axis program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX96.0 : BOOL :=",
        )
        .replace("FineAborted : BOOL :=", "FineAborted AT %MX97.0 : BOOL :=")
        .replace(
            "CoarseAborted : BOOL :=",
            "CoarseAborted AT %MX97.1 : BOOL :=",
        )
        .replace(
            "CorrectedAborted : BOOL :=",
            "CorrectedAborted AT %MX97.2 : BOOL :=",
        )
        .replace(
            "FineRequestedForce : LREAL :=",
            "FineRequestedForce AT %ML0 : LREAL :=",
        )
        .replace(
            "FinePendingForce : LREAL :=",
            "FinePendingForce AT %ML8 : LREAL :=",
        )
        .replace(
            "CoarseRequestedForce : LREAL :=",
            "CoarseRequestedForce AT %ML16 : LREAL :=",
        )
        .replace(
            "CoarsePendingForce : LREAL :=",
            "CoarsePendingForce AT %ML24 : LREAL :=",
        )
        .replace(
            "CorrectedRequestedForce : LREAL :=",
            "CorrectedRequestedForce AT %ML32 : LREAL :=",
        )
        .replace(
            "CorrectedPendingForce : LREAL :=",
            "CorrectedPendingForce AT %ML40 : LREAL :=",
        )
        .replace(
            "FineAppliedForce : LREAL :=",
            "FineAppliedForce AT %QL0 : LREAL :=",
        )
        .replace(
            "CoarseAppliedForce : LREAL :=",
            "CoarseAppliedForce AT %QL8 : LREAL :=",
        )
        .replace(
            "CorrectedAppliedForce : LREAL :=",
            "CorrectedAppliedForce AT %QL16 : LREAL :=",
        )
        .replace("FinePosition : LREAL :=", "FinePosition AT %IL0 : LREAL :=")
        .replace("FineVelocity : LREAL :=", "FineVelocity AT %IL8 : LREAL :=")
        .replace(
            "CoarsePosition : LREAL :=",
            "CoarsePosition AT %IL16 : LREAL :=",
        )
        .replace(
            "CoarseVelocity : LREAL :=",
            "CoarseVelocity AT %IL24 : LREAL :=",
        )
        .replace(
            "CorrectedPosition : LREAL :=",
            "CorrectedPosition AT %IL32 : LREAL :=",
        )
        .replace(
            "CorrectedVelocity : LREAL :=",
            "CorrectedVelocity AT %IL40 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "position-z-plane/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile three sampled positioning-axis paths");
    let path = temporary_config_path("position-z-plane-three-cases");
    std::fs::write(&path, simulation_source).expect("write sampled-axis simulation configuration");
    let mut plants = SimulationController::new(
        SimulationConfig::load(&path).expect("load three equivalent sampled axes"),
    );
    let enabled = IoAddress::parse("%MX96.0").unwrap();
    let positions = ["%IL0", "%IL16", "%IL32"].map(|address| IoAddress::parse(address).unwrap());
    let velocities = ["%IL8", "%IL24", "%IL40"].map(|address| IoAddress::parse(address).unwrap());
    let requested_forces =
        ["%ML0", "%ML16", "%ML32"].map(|address| IoAddress::parse(address).unwrap());
    let pending_forces =
        ["%ML8", "%ML24", "%ML40"].map(|address| IoAddress::parse(address).unwrap());
    let forces = ["%QL0", "%QL8", "%QL16"].map(|address| IoAddress::parse(address).unwrap());
    let aborts =
        ["%MX97.0", "%MX97.1", "%MX97.2"].map(|address| IoAddress::parse(address).unwrap());
    plants
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &enabled, true);
    let mut position_traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(1_000));
    let mut velocity_traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(1_000));
    let mut requested_force_traces: [Vec<f64>; 3] =
        std::array::from_fn(|_| Vec::with_capacity(1_000));
    let mut pending_force_traces: [Vec<f64>; 3] =
        std::array::from_fn(|_| Vec::with_capacity(1_000));
    let mut force_traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(1_000));
    for cycle in 1..=1_000 {
        runtime
            .execute_cycle()
            .expect("execute sampled positioning controllers");
        let now = Duration::from_millis((cycle * 10) as i64);
        plants
            .apply_post_cycle(now, &runtime)
            .expect("advance positioning plants");
        plants
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish positioning states");
        for index in 0..3 {
            position_traces[index].push(read_lreal(&runtime, &positions[index]));
            velocity_traces[index].push(read_lreal(&runtime, &velocities[index]));
            requested_force_traces[index].push(read_lreal(&runtime, &requested_forces[index]));
            pending_force_traces[index].push(read_lreal(&runtime, &pending_forces[index]));
            force_traces[index].push(read_lreal(&runtime, &forces[index]));
        }
    }

    let pole_sets = [
        cubic_roots(
            -1.9801986733067553,
            1.0217880619509028,
            -0.03762912037341998,
        ),
        cubic_roots(
            -1.8187307530779817,
            1.3685767779398233,
            -0.18730753077981862,
        ),
        cubic_roots(-1.8187307530779817, 1.2280961296169728, -0.3187307530779818),
    ];
    let maximum_magnitudes = pole_sets.map(|roots| {
        roots
            .iter()
            .map(|root| root.re.hypot(root.im))
            .fold(0.0, f64::max)
    });
    assert!((maximum_magnitudes[0] - 0.971845).abs() < 1.0e-5);
    assert!((maximum_magnitudes[1] - 1.041260).abs() < 1.0e-5);
    assert!((maximum_magnitudes[2] - 0.743613).abs() < 1.0e-5);

    let sampling_zero = |sample_seconds: f64| {
        let decay = (-2.0 * sample_seconds).exp();
        let position_input = 5.0 * sample_seconds - 2.5 * (1.0 - decay);
        let velocity_input = 5.0 * (1.0 - decay);
        -(((1.0 - decay) / 2.0) * velocity_input - decay * position_input) / position_input
    };
    assert!((sampling_zero(0.01) + 0.993356).abs() < 1.0e-5);
    assert!((sampling_zero(0.1) + 0.935525).abs() < 1.0e-5);
    let coarse_alias = 2.0 * std::f64::consts::PI / 0.1 - 40.0;
    assert!((coarse_alias - 22.831853).abs() < 1.0e-6);

    assert!(
        !read_bool(&runtime, &aborts[0]),
        "fine sampled axis must remain inside its range"
    );
    assert!(
        read_bool(&runtime, &aborts[1]),
        "unstable coarse baseline must reach its bounded stop"
    );
    assert!(
        !read_bool(&runtime, &aborts[2]),
        "corrected coarse axis must remain inside its range"
    );
    for index in [0, 2] {
        let final_error = position_traces[index][900..]
            .iter()
            .map(|value| (1.0 - value).abs())
            .sum::<f64>()
            / 100.0;
        assert!(
            final_error < 0.02,
            "qualified axis {index} final-window error={final_error}"
        );
    }
    assert!(force_traces
        .iter()
        .flatten()
        .all(|value| value.is_finite() && value.abs() <= 10.0));
    assert!(
        position_traces[1].iter().all(|value| value.abs() <= 5.5),
        "controlled stop failed to bound the coarse axis"
    );
    for forbidden in [
        "PoleMagnitude",
        "SamplingZero",
        "StableLabel",
        "AcceptedCandidate",
        "AliasFrequency",
    ] {
        assert!(
            !shipped_source.contains(forbidden),
            "ST embeds z-plane answer '{forbidden}'"
        );
    }
    let time = time_axis(position_traces[0].len(), 0.01);
    let unit_angle = (0..=100)
        .map(|index| index as f64 * 2.0 * std::f64::consts::PI / 100.0)
        .collect::<Vec<_>>();
    let unit_x = unit_angle
        .iter()
        .map(|angle| angle.cos())
        .collect::<Vec<_>>();
    let unit_y = unit_angle
        .iter()
        .map(|angle| angle.sin())
        .collect::<Vec<_>>();
    let pole_x = pole_sets.map(|set| set.map(|root| root.re));
    let pole_y = pole_sets.map(|set| set.map(|root| root.im));
    let locus_gains = (1..=80)
        .map(|index| index as f64 * 0.05)
        .collect::<Vec<_>>();
    let locus_roots = locus_gains
        .iter()
        .map(|gain| {
            cubic_roots(
                -1.8187307530779817,
                1.1812692518426893 + 0.0468268777742835 * gain,
                -0.3625384938440362 + 0.0438077407660544 * gain,
            )
        })
        .collect::<Vec<_>>();
    let locus_branch_x: [Vec<f64>; 3] =
        std::array::from_fn(|branch| locus_roots.iter().map(|roots| roots[branch].re).collect());
    let locus_branch_y: [Vec<f64>; 3] =
        std::array::from_fn(|branch| locus_roots.iter().map(|roots| roots[branch].im).collect());
    let final_errors = position_traces.each_ref().map(|trace| {
        trace[900..]
            .iter()
            .map(|value| (1.0 - value).abs())
            .sum::<f64>()
            / 100.0
    });
    let abort_values = aborts
        .each_ref()
        .map(|address| read_bool(&runtime, address));
    let overshoot_percent = position_traces.each_ref().map(|trace| {
        ((trace.iter().copied().fold(f64::NEG_INFINITY, f64::max) - 1.0) * 100.0).max(0.0)
    });
    let settling = position_traces
        .each_ref()
        .map(|trace| settling_time(trace, 1.0, 0.02, 0.01));
    let requested_travel = requested_force_traces
        .each_ref()
        .map(|trace| total_variation(trace));
    let applied_travel = force_traces.each_ref().map(|trace| total_variation(trace));
    let saturation_duration = force_traces.each_ref().map(|trace| {
        trace
            .iter()
            .filter(|value| value.abs() >= 10.0 - 1.0e-9)
            .count() as f64
            * 0.01
    });
    let controlled_stop = std::array::from_fn::<_, 3, _>(|index| {
        !abort_values[index]
            || velocity_traces[index]
                .last()
                .is_some_and(|value| value.abs() < 0.1)
    });
    let sampled = |sample_seconds: f64| {
        let decay = (-2.0 * sample_seconds).exp();
        let q = (1.0 - decay) / 2.0;
        let bp = 5.0 * sample_seconds - 2.5 * (1.0 - decay);
        let bv = 5.0 * (1.0 - decay);
        (decay, q, bp, bv, q * bv - decay * bp)
    };
    let sampled_values = [sampled(0.01), sampled(0.1), sampled(0.1)];
    let pole_text = pole_sets.map(|roots| {
        format!(
            "{:.6}{:+.6}j;{:.6}{:+.6}j;{:.6}{:+.6}j",
            roots[0].re, roots[0].im, roots[1].re, roots[1].im, roots[2].re, roots[2].im
        )
    });
    let pole_magnitude_text = pole_sets.map(|roots| {
        format!(
            "{:.6};{:.6};{:.6}",
            roots[0].magnitude(),
            roots[1].magnitude(),
            roots[2].magnitude()
        )
    });
    let summary = format!(
        "case,continuous_model_revision,discrete_model_revision,controller_revision,plant_sample_s,controller_sample_s,hold_convention,computation_delay_samples,ad,bd,sampled_numerator,sampled_denominator,poles,pole_magnitudes,maximum_pole_magnitude,unit_circle_clearance,sampling_zero,nyquist_rad_s,aliased_frequency_rad_s,overshoot_percent,settling_s,final_error_mm,requested_force_travel_pp,applied_force_travel_pp,saturation_duration_s,aborted,abort_trigger,controlled_stop_complete,stop_reason,qualified,seed,applied,evidence_class\n{}",
        (0..3).map(|index| {
            let sample_seconds = if index == 0 { 0.01 } else { 0.1 };
            let (decay, q, bp, bv, numerator_constant) = sampled_values[index];
            format!(
                "{},mass-damper-v1,zoh-exact-v1,{},0.01,{:.2},zero_order_hold,1,1;{:.9};0;{:.9},{:.9};{:.9},{:.9};{:.9},1;{:.9};{:.9},{},{},{:.9},{:.9},{:.9},{:.9},{},{:.9},{:.9},{:.9},{:.9},{},{},{},{},{},{},{},20260829,false,simulation\n",
                ["fine", "coarse_unchanged", "coarse_corrected"][index],
                ["pd-kp4-kd0.4", "pd-kp4-kd0.4", "pd-kp1-kd0.4"][index],
                sample_seconds, q, decay, bp, bv, bp, numerator_constant, -(1.0 + decay), decay,
                pole_text[index], pole_magnitude_text[index], maximum_magnitudes[index],
                1.0 - maximum_magnitudes[index], sampling_zero(sample_seconds),
                std::f64::consts::PI / sample_seconds,
                if index == 0 { 40.0 } else { coarse_alias }, overshoot_percent[index],
                settling[index].map_or_else(|| "not_settled".to_owned(), |value| format!("{value:.6}")),
                final_errors[index], requested_travel[index], applied_travel[index], saturation_duration[index],
                abort_values[index], if abort_values[index] { "force_or_position_boundary" } else { "none" },
                controlled_stop[index], if abort_values[index] { "bounded_stop" } else { "duration" },
                !abort_values[index],
            )
        }).collect::<String>(),
    );
    evidence::retain(
        &evidence_root("position-z-plane"),
        "position-z-plane-evidence",
        "Sampled-axis poles and measured position",
        "The same continuous gains fail at 100 ms; a discrete redesign restores stability.",
        &[
            evidence::Panel {
                title: "Discrete poles",
                x_unit: "real z",
                y_unit: "imaginary z",
                x_minimum: -1.2,
                x_maximum: 1.2,
                y_minimum: -1.2,
                y_maximum: 1.2,
                series: vec![
                    evidence::Series {
                        name: "Unit circle",
                        color: "#526078",
                        dashed: true,
                        x: &unit_x,
                        y: &unit_y,
                    },
                    evidence::Series {
                        name: "Corrected root-locus branch 1",
                        color: "#2673b8",
                        dashed: false,
                        x: &locus_branch_x[0],
                        y: &locus_branch_y[0],
                    },
                    evidence::Series {
                        name: "Corrected root-locus branch 2",
                        color: "#b97a28",
                        dashed: false,
                        x: &locus_branch_x[1],
                        y: &locus_branch_y[1],
                    },
                    evidence::Series {
                        name: "Corrected root-locus branch 3",
                        color: "#27865d",
                        dashed: false,
                        x: &locus_branch_x[2],
                        y: &locus_branch_y[2],
                    },
                    evidence::Series {
                        name: "10 ms selected poles",
                        color: "#2673b8",
                        dashed: false,
                        x: &pole_x[0],
                        y: &pole_y[0],
                    },
                    evidence::Series {
                        name: "100 ms unchanged selected poles",
                        color: "#b43c32",
                        dashed: false,
                        x: &pole_x[1],
                        y: &pole_y[1],
                    },
                    evidence::Series {
                        name: "100 ms corrected selected poles",
                        color: "#27865d",
                        dashed: false,
                        x: &pole_x[2],
                        y: &pole_y[2],
                    },
                ],
            },
            evidence::Panel {
                title: "Position response",
                x_unit: "time (s)",
                y_unit: "position (mm)",
                x_minimum: 0.0,
                x_maximum: 10.0,
                y_minimum: -0.5,
                y_maximum: 5.5,
                series: vec![
                    evidence::Series {
                        name: "10 ms position",
                        color: "#2673b8",
                        dashed: false,
                        x: &time,
                        y: &position_traces[0],
                    },
                    evidence::Series {
                        name: "100 ms unchanged position",
                        color: "#b43c32",
                        dashed: true,
                        x: &time,
                        y: &position_traces[1],
                    },
                    evidence::Series {
                        name: "100 ms corrected position",
                        color: "#27865d",
                        dashed: false,
                        x: &time,
                        y: &position_traces[2],
                    },
                ],
            },
        ],
        &[],
        &summary,
    );
    evidence::retain(
        &evidence_root("position-z-plane"),
        "position-z-plane-force-evidence",
        "Sampled force request, delay and application",
        "Each controller applies the previous request, making the one-sample computation delay visible.",
        &[
            evidence::Panel {
                title: "Fine and corrected force", x_unit: "time (s)", y_unit: "force (%)",
                x_minimum: 0.0, x_maximum: 10.0, y_minimum: -1.0, y_maximum: 4.5,
                series: vec![
                    evidence::Series { name: "10 ms requested", color: "#2673b8", dashed: true, x: &time, y: &requested_force_traces[0] },
                    evidence::Series { name: "10 ms pending", color: "#6b7280", dashed: true, x: &time, y: &pending_force_traces[0] },
                    evidence::Series { name: "10 ms applied", color: "#27865d", dashed: false, x: &time, y: &force_traces[0] },
                    evidence::Series { name: "100 ms corrected requested", color: "#b97a28", dashed: true, x: &time, y: &requested_force_traces[2] },
                    evidence::Series { name: "100 ms corrected pending", color: "#6b7280", dashed: true, x: &time, y: &pending_force_traces[2] },
                    evidence::Series { name: "100 ms corrected applied", color: "#7c3aed", dashed: false, x: &time, y: &force_traces[2] },
                ],
            },
            evidence::Panel {
                title: "Unchanged coarse force", x_unit: "time (s)", y_unit: "force (%)",
                x_minimum: 0.0, x_maximum: 10.0, y_minimum: -10.0, y_maximum: 10.0,
                series: vec![
                    evidence::Series { name: "100 ms unchanged requested", color: "#b43c32", dashed: true, x: &time, y: &requested_force_traces[1] },
                    evidence::Series { name: "100 ms unchanged pending", color: "#6b7280", dashed: true, x: &time, y: &pending_force_traces[1] },
                    evidence::Series { name: "100 ms unchanged applied", color: "#111827", dashed: false, x: &time, y: &force_traces[1] },
                ],
            },
        ],
        &[],
        &summary,
    );
    let _ = std::fs::remove_file(path);
    println!(
        "sampled-axis pole magnitudes={maximum_magnitudes:?}, zeros=[{}, {}], aborts=[{}, {}, {}]",
        sampling_zero(0.01),
        sampling_zero(0.1),
        read_bool(&runtime, &aborts[0]),
        read_bool(&runtime, &aborts[1]),
        read_bool(&runtime, &aborts[2])
    );
}

#[test]
fn control_curriculum_classical_analysis_matches_declared_oracles() {
    for (name, source, simulation) in PROJECTS {
        CompileSession::from_sources(vec![
            SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
            SourceFile::with_path(format!("{name}/src/Main.st"), source),
        ])
        .label_errors(true)
        .build_runtime()
        .unwrap_or_else(|error| panic!("compile classical lesson {name}: {error}"));
        let path = temporary_config_path(name);
        std::fs::write(&path, simulation).expect("write project simulation configuration");
        SimulationConfig::load(&path)
            .unwrap_or_else(|error| panic!("load classical lesson {name}: {error}"));
        let _ = std::fs::remove_file(path);
    }

    // S + T = 1 for the declared real loop-gain examples.
    for loop_gain in [9.0_f64, 0.01] {
        let sensitivity = 1.0 / (1.0 + loop_gain);
        let complementary = loop_gain / (1.0 + loop_gain);
        assert!((sensitivity + complementary - 1.0).abs() < 1.0e-12);
    }

    // For s^3 + 2s^2 + 3s + K, the cubic Routh row is (6-K)/2.
    for (gain, expected_stable) in [(4.0_f64, true), (8.0, false)] {
        let routh_row = (6.0 - gain) / 2.0;
        assert_eq!(routh_row > 0.0, expected_stable);
    }

    // The uncompensated servo poles at K=4 are -1 ± j*sqrt(3).
    let real = -1.0_f64;
    let imaginary = 3.0_f64.sqrt();
    let natural_frequency = real.hypot(imaginary);
    let damping = -real / natural_frequency;
    assert!((natural_frequency - 2.0).abs() < 1.0e-12);
    assert!((damping - 0.5).abs() < 1.0e-12);

    // At resonance, G(j2)=1/(j0.4), so magnitude is 2.5.
    let resonance_magnitude = 1.0_f64 / 0.4;
    assert!((resonance_magnitude - 2.5).abs() < 1.0e-12);
}
