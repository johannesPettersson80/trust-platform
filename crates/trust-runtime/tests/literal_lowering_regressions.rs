//! Regression tests for literal lowering in the harness: ULINT literals above i64::MAX, exact
//! TIME and LTIME literals, and REAL# literals rounded once from their decimal text. Each case
//! compiles Structured Text through TestHarness and reads the result after one cycle.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

fn compile(source: &str) -> TestHarness {
    TestHarness::from_source(source).unwrap_or_else(|error| panic!("fixture must compile: {error}"))
}

fn output_after_cycle(source: &str, name: &str) -> Value {
    let mut harness = compile(source);
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness.try_get_output(name).unwrap()
}

fn assert_compile_error(source: &str) {
    assert!(
        TestHarness::from_source(source).is_err(),
        "fixture must be rejected at compile time: {source}"
    );
}

fn program(declarations: &str, body: &str) -> String {
    format!("PROGRAM Main\nVAR {declarations} END_VAR\n{body}\nEND_PROGRAM")
}

#[test]
fn typed_ulint_literal_at_u64_max_is_exact() {
    let source = program("x : ULINT;", "x := ULINT#18446744073709551615;");
    assert_eq!(output_after_cycle(&source, "x"), Value::ULInt(u64::MAX));
}

#[test]
fn typed_udint_literal_at_u32_max_is_unchanged() {
    let source = program("x : UDINT;", "x := UDINT#4294967295;");
    assert_eq!(output_after_cycle(&source, "x"), Value::UDInt(u32::MAX));
}

#[test]
fn untyped_literal_at_u64_max_assigned_to_ulint_is_exact() {
    let source = program("x : ULINT;", "x := 18446744073709551615;");
    assert_eq!(output_after_cycle(&source, "x"), Value::ULInt(u64::MAX));
}

#[test]
fn untyped_literal_above_i32_assigned_to_ulint_keeps_its_value() {
    let source = program("x : ULINT;", "x := 3000000000;");
    assert_eq!(
        output_after_cycle(&source, "x"),
        Value::ULInt(3_000_000_000)
    );
}

#[test]
fn untyped_literal_assigned_to_ulint_alias_is_exact() {
    let source = "TYPE Count : ULINT;\nEND_TYPE\nPROGRAM Main\nVAR c : Count; END_VAR\n\
                  c := 18446744073709551615;\nEND_PROGRAM";
    assert_eq!(output_after_cycle(source, "c"), Value::ULInt(u64::MAX));
}

#[test]
fn literal_above_u64_max_is_rejected() {
    assert_compile_error(&program("x : ULINT;", "x := ULINT#18446744073709551616;"));
    assert_compile_error(&program("x : ULINT;", "x := 18446744073709551616;"));
}

#[test]
fn typed_negative_ulint_literal_stays_rejected() {
    assert_compile_error(&program("x : ULINT;", "x := ULINT#-1;"));
}

#[test]
fn signed_literals_keep_their_values() {
    let typed = program("x : LINT;", "x := LINT#-1;");
    assert_eq!(output_after_cycle(&typed, "x"), Value::LInt(-1));
    let untyped = program("y : DINT;", "y := -5;");
    assert_eq!(output_after_cycle(&untyped, "y"), Value::DInt(-5));
}

#[test]
fn time_literal_at_i64_max_is_exact() {
    let source = program("t : TIME;", "t := TIME#9223372036s854ms775us807ns;");
    assert_eq!(
        output_after_cycle(&source, "t"),
        Value::Time(Duration::from_nanos(i64::MAX))
    );
}

#[test]
fn ltime_literal_at_i64_max_is_exact() {
    let source = program("t : LTIME;", "t := LTIME#9223372036s854ms775us807ns;");
    assert_eq!(
        output_after_cycle(&source, "t"),
        Value::LTime(Duration::from_nanos(i64::MAX))
    );
}

#[test]
fn time_and_ltime_literals_one_nanosecond_past_i64_max_are_rejected() {
    assert_compile_error(&program(
        "t : TIME;",
        "t := TIME#9223372036s854ms775us808ns;",
    ));
    assert_compile_error(&program(
        "t : LTIME;",
        "t := LTIME#9223372036s854ms775us808ns;",
    ));
}

