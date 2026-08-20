use crate::harness::{CompileSession, SourceFile, TestHarness};
use crate::value::Value;
use crate::Runtime;

fn source_files(files: &[(&str, &str)]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|(path, source)| SourceFile::with_path(*path, *source))
        .collect()
}

fn runtime(files: &[(&str, &str)]) -> Runtime {
    CompileSession::from_sources(source_files(files))
        .build_runtime()
        .unwrap_or_else(|error| panic!("constant-initializer fixture must compile: {error}"))
}

fn compile_error(files: &[(&str, &str)]) -> String {
    match CompileSession::from_sources(source_files(files)).build_runtime() {
        Ok(_) => panic!("constant-initializer fixture must fail"),
        Err(error) => error.to_string(),
    }
}

fn int_global(runtime: &Runtime, name: &str) -> i16 {
    match runtime.storage().get_global(name) {
        Some(Value::Int(value)) => *value,
        other => panic!("expected INT global {name}, got {other:?}"),
    }
}

#[test]
fn constant_initializer_contract_resolves_forward_reference_in_same_block() {
    let runtime = runtime(&[(
        "main.st",
        r#"
VAR_GLOBAL CONSTANT
    Derived : INT := Base + INT#3;
    Base : INT := INT#4;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "Derived"), 7);
    assert_eq!(int_global(&runtime, "Base"), 4);
}

#[test]
fn constant_initializer_contract_resolves_forward_reference_across_blocks() {
    let runtime = runtime(&[(
        "main.st",
        r#"
VAR_GLOBAL CONSTANT
    Derived : INT := Base * INT#2;
END_VAR
VAR_GLOBAL CONSTANT
    Base : INT := INT#6;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "Derived"), 12);
}

#[test]
fn constant_initializer_contract_resolves_forward_reference_across_sources() {
    let runtime = runtime(&[
        (
            "consumer.st",
            r#"
VAR_GLOBAL CONSTANT
    Derived : INT := Base + INT#5;
END_VAR
"#,
        ),
        (
            "provider.st",
            r#"
VAR_GLOBAL CONSTANT
    Base : INT := INT#8;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(int_global(&runtime, "Derived"), 13);
    assert_eq!(int_global(&runtime, "Base"), 8);
}

#[test]
fn constant_initializer_contract_source_permutation_preserves_values() {
    const DERIVED: &str = r#"
VAR_GLOBAL CONSTANT
    Derived : INT := Base + INT#2;
END_VAR
"#;
    const BASE: &str = r#"
VAR_GLOBAL CONSTANT
    Base : INT := INT#9;
END_VAR
"#;
    const PROGRAM: &str = r#"
PROGRAM Main
END_PROGRAM
"#;

    let forward = runtime(&[
        ("derived.st", DERIVED),
        ("base.st", BASE),
        ("main.st", PROGRAM),
    ]);
    let reverse = runtime(&[
        ("main.st", PROGRAM),
        ("base.st", BASE),
        ("derived.st", DERIVED),
    ]);

    for runtime in [&forward, &reverse] {
        assert_eq!(int_global(runtime, "Base"), 9);
        assert_eq!(int_global(runtime, "Derived"), 11);
    }
}

#[test]
fn constant_initializer_contract_resolves_reverse_multi_hop_graph() {
    let runtime = runtime(&[
        (
            "final.st",
            r#"
VAR_GLOBAL CONSTANT
    ResultValue : INT := Middle * INT#3;
END_VAR
"#,
        ),
        (
            "middle.st",
            r#"
VAR_GLOBAL CONSTANT
    Middle : INT := Base + INT#2;
END_VAR
"#,
        ),
        (
            "base.st",
            r#"
VAR_GLOBAL CONSTANT
    Base : INT := INT#4;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert_eq!(int_global(&runtime, "Base"), 4);
    assert_eq!(int_global(&runtime, "Middle"), 6);
    assert_eq!(int_global(&runtime, "ResultValue"), 18);
}

#[test]
fn constant_initializer_contract_dependency_lookup_is_case_insensitive() {
    let runtime = runtime(&[(
        "main.st",
        r#"
VAR_GLOBAL CONSTANT
    Derived : INT := bAsEvAlUe + INT#1;
    BaseValue : INT := INT#14;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "Derived"), 15);
}

#[test]
fn constant_initializer_contract_multi_name_forward_dependency_has_equal_values() {
    let runtime = runtime(&[(
        "main.st",
        r#"
VAR_GLOBAL CONSTANT
    First, Second, Third : INT := Seed + INT#1;
    Seed : INT := INT#20;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    for name in ["First", "Second", "Third"] {
        assert_eq!(int_global(&runtime, name), 21);
    }
}

#[test]
fn constant_initializer_contract_nonconstant_global_uses_later_constant_value() {
    let runtime = runtime(&[(
        "main.st",
        r#"
VAR_GLOBAL
    Observed : INT := Base + INT#4;
END_VAR
VAR_GLOBAL CONSTANT
    Base : INT := INT#10;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(int_global(&runtime, "Observed"), 14);
}

#[test]
fn constant_initializer_contract_program_global_uses_later_root_constant() {
    let runtime = runtime(&[
        (
            "program.st",
            r#"
PROGRAM Main
VAR_GLOBAL
    ProgramValue : INT := Base + INT#7;
END_VAR
END_PROGRAM
"#,
        ),
        (
            "constants.st",
            r#"
VAR_GLOBAL CONSTANT
    Base : INT := INT#5;
END_VAR
"#,
        ),
    ]);

    assert_eq!(int_global(&runtime, "ProgramValue"), 12);
}

#[test]
fn constant_initializer_contract_program_variable_uses_later_root_constant() {
    let mut harness = TestHarness::from_sources(&[
        r#"
PROGRAM Main
VAR
    Stored : INT := Base + INT#3;
END_VAR
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Stored;
END_PROGRAM
"#,
        r#"
VAR_GLOBAL CONSTANT
    Base : INT := INT#16;
END_VAR
"#,
    ])
    .expect("later project constant must initialize program storage");

    harness.cycle();
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(19)));
}

#[test]
fn constant_initializer_contract_function_local_constants_resolve_forward() {
    let mut harness = TestHarness::from_source(
        r#"
FUNCTION Calculate : INT
VAR CONSTANT
    Derived : INT := Base + INT#2;
    Base : INT := INT#7;
END_VAR
Calculate := Derived;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Calculate();
END_PROGRAM
"#,
    )
    .expect("function-local constant graph must compile");

    harness.cycle();
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(9)));
}

#[test]
fn constant_initializer_contract_program_constants_resolve_forward() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR CONSTANT
    Derived : INT := Base * INT#2;
    Base : INT := INT#6;
END_VAR
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Derived;
END_PROGRAM
"#,
    )
    .expect("program-local constant graph must compile");

    harness.cycle();
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(12)));
}

#[test]
fn constant_initializer_contract_function_block_constants_resolve_forward() {
    let mut harness = TestHarness::from_source(
        r#"
FUNCTION_BLOCK Limits
VAR CONSTANT
    Derived : INT := Base + INT#4;
    Base : INT := INT#5;
END_VAR
VAR_OUTPUT
    Value : INT;
END_VAR
Value := Derived;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Instance : Limits;
END_VAR
VAR_OUTPUT
    Observed : INT;
END_VAR
Instance();
Observed := Instance.Value;
END_PROGRAM
"#,
    )
    .expect("function-block constant graph must compile");

    harness.cycle();
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(9)));
}

#[test]
fn constant_initializer_contract_configuration_globals_resolve_forward() {
    let runtime = runtime(&[(
        "main.st",
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL
    Observed : INT := Derived + INT#1;
END_VAR
VAR_GLOBAL CONSTANT
    Derived : INT := Base + INT#2;
    Base : INT := INT#8;
END_VAR
PROGRAM P : Main;
END_CONFIGURATION
"#,
    )]);

    assert_eq!(int_global(&runtime, "Base"), 8);
    assert_eq!(int_global(&runtime, "Derived"), 10);
    assert_eq!(int_global(&runtime, "Observed"), 11);
}

#[test]
fn constant_initializer_contract_resource_globals_resolve_forward() {
    let runtime = runtime(&[(
        "main.st",
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
RESOURCE R ON CPU
VAR_GLOBAL
    Observed : INT := Derived + INT#1;
END_VAR
VAR_GLOBAL CONSTANT
    Derived : INT := Base * INT#2;
    Base : INT := INT#6;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    )]);

    assert_eq!(int_global(&runtime, "Base"), 6);
    assert_eq!(int_global(&runtime, "Derived"), 12);
    assert_eq!(int_global(&runtime, "Observed"), 13);
}

#[test]
fn constant_initializer_contract_configuration_constant_is_visible_to_resource() {
    let runtime = runtime(&[(
        "main.st",
        r#"
PROGRAM Main
END_PROGRAM
CONFIGURATION C
VAR_GLOBAL CONSTANT
    ConfigBase : INT := INT#11;
END_VAR
RESOURCE R ON CPU
VAR_GLOBAL CONSTANT
    ResourceValue : INT := ConfigBase + INT#4;
END_VAR
PROGRAM P : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    )]);

    assert_eq!(int_global(&runtime, "ResourceValue"), 15);
}

#[test]
fn constant_initializer_contract_forward_values_preserve_elementary_tags() {
    let runtime = runtime(&[(
        "main.st",
        r#"
VAR_GLOBAL CONSTANT
    ForwardBool : BOOL := BaseBool;
    BaseBool : BOOL := TRUE;
    ForwardReal : REAL := BaseReal;
    BaseReal : REAL := REAL#1.5;
    ForwardText : STRING := BaseText;
    BaseText : STRING := 'ready';
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert_eq!(
        runtime.storage().get_global("ForwardBool"),
        Some(&Value::Bool(true))
    );
    assert_eq!(
        runtime.storage().get_global("ForwardReal"),
        Some(&Value::Real(1.5))
    );
    assert_eq!(
        runtime.storage().get_global("ForwardText"),
        Some(&Value::String("ready".into()))
    );
}

#[test]
fn constant_initializer_contract_rejects_self_cycle() {
    let error = compile_error(&[(
        "self_cycle.st",
        r#"
VAR_GLOBAL CONSTANT
    Loop : INT := Loop + INT#1;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("cyclic")
            && error.to_ascii_lowercase().contains("loop"),
        "{error}"
    );
}

#[test]
fn constant_initializer_contract_rejects_two_node_cycle() {
    let error = compile_error(&[(
        "cycle.st",
        r#"
VAR_GLOBAL CONSTANT
    Left : INT := Right + INT#1;
    Right : INT := Left + INT#1;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.to_ascii_lowercase().contains("cyclic")
            && (error.to_ascii_lowercase().contains("left")
                || error.to_ascii_lowercase().contains("right")),
        "{error}"
    );
}

#[test]
fn constant_initializer_contract_rejects_cross_source_cycle() {
    let error = compile_error(&[
        (
            "first.st",
            r#"
VAR_GLOBAL CONSTANT
    First : INT := Second + INT#1;
END_VAR
"#,
        ),
        (
            "second.st",
            r#"
VAR_GLOBAL CONSTANT
    Second : INT := Third + INT#1;
END_VAR
"#,
        ),
        (
            "third.st",
            r#"
VAR_GLOBAL CONSTANT
    Third : INT := First + INT#1;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert!(error.to_ascii_lowercase().contains("cyclic"), "{error}");
}

#[test]
fn constant_initializer_contract_rejects_mutable_dependency() {
    let error = compile_error(&[(
        "mutable.st",
        r#"
VAR_GLOBAL
    MutableValue : INT := INT#4;
    Derived : INT := MutableValue + INT#1;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.contains("variable initializer must be a literal or constant expression"),
        "{error}"
    );
}

#[test]
fn constant_initializer_contract_rejects_undefined_dependency_with_consumer_label() {
    let error = compile_error(&[
        (
            "consumer.st",
            r#"
VAR_GLOBAL CONSTANT
    Derived : INT := MissingBase + INT#1;
END_VAR
"#,
        ),
        (
            "main.st",
            r#"
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    assert!(error.contains("consumer.st:"), "{error}");
    assert!(
        error.to_ascii_lowercase().contains("missingbase")
            && (error.to_ascii_lowercase().contains("undefined")
                || error.to_ascii_lowercase().contains("unknown")),
        "{error}"
    );
}

#[test]
fn constant_initializer_contract_rejects_divide_by_zero_through_forward_dependency() {
    let error = compile_error(&[(
        "division.st",
        r#"
VAR_GLOBAL CONSTANT
    Result : INT := INT#12 / Divisor;
    Divisor : INT := INT#0;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(
        error.contains("constant expression divides by zero")
            || error.to_ascii_lowercase().contains("division by zero"),
        "{error}"
    );
}

#[test]
fn constant_initializer_contract_rejects_overflow_through_forward_dependency() {
    let error = compile_error(&[(
        "overflow.st",
        r#"
VAR_GLOBAL CONSTANT
    Result : LINT := Maximum + LINT#1;
    Maximum : LINT := LINT#9223372036854775807;
END_VAR
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(error.to_ascii_lowercase().contains("overflow"), "{error}");
}
