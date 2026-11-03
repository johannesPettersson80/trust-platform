//! Function-block invocation argument association in a compiled program (specification 04,
//! "Function Block Call (Table 42)"; specification 44 §6.4): under the CODESYS profile a
//! function block invocation binds positionally to its input and in-out formals in declaration
//! order and its output formals bind only by name with `=>`, as the analyzer binds them
//! (docs/IEC_DEVIATIONS.md DEV-044). The values are worked out by hand.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;

/// The program of `session` run for `cycles` cycles, each without a runtime error.
fn run(session: CompileSession, cycles: usize) -> TestHarness {
    let mut harness = TestHarness::from_session(session).expect("the program builds");
    for _ in 0..cycles {
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }
    harness
}

const REGULATOR: &str = r#"
FUNCTION_BLOCK Regulator
VAR_INPUT
    setpoint : DINT;
    measure : DINT;
END_VAR
VAR_OUTPUT
    diff : DINT;
END_VAR
    diff := setpoint - measure;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    regulator : Regulator;
    first : DINT;
    second : DINT;
END_VAR
regulator(10, 4);
first := regulator.diff;
regulator(7, 2, diff => second);
END_PROGRAM
"#;

// DEV-044: under the CODESYS profile the positional call `regulator(10, 4)` binds its two
// inputs and leaves the output formal outside the positional association — the body still
// computes it, and the second call's named `diff => second` copies it. Worked by hand:
// 10 - 4 = 6, 7 - 2 = 5. Catches the generator counting the output formal positionally (the
// build would refuse the call) and an output copied into the wrong variable.
#[test]
fn the_codesys_profile_binds_a_function_blocks_positional_inputs_and_named_output() {
    let harness = run(
        CompileSession::from_source(REGULATOR).with_conversion_profile(ConversionProfile::Codesys),
        1,
    );
    assert_eq!(harness.get_output("Main.first"), Some(Value::DInt(6)));
    assert_eq!(harness.get_output("Main.second"), Some(Value::DInt(5)));
}

// DEV-044: the IEC rules keep the output formal inside positional association (IEC 61131-3
// Ed.3 §6.6.3.4, Table 42), so the same source's positional call lacks the output's actual and
// the build refuses it. Catches the CODESYS rule applied to an IEC project.
#[test]
fn the_iec_rules_refuse_a_positional_function_block_call_that_skips_the_output() {
    let Err(error) = TestHarness::from_session(CompileSession::from_source(REGULATOR)) else {
        panic!("the IEC rules refuse `regulator(10, 4)`");
    };
    let text = error.to_string();
    assert!(text.contains("expected 3 arguments"), "{text}");
}
