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

const CONTROL_SOURCE: &str = include_str!("../../../libraries/control/src/control.st");
const ADVANCED_SOURCE: &str = include_str!("../../../libraries/control/src/advanced_control.st");

fn project_file(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/14-integrated-capstones/82-distillation-column")
        .join(relative);
    assert!(
        path.is_file(),
        "Lesson 82 executable distillation capstone file is missing: {}",
        path.display()
    );
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read Lesson 82 file {}: {error}", path.display()))
}

fn temporary_config_path(label: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-distillation-capstone-{label}-{}-{stamp}.toml",
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

fn write_int(runtime: &mut Runtime, address: &IoAddress, value: i16) {
    runtime
        .io_mut()
        .write(address, Value::Word(value as u16))
        .expect("write INT architecture selector");
}

fn write_dint(runtime: &mut Runtime, address: &IoAddress, value: i32) {
    runtime
        .io_mut()
        .write(address, Value::DWord(value as u32))
        .expect("write DINT operating mode");
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

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL lesson signal") {
        Value::Bool(value) => value,
        value => panic!("expected BOOL, got {value:?}"),
    }
}

fn read_int(runtime: &Runtime, address: &IoAddress) -> i16 {
    match runtime.io().read(address).expect("read INT lesson signal") {
        Value::Word(value) => value as i16,
        value => panic!("expected INT, got {value:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &IoAddress) -> i32 {
    match runtime.io().read(address).expect("read DINT lesson signal") {
        Value::DWord(value) => value as i32,
        value => panic!("expected DINT, got {value:?}"),
    }
}

#[derive(Clone, Copy, Debug)]
enum Scenario {
    Overhead,
    Bottoms,
    Common,
    Feed,
    Constraint,
}

#[derive(Default)]
struct Trace {
    sp1: Vec<f64>,
    sp2: Vec<f64>,
    pv1: Vec<f64>,
    pv2: Vec<f64>,
    requested1: Vec<f64>,
    requested2: Vec<f64>,
    applied1: Vec<f64>,
    applied2: Vec<f64>,
    scale: Vec<f64>,
    event_end: usize,
}

fn compile_project() -> CompileSession {
    let source = project_file("src/Main.st")
        .split("CONFIGURATION Config")
        .next()
        .expect("distillation capstone program before configuration")
        .replace(
            "OperatingMode : DINT :=",
            "OperatingMode AT %ID16 : DINT :=",
        )
        .replace(
            "ArchitectureRequested : INT :=",
            "ArchitectureRequested AT %MW20 : INT :=",
        )
        .replace(
            "ArchitectureActive : INT :=",
            "ArchitectureActive AT %MW22 : INT :=",
        )
        .replace("QualityGood : BOOL :=", "QualityGood AT %MX24.0 : BOOL :=")
        .replace(
            "RefluxAvailable : BOOL :=",
            "RefluxAvailable AT %MX24.1 : BOOL :=",
        )
        .replace(
            "ReboilerAvailable : BOOL :=",
            "ReboilerAvailable AT %MX24.2 : BOOL :=",
        )
        .replace(
            "ArchitectureQualified : BOOL :=",
            "ArchitectureQualified AT %MX24.3 : BOOL :=",
        )
        .replace(
            "OverheadSetpoint : LREAL :=",
            "OverheadSetpoint AT %ML32 : LREAL :=",
        )
        .replace(
            "BottomsSetpoint : LREAL :=",
            "BottomsSetpoint AT %ML40 : LREAL :=",
        )
        .replace(
            "FeedDisturbanceRequested : LREAL :=",
            "FeedDisturbanceRequested AT %ML48 : LREAL :=",
        )
        .replace(
            "RefluxRequested : LREAL :=",
            "RefluxRequested AT %ML72 : LREAL :=",
        )
        .replace(
            "ReboilerRequested : LREAL :=",
            "ReboilerRequested AT %ML80 : LREAL :=",
        )
        .replace("CommonScale : LREAL :=", "CommonScale AT %ML88 : LREAL :=")
        .replace(
            "ManualReflux : LREAL :=",
            "ManualReflux AT %ML96 : LREAL :=",
        )
        .replace(
            "ManualReboiler : LREAL :=",
            "ManualReboiler AT %ML104 : LREAL :=",
        )
        .replace(
            "TrackingActive : BOOL :=",
            "TrackingActive AT %MX24.6 : BOOL :=",
        )
        .replace("FaultActive : BOOL :=", "FaultActive AT %MX24.7 : BOOL :=")
        .replace(
            "ControllerRevision : DINT :=",
            "ControllerRevision AT %MD112 : DINT :=",
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
            "FeedDisturbanceApplied : LREAL :=",
            "FeedDisturbanceApplied AT %QL16 : LREAL :=",
        )
        .replace(
            "OverheadProcessValue : LREAL :=",
            "OverheadProcessValue AT %IL0 : LREAL :=",
        )
        .replace(
            "BottomsProcessValue : LREAL :=",
            "BottomsProcessValue AT %IL8 : LREAL :=",
        );
    CompileSession::from_sources(vec![
        SourceFile::with_path("Control/control.st", CONTROL_SOURCE),
        SourceFile::with_path("Control/advanced_control.st", ADVANCED_SOURCE),
        SourceFile::with_path(
            "capstones/distillation-column/src/DistillationController.st",
            project_file("src/DistillationController.st"),
        ),
        SourceFile::with_path("capstones/distillation-column/src/Main.st", source),
    ])
    .label_errors(true)
}

fn run(session: &CompileSession, model_file: &str, architecture: i16, scenario: Scenario) -> Trace {
    let mut runtime = session
        .build_runtime()
        .expect("compile a fresh runtime from the shipped lesson");

    let path = temporary_config_path(model_file);
    std::fs::write(&path, project_file(model_file)).expect("write column simulation config");
    let config = SimulationConfig::load(&path).expect("load column simulation config");
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial product deviations");
    let _ = std::fs::remove_file(path);

    let operating_mode = IoAddress::parse("%ID16").unwrap();
    let architecture_address = IoAddress::parse("%MW20").unwrap();
    let quality = IoAddress::parse("%MX24.0").unwrap();
    let reflux_available = IoAddress::parse("%MX24.1").unwrap();
    let reboiler_available = IoAddress::parse("%MX24.2").unwrap();
    let setpoint1 = IoAddress::parse("%ML32").unwrap();
    let setpoint2 = IoAddress::parse("%ML40").unwrap();
    let disturbance = IoAddress::parse("%ML48").unwrap();
    let requested1 = IoAddress::parse("%ML72").unwrap();
    let requested2 = IoAddress::parse("%ML80").unwrap();
    let scale = IoAddress::parse("%ML88").unwrap();
    let pv1 = IoAddress::parse("%IL0").unwrap();
    let pv2 = IoAddress::parse("%IL8").unwrap();
    let applied1 = IoAddress::parse("%QL0").unwrap();
    let applied2 = IoAddress::parse("%QL8").unwrap();

    let event_start = 200;
    let event_end = if matches!(scenario, Scenario::Constraint) {
        4_200
    } else {
        2_600
    };
    let sample_count = event_end + 3_000;
    let mut trace = Trace {
        event_end,
        ..Trace::default()
    };
    for cycle in 0..sample_count {
        let event_active = cycle >= event_start && cycle < event_end;
        let (sp1, sp2, load) = if event_active {
            match scenario {
                Scenario::Overhead => (1.0, 0.0, 0.0),
                Scenario::Bottoms => (0.0, 1.0, 0.0),
                Scenario::Common => (1.0, 1.0, 0.0),
                Scenario::Feed => (0.0, 0.0, 2.0),
                Scenario::Constraint => (2.0, 2.0, 0.0),
            }
        } else {
            (0.0, 0.0, 0.0)
        };
        write_dint(
            &mut runtime,
            &operating_mode,
            if cycle < event_end { 1 } else { 2 },
        );
        write_int(&mut runtime, &architecture_address, architecture);
        write_bool(&mut runtime, &quality, true);
        write_bool(&mut runtime, &reflux_available, true);
        write_bool(&mut runtime, &reboiler_available, true);
        write_lreal(&mut runtime, &setpoint1, sp1);
        write_lreal(&mut runtime, &setpoint2, sp2);
        write_lreal(&mut runtime, &disturbance, load);
        runtime
            .execute_cycle()
            .expect("execute robust two-loop controller");
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance coupled column");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish product deviations");
        trace.sp1.push(sp1);
        trace.sp2.push(sp2);
        trace.pv1.push(read_lreal(&runtime, &pv1));
        trace.pv2.push(read_lreal(&runtime, &pv2));
        trace.requested1.push(read_lreal(&runtime, &requested1));
        trace.requested2.push(read_lreal(&runtime, &requested2));
        trace.applied1.push(read_lreal(&runtime, &applied1));
        trace.applied2.push(read_lreal(&runtime, &applied2));
        trace.scale.push(read_lreal(&runtime, &scale));
    }
    trace
}

#[derive(Debug)]
struct Metrics {
    final_error: f64,
    cross_peak: f64,
    iae: f64,
    applied_travel: f64,
    limit_duration: f64,
    restoration_input: f64,
    restoration_error: f64,
    minimum_scale: f64,
}

fn metrics(trace: &Trace, scenario: Scenario) -> Metrics {
    let event_start = 200;
    let final_start = trace.event_end - 100;
    let final_error = (final_start..trace.event_end)
        .map(|index| {
            (trace.sp1[index] - trace.pv1[index]).abs()
                + (trace.sp2[index] - trace.pv2[index]).abs()
        })
        .sum::<f64>()
        / 100.0;
    let cross_peak = match scenario {
        Scenario::Overhead => trace.pv2[event_start..trace.event_end]
            .iter()
            .map(|value| value.abs())
            .fold(0.0, f64::max),
        Scenario::Bottoms => trace.pv1[event_start..trace.event_end]
            .iter()
            .map(|value| value.abs())
            .fold(0.0, f64::max),
        _ => 0.0,
    };
    let iae = (event_start..trace.event_end)
        .map(|index| {
            ((trace.sp1[index] - trace.pv1[index]).abs()
                + (trace.sp2[index] - trace.pv2[index]).abs())
                * 0.1
        })
        .sum();
    let applied_travel = trace.applied1[event_start..trace.event_end]
        .windows(2)
        .chain(trace.applied2[event_start..trace.event_end].windows(2))
        .map(|pair| (pair[1] - pair[0]).abs())
        .sum();
    let limit_duration = (event_start..trace.event_end)
        .filter(|index| {
            trace.applied1[*index].abs() >= 9.999 || trace.applied2[*index].abs() >= 9.999
        })
        .count() as f64
        * 0.1;
    let restoration_start = trace.pv1.len() - 100;
    let restoration_input = (restoration_start..trace.pv1.len())
        .map(|index| trace.applied1[index].abs() + trace.applied2[index].abs())
        .sum::<f64>()
        / 100.0;
    let restoration_error = (restoration_start..trace.pv1.len())
        .map(|index| trace.pv1[index].abs() + trace.pv2[index].abs())
        .sum::<f64>()
        / 100.0;
    Metrics {
        final_error,
        cross_peak,
        iae,
        applied_travel,
        limit_duration,
        restoration_input,
        restoration_error,
        minimum_scale: trace.scale[event_start..trace.event_end]
            .iter()
            .copied()
            .fold(1.0, f64::min),
    }
}

fn assert_valid(trace: &Trace) {
    assert!(trace
        .sp1
        .iter()
        .chain(&trace.sp2)
        .chain(&trace.pv1)
        .chain(&trace.pv2)
        .chain(&trace.requested1)
        .chain(&trace.requested2)
        .chain(&trace.applied1)
        .chain(&trace.applied2)
        .chain(&trace.scale)
        .all(|value| value.is_finite()));
    assert!(trace
        .pv1
        .iter()
        .chain(&trace.pv2)
        .all(|value| value.abs() <= 6.0));
    assert!(trace
        .applied1
        .iter()
        .chain(&trace.applied2)
        .all(|value| value.abs() <= 10.0 + 1e-9));
}

fn run_manual_record(session: &CompileSession, command: [f64; 2]) -> Vec<[f64; 2]> {
    let mut runtime = session
        .build_runtime()
        .expect("compile a fresh identification runtime from the shipped capstone");
    let config = SimulationConfig::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/control/14-integrated-capstones/82-distillation-column/simulation.toml"),
    )
    .expect("load nominal column for four-path identification");
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial identification outputs");

    let mode = IoAddress::parse("%ID16").unwrap();
    let architecture = IoAddress::parse("%MW20").unwrap();
    let quality = IoAddress::parse("%MX24.0").unwrap();
    let reflux_available = IoAddress::parse("%MX24.1").unwrap();
    let reboiler_available = IoAddress::parse("%MX24.2").unwrap();
    let manual_reflux = IoAddress::parse("%ML96").unwrap();
    let manual_reboiler = IoAddress::parse("%ML104").unwrap();
    let pv1 = IoAddress::parse("%IL0").unwrap();
    let pv2 = IoAddress::parse("%IL8").unwrap();
    let mut outputs = Vec::with_capacity(2_200);
    for cycle in 0..2_200 {
        write_dint(&mut runtime, &mode, 2);
        write_int(&mut runtime, &architecture, 0);
        write_bool(&mut runtime, &quality, true);
        write_bool(&mut runtime, &reflux_available, true);
        write_bool(&mut runtime, &reboiler_available, true);
        write_lreal(
            &mut runtime,
            &manual_reflux,
            if cycle < 200 { 0.0 } else { command[0] },
        );
        write_lreal(
            &mut runtime,
            &manual_reboiler,
            if cycle < 200 { 0.0 } else { command[1] },
        );
        runtime
            .execute_cycle()
            .expect("execute manual identification scan");
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance identification plant");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish identification outputs");
        outputs.push([read_lreal(&runtime, &pv1), read_lreal(&runtime, &pv2)]);
    }
    outputs
}

