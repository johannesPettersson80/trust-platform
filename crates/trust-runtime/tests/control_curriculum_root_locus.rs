use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::simulation::{SimulationConfig, SimulationController};
use trust_runtime::value::{Duration, Value};
use trust_runtime::CompiledRuntime;

mod control_heater_support;
use control_heater_support::{compile, LessonRuntime};

const MAIN: &str = include_str!(
    "../../../examples/control/07-classical-stability-and-frequency-design/45-root-locus/src/Main.st"
);

fn read(runtime: &CompiledRuntime, address: &str) -> f64 {
    match runtime
        .read_io(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        Value::LReal(value) => value,
        other => panic!("expected LREAL bits at {address}, got {other:?}"),
    }
}

#[test]
fn root_locus_shaft_candidates_meet_the_declared_step_requirements() {
    let project = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/control/07-classical-stability-and-frequency-design/45-root-locus");
    for gain in [12, 16, 20] {
        let source = MAIN.replace("Kp : LREAL := 16.0;", &format!("Kp : LREAL := {gain}.0;"));
        let session =
            CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", source)]);
        let mut runtime = compile(session);
        let debug = runtime.enable_debug();
        let mut simulation = SimulationController::new(
            SimulationConfig::load(project.join("simulation.toml")).unwrap(),
        );
        let mut peak = 0.0_f64;
        let mut late_error = 0.0_f64;
        for scan in 0..401 {
            simulation
                .apply_pre_cycle(Duration::from_millis(scan * 20), &mut runtime)
                .unwrap();
            debug.enqueue_io_write(IoAddress::parse("%MX96.0").unwrap(), Value::Bool(true));
            runtime.advance_time(Duration::from_millis(20));
            runtime.execute_cycle().unwrap();
            let angle = read(&runtime, "%IL0");
            let velocity = read(&runtime, "%IL8");
            let torque = read(&runtime, "%QL0");
            if scan == 0 {
                assert!(
                    (torque - gain as f64).abs() < 1e-9,
                    "selected Kp must reach the actual ST controller"
                );
            }
            assert!(angle.abs() <= 2.0 && velocity.abs() <= 10.0 && torque.abs() <= 20.0);
            peak = peak.max(angle);
            if scan >= 200 {
                late_error = late_error.max((1.0 - angle).abs());
            }
            simulation
                .apply_post_cycle(Duration::from_millis((scan + 1) * 20), &runtime)
                .unwrap();
        }
        assert!(peak <= 1.1, "Kp={gain}: peak {peak} rad exceeds 1.1 rad");
        assert!(
            late_error <= 0.02,
            "Kp={gain}: 4–8s error {late_error} rad exceeds 0.02 rad"
        );
        eprintln!("Kp={gain}: peak={peak:.8} rad, maximum4to8sError={late_error:.8} rad");
    }
}

#[test]
fn root_locus_shaft_applies_selected_gain_and_fixed_measured_velocity_braking() {
    let session = CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", MAIN)]);
    let mut runtime = compile(session);
    let debug = runtime.enable_debug();
    for (angle, velocity, expected) in
        [(0.0_f64, 0.0_f64, 16.0), (0.5, 0.5, 4.5), (-1.0, 0.0, 20.0)]
    {
        for (address, value) in [
            ("%MX96.0", Value::Bool(true)),
            ("%IL0", Value::LWord(angle.to_bits())),
            ("%IL8", Value::LWord(velocity.to_bits())),
        ] {
            debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value);
        }
        runtime.advance_time(Duration::from_millis(20));
        runtime.execute_cycle().expect("execute shaft control scan");
        assert!(
            (read(&runtime, "%QL0") - expected).abs() < 1e-9,
            "shaft torque must be clamp(16 * (1 - angle) - 7 * velocity, -20, 20); got {}",
            read(&runtime, "%QL0")
        );
    }
    debug.enqueue_io_write(IoAddress::parse("%MX96.0").unwrap(), Value::Bool(false));
    runtime.advance_time(Duration::from_millis(20));
    runtime.execute_cycle().unwrap();
    assert_eq!(
        read(&runtime, "%QL0"),
        0.0,
        "disabled experiment commands zero torque"
    );
}
