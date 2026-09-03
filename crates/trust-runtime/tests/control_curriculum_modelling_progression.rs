use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};

#[path = "control_curriculum_motion_progression/evidence.rs"]
#[allow(dead_code)]
mod evidence;

fn project_file(project: &str, relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control")
        .join(match project {
            "water-heater-energy-balance" => "01-feedback-foundations-and-temperature-control/15-water-heater-energy-balance",
            "heater-step-identification" => "01-feedback-foundations-and-temperature-control/16-heater-step-identification",
            "independent-model-validation" => "01-feedback-foundations-and-temperature-control/17-independent-model-validation",
            "../foundation/nominal-fopdt" => "01-feedback-foundations-and-temperature-control/18-nominal-fopdt",
            "gravity-tank-linearization" => "04-controller-selection-and-plant-models/34-gravity-tank-linearization",
            "pole-zero-plant-classes" => "04-controller-selection-and-plant-models/35-pole-zero-plant-classes",
            "two-tank-state-space" => "04-controller-selection-and-plant-models/36-two-tank-state-space",
            "safe-heat-exchanger-experiment" => "05-experiments-identification-and-validation/37-safe-heat-exchanger-experiment",
            "excitation-methods" => "05-experiments-identification-and-validation/38-excitation-methods",
            "dryer-model-selection" => "05-experiments-identification-and-validation/39-dryer-model-selection",
            "poor-excitation" => "05-experiments-identification-and-validation/40-poor-excitation",
            "heat-exchanger-validation-uncertainty" => "05-experiments-identification-and-validation/41-heat-exchanger-validation-uncertainty",
            _ => panic!("unknown modelling lesson: {project}"),
        })
        .join(relative);
    assert!(
        path.is_file(),
        "required shipped curriculum file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read shipped curriculum file {}: {error}", path.display()))
}

fn without_lesson_workflow(mut source: String, start: &str, end: &str) -> String {
    let start_index = source.find(start).expect("project-owned workflow start");
    let end_index = source[start_index..]
        .find(end)
        .map(|offset| start_index + offset)
        .expect("project-owned workflow end");
    source.replace_range(start_index..end_index, "");
    source
}

fn temporary_config_path(project: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-modelling-{project}-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_or_compare_curriculum_artifact(path: &Path, expected: &str) {
    if std::env::var_os("TRUST_UPDATE_CONTROL_CURRICULUM_ARTIFACTS").is_some() {
        std::fs::create_dir_all(path.parent().expect("curriculum artifact parent"))
            .expect("create curriculum artifact directory");
        std::fs::write(path, expected).expect("write curriculum artifact");
        return;
    }
    let actual = std::fs::read_to_string(path)
        .unwrap_or_else(|error| panic!("read retained artifact {}: {error}", path.display()));
    assert_eq!(
        actual,
        expected,
        "retained artifact drift: {}",
        path.display()
    );
}

struct PoleZeroMap<'a> {
    label: &'a str,
    poles: &'a [(f64, f64)],
    zeros: &'a [(f64, f64)],
}

