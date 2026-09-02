use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

#[path = "control_curriculum_classical/evidence.rs"]
mod evidence;

fn evidence_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/11-state-space-estimation-and-predictive-control/66-weigh-feeder-kalman/figures")
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
    let padding = ((maximum - minimum) * 0.08).max(0.05);
    (minimum - padding, maximum + padding)
}

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/11-state-space-estimation-and-predictive-control/66-weigh-feeder-kalman")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 66 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 66 file {}: {error}", path.display()))
}

fn mapped_source() -> String {
    project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("Lesson 66 program before configuration")
        .replace("ProcessNoiseVariance : LREAL :=", "ProcessNoiseVariance AT %ML0 : LREAL :=")
        .replace(
            "MeasurementNoiseVariance : LREAL :=",
            "MeasurementNoiseVariance AT %ML8 : LREAL :=",
        )
        .replace("BeltCommandRequested : LREAL :=", "BeltCommandRequested AT %ML16 : LREAL :=")
        .replace("PredictedEstimate : LREAL :=", "PredictedEstimate AT %ML24 : LREAL :=")
        .replace(
            "PredictedCovariance : LREAL :=",
            "PredictedCovariance AT %ML32 : LREAL :=",
        )
        .replace("KalmanGain : LREAL :=", "KalmanGain AT %ML40 : LREAL :=")
        // Map longer names before `Estimate`; substring replacement would
        // otherwise assign the generic estimate address to both diagnostics.
        .replace("SimpleEstimate : LREAL :=", "SimpleEstimate AT %ML64 : LREAL :=")
        .replace("SelectedEstimate : LREAL :=", "SelectedEstimate AT %ML72 : LREAL :=")
        .replace("Estimate : LREAL :=", "Estimate AT %ML48 : LREAL :=")
        .replace("Covariance : LREAL :=", "Covariance AT %ML56 : LREAL :=")
        .replace("Innovation : LREAL :=", "Innovation AT %ML80 : LREAL :=")
        .replace(
            "InnovationVariance : LREAL :=",
            "InnovationVariance AT %ML88 : LREAL :=",
        )
        .replace(
            "NormalizedInnovationSquared : LREAL :=",
            "NormalizedInnovationSquared AT %ML96 : LREAL :=",
        )
        .replace("Enabled : BOOL :=", "Enabled AT %MX120.0 : BOOL :=")
        .replace("UseKalman : BOOL :=", "UseKalman AT %MX120.1 : BOOL :=")
        .replace("EstimatorValid : BOOL :=", "EstimatorValid AT %MX120.2 : BOOL :=")
        .replace("BeltCommandApplied : LREAL :=", "BeltCommandApplied AT %QL0 : LREAL :=")
        .replace("MassFlowTruth : LREAL :=", "MassFlowTruth AT %IL0 : LREAL :=")
        .replace(
            "MassFlowMeasurement : LREAL :=",
            "MassFlowMeasurement AT %IL8 : LREAL :=",
        )
}

