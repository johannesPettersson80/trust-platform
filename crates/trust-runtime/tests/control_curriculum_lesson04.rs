//! Teaching lesson5 uses zero-bias P on the heater. The folder keeps legacy04.
//! Authority: control-course-content/01-11-foundations-and-pi.md, lesson5.
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};

const MAIN: &str = include_str!(
    "../../../examples/control/01-feedback-foundations-and-temperature-control/04-water-heater-p/src/Main.st"
);

fn applied(temperature: f64, enabled: bool, abort: bool) -> f64 {
    let session = CompileSession::from_sources(vec![
        SourceFile::with_path("src/Main.st", MAIN),
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
    ]);
    let mut runtime = session.build_runtime().unwrap();
    runtime
        .apply_bytecode_bytes(&session.build_bytecode_bytes().unwrap(), None)
        .unwrap();
    let debug = runtime.enable_debug();
    for (address, value) in [
        ("%IL0", Value::LWord(temperature.to_bits())),
        ("%IL8", Value::LWord(0.0_f64.to_bits())),
        ("%MX24.1", Value::Bool(enabled)),
        ("%MX24.2", Value::Bool(abort)),
    ] {
        debug.enqueue_io_write(IoAddress::parse(address).unwrap(), value);
    }
    runtime.advance_time(Duration::from_millis(100));
    runtime.execute_cycle().unwrap();
    match runtime
        .io()
        .read(&IoAddress::parse("%QL0").unwrap())
        .unwrap()
    {
        Value::LWord(bits) => f64::from_bits(bits),
        other => panic!("expected LREAL output, got {other:?}"),
    }
}

#[test]
fn heater_p_zero_error_means_zero_heat_and_cold_error_requests_thirty_percent() {
    assert_eq!(applied(20.0, true, false), 30.0);
    assert_eq!(applied(35.0, true, false), 0.0);
    assert_eq!(
        applied(40.0, true, false),
        0.0,
        "one-sided heater cannot cool"
    );
}

#[test]
fn heater_p_disabled_aborted_or_out_of_range_measurement_removes_heat() {
    assert_eq!(applied(20.0, false, false), 0.0);
    assert_eq!(applied(20.0, true, true), 0.0);
    assert_eq!(applied(81.0, true, false), 0.0);
}