fn record_mean(record: &[[f64; 2]], output: usize, range: std::ops::Range<usize>) -> f64 {
    range
        .clone()
        .map(|index| record[index][output])
        .sum::<f64>()
        / range.len() as f64
}

fn identified_response_time(record: &[[f64; 2]], output: usize) -> f64 {
    let baseline = record_mean(record, output, 100..200);
    let final_value = record_mean(record, output, 2_100..2_200);
    let target = baseline + 0.632_120_558_8 * (final_value - baseline);
    let rising = final_value >= baseline;
    record[200..]
        .iter()
        .position(|sample| {
            if rising {
                sample[output] >= target
            } else {
                sample[output] <= target
            }
        })
        .expect("identified path reaches its 63.2-percent point") as f64
        * 0.1
}

fn identify_four_paths(session: &CompileSession) -> ([[f64; 2]; 2], [[f64; 2]; 2], f64) {
    let reflux = run_manual_record(session, [1.0, 0.0]);
    let reboiler = run_manual_record(session, [0.0, 1.0]);
    let simultaneous = run_manual_record(session, [1.0, 1.0]);
    let gain = [
        [
            record_mean(&reflux, 0, 2_100..2_200),
            record_mean(&reboiler, 0, 2_100..2_200),
        ],
        [
            record_mean(&reflux, 1, 2_100..2_200),
            record_mean(&reboiler, 1, 2_100..2_200),
        ],
    ];
    let response_time = [
        [
            identified_response_time(&reflux, 0),
            identified_response_time(&reboiler, 0),
        ],
        [
            identified_response_time(&reflux, 1),
            identified_response_time(&reboiler, 1),
        ],
    ];
    let simultaneous_final = [
        record_mean(&simultaneous, 0, 2_100..2_200),
        record_mean(&simultaneous, 1, 2_100..2_200),
    ];
    let validation_error = (simultaneous_final[0] - gain[0][0] - gain[0][1])
        .abs()
        .max((simultaneous_final[1] - gain[1][0] - gain[1][1]).abs());
    (gain, response_time, validation_error)
}

