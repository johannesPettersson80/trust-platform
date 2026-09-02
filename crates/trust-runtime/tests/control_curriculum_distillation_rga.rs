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
        .join("../../examples/control/10-multivariable-control/61-distillation-mimo/figures")
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
        .join("../../examples/control/10-multivariable-control/61-distillation-mimo")
        .join(relative);
    assert!(
        path.is_file(),
        "required Lesson 61 file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 61 file {}: {error}", path.display()))
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-distillation-rga-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL interaction-test input");
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL interaction-test mode");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime
        .io()
        .read(address)
        .expect("read LREAL interaction-test signal")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        value => panic!("expected LREAL, got {value:?}"),
    }
}

struct ColumnBench {
    runtime: Runtime,
    simulation: SimulationController,
    now_ms: i64,
    auto_mode: IoAddress,
    commands: [IoAddress; 2],
    applied: [IoAddress; 2],
    compositions: [IoAddress; 2],
}

impl ColumnBench {
    fn new() -> Self {
        let source = project_file("src/Main.st")
            .split("CONFIGURATION Config")
            .next()
            .expect("distillation lesson program before configuration")
            .replace("AutoMode : BOOL :=", "AutoMode AT %MX16.0 : BOOL :=")
            .replace(
                "RefluxRequested : LREAL :=",
                "RefluxRequested AT %ML0 : LREAL :=",
            )
            .replace(
                "ReboilerRequested : LREAL :=",
                "ReboilerRequested AT %ML8 : LREAL :=",
            )
            .replace(
                "RefluxApplied : LREAL :=",
                "RefluxApplied AT %QL0 : LREAL :=",
            )
            .replace(
                "ReboilerApplied : LREAL :=",
                "ReboilerApplied AT %QL8 : LREAL :=",
            )
            .replace(
                "OverheadPurityDeviation : LREAL :=",
                "OverheadPurityDeviation AT %IL0 : LREAL :=",
            )
            .replace(
                "BottomsPurityDeviation : LREAL :=",
                "BottomsPurityDeviation AT %IL8 : LREAL :=",
            );
        let runtime = CompileSession::from_sources(vec![SourceFile::with_path(
            "distillation-mimo/src/Main.st",
            source,
        )])
        .label_errors(true)
        .build_runtime()
        .expect("compile shipped distillation interaction lesson");

        let path = temporary_config_path();
        std::fs::write(&path, project_file("simulation.toml"))
            .expect("write distillation interaction simulation config");
        let config = SimulationConfig::load(&path).expect("load two-input column model");
        assert_eq!(config.plant_system.plants.len(), 1);
        let mut simulation = SimulationController::new(config);
        let mut runtime = runtime;
        simulation
            .apply_pre_cycle(Duration::ZERO, &mut runtime)
            .expect("publish initial composition deviations");
        let _ = std::fs::remove_file(path);

        Self {
            runtime,
            simulation,
            now_ms: 0,
            auto_mode: IoAddress::parse("%MX16.0").unwrap(),
            commands: ["%ML0", "%ML8"].map(|address| IoAddress::parse(address).unwrap()),
            applied: ["%QL0", "%QL8"].map(|address| IoAddress::parse(address).unwrap()),
            compositions: ["%IL0", "%IL8"].map(|address| IoAddress::parse(address).unwrap()),
        }
    }

    fn step(&mut self, inputs: [f64; 2]) -> ([f64; 2], [f64; 2]) {
        write_bool(&mut self.runtime, &self.auto_mode, false);
        for (address, value) in self.commands.iter().zip(inputs) {
            write_lreal(&mut self.runtime, address, value);
        }
        self.runtime
            .execute_cycle()
            .expect("execute manual command path");
        self.now_ms += 100;
        let now = Duration::from_millis(self.now_ms);
        self.simulation
            .apply_post_cycle(now, &self.runtime)
            .expect("advance binary-column model");
        self.simulation
            .apply_pre_cycle(now, &mut self.runtime)
            .expect("publish composition deviations");
        (
            self.applied
                .each_ref()
                .map(|address| read_lreal(&self.runtime, address)),
            self.compositions
                .each_ref()
                .map(|address| read_lreal(&self.runtime, address)),
        )
    }
}

struct StepRecord {
    applied: Vec<[f64; 2]>,
    outputs: Vec<[f64; 2]>,
}

fn run_restored_step(excitation: [f64; 2]) -> StepRecord {
    let mut bench = ColumnBench::new();
    let mut applied = Vec::with_capacity(2_000);
    let mut outputs = Vec::with_capacity(2_000);
    for cycle in 0..2_000 {
        let inputs = if cycle < 200 { [0.0, 0.0] } else { excitation };
        let (actual_inputs, actual_outputs) = bench.step(inputs);
        applied.push(actual_inputs);
        outputs.push(actual_outputs);
    }
    StepRecord { applied, outputs }
}

