//! Release checks execute the delivered Main programs, never copies of their logic.
use std::path::PathBuf;
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::io::IoAddress;
use trust_runtime::value::{Duration, Value};
use trust_runtime::Runtime;

fn application(relative: &str, baseline_variable: &str) -> Runtime {
    let source_path = std::env::var_os(baseline_variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../examples/control")
                .join(relative)
                .join("src/Main.st")
        });
    let source = std::fs::read_to_string(&source_path).expect("read the actual lesson Main source");
    CompileSession::from_sources(vec![
        SourceFile::with_path(
            "Control/control.st",
            include_str!("../../../libraries/control/src/control.st"),
        ),
        SourceFile::with_path("src/Main.st", source),
    ])
    .build_runtime()
    .expect("compile the actual lesson and Control.PID")
}
fn value(runtime: &Runtime, member: &str) -> Value {
    let Value::Instance(program) = runtime.storage().get_global("P1").unwrap() else {
        panic!("P1 instance");
    };
    runtime
        .storage()
        .get_instance_var(*program, member)
        .unwrap()
        .clone()
}
fn library_state(runtime: &Runtime) -> Value {
    let Value::Instance(controller) = value(runtime, "Controller") else {
        panic!("actual Control.PID instance");
    };
    runtime
        .storage()
        .get_instance_var(controller, "StateValue")
        .unwrap()
        .clone()
}
fn cycle(runtime: &mut Runtime, booleans: &[(&str, bool)], numbers: &[(&str, f64)]) {
    let debug = runtime.enable_debug();
    for (address, value) in booleans {
        debug.enqueue_io_write(IoAddress::parse(address).unwrap(), Value::Bool(*value));
    }
    for (address, value) in numbers {
        debug.enqueue_io_write(
            IoAddress::parse(address).unwrap(),
            Value::LWord(value.to_bits()),
        );
    }
    runtime.advance_time(Duration::from_millis(100));
    runtime
        .execute_cycle()
        .expect("an invalid candidate must be rejected without faulting the running PID");
}

#[test]
fn parameter_lesson_rejects_invalid_candidates_in_the_running_main() {
    let project = "03-production-quality-digital-pid/41-atomic-parameter-update";
    // Nonfinite process-image writes are rejected by the runtime before ST.
    // These finite inputs reach Main and exercise its own cross-field rejection.
    for (address, invalid) in [("%ML32", 0.0), ("%ML32", -0.5), ("%ML48", -20.0)] {
        let mut runtime = application(project, "TRUST_PARAMETER_LESSON_BASELINE_MAIN");
        cycle(
            &mut runtime,
            &[("%MX120.0", true)],
            &[
                ("%IL0", 6.0),
                ("%ML0", 6.0),
                ("%ML8", 40.0),
                ("%ML48", 20.0),
            ],
        );
        let active = value(&runtime, "ActiveParameters");
        let revision = value(&runtime, "ActiveRevision");
        let states = library_state(&runtime);
        cycle(
            &mut runtime,
            &[("%MX120.0", false), ("%MX120.1", true)],
            &[
                ("%IL0", 6.0),
                ("%ML16", 15.0),
                ("%ML24", 3.0),
                ("%ML32", 0.5),
                (address, invalid),
            ],
        );
        assert_eq!(
            value(&runtime, "ActiveRevision"),
            revision,
            "invalid candidate must not publish a revision"
        );
        assert_eq!(
            value(&runtime, "ActiveParameters"),
            active,
            "invalid candidate must preserve every active parameter"
        );
        assert_eq!(
            library_state(&runtime),
            states,
            "steady operation must preserve all seven states after rejection"
        );
        cycle(
            &mut runtime,
            &[("%MX120.1", true)],
            &[
                ("%IL0", 6.0),
                ("%ML16", 15.0),
                ("%ML24", 3.0),
                ("%ML32", 0.5),
                ("%ML48", 20.0),
            ],
        );
        assert_eq!(value(&runtime, "ActiveRevision"), Value::Int(1));
        assert_eq!(
            value(&runtime, "ActiveProportionalGain"),
            Value::LReal(15.0)
        );
        assert_eq!(value(&runtime, "ActiveIntegralGain"), Value::LReal(3.0));
        assert_eq!(value(&runtime, "ActiveTrackingTime"), Value::LReal(0.5));
    }
}

#[test]
fn ratio_lesson_main_uses_the_taught_one_to_ten_recipe() {
    let mut runtime = application(
        "09-industrial-control-structures/53-ratio-control",
        "TRUST_RATIO_LESSON_BASELINE_MAIN",
    );
    for _ in 0..305 {
        cycle(
            &mut runtime,
            &[("%MX160.1", true), ("%MX88.2", true), ("%MX160.3", true)],
            &[("%IL48", 0.1), ("%IL56", 0.1)],
        );
    }
    assert_eq!(
        value(&runtime, "MeasuredWildFlow"),
        Value::LReal(120.0),
        "execute the application's actual carrier-flow change"
    );
    assert_eq!(
        value(&runtime, "RatioSetpoint"),
        Value::LReal(0.1),
        "the delivered application must use the taught recipe"
    );
    assert_eq!(
        value(&runtime, "RatioDemand"),
        Value::LReal(12.0),
        "actual Main demand must be 12 L/min for 120 L/min carrier"
    );
    cycle(
        &mut runtime,
        &[("%MX160.2", true)],
        &[("%IL48", 0.1), ("%IL56", 0.1)],
    );
    assert_eq!(value(&runtime, "RatioAuthority"), Value::Bool(false));
    assert_eq!(
        value(&runtime, "RatioDemand"),
        Value::LReal(0.0),
        "abort must remove the ratio demand"
    );
}