#[test]
fn time_literals_with_fractions_and_compound_units_are_exact() {
    let cases: [(&str, i64); 3] = [
        ("T#1.5s", 1_500_000_000),
        ("T#0.000000001s", 1),
        ("T#1d2h3m4s", 93_784_000_000_000),
    ];
    for (text, nanos) in cases {
        let source = program("t : TIME;", &format!("t := {text};"));
        assert_eq!(
            output_after_cycle(&source, "t"),
            Value::Time(Duration::from_nanos(nanos)),
            "{text}"
        );
    }
}

#[test]
fn negative_time_literals_are_exact_and_reach_i64_min() {
    let fraction = program("t : TIME;", "t := T#-1.5s;");
    assert_eq!(
        output_after_cycle(&fraction, "t"),
        Value::Time(Duration::from_nanos(-1_500_000_000))
    );
    let compound = program("t : LTIME;", "t := LT#-1d2h3m4s;");
    assert_eq!(
        output_after_cycle(&compound, "t"),
        Value::LTime(Duration::from_nanos(-93_784_000_000_000))
    );
    let lowest = program("t : TIME;", "t := TIME#-9223372036s854ms775us808ns;");
    assert_eq!(
        output_after_cycle(&lowest, "t"),
        Value::Time(Duration::from_nanos(i64::MIN))
    );
    assert_compile_error(&program(
        "t : TIME;",
        "t := TIME#-9223372036s854ms775us809ns;",
    ));
}

#[test]
fn sub_nanosecond_fractions_round_half_away_from_zero() {
    let cases: [(&str, i64); 4] = [
        ("T#0.0000000004s", 0),
        ("T#0.0000000005s", 1),
        ("T#0.0000000015s", 2),
        ("T#-0.0000000005s", -1),
    ];
    for (text, nanos) in cases {
        let source = program("t : TIME;", &format!("t := {text};"));
        assert_eq!(
            output_after_cycle(&source, "t"),
            Value::Time(Duration::from_nanos(nanos)),
            "{text}"
        );
    }
}

#[test]
fn real_literal_rounds_once_from_its_decimal_text() {
    // Through binary64 first this literal lands on the midpoint 1 + 2^-24 and ties to 1.0. The
    // decimal lies just above that midpoint, so one rounding gives 1 + 2^-23.
    let source = program("r : REAL;", "r := REAL#1.0000000596046447753906250000001;");
    assert_eq!(
        output_after_cycle(&source, "r"),
        Value::Real(1.0 + f32::EPSILON)
    );
    let negative = program("r : REAL;", "r := REAL#-1.0000000596046447753906250000001;");
    assert_eq!(
        output_after_cycle(&negative, "r"),
        Value::Real(-(1.0 + f32::EPSILON))
    );
}

#[test]
fn ordinary_real_literals_are_unchanged() {
    let whole = program("r : REAL;", "r := REAL#5;");
    assert_eq!(output_after_cycle(&whole, "r"), Value::Real(5.0));
    let fraction = program("r : REAL;", "r := REAL#1.5;");
    assert_eq!(output_after_cycle(&fraction, "r"), Value::Real(1.5));
    let tenth = program("r : REAL;", "r := REAL#0.1;");
    assert_eq!(output_after_cycle(&tenth, "r"), Value::Real(0.1));
}

#[test]
fn real_literal_beyond_the_real_range_is_rejected() {
    assert_compile_error(&program("r : REAL;", "r := REAL#1.0E39;"));
}

/// One integer destination: its maximum and minimum as untyped literal text, the values they must
/// lower to, and the literal text one past each bound.
struct IntegerBounds {
    name: &'static str,
    max: String,
    max_value: Value,
    min: String,
    min_value: Value,
    past_max: String,
    below_min: String,
}

fn integer_bounds_of<T>(
    name: &'static str,
    variant: fn(T) -> Value,
    min: T,
    max: T,
) -> IntegerBounds
where
    T: Copy + std::fmt::Display + Into<i128>,
{
    let max_wide: i128 = max.into();
    let min_wide: i128 = min.into();
    IntegerBounds {
        name,
        max: max.to_string(),
        max_value: variant(max),
        min: min.to_string(),
        min_value: variant(min),
        past_max: (max_wide + 1).to_string(),
        below_min: (min_wide - 1).to_string(),
    }
}

