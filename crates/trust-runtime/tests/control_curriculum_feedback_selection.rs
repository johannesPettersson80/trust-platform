use trust_runtime::debug::DebugControl;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const LESSON_68_MAIN: &str = include_str!(
    "../../../examples/control/11-state-space-estimation-and-predictive-control/68-state-feedback/src/Main.st"
);
const LESSON_70_MAIN: &str = include_str!(
    "../../../examples/control/11-state-space-estimation-and-predictive-control/70-state-observer/src/Main.st"
);

fn read_value(runtime: &Runtime, address: &str) -> Value {
    runtime
        .io()
        .read(&IoAddress::parse(address).expect("I/O address"))
        .expect("read mapped value")
}

fn read_bool(runtime: &Runtime, address: &str) -> bool {
    match read_value(runtime, address) {
        Value::Bool(value) => value,
        other => panic!("expected BOOL at {address}, got {other:?}"),
    }
}

fn read_int(runtime: &Runtime, address: &str) -> i16 {
    match read_value(runtime, address) {
        Value::Word(value) => value as i16,
        other => panic!("expected INT at {address}, got {other:?}"),
    }
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match read_value(runtime, address) {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL at {address}, got {other:?}"),
    }
}

fn queue_inputs(
    debug: &DebugControl,
    architecture: i16,
    observer_enabled: bool,
    experiment_enabled: bool,
    safety_abort: bool,
) {
    for (address, value) in [
        ("%MW0", Value::Word(architecture as u16)),
        ("%MX120.2", Value::Bool(observer_enabled)),
        ("%MX120.3", Value::Bool(experiment_enabled)),
        ("%MX120.4", Value::Bool(safety_abort)),
        ("%MX120.5", Value::Bool(true)),
        ("%IL0", Value::LWord(0.01_f64.to_bits())),
        ("%IL8", Value::LWord(0.02_f64.to_bits())),
        ("%IL16", Value::LWord(0.25_f64.to_bits())),
    ] {
        debug.enqueue_io_write(IoAddress::parse(address).expect("input address"), value);
    }
}

fn build_runtime(source: &'static str) -> Runtime {
    let session = CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", source)]);
    let mut runtime = session
        .build_runtime()
        .expect("build state-feedback/observer lesson");
    let bytecode = session
        .build_bytecode_bytes()
        .expect("build lesson bytecode");
    runtime
        .apply_bytecode_bytes(&bytecode, None)
        .expect("install lesson bytecode");
    runtime
}

fn run_selection_case(source: &'static str, architecture: i16, observer_enabled: bool) {
    let mut runtime = build_runtime(source);
    let debug = runtime.enable_debug();

    queue_inputs(&debug, architecture, observer_enabled, false, false);
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("run disabled scan");
    assert_eq!(read_int(&runtime, "%MW0"), architecture);
    assert_eq!(read_bool(&runtime, "%MX120.2"), observer_enabled);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL16"), 0.0);

    queue_inputs(&debug, architecture, observer_enabled, true, false);
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("run enabled scan");
    assert_eq!(read_int(&runtime, "%MW0"), architecture);
    assert_eq!(read_bool(&runtime, "%MX120.2"), observer_enabled);
    let expected_bearing = if architecture == 0 { -0.05 } else { -0.30 };
    let expected_speed = if observer_enabled { 0.20 } else { 0.25 };
    let expected_load = if observer_enabled { -0.50 } else { 0.0 };
    let expected_torque = if observer_enabled { 2.10 } else { 2.50 };
    assert!((read_lreal(&runtime, "%ML24") - expected_bearing).abs() <= 1.0e-12);
    assert!((read_lreal(&runtime, "%ML40") - expected_speed).abs() <= 1.0e-12);
    assert!((read_lreal(&runtime, "%ML48") - expected_load).abs() <= 1.0e-12);
    assert!((read_lreal(&runtime, "%ML32") - expected_torque).abs() <= 1.0e-12);
    assert!((read_lreal(&runtime, "%QL0") - expected_bearing).abs() <= 1.0e-12);
    assert!((read_lreal(&runtime, "%QL16") - expected_torque).abs() <= 1.0e-12);

    queue_inputs(&debug, architecture, observer_enabled, true, true);
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("run abort scan");
    assert_eq!(read_int(&runtime, "%MW0"), architecture);
    assert_eq!(read_bool(&runtime, "%MX120.2"), observer_enabled);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL16"), 0.0);
}

#[test]
fn lesson_68_and_70_retain_feedback_selections_and_fail_safe_on_abort() {
    for source in [LESSON_68_MAIN, LESSON_70_MAIN] {
        for architecture in [0, 1] {
            for observer_enabled in [false, true] {
                run_selection_case(source, architecture, observer_enabled);
            }
        }
    }
}
