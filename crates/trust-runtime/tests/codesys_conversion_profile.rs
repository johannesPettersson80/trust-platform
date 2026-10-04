use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;

// docs/specs/07-standard-functions.md 2.7: a compile session with the CODESYS conversion
// profile uses it in the bytecode VM, and a reload keeps it.
const SOURCE: &str = r#"
PROGRAM Main
VAR
    half : REAL := 2.5;
    level : DINT := 200;
    raw : DWORD := 16#0000_0005;
    rounded : INT;
    narrowed : SINT;
    scaled : REAL;
    packed : DWORD;
END_VAR
rounded := REAL_TO_INT(half);
narrowed := DINT_TO_SINT(level);
scaled := DWORD_TO_REAL(raw) * 2.0;
packed := REAL_TO_DWORD(half);
END_PROGRAM
"#;

#[test]
fn codesys_conversion_profile_applies_to_program_code_and_survives_reload() {
    let session =
        CompileSession::from_source(SOURCE).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).unwrap();
    harness.cycle();

    assert_eq!(harness.get_output("rounded"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("narrowed"), Some(Value::SInt(-56)));
    assert_eq!(harness.get_output("scaled"), Some(Value::Real(10.0)));
    assert_eq!(harness.get_output("packed"), Some(Value::DWord(3)));

    harness.reload_sources(&[SOURCE]).unwrap();
    harness.cycle();
    assert_eq!(harness.get_output("narrowed"), Some(Value::SInt(-56)));
}