fn integer_bounds() -> Vec<IntegerBounds> {
    vec![
        integer_bounds_of("SINT", Value::SInt, i8::MIN, i8::MAX),
        integer_bounds_of("INT", Value::Int, i16::MIN, i16::MAX),
        integer_bounds_of("DINT", Value::DInt, i32::MIN, i32::MAX),
        integer_bounds_of("LINT", Value::LInt, i64::MIN, i64::MAX),
        integer_bounds_of("USINT", Value::USInt, u8::MIN, u8::MAX),
        integer_bounds_of("UINT", Value::UInt, u16::MIN, u16::MAX),
        integer_bounds_of("UDINT", Value::UDInt, u32::MIN, u32::MAX),
        integer_bounds_of("ULINT", Value::ULInt, u64::MIN, u64::MAX),
    ]
}

/// A program that assigns `value` to a variable `x` of type `type_name` in its body.
fn assignment_source(type_name: &str, value: &str) -> String {
    program(&format!("x : {type_name};"), &format!("x := {value};"))
}

/// A program whose variable `x` of type `type_name` is initialised to `value`.
fn initializer_source(type_name: &str, value: &str) -> String {
    program(&format!("x : {type_name} := {value};"), "")
}

#[test]
fn untyped_integer_bounds_are_exact_in_assignments() {
    for bounds in integer_bounds() {
        let name = bounds.name;
        // The ULINT maximum is checked by `untyped_ulint_maximum_is_exact` below.
        if name != "ULINT" {
            let source = assignment_source(name, &bounds.max);
            assert_eq!(
                output_after_cycle(&source, "x"),
                bounds.max_value,
                "{name} maximum"
            );
        }
        let source = assignment_source(name, &bounds.min);
        assert_eq!(
            output_after_cycle(&source, "x"),
            bounds.min_value,
            "{name} minimum"
        );
    }
}

#[test]
fn untyped_integer_bounds_are_exact_in_initializers() {
    for bounds in integer_bounds() {
        let name = bounds.name;
        // The ULINT maximum and the LINT minimum are checked by the tests below.
        if name != "ULINT" {
            let source = initializer_source(name, &bounds.max);
            assert_eq!(
                output_after_cycle(&source, "x"),
                bounds.max_value,
                "{name} maximum"
            );
        }
        if name != "LINT" {
            let source = initializer_source(name, &bounds.min);
            assert_eq!(
                output_after_cycle(&source, "x"),
                bounds.min_value,
                "{name} minimum"
            );
        }
    }
}

#[test]
fn one_past_each_integer_bound_is_rejected_in_assignments_and_initializers() {
    for bounds in integer_bounds() {
        let name = bounds.name;
        assert_compile_error(&assignment_source(name, &bounds.past_max));
        assert_compile_error(&assignment_source(name, &bounds.below_min));
        assert_compile_error(&initializer_source(name, &bounds.past_max));
        assert_compile_error(&initializer_source(name, &bounds.below_min));
    }
}

#[test]
fn typed_minimum_literals_are_accepted_in_assignments() {
    let cases = [
        ("LINT", "LINT#-9223372036854775808", Value::LInt(i64::MIN)),
        ("DINT", "DINT#-2147483648", Value::DInt(i32::MIN)),
        ("INT", "INT#-32768", Value::Int(i16::MIN)),
        ("SINT", "SINT#-128", Value::SInt(i8::MIN)),
    ];
    for (name, literal, expected) in cases {
        let source = assignment_source(name, literal);
        assert_eq!(output_after_cycle(&source, "x"), expected, "{literal}");
    }
}

#[test]
fn typed_minimum_literals_are_accepted_in_initializers() {
    let cases = [
        ("DINT", "DINT#-2147483648", Value::DInt(i32::MIN)),
        ("INT", "INT#-32768", Value::Int(i16::MIN)),
        ("SINT", "SINT#-128", Value::SInt(i8::MIN)),
    ];
    for (name, literal, expected) in cases {
        let source = initializer_source(name, literal);
        assert_eq!(output_after_cycle(&source, "x"), expected, "{literal}");
    }
}

