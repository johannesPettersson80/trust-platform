use trust_runtime::debug::DebugControl;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const LESSON_66_MAIN: &str = include_str!(
    "../../../examples/control/11-state-space-estimation-and-predictive-control/66-controllability/src/Main.st"
);
const LESSON_67_MAIN: &str = include_str!(
    "../../../examples/control/11-state-space-estimation-and-predictive-control/67-observability/src/Main.st"
);

fn read_int(runtime: &Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("INT address"))
        .expect("read INT")
    {
        Value::Word(value) => value as i16,
        other => panic!("expected WORD at {address}, got {other:?}"),
    }
}

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).expect("LREAL address"))
        .expect("read LREAL")
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LWORD at {address}, got {other:?}"),
    }
}

fn queue_layout(
    debug: &DebugControl,
    actuator_layout: i16,
    sensor_layout: i16,
    experiment_enabled: bool,
    safety_abort: bool,
) {
    for (address, value) in [
        ("%MW0", Value::Word(actuator_layout as u16)),
        ("%MW2", Value::Word(sensor_layout as u16)),
        ("%MX32.0", Value::Bool(experiment_enabled)),
        ("%MX32.1", Value::Bool(safety_abort)),
        ("%MX32.2", Value::Bool(true)),
        ("%IL0", Value::LWord(11.0_f64.to_bits())),
        ("%IL8", Value::LWord(22.0_f64.to_bits())),
    ] {
        debug.enqueue_io_write(IoAddress::parse(address).expect("layout address"), value);
    }
}

fn build_runtime(source: &'static str) -> Runtime {
    let session = CompileSession::from_sources(vec![SourceFile::with_path("src/Main.st", source)]);
    let mut runtime = session
        .build_runtime()
        .expect("build controllability/observability lesson");
    let bytecode = session
        .build_bytecode_bytes()
        .expect("build lesson bytecode");
    runtime
        .apply_bytecode_bytes(&bytecode, None)
        .expect("install lesson bytecode");
    runtime
}

fn run_layout_case(source: &'static str, actuator_layout: i16, sensor_layout: i16) {
    let mut runtime = build_runtime(source);
    let debug = runtime.enable_debug();

    queue_layout(&debug, actuator_layout, sensor_layout, false, false);
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("run disabled scan");
    assert_eq!(read_int(&runtime, "%MW0"), actuator_layout);
    assert_eq!(read_int(&runtime, "%MW2"), sensor_layout);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);

    queue_layout(&debug, actuator_layout, sensor_layout, true, false);
    for _ in 0..201 {
        runtime.advance_time(Duration::from_millis(100));
        runtime.execute_cycle().expect("run enabled scan");
        assert_eq!(read_int(&runtime, "%MW0"), actuator_layout);
        assert_eq!(read_int(&runtime, "%MW2"), sensor_layout);
    }
    assert_eq!(read_int(&runtime, "%MW0"), actuator_layout);
    assert_eq!(read_int(&runtime, "%MW2"), sensor_layout);
    if actuator_layout == 0 {
        assert_eq!(read_lreal(&runtime, "%QL0"), 1.0);
        assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);
    } else {
        assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
        assert_eq!(read_lreal(&runtime, "%QL8"), 1.0);
    }
    assert_eq!(
        read_lreal(&runtime, "%ML16"),
        if sensor_layout == 0 { 22.0 } else { 11.0 }
    );

    queue_layout(&debug, actuator_layout, sensor_layout, true, true);
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("run abort scan");
    assert_eq!(read_int(&runtime, "%MW0"), actuator_layout);
    assert_eq!(read_int(&runtime, "%MW2"), sensor_layout);
    assert_eq!(read_lreal(&runtime, "%QL0"), 0.0);
    assert_eq!(read_lreal(&runtime, "%QL8"), 0.0);
}

#[test]
fn lesson_66_and_67_retain_selected_layouts_and_fail_safe_on_abort() {
    for source in [LESSON_66_MAIN, LESSON_67_MAIN] {
        for actuator_layout in [0, 1] {
            for sensor_layout in [0, 1] {
                run_layout_case(source, actuator_layout, sensor_layout);
            }
        }
    }
}
