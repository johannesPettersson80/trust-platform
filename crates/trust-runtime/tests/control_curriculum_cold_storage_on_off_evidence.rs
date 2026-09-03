use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/src/Main.st");
const SIMULATION: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/02-cold-storage-on-off/simulation.toml");

#[derive(Clone, Copy)]
struct Sample {
    time_s: f64,
    pv: f64,
    mv: f64,
    switch_count: i32,
    auto_mode: bool,
    door_open: f64,
    effective_surroundings: f64,
}

#[derive(Clone, Copy)]
struct NoiseSample {
    underlying_temperature: f64,
    measurement_noise: f64,
    measured_pv: f64,
    mv: f64,
    transition: bool,
}

fn temporary_config_path() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-curriculum-cold-storage-on-off-{}-{stamp}.toml",
        std::process::id()
    ))
}

fn addressed_source() -> String {
    SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("lesson program before configuration")
        .replace("AutoMode : BOOL :=", "AutoMode AT %MX28.0 : BOOL :=")
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX24.0 : BOOL :=",
        )
        .replace("Setpoint : LREAL :=", "Setpoint AT %IL32 : LREAL :=")
        .replace("ProcessValue : LREAL :=", "ProcessValue AT %IL0 : LREAL :=")
        .replace(
            "ManipulatedValue : LREAL :=",
            "ManipulatedValue AT %QL0 : LREAL :=",
        )
        .replace(
            "DoorOpenIndication : LREAL :=",
            "DoorOpenIndication AT %IL8 : LREAL :=",
        )
        .replace(
            "EffectiveSurroundings : LREAL :=",
            "EffectiveSurroundings AT %IL16 : LREAL :=",
        )
        .replace("SwitchCount : DINT :=", "SwitchCount AT %MD24 : DINT :=")
}

fn write_lreal(runtime: &mut Runtime, address: &IoAddress, value: f64) {
    runtime
        .io_mut()
        .write(address, Value::LWord(value.to_bits()))
        .expect("write LREAL process-image value");
}

fn write_bool(runtime: &mut Runtime, address: &IoAddress, value: bool) {
    runtime
        .io_mut()
        .write(address, Value::Bool(value))
        .expect("write BOOL process-image value");
}

