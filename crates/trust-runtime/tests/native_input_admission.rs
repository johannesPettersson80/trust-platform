//! What a compiled program admits from its input image before the tasks run (specification 11,
//! "Floating-point boundary admission policy"; specification 44 §5.6): a bound `REAL` or `LREAL`
//! whose bits are no finite number fails the scan with an I/O error and is never sampled, and a
//! write past the program's input area never stops the sampling of the inputs it has.

use trust_runtime::harness::TestHarness;
use trust_runtime::io::IoAddress;
use trust_runtime::value::Value;
use trust_runtime::CompiledRuntime;

/// `Out := Level; Copy := Flow;` with `Level` a `REAL` at `%ID0`, `Flow` an `LREAL` at `%IL8`, and
/// their copies at `%QD0` and `%QL8`.
const PROGRAM: &str = "PROGRAM Main
VAR
    Level AT %ID0 : REAL;
    Flow AT %IL8 : LREAL;
    Out AT %QD0 : REAL;
    Copy AT %QL8 : LREAL;
END_VAR
    Out := Level;
    Copy := Flow;
END_PROGRAM
";

fn runtime() -> CompiledRuntime {
    TestHarness::from_source(PROGRAM)
        .expect("the program compiles")
        .into_runtime()
}

fn address(text: &str) -> IoAddress {
    IoAddress::parse(text).expect("an address")
}

fn real_out(runtime: &CompiledRuntime) -> f32 {
    match runtime.read_io(&address("%QD0")).expect("the output") {
        Value::DWord(bits) => f32::from_bits(bits),
        Value::Real(value) => value,
        other => panic!("a REAL output, not {other:?}"),
    }
}

/// Specification 11's admission table, typed `%I` snapshot row: NaN bits at the `REAL` input fail
/// the scan with `typed REAL process-image value must be finite`, and +infinity at the `LREAL` input
/// with `typed LREAL process-image value must be finite`; no value is substituted, so the program's
/// output keeps its last value (0, the scan never ran). Catches a non-finite value sampled into the
/// program, a substituted zero, and an error that names the wrong type.
#[test]
fn a_non_finite_real_or_lreal_input_fails_the_scan_before_the_program_sees_it() {
    for (at, value, name) in [
        ("%ID0", Value::DWord(f32::NAN.to_bits()), "REAL"),
        ("%IL8", Value::LWord(f64::INFINITY.to_bits()), "LREAL"),
    ] {
        let mut runtime = runtime();
        runtime
            .write_io(&address(at), value)
            .expect("the input write");
        let error = runtime
            .execute_cycle()
            .expect_err("a non-finite input fails the scan");
        assert!(
            error
                .to_string()
                .contains(&format!("typed {name} process-image value must be finite")),
            "{at}: {error}"
        );
        assert_eq!(real_out(&runtime), 0.0, "{at}: the program did not run");
    }
}

/// The same inputs with finite bits are sampled as they are: `Level` 2.5 gives `Out` 2.5. Catches
/// an admission check that refuses finite values.
#[test]
fn finite_real_and_lreal_inputs_are_sampled() {
    let mut runtime = runtime();
    runtime
        .write_io(&address("%ID0"), Value::DWord(2.5_f32.to_bits()))
        .expect("the input write");
    runtime
        .write_io(&address("%IL8"), Value::LWord((-1.25_f64).to_bits()))
        .expect("the input write");
    runtime.execute_cycle().expect("the scan");
    assert_eq!(real_out(&runtime), 2.5);
}

/// A write at `%IL64`, past the program's 16-byte input area, does not stop the sampling: `Level`
/// 2.5 at `%ID0` still reaches `Out` in the same and the next scan, whether the write past the area
/// is refused or not. Catches an input image grown past the area that the sample can no longer copy
/// (the defect the control course's lesson 4 test found, 26 October 2026).
#[test]
fn a_write_past_the_input_area_does_not_stop_the_sampling() {
    let mut runtime = runtime();
    let _ = runtime.write_io(&address("%IL64"), Value::LWord(0.0_f64.to_bits()));
    runtime
        .write_io(&address("%ID0"), Value::DWord(2.5_f32.to_bits()))
        .expect("the input write");
    runtime.execute_cycle().expect("the scan");
    assert_eq!(real_out(&runtime), 2.5);
    runtime
        .write_io(&address("%ID0"), Value::DWord(4.0_f32.to_bits()))
        .expect("the input write");
    runtime.execute_cycle().expect("the next scan");
    assert_eq!(real_out(&runtime), 4.0);
}