fn assert_abnormal_authority(session: &CompileSession) {
    let mut runtime = session
        .build_runtime()
        .expect("compile abnormal-authority runtime from the shipped capstone");
    let config = SimulationConfig::load(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../examples/control/14-integrated-capstones/82-distillation-column/simulation.toml"),
    )
    .expect("load nominal column for abnormal-authority validation");
    let mut simulation = SimulationController::new(config);
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .unwrap();

    let mode = IoAddress::parse("%ID16").unwrap();
    let architecture_request = IoAddress::parse("%MW20").unwrap();
    let architecture_active = IoAddress::parse("%MW22").unwrap();
    let quality = IoAddress::parse("%MX24.0").unwrap();
    let reflux_available = IoAddress::parse("%MX24.1").unwrap();
    let reboiler_available = IoAddress::parse("%MX24.2").unwrap();
    let architecture_qualified = IoAddress::parse("%MX24.3").unwrap();
    let tracking = IoAddress::parse("%MX24.6").unwrap();
    let fault = IoAddress::parse("%MX24.7").unwrap();
    let sp1 = IoAddress::parse("%ML32").unwrap();
    let sp2 = IoAddress::parse("%ML40").unwrap();
    let manual1 = IoAddress::parse("%ML96").unwrap();
    let manual2 = IoAddress::parse("%ML104").unwrap();
    let applied1 = IoAddress::parse("%QL0").unwrap();
    let applied2 = IoAddress::parse("%QL8").unwrap();
    let revision = IoAddress::parse("%MD112").unwrap();
    let mut previous_applied = [0.0, 0.0];
    for cycle in 0..700 {
        let operating_mode = if (500..530).contains(&cycle) {
            2
        } else if (530..600).contains(&cycle) {
            3
        } else if (600..620).contains(&cycle) {
            0
        } else {
            1
        };
        write_dint(&mut runtime, &mode, operating_mode);
        write_int(
            &mut runtime,
            &architecture_request,
            if (300..350).contains(&cycle) { 9 } else { 1 },
        );
        write_bool(&mut runtime, &quality, !(400..420).contains(&cycle));
        write_bool(
            &mut runtime,
            &reflux_available,
            !(430..450).contains(&cycle),
        );
        write_bool(&mut runtime, &reboiler_available, true);
        write_lreal(&mut runtime, &sp1, 1.0);
        write_lreal(&mut runtime, &sp2, 1.0);
        write_lreal(&mut runtime, &manual1, 2.0);
        write_lreal(&mut runtime, &manual2, -2.0);
        runtime
            .execute_cycle()
            .expect("execute abnormal authority scan");
        let applied = [
            read_lreal(&runtime, &applied1),
            read_lreal(&runtime, &applied2),
        ];
        assert_eq!(read_dint(&runtime, &revision), 82);
        if (300..350).contains(&cycle) {
            assert_eq!(read_int(&runtime, &architecture_active), 0);
            assert!(!read_bool(&runtime, &architecture_qualified));
            assert!(!read_bool(&runtime, &fault));
        }
        if (400..420).contains(&cycle) || (430..450).contains(&cycle) || (600..620).contains(&cycle)
        {
            assert!(applied.iter().all(|value| value.abs() <= 1.0e-12));
            assert!(read_bool(&runtime, &tracking));
        }
        if (500..600).contains(&cycle) {
            assert!(read_bool(&runtime, &tracking));
            assert!(applied
                .iter()
                .zip(previous_applied)
                .all(|(current, previous)| (current - previous).abs() <= 0.2 + 1.0e-9));
        }
        if cycle == 599 {
            assert!(applied.iter().all(|value| value.abs() <= 1.0e-12));
        }
        previous_applied = applied;
        let now = Duration::from_millis((cycle as i64 + 1) * 100);
        simulation.apply_post_cycle(now, &runtime).unwrap();
        simulation.apply_pre_cycle(now, &mut runtime).unwrap();
    }
}

