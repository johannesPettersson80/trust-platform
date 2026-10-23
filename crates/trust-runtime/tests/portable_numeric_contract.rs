use trust_runtime::error::RuntimeError;
use trust_runtime::stdlib::StandardLibrary;
use trust_runtime::value::Value;

// Finite A1 corpus. These references and tolerances are frozen in spec 34;
// the old std implementations are retained here only for compatibility evidence.
type NumericCase = (&'static str, f64, f64, fn(f64) -> f64);

#[test]
fn numeric_reference_corpus_preserves_width_accuracy_and_control_outcomes() {
    let lib = StandardLibrary::new();
    let cases: &[NumericCase] = &[
        ("SQRT", 2.0, core::f64::consts::SQRT_2, f64::sqrt),
        ("LN", 2.0, core::f64::consts::LN_2, f64::ln),
        ("LOG", 10.0, 1.0, f64::log10),
        ("EXP", 1.0, core::f64::consts::E, f64::exp),
        ("SIN", 0.5, 0.479_425_538_604_203, f64::sin),
        ("COS", 0.5, 0.877_582_561_890_372_8, f64::cos),
        ("TAN", 0.5, 0.546_302_489_843_790_5, f64::tan),
        ("ASIN", 0.5, core::f64::consts::FRAC_PI_6, f64::asin),
        ("ACOS", 0.5, core::f64::consts::FRAC_PI_3, f64::acos),
        ("ATAN", 1.0, core::f64::consts::FRAC_PI_4, f64::atan),
    ];
    for &(name, input, reference, legacy) in cases {
        let Value::LReal(value) = lib.call(name, &[Value::LReal(input)]).unwrap() else {
            panic!("{name} must preserve LREAL");
        };
        let tolerance = 8.0 * f64::EPSILON * reference.abs().max(1.0);
        assert!((value - reference).abs() <= tolerance, "{name}: {value}");
        assert!(
            value < reference + 2.0 * tolerance,
            "{name} upper control threshold"
        );
        assert!(
            value > reference - 2.0 * tolerance,
            "{name} lower control threshold"
        );
        let old = legacy(input);
        assert!((old - reference).abs() <= tolerance, "legacy {name}");
        println!(
            "{name} input={:016x} old={:016x} portable={:016x}",
            input.to_bits(),
            old.to_bits(),
            value.to_bits()
        );

        let Value::Real(value) = lib.call(name, &[Value::Real(input as f32)]).unwrap() else {
            panic!("{name} must preserve REAL");
        };
        let reference = reference as f32;
        let tolerance = 4.0 * f32::EPSILON * reference.abs().max(1.0);
        assert!((value - reference).abs() <= tolerance, "REAL {name}");
        assert!(value < reference + 2.0 * tolerance);
        assert!(value > reference - 2.0 * tolerance);
    }
}

#[test]
fn numeric_special_values_and_faults_preserve_the_existing_policy() {
    let lib = StandardLibrary::new();
    assert_eq!(
        lib.call("SIN", &[Value::Real(f32::from_bits(1))]),
        Ok(Value::Real(f32::from_bits(1)))
    );
    for name in ["SIN", "TAN", "ASIN", "ATAN", "SQRT"] {
        let Value::LReal(value) = lib.call(name, &[Value::LReal(-0.0)]).unwrap() else {
            panic!("expected LREAL");
        };
        assert_eq!(value.to_bits(), (-0.0_f64).to_bits(), "{name}");
    }
    for (name, input) in [
        ("SQRT", -1.0),
        ("LN", 0.0),
        ("LOG", -1.0),
        ("ASIN", 2.0),
        ("ACOS", -2.0),
        ("EXP", 1024.0),
        ("SIN", f64::INFINITY),
    ] {
        assert_eq!(
            lib.call(name, &[Value::LReal(input)]),
            Err(RuntimeError::Overflow),
            "{name}"
        );
    }
    assert_eq!(
        lib.call("EXP", &[Value::Real(100.0)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("EXPT", &[Value::Real(10.0), Value::Real(100.0)]),
        Err(RuntimeError::Overflow)
    );
    assert_eq!(
        lib.call("EXPT", &[Value::LReal(2.0), Value::LReal(3.0)]),
        Ok(Value::LReal(8.0))
    );
    assert_eq!(lib.call("SQRT", &[Value::Real(9.0)]), Ok(Value::Real(3.0)));
}

#[test]
fn atan2_retains_width_promotion_and_signed_zero_quadrant() {
    let lib = StandardLibrary::new();
    for (args, real_result) in [
        ([Value::Real(-0.0), Value::Real(1.0)], true),
        ([Value::LReal(-0.0), Value::LReal(1.0)], false),
        ([Value::Real(-0.0), Value::LReal(1.0)], false),
        ([Value::LReal(-0.0), Value::Real(1.0)], false),
    ] {
        match lib.call("ATAN2", &args).unwrap() {
            Value::Real(value) if real_result => assert_eq!(value.to_bits(), (-0.0_f32).to_bits()),
            Value::LReal(value) if !real_result => {
                assert_eq!(value.to_bits(), (-0.0_f64).to_bits())
            }
            value => panic!("unexpected ATAN2 type: {value:?}"),
        }
    }
    assert_eq!(
        lib.call("ATAN2", &[Value::LReal(0.0), Value::LReal(-1.0)]),
        Ok(Value::LReal(core::f64::consts::PI))
    );
}

#[test]
fn numeric_fixture_preserves_plc_control_outputs() {
    use trust_runtime::harness::TestHarness;
    let source = r#"
PROGRAM NumericContract
VAR
    root : REAL;
    power : LREAL;
    sine : REAL;
    scaled : TIME;
    at_threshold : BOOL;
    above_threshold : BOOL;
    power_in_range : BOOL;
    sine_in_range : BOOL;
    time_matches : BOOL;
END_VAR
root := SQRT(REAL#9.0);
power := LREAL#2.0 ** LREAL#0.5;
sine := SIN(REAL#0.5);
scaled := T#3ms * -1.5;
at_threshold := root >= REAL#3.0;
above_threshold := root > REAL#3.0;
power_in_range := power > LREAL#1.414 AND power < LREAL#1.415;
sine_in_range := sine > REAL#0.4794 AND sine < REAL#0.4795;
time_matches := scaled = T#-4.5ms;
END_PROGRAM
"#;
    let mut harness = TestHarness::from_source(source).unwrap();
    harness.cycle();
    harness.assert_eq("at_threshold", true);
    harness.assert_eq("above_threshold", false);
    harness.assert_eq("power_in_range", true);
    harness.assert_eq("sine_in_range", true);
    harness.assert_eq("time_matches", true);
}

#[test]
fn duration_scaling_preserves_numeric_operand_context_and_duration_width() {
    use trust_runtime::harness::TestHarness;

    for (type_name, prefix, is_long) in [
        ("TIME", "T", false),
        ("LTIME", "LTIME", true),
        ("ShortDuration", "T", false),
        ("LongDuration", "LTIME", true),
    ] {
        for (operation, expected_nanos) in [
            ("* -1.5", -4_500_000),
            ("/ -2", -1_500_000),
            ("* -(1.0 + 0.5)", -4_500_000),
            ("/ (1 + 1)", 1_500_000),
            ("/ 7", 428_571),
        ] {
            let source = format!(
                "TYPE ShortDuration : TIME; LongDuration : LTIME; END_TYPE\n\
                 PROGRAM Main\nVAR result : {type_name}; END_VAR\n\
                 result := {prefix}#3ms {operation};\nEND_PROGRAM"
            );
            let mut harness = TestHarness::from_source(&source)
                .unwrap_or_else(|error| panic!("{type_name} {operation}: {error}"));
            harness.cycle();
            match harness.get_output("result").expect("duration output") {
                Value::Time(value) if !is_long => assert_eq!(value.as_nanos(), expected_nanos),
                Value::LTime(value) if is_long => assert_eq!(value.as_nanos(), expected_nanos),
                value => panic!("{type_name} {operation}: wrong duration width {value:?}"),
            }
        }
    }
}

#[test]
fn duration_scaling_rejects_reversed_division() {
    use trust_runtime::harness::TestHarness;

    for (type_name, prefix) in [("TIME", "T"), ("LTIME", "LTIME")] {
        let source = format!(
            "PROGRAM Main\nVAR result : {type_name}; END_VAR\n\
             result := 2 / {prefix}#3ms;\nEND_PROGRAM"
        );
        assert!(TestHarness::from_source(&source).is_err());
    }
}
