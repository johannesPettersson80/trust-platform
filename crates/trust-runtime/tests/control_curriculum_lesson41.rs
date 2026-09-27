use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

const LESSON_MAIN: &str = include_str!(
    "../../../examples/control/03-production-quality-digital-pid/41-atomic-parameter-update/src/Main.st"
);
const CONTROL_LIBRARY: &str = include_str!("../../../libraries/control/src/control.st");

fn read_lreal(runtime: &Runtime, address: &str) -> f64 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LWORD at {address}, got {other:?}"),
    }
}

fn read_int(runtime: &Runtime, address: &str) -> i16 {
    match runtime
        .io()
        .read(&IoAddress::parse(address).unwrap())
        .unwrap()
    {
        Value::Word(value) => value as i16,
        other => panic!("expected WORD at {address}, got {other:?}"),
    }
}

fn queue_scan(
    debug: &trust_runtime::debug::DebugControl,
    experiment_enabled: bool,
    initialize: bool,
    apply: bool,
    unsafe_update: bool,
) {
    for (address, value) in [
        ("%MX121.0", Value::Bool(experiment_enabled)),
        ("%MX120.0", Value::Bool(initialize)),
        ("%MX120.1", Value::Bool(apply)),
        ("%MX120.2", Value::Bool(unsafe_update)),
        ("%IL0", Value::LWord(6.0_f64.to_bits())),
        ("%ML0", Value::LWord(6.0_f64.to_bits())),
        ("%ML8", Value::LWord(40.0_f64.to_bits())),
        ("%ML16", Value::LWord(15.0_f64.to_bits())),
        ("%ML24", Value::LWord(3.0_f64.to_bits())),
        ("%ML32", Value::LWord(0.5_f64.to_bits())),
        ("%ML48", Value::LWord(1000.0_f64.to_bits())),
    ] {
        debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value);
    }
}

#[test]
fn lesson_41_main_applies_one_complete_parameter_revision_per_scan() {
    let sources = vec![
        SourceFile::with_path("src/Main.st", LESSON_MAIN),
        SourceFile::with_path("Control/control.st", CONTROL_LIBRARY),
    ];
    let session = CompileSession::from_sources(sources).with_extra_program_instances(["P1"]);
    let mut runtime = session.build_runtime().expect("build Lesson 41 runtime");
    let bytecode = session
        .build_bytecode_module()
        .expect("build bytecode")
        .encode()
        .expect("encode bytecode");
    runtime
        .apply_bytecode_bytes(&bytecode, None)
        .expect("install bytecode");
    let debug = runtime.enable_debug();
    queue_scan(&debug, true, false, false, false);

    runtime.advance_time(Duration::from_millis(100));
    assert!(runtime.execute_cycle().is_ok());
    assert_eq!(read_int(&runtime, "%MW112"), 0);
    assert_eq!(read_lreal(&runtime, "%ML56"), 5.0);
    assert_eq!(read_lreal(&runtime, "%ML64"), 1.0);
    assert_eq!(read_lreal(&runtime, "%ML72"), 1.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 40.0);
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX120.4").unwrap())
            .unwrap(),
        Value::Bool(false)
    );

    queue_scan(&debug, true, false, false, false);
    runtime.advance_time(Duration::from_millis(100));
    assert!(runtime.execute_cycle().is_ok());
    assert_eq!(read_int(&runtime, "%MW112"), 1);
    assert_eq!(read_lreal(&runtime, "%ML56"), 15.0);
    assert_eq!(read_lreal(&runtime, "%ML64"), 3.0);
    assert_eq!(read_lreal(&runtime, "%ML72"), 0.5);
    assert_eq!(read_lreal(&runtime, "%QL0"), 55.3);
    assert_eq!(read_lreal(&runtime, "%ML16"), 15.0);
    assert_eq!(read_lreal(&runtime, "%ML24"), 3.0);
    assert_eq!(read_lreal(&runtime, "%ML32"), 0.5);
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX120.4").unwrap())
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX120.5").unwrap())
            .unwrap(),
        Value::Bool(false)
    );

    queue_scan(&debug, true, false, false, false);
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("run post-commit scan");
    assert_eq!(read_int(&runtime, "%MW112"), 1);
    assert_eq!(read_lreal(&runtime, "%ML56"), 15.0);
    assert_eq!(read_lreal(&runtime, "%ML64"), 3.0);
    assert_eq!(read_lreal(&runtime, "%ML72"), 0.5);
    assert!((read_lreal(&runtime, "%QL0") - 55.6).abs() < 1e-9);
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX120.4").unwrap())
            .unwrap(),
        Value::Bool(false)
    );
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX120.5").unwrap())
            .unwrap(),
        Value::Bool(false)
    );
}

#[test]
fn lesson_41_main_demonstrates_unsafe_sequential_update_mixing() {
    let sources = vec![
        SourceFile::with_path("src/Main.st", LESSON_MAIN),
        SourceFile::with_path("Control/control.st", CONTROL_LIBRARY),
    ];
    let session = CompileSession::from_sources(sources);
    let mut runtime = session.build_runtime().expect("build Lesson 41 runtime");
    let bytecode = session
        .build_bytecode_module()
        .expect("build bytecode")
        .encode()
        .expect("encode bytecode");
    runtime
        .apply_bytecode_bytes(&bytecode, None)
        .expect("install bytecode");
    let debug = runtime.enable_debug();

    queue_scan(&debug, false, true, false, false);
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("initialize manually");
    assert_eq!(read_int(&runtime, "%MW112"), 0);
    assert_eq!(read_lreal(&runtime, "%ML56"), 5.0);
    assert_eq!(read_lreal(&runtime, "%ML64"), 1.0);
    assert_eq!(read_lreal(&runtime, "%ML72"), 1.0);

    queue_scan(&debug, false, false, true, true);
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().expect("apply unsafe update");
    assert_eq!(read_int(&runtime, "%MW112"), 0);
    assert_eq!(read_lreal(&runtime, "%ML56"), 15.0);
    assert_eq!(read_lreal(&runtime, "%ML64"), 1.0);
    assert_eq!(read_lreal(&runtime, "%ML72"), 1.0);
    assert_eq!(read_lreal(&runtime, "%QL0"), 40.0);
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX120.4").unwrap())
            .unwrap(),
        Value::Bool(true)
    );
    assert_eq!(
        runtime
            .io()
            .read(&IoAddress::parse("%MX120.5").unwrap())
            .unwrap(),
        Value::Bool(true)
    );
}