fn singular_values(matrix: [[f64; 2]; 2]) -> [f64; 2] {
    let trace = matrix
        .iter()
        .flatten()
        .map(|value| value * value)
        .sum::<f64>();
    let determinant = matrix[0][0] * matrix[1][1] - matrix[0][1] * matrix[1][0];
    let discriminant = (trace * trace - 4.0 * determinant * determinant)
        .max(0.0)
        .sqrt();
    [
        ((trace + discriminant) / 2.0).sqrt(),
        ((trace - discriminant) / 2.0).sqrt(),
    ]
}

#[derive(Clone, Copy)]
struct Complex {
    real: f64,
    imaginary: f64,
}

impl Complex {
    fn new(real: f64, imaginary: f64) -> Self {
        Self { real, imaginary }
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.real + other.real, self.imaginary + other.imaginary)
    }

    fn multiply(self, other: Self) -> Self {
        Self::new(
            self.real * other.real - self.imaginary * other.imaginary,
            self.real * other.imaginary + self.imaginary * other.real,
        )
    }

    fn divide(self, other: Self) -> Self {
        let denominator = other.real * other.real + other.imaginary * other.imaginary;
        Self::new(
            (self.real * other.real + self.imaginary * other.imaginary) / denominator,
            (self.imaginary * other.real - self.real * other.imaginary) / denominator,
        )
    }

    fn magnitude_squared(self) -> f64 {
        self.real * self.real + self.imaginary * self.imaginary
    }
}