#[test]
fn typed_literals_one_below_their_minimum_are_rejected() {
    let cases = [
        ("LINT", "LINT#-9223372036854775809"),
        ("DINT", "DINT#-2147483649"),
        ("INT", "INT#-32769"),
        ("SINT", "SINT#-129"),
    ];
    for (name, literal) in cases {
        assert_compile_error(&assignment_source(name, literal));
        assert_compile_error(&initializer_source(name, literal));
    }
}

// The tests from here to the end of the file depend on the trust-hir literal change made in this
// round. Before it, analysis rejected integer literals above i64::MAX and LINT minimums in
// initializers.

#[test]
fn untyped_ulint_maximum_is_exact() {
    let assignment = assignment_source("ULINT", &u64::MAX.to_string());
    assert_eq!(output_after_cycle(&assignment, "x"), Value::ULInt(u64::MAX));
    let initializer = initializer_source("ULINT", &u64::MAX.to_string());
    assert_eq!(
        output_after_cycle(&initializer, "x"),
        Value::ULInt(u64::MAX)
    );
}

#[test]
fn untyped_lint_minimum_is_exact_in_initializers() {
    let source = initializer_source("LINT", &i64::MIN.to_string());
    assert_eq!(output_after_cycle(&source, "x"), Value::LInt(i64::MIN));
}

#[test]
fn typed_lint_minimum_is_exact_in_initializers() {
    let source = initializer_source("LINT", "LINT#-9223372036854775808");
    assert_eq!(output_after_cycle(&source, "x"), Value::LInt(i64::MIN));
}

#[test]
fn untyped_literal_passed_to_a_lint_input_takes_lint() {
    let positive = "FUNCTION Echo : LINT\nVAR_INPUT n : LINT; END_VAR\nEcho := n;\nEND_FUNCTION\n\
                    PROGRAM Main\nVAR r : LINT; END_VAR\nr := Echo(3000000000);\nEND_PROGRAM";
    assert_eq!(
        output_after_cycle(positive, "r"),
        Value::LInt(3_000_000_000)
    );
    let negative = "FUNCTION Echo : LINT\nVAR_INPUT n : LINT; END_VAR\nEcho := n;\nEND_FUNCTION\n\
                    PROGRAM Main\nVAR r : LINT; END_VAR\nr := Echo(-3000000000);\nEND_PROGRAM";
    assert_eq!(
        output_after_cycle(negative, "r"),
        Value::LInt(-3_000_000_000)
    );
}

#[test]
fn untyped_literal_returned_from_a_lint_function_takes_lint() {
    let by_return = "FUNCTION Limit : LINT\nRETURN 3000000000;\nEND_FUNCTION\n\
                     PROGRAM Main\nVAR r : LINT; END_VAR\nr := Limit();\nEND_PROGRAM";
    assert_eq!(
        output_after_cycle(by_return, "r"),
        Value::LInt(3_000_000_000)
    );
    let by_result_slot = "FUNCTION Big : LINT\nBig := 3000000000;\nEND_FUNCTION\n\
                          PROGRAM Main\nVAR r : LINT; END_VAR\nr := Big();\nEND_PROGRAM";
    assert_eq!(
        output_after_cycle(by_result_slot, "r"),
        Value::LInt(3_000_000_000)
    );
}

#[test]
fn constant_and_global_initializers_take_their_declared_type() {
    let constant = "PROGRAM Main\nVAR CONSTANT c : LINT := 3000000000; END_VAR\n\
                    VAR r : LINT; END_VAR\nr := c;\nEND_PROGRAM";
    assert_eq!(
        output_after_cycle(constant, "r"),
        Value::LInt(3_000_000_000)
    );
    let global = "VAR_GLOBAL g : LINT := 3000000000; END_VAR\n\
                  PROGRAM Main\nVAR r : LINT; END_VAR\nr := g;\nEND_PROGRAM";
    assert_eq!(output_after_cycle(global, "r"), Value::LInt(3_000_000_000));
}

