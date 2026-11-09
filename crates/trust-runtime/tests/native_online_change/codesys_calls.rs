//! CODESYS's calls at an online change seen from the kernel (specification 44 §12.3; specification
//! 10 §4.8): a fault of one of them is reported for the new program to record, and the change goes
//! on. In a module of its own beside the carry's tests.

use super::*;
use trust_runtime::native::build_with_conversions;
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime_core::error::RuntimeError;

/// The program of `source` built under the CODESYS profile for this computer, as [`program`] builds
/// one under the IEC profile.
fn codesys_program(source: &str, name: &str) -> NativeProgram<'static, HostCode> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!("trust-native-{name}-{}", std::process::id()));
    let built = build_with_conversions(
        &[SourceFile::new(source)],
        &[],
        &Options {
            target: host,
            hooks: true,
        },
        ConversionProfile::Codesys,
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 10_000,
        trusted_keys: &[],
    };
    let image: &'static [u8] = Box::leak(built.image.into_boxed_slice());
    load(image, &device).expect("the image loads")
}

/// The running program: `FB_Probe` with one member.
const PROBE_RUNNING: &str = "
FUNCTION_BLOCK FB_Probe
VAR
    count : INT;
END_VAR
count := count + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    probe : FB_Probe;
    scans : INT;
END_VAR
probe();
scans := scans + 1;
END_PROGRAM
";

/// The changed program: `FB_Probe` gains `zero`, so the change copies `probe`, and its `FB_reinit`
/// divides by it.
const PROBE_CHANGED: &str = "
FUNCTION_BLOCK FB_Probe
VAR
    count : INT;
    zero : INT;
END_VAR
METHOD FB_reinit : BOOL
count := count / zero;
FB_reinit := TRUE;
END_METHOD
count := count + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    probe : FB_Probe;
    scans : INT;
END_VAR
probe();
scans := scans + 1;
END_PROGRAM
";

// Specification 10 §4.8 and specification 44 §12.3: a fault of CODESYS's calls at an online change
// is recorded as an event of the new program and the change goes on. Worked by hand: the running
// program scans once (`scans` 1); the change copies `probe`, whose type gained a member, and its
// FB_reinit divides by zero after the carry, so the change's report holds that one fault; the
// changed program runs on, and its first scan counts `scans` from the carried 1 to 2. Catches a
// change that the fault stops, a fault left unreported, and a change that calls FB_reinit of an
// instance it did not copy (a second fault is impossible here, as the program has one instance).
#[test]
fn a_fault_of_codesys_calls_at_an_online_change_is_reported_and_the_change_goes_on() {
    let mut running = codesys_program(PROBE_RUNNING, "probe-running");
    running.cold_start(Duration::ZERO).expect("the cold start");
    running
        .scan(Duration::ZERO, &mut trust_runtime::native::NoHooks)
        .expect("the scan runs");
    let mut changed = codesys_program(PROBE_CHANGED, "probe-changed");
    let report = changed
        .change_from(&mut running, Duration::from_millis(10))
        .expect("the change applies");
    assert_eq!(report.faults, [RuntimeError::DivisionByZero]);
    changed
        .scan(
            Duration::from_millis(10),
            &mut trust_runtime::native::NoHooks,
        )
        .expect("the changed program scans");
    assert_eq!(changed.value("MAIN.SCANS"), Some(Value::Int(2)));
}