fn mean(values: impl Iterator<Item = f64>) -> f64 {
    let values: Vec<_> = values.collect();
    values.iter().sum::<f64>() / values.len() as f64
}

fn span(values: impl Iterator<Item = f64>) -> f64 {
    let values: Vec<_> = values.collect();
    values.iter().copied().fold(f64::NEG_INFINITY, f64::max)
        - values.iter().copied().fold(f64::INFINITY, f64::min)
}

fn response_time_seconds(record: &StepRecord, output_index: usize) -> f64 {
    let baseline = mean(
        record.outputs[100..200]
            .iter()
            .map(|outputs| outputs[output_index]),
    );
    let final_output = mean(
        record.outputs[1_900..]
            .iter()
            .map(|outputs| outputs[output_index]),
    );
    let target = baseline + 0.632_120_558_8 * (final_output - baseline);
    let rising = final_output >= baseline;
    let crossing = record.outputs[200..]
        .iter()
        .position(|outputs| {
            if rising {
                outputs[output_index] >= target
            } else {
                outputs[output_index] <= target
            }
        })
        .expect("path reaches its 63.2-percent response point");
    (crossing + 1) as f64 * 0.1
}

#[test]
fn control_curriculum_distillation_steps_identify_gain_matrix_and_rga_pairing() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 61 descriptor");
    assert_eq!(
        lesson["schemaVersion"], 4,
        "Lesson 61 must use the pedagogical schema"
    );
    assert_eq!(lesson["sequence"], 61);
    assert_eq!(lesson["prerequisites"][0], "control-valve-stiction");
    assert_eq!(lesson["nextLesson"], "control-distillation-decoupling");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_distillation_steps_identify_gain_matrix_and_rga_pairing"
    );
    let source = project_file("src/Main.st");
    for forbidden in ["RelativeGain", "PairingVerdict", "Rga11", "Determinant"] {
        assert!(
            !source.contains(forbidden),
            "Structured Text must not hard-code analytical result '{forbidden}'"
        );
    }

    let records = [run_restored_step([5.0, 0.0]), run_restored_step([0.0, 5.0])];
    let mut gain = [[0.0_f64; 2]; 2];
    for (input_index, record) in records.iter().enumerate() {
        let other = 1 - input_index;
        assert!(
            record
                .applied
                .iter()
                .all(|inputs| inputs[other].abs() <= 0.02),
            "the non-excited input must remain fixed"
        );
        for output_index in 0..2 {
            let final_span = span(
                record.outputs[1_900..]
                    .iter()
                    .map(|outputs| outputs[output_index]),
            );
            assert!(
                final_span <= 0.02,
                "path y{}-u{} did not settle: final span {final_span:.6} pp",
                output_index + 1,
                input_index + 1
            );
            let baseline = mean(
                record.outputs[100..200]
                    .iter()
                    .map(|outputs| outputs[output_index]),
            );
            let final_output = mean(
                record.outputs[1_900..]
                    .iter()
                    .map(|outputs| outputs[output_index]),
            );
            let baseline_input = mean(
                record.applied[100..200]
                    .iter()
                    .map(|inputs| inputs[input_index]),
            );
            let final_input = mean(
                record.applied[1_900..]
                    .iter()
                    .map(|inputs| inputs[input_index]),
            );
            let input_change = final_input - baseline_input;
            assert!(
                input_change.abs() >= 4.95,
                "u{} step is too small to identify a gain: baseline {baseline_input:.6}, final {final_input:.6}, first applied after step {:?}",
                input_index + 1,
                record.applied[200]
            );
            gain[output_index][input_index] = (final_output - baseline) / input_change;
        }
    }

    let expected = [[0.8, -0.4], [-0.3, 0.7]];
    for row in 0..2 {
        for column in 0..2 {
            assert!(
                (gain[row][column] - expected[row][column]).abs() <= 0.01,
                "g{}{} expected {:.4}, measured {:.6}",
                row + 1,
                column + 1,
                expected[row][column],
                gain[row][column]
            );
        }
    }

    let response_times = [
        response_time_seconds(&records[0], 0),
        response_time_seconds(&records[1], 0),
        response_time_seconds(&records[0], 1),
        response_time_seconds(&records[1], 1),
    ];
    for (measured, expected) in response_times.into_iter().zip([20.0, 35.0, 40.0, 30.0]) {
        assert!(
            (measured - expected).abs() <= 1.0,
            "path response time expected {expected:.1} s, measured {measured:.1} s"
        );
    }

    let determinant = gain[0][0] * gain[1][1] - gain[0][1] * gain[1][0];
    assert!(
        determinant.abs() >= 0.1,
        "gain matrix is too close to singular"
    );
    let cross = -(gain[0][1] * gain[1][0]) / determinant;
    let diagonal = gain[0][0] * gain[1][1] / determinant;
    let relative_gain = [[diagonal, cross], [cross, diagonal]];
    for row in &relative_gain {
        assert!((row.iter().sum::<f64>() - 1.0).abs() <= 0.001);
    }
    for column in 0..2 {
        assert!(((relative_gain[0][column] + relative_gain[1][column]) - 1.0).abs() <= 0.001);
    }
    assert!(relative_gain[0][0] > 0.0 && relative_gain[1][1] > 0.0);
    assert!(relative_gain[0][1] < 0.0 && relative_gain[1][0] < 0.0);
    println!(
        "identified G={gain:?}, response times={response_times:?} s, det={determinant:.6}, RGA={relative_gain:?}; diagonal pairing accepted, cross pairing rejected"
    );

    let time = (0..records[0].outputs.len())
        .map(|sample| sample as f64 * 0.1)
        .collect::<Vec<_>>();
    let reflux_y1 = records[0]
        .outputs
        .iter()
        .map(|value| value[0])
        .collect::<Vec<_>>();
    let reflux_y2 = records[0]
        .outputs
        .iter()
        .map(|value| value[1])
        .collect::<Vec<_>>();
    let reboiler_y1 = records[1]
        .outputs
        .iter()
        .map(|value| value[0])
        .collect::<Vec<_>>();
    let reboiler_y2 = records[1]
        .outputs
        .iter()
        .map(|value| value[1])
        .collect::<Vec<_>>();
    let reflux_u1 = records[0]
        .applied
        .iter()
        .map(|value| value[0])
        .collect::<Vec<_>>();
    let reflux_u2 = records[0]
        .applied
        .iter()
        .map(|value| value[1])
        .collect::<Vec<_>>();
    let reboiler_u1 = records[1]
        .applied
        .iter()
        .map(|value| value[0])
        .collect::<Vec<_>>();
    let reboiler_u2 = records[1]
        .applied
        .iter()
        .map(|value| value[1])
        .collect::<Vec<_>>();
    let (reflux_min, reflux_max) = plot_bounds([reflux_y1.as_slice(), reflux_y2.as_slice()]);
    let (reboiler_min, reboiler_max) =
        plot_bounds([reboiler_y1.as_slice(), reboiler_y2.as_slice()]);
    let summary = format!(
        "quantity,value_11,value_12,value_21,value_22,status\nsteady_gain,{:.6},{:.6},{:.6},{:.6},identified\nresponse_time_s,{:.6},{:.6},{:.6},{:.6},identified\nrelative_gain,{:.6},{:.6},{:.6},{:.6},diagonal_pairing_qualified\ndeterminant,{determinant:.6},,,,nonsingular\n",
        gain[0][0], gain[0][1], gain[1][0], gain[1][1],
        response_times[0], response_times[1], response_times[2], response_times[3],
        relative_gain[0][0], relative_gain[0][1], relative_gain[1][0], relative_gain[1][1],
    );
    evidence::retain(
        &evidence_root(),
        "distillation-rga-evidence",
        "Two restored steps reveal column interaction",
        "Each input moves both product compositions; the measured gain matrix supports diagonal pairing.",
        &[
            evidence::Panel {
                title: "Five-point reflux step", x_unit: "time (s)", y_unit: "composition deviation (pp)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(), y_minimum: reflux_min, y_maximum: reflux_max,
                series: vec![
                    evidence::Series { name: "Overhead", color: "#2563eb", dashed: false, x: &time, y: &reflux_y1 },
                    evidence::Series { name: "Bottoms", color: "#dc2626", dashed: false, x: &time, y: &reflux_y2 },
                ],
            },
            evidence::Panel {
                title: "Five-point reboiler step", x_unit: "time (s)", y_unit: "composition deviation (pp)",
                x_minimum: 0.0, x_maximum: *time.last().unwrap(), y_minimum: reboiler_min, y_maximum: reboiler_max,
                series: vec![
                    evidence::Series { name: "Overhead", color: "#2563eb", dashed: false, x: &time, y: &reboiler_y1 },
                    evidence::Series { name: "Bottoms", color: "#dc2626", dashed: false, x: &time, y: &reboiler_y2 },
                ],
            },
        ],
        &[
            evidence::RetainedSeries { panel: "Reflux experiment", name: "Reflux applied", x: &time, y: &reflux_u1, x_unit: "time (s)", y_unit: "command (pp)" },
            evidence::RetainedSeries { panel: "Reflux experiment", name: "Reboiler held", x: &time, y: &reflux_u2, x_unit: "time (s)", y_unit: "command (pp)" },
            evidence::RetainedSeries { panel: "Reboiler experiment", name: "Reflux held", x: &time, y: &reboiler_u1, x_unit: "time (s)", y_unit: "command (pp)" },
            evidence::RetainedSeries { panel: "Reboiler experiment", name: "Reboiler applied", x: &time, y: &reboiler_u2, x_unit: "time (s)", y_unit: "command (pp)" },
        ],
        &summary,
    );
}
