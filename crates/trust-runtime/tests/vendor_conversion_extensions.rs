use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

// docs/specs/07-standard-functions.md 2.6: the vendor-extension conversions compile and run
// in the bytecode VM.
#[test]
fn vendor_extension_conversions_run_in_programs() {
    let source = r#"
PROGRAM Main
VAR
    speed : INT := 4;
    level : REAL := 127.6;
    status : BYTE := 16#80;
    out_byte : BYTE;
    out_word : WORD;
    running : BOOL;
    flagged : BOOL;
    idle : BOOL;
    gain : REAL;
END_VAR
out_byte := REAL_TO_BYTE(INT_TO_REAL(speed) / 10.0 * 255.0);
out_word := REAL_TO_WORD(level);
running := INT_TO_BOOL(speed);
flagged := BYTE_TO_BOOL(status AND BYTE#16#80);
idle := REAL_TO_BOOL(level - 127.6);
gain := BOOL_TO_REAL(running) * 2.5;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("out_byte"), Some(Value::Byte(102)));
    assert_eq!(harness.get_output("out_word"), Some(Value::Word(128)));
    assert_eq!(harness.get_output("running"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("flagged"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("idle"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("gain"), Some(Value::Real(2.5)));
}
