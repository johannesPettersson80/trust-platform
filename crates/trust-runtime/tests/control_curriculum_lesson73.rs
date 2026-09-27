use trust_runtime::debug::DebugControl;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const MAIN: &str = include_str!(
    "../../../examples/control/11-state-space-estimation-and-predictive-control/73-mpc/src/Main.st"
);
const TYPES: &str = include_str!(
    "../../../examples/control/11-state-space-estimation-and-predictive-control/73-mpc/src/MpcTypes.st"
);
const MPC: &str = include_str!(
    "../../../examples/control/11-state-space-estimation-and-predictive-control/73-mpc/src/FiniteGridMpcController.st"
);

const INITIAL_HEAT: f64 = 80.0;
const INDIVIDUAL_HEAT_LIMIT: f64 = 100.0;
const MOVE_LIMIT: f64 = 20.0;
const SHARED_HEAT_LIMIT: f64 = 170.0;

fn build_lesson_runtime() -> Runtime {
    let session = CompileSession::from_sources(vec![
        SourceFile::with_path("src/Main.st", MAIN),
        SourceFile::with_path("src/MpcTypes.st", TYPES),
        SourceFile::with_path("src/FiniteGridMpcController.st", MPC),
    ]);
    let mut runtime = session.build_runtime().expect("build Lesson 73 runtime");
    let bytes = session
        .build_bytecode_bytes()
        .expect("build Lesson 73 bytecode");
    runtime
        .apply_bytecode_bytes(&bytes, None)
        .expect("install Lesson 73 bytecode");
    runtime
}

fn queue_initial_scan(
    debug: &DebugControl,
    zone_1_temperature: f64,
    zone_2_temperature: f64,
    safety_abort: bool,
) {
    for (address, value) in [
        ("%MX241.0", Value::Bool(true)),
        ("%MX241.1", Value::Bool(safety_abort)),
        ("%ML88", Value::LWord(INITIAL_HEAT.to_bits())),
        ("%ML96", Value::LWord(INITIAL_HEAT.to_bits())),
    ] {
        debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value);
    }
    queue_external_scan(debug, zone_1_temperature, zone_2_temperature, true);
}

fn queue_external_scan(
    debug: &DebugControl,
    zone_1_temperature: f64,
    zone_2_temperature: f64,
    inputs_valid: bool,
) {
    for (address, value) in [
        ("%MX241.2", Value::Bool(inputs_valid)),
        ("%IL0", Value::LWord(zone_1_temperature.to_bits())),
        ("%IL8", Value::LWord(zone_2_temperature.to_bits())),
    ] {
        debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value);
    }
}

fn read_value(runtime: &Runtime, address: &str) -> Value {
    runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap_or_else(|_| panic!("missing value at {address}"))
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match read_value(runtime, address) {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LWORD at {address}, got {other:?}"),
    }
}

fn read_dint(runtime: &Runtime, address: &str) -> i32 {
    match read_value(runtime, address) {
        Value::DWord(value) => value as i32,
        other => panic!("expected DWORD at {address}, got {other:?}"),
    }
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match read_value(runtime, address) {
        Value::Bool(value) => value,
        other => panic!("expected BOOL at {address}, got {other:?}"),
    }
}

fn assert_hard_heat_limits(zone_1_heat: f64, zone_2_heat: f64) {
    assert!((0.0..=INDIVIDUAL_HEAT_LIMIT).contains(&zone_1_heat));
    assert!((0.0..=INDIVIDUAL_HEAT_LIMIT).contains(&zone_2_heat));
    assert!((zone_1_heat - INITIAL_HEAT).abs() <= MOVE_LIMIT);
    assert!((zone_2_heat - INITIAL_HEAT).abs() <= MOVE_LIMIT);
    assert!(zone_1_heat + zone_2_heat <= SHARED_HEAT_LIMIT);
}

fn assert_experiment_configuration(runtime: &Runtime) {
    assert_eq!(read_lreal(runtime, "%ML16"), 21.0);
    assert_eq!(read_lreal(runtime, "%ML24"), 21.0);
    assert_eq!(read_lreal(runtime, "%ML32"), 20.0);
    assert_eq!(read_lreal(runtime, "%ML40"), 22.5);
    assert_eq!(read_lreal(runtime, "%ML48"), 100.0);
    assert_eq!(read_lreal(runtime, "%ML56"), 0.01);
    assert_eq!(read_lreal(runtime, "%ML64"), 0.0025);
}