#[test]
fn comparison_literal_above_i32_takes_the_udint_operand_type() {
    let above = "PROGRAM Main\nVAR d : UDINT := 4000000000; r : INT := 0; END_VAR\n\
                 IF d > 3000000000 THEN\nr := 1;\nEND_IF;\nEND_PROGRAM";
    assert_eq!(output_after_cycle(above, "r"), Value::Int(1));
    let below = "PROGRAM Main\nVAR d : UDINT := 4000000000; r : INT := 0; END_VAR\n\
                 IF d < 3000000000 THEN\nr := 1;\nEND_IF;\nEND_PROGRAM";
    assert_eq!(output_after_cycle(below, "r"), Value::Int(0));
}

#[test]
fn comparison_literal_above_i32_takes_the_lint_operand_type() {
    let source = "PROGRAM Main\nVAR x : LINT := 5000000000; r : INT := 0; END_VAR\n\
                  IF x > 4000000000 THEN\nr := 1;\nEND_IF;\nEND_PROGRAM";
    assert_eq!(output_after_cycle(source, "r"), Value::Int(1));
}

#[test]
fn comparison_literal_outside_the_other_operand_type_is_rejected() {
    assert_compile_error(
        "PROGRAM Main\nVAR d : UDINT; r : INT := 0; END_VAR\n\
         IF d > 5000000000 THEN\nr := 1;\nEND_IF;\nEND_PROGRAM",
    );
}

#[test]
fn array_elements_take_the_declared_element_type() {
    let source = "PROGRAM Main\nVAR x : ARRAY[0..1] OF LINT := [3000000000, -3000000000]; \
                  a : LINT; b : LINT; END_VAR\na := x[0];\nb := x[1];\nEND_PROGRAM";
    assert_eq!(output_after_cycle(source, "a"), Value::LInt(3_000_000_000));
    assert_eq!(output_after_cycle(source, "b"), Value::LInt(-3_000_000_000));
}

#[test]
fn struct_field_takes_its_declared_ulint_type_from_a_default_and_an_initializer() {
    let by_default = "TYPE S : STRUCT f : ULINT := 18446744073709551615; END_STRUCT END_TYPE\n\
                      PROGRAM Main\nVAR s : S; v : ULINT; END_VAR\nv := s.f;\nEND_PROGRAM";
    assert_eq!(output_after_cycle(by_default, "v"), Value::ULInt(u64::MAX));
    let by_initializer = "TYPE S : STRUCT f : ULINT; END_STRUCT END_TYPE\n\
                          PROGRAM Main\nVAR s : S := (f := 18446744073709551615);\n\
                          v : ULINT; END_VAR\nv := s.f;\nEND_PROGRAM";
    assert_eq!(
        output_after_cycle(by_initializer, "v"),
        Value::ULInt(u64::MAX)
    );
}

#[test]
fn integer_subrange_takes_its_base_type_for_initializers_and_assignments() {
    let initialized = "PROGRAM Main\nVAR x : LINT(0..10000000000) := 5000000000;\n\
                       y : LINT; END_VAR\ny := x;\nEND_PROGRAM";
    assert_eq!(
        output_after_cycle(initialized, "y"),
        Value::LInt(5_000_000_000)
    );
    let assigned = "PROGRAM Main\nVAR x : LINT(0..10000000000); y : LINT; END_VAR\n\
                    x := 5000000000;\ny := x;\nEND_PROGRAM";
    assert_eq!(
        output_after_cycle(assigned, "y"),
        Value::LInt(5_000_000_000)
    );
}

#[test]
fn integer_subrange_bound_violation_is_rejected() {
    assert_compile_error(
        "PROGRAM Main\nVAR x : LINT(0..10000000000) := 20000000000; END_VAR\nEND_PROGRAM",
    );
    assert_compile_error(
        "PROGRAM Main\nVAR x : LINT(0..10000000000); END_VAR\nx := 20000000000;\nEND_PROGRAM",
    );
}

#[test]
fn literal_above_lint_maximum_is_reported_by_analysis_with_its_range() {
    let error = TestHarness::from_source(
        "PROGRAM Main\nVAR x : LINT; END_VAR\nx := 9223372036854775808;\nEND_PROGRAM",
    )
    .err()
    .expect("fixture must be rejected");
    assert!(
        error
            .to_string()
            .contains("integer literal 9223372036854775808 is outside the range of LINT"),
        "{error}"
    );
}