fn multiply_2x2(left: [[Complex; 2]; 2], right: [[Complex; 2]; 2]) -> [[Complex; 2]; 2] {
    let zero = Complex::new(0.0, 0.0);
    let mut result = [[zero; 2]; 2];
    for row in 0..2 {
        for column in 0..2 {
            result[row][column] = left[row][0]
                .multiply(right[0][column])
                .add(left[row][1].multiply(right[1][column]));
        }
    }
    result
}

fn inverse_2x2(matrix: [[Complex; 2]; 2]) -> [[Complex; 2]; 2] {
    let determinant = matrix[0][0]
        .multiply(matrix[1][1])
        .add(Complex::new(-1.0, 0.0).multiply(matrix[0][1].multiply(matrix[1][0])));
    let minus_one = Complex::new(-1.0, 0.0);
    [
        [
            matrix[1][1].divide(determinant),
            minus_one.multiply(matrix[0][1]).divide(determinant),
        ],
        [
            minus_one.multiply(matrix[1][0]).divide(determinant),
            matrix[0][0].divide(determinant),
        ],
    ]
}

fn maximum_singular_value(matrix: [[Complex; 2]; 2]) -> f64 {
    let trace = matrix
        .iter()
        .flatten()
        .map(|value| value.magnitude_squared())
        .sum::<f64>();
    let determinant = matrix[0][0]
        .multiply(matrix[1][1])
        .add(Complex::new(-1.0, 0.0).multiply(matrix[0][1].multiply(matrix[1][0])));
    let determinant_squared = determinant.magnitude_squared();
    ((trace + (trace * trace - 4.0 * determinant_squared).max(0.0).sqrt()) / 2.0).sqrt()
}