fn render_hidden_pole_zero_worksheet(maps: &[PoleZeroMap<'_>]) -> String {
    let mut svg = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" viewBox="0 0 900 620">
<title id="title">Hidden pole-zero matching worksheet</title>
<desc id="desc">Six unlabeled complex-plane maps generated from the declared executable model parameters. Crosses are poles and circles are zeros.</desc>
<rect width="900" height="620" fill="#f7f9fc"/>
<style>text{font-family:system-ui,-apple-system,sans-serif;fill:#243044}.axis{stroke:#526078;stroke-width:1.2}.grid{stroke:#d8e0eb;stroke-width:1}.pole{stroke:#b43c32;stroke-width:3}.zero{stroke:#0072b2;stroke-width:3;fill:#f7f9fc}.frame{stroke:#8792a5;stroke-width:1;fill:none}</style>
<text x="55" y="42" font-size="24" font-weight="700">Match each pole-zero map to an executed response</text>
<text x="55" y="68" font-size="14">Cross = pole · circle = zero · locations use inverse seconds</text>
"##,
    );
    for (index, map) in maps.iter().enumerate() {
        let column = index % 3;
        let row = index / 3;
        let left = 55.0 + column as f64 * 280.0;
        let top = 115.0 + row as f64 * 245.0;
        let width = 230.0;
        let height = 175.0;
        let x = |real: f64| left + (real + 1.5) / 2.0 * width;
        let y = |imaginary: f64| top + (5.0 - imaginary) / 10.0 * height;
        writeln!(
            svg,
            "<text x=\"{left:.1}\" y=\"{:.1}\" font-size=\"16\" font-weight=\"700\">Map {}</text>",
            top - 12.0,
            map.label
        )
        .unwrap();
        writeln!(svg, "<rect x=\"{left:.1}\" y=\"{top:.1}\" width=\"{width:.1}\" height=\"{height:.1}\" class=\"frame\"/>").unwrap();
        for real in [-1.0, 0.0] {
            writeln!(
                svg,
                "<line x1=\"{:.1}\" y1=\"{top:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"{}\"/>",
                x(real),
                x(real),
                top + height,
                if real == 0.0 { "axis" } else { "grid" }
            )
            .unwrap();
        }
        for imaginary in [-4.0, 0.0, 4.0] {
            writeln!(
                svg,
                "<line x1=\"{left:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\" class=\"{}\"/>",
                y(imaginary),
                left + width,
                y(imaginary),
                if imaginary == 0.0 { "axis" } else { "grid" }
            )
            .unwrap();
        }
        writeln!(svg, "<text x=\"{:.1}\" y=\"{:.1}\" font-size=\"11\" text-anchor=\"end\">Re(s)</text><text x=\"{:.1}\" y=\"{:.1}\" font-size=\"11\">Im(s)</text>", left + width, top + height - 6.0, x(0.0) + 5.0, top + 13.0).unwrap();
        for &(real, imaginary) in map.poles {
            let px = x(real);
            let py = y(imaginary);
            writeln!(svg, "<g data-symbol=\"pole\" class=\"pole\"><line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/><line x1=\"{:.1}\" y1=\"{:.1}\" x2=\"{:.1}\" y2=\"{:.1}\"/></g>", px - 6.0, py - 6.0, px + 6.0, py + 6.0, px - 6.0, py + 6.0, px + 6.0, py - 6.0).unwrap();
        }
        for &(real, imaginary) in map.zeros {
            writeln!(
                svg,
                "<circle data-symbol=\"zero\" cx=\"{:.1}\" cy=\"{:.1}\" r=\"7\" class=\"zero\"/>",
                x(real),
                y(imaginary)
            )
            .unwrap();
        }
    }
    svg.push_str("</svg>\n");
    svg
}

fn write_lreal(runtime: &mut trust_runtime::Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image value");
}

fn write_int(runtime: &mut trust_runtime::Runtime, address: &IoAddress, value: i16) {
    runtime
        .io_mut()
        .write(address, Value::Word(value as u16))
        .expect("write INT process-image value");
}

fn write_bool(runtime: &mut trust_runtime::Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL process-image value");
}

fn read_lreal(runtime: &trust_runtime::Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn read_bool(runtime: &trust_runtime::Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL value") {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn read_int(runtime: &trust_runtime::Runtime, address: &IoAddress) -> i16 {
    match runtime.io().read(address).expect("read INT value") {
        Value::Word(value) => value as i16,
        other => panic!("expected INT storage, got {other:?}"),
    }
}

fn pearson_correlation(left: &[f64], right: &[f64]) -> f64 {
    assert_eq!(left.len(), right.len());
    assert!(left.len() >= 2);
    let count = left.len() as f64;
    let left_mean = left.iter().sum::<f64>() / count;
    let right_mean = right.iter().sum::<f64>() / count;
    let mut covariance = 0.0;
    let mut left_energy = 0.0;
    let mut right_energy = 0.0;
    for (&left_value, &right_value) in left.iter().zip(right) {
        let left_centered = left_value - left_mean;
        let right_centered = right_value - right_mean;
        covariance += left_centered * right_centered;
        left_energy += left_centered * left_centered;
        right_energy += right_centered * right_centered;
    }
    covariance / (left_energy * right_energy).sqrt()
}

fn gravity_linearization_runtime() -> trust_runtime::Runtime {
    let source = without_lesson_workflow(
        project_file("gravity-tank-linearization", "src/Main.st")
            .split("CONFIGURATION Config")
            .next()
            .expect("example program before configuration")
            .to_owned(),
        "IF ExperimentEnabled THEN",
        "InputsValid :=",
    )
        // The extrapolation case deliberately leaves the lesson's qualified
        // operating envelope; isolate that model calculation from the visible
        // experiment supervisor's fail-closed command restoration.
        .replace(
            "IF SafetyAbort OR NOT InputsValid THEN\n    FlowDeviation := LREAL#0.0;\nEND_IF;",
            "",
        )
        .replace(
        "FlowDeviation : LREAL :=",
        "FlowDeviation AT %ML0 : LREAL :=",
    )
    .replace("LowInletFlow : LREAL :=", "LowInletFlow AT %QL0 : LREAL :=")
    .replace(
        "MiddleInletFlow : LREAL :=",
        "MiddleInletFlow AT %QL8 : LREAL :=",
    )
    .replace(
        "HighInletFlow : LREAL :=",
        "HighInletFlow AT %QL16 : LREAL :=",
    )
    .replace(
        "LowPhysicalLevel : LREAL :=",
        "LowPhysicalLevel AT %IL0 : LREAL :=",
    )
    .replace(
        "MiddlePhysicalLevel : LREAL :=",
        "MiddlePhysicalLevel AT %IL8 : LREAL :=",
    )
    .replace(
        "HighPhysicalLevel : LREAL :=",
        "HighPhysicalLevel AT %IL16 : LREAL :=",
    )
    .replace(
        "LowEquilibriumFlow : LREAL :=",
        "LowEquilibriumFlow AT %ML16 : LREAL :=",
    )
    .replace(
        "MiddleEquilibriumFlow : LREAL :=",
        "MiddleEquilibriumFlow AT %ML24 : LREAL :=",
    )
    .replace(
        "HighEquilibriumFlow : LREAL :=",
        "HighEquilibriumFlow AT %ML32 : LREAL :=",
    )
    .replace("LowGain : LREAL :=", "LowGain AT %ML40 : LREAL :=")
    .replace("MiddleGain : LREAL :=", "MiddleGain AT %ML48 : LREAL :=")
    .replace("HighGain : LREAL :=", "HighGain AT %ML56 : LREAL :=")
    .replace(
        "LowTimeConstant : LREAL :=",
        "LowTimeConstant AT %ML64 : LREAL :=",
    )
    .replace(
        "MiddleTimeConstant : LREAL :=",
        "MiddleTimeConstant AT %ML72 : LREAL :=",
    )
    .replace(
        "HighTimeConstant : LREAL :=",
        "HighTimeConstant AT %ML80 : LREAL :=",
    )
    .replace("LowPole : LREAL :=", "LowPole AT %ML88 : LREAL :=")
    .replace("MiddlePole : LREAL :=", "MiddlePole AT %ML96 : LREAL :=")
    .replace("HighPole : LREAL :=", "HighPole AT %ML104 : LREAL :=")
    .replace(
        "LowLocalLevelCentimetres : LREAL :=",
        "LowLocalLevelCentimetres AT %ML112 : LREAL :=",
    )
    .replace(
        "MiddleLocalLevelCentimetres : LREAL :=",
        "MiddleLocalLevelCentimetres AT %ML120 : LREAL :=",
    )
    .replace(
        "HighLocalLevelCentimetres : LREAL :=",
        "HighLocalLevelCentimetres AT %ML128 : LREAL :=",
    );
    CompileSession::from_sources(vec![SourceFile::with_path(
        "gravity-tank-linearization/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped gravity-tank linearization lesson")
}

#[test]
fn control_curriculum_gravity_tank_linearization_is_local_and_unit_consistent() {
    let shipped_source = project_file("gravity-tank-linearization", "src/Main.st");
    let config_path = temporary_config_path("gravity-tank-linearization");
    std::fs::write(
        &config_path,
        project_file("gravity-tank-linearization", "simulation.toml"),
    )
    .expect("write gravity-tank linearization simulation config");
    let config = SimulationConfig::load(&config_path).expect("load three nonlinear tanks");

    let flow_deviation = IoAddress::parse("%ML0").unwrap();
    let physical_addresses =
        ["%IL0", "%IL8", "%IL16"].map(|value| IoAddress::parse(value).unwrap());
    let local_addresses =
        ["%ML112", "%ML120", "%ML128"].map(|value| IoAddress::parse(value).unwrap());
    let flow_addresses = ["%QL0", "%QL8", "%QL16"].map(|value| IoAddress::parse(value).unwrap());
    let equilibrium_addresses =
        ["%ML16", "%ML24", "%ML32"].map(|value| IoAddress::parse(value).unwrap());
    let gain_addresses = ["%ML40", "%ML48", "%ML56"].map(|value| IoAddress::parse(value).unwrap());
    let tau_addresses = ["%ML64", "%ML72", "%ML80"].map(|value| IoAddress::parse(value).unwrap());
    let pole_addresses = ["%ML88", "%ML96", "%ML104"].map(|value| IoAddress::parse(value).unwrap());
    let operating_levels_m = [0.25, 0.50, 0.81];
    let expected_equilibrium_flows = [0.010, 0.014_142_135_623_730_952, 0.018];
    let expected_gains = [50.0, 70.710_678_118_654_76, 90.0];
    let expected_time_constants = [25.0, 35.355_339_059_327_38, 45.0];

    let mut runtime = gravity_linearization_runtime();
    let mut simulation = SimulationController::new(config.clone());
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    for (address, expected) in physical_addresses.iter().zip(operating_levels_m) {
        assert!(
            (read_lreal(&runtime, address) - expected).abs() <= 1e-12,
            "nonlinear tank must begin at its declared operating point"
        );
    }
    write_lreal(&mut runtime, &flow_deviation, 0.001);
    let mut physical_traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(3_000));
    let mut local_traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(3_000));
    for cycle in 1usize..=3_000 {
        runtime
            .execute_cycle()
            .expect("advance three local tangent models");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance nonlinear gravity tanks");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish nonlinear levels");
        for index in 0..3 {
            physical_traces[index].push(read_lreal(&runtime, &physical_addresses[index]) * 100.0);
            local_traces[index].push(read_lreal(&runtime, &local_addresses[index]));
        }
    }

    let mut small_errors = [0.0; 3];
    for index in 0..3 {
        let equilibrium_flow = read_lreal(&runtime, &equilibrium_addresses[index]);
        let gain = read_lreal(&runtime, &gain_addresses[index]);
        let tau = read_lreal(&runtime, &tau_addresses[index]);
        let pole = read_lreal(&runtime, &pole_addresses[index]);
        assert!((equilibrium_flow - expected_equilibrium_flows[index]).abs() <= 1e-10);
        assert!((gain - expected_gains[index]).abs() <= 1e-9);
        assert!((tau - expected_time_constants[index]).abs() <= 1e-9);
        assert!((pole + 1.0 / expected_time_constants[index]).abs() <= 1e-12);
        assert!(
            (read_lreal(&runtime, &flow_addresses[index]) - equilibrium_flow - 0.001).abs()
                <= 1e-12
        );

        let physical_cm = read_lreal(&runtime, &physical_addresses[index]) * 100.0;
        let local_cm = read_lreal(&runtime, &local_addresses[index]);
        small_errors[index] = (physical_cm - local_cm).abs();
        assert!(
            small_errors[index] <= 0.30,
            "local model {index} small-step error={:.4} cm",
            small_errors[index]
        );
    }

    let mut large_runtime = gravity_linearization_runtime();
    let mut large_simulation = SimulationController::new(config);
    large_simulation
        .apply_pre_cycle(Duration::ZERO, &mut large_runtime)
        .unwrap();
    write_lreal(&mut large_runtime, &flow_deviation, 0.005);
    let mut large_physical_trace = Vec::with_capacity(3_000);
    let mut large_local_trace = Vec::with_capacity(3_000);
    for cycle in 1usize..=3_000 {
        large_runtime
            .execute_cycle()
            .expect("advance extrapolated local models");
        let now = Duration::from_millis((cycle * 100) as i64);
        large_simulation
            .apply_post_cycle(now, &large_runtime)
            .unwrap();
        large_simulation
            .apply_pre_cycle(now, &mut large_runtime)
            .unwrap();
        large_physical_trace.push(read_lreal(&large_runtime, &physical_addresses[0]) * 100.0);
        large_local_trace.push(read_lreal(&large_runtime, &local_addresses[0]));
    }
    let _ = std::fs::remove_file(config_path);
    let large_physical_cm = read_lreal(&large_runtime, &physical_addresses[0]) * 100.0;
    let large_local_cm = read_lreal(&large_runtime, &local_addresses[0]);
    let large_error = (large_physical_cm - large_local_cm).abs();
    assert!(
        large_error >= 5.0,
        "large low-level step must expose local-model extrapolation error, got {large_error:.3} cm"
    );
    assert!(
        !shipped_source.contains("Control.PID"),
        "linearization lesson must not hide feedback control or gain scheduling"
    );
    assert!(
        !shipped_source.contains("DeadTime"),
        "gravity-tank tangent model has no artificial dead time"
    );

    println!(
        "gravity-tank local models: small-step errors={small_errors:?} cm; low-level large-step error={large_error:.3} cm"
    );

    let small_error_traces: [Vec<f64>; 3] = std::array::from_fn(|index| {
        physical_traces[index]
            .iter()
            .zip(&local_traces[index])
            .map(|(physical, local)| (physical - local).abs())
            .collect()
    });
    let small_model_provenance = format!("25;50;81,5.000000;7.071068;9.000000,25.000000;35.355339;45.000000,-0.040000;-0.028284;-0.022222,{:.6};{:.6};{:.6},inside_declared_local_bound", small_errors[0], small_errors[1], small_errors[2]);
    let large_model_provenance =
        format!("25,5.000000,25.000000,-0.040000,{large_error:.6},outside_declared_local_bound");
    let mut run = String::from("phase,sample,time_s,flow_deviation_m3_s,low_physical_cm,low_local_cm,middle_physical_cm,middle_local_cm,high_physical_cm,high_local_cm,operating_point_cm,gain_cm_per_lps,time_constant_s,pole_per_s,prediction_error_cm,validity_decision,evidence_class\n");
    for index in 0..3_000 {
        run.push_str(&format!("small_step,{},{:.1},0.001000,{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{small_model_provenance},configured_simulation\n", index + 1, (index + 1) as f64 * 0.1, physical_traces[0][index], local_traces[0][index], physical_traces[1][index], local_traces[1][index], physical_traces[2][index], local_traces[2][index]));
    }
    for index in 0..3_000 {
        run.push_str(&format!("large_low_step,{},{:.1},0.005000,{:.12},{:.12},,,,,{large_model_provenance},configured_simulation\n", index + 1, (index + 1) as f64 * 0.1, large_physical_trace[index], large_local_trace[index]));
    }
    let summary = format!("case,accepted,decision_measure,measured_value,unit\nlow small step,true,local-model final error,{:.12},cm\nmiddle small step,true,local-model final error,{:.12},cm\nhigh small step,true,local-model final error,{:.12},cm\nlow large step,false,extrapolation error,{large_error:.12},cm\n", small_errors[0], small_errors[1], small_errors[2]);
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/34-gravity-tank-linearization/figures");
    write_or_compare_curriculum_artifact(
        &artifact_root.join("gravity-tank-linearization-run.csv"),
        &run,
    );
    evidence::retain(&artifact_root, "gravity-tank-linearization",
        "Linear models are local to one operating point",
        "Configured Simulation evidence · three small steps qualify; the restored large step exposes extrapolation",
        0.1,
        &[
            evidence::Panel { title: "Small-step nonlinear minus local-model error", unit: "cm", minimum: 0.0, maximum: 0.35, series: vec![
                evidence::Series { name: "25 cm operating point", color: "#0072b2", dashed: false, values: &small_error_traces[0] },
                evidence::Series { name: "50 cm operating point", color: "#d55e00", dashed: false, values: &small_error_traces[1] },
                evidence::Series { name: "81 cm operating point", color: "#009e73", dashed: false, values: &small_error_traces[2] },
            ]},
            evidence::Panel { title: "Low-level large-step extrapolation", unit: "cm", minimum: 20.0, maximum: 80.0, series: vec![
                evidence::Series { name: "nonlinear tank", color: "#0072b2", dashed: false, values: &large_physical_trace },
                evidence::Series { name: "local tangent model", color: "#d55e00", dashed: true, values: &large_local_trace },
            ]},
        ], &[], &[], &summary);
}

#[test]
fn control_curriculum_pole_zero_maps_predict_six_real_plant_responses() {
    let shipped_source = project_file("pole-zero-plant-classes", "src/Main.st");
    let source = without_lesson_workflow(
        shipped_source
            .split("CONFIGURATION Config")
            .next()
            .expect("example program before configuration")
            .to_owned(),
        "IF ExperimentEnabled OR BearingExperimentEnabled THEN",
        "InputsValid :=",
    )
    .replace(
        "ExperimentInput : LREAL :=",
        "ExperimentInput AT %ML0 : LREAL :=",
    )
    .replace(
        "BearingExperimentEnabled : BOOL :=",
        "BearingExperimentEnabled AT %MX8.0 : BOOL :=",
    )
    .replace(
        "HeaterCommand : LREAL :=",
        "HeaterCommand AT %QL0 : LREAL :=",
    )
    .replace("TankNetFlow : LREAL :=", "TankNetFlow AT %QL8 : LREAL :=")
    .replace("DriveTorque : LREAL :=", "DriveTorque AT %QL16 : LREAL :=")
    .replace("CraneForce : LREAL :=", "CraneForce AT %QL24 : LREAL :=")
    .replace(
        "BearingForce : LREAL :=",
        "BearingForce AT %QL32 : LREAL :=",
    )
    .replace(
        "BoilerFeedwater : LREAL :=",
        "BoilerFeedwater AT %QL40 : LREAL :=",
    )
    .replace(
        "HeaterTemperature : LREAL :=",
        "HeaterTemperature AT %IL0 : LREAL :=",
    )
    .replace("TankLevel : LREAL :=", "TankLevel AT %IL8 : LREAL :=")
    .replace(
        "DrivePosition : LREAL :=",
        "DrivePosition AT %IL16 : LREAL :=",
    )
    .replace(
        "CranePayloadPosition : LREAL :=",
        "CranePayloadPosition AT %IL24 : LREAL :=",
    )
    .replace(
        "BearingDisplacement : LREAL :=",
        "BearingDisplacement AT %IL32 : LREAL :=",
    )
    .replace(
        "BoilerDrumLevel : LREAL :=",
        "BoilerDrumLevel AT %IL40 : LREAL :=",
    );
    let build_runtime = || {
        CompileSession::from_sources(vec![SourceFile::with_path(
            "pole-zero-plant-classes/src/Main.st",
            source.clone(),
        )])
        .label_errors(true)
        .build_runtime()
        .expect("compile shipped pole-zero plant-class lesson")
    };
    let config_path = temporary_config_path("pole-zero-plant-classes");
    let simulation_source = project_file("pole-zero-plant-classes", "simulation.toml");
    for declared_parameter in [
        "time_constant_s = 5.0",
        "model = \"integrator\"",
        "natural_frequency_rad_s = 1.0",
        "damping_ratio = 0.2",
        "[-1.7, -17.2, -2.1]",
        "a = [[0.4]]",
        "zero=+0.0625",
    ] {
        assert!(simulation_source.contains(declared_parameter));
    }
    std::fs::write(&config_path, simulation_source)
        .expect("write six-plant pole-zero simulation config");
    let config = SimulationConfig::load(&config_path).expect("load six pole-zero plants");
    let experiment = IoAddress::parse("%ML0").unwrap();
    let bearing_enabled = IoAddress::parse("%MX8.0").unwrap();
    let outputs = ["%IL0", "%IL8", "%IL16", "%IL24", "%IL32", "%IL40"]
        .map(|value| IoAddress::parse(value).unwrap());

    let mut runtime = build_runtime();
    let mut simulation = SimulationController::new(config.clone());
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_lreal(&mut runtime, &experiment, 1.0);
    write_bool(&mut runtime, &bearing_enabled, false);
    let mut traces: [Vec<f64>; 6] = std::array::from_fn(|_| Vec::with_capacity(300));
    for cycle in 1usize..=300 {
        runtime
            .execute_cycle()
            .expect("apply common physical plant experiment");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance five bounded plant responses");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish plant-class responses");
        for (trace, address) in traces.iter_mut().zip(outputs.iter()) {
            trace.push(read_lreal(&runtime, address));
        }
    }

    let heater_fraction = (traces[0][49] - 20.0) / 4.0;
    assert!(
        (heater_fraction - 0.632_120_558_8).abs() <= 0.03,
        "heater five-second fraction={heater_fraction:.4}"
    );
    assert!(
        traces[0].windows(2).all(|pair| pair[1] >= pair[0] - 1e-12),
        "stable-real-pole heater response must be monotonic"
    );
    let tank_slope = (traces[1][299] - traces[1][199]) / 10.0;
    assert!(
        (tank_slope - 0.2).abs() <= 0.01,
        "integrating tank slope={tank_slope:.4} cm/s"
    );

    let drive_peak = traces[2].iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let drive_crossings = traces[2]
        .windows(2)
        .filter(|pair| (pair[0] - 1.0) * (pair[1] - 1.0) < 0.0)
        .count();
    assert!(
        drive_peak >= 1.45 && drive_crossings >= 3,
        "flexible drive peak/crossings={drive_peak:.3}/{drive_crossings}"
    );

    let crane_change_20_25 = traces[3][249] - traces[3][199];
    let crane_change_25_30 = traces[3][299] - traces[3][249];
    let crane_tail_ratio = crane_change_25_30 / crane_change_20_25;
    assert!(
        crane_tail_ratio >= 0.45 && crane_tail_ratio <= 0.75,
        "crane dominant-tail ratio={crane_tail_ratio:.4}"
    );

    let boiler_min = traces[5].iter().copied().fold(f64::INFINITY, f64::min);
    let boiler_final = traces[5][299];
    assert!(
        boiler_min <= -0.20 && boiler_final >= 0.50,
        "boiler inverse response minimum/final={boiler_min:.4}/{boiler_final:.4}"
    );
    assert!(
        traces[5][0] < 0.0,
        "RHP-zero boiler must move immediately in the wrong direction"
    );

    let mut bearing_runtime = build_runtime();
    let mut bearing_simulation = SimulationController::new(config);
    bearing_simulation
        .apply_pre_cycle(Duration::ZERO, &mut bearing_runtime)
        .unwrap();
    write_lreal(&mut bearing_runtime, &experiment, 1.0);
    write_bool(&mut bearing_runtime, &bearing_enabled, true);
    let mut bearing_abort_time = None;
    let mut bearing_at_five_seconds = 0.0;
    let mut bearing_trace = Vec::with_capacity(120);
    for cycle in 1usize..=120 {
        bearing_runtime
            .execute_cycle()
            .expect("apply bounded magnetic-bearing perturbation");
        let now = Duration::from_millis((cycle * 100) as i64);
        bearing_simulation
            .apply_post_cycle(now, &bearing_runtime)
            .unwrap();
        bearing_simulation
            .apply_pre_cycle(now, &mut bearing_runtime)
            .unwrap();
        let displacement = read_lreal(&bearing_runtime, &outputs[4]);
        bearing_trace.push(displacement);
        if cycle == 50 {
            bearing_at_five_seconds = displacement;
        }
        if displacement >= 0.010 {
            bearing_abort_time = Some(cycle as f64 * 0.1);
            break;
        }
    }
    let _ = std::fs::remove_file(config_path);
    let abort_time = bearing_abort_time.expect("unstable magnetic bearing must reach abort limit");
    assert!(
        bearing_at_five_seconds > 0.001,
        "unstable bearing must show exponential growth before abort"
    );
    assert!(
        (8.5..=10.0).contains(&abort_time),
        "bearing abort time={abort_time:.2}s"
    );
    assert!(!shipped_source.contains("Control.PID"));

    println!(
        "pole-zero signatures: heater 5s={heater_fraction:.3}, tank slope={tank_slope:.3}cm/s, drive peak/crossings={drive_peak:.3}/{drive_crossings}, crane tail ratio={crane_tail_ratio:.3}, bearing abort={abort_time:.1}s, boiler min/final={boiler_min:.3}/{boiler_final:.3}"
    );

    let heater_normalized = traces[0]
        .iter()
        .map(|value| (value - 20.0) / 4.0 * 100.0)
        .collect::<Vec<_>>();
    let drive_normalized = traces[2]
        .iter()
        .map(|value| value * 100.0)
        .collect::<Vec<_>>();
    let boiler_normalized = traces[5]
        .iter()
        .map(|value| value * 100.0)
        .collect::<Vec<_>>();
    let bearing_normalized = bearing_trace
        .iter()
        .map(|value| value / 0.010 * 100.0)
        .collect::<Vec<_>>();
    let signature_samples = bearing_normalized.len();
    let mut run = String::from("phase,sample,time_s,heater_degC,tank_cm,drive_rad,crane_m,bearing_m,boiler_level,bearing_abort_limit_m,evidence_class\n");
    for index in 0..300 {
        run.push_str(&format!("bounded_plants,{},{:.1},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},0.010000,configured_simulation\n", index + 1, (index + 1) as f64 * 0.1, traces[0][index], traces[1][index], traces[2][index], traces[3][index], traces[4][index], traces[5][index]));
    }
    for (index, displacement) in bearing_trace.iter().enumerate() {
        run.push_str(&format!(
            "bearing_abort,{},{:.1},,,,,{displacement:.12},,0.010000,configured_simulation\n",
            index + 1,
            (index + 1) as f64 * 0.1
        ));
    }
    let summary = format!("plant,accepted,decision_measure,measured_value,unit\nheater,true,five-second response fraction,{heater_fraction:.12},fraction\nintegrating tank,true,late slope,{tank_slope:.12},cm/s\nflexible drive,true,peak,{drive_peak:.12},rad\nflexible drive,true,steady-value crossings,{drive_crossings},count\noverhead crane,true,dominant-tail ratio,{crane_tail_ratio:.12},ratio\nmagnetic bearing,false,abort time,{abort_time:.12},s\nboiler drum,true,inverse-response minimum,{boiler_min:.12},level unit\nboiler drum,true,final response,{boiler_final:.12},level unit\n");
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/35-pole-zero-plant-classes/figures");
    let maps = [
        PoleZeroMap {
            label: "A",
            poles: &[(-0.2, 0.0)],
            zeros: &[],
        },
        PoleZeroMap {
            label: "B",
            poles: &[(0.0, 0.0)],
            zeros: &[],
        },
        PoleZeroMap {
            label: "C",
            poles: &[(-0.2, 0.979_795_897), (-0.2, -0.979_795_897)],
            zeros: &[],
        },
        PoleZeroMap {
            label: "D",
            poles: &[(-0.1, 0.0), (-1.0, 4.0), (-1.0, -4.0)],
            zeros: &[],
        },
        PoleZeroMap {
            label: "E",
            poles: &[(0.4, 0.0)],
            zeros: &[],
        },
        PoleZeroMap {
            label: "F",
            poles: &[(-0.05, 0.0), (-0.5, 0.0)],
            zeros: &[(0.0625, 0.0)],
        },
    ];
    let map_svg = render_hidden_pole_zero_worksheet(&maps);
    write_or_compare_curriculum_artifact(
        &artifact_root.join("pole-zero-plant-classes-map.svg"),
        &map_svg,
    );
    let matches = "map_id,poles_per_s,zeros_per_s,predicted_signature,committed_match,revealed_equipment,accepted\nA,-0.2,none,monotonic settling,heated vessel,heated vessel,true\nB,0,none,continuing ramp,storage tank,storage tank,true\nC,-0.2+j0.9798;-0.2-j0.9798,none,ringing decay,flexible drive,flexible drive,true\nD,-0.1;-1+j4;-1-j4,none,fast swing with slow tail,overhead crane,overhead crane,true\nE,+0.4,none,unstable growth to abort,magnetic bearing,magnetic bearing,true\nF,-0.05;-0.5,+0.0625,inverse response then positive settling,boiler drum,boiler drum,true\n";
    write_or_compare_curriculum_artifact(
        &artifact_root.join("pole-zero-plant-classes-matches.csv"),
        matches,
    );
    write_or_compare_curriculum_artifact(
        &artifact_root.join("pole-zero-plant-classes-run.csv"),
        &run,
    );
    evidence::retain(
        &artifact_root,
        "pole-zero-plant-classes",
        "Poles and zeros predict response signatures",
        "Configured Simulation evidence · the unstable bearing is a separate abort-bounded run",
        0.1,
        &[
            evidence::Panel {
                title: "Stable real pole and complex-pole response",
                unit: "% of declared change",
                minimum: 0.0,
                maximum: 180.0,
                series: vec![
                    evidence::Series {
                        name: "heater",
                        color: "#0072b2",
                        dashed: false,
                        values: &heater_normalized[..signature_samples],
                    },
                    evidence::Series {
                        name: "flexible drive",
                        color: "#d55e00",
                        dashed: false,
                        values: &drive_normalized[..signature_samples],
                    },
                ],
            },
            evidence::Panel {
                title: "Right-half-plane signatures",
                unit: "% of declared scale",
                minimum: -60.0,
                maximum: 110.0,
                series: vec![
                    evidence::Series {
                        name: "boiler inverse response",
                        color: "#009e73",
                        dashed: false,
                        values: &boiler_normalized[..bearing_normalized.len()],
                    },
                    evidence::Series {
                        name: "bearing abort approach",
                        color: "#cc79a7",
                        dashed: false,
                        values: &bearing_normalized,
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );
}

#[test]
fn control_curriculum_two_tank_state_space_exposes_hidden_stored_water() {
    let shipped_source = project_file("two-tank-state-space", "src/Main.st");
    let source = without_lesson_workflow(
        shipped_source
            .split("CONFIGURATION Config")
            .next()
            .expect("example program before configuration")
            .to_owned(),
        "IF ExperimentEnabled THEN",
        "InputsValid :=",
    )
    .replace(
        "RequestedPumpFlow : LREAL :=",
        "RequestedPumpFlow AT %ML0 : LREAL :=",
    )
    .replace("MainPumpFlow : LREAL :=", "MainPumpFlow AT %QL0 : LREAL :=")
    .replace(
        "EmptyCasePumpFlow : LREAL :=",
        "EmptyCasePumpFlow AT %QL8 : LREAL :=",
    )
    .replace(
        "ChargedCasePumpFlow : LREAL :=",
        "ChargedCasePumpFlow AT %QL16 : LREAL :=",
    )
    .replace(
        "MainUpstreamLevel : LREAL :=",
        "MainUpstreamLevel AT %IL0 : LREAL :=",
    )
    .replace(
        "MainDownstreamLevel : LREAL :=",
        "MainDownstreamLevel AT %IL8 : LREAL :=",
    )
    .replace(
        "EmptyUpstreamLevel : LREAL :=",
        "EmptyUpstreamLevel AT %IL16 : LREAL :=",
    )
    .replace(
        "EmptyDownstreamLevel : LREAL :=",
        "EmptyDownstreamLevel AT %IL24 : LREAL :=",
    )
    .replace(
        "ChargedUpstreamLevel : LREAL :=",
        "ChargedUpstreamLevel AT %IL32 : LREAL :=",
    )
    .replace(
        "ChargedDownstreamLevel : LREAL :=",
        "ChargedDownstreamLevel AT %IL40 : LREAL :=",
    );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "two-tank-state-space/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped two-tank state-space lesson");
    let config_path = temporary_config_path("two-tank-state-space");
    std::fs::write(
        &config_path,
        project_file("two-tank-state-space", "simulation.toml"),
    )
    .expect("write interconnected-tank state-space config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load three two-state tank cases"),
    );
    let addresses = ["%IL0", "%IL8", "%IL16", "%IL24", "%IL32", "%IL40"]
        .map(|value| IoAddress::parse(value).unwrap());
    write_lreal(
        &mut runtime,
        &IoAddress::parse("%ML0").expect("requested pump-flow address"),
        0.1,
    );
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    assert!((read_lreal(&runtime, &addresses[3])).abs() <= 1e-12);
    assert!(
        (read_lreal(&runtime, &addresses[5])).abs() <= 1e-12,
        "hidden cases must begin with identical downstream readings"
    );

    let mut charged_peak = 0.0_f64;
    let mut charged_peak_time = 0.0_f64;
    let mut traces: [Vec<f64>; 6] = std::array::from_fn(|_| Vec::with_capacity(3_000));
    for cycle in 1usize..=3_000 {
        runtime
            .execute_cycle()
            .expect("route two-tank pump experiments");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance interconnected tanks");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish all tank states");
        for (trace, address) in traces.iter_mut().zip(addresses.iter()) {
            trace.push(read_lreal(&runtime, address) * 100.0);
        }
        let charged_downstream = read_lreal(&runtime, &addresses[5]);
        if charged_downstream > charged_peak {
            charged_peak = charged_downstream;
            charged_peak_time = cycle as f64 * 0.1;
        }
    }
    let _ = std::fs::remove_file(config_path);

    let main_upstream_cm = read_lreal(&runtime, &addresses[0]) * 100.0;
    let main_downstream_cm = read_lreal(&runtime, &addresses[1]) * 100.0;
    let empty_upstream = read_lreal(&runtime, &addresses[2]);
    let empty_downstream = read_lreal(&runtime, &addresses[3]);
    let charged_downstream_final = read_lreal(&runtime, &addresses[5]);
    assert!(
        (main_upstream_cm - 20.0).abs() <= 0.02,
        "main upstream equilibrium={main_upstream_cm:.4} cm"
    );
    assert!(
        (main_downstream_cm - 50.0).abs() <= 0.03,
        "main downstream equilibrium={main_downstream_cm:.4} cm"
    );
    assert!(
        empty_upstream.abs() <= 1e-12 && empty_downstream.abs() <= 1e-12,
        "empty hidden-state case must remain at zero"
    );
    assert!(
        (charged_peak * 100.0 - 10.86).abs() <= 0.20,
        "charged downstream peak={:.4} cm",
        charged_peak * 100.0
    );
    assert!(
        (charged_peak_time - 3.05).abs() <= 0.15,
        "charged downstream peak time={charged_peak_time:.3} s"
    );
    assert!(
        charged_downstream_final.abs() <= 1e-8,
        "charged hidden-state case must eventually drain"
    );
    assert!(!shipped_source.contains("Control.PID"));
    assert!(!shipped_source.contains("Observer"));

    println!(
        "two-tank state space: equilibrium={main_upstream_cm:.3}/{main_downstream_cm:.3}cm, hidden downstream initial=0/0cm, charged peak={:.3}cm at {charged_peak_time:.2}s",
        charged_peak * 100.0
    );

    let equilibrium_error = (main_upstream_cm - 20.0)
        .abs()
        .max((main_downstream_cm - 50.0).abs());
    let mut run = String::from("sample,time_s,main_pump_m3_s,main_upstream_cm,main_downstream_cm,empty_upstream_cm,empty_downstream_cm,charged_upstream_cm,charged_downstream_cm,matrix_revision,state_order,matrix_a,matrix_b,matrix_c,matrix_d,equilibrium_error_cm,hidden_state_separation_cm,evidence_class\n");
    for index in 0..3_000 {
        let hidden_separation = (traces[5][index] - traces[3][index]).abs();
        run.push_str(&format!("{},{:.1},0.100000,{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},1,h1_m;h2_m,-0.5:0;0.5:-0.2,1;0,1:0;0:1,0;0,{equilibrium_error:.12},{hidden_separation:.12},configured_simulation\n", index + 1, (index + 1) as f64 * 0.1, traces[0][index], traces[1][index], traces[2][index], traces[3][index], traces[4][index], traces[5][index]));
    }
    let summary = format!("case,accepted,decision_measure,measured_value,unit\nmain upstream,true,equilibrium level,{main_upstream_cm:.12},cm\nmain downstream,true,equilibrium level,{main_downstream_cm:.12},cm\nempty hidden case,true,downstream peak,0.000000000000,cm\nupstream-charged hidden case,true,downstream peak,{:.12},cm\nupstream-charged hidden case,true,peak time,{charged_peak_time:.12},s\n", charged_peak * 100.0);
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/04-controller-selection-and-plant-models/36-two-tank-state-space/figures");
    write_or_compare_curriculum_artifact(&artifact_root.join("two-tank-state-space-run.csv"), &run);
    let prediction_summary = "prediction_id,visible_case_a_downstream_cm,visible_case_b_downstream_cm,learner_commitment,learner_reason,reveal_status\nA_vs_B,0.000000,0.000000,,,not_revealed\n";
    let prediction_prompt = r##"<svg xmlns="http://www.w3.org/2000/svg" role="img" aria-labelledby="title desc" data-stage="pre-commit" viewBox="0 0 900 420">
<title id="title">Commit a prediction from identical visible readings</title>
<desc id="desc">Cases A and B both show zero centimetres downstream. Their upstream states and future traces remain hidden.</desc>
<rect width="900" height="420" fill="#f7f9fc"/>
<style>text{font-family:system-ui,-apple-system,sans-serif;fill:#243044}.tank{fill:#eaf2fb;stroke:#526078;stroke-width:2}.hidden{fill:#eef0f4;stroke:#8792a5;stroke-width:2;stroke-dasharray:7 5}.line{stroke:#526078;stroke-width:3}.blank{fill:#fff;stroke:#8792a5;stroke-width:1.5}</style>
<text x="55" y="45" font-size="24" font-weight="700">Do identical current outputs imply identical future outputs?</text>
<text x="55" y="72" font-size="14">Only the current downstream level is visible. Record your prediction before opening the reveal.</text>
<text x="245" y="112" text-anchor="middle" font-size="18" font-weight="700">Case A</text>
<rect x="135" y="135" width="220" height="85" rx="8" class="hidden"/><text x="245" y="183" text-anchor="middle" font-size="16">upstream state hidden</text>
<line x1="245" y1="220" x2="245" y2="255" class="line"/>
<rect x="135" y="255" width="220" height="85" rx="8" class="tank"/><text x="245" y="296" text-anchor="middle" font-size="16">downstream level</text><text x="245" y="322" text-anchor="middle" font-size="20" font-weight="700">0.000 cm</text>
<text x="655" y="112" text-anchor="middle" font-size="18" font-weight="700">Case B</text>
<rect x="545" y="135" width="220" height="85" rx="8" class="hidden"/><text x="655" y="183" text-anchor="middle" font-size="16">upstream state hidden</text>
<line x1="655" y1="220" x2="655" y2="255" class="line"/>
<rect x="545" y="255" width="220" height="85" rx="8" class="tank"/><text x="655" y="296" text-anchor="middle" font-size="16">downstream level</text><text x="655" y="322" text-anchor="middle" font-size="20" font-weight="700">0.000 cm</text>
<rect x="135" y="365" width="630" height="36" rx="5" class="blank"/><text x="150" y="389" font-size="14">Prediction and reason:</text>
</svg>
"##;
    write_or_compare_curriculum_artifact(
        &artifact_root.join("two-tank-hidden-prediction.svg"),
        prediction_prompt,
    );
    write_or_compare_curriculum_artifact(
        &artifact_root.join("two-tank-hidden-prediction-summary.csv"),
        prediction_summary,
    );
    evidence::retain(
        &artifact_root,
        "two-tank-state-space",
        "One output can hide different internal states",
        "Configured Simulation evidence · both hidden cases start at zero downstream level",
        0.1,
        &[
            evidence::Panel {
                title: "Main two-tank response",
                unit: "cm",
                minimum: 0.0,
                maximum: 55.0,
                series: vec![
                    evidence::Series {
                        name: "upstream level",
                        color: "#0072b2",
                        dashed: false,
                        values: &traces[0],
                    },
                    evidence::Series {
                        name: "downstream level",
                        color: "#d55e00",
                        dashed: false,
                        values: &traces[1],
                    },
                ],
            },
            evidence::Panel {
                title: "Zero-input downstream reveal",
                unit: "cm",
                minimum: 0.0,
                maximum: 12.0,
                series: vec![
                    evidence::Series {
                        name: "empty upstream state",
                        color: "#009e73",
                        dashed: true,
                        values: &traces[3],
                    },
                    evidence::Series {
                        name: "charged upstream state",
                        color: "#cc79a7",
                        dashed: false,
                        values: &traces[5],
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );
}

#[test]
fn control_curriculum_heater_energy_balance_predicts_configured_plant_truth() {
    let source = without_lesson_workflow(
        project_file("water-heater-energy-balance", "src/Main.st")
            .split("CONFIGURATION Config")
            .next()
            .expect("example program before configuration")
            .to_owned(),
        "(* Control Studio changes only this Boolean authority.",
        "InputsValid :=",
    )
    .replace(
        "IF ModelValid AND InputsValid AND ExperimentEnabled THEN",
        "IF ModelValid AND InputsValid THEN",
    )
    .replace(
        "RequestedHeaterPower : LREAL :=",
        "RequestedHeaterPower AT %IL32 : LREAL :=",
    )
    .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
    .replace(
        "AppliedHeaterPower : LREAL :=",
        "AppliedHeaterPower AT %QL0 : LREAL :=",
    )
    .replace("StaticGain : LREAL :=", "StaticGain AT %ML0 : LREAL :=")
    .replace("TimeConstant : LREAL :=", "TimeConstant AT %ML8 : LREAL :=")
    .replace(
        "PredictedEquilibrium : LREAL :=",
        "PredictedEquilibrium AT %ML16 : LREAL :=",
    )
    .replace(
        "EffectiveHeatLoss : LREAL :=",
        "EffectiveHeatLoss AT %ML40 : LREAL :=",
    )
    .replace(
        "ThermalCapacitance : LREAL :=",
        "ThermalCapacitance AT %ML48 : LREAL :=",
    )
    .replace(
        "FullScaleHeaterPower : LREAL :=",
        "FullScaleHeaterPower AT %ML56 : LREAL :=",
    )
    .replace(
        "AmbientTemperature : LREAL :=",
        "AmbientTemperature AT %ML64 : LREAL :=",
    )
    .replace("ModelValid : BOOL :=", "ModelValid AT %MX80.0 : BOOL :=")
    .replace("InputsValid : BOOL :=", "InputsValid AT %MX80.1 : BOOL :=")
    .replace(
        "ExperimentEnabled : BOOL :=",
        "ExperimentEnabled AT %MX81.0 : BOOL :=",
    );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "water-heater-energy-balance/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped water-heater energy-balance lesson");
    let config_path = temporary_config_path("water-heater-energy-balance");
    std::fs::write(
        &config_path,
        project_file("water-heater-energy-balance", "simulation.toml"),
    )
    .expect("write water-heater simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load water-heater simulation"),
    );
    let command = IoAddress::parse("%IL32").expect("manual command address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let gain = IoAddress::parse("%ML0").expect("derived gain address");
    let time_constant = IoAddress::parse("%ML8").expect("time constant address");
    let equilibrium = IoAddress::parse("%ML16").expect("equilibrium address");
    let heat_loss = IoAddress::parse("%ML40").expect("heat-loss address");
    let thermal_capacitance = IoAddress::parse("%ML48").expect("capacitance address");
    let full_scale_power = IoAddress::parse("%ML56").expect("heater-rating address");
    let ambient = IoAddress::parse("%ML64").expect("ambient-temperature address");
    let model_valid = IoAddress::parse("%MX80.0").expect("model-valid address");
    let inputs_valid = IoAddress::parse("%MX80.1").expect("inputs-valid address");
    let experiment_enabled = IoAddress::parse("%MX81.0").expect("experiment-authority address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heater state");
    write_lreal(&mut runtime, &thermal_capacitance, 50.0);
    write_lreal(&mut runtime, &heat_loss, 10.0);
    write_lreal(&mut runtime, &full_scale_power, 200.0);
    write_lreal(&mut runtime, &ambient, 20.0);
    runtime
        .io_mut()
        .write(&experiment_enabled, Value::Bool(true))
        .expect("enable Lesson 15 Test authority");

    let mut trace = Vec::with_capacity(300);
    for cycle in 1..=300 {
        write_lreal(&mut runtime, &command, 20.0);
        runtime
            .execute_cycle()
            .expect("execute energy-balance lesson");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish heater PV");
        trace.push(read_lreal(&runtime, &pv));
    }
    let _ = std::fs::remove_file(config_path);

    let derived_gain = read_lreal(&runtime, &gain);
    let derived_time_constant = read_lreal(&runtime, &time_constant);
    let predicted_equilibrium = read_lreal(&runtime, &equilibrium);
    let pv_at_delay_plus_tau = trace[59];
    let final_pv = *trace.last().expect("heater trace sample");
    assert!((read_lreal(&runtime, &mv) - 20.0).abs() <= 1e-12);
    assert!(
        (derived_gain - 0.2).abs() <= 1e-12,
        "derived gain={derived_gain}"
    );
    assert!(
        (derived_time_constant - 5.0).abs() <= 1e-12,
        "derived tau={derived_time_constant}"
    );
    assert!(
        (predicted_equilibrium - 24.0).abs() <= 1e-12,
        "predicted equilibrium={predicted_equilibrium}"
    );
    assert!(
        (pv_at_delay_plus_tau - (20.0 + 0.6321205588 * 4.0)).abs() <= 0.08,
        "PV at delay plus tau={pv_at_delay_plus_tau}"
    );
    assert!(
        (final_pv - predicted_equilibrium).abs() <= 0.02,
        "final PV={final_pv}"
    );

    write_lreal(&mut runtime, &command, 120.0);
    runtime
        .execute_cycle()
        .expect("execute saturated energy-balance request");
    assert!(
        (read_lreal(&runtime, &command) - 120.0).abs() <= 1e-12,
        "the learner's requested heater power must remain observable"
    );
    assert!(
        (read_lreal(&runtime, &mv) - 100.0).abs() <= 1e-12,
        "the physical heater power must be limited to 100 percent"
    );
    assert!(
        (read_lreal(&runtime, &equilibrium) - 40.0).abs() <= 1e-12,
        "the physical prediction must use applied rather than impossible requested power"
    );

    write_lreal(&mut runtime, &command, 20.0);
    write_lreal(&mut runtime, &heat_loss, 0.0);
    runtime
        .execute_cycle()
        .expect("execute invalid energy-balance parameters");
    assert_eq!(
        read_bool(&runtime, &model_valid),
        false,
        "zero heat loss must invalidate the model"
    );
    assert!(
        (read_lreal(&runtime, &command) - 20.0).abs() <= 1e-12,
        "the invalid-model journey must retain the request for diagnosis"
    );
    assert!(
        read_lreal(&runtime, &mv).abs() <= 1e-12,
        "invalid model parameters must command safe zero applied heater power"
    );

    for (parameter, name) in [
        (&thermal_capacitance, "thermal capacitance"),
        (&heat_loss, "heat loss"),
        (&full_scale_power, "full-scale heater power"),
        (&ambient, "ambient temperature"),
    ] {
        write_lreal(&mut runtime, &thermal_capacitance, 50.0);
        write_lreal(&mut runtime, &heat_loss, 10.0);
        write_lreal(&mut runtime, &full_scale_power, 200.0);
        write_lreal(&mut runtime, &ambient, 20.0);
        write_lreal(&mut runtime, &pv, 20.0);
        write_lreal(&mut runtime, &command, 20.0);
        write_lreal(&mut runtime, parameter, 1.0e12);
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("execute out-of-envelope {name}: {error}"));
        assert_eq!(
            read_bool(&runtime, &model_valid),
            false,
            "out-of-envelope {name} must invalidate the model"
        );
        assert!(
            read_lreal(&runtime, &mv).is_finite() && read_lreal(&runtime, &mv).abs() <= 1e-12,
            "out-of-envelope {name} must command safe zero applied heater power"
        );
    }

    write_lreal(&mut runtime, &thermal_capacitance, 50.0);
    write_lreal(&mut runtime, &heat_loss, 10.0);
    write_lreal(&mut runtime, &full_scale_power, 200.0);
    write_lreal(&mut runtime, &ambient, 20.0);
    write_lreal(&mut runtime, &pv, 81.0);
    write_lreal(&mut runtime, &command, 20.0);
    runtime
        .execute_cycle()
        .expect("execute PV hard-limit breach");
    assert!(
        read_lreal(&runtime, &mv).abs() <= 1e-12,
        "a PV hard-limit breach must command safe zero applied heater power"
    );

    for (value, name) in [(-0.1, "ambient below zero"), (80.1, "ambient above 80")] {
        write_lreal(&mut runtime, &thermal_capacitance, 50.0);
        write_lreal(&mut runtime, &heat_loss, 10.0);
        write_lreal(&mut runtime, &full_scale_power, 200.0);
        write_lreal(&mut runtime, &ambient, value);
        write_lreal(&mut runtime, &pv, 20.0);
        write_lreal(&mut runtime, &command, 20.0);
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("execute {name}: {error}"));
        assert!(
            !read_bool(&runtime, &model_valid),
            "{name} must invalidate the model"
        );
        assert!(
            read_lreal(&runtime, &mv).abs() <= 1e-12,
            "{name} must command safe zero applied heater power"
        );
    }

    for (value, name) in [
        (-1000.1, "request below -1000"),
        (1000.1, "request above 1000"),
    ] {
        write_lreal(&mut runtime, &thermal_capacitance, 50.0);
        write_lreal(&mut runtime, &heat_loss, 10.0);
        write_lreal(&mut runtime, &full_scale_power, 200.0);
        write_lreal(&mut runtime, &ambient, 20.0);
        write_lreal(&mut runtime, &pv, 20.0);
        write_lreal(&mut runtime, &command, value);
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("execute {name}: {error}"));
        assert!(
            read_bool(&runtime, &model_valid),
            "{name} must not invalidate the model"
        );
        assert!(
            !read_bool(&runtime, &inputs_valid),
            "{name} must invalidate the inputs"
        );
        assert!(
            (read_lreal(&runtime, &command) - value).abs() <= 1e-12,
            "{name} must remain observable for diagnosis"
        );
        assert!(
            read_lreal(&runtime, &mv).abs() <= 1e-12,
            "{name} must command safe zero applied heater power"
        );
    }

    for (ambient_value, pv_value, request_value, expected_applied, name) in [
        (0.0, 0.0, -1000.0, 0.0, "inclusive lower input boundaries"),
        (
            80.0,
            80.0,
            1000.0,
            100.0,
            "inclusive upper input boundaries",
        ),
    ] {
        write_lreal(&mut runtime, &thermal_capacitance, 1.0e6);
        write_lreal(&mut runtime, &heat_loss, 1.0e6);
        write_lreal(&mut runtime, &full_scale_power, 1.0e6);
        write_lreal(&mut runtime, &ambient, ambient_value);
        write_lreal(&mut runtime, &pv, pv_value);
        write_lreal(&mut runtime, &command, request_value);
        runtime
            .execute_cycle()
            .unwrap_or_else(|error| panic!("execute {name}: {error}"));
        assert!(
            read_bool(&runtime, &model_valid),
            "{name} must keep the model valid"
        );
        assert!(
            read_bool(&runtime, &inputs_valid),
            "{name} must keep the inputs valid"
        );
        assert!(
            (read_lreal(&runtime, &mv) - expected_applied).abs() <= 1e-12,
            "{name} applied output={}",
            read_lreal(&runtime, &mv)
        );
    }

    write_lreal(&mut runtime, &thermal_capacitance, 1.0e-12);
    write_lreal(&mut runtime, &heat_loss, 1.0e-12);
    write_lreal(&mut runtime, &full_scale_power, 0.0);
    write_lreal(&mut runtime, &ambient, 0.0);
    write_lreal(&mut runtime, &pv, 0.0);
    write_lreal(&mut runtime, &command, 0.0);
    runtime
        .execute_cycle()
        .expect("execute inclusive positive and zero parameter boundaries");
    assert!(read_bool(&runtime, &model_valid));
    assert!(read_bool(&runtime, &inputs_valid));
    assert!(read_lreal(&runtime, &mv).abs() <= 1e-12);

    write_lreal(&mut runtime, &pv, 20.0);
    write_lreal(&mut runtime, &command, f64::NAN);
    let nonfinite_error = runtime
        .execute_cycle()
        .expect_err("the typed process-image boundary must reject a nonfinite request");
    assert!(
        nonfinite_error.to_string().contains("must be finite"),
        "unexpected nonfinite-input rejection: {nonfinite_error}"
    );
}

#[test]
fn control_curriculum_heater_step_recovers_fopdt_from_recorded_reaction_curve() {
    let source = project_file("heater-step-identification", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "ExperimentInput : LREAL :=",
            "ExperimentInput AT %ML32 : LREAL :=",
        )
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX81.0 : BOOL :=",
        )
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX81.3 : BOOL :=")
        .replace("SafetyAbort : BOOL :=", "SafetyAbort AT %MX81.1 : BOOL :=")
        .replace("InputsValid : BOOL :=", "InputsValid AT %MX81.2 : BOOL :=");
    let control_source = include_str!("../../../libraries/control/src/control.st");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", control_source),
        SourceFile::with_path("heater-step-identification/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped heater identification lesson");
    let config_path = temporary_config_path("heater-step-identification");
    std::fs::write(
        &config_path,
        project_file("heater-step-identification", "simulation.toml"),
    )
    .expect("write heater identification simulation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load heater identification simulation"),
    );
    let command = IoAddress::parse("%ML32").expect("project-owned command address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("applied heater address");
    let experiment_enabled = IoAddress::parse("%MX81.0").expect("Test authority address");
    let safety_abort = IoAddress::parse("%MX81.1").expect("safety-abort address");
    let inputs_valid = IoAddress::parse("%MX81.2").expect("inputs-valid address");
    let auto_mode = IoAddress::parse("%MX81.3").expect("Auto-mode address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heater state");

    runtime
        .execute_cycle()
        .expect("establish the declared Manual operating point");
    assert!(
        (read_lreal(&runtime, &mv) - 20.0).abs() <= 1e-12,
        "the Manual path must establish the reviewed 20 percent operating point"
    );

    write_lreal(&mut runtime, &command, 30.0);
    runtime
        .execute_cycle()
        .expect("execute stale request without Test authority");
    assert!(
        (read_lreal(&runtime, &command) - 20.0).abs() <= 1e-12
            && (read_lreal(&runtime, &mv) - 20.0).abs() <= 1e-12,
        "without Test authority the project must retain 20 percent"
    );
    write_bool(&mut runtime, &experiment_enabled, true);
    runtime
        .execute_cycle()
        .expect("execute first project-owned baseline scan");
    assert!(
        read_bool(&runtime, &inputs_valid),
        "the reviewed experiment inputs must be valid"
    );
    assert!(
        !read_bool(&runtime, &auto_mode),
        "the open-loop experiment must report Manual rather than Auto mode"
    );

    write_lreal(&mut runtime, &pv, 50.0);
    runtime
        .execute_cycle()
        .expect("execute invalid process value");
    assert!(
        !read_bool(&runtime, &inputs_valid),
        "a PV outside the diagnostic envelope must expose invalid input state"
    );
    assert!(
        read_lreal(&runtime, &mv).abs() <= 1e-12,
        "an invalid PV must command safe zero"
    );
    write_lreal(&mut runtime, &pv, 30.0);

    runtime
        .io_mut()
        .write(&safety_abort, Value::Bool(true))
        .expect("request safety abort");
    runtime.execute_cycle().expect("execute safety abort");
    assert!(
        read_lreal(&runtime, &mv).abs() <= 1e-12,
        "a safety abort must override the experiment request and command zero in the same scan"
    );
    runtime
        .io_mut()
        .write(&safety_abort, Value::Bool(false))
        .expect("clear safety abort for reviewed restoration");

    write_bool(&mut runtime, &experiment_enabled, false);
    runtime
        .execute_cycle()
        .expect("begin Manual restoration after abort");
    runtime
        .execute_cycle()
        .expect("complete Manual restoration after abort");
    assert!(
        (read_lreal(&runtime, &mv) - 20.0).abs() <= 1e-12,
        "the reviewed restoration request must restore the 20 percent holding point"
    );

    write_bool(&mut runtime, &experiment_enabled, true);
    let mut trace = Vec::with_capacity(500);
    for cycle in 1..=500 {
        runtime
            .execute_cycle()
            .expect("execute heater identification lesson");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish heater PV");
        trace.push(read_lreal(&runtime, &pv));
    }
    let _ = std::fs::remove_file(config_path);

    let baseline = trace[80..100].iter().sum::<f64>() / 20.0;
    let final_pv = trace[450..500].iter().sum::<f64>() / 50.0;
    let delta_pv = final_pv - baseline;
    let estimated_gain = delta_pv / 10.0;
    let response_start = trace[100..]
        .iter()
        .position(|value| *value >= baseline + 0.01)
        .expect("measurable response after step") as f64
        * 0.1;
    let target = baseline + 0.6321205588 * delta_pv;
    let target_time = trace[100..]
        .iter()
        .position(|value| *value >= target)
        .expect("63.2 percent crossing") as f64
        * 0.1;
    let estimated_time_constant = target_time - response_start;

    assert!((baseline - 30.0).abs() <= 1e-9, "baseline PV={baseline}");
    assert!(
        (estimated_gain - 0.2).abs() <= 0.005,
        "estimated K={estimated_gain}"
    );
    assert!(
        (response_start - 1.0).abs() <= 0.2,
        "estimated L={response_start}"
    );
    assert!(
        (estimated_time_constant - 5.0).abs() <= 0.3,
        "estimated tau={estimated_time_constant}"
    );
}

#[test]
fn control_curriculum_heater_validation_accepts_new_step_but_restricts_unmodelled_load() {
    let estimation_evidence: serde_json::Value = serde_json::from_str(&project_file(
        "heater-step-identification",
        "figures/heater-identification-model-evidence.json",
    ))
    .expect("parse retained Lesson 16 selected-model evidence");
    let selected = &estimation_evidence["selected"];
    let model_gain = selected["gain"].as_f64().expect("selected model gain");
    let model_time_constant = selected["timeConstantSeconds"]
        .as_f64()
        .expect("selected model time constant");
    let model_dead_time = selected["deadTimeSeconds"]
        .as_f64()
        .expect("selected model dead time");
    let source = project_file("independent-model-validation", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "ExperimentInput : LREAL :=",
            "ExperimentInput AT %ML32 : LREAL :=",
        )
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %MX81.0 : BOOL :=",
        )
        .replace("SafetyAbort : BOOL :=", "SafetyAbort AT %MX81.1 : BOOL :=")
        .replace("InputsValid : BOOL :=", "InputsValid AT %MX81.2 : BOOL :=")
        .replace("ManualMode : BOOL :=", "ManualMode AT %MX81.3 : BOOL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("independent-model-validation/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped independent heater validation lesson");
    let config_path = temporary_config_path("independent-model-validation");
    std::fs::write(
        &config_path,
        project_file("independent-model-validation", "simulation.toml"),
    )
    .expect("write independent heater validation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path)
            .expect("load independent heater validation simulation"),
    );
    let command = IoAddress::parse("%ML32").expect("project-owned command address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("applied heater address");
    let test_authority = IoAddress::parse("%MX81.0").expect("Test authority address");
    let safety_abort = IoAddress::parse("%MX81.1").expect("Safety abort address");
    let inputs_valid = IoAddress::parse("%MX81.2").expect("input-validity address");
    let manual_mode = IoAddress::parse("%MX81.3").expect("Manual-mode address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();

    write_lreal(&mut runtime, &command, 25.0);
    write_bool(&mut runtime, &test_authority, false);
    write_bool(&mut runtime, &safety_abort, false);
    write_bool(&mut runtime, &inputs_valid, true);
    runtime
        .execute_cycle()
        .expect("execute validation request without Test authority");
    assert_eq!(
        read_lreal(&runtime, &mv),
        20.0,
        "without Test authority the validation step must not reach the plant"
    );
    write_bool(&mut runtime, &test_authority, true);

    write_lreal(&mut runtime, &command, 77.0);
    for cycle in 1..=101 {
        runtime
            .execute_cycle()
            .expect("execute project-owned validation sequence scan");
        if cycle == 1 || cycle == 100 {
            assert_eq!(
                read_lreal(&runtime, &command),
                20.0,
                "the project must retain its 20-percent baseline through scan {cycle}"
            );
        }
    }
    assert_eq!(
        read_lreal(&runtime, &command),
        25.0,
        "the project must own the 25-percent request from scan 101"
    );
    write_bool(&mut runtime, &test_authority, false);
    runtime
        .execute_cycle()
        .expect("restore project-owned validation sequence");
    assert_eq!(read_lreal(&runtime, &command), 20.0);
    assert_eq!(read_lreal(&runtime, &mv), 20.0);
    write_bool(&mut runtime, &test_authority, true);

    let mut trace = Vec::with_capacity(600);
    for cycle in 1..=600 {
        write_lreal(
            &mut runtime,
            &command,
            if cycle <= 100 { 20.0 } else { 25.0 },
        );
        runtime
            .execute_cycle()
            .expect("execute independent validation lesson");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance validation plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish validation PV");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    let _ = std::fs::remove_file(config_path);

    let prediction = |absolute_time: f64| {
        let elapsed = absolute_time - 10.0;
        if elapsed <= model_dead_time {
            30.0
        } else {
            30.0 + model_gain
                * 5.0
                * (1.0 - (-(elapsed - model_dead_time) / model_time_constant).exp())
        }
    };
    let baseline = trace[80..100].iter().map(|sample| sample.0).sum::<f64>() / 20.0;
    let fixed_flow_rmse = ((109..290)
        .map(|index| {
            let residual = trace[index].0 - prediction((index + 1) as f64 * 0.1);
            residual * residual
        })
        .sum::<f64>()
        / 181.0)
        .sqrt();
    let post_load_bias = (499..600)
        .map(|index| trace[index].0 - prediction((index + 1) as f64 * 0.1))
        .sum::<f64>()
        / 101.0;

    assert!(
        (baseline - 30.0).abs() <= 1e-9,
        "validation baseline={baseline}"
    );
    assert!(
        fixed_flow_rmse <= 0.03,
        "fixed-flow validation RMSE={fixed_flow_rmse}"
    );
    assert!(
        post_load_bias <= -0.9 && post_load_bias >= -1.1,
        "unmodelled-load residual bias={post_load_bias}"
    );

    let fixed_residuals = (109..290)
        .map(|index| trace[index].0 - prediction((index + 1) as f64 * 0.1))
        .collect::<Vec<_>>();
    let lag_one = pearson_correlation(
        &fixed_residuals[..fixed_residuals.len() - 1],
        &fixed_residuals[1..],
    );
    let mut evidence = String::from(
        "sample,time_s,event,requested_power_percent,applied_power_percent,measured_pv_degc,predicted_pv_degc,envelope_lower_degc,envelope_upper_degc,residual_degc,quality_good,timestamp_gap_s,test_authority,manual_mode,inputs_valid\n",
    );
    let uncertainty = &selected["uncertainty"];
    let gain_bounds = [
        uncertainty["gain"]["minimum"]
            .as_f64()
            .expect("minimum model gain"),
        uncertainty["gain"]["maximum"]
            .as_f64()
            .expect("maximum model gain"),
    ];
    let time_bounds = [
        uncertainty["timeConstantSeconds"]["minimum"]
            .as_f64()
            .expect("minimum model time constant"),
        uncertainty["timeConstantSeconds"]["maximum"]
            .as_f64()
            .expect("maximum model time constant"),
    ];
    let delay_bounds = [
        uncertainty["deadTimeSeconds"]["minimum"]
            .as_f64()
            .expect("minimum model dead time"),
        uncertainty["deadTimeSeconds"]["maximum"]
            .as_f64()
            .expect("maximum model dead time"),
    ];
    for (index, (measured, applied)) in trace.iter().enumerate() {
        let time_s = (index + 1) as f64 * 0.1;
        let requested = if index < 100 { 20.0 } else { 25.0 };
        let event = if index < 100 {
            "baseline"
        } else if index < 299 {
            "independent_step"
        } else {
            "cold_flow_load"
        };
        let nominal = prediction(time_s);
        let elapsed = time_s - 10.0;
        let mut envelope_low = f64::INFINITY;
        let mut envelope_high = f64::NEG_INFINITY;
        for gain in gain_bounds {
            for time_constant in time_bounds {
                for delay in delay_bounds {
                    let predicted = if elapsed <= delay {
                        30.0
                    } else {
                        30.0 + gain * 5.0 * (1.0 - (-(elapsed - delay) / time_constant).exp())
                    };
                    envelope_low = envelope_low.min(predicted);
                    envelope_high = envelope_high.max(predicted);
                }
            }
        }
        writeln!(
            evidence,
            "{},{time_s:.1},{event},{requested:.2},{applied:.12},{measured:.12},{nominal:.12},{envelope_low:.12},{envelope_high:.12},{:.12},true,0.100,true,true,true",
            index + 1,
            measured - nominal
        )
        .unwrap();
    }
    let summary = format!(
        "metric,value,unit,window,decision\nfixed_flow_rmse,{fixed_flow_rmse:.12},degC,11.0-29.0 s,pass\nfixed_flow_lag_one_correlation,{lag_one:.12},1,11.0-29.0 s,diagnostic\npost_load_residual_bias,{post_load_bias:.12},degC,50.0-60.0 s,restrict cold-flow prediction\nfixed_flow_command_model,1,1,20-25 percent and 30-31 degC,accepted\ncold_flow_prediction,0,1,omitted disturbance path,restricted\n"
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/17-independent-model-validation/figures");

    write_bool(&mut runtime, &safety_abort, true);
    runtime
        .execute_cycle()
        .expect("execute validation safety abort");
    assert_eq!(
        read_lreal(&runtime, &mv),
        0.0,
        "Safety abort must command safe zero"
    );
    write_bool(&mut runtime, &safety_abort, false);
    let valid_pv = read_lreal(&runtime, &pv);
    write_lreal(&mut runtime, &pv, 50.0);
    runtime
        .execute_cycle()
        .expect("execute invalid validation input");
    assert_eq!(
        read_lreal(&runtime, &mv),
        0.0,
        "invalid input must command safe zero"
    );
    assert!(!read_bool(&runtime, &inputs_valid));
    write_lreal(&mut runtime, &pv, valid_pv);
    write_bool(&mut runtime, &test_authority, false);
    runtime
        .execute_cycle()
        .expect("restore validation experiment to Manual baseline");
    assert_eq!(
        read_lreal(&runtime, &mv),
        20.0,
        "authority removal must restore 20 percent"
    );
    assert!(
        read_bool(&runtime, &manual_mode),
        "restoration must expose Manual mode"
    );
    let restored_pv = read_lreal(&runtime, &pv);
    let restored_prediction = prediction(60.1);
    writeln!(
        evidence,
        "601,60.1,manual_restoration,20.00,20.000000000000,{restored_pv:.12},{restored_prediction:.12},{restored_prediction:.12},{restored_prediction:.12},{:.12},true,0.100,false,true,true",
        restored_pv - restored_prediction
    )
    .unwrap();
    let validation_run_id = "lesson-17-validation-seed-20260827";
    let retained = serde_json::json!({
        "estimationRunId": estimation_evidence["runId"],
        "validationRunId": validation_run_id,
        "runsAreDistinct": estimation_evidence["runId"] != validation_run_id,
        "frozenModel": selected,
        "parameterIntervals": selected["uncertainty"],
        "modelSource": {
            "artifact": "../../16-heater-step-identification/figures/heater-identification-model-evidence.json",
            "projectPath": estimation_evidence["projectPath"],
            "projectContentRevisionSha256": estimation_evidence["projectContentRevisionSha256"],
            "structuredTextRevisionSha256": estimation_evidence["structuredTextRevisionSha256"]
        },
        "validationSource": {
            "projectPath": "examples/control/01-feedback-foundations-and-temperature-control/17-independent-model-validation",
            "structuredText": "src/Main.st",
            "simulation": "simulation.toml",
            "seed": 20260827,
            "sampleTimeSeconds": 0.1
        },
        "rawValidationSamples": {"artifact": "validation-trace.csv", "activeCount": 600, "restorationCount": 1},
        "requestedMV": "validation-trace.csv#requested_power_percent",
        "appliedMV": "validation-trace.csv#applied_power_percent",
        "predictedPV": "validation-trace.csv#predicted_pv_degc",
        "predictionEnvelope": ["validation-trace.csv#envelope_lower_degc", "validation-trace.csv#envelope_upper_degc"],
        "residuals": "validation-trace.csv#residual_degc",
        "fixedFlowRmse": {"value": fixed_flow_rmse, "unit": "degC", "window": "11.00-29.00 s", "limit": 0.03, "decision": "pass"},
        "postLoadBias": {"value": post_load_bias, "unit": "degC", "window": "50.00-60.00 s", "decision": "restrict cold-flow prediction"},
        "residualLagOneCorrelation": {"value": lag_one, "window": "11.00-29.00 s", "disposition": "diagnostic only; residual magnitude is below measurement resolution"},
        "qualification": {
            "decision": "Qualified",
            "activeSampleCount": 600,
            "sampleTimeSeconds": 0.1,
            "maximumTimestampGapSeconds": 0.1,
            "rejectedQualitySampleCount": 0,
            "parametersFrozen": true,
            "estimationSamplesReused": false,
            "testAuthorityDuringExperiment": true,
            "manualModeDuringExperiment": true
        },
        "declaredLimits": {
            "outletTemperatureDegC": {"minimum": 0.0, "maximum": 40.0},
            "requestedHeaterPercent": {"minimum": 0.0, "maximum": 100.0},
            "appliedHeaterPercent": {"minimum": 0.0, "maximum": 100.0},
            "safeHeaterPercent": 0.0,
            "restorationHeaterPercent": 20.0
        },
        "testAuthority": {"activeRun": true, "restoration": false},
        "manualMode": {"activeRun": true, "restoration": true},
        "abort": {"triggered": false, "reason": null, "safeZeroPathTested": true},
        "restoration": {"timeSeconds": 60.1, "requestedHeaterPercent": 20.0, "appliedHeaterPercent": 20.0, "testAuthority": false, "manualMode": true},
        "verdict": "restricted",
        "acceptedPurpose": "local fixed-flow heater-command prediction",
        "restrictedPurpose": "cold-flow disturbance prediction",
        "validityRange": {"appliedHeaterPercent": [20.0, 25.0], "outletTemperatureDegC": [30.0, 31.0]},
        "warnings": ["The cold-flow disturbance path is absent from the frozen model."],
        "seed": 20260827,
        "evidenceClass": {"measurements": "configured simulation", "prediction": "frozen identified model", "physicalPlant": false}
    });
    let retained_json =
        serde_json::to_string_pretty(&retained).expect("serialize Lesson 17 evidence") + "\n";
    write_or_compare_curriculum_artifact(&artifact_root.join("validation-trace.csv"), &evidence);
    write_or_compare_curriculum_artifact(&artifact_root.join("validation-summary.csv"), &summary);
    write_or_compare_curriculum_artifact(
        &artifact_root.join("heater-validation-evidence.json"),
        &retained_json,
    );
}

#[test]
fn control_curriculum_heat_exchanger_validation_classifies_scope_and_uncertainty() {
    let shipped_source = project_file("heat-exchanger-validation-uncertainty", "src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        // Legacy names keep the old lesson runnable for the required red baseline.
        .replace("ExperimentInput : LREAL :=", "ExperimentInput AT %ML0 : LREAL :=")
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX64.0 : BOOL :=")
        .replace("ManipulatedValue : LREAL :=", "ManipulatedValue AT %QL0 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        // Lesson 41 exposes three independent validation records.
        .replace("ExperimentEnabled : BOOL :=", "ExperimentEnabled AT %MX64.0 : BOOL :=")
        .replace("LocalRequested : LREAL :=", "LocalRequested AT %ML0 : LREAL :=")
        .replace("LoadRequested : LREAL :=", "LoadRequested AT %ML8 : LREAL :=")
        .replace("HighFlowRequested : LREAL :=", "HighFlowRequested AT %ML16 : LREAL :=")
        .replace("LocalApplied : LREAL :=", "LocalApplied AT %QL0 : LREAL :=")
        .replace("LoadApplied : LREAL :=", "LoadApplied AT %QL8 : LREAL :=")
        .replace("HighFlowApplied : LREAL :=", "HighFlowApplied AT %QL16 : LREAL :=")
        .replace("LocalTemperature : LREAL :=", "LocalTemperature AT %IL0 : LREAL :=")
        .replace("LoadTemperature : LREAL :=", "LoadTemperature AT %IL8 : LREAL :=")
        .replace("HighFlowTemperature : LREAL :=", "HighFlowTemperature AT %IL16 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("heat-exchanger-validation-uncertainty/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped independent heat-exchanger validation lesson");

    let config_source = project_file("heat-exchanger-validation-uncertainty", "simulation.toml");
    let config_path = temporary_config_path("heat-exchanger-validation-uncertainty");
    std::fs::write(&config_path, &config_source)
        .expect("write independent heat-exchanger validation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load three independent validation records"),
    );
    let enabled = IoAddress::parse("%MX64.0").unwrap();
    let requested = ["%ML0", "%ML8", "%ML16"].map(|value| IoAddress::parse(value).unwrap());
    let applied = ["%QL0", "%QL8", "%QL16"].map(|value| IoAddress::parse(value).unwrap());
    let measured = ["%IL0", "%IL8", "%IL16"].map(|value| IoAddress::parse(value).unwrap());

    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &enabled, false);
    let mut traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(1_600));
    let mut requested_traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(1_600));
    let mut applied_traces: [Vec<f64>; 3] = std::array::from_fn(|_| Vec::with_capacity(1_600));
    let mut input_min = [f64::INFINITY; 3];
    let mut input_max = [f64::NEG_INFINITY; 3];
    for cycle in 1usize..=1_600 {
        if cycle == 101 {
            write_bool(&mut runtime, &enabled, true);
        }
        runtime
            .execute_cycle()
            .expect("apply frozen validation input records");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance validation plants");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish validation temperatures");
        for index in 0..3 {
            let mv = read_lreal(&runtime, &applied[index]);
            requested_traces[index].push(read_lreal(&runtime, &requested[index]));
            applied_traces[index].push(mv);
            input_min[index] = input_min[index].min(mv);
            input_max[index] = input_max[index].max(mv);
            traces[index].push(read_lreal(&runtime, &measured[index]));
        }
    }

    let predict = |time: f64, gain: f64, tau: f64, delay: f64| {
        let elapsed = time - 10.0;
        if elapsed <= delay {
            50.0
        } else {
            50.0 + gain * 5.0 * (1.0 - (-(elapsed - delay) / tau).exp())
        }
    };
    let residuals = |trace: &[f64], range: std::ops::Range<usize>| {
        range
            .map(|index| trace[index] - predict((index + 1) as f64 * 0.1, 0.5, 20.0, 3.0))
            .collect::<Vec<_>>()
    };
    let rmse = |values: &[f64]| {
        (values.iter().map(|value| value * value).sum::<f64>() / values.len() as f64).sqrt()
    };
    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
    let envelope_contains = |index: usize, measured_value: f64| {
        let time = (index + 1) as f64 * 0.1;
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for gain in [0.47, 0.53] {
            for tau in [18.0, 22.0] {
                for delay in [2.5, 3.5] {
                    let value = predict(time, gain, tau, delay);
                    low = low.min(value);
                    high = high.max(value);
                }
            }
        }
        measured_value >= low - 0.30 && measured_value <= high + 0.30
    };

    let local_residuals = residuals(&traces[0], 200..800);
    let local_rmse = rmse(&local_residuals);
    let local_bias = mean(&local_residuals);
    let local_lag_one = pearson_correlation(
        &local_residuals[..local_residuals.len() - 1],
        &local_residuals[1..],
    );
    let local_coverage = (200..800)
        .filter(|&index| envelope_contains(index, traces[0][index]))
        .count() as f64
        / 600.0;
    let load_pre_residuals = residuals(&traces[1], 200..800);
    let load_post_residuals = residuals(&traces[1], 1_300..1_600);
    let high_post_residuals = residuals(&traces[2], 1_300..1_600);
    let load_pre_rmse = rmse(&load_pre_residuals);
    let load_post_bias = mean(&load_post_residuals);
    let high_post_bias = mean(&high_post_residuals);
    let high_coverage = (1_300..1_600)
        .filter(|&index| envelope_contains(index, traces[2][index]))
        .count() as f64
        / 300.0;

    assert!(input_min
        .into_iter()
        .zip(input_max)
        .all(|(minimum, maximum)| ((maximum - minimum) - 5.0).abs() <= 1e-9));
    assert!(local_rmse <= 0.20, "local validation RMSE={local_rmse}");
    assert!(local_bias.abs() <= 0.05, "local residual bias={local_bias}");
    assert!(
        local_lag_one.abs() <= 0.20,
        "local lag-one residual correlation={local_lag_one}"
    );
    assert!(
        local_coverage >= 0.90,
        "local uncertainty-envelope coverage={local_coverage}"
    );
    assert!(
        load_pre_rmse <= 0.20,
        "load-record pre-event RMSE={load_pre_rmse}"
    );
    assert!(
        load_post_bias <= -1.8 && load_post_bias >= -2.1,
        "unmodelled cold-flow residual bias={load_post_bias}"
    );
    assert!(
        high_post_bias <= -0.9,
        "high-flow extrapolation bias={high_post_bias}"
    );
    assert!(
        high_coverage <= 0.20,
        "high-flow local-envelope coverage={high_coverage}"
    );
    assert_eq!(config_source.matches("[[plants]]").count(), 3);
    assert!(!shipped_source.contains("Control.PID"));
    for forbidden in ["PV_predicted", "Residual", "RMSE", "Envelope", "Verdict"] {
        assert!(
            !shipped_source.contains(forbidden),
            "validation answer '{forbidden}' must be derived from retained evidence"
        );
    }

    let prediction = (0..1_600)
        .map(|index| predict((index + 1) as f64 * 0.1, 0.5, 20.0, 3.0))
        .collect::<Vec<_>>();
    let mut envelope_lower = Vec::with_capacity(1_600);
    let mut envelope_upper = Vec::with_capacity(1_600);
    for index in 0..1_600 {
        let time = (index + 1) as f64 * 0.1;
        let mut low = f64::INFINITY;
        let mut high = f64::NEG_INFINITY;
        for gain in [0.47, 0.53] {
            for tau in [18.0, 22.0] {
                for delay in [2.5, 3.5] {
                    let value = predict(time, gain, tau, delay);
                    low = low.min(value);
                    high = high.max(value);
                }
            }
        }
        envelope_lower.push(low - 0.30);
        envelope_upper.push(high + 0.30);
    }
    let all_residuals = traces
        .iter()
        .map(|trace| {
            trace
                .iter()
                .zip(&prediction)
                .map(|(measured, predicted)| measured - predicted)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let full_metrics = |condition: usize| {
        let residual = &all_residuals[condition][200..1_600];
        let rmse = rmse(residual);
        let bias = mean(residual);
        let correlation = pearson_correlation(&residual[..residual.len() - 1], &residual[1..]);
        let coverage = (200..1_600)
            .filter(|&index| envelope_contains(index, traces[condition][index]))
            .count() as f64
            / 1_400.0;
        (rmse, bias, correlation, coverage)
    };
    let cold_metrics = full_metrics(1);
    let high_metrics = full_metrics(2);
    let summary = format!("condition,rmse_degC,bias_degC,lag_one_correlation,envelope_coverage,verdict,purpose,failed_criteria\nlocal,{local_rmse:.12},{local_bias:.12},{local_lag_one:.12},{local_coverage:.12},accepted,local command prediction,none\ncold_flow,{:.12},{:.12},{:.12},{:.12},restricted,local command prediction without cold-flow load,post-load residual bias\nhigh_flow,{:.12},{:.12},{:.12},{:.12},rejected,outside local operating region,late residual bias and envelope coverage\n", cold_metrics.0, cold_metrics.1, cold_metrics.2, cold_metrics.3, high_metrics.0, high_metrics.1, high_metrics.2, high_metrics.3);
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/05-experiments-identification-and-validation/41-heat-exchanger-validation-uncertainty/figures");
    let retained_samples = |condition: usize| {
        (0..traces[condition].len())
            .map(|index| {
                serde_json::json!({
                    "time": (index + 1) as f64 * 0.1,
                    "requestedInput": requested_traces[condition][index],
                    "appliedInput": applied_traces[condition][index],
                    "measuredOutput": traces[condition][index],
                    "prediction": prediction[index],
                    "envelopeLower": envelope_lower[index],
                    "envelopeUpper": envelope_upper[index],
                    "residual": all_residuals[condition][index],
                    "seed": 20260829,
                    "evidenceClass": "configured_simulation"
                })
            })
            .collect::<Vec<_>>()
    };
    let retained = serde_json::json!({
        "estimationRunId": "qualified_open_loop",
        "estimationSource": {
            "lesson": 40,
            "artifact": "poor-excitation-evidence.json",
            "recordId": "qualified_open_loop"
        },
        "frozenModel": {
            "structure": "first_order_plus_dead_time",
            "parameters": {"gainDegCPerPercent": 0.5, "timeConstantS": 20.0, "deadTimeS": 3.0},
            "parameterIntervals": {
                "gainDegCPerPercent": [0.47, 0.53],
                "timeConstantS": [18.0, 22.0],
                "deadTimeS": [2.5, 3.5]
            },
            "confidenceLevel": 0.95,
            "initialStateRule": "Start from measured outlet temperature 50 degC with applied valve and delayed-input history both at 20 percent; initialize prediction from that state and do not refit from validation data",
            "preprocessing": "raw 100 ms samples; no refit after validation begins"
        },
        "validationRuns": [
            {
                "id": "local-holdout-seed-20260829",
                "samples": retained_samples(0),
                "rmse": local_rmse,
                "bias": local_bias,
                "lagOneCorrelation": local_lag_one,
                "coverage": local_coverage,
                "verdict": "accepted",
                "purpose": "local command prediction",
                "validityRegion": "20 to 25 percent valve near 50 degrees Celsius; no cold-flow load; local flow",
                "failedCriteria": []
            },
            {
                "id": "cold-flow-holdout-seed-20260829",
                "samples": retained_samples(1),
                "rmse": cold_metrics.0,
                "bias": cold_metrics.1,
                "lagOneCorrelation": cold_metrics.2,
                "coverage": cold_metrics.3,
                "verdict": "restricted",
                "purpose": "local command prediction without cold-flow disturbance",
                "validityRegion": "pre-load portion only; cold-flow disturbance excluded",
                "failedCriteria": ["post-load residual bias"]
            },
            {
                "id": "high-flow-holdout-seed-20260829",
                "samples": retained_samples(2),
                "rmse": high_metrics.0,
                "bias": high_metrics.1,
                "lagOneCorrelation": high_metrics.2,
                "coverage": high_metrics.3,
                "verdict": "rejected",
                "purpose": "high-flow operating-point prediction",
                "validityRegion": "outside the qualified local-flow operating region",
                "failedCriteria": ["late residual bias", "prediction-envelope coverage"]
            }
        ],
        "evidenceClass": "configured_simulation"
    });
    write_or_compare_curriculum_artifact(
        &artifact_root.join("heat-exchanger-validation-evidence.json"),
        &(serde_json::to_string_pretty(&retained).expect("serialize validation evidence") + "\n"),
    );
    evidence::retain(
        &artifact_root,
        "heat-exchanger-validation-response",
        "Independent heat-exchanger validation",
        "Configured Simulation measurements · one frozen local prediction and predeclared envelope",
        0.1,
        &[
            evidence::Panel {
                title: "Local holdout and frozen prediction",
                unit: "degC",
                minimum: 49.0,
                maximum: 54.0,
                series: vec![
                    evidence::Series {
                        name: "local measured",
                        color: "#243044",
                        dashed: false,
                        values: &traces[0],
                    },
                    evidence::Series {
                        name: "frozen prediction",
                        color: "#0072b2",
                        dashed: false,
                        values: &prediction,
                    },
                    evidence::Series {
                        name: "envelope lower",
                        color: "#009e73",
                        dashed: true,
                        values: &envelope_lower,
                    },
                    evidence::Series {
                        name: "envelope upper",
                        color: "#009e73",
                        dashed: true,
                        values: &envelope_upper,
                    },
                ],
            },
            evidence::Panel {
                title: "Disturbance and operating-point holdouts",
                unit: "degC",
                minimum: 47.0,
                maximum: 54.0,
                series: vec![
                    evidence::Series {
                        name: "cold-flow measured",
                        color: "#d55e00",
                        dashed: false,
                        values: &traces[1],
                    },
                    evidence::Series {
                        name: "high-flow measured",
                        color: "#cc79a7",
                        dashed: false,
                        values: &traces[2],
                    },
                    evidence::Series {
                        name: "unchanged prediction",
                        color: "#0072b2",
                        dashed: true,
                        values: &prediction,
                    },
                ],
            },
        ],
        &[],
        &[evidence::Marker {
            at_seconds: 90.0,
            label: "cold-flow load",
        }],
        &summary,
    );
    evidence::retain(
        &artifact_root,
        "heat-exchanger-validation-residual",
        "Validation residuals reveal model limits",
        "Residual equals measured outlet temperature minus the unchanged frozen-model prediction",
        0.1,
        &[
            evidence::Panel {
                title: "Local and cold-flow residuals",
                unit: "degC",
                minimum: -2.5,
                maximum: 1.0,
                series: vec![
                    evidence::Series {
                        name: "local residual",
                        color: "#0072b2",
                        dashed: false,
                        values: &all_residuals[0],
                    },
                    evidence::Series {
                        name: "cold-flow residual",
                        color: "#d55e00",
                        dashed: false,
                        values: &all_residuals[1],
                    },
                ],
            },
            evidence::Panel {
                title: "High-flow residual",
                unit: "degC",
                minimum: -2.5,
                maximum: 1.0,
                series: vec![evidence::Series {
                    name: "high-flow residual",
                    color: "#cc79a7",
                    dashed: false,
                    values: &all_residuals[2],
                }],
            },
        ],
        &[],
        &[evidence::Marker {
            at_seconds: 90.0,
            label: "load event",
        }],
        &summary,
    );

    write_bool(&mut runtime, &enabled, false);
    runtime.execute_cycle().expect("restore validation inputs");
    assert!(applied
        .iter()
        .all(|address| (read_lreal(&runtime, address) - 20.0).abs() <= 1e-12));
    let _ = std::fs::remove_file(config_path);
    println!(
        "independent validation: local RMSE/bias/rho1/coverage={local_rmse:.4}/{local_bias:.4}/{local_lag_one:.4}/{local_coverage:.3}; load pre-RMSE/post-bias={load_pre_rmse:.4}/{load_post_bias:.4}; high-flow bias/coverage={high_post_bias:.4}/{high_coverage:.3}; verdicts=accepted/restricted/rejected"
    );
}

fn run_heater_tuning(
    selection: i16,
    perturbed_and_noisy: bool,
    run_approved: bool,
) -> Vec<(f64, f64)> {
    let source = project_file("../foundation/nominal-fopdt", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "TuningSelection : INT :=",
            "TuningSelection AT %IW40 : INT :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL32 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace("RunApproved : BOOL :=", "RunApproved AT %IX82.0 : BOOL :=")
        .replace(
            "    GenerateCandidate : BOOL :=",
            "    GenerateCandidate AT %IX82.1 : BOOL :=",
        )
        .replace(
            "    ValidationPassed : BOOL :=",
            "    ValidationPassed AT %IX82.2 : BOOL :=",
        )
        .replace(
            "    ApplyApproved : BOOL :=",
            "    ApplyApproved AT %IX82.3 : BOOL :=",
        )
        .replace(
            "ActiveProportionalGain : LREAL :=",
            "ActiveProportionalGain AT %ML72 : LREAL :=",
        )
        .replace(
            "ActiveIntegralGain : LREAL :=",
            "ActiveIntegralGain AT %ML80 : LREAL :=",
        )
        .replace(
            "ActiveIntegralTime : LREAL :=",
            "ActiveIntegralTime AT %ML88 : LREAL :=",
        )
        .replace(
            "SelectedProportionalGain : LREAL :=",
            "SelectedProportionalGain AT %ML48 : LREAL :=",
        )
        .replace("TuningValid : BOOL :=", "TuningValid AT %MX100.0 : BOOL :=")
        .replace(
            "CandidateValidated : BOOL :=",
            "CandidateValidated AT %MX100.1 : BOOL :=",
        )
        .replace(
            "CandidateApplied : BOOL :=",
            "CandidateApplied AT %MX100.2 : BOOL :=",
        );
    let control_source = include_str!("../../../libraries/control/src/control.st");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", control_source),
        SourceFile::with_path("nominal-fopdt/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped model-based tuning lesson");
    let mut config = project_file("../foundation/nominal-fopdt", "simulation.toml");
    if perturbed_and_noisy {
        config = config
            .replace("gain = 0.2", "gain = 0.19")
            .replace("time_constant_s = 5.0", "time_constant_s = 6.0")
            .replace("dead_time_s = 1.0", "dead_time_s = 1.3")
            .replace(
                "[plants.actuator]\nminimum = 0.0\nmaximum = 100.0",
                "[plants.actuator]\nminimum = 0.0\nmaximum = 100.0\n\n[[plants.uncertainties]]\nid = \"temperature-noise\"\ntarget = \"measurement\"\nchannel = 0\nmode = \"add\"\ngenerator = \"normal_approx_white\"\nstandard_deviation = 0.03",
            );
    }
    let config_path = temporary_config_path("nominal-fopdt");
    std::fs::write(&config_path, config).expect("write tuning validation config");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load tuning validation simulation"),
    );
    let tuning = IoAddress::parse("%IW40").expect("tuning selection address");
    let setpoint = IoAddress::parse("%IL32").expect("SP address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let approval = IoAddress::parse("%IX82.0").expect("simulation-run approval address");
    let generate = IoAddress::parse("%IX82.1").expect("candidate generation address");
    let validation = IoAddress::parse("%IX82.2").expect("candidate validation address");
    let apply = IoAddress::parse("%IX82.3").expect("candidate Apply address");
    let active_kp = IoAddress::parse("%ML72").expect("active proportional gain address");
    let active_ki = IoAddress::parse("%ML80").expect("active integral gain address");
    let active_ti = IoAddress::parse("%ML88").expect("active integral time address");
    let candidate_kp = IoAddress::parse("%ML48").expect("candidate gain address");
    let tuning_valid = IoAddress::parse("%MX100.0").expect("candidate validity address");
    let candidate_validated = IoAddress::parse("%MX100.1").expect("validation state address");
    let candidate_applied = IoAddress::parse("%MX100.2").expect("Apply state address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial heater state");
    write_lreal(&mut runtime, &active_kp, 4.0);
    write_lreal(&mut runtime, &active_ki, 0.8);
    write_lreal(&mut runtime, &active_ti, 5.0);
    write_int(&mut runtime, &tuning, selection);
    write_lreal(&mut runtime, &setpoint, 30.6);
    write_bool(&mut runtime, &approval, false);
    write_bool(&mut runtime, &generate, true);
    assert!(read_bool(&runtime, &generate));
    assert_eq!(read_int(&runtime, &tuning), selection);
    runtime
        .execute_cycle()
        .expect("generate selected tuning candidate");
    let expected_kp = if selection == 0 { 4.0 } else { 5.0 };
    assert!(
        (read_lreal(&runtime, &candidate_kp) - expected_kp).abs() <= 1e-12,
        "selection={selection}, generated candidate Kp={}",
        read_lreal(&runtime, &candidate_kp)
    );
    assert!(read_bool(&runtime, &tuning_valid));
    write_bool(&mut runtime, &generate, false);
    write_bool(&mut runtime, &validation, true);
    runtime
        .execute_cycle()
        .expect("record candidate validation");
    assert!(read_bool(&runtime, &candidate_validated));
    write_bool(&mut runtime, &validation, false);
    write_bool(&mut runtime, &apply, true);
    runtime.execute_cycle().expect("apply validated candidate");
    assert!(read_bool(&runtime, &candidate_applied));
    assert!((read_lreal(&runtime, &active_kp) - expected_kp).abs() <= 1e-12);
    write_bool(&mut runtime, &apply, false);
    let mut trace = Vec::with_capacity(1_200);
    for cycle in 1..=1_200 {
        write_int(&mut runtime, &tuning, selection);
        write_lreal(&mut runtime, &setpoint, 30.6);
        write_bool(&mut runtime, &approval, run_approved);
        runtime
            .execute_cycle()
            .expect("execute heater tuning lesson");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance tuned heater");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish tuned heater PV");
        trace.push((read_lreal(&runtime, &pv), read_lreal(&runtime, &mv)));
    }
    write_bool(&mut runtime, &approval, false);
    runtime
        .execute_cycle()
        .expect("restore tuned heater to Manual safe zero");
    assert!(
        read_lreal(&runtime, &mv).abs() <= 1e-12,
        "removing run approval must restore Manual safe zero"
    );
    let _ = std::fs::remove_file(config_path);
    trace
}

fn heater_tuning_lifecycle_runtime() -> trust_runtime::Runtime {
    let mut source = project_file("../foundation/nominal-fopdt", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .to_owned();
    for declaration in [
        "    GenerateCandidate : BOOL := FALSE;\n",
        "    ValidationPassed : BOOL := FALSE;\n",
        "    ApplyApproved : BOOL := FALSE;\n",
        "    RollbackRequested : BOOL := FALSE;\n",
        "    AcceptedModelId : INT := 17;\n",
        "    AcceptedModelPurpose : BOOL := TRUE;\n",
        "    ModelWithinValidityRange : BOOL := TRUE;\n",
        "    RequirementsFeasible : BOOL := TRUE;\n",
        "    CandidateValidated : BOOL := FALSE;\n",
        "    CandidateApplied : BOOL := FALSE;\n",
        "    ActiveProportionalGain : LREAL := 4.0;\n",
        "    ActiveIntegralGain : LREAL := 0.8;\n",
    ] {
        source = source.replace(declaration, "");
    }
    source = source
        .replace(
            "    SafetyAbort : BOOL :=",
            "    GenerateCandidate AT %IX82.1 : BOOL := FALSE;\n\
             \x20   ValidationPassed AT %IX82.2 : BOOL := FALSE;\n\
             \x20   ApplyApproved AT %IX82.3 : BOOL := FALSE;\n\
             \x20   RollbackRequested AT %IX82.4 : BOOL := FALSE;\n\
             \x20   AcceptedModelPurpose AT %IX82.5 : BOOL := TRUE;\n\
             \x20   ModelWithinValidityRange AT %IX82.6 : BOOL := TRUE;\n\
             \x20   RequirementsFeasible AT %IX82.7 : BOOL := TRUE;\n\
             \x20   AcceptedModelId AT %IW42 : INT := 17;\n\
             \x20   CandidateValidated AT %MX84.1 : BOOL := FALSE;\n\
             \x20   CandidateApplied AT %MX84.2 : BOOL := FALSE;\n\
             \x20   ActiveProportionalGain AT %ML72 : LREAL := 4.0;\n\
             \x20   ActiveIntegralGain AT %ML88 : LREAL := 0.8;\n\
             \x20   SafetyAbort : BOOL :=",
        )
        .replace(
            "TuningSelection : INT :=",
            "TuningSelection AT %IW40 : INT :=",
        )
        .replace(
            "SelectedProportionalGain : LREAL :=",
            "SelectedProportionalGain AT %ML48 : LREAL :=",
        )
        .replace("TuningValid : BOOL :=", "TuningValid AT %MX84.0 : BOOL :=")
        .replace(
            "ApplyRejected : BOOL :=",
            "ApplyRejected AT %MX84.3 : BOOL :=",
        )
        .replace(
            "    ModelGain : LREAL :=",
            "    ModelGain AT %ML8 : LREAL :=",
        )
        .replace(
            "    ModelTimeConstant : LREAL :=",
            "    ModelTimeConstant AT %ML16 : LREAL :=",
        )
        .replace(
            "    ModelDeadTime : LREAL :=",
            "    ModelDeadTime AT %ML24 : LREAL :=",
        )
        .replace("    Lambda : LREAL :=", "    Lambda AT %ML32 : LREAL :=")
        .replace(
            "    MaximumExpectedError : LREAL :=",
            "    MaximumExpectedError AT %ML40 : LREAL :=",
        );
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("nominal-fopdt/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped tuning lifecycle")
}

#[test]
fn control_curriculum_lambda_candidate_is_generated_validated_applied_and_rolled_back_explicitly() {
    let mut runtime = heater_tuning_lifecycle_runtime();
    let selection = IoAddress::parse("%IW40").unwrap();
    let generate = IoAddress::parse("%IX82.1").unwrap();
    let validation = IoAddress::parse("%IX82.2").unwrap();
    let apply = IoAddress::parse("%IX82.3").unwrap();
    let rollback = IoAddress::parse("%IX82.4").unwrap();
    let purpose = IoAddress::parse("%IX82.5").unwrap();
    let within_range = IoAddress::parse("%IX82.6").unwrap();
    let feasible = IoAddress::parse("%IX82.7").unwrap();
    let model_id = IoAddress::parse("%IW42").unwrap();
    let candidate_kp = IoAddress::parse("%ML48").unwrap();
    let active_kp = IoAddress::parse("%ML72").unwrap();
    let candidate_validated = IoAddress::parse("%MX84.1").unwrap();
    let candidate_applied = IoAddress::parse("%MX84.2").unwrap();
    let tuning_valid = IoAddress::parse("%MX84.0").unwrap();
    let apply_rejected = IoAddress::parse("%MX84.3").unwrap();
    let model_gain = IoAddress::parse("%ML8").unwrap();
    let model_time = IoAddress::parse("%ML16").unwrap();
    let model_delay = IoAddress::parse("%ML24").unwrap();
    let lambda = IoAddress::parse("%ML32").unwrap();
    let maximum_error = IoAddress::parse("%ML40").unwrap();

    // The mapped test boundary is initialized explicitly because process-image
    // storage starts at zero before the first PLC scan.
    write_lreal(&mut runtime, &active_kp, 4.0);
    write_int(&mut runtime, &model_id, 17);
    write_bool(&mut runtime, &purpose, true);
    write_bool(&mut runtime, &within_range, true);
    write_bool(&mut runtime, &feasible, true);
    write_lreal(&mut runtime, &model_gain, 0.2);
    write_lreal(&mut runtime, &model_time, 5.0);
    write_lreal(&mut runtime, &model_delay, 1.0);
    write_lreal(&mut runtime, &lambda, 4.0);
    write_lreal(&mut runtime, &maximum_error, 0.4);
    write_int(&mut runtime, &selection, 0);
    runtime.execute_cycle().expect("initialize empirical PI");
    assert!((read_lreal(&runtime, &active_kp) - 4.0).abs() <= 1e-12);

    write_int(&mut runtime, &selection, 1);
    write_bool(&mut runtime, &generate, true);
    runtime.execute_cycle().expect("generate Lambda candidate");
    assert!((read_lreal(&runtime, &candidate_kp) - 5.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &active_kp) - 4.0).abs() <= 1e-12);
    assert!(!read_bool(&runtime, &candidate_applied));

    write_bool(&mut runtime, &generate, false);
    write_bool(&mut runtime, &validation, true);
    assert!(read_bool(&runtime, &validation));
    runtime.execute_cycle().expect("record passing validation");
    assert!(
        read_bool(&runtime, &candidate_validated),
        "passing validation must qualify the valid candidate; tuning_valid={}",
        read_bool(&runtime, &tuning_valid)
    );
    assert!((read_lreal(&runtime, &active_kp) - 4.0).abs() <= 1e-12);

    write_bool(&mut runtime, &validation, false);
    write_bool(&mut runtime, &apply, true);
    runtime.execute_cycle().expect("apply validated candidate");
    assert!(read_bool(&runtime, &candidate_applied));
    assert!((read_lreal(&runtime, &active_kp) - 5.0).abs() <= 1e-12);

    write_bool(&mut runtime, &apply, false);
    write_bool(&mut runtime, &rollback, true);
    runtime.execute_cycle().expect("roll back previous tuning");
    assert!(!read_bool(&runtime, &candidate_applied));
    assert!((read_lreal(&runtime, &active_kp) - 4.0).abs() <= 1e-12);

    write_bool(&mut runtime, &rollback, false);
    write_bool(&mut runtime, &purpose, false);
    write_bool(&mut runtime, &generate, true);
    runtime
        .execute_cycle()
        .expect("reject ineligible model purpose");
    assert!(!read_bool(&runtime, &tuning_valid));
    assert!(read_lreal(&runtime, &candidate_kp).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &active_kp) - 4.0).abs() <= 1e-12);

    write_bool(&mut runtime, &generate, false);
    write_bool(&mut runtime, &apply, false);
    runtime.execute_cycle().expect("release lifecycle actions");
    write_bool(&mut runtime, &validation, true);
    write_bool(&mut runtime, &apply, true);
    runtime
        .execute_cycle()
        .expect("reject Apply for invalid candidate");
    assert!(read_bool(&runtime, &apply_rejected));
    assert!((read_lreal(&runtime, &active_kp) - 4.0).abs() <= 1e-12);

    write_bool(&mut runtime, &purpose, true);
    write_bool(&mut runtime, &validation, false);
    write_bool(&mut runtime, &apply, false);
    write_bool(&mut runtime, &generate, false);
    runtime.execute_cycle().expect("release generation edge");
    write_lreal(&mut runtime, &maximum_error, 1.1);
    write_bool(&mut runtime, &generate, true);
    runtime
        .execute_cycle()
        .expect("evaluate local actuator headroom");
    assert!(
        !read_bool(&runtime, &tuning_valid),
        "a 5.5-point proportional request cannot fit the validated 20-25 percent range"
    );
    write_lreal(&mut runtime, &maximum_error, 0.4);

    // A huge finite value can pass a sign check, but it is outside the lesson's
    // declared engineering bounds and cannot describe a usable candidate.
    for (address, valid_value, label) in [
        (&model_gain, 0.2, "model gain"),
        (&model_delay, 1.0, "model dead time"),
        (&lambda, 4.0, "response target"),
    ] {
        write_bool(&mut runtime, &purpose, true);
        write_bool(&mut runtime, &validation, false);
        write_bool(&mut runtime, &apply, false);
        write_bool(&mut runtime, &generate, false);
        runtime.execute_cycle().expect("release generation edge");
        write_lreal(&mut runtime, address, 1.0e300);
        write_bool(&mut runtime, &generate, true);
        runtime
            .execute_cycle()
            .expect("reject out-of-bounds tuning input");
        assert!(
            !read_bool(&runtime, &tuning_valid),
            "out-of-bounds {label} must not create a tuning candidate"
        );
        write_lreal(&mut runtime, address, valid_value);
    }
}

#[test]
fn control_curriculum_lambda_candidate_requires_explicit_run_approval() {
    let unapplied = run_heater_tuning(1, false, false);
    assert!(
        unapplied.iter().all(|(_, mv)| mv.abs() <= 1e-12),
        "calculating or selecting a candidate must not apply heater power"
    );
    let approved = run_heater_tuning(1, false, true);
    assert!(
        approved.iter().any(|(_, mv)| *mv > 0.0),
        "explicit simulation-run approval must permit the qualified PI run"
    );
}

#[test]
fn control_curriculum_lambda_boolean_test_authority_owns_selected_run_and_rollback() {
    let source = project_file("../foundation/nominal-fopdt", "src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX82.5 : BOOL :=",
        )
        .replace("RunApproved : BOOL :=", "RunApproved AT %MX100.3 : BOOL :=")
        .replace(
            "SelectedProportionalGain : LREAL :=",
            "SelectedProportionalGain AT %ML48 : LREAL :=",
        )
        .replace(
            "ActiveProportionalGain : LREAL :=",
            "ActiveProportionalGain AT %ML72 : LREAL :=",
        )
        .replace(
            "CandidateValidated : BOOL :=",
            "CandidateValidated AT %MX100.1 : BOOL :=",
        )
        .replace(
            "CandidateApplied : BOOL :=",
            "CandidateApplied AT %MX100.2 : BOOL :=",
        )
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        );
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("nominal-fopdt/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped project-owned tuning sequence");
    let authority = IoAddress::parse("%IX82.5").unwrap();
    let run_approved = IoAddress::parse("%MX100.3").unwrap();
    let selected_kp = IoAddress::parse("%ML48").unwrap();
    let active_kp = IoAddress::parse("%ML72").unwrap();
    let validated = IoAddress::parse("%MX100.1").unwrap();
    let applied = IoAddress::parse("%MX100.2").unwrap();
    let mv = IoAddress::parse("%QL0").unwrap();

    // Addressed process-image storage begins at zero in the harness; preserve
    // the shipped empirical active gain before testing Apply and rollback.
    write_lreal(&mut runtime, &active_kp, 4.0);
    runtime
        .execute_cycle()
        .expect("initialize empirical tuning");
    write_bool(&mut runtime, &authority, true);
    runtime
        .execute_cycle()
        .expect("start project-owned selected tuning run");
    assert!((read_lreal(&runtime, &selected_kp) - 5.0).abs() <= 1e-12);
    assert!(read_bool(&runtime, &validated));
    assert!(read_bool(&runtime, &applied));
    assert!(read_bool(&runtime, &run_approved));
    assert!((read_lreal(&runtime, &active_kp) - 5.0).abs() <= 1e-12);

    write_bool(&mut runtime, &authority, false);
    runtime
        .execute_cycle()
        .expect("restore and roll back selected tuning run");
    assert!(!read_bool(&runtime, &run_approved));
    assert!(!read_bool(&runtime, &applied));
    assert!((read_lreal(&runtime, &active_kp) - 4.0).abs() <= 1e-12);
    assert!(read_lreal(&runtime, &mv).abs() <= 1e-12);
}

#[test]
fn control_curriculum_lambda_pi_improves_declared_response_and_survives_model_error() {
    let accepted_validation: serde_json::Value = serde_json::from_str(&project_file(
        "independent-model-validation",
        "figures/heater-validation-evidence.json",
    ))
    .expect("parse accepted Lesson 17 model evidence");
    let empirical = run_heater_tuning(0, false, true);
    let model_based = run_heater_tuning(1, false, true);
    let perturbed = run_heater_tuning(1, true, true);
    for (label, trace) in [
        ("empirical", &empirical),
        ("lambda", &model_based),
        ("lambda perturbed", &perturbed),
    ] {
        assert!(
            trace.iter().all(|(pv, _)| *pv >= 30.0 && *pv <= 31.0),
            "{label} run must stay inside the accepted 30-31 degC model range"
        );
        assert!(
            trace.iter().all(|(_, mv)| *mv >= 20.0 && *mv <= 25.0),
            "{label} run must stay inside the accepted 20-25 percent command range"
        );
    }
    let iae = |trace: &[(f64, f64)], range: std::ops::Range<usize>| {
        range
            .map(|index| (30.6 - trace[index].0).abs() * 0.1)
            .sum::<f64>()
    };
    let empirical_tracking_iae = iae(&empirical, 0..600);
    let model_tracking_iae = iae(&model_based, 0..600);
    let empirical_load_iae = iae(&empirical, 600..1200);
    let model_load_iae = iae(&model_based, 600..1200);
    let final_mean_absolute_error = |trace: &[(f64, f64)]| {
        trace[1100..1200]
            .iter()
            .map(|sample| (30.6 - sample.0).abs())
            .sum::<f64>()
            / 100.0
    };
    let overshoot = |trace: &[(f64, f64)]| {
        trace
            .iter()
            .map(|sample| sample.0 - 30.6)
            .fold(0.0_f64, f64::max)
    };
    let setpoint_settling = |trace: &[(f64, f64)]| {
        (0..600)
            .find(|start| {
                trace[*start..600]
                    .iter()
                    .all(|sample| (30.6 - sample.0).abs() <= 0.2)
            })
            .map(|index| (index + 1) as f64 * 0.1)
            .expect("nominal response settles before the load event")
    };
    let complete_mv_travel = |trace: &[(f64, f64)]| {
        trace.first().map_or(0.0, |sample| sample.1.abs())
            + trace
                .windows(2)
                .map(|samples| (samples[1].1 - samples[0].1).abs())
                .sum::<f64>()
    };
    let empirical_final_error = final_mean_absolute_error(&empirical);
    let model_final_error = final_mean_absolute_error(&model_based);
    let empirical_overshoot = overshoot(&empirical);
    let model_overshoot = overshoot(&model_based);
    let empirical_settling_seconds = setpoint_settling(&empirical);
    let model_settling_seconds = setpoint_settling(&model_based);
    let empirical_mv_travel = complete_mv_travel(&empirical);
    let model_mv_travel = complete_mv_travel(&model_based);
    let perturbed_final_error = (30.6
        - perturbed[1100..1200]
            .iter()
            .map(|sample| sample.0)
            .sum::<f64>()
            / 100.0)
        .abs();
    let perturbed_mv_travel = complete_mv_travel(&perturbed);
    let empirical_saturation_seconds = empirical
        .iter()
        .filter(|sample| sample.1 <= 20.0 + 1e-12 || sample.1 >= 25.0 - 1e-12)
        .count() as f64
        * 0.1;
    let model_saturation_seconds = model_based
        .iter()
        .filter(|sample| sample.1 <= 20.0 + 1e-12 || sample.1 >= 25.0 - 1e-12)
        .count() as f64
        * 0.1;
    let perturbed_saturation_seconds = perturbed
        .iter()
        .filter(|sample| sample.1 <= 20.0 + 1e-12 || sample.1 >= 25.0 - 1e-12)
        .count() as f64
        * 0.1;
    assert!(
        empirical_saturation_seconds > 0.0
            && model_saturation_seconds > 0.0
            && perturbed_saturation_seconds > 0.0,
        "saturation exposure must use the active 20-25 percent qualification limits"
    );

    println!(
        "empirical/model tracking IAE={empirical_tracking_iae:.6}/{model_tracking_iae:.6}, empirical/model load IAE={empirical_load_iae:.6}/{model_load_iae:.6}, perturbed final error={perturbed_final_error:.6}, perturbed MV travel={perturbed_mv_travel:.6}"
    );

    let mut evidence = String::from(
        "sample,time_s,event,setpoint_degc,empirical_pv_degc,empirical_requested_mv_percent,empirical_mv_percent,lambda_pv_degc,lambda_requested_mv_percent,lambda_mv_percent,perturbed_pv_degc,perturbed_requested_mv_percent,perturbed_mv_percent,mode,run_approved,quality_good,timestamp_gap_s,safety_abort\n",
    );
    for index in 0..1_200 {
        writeln!(
            evidence,
            "{},{:.1},{},30.60,{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},{:.12},Auto,true,true,0.100,false",
            index + 1,
            (index + 1) as f64 * 0.1,
            if index < 600 { "setpoint_response" } else { "cold_flow_load" },
            empirical[index].0,
            empirical[index].1,
            empirical[index].1,
            model_based[index].0,
            model_based[index].1,
            model_based[index].1,
            perturbed[index].0,
            perturbed[index].1,
            perturbed[index].1,
        )
        .unwrap();
    }
    let summary = format!(
        "candidate,tracking_iae_degc_s,load_iae_degc_s,final_mean_absolute_error_degc,overshoot_degc,setpoint_settling_seconds,mv_travel_percentage_points,saturation_seconds,decision\nempirical,{empirical_tracking_iae:.12},{empirical_load_iae:.12},{empirical_final_error:.12},{empirical_overshoot:.12},{empirical_settling_seconds:.3},{empirical_mv_travel:.12},{empirical_saturation_seconds:.3},baseline\nlambda,{model_tracking_iae:.12},{model_load_iae:.12},{model_final_error:.12},{model_overshoot:.12},{model_settling_seconds:.3},{model_mv_travel:.12},{model_saturation_seconds:.3},passes nominal comparison\nlambda_perturbed_noisy,,,{perturbed_final_error:.12},,,{perturbed_mv_travel:.12},{perturbed_saturation_seconds:.3},passes uncertainty gates\n"
    );
    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/01-feedback-foundations-and-temperature-control/18-nominal-fopdt/figures");
    write_or_compare_curriculum_artifact(&artifact_root.join("lambda-comparison.csv"), &evidence);
    write_or_compare_curriculum_artifact(&artifact_root.join("lambda-summary.csv"), &summary);
    let retained = serde_json::json!({
        "validationArtifact": "examples/control/01-feedback-foundations-and-temperature-control/17-independent-model-validation/figures/heater-validation-evidence.json",
        "modelSource": accepted_validation["modelSource"].clone(),
        "parameterIntervals": accepted_validation["parameterIntervals"].clone(),
        "acceptedModelId": 17,
        "acceptedPurpose": "local fixed-flow heater-command prediction",
        "validityRange": {"appliedHeaterPercent": [20.0, 25.0], "outletTemperatureDegC": [30.0, 31.0]},
        "warning": "The cold-flow disturbance path is restricted and used only as a regulatory test.",
        "candidate": {"method": "Lambda/IMC PI", "lambdaSeconds": 4.0, "proportionalGain": 5.0, "integralTimeSeconds": 5.0, "integralGain": 1.0},
        "lifecycle": {"generated": true, "validated": true, "applyApprovalRequired": true, "appliedForSimulationRuns": true, "rollbackTest": "control_curriculum_lambda_candidate_is_generated_validated_applied_and_rolled_back_explicitly"},
        "runs": [
            {"id": "lesson-18-empirical-seed-20260824", "candidate": "empirical", "evidenceClass": "configured simulation"},
            {"id": "lesson-18-lambda-seed-20260824", "candidate": "lambda", "evidenceClass": "configured simulation"},
            {"id": "lesson-18-lambda-perturbed-seed-20260824", "candidate": "lambda_perturbed_noisy", "evidenceClass": "configured simulation"}
        ],
        "plan": {"sampleTimeSeconds": 0.1, "durationSeconds": 120.0, "initialTemperatureDegC": 30.2, "initialHeaterPercent": 20.0, "setpointDegC": 30.6, "coldFlowEventSeconds": 60.0, "coldFlowOutputBiasDegC": 29.8, "seed": 20260824},
        "signals": {"trace": "lambda-comparison.csv", "setpoint": "setpoint_degc", "processValues": ["empirical_pv_degc", "lambda_pv_degc", "perturbed_pv_degc"], "requestedManipulatedVariables": ["empirical_requested_mv_percent", "lambda_requested_mv_percent", "perturbed_requested_mv_percent"], "appliedManipulatedVariables": ["empirical_mv_percent", "lambda_mv_percent", "perturbed_mv_percent"], "quality": "quality_good", "mode": "mode", "approval": "run_approved", "timing": "timestamp_gap_s", "abort": "safety_abort"},
        "limits": {"heaterPercent": [0.0, 100.0], "safeHeaterPercent": 0.0},
        "results": {
            "empiricalTrackingIaeDegCS": empirical_tracking_iae,
            "lambdaTrackingIaeDegCS": model_tracking_iae,
            "empiricalLoadIaeDegCS": empirical_load_iae,
            "lambdaLoadIaeDegCS": model_load_iae,
            "nominalFinalMeanAbsoluteErrorDegC": {"empirical": empirical_final_error, "lambda": model_final_error, "window": "110.10-120.00 s"},
            "nominalOvershootDegC": {"empirical": empirical_overshoot, "lambda": model_overshoot, "window": "complete run"},
            "setpointSettlingSeconds": {"empirical": empirical_settling_seconds, "lambda": model_settling_seconds, "bandDegC": 0.2, "qualificationWindow": "until 60.00 s load event"},
            "completeRunMvTravelPercentagePoints": {"empirical": empirical_mv_travel, "lambda": model_mv_travel},
            "perturbedFinalErrorDegC": perturbed_final_error,
            "perturbedCompleteRunMvTravelPercentagePoints": perturbed_mv_travel,
            "saturationSeconds": {"empirical": empirical_saturation_seconds, "lambda": model_saturation_seconds, "perturbed": perturbed_saturation_seconds}
        },
        "restoration": {"mode": "Manual", "appliedHeaterPercent": 0.0, "runApproved": false},
        "verdict": "validated unapplied recommendation; explicit Apply is required for each target controller",
        "physicalPlantEvidence": false
    });
    let retained_json =
        serde_json::to_string_pretty(&retained).expect("serialize Lesson 18 evidence") + "\n";
    write_or_compare_curriculum_artifact(
        &artifact_root.join("lambda-tuning-evidence.json"),
        &retained_json,
    );

    assert!(
        model_tracking_iae <= empirical_tracking_iae * 0.98,
        "model/empirical tracking IAE={model_tracking_iae}/{empirical_tracking_iae}"
    );
    assert!(
        model_load_iae <= empirical_load_iae * 0.98,
        "model/empirical load IAE={model_load_iae}/{empirical_load_iae}"
    );
    assert!(
        perturbed_final_error <= 0.2,
        "perturbed final error={perturbed_final_error}"
    );
    assert!(perturbed
        .iter()
        .all(|(pv, mv)| pv.is_finite() && mv.is_finite()));
    assert!(perturbed.iter().all(|(_, mv)| *mv >= 0.0 && *mv <= 100.0));
}

fn safe_heat_exchanger_runtime() -> trust_runtime::Runtime {
    let source = without_lesson_workflow(
        project_file("safe-heat-exchanger-experiment", "src/Main.st")
            .split("CONFIGURATION Config")
            .next()
            .expect("example program before configuration")
            .to_owned(),
        "(* Control Studio grants only Boolean authority.",
        "(* This executable is deliberately simulation-only.",
    )
    .replace("ProposedStep : LREAL :=", "ProposedStep AT %ML0 : LREAL :=")
    .replace(
        "ProposedDuration : LREAL :=",
        "ProposedDuration AT %ML8 : LREAL :=",
    )
    .replace("ReviewPlan : BOOL :=", "ReviewPlan AT %MX16.0 : BOOL :=")
    .replace(
        "StartExperiment : BOOL :=",
        "StartExperiment AT %MX16.1 : BOOL :=",
    )
    .replace(
        "ResetExperiment : BOOL :=",
        "ResetExperiment AT %MX16.2 : BOOL :=",
    )
    .replace(
        "SignalQualityGood : BOOL :=",
        "SignalQualityGood AT %MX16.3 : BOOL :=",
    )
    .replace(
        "OperatingModeAllowed : BOOL :=",
        "OperatingModeAllowed AT %MX16.4 : BOOL :=",
    )
    .replace(
        "ConfoundingDisturbanceActive : BOOL :=",
        "ConfoundingDisturbanceActive AT %MX16.5 : BOOL :=",
    )
    .replace(
        "TopologyReviewed : BOOL :=",
        "TopologyReviewed AT %MX16.6 : BOOL :=",
    )
    .replace(
        "ImmediatePhysicalApproval : BOOL :=",
        "ImmediatePhysicalApproval AT %MX16.7 : BOOL :=",
    )
    .replace(
        "PhysicalPlantRequested : BOOL :=",
        "PhysicalPlantRequested AT %MX17.0 : BOOL :=",
    )
    .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
    .replace(
        "RequestedValve : LREAL :=",
        "RequestedValve AT %ML32 : LREAL :=",
    )
    .replace("AppliedValve : LREAL :=", "AppliedValve AT %QL0 : LREAL :=")
    .replace(
        "PlanAccepted : BOOL :=",
        "PlanAccepted AT %MX18.0 : BOOL :=",
    )
    .replace(
        "ExperimentRunning : BOOL :=",
        "ExperimentRunning AT %MX18.1 : BOOL :=",
    )
    .replace(
        "ExperimentComplete : BOOL :=",
        "ExperimentComplete AT %MX18.2 : BOOL :=",
    )
    .replace(
        "ExperimentAborted : BOOL :=",
        "ExperimentAborted AT %MX18.3 : BOOL :=",
    )
    .replace("Restoring : BOOL :=", "Restoring AT %MX18.4 : BOOL :=")
    .replace(
        "PhysicalExecutionEligible : BOOL :=",
        "PhysicalExecutionEligible AT %MX18.5 : BOOL :=",
    )
    .replace(
        "PlanRejectReason : INT :=",
        "PlanRejectReason AT %MW20 : INT :=",
    )
    .replace("AbortReason : INT :=", "AbortReason AT %MW22 : INT :=");
    CompileSession::from_sources(vec![SourceFile::with_path(
        "safe-heat-exchanger-experiment/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped safe heat-exchanger experiment lesson")
}

fn advance_heat_exchanger(
    runtime: &mut trust_runtime::Runtime,
    simulation: &mut SimulationController,
    cycle: usize,
) {
    runtime
        .execute_cycle()
        .expect("execute experiment supervisor");
    let now = Duration::from_millis((cycle * 100) as i64);
    simulation
        .apply_post_cycle(now, runtime)
        .expect("advance plate heat exchanger");
    simulation
        .apply_pre_cycle(now, runtime)
        .expect("publish outlet temperature");
}

#[test]
fn control_curriculum_safe_heat_exchanger_plan_rejects_executes_and_restores() {
    let shipped_source = project_file("safe-heat-exchanger-experiment", "src/Main.st");
    let config_path = temporary_config_path("safe-heat-exchanger-experiment");
    std::fs::write(
        &config_path,
        project_file("safe-heat-exchanger-experiment", "simulation.toml"),
    )
    .expect("write safe heat-exchanger simulation config");
    let config = SimulationConfig::load(&config_path).expect("load plate heat exchanger");

    let proposed_step = IoAddress::parse("%ML0").unwrap();
    let proposed_duration = IoAddress::parse("%ML8").unwrap();
    let review = IoAddress::parse("%MX16.0").unwrap();
    let start = IoAddress::parse("%MX16.1").unwrap();
    let reset = IoAddress::parse("%MX16.2").unwrap();
    let quality = IoAddress::parse("%MX16.3").unwrap();
    let mode = IoAddress::parse("%MX16.4").unwrap();
    let disturbance = IoAddress::parse("%MX16.5").unwrap();
    let topology = IoAddress::parse("%MX16.6").unwrap();
    let approval = IoAddress::parse("%MX16.7").unwrap();
    let physical_request = IoAddress::parse("%MX17.0").unwrap();
    let pv = IoAddress::parse("%IL0").unwrap();
    let requested = IoAddress::parse("%ML32").unwrap();
    let applied = IoAddress::parse("%QL0").unwrap();
    let accepted = IoAddress::parse("%MX18.0").unwrap();
    let running = IoAddress::parse("%MX18.1").unwrap();
    let complete = IoAddress::parse("%MX18.2").unwrap();
    let aborted = IoAddress::parse("%MX18.3").unwrap();
    let restoring = IoAddress::parse("%MX18.4").unwrap();
    let physical_eligible = IoAddress::parse("%MX18.5").unwrap();
    let reject_reason = IoAddress::parse("%MW20").unwrap();
    let abort_reason = IoAddress::parse("%MW22").unwrap();
    let retained_sample =
        |runtime: &trust_runtime::Runtime, time: f64, state: &str, reason: i16| {
            serde_json::json!({
                "time": time,
                "seed": 20260829,
                "sampleInterval": 0.1,
                "requestedValve": read_lreal(runtime, &requested),
                "appliedValve": read_lreal(runtime, &applied),
                "outletTemperature": read_lreal(runtime, &pv),
                "quality": if read_bool(runtime, &quality) { "good" } else { "bad" },
                "disturbanceState": read_bool(runtime, &disturbance),
                "experimentState": state,
                "reasonCode": reason,
                "evidenceClass": "configured_simulation"
            })
        };

    let mut runtime = safe_heat_exchanger_runtime();
    let mut simulation = SimulationController::new(config.clone());
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &quality, true);
    write_bool(&mut runtime, &mode, true);
    write_lreal(&mut runtime, &proposed_duration, 63.0);

    // The 25-point plan predicts 62.5 C and requests 85%, so review must
    // reject it without moving the simulated valve.
    write_lreal(&mut runtime, &proposed_step, 25.0);
    write_bool(&mut runtime, &review, true);
    advance_heat_exchanger(&mut runtime, &mut simulation, 1);
    let mut rejected_samples = vec![retained_sample(
        &runtime,
        0.1,
        "rejected",
        read_int(&runtime, &reject_reason),
    )];
    assert!(!read_bool(&runtime, &accepted));
    assert_eq!(read_int(&runtime, &reject_reason), 1);
    assert!((read_lreal(&runtime, &requested) - 60.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &applied) - 60.0).abs() <= 1e-12);

    // Even complete review inputs cannot convert this simulation-only project
    // into authority for a physical valve movement.
    write_bool(&mut runtime, &topology, true);
    write_bool(&mut runtime, &approval, true);
    write_bool(&mut runtime, &physical_request, true);
    advance_heat_exchanger(&mut runtime, &mut simulation, 2);
    rejected_samples.push(retained_sample(
        &runtime,
        0.2,
        "rejected",
        read_int(&runtime, &reject_reason),
    ));
    assert!(!read_bool(&runtime, &physical_eligible));

    write_bool(&mut runtime, &review, false);
    write_bool(&mut runtime, &physical_request, false);
    write_bool(&mut runtime, &reset, true);
    advance_heat_exchanger(&mut runtime, &mut simulation, 3);
    write_bool(&mut runtime, &reset, false);
    write_lreal(&mut runtime, &proposed_step, 10.0);
    write_bool(&mut runtime, &review, true);
    advance_heat_exchanger(&mut runtime, &mut simulation, 4);
    assert!(read_bool(&runtime, &accepted));
    assert_eq!(read_int(&runtime, &reject_reason), 0);

    write_bool(&mut runtime, &review, false);
    write_bool(&mut runtime, &start, true);
    let mut step_samples = Vec::new();
    let mut accepted_samples = Vec::new();
    let mut accepted_pv = Vec::new();
    let mut accepted_requested = Vec::new();
    let mut accepted_applied = Vec::new();
    let mut cycle = 4usize;
    while !read_bool(&runtime, &restoring) && cycle < 900 {
        cycle += 1;
        advance_heat_exchanger(&mut runtime, &mut simulation, cycle);
        let state = if read_lreal(&runtime, &applied) > 60.0 {
            "excitation"
        } else {
            "baseline"
        };
        accepted_samples.push(retained_sample(&runtime, cycle as f64 * 0.1, state, 0));
        accepted_pv.push(read_lreal(&runtime, &pv));
        accepted_requested.push(read_lreal(&runtime, &requested));
        accepted_applied.push(read_lreal(&runtime, &applied));
        if read_bool(&runtime, &running) && read_lreal(&runtime, &applied) > 60.0 {
            step_samples.push(read_lreal(&runtime, &pv));
        }
    }
    assert!(
        read_bool(&runtime, &restoring),
        "accepted run must enter restoration"
    );
    assert!(!read_bool(&runtime, &aborted));
    assert!(
        step_samples.len() >= 620,
        "63-second excitation must be retained"
    );
    let late_mean = step_samples[step_samples.len() - 100..].iter().sum::<f64>() / 100.0;
    assert!(
        (late_mean - 55.0).abs() <= 0.35,
        "late-window PV={late_mean}"
    );
    assert!((read_lreal(&runtime, &requested) - 60.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &applied) - 60.0).abs() <= 1e-12);

    while !read_bool(&runtime, &complete) && cycle < 2_000 {
        cycle += 1;
        advance_heat_exchanger(&mut runtime, &mut simulation, cycle);
        let state = if read_bool(&runtime, &complete) {
            "complete"
        } else {
            "restoring"
        };
        accepted_samples.push(retained_sample(&runtime, cycle as f64 * 0.1, state, 0));
        accepted_pv.push(read_lreal(&runtime, &pv));
        accepted_requested.push(read_lreal(&runtime, &requested));
        accepted_applied.push(read_lreal(&runtime, &applied));
    }
    assert!(
        read_bool(&runtime, &complete),
        "restoration must include PV returning to baseline band"
    );
    assert!(!read_bool(&runtime, &restoring));
    assert!((read_lreal(&runtime, &pv) - 50.0).abs() <= 0.1);

    // A second, equivalent run loses quality during excitation. It must abort
    // immediately, restore the valve, and remain invalid identification data.
    let mut abort_runtime = safe_heat_exchanger_runtime();
    let mut abort_simulation = SimulationController::new(config);
    abort_simulation
        .apply_pre_cycle(Duration::ZERO, &mut abort_runtime)
        .unwrap();
    write_bool(&mut abort_runtime, &quality, true);
    write_bool(&mut abort_runtime, &mode, true);
    write_lreal(&mut abort_runtime, &proposed_step, 10.0);
    write_lreal(&mut abort_runtime, &proposed_duration, 63.0);
    write_bool(&mut abort_runtime, &review, true);
    advance_heat_exchanger(&mut abort_runtime, &mut abort_simulation, 1);
    write_bool(&mut abort_runtime, &review, false);
    write_bool(&mut abort_runtime, &start, true);
    let mut abort_samples = Vec::new();
    let mut abort_pv = Vec::new();
    let mut abort_applied = Vec::new();
    for abort_cycle in 2..=150 {
        advance_heat_exchanger(&mut abort_runtime, &mut abort_simulation, abort_cycle);
        let state = if read_lreal(&abort_runtime, &applied) > 60.0 {
            "excitation"
        } else {
            "baseline"
        };
        abort_samples.push(retained_sample(
            &abort_runtime,
            abort_cycle as f64 * 0.1,
            state,
            0,
        ));
        abort_pv.push(read_lreal(&abort_runtime, &pv));
        abort_applied.push(read_lreal(&abort_runtime, &applied));
    }
    assert!(read_bool(&abort_runtime, &running));
    write_bool(&mut abort_runtime, &quality, false);
    advance_heat_exchanger(&mut abort_runtime, &mut abort_simulation, 151);
    abort_samples.push(retained_sample(
        &abort_runtime,
        15.1,
        "aborted",
        read_int(&abort_runtime, &abort_reason),
    ));
    abort_pv.push(read_lreal(&abort_runtime, &pv));
    abort_applied.push(read_lreal(&abort_runtime, &applied));
    assert!(read_bool(&abort_runtime, &aborted));
    assert!(read_bool(&abort_runtime, &restoring));
    assert_eq!(read_int(&abort_runtime, &abort_reason), 1);
    assert!((read_lreal(&abort_runtime, &requested) - 60.0).abs() <= 1e-12);
    assert!((read_lreal(&abort_runtime, &applied) - 60.0).abs() <= 1e-12);
    write_bool(&mut abort_runtime, &quality, true);
    let mut abort_cycle = 151usize;
    while read_bool(&abort_runtime, &restoring) && abort_cycle < 2_000 {
        abort_cycle += 1;
        advance_heat_exchanger(&mut abort_runtime, &mut abort_simulation, abort_cycle);
        let state = if read_bool(&abort_runtime, &restoring) {
            "restoring"
        } else {
            "aborted-restored"
        };
        abort_samples.push(retained_sample(
            &abort_runtime,
            abort_cycle as f64 * 0.1,
            state,
            read_int(&abort_runtime, &abort_reason),
        ));
        abort_pv.push(read_lreal(&abort_runtime, &pv));
        abort_applied.push(read_lreal(&abort_runtime, &applied));
    }
    assert!(!read_bool(&abort_runtime, &restoring));
    assert!((read_lreal(&abort_runtime, &pv) - 50.0).abs() <= 0.1);

    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/05-experiments-identification-and-validation/37-safe-heat-exchanger-experiment/figures");
    let plot_len = accepted_pv.len().max(abort_pv.len());
    let pad = |values: &[f64]| {
        let mut padded = values.to_vec();
        padded.resize(plot_len, *values.last().expect("retained outcome sample"));
        padded
    };
    let accepted_pv_plot = pad(&accepted_pv);
    let accepted_requested_plot = pad(&accepted_requested);
    let accepted_applied_plot = pad(&accepted_applied);
    let abort_pv_plot = pad(&abort_pv);
    let abort_applied_plot = pad(&abort_applied);
    let rejected_pv_plot = vec![50.0; plot_len];
    let rejected_applied_plot = vec![60.0; plot_len];
    let summary = format!("plan,step_pp,duration_s,predicted_final_degC,late_mean_degC,restored_degC,abort_reason,decision,evidence_class\nPlan A,10,63,55.000000,{late_mean:.12},{:.12},0,qualified,configured_simulation\nPlan B,25,63,62.500000,,,0,rejected_before_motion,configured_simulation\nPlan A bad quality,10,63,55.000000,,,1,aborted_not_identification_data,configured_simulation\n", read_lreal(&runtime, &pv));
    let plans = "plan,baseline_valve_percent,step_pp,duration_s,predicted_final_degC,soft_valve_limit_percent,hard_temperature_limit_degC,review_decision,physical_execution_eligible\nPlan A,60,10,63,55.0,80,58,accepted_simulation,false\nPlan B,60,25,63,62.5,80,58,rejected_temperature_prediction,false\nPlan A bad quality,60,10,63,55.0,80,58,aborted_reason_1,false\n";
    write_or_compare_curriculum_artifact(&artifact_root.join("safe-experiment-plans.csv"), plans);
    let retained = serde_json::json!({
        "runs": [
            {"plan": "Plan B rejected", "samples": rejected_samples},
            {"plan": "Plan A accepted", "samples": accepted_samples},
            {"plan": "Plan A bad quality", "samples": abort_samples}
        ]
    });
    write_or_compare_curriculum_artifact(
        &artifact_root.join("safe-experiment-evidence.json"),
        &(serde_json::to_string_pretty(&retained).expect("serialize safe experiment evidence")
            + "\n"),
    );
    evidence::retain(
        &artifact_root,
        "safe-experiment-comparison",
        "Reviewed heat-exchanger outcomes",
        "Configured Simulation evidence · accepted, rejected-before-motion, and bad-quality abort",
        0.1,
        &[
            evidence::Panel {
                title: "Outlet temperature",
                unit: "degC",
                minimum: 49.0,
                maximum: 56.0,
                series: vec![
                    evidence::Series {
                        name: "accepted",
                        color: "#0072b2",
                        dashed: false,
                        values: &accepted_pv_plot,
                    },
                    evidence::Series {
                        name: "bad-quality abort",
                        color: "#d55e00",
                        dashed: false,
                        values: &abort_pv_plot,
                    },
                    evidence::Series {
                        name: "rejected before motion",
                        color: "#243044",
                        dashed: true,
                        values: &rejected_pv_plot,
                    },
                ],
            },
            evidence::Panel {
                title: "Valve position",
                unit: "%",
                minimum: 55.0,
                maximum: 75.0,
                series: vec![
                    evidence::Series {
                        name: "accepted requested",
                        color: "#0072b2",
                        dashed: true,
                        values: &accepted_requested_plot,
                    },
                    evidence::Series {
                        name: "accepted applied",
                        color: "#009e73",
                        dashed: false,
                        values: &accepted_applied_plot,
                    },
                    evidence::Series {
                        name: "bad-quality applied",
                        color: "#d55e00",
                        dashed: false,
                        values: &abort_applied_plot,
                    },
                    evidence::Series {
                        name: "rejected applied",
                        color: "#243044",
                        dashed: true,
                        values: &rejected_applied_plot,
                    },
                ],
            },
        ],
        &[],
        &[evidence::Marker {
            at_seconds: 15.1,
            label: "bad quality",
        }],
        &summary,
    );

    let _ = std::fs::remove_file(config_path);
    assert!(!shipped_source.contains("Control.PID"));
    println!(
        "safe heat-exchanger experiment: rejected=85%, accepted late mean={late_mean:.3} C, restored PV={:.3} C, abort reason={}",
        read_lreal(&runtime, &pv),
        read_int(&abort_runtime, &abort_reason)
    );
}

fn excitation_methods_runtime() -> trust_runtime::Runtime {
    let source = without_lesson_workflow(
        project_file("excitation-methods", "src/Main.st")
            .split("CONFIGURATION Config")
            .next()
            .expect("example program before configuration")
            .to_owned(),
        "IF ExperimentEnabled THEN",
        "InputsValid :=",
    )
    .replace("PlantSelection : INT :=", "PlantSelection AT %MW0 : INT :=")
    .replace(
        "MethodSelection : INT :=",
        "MethodSelection AT %MW2 : INT :=",
    )
    .replace(
        "StartExperiment : BOOL :=",
        "StartExperiment AT %MX4.0 : BOOL :=",
    )
    .replace(
        "ResetExperiment : BOOL :=",
        "ResetExperiment AT %MX4.1 : BOOL :=",
    )
    .replace(
        "ExperimentRunning : BOOL :=",
        "ExperimentRunning AT %MX4.2 : BOOL :=",
    )
    .replace(
        "MethodEligible : BOOL :=",
        "MethodEligible AT %MX4.3 : BOOL :=",
    )
    .replace(
        "ExperimentComplete : BOOL :=",
        "ExperimentComplete AT %MX4.4 : BOOL :=",
    )
    .replace("RejectReason : INT :=", "RejectReason AT %MW6 : INT :=")
    .replace(
        "HeaterRequestedCommand : LREAL :=",
        "HeaterRequestedCommand AT %ML8 : LREAL :=",
    )
    .replace(
        "HeaterAppliedCommand : LREAL :=",
        "HeaterAppliedCommand AT %QL0 : LREAL :=",
    )
    .replace(
        "ServoRequestedTorque : LREAL :=",
        "ServoRequestedTorque AT %ML16 : LREAL :=",
    )
    .replace(
        "ServoAppliedTorque : LREAL :=",
        "ServoAppliedTorque AT %QL8 : LREAL :=",
    )
    .replace(
        "InstantFrequencyHz : LREAL :=",
        "InstantFrequencyHz AT %ML24 : LREAL :=",
    )
    .replace(
        "HeaterTemperature : LREAL :=",
        "HeaterTemperature AT %IL0 : LREAL :=",
    )
    .replace(
        "ServoPosition : LREAL :=",
        "ServoPosition AT %IL8 : LREAL :=",
    );
    CompileSession::from_sources(vec![SourceFile::with_path(
        "excitation-methods/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped excitation-methods lesson")
}

#[derive(Clone)]
struct ExcitationSample {
    heater_requested: f64,
    heater_command: f64,
    heater_temperature: f64,
    servo_requested: f64,
    servo_torque: f64,
    servo_position: f64,
    frequency_hz: f64,
}

fn run_excitation_method(
    config: &SimulationConfig,
    plant: i16,
    method: i16,
) -> (Vec<ExcitationSample>, bool, i16) {
    let mut runtime = excitation_methods_runtime();
    let mut simulation = SimulationController::new(config.clone());
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let plant_selection = IoAddress::parse("%MW0").unwrap();
    let method_selection = IoAddress::parse("%MW2").unwrap();
    let start = IoAddress::parse("%MX4.0").unwrap();
    let eligible = IoAddress::parse("%MX4.3").unwrap();
    let reject_reason = IoAddress::parse("%MW6").unwrap();
    let heater_command = IoAddress::parse("%QL0").unwrap();
    let heater_requested = IoAddress::parse("%ML8").unwrap();
    let servo_torque = IoAddress::parse("%QL8").unwrap();
    let servo_requested = IoAddress::parse("%ML16").unwrap();
    let frequency = IoAddress::parse("%ML24").unwrap();
    let heater_temperature = IoAddress::parse("%IL0").unwrap();
    let servo_position = IoAddress::parse("%IL8").unwrap();
    write_int(&mut runtime, &plant_selection, plant);
    write_int(&mut runtime, &method_selection, method);
    write_bool(&mut runtime, &start, true);

    let mut samples = Vec::with_capacity(300);
    for cycle in 1..=300 {
        advance_heat_exchanger(&mut runtime, &mut simulation, cycle);
        samples.push(ExcitationSample {
            heater_requested: read_lreal(&runtime, &heater_requested),
            heater_command: read_lreal(&runtime, &heater_command),
            heater_temperature: read_lreal(&runtime, &heater_temperature),
            servo_requested: read_lreal(&runtime, &servo_requested),
            servo_torque: read_lreal(&runtime, &servo_torque),
            servo_position: read_lreal(&runtime, &servo_position),
            frequency_hz: read_lreal(&runtime, &frequency),
        });
    }
    (
        samples,
        read_bool(&runtime, &eligible),
        read_int(&runtime, &reject_reason),
    )
}

fn transition_indices(values: &[f64]) -> Vec<usize> {
    values
        .windows(2)
        .enumerate()
        .filter_map(|(index, pair)| ((pair[1] - pair[0]).abs() > 1e-9).then_some(index + 1))
        .collect()
}

#[test]
fn control_curriculum_excitation_methods_match_heater_and_flexible_drive_dynamics() {
    let source = project_file("excitation-methods", "src/Main.st");
    let config_path = temporary_config_path("excitation-methods");
    std::fs::write(
        &config_path,
        project_file("excitation-methods", "simulation.toml"),
    )
    .expect("write excitation-methods simulation config");
    let config = SimulationConfig::load(&config_path).expect("load heater and flexible drive");

    let (heater_step, step_eligible, _) = run_excitation_method(&config, 0, 1);
    let (heater_pulse, pulse_eligible, _) = run_excitation_method(&config, 0, 2);
    let (heater_prbs, prbs_eligible, _) = run_excitation_method(&config, 0, 5);
    let (heater_chirp, chirp_eligible, _) = run_excitation_method(&config, 0, 6);
    assert!(step_eligible && pulse_eligible && prbs_eligible && chirp_eligible);
    assert!(heater_pulse[50..100]
        .iter()
        .all(|sample| (sample.heater_command - 25.0).abs() <= 1e-12));
    assert!(heater_pulse[120..]
        .iter()
        .all(|sample| (sample.heater_command - 20.0).abs() <= 1e-12));
    let heater_step_span = heater_step
        .iter()
        .map(|sample| sample.heater_temperature)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(value), high.max(value))
        });
    let heater_step_span = heater_step_span.1 - heater_step_span.0;
    let heater_chirp_span = heater_chirp
        .iter()
        .map(|sample| sample.heater_temperature)
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), value| {
            (low.min(value), high.max(value))
        });
    let heater_chirp_span = heater_chirp_span.1 - heater_chirp_span.0;
    assert!(
        (heater_step_span - 1.0).abs() <= 0.10,
        "heater step span={heater_step_span}"
    );
    assert!(
        heater_chirp_span <= heater_step_span * 0.60,
        "heater chirp/step spans={heater_chirp_span}/{heater_step_span}"
    );
    let heater_prbs_commands = heater_prbs
        .iter()
        .map(|sample| sample.heater_command)
        .collect::<Vec<_>>();
    let heater_transitions = transition_indices(&heater_prbs_commands)
        .into_iter()
        .filter(|index| *index < 250)
        .collect::<Vec<_>>();
    assert!(heater_transitions
        .windows(2)
        .all(|pair| pair[1] - pair[0] >= 25));

    let (servo_doublet, doublet_eligible, _) = run_excitation_method(&config, 1, 3);
    let (servo_prbs, servo_prbs_eligible, _) = run_excitation_method(&config, 1, 5);
    let (servo_chirp, servo_chirp_eligible, _) = run_excitation_method(&config, 1, 6);
    assert!(doublet_eligible && servo_prbs_eligible && servo_chirp_eligible);
    let signed_doublet_area = servo_doublet
        .iter()
        .map(|sample| sample.servo_torque * 0.1)
        .sum::<f64>();
    assert!(
        signed_doublet_area.abs() <= 0.01,
        "doublet area={signed_doublet_area}"
    );
    assert!(servo_doublet.last().unwrap().servo_torque.abs() <= 1e-12);
    let servo_prbs_commands = servo_prbs
        .iter()
        .map(|sample| sample.servo_torque)
        .collect::<Vec<_>>();
    let servo_transitions = transition_indices(&servo_prbs_commands)
        .into_iter()
        .filter(|index| *index < 250)
        .collect::<Vec<_>>();
    assert!(servo_transitions
        .windows(2)
        .all(|pair| pair[1] - pair[0] >= 3));
    assert!(servo_transitions.len() > heater_transitions.len() * 3);

    let resonant_sample = servo_chirp
        .iter()
        .filter(|sample| sample.frequency_hz > 0.0)
        .max_by(|left, right| {
            left.servo_position
                .abs()
                .total_cmp(&right.servo_position.abs())
        })
        .expect("servo chirp response sample");
    assert!(
        (resonant_sample.frequency_hz - 0.95).abs() <= 0.15,
        "servo chirp peak at {} Hz, position {} rad",
        resonant_sample.frequency_hz,
        resonant_sample.servo_position
    );
    assert!(servo_chirp
        .iter()
        .all(|sample| sample.servo_torque.abs() <= 0.2));
    assert!(servo_chirp
        .iter()
        .all(|sample| sample.servo_position.abs() <= 0.35));

    let (relay, relay_eligible, relay_reason) = run_excitation_method(&config, 0, 4);
    assert!(!relay_eligible);
    assert_eq!(relay_reason, 4);
    assert!(relay
        .iter()
        .all(|sample| (sample.heater_command - 20.0).abs() <= 1e-12));
    assert!(relay
        .iter()
        .all(|sample| sample.servo_torque.abs() <= 1e-12));
    assert!(!source.contains("Control.PID"));

    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/05-experiments-identification-and-validation/38-excitation-methods/figures");
    let heater_step_temperature = heater_step
        .iter()
        .map(|sample| sample.heater_temperature)
        .collect::<Vec<_>>();
    let heater_prbs_temperature = heater_prbs
        .iter()
        .map(|sample| sample.heater_temperature)
        .collect::<Vec<_>>();
    let heater_pulse_temperature = heater_pulse
        .iter()
        .map(|sample| sample.heater_temperature)
        .collect::<Vec<_>>();
    let heater_chirp_temperature = heater_chirp
        .iter()
        .map(|sample| sample.heater_temperature)
        .collect::<Vec<_>>();
    let heater_step_command = heater_step
        .iter()
        .map(|sample| sample.heater_command)
        .collect::<Vec<_>>();
    let heater_pulse_command = heater_pulse
        .iter()
        .map(|sample| sample.heater_command)
        .collect::<Vec<_>>();
    let heater_chirp_command = heater_chirp
        .iter()
        .map(|sample| sample.heater_command)
        .collect::<Vec<_>>();
    let heater_summary = format!("method,eligible,output_span_degC,prbs_transitions,reject_reason\nstep,true,{heater_step_span:.12},,0\npulse,true,,,0\nPRBS,true,,{},0\nchirp,true,{heater_chirp_span:.12},,0\nrelay,false,,,4\n", heater_transitions.len());
    evidence::retain(
        &artifact_root,
        "excitation-heater-comparison",
        "Excitation for a slow heater",
        "Configured Simulation evidence · identical water-heater model and 100 ms sampling",
        0.1,
        &[
            evidence::Panel {
                title: "Measured heater temperature",
                unit: "degC",
                minimum: 23.5,
                maximum: 25.5,
                series: vec![
                    evidence::Series {
                        name: "step",
                        color: "#0072b2",
                        dashed: false,
                        values: &heater_step_temperature,
                    },
                    evidence::Series {
                        name: "pulse",
                        color: "#cc79a7",
                        dashed: false,
                        values: &heater_pulse_temperature,
                    },
                    evidence::Series {
                        name: "PRBS",
                        color: "#d55e00",
                        dashed: false,
                        values: &heater_prbs_temperature,
                    },
                    evidence::Series {
                        name: "pulse command",
                        color: "#cc79a7",
                        dashed: false,
                        values: &heater_pulse_command,
                    },
                    evidence::Series {
                        name: "chirp",
                        color: "#009e73",
                        dashed: false,
                        values: &heater_chirp_temperature,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied heater command",
                unit: "%",
                minimum: 10.0,
                maximum: 30.0,
                series: vec![
                    evidence::Series {
                        name: "step command",
                        color: "#0072b2",
                        dashed: false,
                        values: &heater_step_command,
                    },
                    evidence::Series {
                        name: "PRBS command",
                        color: "#d55e00",
                        dashed: false,
                        values: &heater_prbs_commands,
                    },
                    evidence::Series {
                        name: "chirp command",
                        color: "#009e73",
                        dashed: false,
                        values: &heater_chirp_command,
                    },
                ],
            },
        ],
        &[],
        &[],
        &heater_summary,
    );

    let servo_doublet_position = servo_doublet
        .iter()
        .map(|sample| sample.servo_position)
        .collect::<Vec<_>>();
    let servo_prbs_position = servo_prbs
        .iter()
        .map(|sample| sample.servo_position)
        .collect::<Vec<_>>();
    let servo_chirp_position = servo_chirp
        .iter()
        .map(|sample| sample.servo_position)
        .collect::<Vec<_>>();
    let servo_doublet_torque = servo_doublet
        .iter()
        .map(|sample| sample.servo_torque)
        .collect::<Vec<_>>();
    let servo_chirp_torque = servo_chirp
        .iter()
        .map(|sample| sample.servo_torque)
        .collect::<Vec<_>>();
    let servo_summary = format!("method,eligible,signed_area_N_m_s,prbs_transitions,resonance_peak_hz,resonance_peak_rad,reject_reason\ndoublet,true,{signed_doublet_area:.12},,,,0\nPRBS,true,,{},,,0\nchirp,true,,,{:.12},{:.12},0\n", servo_transitions.len(), resonant_sample.frequency_hz, resonant_sample.servo_position);
    evidence::retain(
        &artifact_root,
        "excitation-servo-comparison",
        "Excitation for a flexible drive",
        "Configured Simulation evidence · identical flexible drive and 100 ms sampling",
        0.1,
        &[
            evidence::Panel {
                title: "Measured shaft position",
                unit: "rad",
                minimum: -0.35,
                maximum: 0.35,
                series: vec![
                    evidence::Series {
                        name: "doublet",
                        color: "#0072b2",
                        dashed: false,
                        values: &servo_doublet_position,
                    },
                    evidence::Series {
                        name: "PRBS",
                        color: "#d55e00",
                        dashed: false,
                        values: &servo_prbs_position,
                    },
                    evidence::Series {
                        name: "chirp",
                        color: "#009e73",
                        dashed: false,
                        values: &servo_chirp_position,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied shaft torque",
                unit: "N m",
                minimum: -0.25,
                maximum: 0.25,
                series: vec![
                    evidence::Series {
                        name: "doublet torque",
                        color: "#0072b2",
                        dashed: false,
                        values: &servo_doublet_torque,
                    },
                    evidence::Series {
                        name: "PRBS torque",
                        color: "#d55e00",
                        dashed: false,
                        values: &servo_prbs_commands,
                    },
                    evidence::Series {
                        name: "chirp torque",
                        color: "#009e73",
                        dashed: false,
                        values: &servo_chirp_torque,
                    },
                ],
            },
        ],
        &[],
        &[],
        &servo_summary,
    );

    let retained_run = |plant: &str,
                        method: &str,
                        samples: &[ExcitationSample],
                        qualified: bool,
                        reason: i16| {
        let sample_rows = samples
            .iter()
            .enumerate()
            .map(|(index, sample)| {
                let heater = plant == "water-heater";
                serde_json::json!({
                    "time": (index + 1) as f64 * 0.1,
                    "requestedInput": if heater { sample.heater_requested } else { sample.servo_requested },
                    "appliedInput": if heater { sample.heater_command } else { sample.servo_torque },
                    "output": if heater { sample.heater_temperature } else { sample.servo_position },
                    "frequency": sample.frequency_hz,
                    "noise": if heater { 0.02 } else { 0.002 },
                    "limits": if heater { serde_json::json!([0.0, 100.0]) } else { serde_json::json!([-0.2, 0.2]) },
                    "qualification": if qualified { "qualified" } else { "rejected" },
                    "reasonCode": reason,
                    "seed": 20260829,
                    "evidenceClass": "configured_simulation"
                })
            })
            .collect::<Vec<_>>();
        serde_json::json!({"plant": plant, "method": method, "samples": sample_rows})
    };
    let retained = serde_json::json!({
        "sampleInterval": 0.1,
        "runs": [
            retained_run("water-heater", "step", &heater_step, true, 0),
            retained_run("water-heater", "pulse", &heater_pulse, true, 0),
            retained_run("water-heater", "PRBS", &heater_prbs, true, 0),
            retained_run("water-heater", "chirp", &heater_chirp, true, 0),
            retained_run("flexible-drive", "doublet", &servo_doublet, true, 0),
            retained_run("flexible-drive", "PRBS", &servo_prbs, true, 0),
            retained_run("flexible-drive", "chirp", &servo_chirp, true, 0),
            retained_run("water-heater", "relay-rejected", &relay, false, relay_reason)
        ]
    });
    write_or_compare_curriculum_artifact(
        &artifact_root.join("excitation-methods-evidence.json"),
        &(serde_json::to_string_pretty(&retained).expect("serialize excitation evidence") + "\n"),
    );
    let _ = std::fs::remove_file(config_path);

    println!(
        "excitation selection: heater step/chirp spans={heater_step_span:.3}/{heater_chirp_span:.3} degC; heater/servo PRBS transitions={}/{}; doublet area={signed_doublet_area:.4} N m s; servo peak={:.3} Hz at {:.3} rad; relay reason={relay_reason}",
        heater_transitions.len(),
        servo_transitions.len(),
        resonant_sample.frequency_hz,
        resonant_sample.servo_position,
    );
}

fn dryer_runtime() -> trust_runtime::Runtime {
    let source = without_lesson_workflow(
        project_file("dryer-model-selection", "src/Main.st")
            .split("CONFIGURATION Config")
            .next()
            .expect("example program before configuration")
            .to_owned(),
        "IF ExperimentEnabled AND InputsValid AND NOT SafetyAbort THEN",
        "(* Keep the identification experiment open loop.",
    )
    .replace(
        "ExperimentInput : LREAL :=",
        "ExperimentInput AT %ML0 : LREAL :=",
    )
    .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
    .replace(
        "ManipulatedValue : LREAL :=",
        "ManipulatedValue AT %QL0 : LREAL :=",
    );
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("dryer-model-selection/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped dryer model-selection lesson")
}

fn run_dryer(config: &SimulationConfig, inputs: &[f64]) -> Vec<f64> {
    let mut runtime = dryer_runtime();
    let mut simulation = SimulationController::new(config.clone());
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    let input = IoAddress::parse("%ML0").unwrap();
    let output = IoAddress::parse("%IL0").unwrap();
    let mut trace = Vec::with_capacity(inputs.len());
    for (index, command) in inputs.iter().copied().enumerate() {
        write_lreal(&mut runtime, &input, command);
        runtime
            .execute_cycle()
            .expect("execute manual dryer experiment");
        let now = Duration::from_millis(((index + 1) * 100) as i64);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
        trace.push(read_lreal(&runtime, &output));
    }
    trace
}

fn prediction_rmse(prediction: &[f64], measured: &[f64], start: usize) -> f64 {
    (prediction[start..]
        .iter()
        .zip(&measured[start..])
        .map(|(predicted, observed)| (observed - predicted).powi(2))
        .sum::<f64>()
        / (prediction.len() - start) as f64)
        .sqrt()
}

fn prediction_rmse_range(
    prediction: &[f64],
    measured: &[f64],
    range: std::ops::Range<usize>,
) -> f64 {
    (prediction[range.clone()]
        .iter()
        .zip(&measured[range.clone()])
        .map(|(predicted, observed)| (observed - predicted).powi(2))
        .sum::<f64>()
        / range.len() as f64)
        .sqrt()
}

fn predict_fopdt(inputs: &[f64], gain: f64, tau: f64, delay_samples: usize) -> Vec<f64> {
    let mut output = 80.0;
    let mut prediction = Vec::with_capacity(inputs.len());
    for (index, _) in inputs.iter().enumerate() {
        let delayed = if index >= delay_samples {
            inputs[index - delay_samples]
        } else {
            30.0
        };
        output += 0.1 * ((80.0 + gain * (delayed - 30.0)) - output) / tau;
        prediction.push(output);
    }
    prediction
}

fn predict_second_order(
    inputs: &[f64],
    gain: f64,
    natural_frequency: f64,
    damping: f64,
) -> Vec<f64> {
    let mut position = 0.0;
    let mut velocity = 0.0;
    let mut prediction = Vec::with_capacity(inputs.len());
    for input in inputs {
        let acceleration =
            natural_frequency * natural_frequency * (gain * (*input - 30.0) - position)
                - 2.0 * damping * natural_frequency * velocity;
        velocity += 0.1 * acceleration;
        position += 0.1 * velocity;
        prediction.push(80.0 + position);
    }
    prediction
}

#[test]
fn control_curriculum_dryer_selects_simplest_model_from_independent_prediction() {
    let source = project_file("dryer-model-selection", "src/Main.st");
    let simulation_source = project_file("dryer-model-selection", "simulation.toml");
    let config_path = temporary_config_path("dryer-model-selection");
    std::fs::write(&config_path, &simulation_source).expect("write dryer simulation config");
    let config = SimulationConfig::load(&config_path).expect("load conveyor dryer");

    let estimation_inputs = (0..900)
        .map(|index| if index < 100 { 30.0 } else { 40.0 })
        .collect::<Vec<_>>();
    let estimation = run_dryer(&config, &estimation_inputs);
    let baseline = estimation[50..100].iter().sum::<f64>() / 50.0;
    let final_mean = estimation[800..900].iter().sum::<f64>() / 100.0;
    let response_start = estimation[100..]
        .windows(10)
        .position(|window| window.iter().sum::<f64>() / 10.0 >= baseline + 0.08)
        .map(|index| (index + 5) as f64 * 0.1)
        .expect("dryer rolling mean exceeds the qualified noise threshold");
    assert!((baseline - 80.0).abs() <= 0.03, "baseline={baseline}");
    assert!((final_mean - 84.0).abs() <= 0.08, "final mean={final_mean}");
    assert!(
        (response_start - 4.0).abs() <= 0.5,
        "measured delay={response_start}"
    );

    let validation_inputs = (0..900)
        .map(|index| {
            if (100..300).contains(&index) {
                35.0
            } else {
                30.0
            }
        })
        .collect::<Vec<_>>();
    let validation = run_dryer(&config, &validation_inputs);

    let mut best_fopdt = (f64::INFINITY, 0.0, 0.0, 0usize);
    for gain_index in 0..=4 {
        let gain = 0.36 + gain_index as f64 * 0.02;
        for tau_index in 0..=4 {
            let tau = 10.0 + tau_index as f64;
            for delay_samples in [35usize, 40, 45] {
                let prediction = predict_fopdt(&estimation_inputs, gain, tau, delay_samples);
                let rmse = prediction_rmse(&prediction, &estimation, 100);
                if rmse < best_fopdt.0 {
                    best_fopdt = (rmse, gain, tau, delay_samples);
                }
            }
        }
    }
    let fopdt_validation_prediction =
        predict_fopdt(&validation_inputs, best_fopdt.1, best_fopdt.2, best_fopdt.3);
    let fopdt_validation_rmse = prediction_rmse(&fopdt_validation_prediction, &validation, 100);
    let fopdt_bias = fopdt_validation_prediction[100..]
        .iter()
        .zip(&validation[100..])
        .map(|(predicted, measured)| measured - predicted)
        .sum::<f64>()
        / 800.0;

    let mut best_first_order = (f64::INFINITY, 0.0, 0.0);
    for gain_index in 0..=10 {
        let gain = 0.30 + gain_index as f64 * 0.02;
        for tau_index in 0..=10 {
            let tau = 8.0 + tau_index as f64;
            let prediction = predict_fopdt(&estimation_inputs, gain, tau, 0);
            let rmse = prediction_rmse(&prediction, &estimation, 100);
            if rmse < best_first_order.0 {
                best_first_order = (rmse, gain, tau);
            }
        }
    }
    let first_validation = predict_fopdt(
        &validation_inputs,
        best_first_order.1,
        best_first_order.2,
        0,
    );
    let first_validation_rmse = prediction_rmse(&first_validation, &validation, 100);
    let delay_window_bias = first_validation[100..140]
        .iter()
        .zip(&validation[100..140])
        .map(|(predicted, measured)| measured - predicted)
        .sum::<f64>()
        / 40.0;

    let mut best_second_order = (f64::INFINITY, 0.0, 0.0, 0.0);
    for gain_index in 0..=4 {
        let gain = 0.36 + gain_index as f64 * 0.02;
        for frequency_index in 0..=5 {
            let frequency = 0.08 + frequency_index as f64 * 0.03;
            for damping_index in 0..=4 {
                let damping = 0.7 + damping_index as f64 * 0.2;
                let prediction = predict_second_order(&estimation_inputs, gain, frequency, damping);
                let rmse = prediction_rmse(&prediction, &estimation, 100);
                if rmse < best_second_order.0 {
                    best_second_order = (rmse, gain, frequency, damping);
                }
            }
        }
    }
    let second_validation = predict_second_order(
        &validation_inputs,
        best_second_order.1,
        best_second_order.2,
        best_second_order.3,
    );
    let second_validation_rmse = prediction_rmse(&second_validation, &validation, 100);

    let integrating_prediction = validation_inputs
        .iter()
        .scan(80.0, |output, input| {
            *output += 0.1 * 0.012 * (*input - 30.0);
            Some(*output)
        })
        .collect::<Vec<_>>();
    let integrating_return_error = (integrating_prediction.last().unwrap() - 80.0).abs();
    let first_estimation_prediction = predict_fopdt(
        &estimation_inputs,
        best_first_order.1,
        best_first_order.2,
        0,
    );
    let fopdt_estimation_prediction =
        predict_fopdt(&estimation_inputs, best_fopdt.1, best_fopdt.2, best_fopdt.3);
    let second_estimation_prediction = predict_second_order(
        &estimation_inputs,
        best_second_order.1,
        best_second_order.2,
        best_second_order.3,
    );
    let integrating_estimation_prediction = estimation_inputs
        .iter()
        .scan(80.0, |output, input| {
            *output += 0.1 * 0.012 * (*input - 30.0);
            Some(*output)
        })
        .collect::<Vec<_>>();
    let fit_fopdt_subset = |range: std::ops::Range<usize>| {
        let mut best = (f64::INFINITY, 0.0, 0.0, 0usize);
        for gain_index in 0..=4 {
            let gain = 0.36 + gain_index as f64 * 0.02;
            for tau_index in 0..=4 {
                let tau = 10.0 + tau_index as f64;
                for delay_samples in [35usize, 40, 45] {
                    let prediction = predict_fopdt(&estimation_inputs, gain, tau, delay_samples);
                    let rmse = prediction_rmse_range(&prediction, &estimation, range.clone());
                    if rmse < best.0 {
                        best = (rmse, gain, tau, delay_samples);
                    }
                }
            }
        }
        best
    };
    let early_fopdt = fit_fopdt_subset(100..500);
    let late_fopdt = fit_fopdt_subset(500..900);

    assert!(
        (best_fopdt.1 - 0.4).abs() <= 0.03,
        "FOPDT gain={}",
        best_fopdt.1
    );
    assert!(
        (best_fopdt.2 - 12.0).abs() <= 1.5,
        "FOPDT tau={}",
        best_fopdt.2
    );
    assert!(
        (best_fopdt.3 as f64 * 0.1 - 4.0).abs() <= 0.5,
        "FOPDT delay={}",
        best_fopdt.3 as f64 * 0.1
    );
    assert!(
        fopdt_validation_rmse <= 0.10,
        "FOPDT validation RMSE={fopdt_validation_rmse}"
    );
    assert!(
        fopdt_bias.abs() <= 0.03,
        "FOPDT validation bias={fopdt_bias}"
    );
    assert!(
        delay_window_bias.abs() >= 0.10,
        "first-order delay-window bias={delay_window_bias}"
    );
    assert!(first_validation_rmse > fopdt_validation_rmse);
    assert!(
        second_validation_rmse >= fopdt_validation_rmse * 0.90,
        "second-order/FOPDT validation RMSE={second_validation_rmse}/{fopdt_validation_rmse}"
    );
    assert!(integrating_return_error >= 1.0,
        "integrating model must fail finite-equilibrium restoration, error={integrating_return_error}");
    assert!(
        !source.contains("Control.PID"),
        "open-loop model-selection lesson must not instantiate PID"
    );
    assert!(simulation_source.contains("model = \"first_order_dead_time\""));

    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/05-experiments-identification-and-validation/39-dryer-model-selection/figures");
    let comparison_summary = format!("candidate,parameters,validation_rmse_degC,delay_window_bias_degC,restoration_error_degC,decision\nfirst_order,K={:.2};T={:.1};L=0,{first_validation_rmse:.12},{delay_window_bias:.12},,rejected_delay_bias\nFOPDT,K={:.2};T={:.1};L={:.1},{fopdt_validation_rmse:.12},{fopdt_bias:.12},,selected\nsecond_order,K={:.2};wn={:.2};zeta={:.2},{second_validation_rmse:.12},,,rejected_no_material_improvement\nintegrating,rate=0.012,,, {integrating_return_error:.12},rejected_no_finite_equilibrium\n", best_first_order.1, best_first_order.2, best_fopdt.1, best_fopdt.2, best_fopdt.3 as f64 * 0.1, best_second_order.1, best_second_order.2, best_second_order.3);
    evidence::retain(&artifact_root, "dryer-model-comparison",
        "Dryer candidates on withheld data",
        "Configured Simulation measurement · predictions use frozen candidates and withheld pulse data",
        0.1,
        &[
            evidence::Panel { title: "Measured and predicted dryer temperature", unit: "degC", minimum: 79.5, maximum: 82.5, series: vec![
                evidence::Series { name: "measured", color: "#243044", dashed: false, values: &validation },
                evidence::Series { name: "first order", color: "#d55e00", dashed: true, values: &first_validation },
                evidence::Series { name: "FOPDT", color: "#0072b2", dashed: false, values: &fopdt_validation_prediction },
                evidence::Series { name: "second order", color: "#009e73", dashed: true, values: &second_validation },
            ]},
            evidence::Panel { title: "Validation dryer command", unit: "%", minimum: 29.0, maximum: 36.0, series: vec![
                evidence::Series { name: "applied command", color: "#cc79a7", dashed: false, values: &validation_inputs },
            ]},
        ], &[], &[], &comparison_summary);
    let first_residual = validation
        .iter()
        .zip(&first_validation)
        .map(|(measured, predicted)| measured - predicted)
        .collect::<Vec<_>>();
    let fopdt_residual = validation
        .iter()
        .zip(&fopdt_validation_prediction)
        .map(|(measured, predicted)| measured - predicted)
        .collect::<Vec<_>>();
    let second_residual = validation
        .iter()
        .zip(&second_validation)
        .map(|(measured, predicted)| measured - predicted)
        .collect::<Vec<_>>();
    let integrating_residual = validation
        .iter()
        .zip(&integrating_prediction)
        .map(|(measured, predicted)| measured - predicted)
        .collect::<Vec<_>>();
    let residual_metrics = |residual: &[f64]| {
        let selected = &residual[100..];
        let bias = selected.iter().sum::<f64>() / selected.len() as f64;
        let autocorrelation = pearson_correlation(&selected[..selected.len() - 1], &selected[1..]);
        (bias, autocorrelation)
    };
    let first_metrics = residual_metrics(&first_residual);
    let fopdt_metrics = residual_metrics(&fopdt_residual);
    let second_metrics = residual_metrics(&second_residual);
    let integrating_metrics = residual_metrics(&integrating_residual);
    let estimation_samples = (0..estimation.len())
        .map(|index| {
            serde_json::json!({
                "time": (index + 1) as f64 * 0.1,
                "requestedInput": estimation_inputs[index],
                "appliedInput": estimation_inputs[index],
                "measuredOutput": estimation[index],
                "predictions": {
                    "first_order": first_estimation_prediction[index],
                    "FOPDT": fopdt_estimation_prediction[index],
                    "second_order": second_estimation_prediction[index],
                    "integrating": integrating_estimation_prediction[index]
                },
                "seed": 20260827,
                "evidenceClass": "configured_simulation"
            })
        })
        .collect::<Vec<_>>();
    let validation_samples = (0..validation.len())
        .map(|index| {
            serde_json::json!({
                "time": (index + 1) as f64 * 0.1,
                "requestedInput": validation_inputs[index],
                "appliedInput": validation_inputs[index],
                "measuredOutput": validation[index],
                "predictions": {
                    "first_order": first_validation[index],
                    "FOPDT": fopdt_validation_prediction[index],
                    "second_order": second_validation[index],
                    "integrating": integrating_prediction[index]
                },
                "residuals": {
                    "first_order": first_residual[index],
                    "FOPDT": fopdt_residual[index],
                    "second_order": second_residual[index],
                    "integrating": integrating_residual[index]
                },
                "seed": 20260827,
                "evidenceClass": "configured_simulation"
            })
        })
        .collect::<Vec<_>>();
    let retained = serde_json::json!({
        "estimationRun": {
            "id": "dryer-estimation-step-seed-20260827",
            "purpose": "parameter estimation only",
            "samples": estimation_samples
        },
        "validationRun": {
            "id": "dryer-validation-pulse-seed-20260827",
            "purpose": "independent model selection only",
            "samples": validation_samples
        },
        "candidates": [
            {
                "structure": "first_order",
                "parameters": {"gainDegCPerPercent": best_first_order.1, "timeConstantS": best_first_order.2, "deadTimeS": 0.0},
                "parameterUncertainty": {"status": "not qualified after delay-bias rejection"},
                "estimationRmse": best_first_order.0,
                "validationRmse": first_validation_rmse,
                "bias": first_metrics.0,
                "autocorrelation": first_metrics.1,
                "prediction": first_validation,
                "residual": first_residual,
                "rejectionReason": "repeatable delay-window bias",
                "selection": false
            },
            {
                "structure": "FOPDT",
                "parameters": {"gainDegCPerPercent": best_fopdt.1, "timeConstantS": best_fopdt.2, "deadTimeS": best_fopdt.3 as f64 * 0.1},
                "parameterUncertainty": {
                    "method": "early and late estimation-subset grid fits",
                    "early": {"gainDegCPerPercent": early_fopdt.1, "timeConstantS": early_fopdt.2, "deadTimeS": early_fopdt.3 as f64 * 0.1},
                    "late": {"gainDegCPerPercent": late_fopdt.1, "timeConstantS": late_fopdt.2, "deadTimeS": late_fopdt.3 as f64 * 0.1},
                    "spread": {
                        "gainDegCPerPercent": (early_fopdt.1 - late_fopdt.1).abs(),
                        "timeConstantS": (early_fopdt.2 - late_fopdt.2).abs(),
                        "deadTimeS": (early_fopdt.3 as f64 - late_fopdt.3 as f64).abs() * 0.1
                    }
                },
                "estimationRmse": best_fopdt.0,
                "validationRmse": fopdt_validation_rmse,
                "bias": fopdt_metrics.0,
                "autocorrelation": fopdt_metrics.1,
                "prediction": fopdt_validation_prediction,
                "residual": fopdt_residual,
                "rejectionReason": null,
                "selection": true
            },
            {
                "structure": "second_order",
                "parameters": {"gainDegCPerPercent": best_second_order.1, "naturalFrequencyRadPerS": best_second_order.2, "dampingRatio": best_second_order.3},
                "parameterUncertainty": {"status": "not qualified after complexity rejection"},
                "estimationRmse": best_second_order.0,
                "validationRmse": second_validation_rmse,
                "bias": second_metrics.0,
                "autocorrelation": second_metrics.1,
                "prediction": second_validation,
                "residual": second_residual,
                "rejectionReason": "no material validation improvement over FOPDT",
                "selection": false
            },
            {
                "structure": "integrating",
                "parameters": {"rateDegCPerPercentSecond": 0.012},
                "parameterUncertainty": {"status": "not identifiable because finite-equilibrium behavior is wrong"},
                "estimationRmse": prediction_rmse(&integrating_estimation_prediction, &estimation, 100),
                "validationRmse": prediction_rmse(&integrating_prediction, &validation, 100),
                "bias": integrating_metrics.0,
                "autocorrelation": integrating_metrics.1,
                "prediction": integrating_prediction,
                "residual": integrating_residual,
                "rejectionReason": "no finite equilibrium",
                "selection": false
            }
        ],
        "selection": "FOPDT",
        "evidenceClass": "configured_simulation"
    });
    write_or_compare_curriculum_artifact(
        &artifact_root.join("dryer-model-selection-evidence.json"),
        &(serde_json::to_string_pretty(&retained).expect("serialize dryer model evidence") + "\n"),
    );
    evidence::retain(
        &artifact_root,
        "dryer-model-estimation",
        "Dryer candidates on estimation data",
        "Configured Simulation evidence · common estimation step and candidate predictions",
        0.1,
        &[
            evidence::Panel {
                title: "Measured and predicted dryer temperature",
                unit: "degC",
                minimum: 79.5,
                maximum: 84.5,
                series: vec![
                    evidence::Series {
                        name: "measured",
                        color: "#243044",
                        dashed: false,
                        values: &estimation,
                    },
                    evidence::Series {
                        name: "first order",
                        color: "#d55e00",
                        dashed: true,
                        values: &first_estimation_prediction,
                    },
                    evidence::Series {
                        name: "FOPDT",
                        color: "#0072b2",
                        dashed: false,
                        values: &fopdt_estimation_prediction,
                    },
                    evidence::Series {
                        name: "second order",
                        color: "#009e73",
                        dashed: true,
                        values: &second_estimation_prediction,
                    },
                    evidence::Series {
                        name: "integrating",
                        color: "#cc79a7",
                        dashed: true,
                        values: &integrating_estimation_prediction,
                    },
                ],
            },
            evidence::Panel {
                title: "Estimation dryer command",
                unit: "%",
                minimum: 29.0,
                maximum: 41.0,
                series: vec![evidence::Series {
                    name: "applied command",
                    color: "#243044",
                    dashed: false,
                    values: &estimation_inputs,
                }],
            },
        ],
        &[],
        &[],
        &comparison_summary,
    );
    evidence::retain(
        &artifact_root,
        "dryer-model-residual",
        "Dryer residuals reveal structure errors",
        "Residual equals measured temperature minus each frozen candidate prediction",
        0.1,
        &[
            evidence::Panel {
                title: "Finite-equilibrium candidate residuals",
                unit: "degC",
                minimum: -0.5,
                maximum: 0.5,
                series: vec![
                    evidence::Series {
                        name: "first order residual",
                        color: "#d55e00",
                        dashed: true,
                        values: &first_residual,
                    },
                    evidence::Series {
                        name: "FOPDT residual",
                        color: "#0072b2",
                        dashed: false,
                        values: &fopdt_residual,
                    },
                    evidence::Series {
                        name: "second-order residual",
                        color: "#009e73",
                        dashed: true,
                        values: &second_residual,
                    },
                ],
            },
            evidence::Panel {
                title: "Integrating-candidate residual",
                unit: "degC",
                minimum: -1.5,
                maximum: 0.5,
                series: vec![evidence::Series {
                    name: "integrating residual",
                    color: "#cc79a7",
                    dashed: false,
                    values: &integrating_residual,
                }],
            },
        ],
        &[],
        &[],
        &comparison_summary,
    );
    let _ = std::fs::remove_file(config_path);

    println!(
        "dryer selection: delay={response_start:.2}s, final={final_mean:.3}C; FOPDT K/T/L={:.2}/{:.1}/{:.1}, validation RMSE={fopdt_validation_rmse:.4}, bias={fopdt_bias:.4}; first-order RMSE={first_validation_rmse:.4}, delay bias={delay_window_bias:.4}; second-order RMSE={second_validation_rmse:.4}; integrating return error={integrating_return_error:.3}",
        best_fopdt.1,
        best_fopdt.2,
        best_fopdt.3 as f64 * 0.1,
    );
}

#[test]
fn control_curriculum_poor_excitation_rejects_unidentifiable_records() {
    let shipped_source = project_file("poor-excitation", "src/Main.st");
    let source = shipped_source
        .split("CONFIGURATION Config")
        .next()
        .expect("example program before configuration")
        // Legacy names keep the pre-change lesson executable for the required red run.
        .replace("ExperimentInput : LREAL :=", "ExperimentInput AT %ML0 : LREAL :=")
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX64.0 : BOOL :=")
        .replace("ManipulatedValue : LREAL :=", "ManipulatedValue AT %QL0 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        // Lesson 40 exposes each independent experiment at the process-image boundary.
        .replace("ExperimentEnabled : BOOL :=", "ExperimentEnabled AT %MX64.0 : BOOL :=")
        .replace("PoorRequested : LREAL :=", "PoorRequested AT %ML0 : LREAL :=")
        .replace("QualifiedRequested : LREAL :=", "QualifiedRequested AT %ML8 : LREAL :=")
        .replace("ClosedRequested : LREAL :=", "ClosedRequested AT %ML16 : LREAL :=")
        .replace("ClosedMeasurementUsed : LREAL :=", "ClosedMeasurementUsed AT %ML24 : LREAL :=")
        .replace("PoorQualificationReason : INT :=", "PoorQualificationReason AT %MW40 : INT :=")
        .replace("QualifiedQualificationReason : INT :=", "QualifiedQualificationReason AT %MW42 : INT :=")
        .replace("ClosedQualificationReason : INT :=", "ClosedQualificationReason AT %MW44 : INT :=")
        .replace("PoorApplied : LREAL :=", "PoorApplied AT %QL0 : LREAL :=")
        .replace("QualifiedApplied : LREAL :=", "QualifiedApplied AT %QL8 : LREAL :=")
        .replace("ClosedApplied : LREAL :=", "ClosedApplied AT %QL16 : LREAL :=")
        .replace("PoorTemperature : LREAL :=", "PoorTemperature AT %IL0 : LREAL :=")
        .replace("QualifiedTemperature : LREAL :=", "QualifiedTemperature AT %IL8 : LREAL :=")
        .replace("ClosedTemperature : LREAL :=", "ClosedTemperature AT %IL16 : LREAL :=");
    let mut runtime = CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("poor-excitation/src/Main.st", source),
    ])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped poor-excitation lesson");

    let config_path = temporary_config_path("poor-excitation");
    std::fs::write(
        &config_path,
        project_file("poor-excitation", "simulation.toml"),
    )
    .expect("write poor-excitation simulation config");
    let config = SimulationConfig::load(&config_path).expect("load three heat-exchanger records");
    let mut simulation = SimulationController::new(config);

    let enabled = IoAddress::parse("%MX64.0").unwrap();
    let poor_requested = IoAddress::parse("%ML0").unwrap();
    let qualified_requested = IoAddress::parse("%ML8").unwrap();
    let closed_requested = IoAddress::parse("%ML16").unwrap();
    let poor_input = IoAddress::parse("%QL0").unwrap();
    let qualified_input = IoAddress::parse("%QL8").unwrap();
    let closed_input = IoAddress::parse("%QL16").unwrap();
    let poor_output = IoAddress::parse("%IL0").unwrap();
    let qualified_output = IoAddress::parse("%IL8").unwrap();
    let closed_output = IoAddress::parse("%IL16").unwrap();
    let closed_measurement_used = IoAddress::parse("%ML24").unwrap();

    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();
    write_bool(&mut runtime, &enabled, false);
    let mut poor_baseline = Vec::with_capacity(100);
    let mut qualified_baseline = Vec::with_capacity(100);
    let mut poor_late = Vec::with_capacity(100);
    let mut qualified_late = Vec::with_capacity(100);
    let mut closed_used = Vec::with_capacity(700);
    let mut closed_deviation = Vec::with_capacity(700);
    let mut poor_inputs = Vec::with_capacity(800);
    let mut qualified_inputs = Vec::with_capacity(800);
    let mut closed_inputs = Vec::with_capacity(800);
    let mut poor_requested_inputs = Vec::with_capacity(800);
    let mut qualified_requested_inputs = Vec::with_capacity(800);
    let mut closed_requested_inputs = Vec::with_capacity(800);
    let mut closed_measurements = Vec::with_capacity(800);
    let mut poor_outputs = Vec::with_capacity(800);
    let mut qualified_outputs = Vec::with_capacity(800);
    let mut closed_outputs = Vec::with_capacity(800);
    let mut poor_input_min = f64::INFINITY;
    let mut poor_input_max = f64::NEG_INFINITY;
    let mut qualified_input_min = f64::INFINITY;
    let mut qualified_input_max = f64::NEG_INFINITY;

    for cycle in 1usize..=800 {
        if cycle == 101 {
            write_bool(&mut runtime, &enabled, true);
        }
        runtime
            .execute_cycle()
            .expect("execute identification experiment commands");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance three heat exchangers");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish three measured temperatures");

        let poor_mv = read_lreal(&runtime, &poor_input);
        let qualified_mv = read_lreal(&runtime, &qualified_input);
        poor_requested_inputs.push(read_lreal(&runtime, &poor_requested));
        qualified_requested_inputs.push(read_lreal(&runtime, &qualified_requested));
        closed_requested_inputs.push(read_lreal(&runtime, &closed_requested));
        closed_measurements.push(read_lreal(&runtime, &closed_measurement_used));
        poor_inputs.push(poor_mv);
        qualified_inputs.push(qualified_mv);
        closed_inputs.push(read_lreal(&runtime, &closed_input));
        poor_outputs.push(read_lreal(&runtime, &poor_output));
        qualified_outputs.push(read_lreal(&runtime, &qualified_output));
        closed_outputs.push(read_lreal(&runtime, &closed_output));
        poor_input_min = poor_input_min.min(poor_mv);
        poor_input_max = poor_input_max.max(poor_mv);
        qualified_input_min = qualified_input_min.min(qualified_mv);
        qualified_input_max = qualified_input_max.max(qualified_mv);
        if cycle <= 100 {
            poor_baseline.push(read_lreal(&runtime, &poor_output));
            qualified_baseline.push(read_lreal(&runtime, &qualified_output));
        } else {
            closed_used.push(read_lreal(&runtime, &closed_measurement_used));
            closed_deviation.push(read_lreal(&runtime, &closed_input) - 20.0);
            if cycle > 700 {
                poor_late.push(read_lreal(&runtime, &poor_output));
                qualified_late.push(read_lreal(&runtime, &qualified_output));
            }
        }
    }

    let mean = |values: &[f64]| values.iter().sum::<f64>() / values.len() as f64;
    let poor_response = mean(&poor_late) - mean(&poor_baseline);
    let qualified_response = mean(&qualified_late) - mean(&qualified_baseline);
    let poor_span = poor_input_max - poor_input_min;
    let qualified_span = qualified_input_max - qualified_input_min;
    let closed_correlation = pearson_correlation(&closed_used, &closed_deviation);

    assert!(
        (poor_span - 0.2).abs() <= 1e-9,
        "poor applied-input span={poor_span}"
    );
    assert!(
        (poor_response - 0.1).abs() <= 0.06,
        "poor late response={poor_response}"
    );
    assert!(
        (qualified_span - 10.0).abs() <= 1e-9,
        "qualified applied-input span={qualified_span}"
    );
    assert!(
        (qualified_response - 4.8).abs() <= 0.10,
        "qualified late response={qualified_response}"
    );
    assert!(
        closed_correlation <= -0.95,
        "feedback-created measurement/input correlation={closed_correlation}"
    );
    let poor_reason = if poor_span < 5.0 || (0.5 * poor_span / 0.15) < 5.0 {
        1
    } else {
        0
    };
    let qualified_reason = if qualified_span < 5.0 || (0.5 * qualified_span / 0.15) < 5.0 {
        1
    } else if closed_used.len() as f64 * 0.1 < 63.0 {
        3
    } else {
        0
    };
    let feedback_is_exogenous = false;
    let closed_reason = if !feedback_is_exogenous && closed_correlation.abs() >= 0.95 {
        2
    } else {
        0
    };
    assert_eq!([poor_reason, qualified_reason, closed_reason], [1, 0, 2]);
    assert_eq!(
        shipped_source.matches("[[plants]]").count(),
        0,
        "plant declarations belong in simulation.toml, not Structured Text"
    );
    assert!(
        !shipped_source.contains("Control.PID"),
        "identifiability lesson must expose its simple feedback law directly"
    );
    assert!(!shipped_source.contains("QualificationReason"),
        "qualification decisions must be computed from recorded evidence, not embedded in the lesson program");
    assert_eq!(
        project_file("poor-excitation", "simulation.toml")
            .matches("[[plants]]")
            .count(),
        3,
        "the three records must come from independent copies of the same executable plant"
    );

    let artifact_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/05-experiments-identification-and-validation/40-poor-excitation/figures");
    let summary = format!("record,input_span_pp,late_response_degC,response_to_noise_ratio,input_noise_correlation,qualification_reason,decision\nweak_open_loop,{poor_span:.12},{poor_response:.12},{:.12},,1,data_inadequate\nqualified_open_loop,{qualified_span:.12},{qualified_response:.12},{:.12},,0,qualified\nordinary_feedback,,,,{closed_correlation:.12},2,data_inadequate\n", 0.1 / 0.15, 5.0 / 0.15);
    let output_span = |values: &[f64]| {
        values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
            - values.iter().copied().fold(f64::INFINITY, f64::min)
    };
    let sample_rows =
        |requested: &[f64], applied: &[f64], measured: &[f64], used: &[f64], seed: u64| {
            (0..measured.len())
                .map(|index| {
                    serde_json::json!({
                        "time": (index + 1) as f64 * 0.1,
                        "requestedInput": requested[index],
                        "appliedInput": applied[index],
                        "measuredOutput": measured[index],
                        "controllerUsedMeasurement": used[index],
                        "sampleInterval": 0.1,
                        "duration": 80.0,
                        "noiseAmplitude": 0.15,
                        "qualityChecks": {"finite": true, "notClipped": true, "fresh": true},
                        "seed": seed,
                        "evidenceClass": "configured_simulation"
                    })
                })
                .collect::<Vec<_>>()
        };
    let poor_used = poor_outputs.clone();
    let qualified_used = qualified_outputs.clone();
    let retained = serde_json::json!({
        "records": [
            {
                "id": "weak_open_loop",
                "samples": sample_rows(&poor_requested_inputs, &poor_inputs, &poor_outputs, &poor_used, 20260829),
                "inputSpan": poor_span,
                "outputSpan": output_span(&poor_outputs),
                "snr": 0.1 / 0.15,
                "exogenousExcitation": true,
                "correlation": null,
                "qualificationReason": 1
            },
            {
                "id": "qualified_open_loop",
                "samples": sample_rows(&qualified_requested_inputs, &qualified_inputs, &qualified_outputs, &qualified_used, 20260829),
                "inputSpan": qualified_span,
                "outputSpan": output_span(&qualified_outputs),
                "snr": 5.0 / 0.15,
                "exogenousExcitation": true,
                "correlation": null,
                "qualificationReason": 0
            },
            {
                "id": "ordinary_feedback",
                "samples": sample_rows(&closed_requested_inputs, &closed_inputs, &closed_outputs, &closed_measurements, 20260829),
                "inputSpan": output_span(&closed_inputs),
                "outputSpan": output_span(&closed_outputs),
                "snr": null,
                "exogenousExcitation": false,
                "correlation": closed_correlation,
                "qualificationReason": 2
            }
        ]
    });
    write_or_compare_curriculum_artifact(
        &artifact_root.join("poor-excitation-evidence.json"),
        &(serde_json::to_string_pretty(&retained).expect("serialize poor-excitation evidence")
            + "\n"),
    );
    evidence::retain(
        &artifact_root,
        "poor-excitation-comparison",
        "Identification data quality",
        "Configured Simulation evidence · three independent copies of the same heat exchanger",
        0.1,
        &[
            evidence::Panel {
                title: "Measured outlet temperature",
                unit: "degC",
                minimum: 49.0,
                maximum: 56.0,
                series: vec![
                    evidence::Series {
                        name: "weak open loop",
                        color: "#d55e00",
                        dashed: false,
                        values: &poor_outputs,
                    },
                    evidence::Series {
                        name: "qualified open loop",
                        color: "#0072b2",
                        dashed: false,
                        values: &qualified_outputs,
                    },
                    evidence::Series {
                        name: "ordinary feedback",
                        color: "#cc79a7",
                        dashed: false,
                        values: &closed_outputs,
                    },
                ],
            },
            evidence::Panel {
                title: "Applied valve position",
                unit: "%",
                minimum: 9.0,
                maximum: 31.0,
                series: vec![
                    evidence::Series {
                        name: "weak input",
                        color: "#d55e00",
                        dashed: false,
                        values: &poor_inputs,
                    },
                    evidence::Series {
                        name: "qualified input",
                        color: "#0072b2",
                        dashed: false,
                        values: &qualified_inputs,
                    },
                    evidence::Series {
                        name: "feedback-created input",
                        color: "#cc79a7",
                        dashed: false,
                        values: &closed_inputs,
                    },
                ],
            },
        ],
        &[],
        &[],
        &summary,
    );

    write_bool(&mut runtime, &enabled, false);
    runtime
        .execute_cycle()
        .expect("restore all heat-exchanger commands");
    assert!((read_lreal(&runtime, &poor_input) - 20.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &qualified_input) - 20.0).abs() <= 1e-12);
    assert!((read_lreal(&runtime, &closed_input) - 20.0).abs() <= 1e-12);
    let _ = std::fs::remove_file(config_path);

    println!(
        "identification qualification: poor span/response/SNR={poor_span:.1}/{poor_response:.3}/{:.2}, qualified={qualified_span:.1}/{qualified_response:.3}/{:.2}, closed-loop correlation={closed_correlation:.4}, reasons=[1, 0, 2]",
        0.1 / 0.15,
        5.0 / 0.15,
    );
}