#[test]
fn lesson_73_main_applies_replans_and_clears_a_stale_mpc_result() {
    let mut runtime = build_lesson_runtime();
    let debug = runtime.enable_debug();
    queue_initial_scan(&debug, 21.0, 21.0, false);

    runtime.advance_time(Duration::from_secs(60));
    runtime.execute_cycle().expect("run feasible MPC scan");

    assert!(read_bool(&runtime, "%MX240.2"), "configuration valid");
    assert!(read_bool(&runtime, "%MX240.3"), "solver complete");
    assert!(read_bool(&runtime, "%MX240.4"), "solver feasible");
    assert!(!read_bool(&runtime, "%MX240.5"), "not infeasible");
    assert!(!read_bool(&runtime, "%MX240.6"), "fallback inactive");
    assert_eq!(read_dint(&runtime, "%MD0"), 729);
    assert!(read_dint(&runtime, "%MD4") > 0);
    assert_experiment_configuration(&runtime);

    let requested_zone_1_heat = read_lreal(&runtime, "%ML72");
    let requested_zone_2_heat = read_lreal(&runtime, "%ML80");
    let applied_zone_1_heat = read_lreal(&runtime, "%QL0");
    let applied_zone_2_heat = read_lreal(&runtime, "%QL8");
    assert_eq!(requested_zone_1_heat, INITIAL_HEAT);
    assert_eq!(requested_zone_2_heat, INITIAL_HEAT);
    assert_eq!(applied_zone_1_heat, requested_zone_1_heat);
    assert_eq!(applied_zone_2_heat, requested_zone_2_heat);
    assert_eq!(read_lreal(&runtime, "%ML88"), applied_zone_1_heat);
    assert_eq!(read_lreal(&runtime, "%ML96"), applied_zone_2_heat);
    assert_hard_heat_limits(applied_zone_1_heat, applied_zone_2_heat);

    queue_external_scan(&debug, 22.5, 22.5, true);
    runtime.advance_time(Duration::from_secs(60));
    runtime.execute_cycle().expect("run replanned MPC scan");

    assert!(read_bool(&runtime, "%MX240.2"), "configuration valid");
    assert!(read_bool(&runtime, "%MX240.3"), "solver complete");
    assert!(read_bool(&runtime, "%MX240.4"), "solver feasible");
    assert_eq!(read_dint(&runtime, "%MD0"), 729);
    assert!(read_dint(&runtime, "%MD4") > 0);
    let replanned_zone_1_heat = read_lreal(&runtime, "%QL0");
    let replanned_zone_2_heat = read_lreal(&runtime, "%QL8");
    assert_eq!(replanned_zone_1_heat, 60.0);
    assert_eq!(replanned_zone_2_heat, 60.0);
    assert_ne!(replanned_zone_1_heat, applied_zone_1_heat);
    assert_ne!(replanned_zone_2_heat, applied_zone_2_heat);
    assert_eq!(read_lreal(&runtime, "%ML72"), replanned_zone_1_heat);
    assert_eq!(read_lreal(&runtime, "%ML80"), replanned_zone_2_heat);
    assert_hard_heat_limits(replanned_zone_1_heat, replanned_zone_2_heat);

    queue_external_scan(&debug, 22.5, 22.5, false);
    runtime.advance_time(Duration::from_secs(60));
    runtime.execute_cycle().expect("run invalid-input scan");

    assert!(!read_bool(&runtime, "%MX240.0"), "control disabled");
    assert!(!read_bool(&runtime, "%MX240.2"), "configuration invalid");
    assert!(!read_bool(&runtime, "%MX240.3"), "solver result cleared");
    assert!(!read_bool(&runtime, "%MX240.4"), "feasible result cleared");
    assert!(
        !read_bool(&runtime, "%MX240.5"),
        "infeasible result cleared"
    );
    assert_eq!(read_dint(&runtime, "%MD0"), 0);
    assert_eq!(read_dint(&runtime, "%MD4"), 0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);
}

#[test]
fn lesson_73_main_uses_bounded_pi_fallback_for_a_valid_but_infeasible_request() {
    let mut runtime = build_lesson_runtime();
    let debug = runtime.enable_debug();
    queue_initial_scan(&debug, 15.0, 15.0, false);

    runtime.advance_time(Duration::from_secs(60));
    runtime.execute_cycle().expect("run infeasible MPC scan");

    assert!(read_bool(&runtime, "%MX240.2"), "configuration valid");
    assert!(read_bool(&runtime, "%MX240.3"), "solver complete");
    assert!(!read_bool(&runtime, "%MX240.4"), "solver infeasible");
    assert!(read_bool(&runtime, "%MX240.5"), "infeasible reported");
    assert!(read_bool(&runtime, "%MX240.6"), "PI fallback active");
    assert_eq!(read_dint(&runtime, "%MD0"), 729);
    assert_eq!(read_dint(&runtime, "%MD4"), 0);
    assert_experiment_configuration(&runtime);

    let requested_zone_1_heat = read_lreal(&runtime, "%ML72");
    let requested_zone_2_heat = read_lreal(&runtime, "%ML80");
    let applied_zone_1_heat = read_lreal(&runtime, "%QL0");
    let applied_zone_2_heat = read_lreal(&runtime, "%QL8");
    assert_eq!(requested_zone_1_heat, 85.0);
    assert_eq!(requested_zone_2_heat, 85.0);
    assert_eq!(applied_zone_1_heat, requested_zone_1_heat);
    assert_eq!(applied_zone_2_heat, requested_zone_2_heat);
    assert_eq!(applied_zone_1_heat + applied_zone_2_heat, SHARED_HEAT_LIMIT);
    assert_hard_heat_limits(applied_zone_1_heat, applied_zone_2_heat);
}

#[test]
fn lesson_73_main_safety_abort_applies_the_zero_safe_output() {
    let mut runtime = build_lesson_runtime();
    let debug = runtime.enable_debug();
    queue_initial_scan(&debug, 21.0, 21.0, true);

    runtime.advance_time(Duration::from_secs(60));
    runtime.execute_cycle().expect("run safety-abort scan");

    assert!(!read_bool(&runtime, "%MX240.0"), "control disabled");
    assert!(!read_bool(&runtime, "%MX240.2"), "configuration invalid");
    assert!(!read_bool(&runtime, "%MX240.3"), "solver not run");
    assert_eq!(read_dint(&runtime, "%MD0"), 0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);
}