fn sensitivity_peak(gains: [[f64; 2]; 2], response_times: [[f64; 2]; 2], decoupled: bool) -> f64 {
    let decoupler = if decoupled {
        [
            [1.272727272727273, 0.636363636363636],
            [0.545454545454545, 1.272727272727273],
        ]
    } else {
        [[1.0, 0.0], [0.0, 1.0]]
    };
    let zero = Complex::new(0.0, 0.0);
    let mut peak: f64 = 0.0;
    for index in 0..=500 {
        let frequency = 0.002 * (1_000.0_f64).powf(index as f64 / 500.0);
        let mut plant = [[zero; 2]; 2];
        for row in 0..2 {
            for column in 0..2 {
                plant[row][column] = Complex::new(gains[row][column], 0.0)
                    .divide(Complex::new(1.0, frequency * response_times[row][column]));
            }
        }
        let mapping = [
            [
                Complex::new(decoupler[0][0], 0.0),
                Complex::new(decoupler[0][1], 0.0),
            ],
            [
                Complex::new(decoupler[1][0], 0.0),
                Complex::new(decoupler[1][1], 0.0),
            ],
        ];
        let controllers = [
            [
                Complex::new(1.666666666666667, -0.0833333333333333 / frequency),
                zero,
            ],
            [
                zero,
                Complex::new(2.142857142857143, -0.0714285714285714 / frequency),
            ],
        ];
        let loop_matrix = multiply_2x2(multiply_2x2(plant, mapping), controllers);
        let sensitivity = inverse_2x2([
            [
                loop_matrix[0][0].add(Complex::new(1.0, 0.0)),
                loop_matrix[0][1],
            ],
            [
                loop_matrix[1][0],
                loop_matrix[1][1].add(Complex::new(1.0, 0.0)),
            ],
        ]);
        peak = peak.max(maximum_singular_value(sensitivity));
    }
    peak
}

