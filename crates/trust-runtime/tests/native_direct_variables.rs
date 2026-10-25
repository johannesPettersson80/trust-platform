//! Directly represented variables named in bodies (IEC 61131-3 §6.5.5.1: "permitted in the body of
//! functions, function blocks, programs, methods"; Table 16; specification 44 §5.6). Each is the
//! variable at its address: the scan samples an input at the input sample, a marker keeps its value
//! from scan to scan, and an output is published at the output sample. Every expected value is
//! worked out by hand; the 10 ms task first runs at 10 ms (specification 10 §4.3).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

const PROGRAM: &str = r#"
FUNCTION Ready : BOOL
Ready := %IX0.5;
END_FUNCTION

FUNCTION_BLOCK Lamp
VAR_INPUT
    Lit : BOOL;
END_VAR
%QX0.3 := Lit;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Light : Lamp;
END_VAR
%QX0.2 := %IX0.1;
%QW4 := INT_TO_WORD(WORD_TO_INT(%IW2) + 1);
%MW6 := INT_TO_WORD(WORD_TO_INT(%MW6) + 1);
%QX0.4 := %IX1.2.3;
%QX0.5 := Ready();
Light(Lit := NOT %IX0.1);
END_PROGRAM

CONFIGURATION Cell
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;

/// Runs one activation of the 10 ms task.
fn activate(harness: &mut TestHarness) {
    harness.advance_time(Duration::from_millis(10));
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

/// The value at the address `address`.
fn at(harness: &TestHarness, address: &str) -> Value {
    harness
        .get_direct_output(address)
        .unwrap_or_else(|error| panic!("{address}: {error}"))
}

/// A program, a function block and a function read and write addresses in their bodies. With
/// `%IX0.1` TRUE, `%IW2` 41, `%IX1.2.3` and `%IX0.5` TRUE, the first activation gives `%QX0.2` TRUE,
/// `%QW4` 42, `%MW6` 1, `%QX0.4` TRUE (a hierarchical input), `%QX0.5` TRUE (through the function)
/// and `%QX0.3` FALSE (the block's `NOT %IX0.1`); the second, with `%IX0.1` FALSE, `%MW6` 2 (the
/// marker kept from the scan before), `%QX0.2` FALSE and `%QX0.3` TRUE. Catches a direct variable
/// refused at build (as it was), an input not sampled, an output not published, a marker that
/// starts each scan at 0, and a bit written over its neighbours in the same byte.
#[test]
fn bodies_read_inputs_keep_markers_and_write_outputs_at_their_addresses() {
    let mut harness =
        TestHarness::from_source(PROGRAM).unwrap_or_else(|error| panic!("must compile: {error}"));
    for (address, value) in [
        ("%IX0.1", Value::Bool(true)),
        ("%IW2", Value::Word(41)),
        ("%IX1.2.3", Value::Bool(true)),
        ("%IX0.5", Value::Bool(true)),
    ] {
        harness
            .set_direct_input(address, value)
            .unwrap_or_else(|error| panic!("{address}: {error}"));
    }
    activate(&mut harness);
    assert_eq!(at(&harness, "%QX0.2"), Value::Bool(true));
    assert_eq!(at(&harness, "%QW4"), Value::Word(42));
    assert_eq!(at(&harness, "%MW6"), Value::Word(1));
    assert_eq!(at(&harness, "%QX0.4"), Value::Bool(true));
    assert_eq!(at(&harness, "%QX0.5"), Value::Bool(true));
    assert_eq!(at(&harness, "%QX0.3"), Value::Bool(false));

    harness
        .set_direct_input("%IX0.1", Value::Bool(false))
        .expect("an input address");
    activate(&mut harness);
    assert_eq!(at(&harness, "%MW6"), Value::Word(2));
    assert_eq!(at(&harness, "%QX0.2"), Value::Bool(false));
    assert_eq!(at(&harness, "%QX0.3"), Value::Bool(true));
    assert_eq!(at(&harness, "%QX0.4"), Value::Bool(true));
}
