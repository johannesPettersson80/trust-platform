use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const MAIN: &str = include_str!(
    "../../../examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz/src/Main.st"
);

fn project_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/07-classical-stability-and-frequency-design/44-routh-hurwitz")
}

fn read(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL bits at {address}, got {other:?}"),
    }
}

fn run_gain(gain: i32) -> Vec<(f64, f64, f64, bool)> {
    let project = project_path();
    let source = MAIN.replace("Kp : LREAL := 1.0;", &format!("Kp : LREAL := {gain}.0;"));
    let session = CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", source)]);
    let mut runtime = session.build_runtime().expect("compile lesson ST");
    runtime
        .apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
        .unwrap();
    let debug = runtime.enable_debug();
    let mut simulation =
        SimulationController::new(SimulationConfig::load(project.join("simulation.toml")).unwrap());
    let mut samples = Vec::new();
    for scan in 0..500 {
        simulation
            .apply_pre_cycle(Duration::from_millis(scan * 20), &mut runtime)
            .unwrap();
        debug.enqueue_io_write(IoAddress::parse("%MX96.0").unwrap(), Value::Bool(true));
        runtime.advance_time(Duration::from_millis(20));
        runtime.execute_cycle().unwrap();
        samples.push((
            read(&runtime, "%IL0"),
            read(&runtime, "%IL8"),
            read(&runtime, "%QL0"),
            runtime
                .io()
                .read(&IoAddress::parse("%MX96.3").unwrap())
                .unwrap()
                == Value::Bool(true),
        ));
        simulation
            .apply_post_cycle(Duration::from_millis((scan + 1) * 20), &runtime)
            .unwrap();
    }
    samples
}

fn assert_declared_shaft_plant() {
    let project = project_path();
    let simulation =
        SimulationController::new(SimulationConfig::load(project.join("simulation.toml")).unwrap());
    let status = simulation.status();
    assert_eq!(
        status.plants.len(),
        1,
        "Lesson 52 must configure one shaft plant"
    );
    let plant = &status.plants[0];
    assert_eq!(plant.id, "shaft-j1-b1");
    assert_eq!(plant.model, "state_space");
    assert_eq!(plant.step, Duration::from_millis(20));
    assert_eq!(plant.state, vec![0.0, 0.0]);
}

#[test]
fn routh_shaft_gain_cases_match_the_declared_bounded_observations() {
    assert_declared_shaft_plant();
    let positive = run_gain(1);
    assert!(
        (positive[0].2 - 0.01).abs() < 1e-9,
        "Kp=1 must apply u=Kp*(0.01-theta) from the declared shaft initial state"
    );
    assert!(positive.iter().all(|(angle, velocity, torque, _)| {
        angle.abs() <= 0.2 && velocity.abs() <= 2.0 && torque.abs() <= 20.0
    }));
    assert!((0.01 - positive.last().unwrap().0).abs() < 0.001);

    let zero = run_gain(0);
    assert!(zero.iter().all(|(angle, velocity, torque, _)| {
        angle.abs() < 1e-9 && velocity.abs() < 1e-9 && torque.abs() < 1e-9
    }));

    let negative = run_gain(-1);
    assert!(negative.iter().any(|(angle, _, _, _)| angle.abs() > 0.02));
    assert!(
        negative.last().unwrap().3,
        "the safety stop must restore Manual mode"
    );
    assert_eq!(
        negative.last().unwrap().2,
        0.0,
        "the safety stop must restore zero torque"
    );
}