fn build_runtime() -> Runtime {
    CompileSession::from_sources(vec![SourceFile::with_path(
        "weigh-feeder-kalman/src/Main.st",
        mapped_source(),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped Lesson 66 Structured Text")
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-weigh-feeder-kalman-{label}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn simulation(file: &str) -> SimulationController {
    let path = temporary_config_path(file);
    std::fs::write(&path, project_file(file)).expect("write Lesson 66 simulation config");
    let config = SimulationConfig::load(&path).expect("load Lesson 66 simulation config");
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

fn initialize_estimator_state(runtime: &mut Runtime) {
    // The standalone harness injects direct addresses after removing
    // VAR_CONFIG, so seed the externally mapped state with the lesson's
    // declared operating point before executing its first cycle.
    write_lreal(runtime, "%ML48", 5.0);
    write_lreal(runtime, "%ML56", 1.9);
    write_lreal(runtime, "%ML64", 5.0);
    write_lreal(runtime, "%ML72", 5.0);
}

#[derive(Default)]
struct Trace {
    truth: Vec<f64>,
    measurement: Vec<f64>,
    simple: Vec<f64>,
    kalman: Vec<f64>,
    selected: Vec<f64>,
    covariance: Vec<f64>,
    gain: Vec<f64>,
    innovation: Vec<f64>,
    innovation_variance: Vec<f64>,
    nis: Vec<f64>,
    requested: Vec<f64>,
    applied: Vec<f64>,
}

fn run(file: &str, q: f64, r: f64) -> Trace {
    let mut runtime = build_runtime();
    initialize_estimator_state(&mut runtime);
    let mut plant = simulation(file);
    plant
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial feeder signals");
    let mut trace = Trace::default();
    for cycle in 0..300 {
        let command = if cycle < 50 { 5.0 } else { 10.0 };
        write_bool(&mut runtime, "%MX120.0", true);
        write_bool(&mut runtime, "%MX120.1", true);
        write_lreal(&mut runtime, "%ML0", q);
        write_lreal(&mut runtime, "%ML8", r);
        write_lreal(&mut runtime, "%ML16", command);
        runtime
            .execute_cycle()
            .expect("execute weigh-feeder estimators");
        assert!(
            read_bool(&runtime, "%MX120.2"),
            "valid covariance case rejected"
        );
        trace.truth.push(read_lreal(&runtime, "%IL0"));
        trace.measurement.push(read_lreal(&runtime, "%IL8"));
        trace.simple.push(read_lreal(&runtime, "%ML64"));
        trace.kalman.push(read_lreal(&runtime, "%ML48"));
        trace.selected.push(read_lreal(&runtime, "%ML72"));
        trace.covariance.push(read_lreal(&runtime, "%ML56"));
        trace.gain.push(read_lreal(&runtime, "%ML40"));
        trace.innovation.push(read_lreal(&runtime, "%ML80"));
        trace
            .innovation_variance
            .push(read_lreal(&runtime, "%ML88"));
        trace.nis.push(read_lreal(&runtime, "%ML96"));
        trace.requested.push(read_lreal(&runtime, "%ML16"));
        trace.applied.push(read_lreal(&runtime, "%QL0"));
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        plant
            .apply_post_cycle(now, &runtime)
            .expect("advance feeder copies");
        plant
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish feeder signals");
    }
    write_bool(&mut runtime, "%MX120.0", false);
    runtime.execute_cycle().expect("disable feeder command");
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0, "disabled belt command");
    trace
}

fn mean(values: impl Iterator<Item = f64>, count: usize) -> f64 {
    values.sum::<f64>() / count as f64
}

fn rms_error(estimate: &[f64], truth: &[f64], range: std::ops::Range<usize>) -> f64 {
    (range
        .clone()
        .map(|index| (estimate[index] - truth[index]).powi(2))
        .sum::<f64>()
        / range.len() as f64)
        .sqrt()
}

fn first_window_mean_at_least(
    values: &[f64],
    start: usize,
    window: usize,
    threshold: f64,
) -> Option<usize> {
    (start..=values.len() - window).find(|&candidate| {
        mean(
            values[candidate..candidate + window].iter().copied(),
            window,
        ) >= threshold
    })
}

fn total_variation(values: &[f64], range: std::ops::Range<usize>) -> f64 {
    values[range]
        .windows(2)
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum()
}

fn lag_one_correlation(values: &[f64], range: std::ops::Range<usize>) -> f64 {
    let current = &values[range.clone()];
    let left = &current[..current.len() - 1];
    let right = &current[1..];
    let left_mean = mean(left.iter().copied(), left.len());
    let right_mean = mean(right.iter().copied(), right.len());
    let covariance = left
        .iter()
        .zip(right)
        .map(|(left, right)| (left - left_mean) * (right - right_mean))
        .sum::<f64>();
    let left_energy = left
        .iter()
        .map(|value| (value - left_mean).powi(2))
        .sum::<f64>();
    let right_energy = right
        .iter()
        .map(|value| (value - right_mean).powi(2))
        .sum::<f64>();
    covariance / (left_energy * right_energy).sqrt()
}

#[test]
fn control_curriculum_weigh_feeder_kalman_qualifies_covariance_and_innovation() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 66 descriptor");
    assert_eq!(lesson["schemaVersion"], 4, "Lesson 66 must use schema 4");
    assert_eq!(lesson["sequence"], 66);
    assert_eq!(
        lesson["prerequisites"][0],
        "control-magnetic-bearing-state-feedback"
    );
    assert_eq!(lesson["nextLesson"], "control-suspension-lqr");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_weigh_feeder_kalman_qualifies_covariance_and_innovation"
    );
    let source = project_file("src/Main.st");
    assert_eq!(source.matches("MassFlowTruth").count(), 2);
    for forbidden in ["AcceptedFilter", "RmsError", "InnovationMean", "RANDOM"] {
        assert!(
            !source.contains(forbidden),
            "ST embeds result '{forbidden}'"
        );
    }

    let a = (-0.1_f64).exp();
    let b = 1.0 - a;
    let predicted_covariance = a * a * 1.9 + 0.1;
    let gain = predicted_covariance / (predicted_covariance + 3.0);
    let corrected_covariance = (1.0 - gain) * predicted_covariance;
    assert!((a - 0.904_837_418_035_959_5).abs() <= 1e-12);
    assert!((b - 0.095_162_581_964_040_48).abs() <= 1e-12);
    assert!((predicted_covariance - 1.655_588_430_848_165_2).abs() <= 1e-12);
    assert!((gain - 0.355_613_142_235_287).abs() <= 1e-12);
    assert!((corrected_covariance - 1.066_839_426_705_861).abs() <= 1e-12);
    assert!((gain * 5.0 - 1.778_065_711_176_435).abs() <= 1e-12);

    let mut worked = build_runtime();
    initialize_estimator_state(&mut worked);
    write_bool(&mut worked, "%MX120.0", true);
    write_bool(&mut worked, "%MX120.1", true);
    write_lreal(&mut worked, "%ML0", 0.1);
    write_lreal(&mut worked, "%ML8", 3.0);
    write_lreal(&mut worked, "%ML16", 5.0);
    write_lreal(&mut worked, "%IL8", 10.0);
    worked
        .execute_cycle()
        .expect("execute first Kalman correction");
    assert!((read_lreal(&worked, "%ML24") - 5.0).abs() <= 1e-10);
    assert!((read_lreal(&worked, "%ML32") - predicted_covariance).abs() <= 1e-10);
    assert!((read_lreal(&worked, "%ML40") - gain).abs() <= 1e-10);
    assert!((read_lreal(&worked, "%ML48") - (5.0 + gain * 5.0)).abs() <= 1e-10);
    assert!((read_lreal(&worked, "%ML56") - corrected_covariance).abs() <= 1e-10);

    let nominal = run("simulation.toml", 0.1, 3.0);
    let overtrust = run("simulation.toml", 0.1, 0.25);
    let distrust = run("simulation.toml", 0.1, 12.0);
    let bias = run("simulation-load-cell-bias.toml", 0.1, 3.0);
    for trace in [&nominal, &overtrust, &distrust, &bias] {
        assert!(trace
            .truth
            .iter()
            .chain(&trace.measurement)
            .chain(&trace.simple)
            .chain(&trace.kalman)
            .chain(&trace.selected)
            .chain(&trace.covariance)
            .chain(&trace.gain)
            .chain(&trace.innovation)
            .chain(&trace.innovation_variance)
            .chain(&trace.nis)
            .chain(&trace.requested)
            .chain(&trace.applied)
            .all(|value| value.is_finite()));
        assert!(trace.truth.iter().all(|value| (0.0..=20.0).contains(value)));
        assert!(trace
            .applied
            .iter()
            .all(|value| (0.0..=20.0).contains(value)));
        assert!(trace
            .requested
            .iter()
            .zip(&trace.applied)
            .all(|(requested, applied)| (requested - applied).abs() <= f64::EPSILON));
        assert!(trace.covariance.iter().all(|value| *value >= 0.0));
        assert!(trace.innovation_variance.iter().all(|value| *value > 0.0));
    }

    let window = 50..300;
    let simple_rms = rms_error(&nominal.simple, &nominal.truth, window.clone());
    let nominal_rms = rms_error(&nominal.kalman, &nominal.truth, window.clone());
    let overtrust_rms = rms_error(&overtrust.kalman, &overtrust.truth, window.clone());
    let distrust_rms = rms_error(&distrust.kalman, &distrust.truth, window.clone());
    assert!(nominal_rms < simple_rms);
    let nominal_response = first_window_mean_at_least(&nominal.kalman, 50, 20, 9.5)
        .expect("nominal Kalman estimate reaches the mean-response threshold");
    let simple_response = first_window_mean_at_least(&nominal.simple, 50, 20, 9.5)
        .expect("fixed estimate reaches the mean-response threshold");
    let distrust_response = first_window_mean_at_least(&distrust.kalman, 50, 20, 9.5)
        .expect("sensor-distrust estimate reaches the mean-response threshold");
    let overtrust_response = first_window_mean_at_least(&overtrust.kalman, 50, 20, 9.5)
        .expect("sensor-overtrust estimate reaches the mean-response threshold");
    assert!(nominal_response <= simple_response);
    assert!(distrust_response >= nominal_response);
    let nominal_gain = mean(nominal.gain[window.clone()].iter().copied(), window.len());
    let overtrust_gain = mean(overtrust.gain[window.clone()].iter().copied(), window.len());
    let distrust_gain = mean(distrust.gain[window.clone()].iter().copied(), window.len());
    assert!(overtrust_gain > nominal_gain);
    assert!(distrust_gain < nominal_gain);
    let nominal_movement = total_variation(&nominal.kalman, window.clone());
    let overtrust_movement = total_variation(&overtrust.kalman, window.clone());
    let simple_movement = total_variation(&nominal.simple, window.clone());
    let distrust_movement = total_variation(&distrust.kalman, window.clone());
    assert!(overtrust_movement >= 1.2 * nominal_movement);

    let innovation_mean = mean(
        nominal.innovation[window.clone()].iter().copied(),
        window.len(),
    );
    let innovation_rms = (mean(
        nominal.innovation[window.clone()]
            .iter()
            .map(|value| value * value),
        window.len(),
    ))
    .sqrt();
    let nis_mean = mean(nominal.nis[window.clone()].iter().copied(), window.len());
    let innovation_correlation = lag_one_correlation(&nominal.innovation, window.clone());
    assert!(innovation_mean.abs() <= 0.25);
    assert!((0.4..=1.6).contains(&nis_mean));
    assert!(innovation_correlation.abs() <= 0.30);
    let bias_innovation_mean = mean(
        bias.innovation[window.clone()].iter().copied(),
        window.len(),
    );
    let bias_nis_mean = mean(bias.nis[window.clone()].iter().copied(), window.len());
    let bias_final = mean(
        (250..300).map(|index| (bias.kalman[index] - bias.truth[index]).abs()),
        50,
    );
    let paired_bias_shift = mean(
        (250..300).map(|index| bias.kalman[index] - nominal.kalman[index]),
        50,
    );
    assert!(bias_innovation_mean.abs() >= 0.20 || bias_nis_mean > 1.6);
    assert!(
        bias_final >= 0.50,
        "bias final error was {bias_final:.6}; paired shift was {paired_bias_shift:.6}; innovation mean/NIS were {bias_innovation_mean:.6}/{bias_nis_mean:.6}"
    );

    let mut invalid = build_runtime();
    write_bool(&mut invalid, "%MX120.0", true);
    write_bool(&mut invalid, "%MX120.1", true);
    write_lreal(&mut invalid, "%ML0", -0.1);
    write_lreal(&mut invalid, "%ML8", 3.0);
    write_lreal(&mut invalid, "%ML16", 5.0);
    write_lreal(&mut invalid, "%IL8", 5.0);
    invalid
        .execute_cycle()
        .expect("execute invalid covariance fallback");
    assert!(!read_bool(&invalid, "%MX120.2"));
    assert_eq!(read_lreal(&invalid, "%ML72"), read_lreal(&invalid, "%ML64"));
    for address in [
        "%ML24", "%ML32", "%ML40", "%ML48", "%ML56", "%ML64", "%ML72",
    ] {
        assert!(
            read_lreal(&invalid, address).is_finite(),
            "finite fallback {address}"
        );
    }

    eprintln!(
        "weigh-feeder Kalman: RMS simple/nominal/overtrust/distrust={simple_rms:.4}/{nominal_rms:.4}/{overtrust_rms:.4}/{distrust_rms:.4}kg/s, 2 s mean 90% response simple/nominal/overtrust/distrust={:.1}/{:.1}/{:.1}/{:.1}s, gains nominal/overtrust/distrust={nominal_gain:.4}/{overtrust_gain:.4}/{distrust_gain:.4}, movement simple/nominal/overtrust/distrust={simple_movement:.3}/{nominal_movement:.3}/{overtrust_movement:.3}/{distrust_movement:.3}kg/s, innovation mean/RMS/NIS/lag1={innovation_mean:.4}/{innovation_rms:.4}/{nis_mean:.4}/{innovation_correlation:.4}, bias innovation/NIS/final/paired-shift={bias_innovation_mean:.4}/{bias_nis_mean:.4}/{bias_final:.4}/{paired_bias_shift:.4}",
        (simple_response - 50) as f64 * 0.1,
        (nominal_response - 50) as f64 * 0.1,
        (overtrust_response - 50) as f64 * 0.1,
        (distrust_response - 50) as f64 * 0.1,
    );

    let time = (0..nominal.truth.len())
        .map(|sample| sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let (flow_min, flow_max) = plot_bounds([
        nominal.truth.as_slice(),
        nominal.measurement.as_slice(),
        nominal.simple.as_slice(),
        nominal.kalman.as_slice(),
    ]);
    let (innovation_min, innovation_max) =
        plot_bounds([nominal.innovation.as_slice(), bias.innovation.as_slice()]);
    let summary = format!(
        "case,status,rms_error_kg_s,response_s,mean_gain,estimate_movement_kg_s,innovation_mean_kg_s,innovation_rms_kg_s,nis_mean,lag1\nfixed_filter,baseline,{:.6},{:.3},,{:.6},,,,\nkalman_nominal,qualified,{:.6},{:.3},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}\nsensor_overtrust,diagnostic_tradeoff,{:.6},{:.3},{:.6},{:.6},,,,\nsensor_distrust,diagnostic_tradeoff,{:.6},{:.3},{:.6},{:.6},,,,\nload_cell_bias,rejected_bias_model,,,,,{:.6},,{:.6},\ninvalid_covariance,fixed_fallback,,,,,,,,\n",
        simple_rms,
        (simple_response - 50) as f64 * 0.1,
        simple_movement,
        nominal_rms,
        (nominal_response - 50) as f64 * 0.1,
        nominal_gain,
        nominal_movement,
        innovation_mean,
        innovation_rms,
        nis_mean,
        innovation_correlation,
        overtrust_rms,
        (overtrust_response - 50) as f64 * 0.1,
        overtrust_gain,
        overtrust_movement,
        distrust_rms,
        (distrust_response - 50) as f64 * 0.1,
        distrust_gain,
        distrust_movement,
        bias_innovation_mean,
        bias_nis_mean,
    );
    evidence::retain(
        &evidence_root(),
        "weigh-feeder-kalman-evidence",
        "Covariance choices change trust and diagnostic behavior",
        "The nominal Kalman estimate balances noisy load-cell measurements against the feeder model; persistent innovation reveals bias.",
        &[
            evidence::Panel {
                title: "Mass-flow estimate",
                x_unit: "time (s)",
                y_unit: "mass flow (kg/s)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: flow_min,
                y_maximum: flow_max,
                series: vec![
                    evidence::Series { name: "Simulation truth", color: "#111827", dashed: false, x: &time, y: &nominal.truth },
                    evidence::Series { name: "Load cell", color: "#9ca3af", dashed: false, x: &time, y: &nominal.measurement },
                    evidence::Series { name: "Fixed filter", color: "#b45309", dashed: false, x: &time, y: &nominal.simple },
                    evidence::Series { name: "Kalman", color: "#2563eb", dashed: false, x: &time, y: &nominal.kalman },
                ],
            },
            evidence::Panel {
                title: "Innovation diagnosis",
                x_unit: "time (s)",
                y_unit: "innovation (kg/s)",
                x_minimum: 0.0,
                x_maximum: *time.last().unwrap(),
                y_minimum: innovation_min,
                y_maximum: innovation_max,
                series: vec![
                    evidence::Series { name: "Nominal", color: "#2563eb", dashed: false, x: &time, y: &nominal.innovation },
                    evidence::Series { name: "Biased load cell", color: "#dc2626", dashed: false, x: &time, y: &bias.innovation },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Estimator weighting", name: "Nominal covariance", x: &time, y: &nominal.covariance, x_unit: "time (s)", y_unit: "variance ((kg/s)^2)" },
            evidence::RetainedSeries { panel: "Estimator weighting", name: "Nominal gain", x: &time, y: &nominal.gain, x_unit: "time (s)", y_unit: "gain (ratio)" },
            evidence::RetainedSeries { panel: "Estimator diagnosis", name: "Nominal NIS", x: &time, y: &nominal.nis, x_unit: "time (s)", y_unit: "NIS (ratio)" },
            evidence::RetainedSeries { panel: "Covariance comparison", name: "Overtrust estimate", x: &time, y: &overtrust.kalman, x_unit: "time (s)", y_unit: "mass flow (kg/s)" },
            evidence::RetainedSeries { panel: "Covariance comparison", name: "Distrust estimate", x: &time, y: &distrust.kalman, x_unit: "time (s)", y_unit: "mass flow (kg/s)" },
            evidence::RetainedSeries { panel: "Covariance comparison", name: "Overtrust gain", x: &time, y: &overtrust.gain, x_unit: "time (s)", y_unit: "gain (ratio)" },
            evidence::RetainedSeries { panel: "Covariance comparison", name: "Distrust gain", x: &time, y: &distrust.gain, x_unit: "time (s)", y_unit: "gain (ratio)" },
            evidence::RetainedSeries { panel: "Actuator", name: "Requested belt command", x: &time, y: &nominal.requested, x_unit: "time (s)", y_unit: "mass flow (kg/s)" },
            evidence::RetainedSeries { panel: "Actuator", name: "Applied belt command", x: &time, y: &nominal.applied, x_unit: "time (s)", y_unit: "mass flow (kg/s)" },
        ],
        &summary,
    );
}