#[test]
fn control_curriculum_distillation_capstone_selects_robust_architecture() {
    let lesson: serde_json::Value =
        serde_json::from_str(&project_file("lesson.json")).expect("parse Lesson 82 descriptor");
    assert_eq!(lesson["schemaVersion"], 4);
    assert_eq!(lesson["sequence"], 82);
    assert_eq!(lesson["prerequisites"][0], "control-capstone-conveyor-oven");
    assert_eq!(lesson["nextLesson"], "control-capstone-building-hvac");
    assert_eq!(
        lesson["proof"]["nativeTest"],
        "control_curriculum_distillation_capstone_selects_robust_architecture"
    );
    let main_source = project_file("src/Main.st");
    let controller_source = project_file("src/DistillationController.st");
    assert_eq!(controller_source.matches("Control.PID;").count(), 2);
    assert_eq!(
        controller_source
            .matches("Control.StaticDecoupler2x2;")
            .count(),
        1
    );
    let source = format!("{main_source}\n{controller_source}");
    for forbidden in [
        "SelectedArchitecture",
        "SingularValue",
        "SensitivityPeak",
        "RobustWinner",
    ] {
        assert!(
            !source.contains(forbidden),
            "ST embeds analysis verdict '{forbidden}'"
        );
    }
    let session = compile_project();

    let (identified_gain, identified_time, identification_validation_error) =
        identify_four_paths(&session);
    let expected_gain = [[0.8, -0.4], [-0.3, 0.7]];
    let expected_time = [[20.0, 35.0], [40.0, 30.0]];
    for row in 0..2 {
        for column in 0..2 {
            assert!(
                (identified_gain[row][column] - expected_gain[row][column]).abs() <= 0.01,
                "identified gain[{row}][{column}]={}",
                identified_gain[row][column]
            );
            assert!(
                (identified_time[row][column] - expected_time[row][column]).abs() <= 1.0,
                "identified response time[{row}][{column}]={}",
                identified_time[row][column]
            );
        }
    }
    assert!(identification_validation_error <= 0.01);
    let determinant = identified_gain[0][0] * identified_gain[1][1]
        - identified_gain[0][1] * identified_gain[1][0];
    assert!(
        determinant.abs() > 0.1,
        "identified gain matrix must be nonsingular"
    );
    let rga = [
        [
            identified_gain[0][0] * identified_gain[1][1] / determinant,
            identified_gain[0][1] * -identified_gain[1][0] / determinant,
        ],
        [
            identified_gain[1][0] * -identified_gain[0][1] / determinant,
            identified_gain[1][1] * identified_gain[0][0] / determinant,
        ],
    ];
    assert!(rga[0][0] > 0.0 && rga[1][1] > 0.0);
    assert!(rga[0][1] < 0.0 && rga[1][0] < 0.0);
    assert!((rga[0][0] - 1.272727).abs() <= 0.01);
    assert_abnormal_authority(&session);

    let nominal = [[1.0, -0.5], [-0.428571428571429, 1.0]];
    let interaction = [[1.0, -0.8], [-0.7, 1.0]];
    let nominal_singular = singular_values(nominal);
    let interaction_singular = singular_values(interaction);
    let uncertainty = singular_values([[0.0, -0.3], [-0.271428571428571, 0.0]]);
    assert!((nominal_singular[0] - 1.464923).abs() <= 0.001);
    assert!((nominal_singular[1] - 0.536352).abs() <= 0.001);
    assert!((interaction_singular[0] - 1.751249).abs() <= 0.001);
    assert!((interaction_singular[1] - 0.251249).abs() <= 0.001);
    assert!((nominal_singular[0] / nominal_singular[1] - 2.73127).abs() <= 0.01);
    assert!((interaction_singular[0] / interaction_singular[1] - 6.97017).abs() <= 0.01);
    assert!((uncertainty[0] - 0.3).abs() <= 0.001);
    assert!(uncertainty[0] < nominal_singular[1]);

    let model_data = [
        (
            "simulation.toml",
            [[0.8, -0.4], [-0.3, 0.7]],
            [[20.0, 35.0], [40.0, 30.0]],
        ),
        (
            "simulation-strong-interaction.toml",
            [[0.8, -0.64], [-0.49, 0.7]],
            [[20.0, 45.0], [50.0, 30.0]],
        ),
    ];
    for (name, gains, response_times) in model_data {
        let direct_peak = sensitivity_peak(gains, response_times, false);
        let decoupled_peak = sensitivity_peak(gains, response_times, true);
        assert!(
            direct_peak <= 1.20,
            "{name} direct sensitivity peak={direct_peak}"
        );
        assert!(
            decoupled_peak <= 1.20,
            "{name} decoupled sensitivity peak={decoupled_peak}"
        );
        eprintln!("{name} sensitivity direct/static={direct_peak:.6}/{decoupled_peak:.6}");
    }

    let scenarios = [
        Scenario::Overhead,
        Scenario::Bottoms,
        Scenario::Common,
        Scenario::Feed,
    ];
    let mut direct_has_failure = false;
    let mut worst_final = [0.0_f64; 3];
    let mut worst_cross = [0.0_f64; 3];
    for (model_file, _, _) in model_data {
        let records: Vec<Vec<(Trace, Metrics)>> = (0..=2)
            .map(|architecture| {
                scenarios
                    .iter()
                    .map(|scenario| {
                        let trace = run(&session, model_file, architecture, *scenario);
                        assert_valid(&trace);
                        let metrics = metrics(&trace, *scenario);
                        (trace, metrics)
                    })
                    .collect()
            })
            .collect();
        for (scenario_index, scenario) in scenarios.iter().enumerate() {
            for architecture in 0..3 {
                worst_final[architecture] = worst_final[architecture]
                    .max(records[architecture][scenario_index].1.final_error);
                worst_cross[architecture] = worst_cross[architecture]
                    .max(records[architecture][scenario_index].1.cross_peak);
            }
            let direct = &records[0][scenario_index].1;
            let static_case = &records[1][scenario_index].1;
            let centralized = &records[2][scenario_index].1;
            direct_has_failure |= direct.final_error > 0.10
                || direct.cross_peak > 0.20
                || direct.limit_duration > 0.5;
            for (label, result) in [("static", static_case), ("centralized", centralized)] {
                assert!(
                    result.final_error <= 0.10,
                    "{model_file} {scenario:?} {label} final error={}",
                    result.final_error
                );
                assert!(
                    result.cross_peak <= 0.20,
                    "{model_file} {scenario:?} {label} cross peak={}",
                    result.cross_peak
                );
                assert!(result.limit_duration <= 0.5);
                assert!(
                    result.restoration_input <= 0.02,
                    "{model_file} {scenario:?} {label} restoration input={}",
                    result.restoration_input
                );
                assert!(
                    result.restoration_error <= 0.02,
                    "{model_file} {scenario:?} {label} restoration error={}",
                    result.restoration_error
                );
            }
            assert!((static_case.final_error - centralized.final_error).abs() <= 1e-9);
            assert!((static_case.iae - centralized.iae).abs() <= 1e-9);
            assert!((static_case.applied_travel - centralized.applied_travel).abs() <= 1e-9);
            assert!(records[2][scenario_index]
                .0
                .scale
                .iter()
                .all(|scale| (*scale - 1.0).abs() <= 1e-12));
            eprintln!(
                "{model_file} {scenario:?}: direct/static/central final={:.4}/{:.4}/{:.4}, cross={:.4}/{:.4}/{:.4}, IAE={:.3}/{:.3}/{:.3}, travel={:.3}/{:.3}/{:.3}",
                direct.final_error,
                static_case.final_error,
                centralized.final_error,
                direct.cross_peak,
                static_case.cross_peak,
                centralized.cross_peak,
                direct.iae,
                static_case.iae,
                centralized.iae,
                direct.applied_travel,
                static_case.applied_travel,
                centralized.applied_travel,
            );
        }
    }
    assert!(
        direct_has_failure,
        "direct architecture must expose the coupled limitation"
    );

    let constraint_records: Vec<_> = (0..=2)
        .map(|architecture| {
            let trace = run(
                &session,
                "simulation-strong-interaction.toml",
                architecture,
                Scenario::Constraint,
            );
            assert_valid(&trace);
            let result = metrics(&trace, Scenario::Constraint);
            (trace, result)
        })
        .collect();
    assert!(constraint_records
        .iter()
        .all(|(_, result)| result.final_error > 0.10));
    assert!(constraint_records[2].1.minimum_scale < 0.99);
    let centralized_trace = &constraint_records[2].0;
    for index in 200..centralized_trace.event_end {
        if centralized_trace.scale[index] < 0.999 {
            let direction_error = centralized_trace.requested1[index]
                * centralized_trace.applied2[index]
                - centralized_trace.requested2[index] * centralized_trace.applied1[index];
            assert!(direction_error.abs() <= 1e-8);
        }
    }
    let static_trace = &constraint_records[1].0;
    assert!((200..static_trace.event_end).any(|index| {
        static_trace.requested1[index]
            .abs()
            .max(static_trace.requested2[index].abs())
            > 10.0
            && (static_trace.requested1[index] * static_trace.applied2[index]
                - static_trace.requested2[index] * static_trace.applied1[index])
                .abs()
                > 0.01
    }));
    eprintln!(
        "constraint direct/static/central final={:.4}/{:.4}/{:.4}, minimum common scale={:.4}; static is selected for the normal envelope and the +2/+2 request remains Infeasible",
        constraint_records[0].1.final_error,
        constraint_records[1].1.final_error,
        constraint_records[2].1.final_error,
        constraint_records[2].1.minimum_scale,
    );

    let architecture = [0.0, 1.0, 2.0];
    let constraint_final: Vec<f64> = constraint_records
        .iter()
        .map(|(_, result)| result.final_error)
        .collect();
    let mut summary = String::from("architecture,worst_normal_final_error_pp,worst_cross_peak_pp,constraint_final_error_pp,minimum_constraint_scale,verdict\n");
    for (index, name) in ["direct", "static-decoupling", "central-coordination"]
        .iter()
        .enumerate()
    {
        let verdict = if index == 1 {
            "selected-normal-envelope"
        } else if index == 0 {
            "reject-coupling"
        } else {
            "no-material-benefit"
        };
        writeln!(
            summary,
            "{name},{:.9},{:.9},{:.9},{:.9},{verdict}",
            worst_final[index],
            worst_cross[index],
            constraint_records[index].1.final_error,
            constraint_records[index].1.minimum_scale
        )
        .unwrap();
    }
    evidence::retain(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/control/14-integrated-capstones/82-distillation-column/figures"),
        "distillation-capstone-evidence",
        "Static decoupling earns the normal-envelope distillation recommendation",
        "Indices 0, 1, and 2 are direct pairing, static decoupling, and central constraint coordination; the infeasible request remains labelled infeasible.",
        &[
            Panel {
                title: "Worst normal-envelope final error",
                x_unit: "architecture index",
                y_unit: "percentage points",
                x_minimum: -0.1,
                x_maximum: 2.1,
                y_minimum: 0.0,
                y_maximum: 0.20,
                series: vec![Series { name: "Final error", color: "#2563eb", dashed: false, x: &architecture, y: &worst_final }],
            },
            Panel {
                title: "Infeasible +2/+2 final error",
                x_unit: "architecture index",
                y_unit: "percentage points",
                x_minimum: -0.1,
                x_maximum: 2.1,
                y_minimum: 0.0,
                y_maximum: 1.1,
                series: vec![Series { name: "Constraint-case error", color: "#dc2626", dashed: false, x: &architecture, y: &constraint_final }],
            },
        ],
        &[RetainedSeries { panel: "Normal envelope", name: "Cross peak", x: &architecture, y: &worst_cross, x_unit: "architecture index", y_unit: "percentage points" }],
        &summary,
    );
}