fn read_lreal(runtime: &Runtime, address: &IoAddress) -> f64 {
    match runtime.io().read(address).expect("read LREAL value") {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL storage, got {other:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &IoAddress) -> bool {
    match runtime.io().read(address).expect("read BOOL value") {
        Value::Bool(value) => value,
        other => panic!("expected BOOL storage, got {other:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &IoAddress) -> i32 {
    match runtime.io().read(address).expect("read DINT value") {
        Value::DInt(value) => value,
        Value::DWord(value) => value as i32,
        other => panic!("expected DINT storage, got {other:?}"),
    }
}

fn compile_controller() -> Runtime {
    CompileSession::from_sources(vec![SourceFile::with_path(
        "cold-storage-on-off/src/Main.st",
        addressed_source(),
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped ON/OFF lesson")
}

fn run_closed_loop() -> Vec<Sample> {
    let mut runtime = compile_controller();
    let config_path = temporary_config_path();
    std::fs::write(&config_path, SIMULATION).expect("write ON/OFF simulation configuration");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load ON/OFF simulation configuration"),
    );
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let authority = IoAddress::parse("%IX24.0").expect("experiment authority address");
    let auto = IoAddress::parse("%MX28.0").expect("derived Auto address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let switch_count = IoAddress::parse("%MD24").expect("switch-count address");
    let door_open = IoAddress::parse("%IL8").expect("door address");
    let surroundings = IoAddress::parse("%IL16").expect("surroundings address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial plant state");

    write_lreal(&mut runtime, &sp, 2.0);
    write_bool(&mut runtime, &authority, false);
    runtime.execute_cycle().expect("execute initial safe scan");
    assert_eq!(
        read_lreal(&runtime, &mv),
        0.0,
        "the lesson must remain at zero demand before Auto is explicitly enabled"
    );

    let mut trace = Vec::with_capacity(1201);
    for cycle in 1..=1200 {
        write_lreal(&mut runtime, &sp, 2.0);
        write_bool(&mut runtime, &authority, true);
        runtime.execute_cycle().expect("execute ON/OFF controller");
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance cold-storage room");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish cold-storage feedback");
        trace.push(Sample {
            time_s: cycle as f64 * 0.1,
            pv: read_lreal(&runtime, &pv),
            mv: read_lreal(&runtime, &mv),
            switch_count: read_dint(&runtime, &switch_count),
            auto_mode: read_bool(&runtime, &auto),
            door_open: read_lreal(&runtime, &door_open),
            effective_surroundings: read_lreal(&runtime, &surroundings),
        });
    }

    write_bool(&mut runtime, &authority, false);
    runtime
        .execute_cycle()
        .expect("execute explicit safe restoration");
    trace.push(Sample {
        time_s: 120.1,
        pv: read_lreal(&runtime, &pv),
        mv: read_lreal(&runtime, &mv),
        switch_count: read_dint(&runtime, &switch_count),
        auto_mode: read_bool(&runtime, &auto),
        door_open: read_lreal(&runtime, &door_open),
        effective_surroundings: read_lreal(&runtime, &surroundings),
    });
    let _ = std::fs::remove_file(config_path);
    trace
}

fn run_noisy_crossings() -> Vec<NoiseSample> {
    let mut runtime = compile_controller();
    let sp = IoAddress::parse("%IL32").expect("SP address");
    let authority = IoAddress::parse("%IX24.0").expect("experiment authority address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let mut previous = None;
    [0.05, -0.05, 0.04, -0.04, 0.03, -0.03, 0.02, -0.02]
        .into_iter()
        .map(|measurement_noise| {
            let underlying_temperature = 2.0;
            let process_value = underlying_temperature + measurement_noise;
            write_lreal(&mut runtime, &sp, 2.0);
            write_bool(&mut runtime, &authority, true);
            write_lreal(&mut runtime, &pv, process_value);
            runtime.execute_cycle().expect("execute noisy crossing");
            let output = read_lreal(&runtime, &mv);
            let transition = previous.is_some_and(|last| last != output);
            previous = Some(output);
            NoiseSample {
                underlying_temperature,
                measurement_noise,
                measured_pv: process_value,
                mv: output,
                transition,
            }
        })
        .collect()
}

fn export_evidence(directory: &std::path::Path, trace: &[Sample], chatter: &[NoiseSample]) {
    std::fs::create_dir_all(directory).expect("create Lesson 2 figure directory");
    let mut response = String::from(
        "time_s,setpoint_degC,pv_degC,mv_percent,switch_count,auto_mode,phase,door_open,effective_surroundings_degC,evidence_class\n",
    );
    for (index, sample) in trace.iter().enumerate() {
        writeln!(
            response,
            "{:.1},2.0,{:.6},{:.1},{},{},{},{:.0},{:.1},simulation",
            sample.time_s,
            sample.pv,
            sample.mv,
            sample.switch_count,
            u8::from(sample.auto_mode),
            if index < 1200 {
                "experiment"
            } else {
                "safe_restoration"
            },
            sample.door_open,
            sample.effective_surroundings
        )
        .expect("write response row");
    }
    std::fs::write(directory.join("on-off-response.csv"), response)
        .expect("write ON/OFF response evidence");

    let mut crossings =
        String::from("sample,underlying_temperature_degC,measurement_noise_degC,measured_pv_degC,mv_percent,transition,evidence_class\n");
    for (index, sample) in chatter.iter().enumerate() {
        writeln!(
            crossings,
            "{},{:.2},{:.2},{:.2},{:.1},{},simulation",
            index + 1,
            sample.underlying_temperature,
            sample.measurement_noise,
            sample.measured_pv,
            sample.mv,
            u8::from(sample.transition)
        )
        .expect("write chatter row");
    }
    std::fs::write(directory.join("threshold-chatter.csv"), crossings)
        .expect("write chatter evidence");
}

#[test]
fn control_curriculum_cold_storage_on_off_exports_regulation_and_chatter_evidence() {
    let trace = run_closed_loop();
    let chatter = run_noisy_crossings();
    assert_eq!(trace.len(), 1201);
    let experiment = &trace[..1200];
    let restoration = trace.last().expect("safe-restoration sample");
    assert!(trace
        .iter()
        .all(|sample| sample.pv.is_finite() && matches!(sample.mv, 0.0 | 100.0)));
    assert!(experiment.iter().all(|sample| sample.auto_mode));
    assert!(!restoration.auto_mode);
    assert_eq!(restoration.mv, 0.0);
    assert!(experiment[..599]
        .iter()
        .all(|sample| sample.door_open == 0.0 && sample.effective_surroundings == 4.0));
    assert!(experiment[599..899]
        .iter()
        .all(|sample| sample.door_open == 1.0 && sample.effective_surroundings == 6.0));
    assert!(experiment[899..]
        .iter()
        .all(|sample| sample.door_open == 0.0 && sample.effective_surroundings == 4.0));
    let door_samples = &experiment[599..899];
    let door_minimum = door_samples
        .iter()
        .map(|sample| sample.pv)
        .fold(f64::INFINITY, f64::min);
    let door_maximum = door_samples
        .iter()
        .map(|sample| sample.pv)
        .fold(f64::NEG_INFINITY, f64::max);
    assert!(
        door_maximum - door_minimum <= 0.05 + 0.01,
        "door-event range exceeded the published criterion: min={door_minimum}, max={door_maximum}"
    );
    assert!(
        door_maximum <= 3.00 + 0.05,
        "door-event maximum exceeded the separate performance criterion: {door_maximum}"
    );
    assert_eq!(
        experiment
            .last()
            .expect("final experiment sample")
            .switch_count,
        760
    );
    let switching_frequency =
        f64::from(experiment[599].switch_count - experiment[199].switch_count) / (60.0 - 20.0);
    assert!(
        (switching_frequency - 8.0).abs() <= 0.01,
        "unexpected pre-door switching frequency: {switching_frequency}"
    );
    assert_eq!(chatter.iter().filter(|sample| sample.transition).count(), 7);
    assert!(chatter.iter().all(|sample| {
        sample.underlying_temperature == 2.0
            && (sample.measured_pv
                - (sample.underlying_temperature + sample.measurement_noise))
                .abs()
                <= f64::EPSILON
    }), "the focused crossing experiment must isolate measurement noise from a constant underlying temperature");

    if let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR") {
        export_evidence(&PathBuf::from(directory), &trace, &chatter);
    }
}
