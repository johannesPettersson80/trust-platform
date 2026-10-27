use trust_runtime::harness::TestHarness;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

#[test]
fn string_functions() {
    let lib = StandardLibrary::new();

    assert_eq!(
        lib.call("LEN", &[Value::String("abc".into())]).unwrap(),
        Value::Int(3)
    );

    assert_eq!(
        lib.call(
            "CONCAT",
            &[Value::String("foo".into()), Value::String("bar".into())]
        )
        .unwrap(),
        Value::String("foobar".into())
    );
}

#[test]
fn bounded_string_assignment_and_concat_respect_declared_capacity() {
    let source = r#"
PROGRAM Main
VAR
    text : STRING[3] := 'A';
    source : STRING[6] := 'BCDE';
END_VAR
text := CONCAT(text, source);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "bounded string assignment must not fault the cycle: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("text"),
        Some(Value::String("ABC".into()))
    );
}

#[test]
fn concat_into_a_string_it_reads_stores_the_value_of_the_whole_expression() {
    // An assignment stores the value of its expression (IEC 61131-3 §7.3.2; specification 07 §8):
    // `padded := CONCAT('0', padded)` is '0' then the old '4'. Wrong behaviour caught: the
    // concatenation written into its own operand, which reads back the '0' it has just written and
    // gives '00' and '<<<>' instead of '04' and '<ab>'.
    let source = r#"
PROGRAM Main
VAR
    padded : STRING[8] := '4';
    wrapped : STRING[8] := 'ab';
    doubled : STRING[3] := 'xy';
    wide : WSTRING[8] := "4";
END_VAR
padded := CONCAT('0', padded);
wrapped := CONCAT('<', wrapped, '>');
doubled := CONCAT(doubled, doubled);
wide := CONCAT("0", wide);
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).expect("compile runtime");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(
        harness.get_output("padded"),
        Some(Value::String("04".into()))
    );
    assert_eq!(
        harness.get_output("wrapped"),
        Some(Value::String("<ab>".into()))
    );
    // 'xyxy' cut at the capacity of 3 (specification 02 §6).
    assert_eq!(
        harness.get_output("doubled"),
        Some(Value::String("xyx".into()))
    );
    assert_eq!(
        harness.get_output("wide"),
        Some(Value::WString("04".into()))
    );
}
