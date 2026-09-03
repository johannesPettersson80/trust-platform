use std::fmt::Write as _;
use std::path::PathBuf;

use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const LOOP_ANATOMY_SOURCE: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/src/Main.st");
const LOOP_ANATOMY_SIMULATION: &str =
    include_str!("../../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/simulation.toml");

#[derive(Clone, Copy)]
struct ManualSample {
    time_s: f64,
    requested_mv: f64,
    applied_mv: f64,
    pv: f64,
    door_open: f64,
    effective_surroundings: f64,
}

fn temporary_config_path(name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "trust-control-curriculum-{name}-{}-{stamp}.toml",
        std::process::id()
    ))
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

fn run_manual_observation() -> Vec<ManualSample> {
    let source = LOOP_ANATOMY_SOURCE
        .split("CONFIGURATION Config")
        .next()
        .expect("lesson program before configuration")
        .replace(
            "ManualDemand : LREAL :=",
            "ManualDemand AT %ML40 : LREAL :=",
        )
        .replace(
            "ExperimentEnabled : BOOL :=",
            "ExperimentEnabled AT %IX48.0 : BOOL :=",
        )
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
        );
    let mut runtime = CompileSession::from_sources(vec![SourceFile::with_path(
        "cold-storage-loop-anatomy/src/Main.st",
        source,
    )])
    .label_errors(true)
    .build_runtime()
    .expect("compile shipped loop-anatomy lesson");
    let config_path = temporary_config_path("cold-storage-loop-anatomy-figure");
    std::fs::write(&config_path, LOOP_ANATOMY_SIMULATION).expect("write loop-anatomy simulation");
    let mut simulation = SimulationController::new(
        SimulationConfig::load(&config_path).expect("load loop-anatomy simulation"),
    );
    let request = IoAddress::parse("%ML40").expect("project-owned request address");
    let authority = IoAddress::parse("%IX48.0").expect("experiment authority address");
    let pv = IoAddress::parse("%IL0").expect("PV address");
    let mv = IoAddress::parse("%QL0").expect("MV address");
    let door_open = IoAddress::parse("%IL8").expect("door indication address");
    let effective_surroundings = IoAddress::parse("%IL16").expect("effective surroundings address");
    simulation
        .apply_pre_cycle(Duration::ZERO, &mut runtime)
        .expect("publish initial room temperature");

    let mut trace = Vec::with_capacity(1201);
    for cycle in 1..=1201 {
        write_bool(&mut runtime, &authority, cycle <= 1200);
        runtime.execute_cycle().expect("execute manual command");
        let requested_mv = read_lreal(&runtime, &request);
        let now = Duration::from_millis((cycle * 100) as i64);
        simulation
            .apply_post_cycle(now, &runtime)
            .expect("advance cold-storage room");
        simulation
            .apply_pre_cycle(now, &mut runtime)
            .expect("publish cold-room temperature");
        trace.push(ManualSample {
            time_s: cycle as f64 * 0.1,
            requested_mv,
            applied_mv: read_lreal(&runtime, &mv),
            pv: read_lreal(&runtime, &pv),
            door_open: read_lreal(&runtime, &door_open),
            effective_surroundings: read_lreal(&runtime, &effective_surroundings),
        });
    }
    let _ = std::fs::remove_file(config_path);
    trace
}

fn export_manual_trace(directory: &std::path::Path, trace: &[ManualSample]) {
    std::fs::create_dir_all(directory).expect("create figure directory");
    let mut csv = String::from(
        "time_s,setpoint_degC,requested_mv_percent,applied_mv_percent,pv_degC,door_open,effective_surroundings_degC,evidence_class\n",
    );
    for sample in trace {
        writeln!(
            csv,
            "{:.1},2.0,{:.1},{:.1},{:.6},{:.0},{:.1},simulation",
            sample.time_s,
            sample.requested_mv,
            sample.applied_mv,
            sample.pv,
            sample.door_open,
            sample.effective_surroundings
        )
        .expect("write CSV row");
    }
    std::fs::write(directory.join("manual-cooling-response.csv"), csv)
        .expect("write retained simulation evidence");
}

#[test]
fn control_curriculum_cold_storage_loop_anatomy_exports_real_manual_trace() {
    let trace = run_manual_observation();
    assert_eq!(trace.len(), 1201);
    assert!(trace[..200]
        .iter()
        .all(|sample| sample.requested_mv == 150.0 && sample.applied_mv == 100.0));
    assert!(trace[200..]
        .iter()
        .all(|sample| sample.requested_mv == 0.0 && sample.applied_mv == 0.0));
    assert!(
        trace[199].pv <= 1.5,
        "bounded cooling interval must lower room temperature visibly; pv={}",
        trace[199].pv
    );
    assert!(
        trace[399].pv >= trace[199].pv + 1.5,
        "safe-state restoration must let the room warm; cooled={}, recovered={}",
        trace[199].pv,
        trace[399].pv
    );
    assert!(trace[..599]
        .iter()
        .all(|sample| sample.door_open == 0.0 && sample.effective_surroundings == 4.0));
    assert!(trace[599..899]
        .iter()
        .all(|sample| sample.door_open == 1.0 && sample.effective_surroundings == 6.0));
    assert!(trace[899..]
        .iter()
        .all(|sample| sample.door_open == 0.0 && sample.effective_surroundings == 4.0));
    const DOOR_LOAD_RISE_LIMIT_C: f64 = 1.50;
    const NUMERICAL_TOLERANCE_C: f64 = 0.05;
    assert!(
        trace[898].pv >= trace[598].pv + DOOR_LOAD_RISE_LIMIT_C - NUMERICAL_TOLERANCE_C,
        "the open-door disturbance must visibly warm the uncontrolled room; before={}, after={}",
        trace[598].pv,
        trace[898].pv
    );
    assert!(trace.iter().all(|sample| sample.pv.is_finite()));

    if let Some(directory) = std::env::var_os("TRUST_CURRICULUM_FIGURE_DIR") {
        export_manual_trace(&PathBuf::from(directory), &trace);
    }
}
