use crate::harness::{CompileSession, SourceFile, TestHarness};
use crate::value::Value;

fn source_files(files: &[(&str, &str)]) -> Vec<SourceFile> {
    files
        .iter()
        .map(|(path, source)| SourceFile::with_path(*path, *source))
        .collect()
}

fn run(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("POU initializer fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn run_sources(sources: &[&str]) -> TestHarness {
    let mut harness = TestHarness::from_sources(sources)
        .unwrap_or_else(|error| panic!("POU initializer fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn compile_error(files: &[(&str, &str)]) -> String {
    match CompileSession::from_sources(source_files(files)).build_runtime() {
        Ok(_) => panic!("POU initializer fixture must fail"),
        Err(error) => error.to_string(),
    }
}

#[test]
fn pou_initializer_dependency_function_input_uses_later_local_constant() {
    let harness = run(r#"
FUNCTION ReadDefault : INT
VAR_INPUT
    Value : INT := Base;
END_VAR
VAR CONSTANT
    Base : INT := INT#7;
END_VAR
ReadDefault := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := ReadDefault();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_dependency_function_output_uses_later_local_constant() {
    let harness = run(r#"
FUNCTION Produce : INT
VAR_OUTPUT
    Produced : INT := InitialValue;
END_VAR
VAR CONSTANT
    InitialValue : INT := INT#11;
END_VAR
Produce := INT#0;
END_FUNCTION
PROGRAM Main
VAR
    Ignored : INT;
END_VAR
VAR_OUTPUT
    Observed : INT;
END_VAR
Ignored := Produce(Produced => Observed);
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(11)));
}

#[test]
fn pou_initializer_dependency_function_automatic_locals_use_later_constants() {
    let harness = run(r#"
FUNCTION Calculate : INT
VAR
    LocalValue : INT := Base;
END_VAR
VAR_TEMP
    TempValue : INT := Offset;
END_VAR
VAR CONSTANT
    Offset : INT := INT#4;
    Base : INT := INT#6;
END_VAR
Calculate := LocalValue + TempValue;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Calculate();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(10)));
}

#[test]
fn pou_initializer_dependency_function_static_uses_later_constant_once() {
    let harness = run(r#"
FUNCTION Next : INT
VAR_STAT
    Counter : INT := InitialValue;
END_VAR
VAR CONSTANT
    InitialValue : INT := INT#8;
END_VAR
Counter := Counter + INT#1;
Next := Counter;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
First := Next();
Second := Next();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("First"), Some(Value::Int(9)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(10)));
}

#[test]
fn pou_initializer_dependency_method_input_and_local_use_later_constants() {
    let harness = run(r#"
CLASS Calculator
METHOD PUBLIC Read : INT
VAR_INPUT
    Delta : INT := Base;
END_VAR
VAR
    LocalValue : INT := Offset;
END_VAR
VAR CONSTANT
    Offset : INT := INT#4;
    Base : INT := INT#3;
END_VAR
Read := Delta + LocalValue;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    Instance : Calculator;
END_VAR
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Instance.Read();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_dependency_function_block_sections_share_complete_graph() {
    let harness = run(r#"
FUNCTION_BLOCK Accumulate
VAR_INPUT
    Increment : INT := DefaultStep;
END_VAR
VAR_OUTPUT
    Total : INT := InitialValue;
END_VAR
VAR
    Stored : INT := InitialValue;
END_VAR
VAR_TEMP
    Scratch : INT := TempValue;
END_VAR
VAR CONSTANT
    TempValue : INT := INT#3;
    DefaultStep : INT := INT#2;
    InitialValue : INT := INT#10;
END_VAR
Stored := Stored + Increment;
Total := Total + Stored + Scratch;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Instance : Accumulate;
END_VAR
VAR_OUTPUT
    Observed : INT;
END_VAR
Instance();
Observed := Instance.Total;
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(25)));
}

#[test]
fn pou_initializer_dependency_program_sections_share_complete_graph() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR_OUTPUT
    Observed : INT := InitialValue;
END_VAR
VAR
    Stored : INT := InitialValue;
END_VAR
VAR_TEMP
    Scratch : INT := TempValue;
END_VAR
VAR CONSTANT
    TempValue : INT := INT#2;
    InitialValue : INT := INT#10;
END_VAR
Stored := Stored + INT#1;
Scratch := Scratch + INT#1;
Observed := Observed + Stored + Scratch;
END_PROGRAM
"#,
    )
    .expect("program initializer fixture must compile");

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(10)));
    let main_instance = match harness.runtime().storage().get_global("Main") {
        Some(Value::Instance(instance)) => *instance,
        other => panic!("expected Main instance, got {other:?}"),
    };
    assert_eq!(
        harness
            .runtime()
            .storage()
            .get_instance_var(main_instance, "Stored"),
        Some(&Value::Int(10))
    );
    let scratch = harness
        .runtime()
        .programs()
        .values()
        .find(|program| program.name == "Main")
        .and_then(|program| program.temps.iter().find(|var| var.name == "Scratch"))
        .expect("Scratch temp");
    assert!(matches!(
        &scratch.initializer,
        Some(crate::program_model::Expr::Literal(Value::Int(2)))
    ));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "{:?}; observed={:?}; stored={:?}",
        cycle.errors,
        harness.get_output("Observed"),
        harness
            .runtime()
            .storage()
            .get_instance_var(main_instance, "Stored")
    );

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(24)));
}

#[test]
fn pou_initializer_dependency_later_project_constant_initializes_function_sections() {
    let harness = run_sources(&[
        r#"
FUNCTION Read : INT
VAR_INPUT
    InputValue : INT := ProjectBase;
END_VAR
VAR_OUTPUT
    OutputValue : INT := ProjectBase + INT#1;
END_VAR
VAR
    LocalValue : INT := ProjectBase + INT#2;
END_VAR
OutputValue := OutputValue + InputValue;
Read := LocalValue;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    ResultValue : INT;
    ResultOutput : INT;
END_VAR
ResultValue := Read(OutputValue => ResultOutput);
END_PROGRAM
"#,
        r#"
VAR_GLOBAL CONSTANT
    ProjectBase : INT := INT#5;
END_VAR
"#,
    ]);

    assert_eq!(harness.get_output("ResultValue"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("ResultOutput"), Some(Value::Int(11)));
}

#[test]
fn pou_initializer_dependency_source_permutation_preserves_pou_values() {
    const FUNCTION_SOURCE: &str = r#"
FUNCTION Read : INT
VAR_INPUT
    Value : INT := ProjectBase;
END_VAR
Read := Value;
END_FUNCTION
"#;
    const CONSTANT_SOURCE: &str = r#"
VAR_GLOBAL CONSTANT
    ProjectBase : INT := INT#13;
END_VAR
"#;
    const PROGRAM_SOURCE: &str = r#"
PROGRAM Main
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Read();
END_PROGRAM
"#;

    let first = run_sources(&[FUNCTION_SOURCE, CONSTANT_SOURCE, PROGRAM_SOURCE]);
    let second = run_sources(&[PROGRAM_SOURCE, CONSTANT_SOURCE, FUNCTION_SOURCE]);

    assert_eq!(first.get_output("Observed"), Some(Value::Int(13)));
    assert_eq!(second.get_output("Observed"), Some(Value::Int(13)));
}

#[test]
fn pou_initializer_dependency_using_import_selects_namespace_constant() {
    let harness = run(r#"
NAMESPACE Limits
VAR_GLOBAL CONSTANT
    DefaultValue : INT := INT#17;
END_VAR
END_NAMESPACE
FUNCTION Read : INT
USING Limits;
VAR_INPUT
    Value : INT := DefaultValue;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Read();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(17)));
}

#[test]
fn pou_initializer_dependency_same_leaf_local_constants_remain_pou_local() {
    let harness = run(r#"
FUNCTION First : INT
VAR_INPUT
    Value : INT := Limit;
END_VAR
VAR CONSTANT
    Limit : INT := INT#2;
END_VAR
First := Value;
END_FUNCTION
FUNCTION Second : INT
VAR_INPUT
    Value : INT := Limit;
END_VAR
VAR CONSTANT
    Limit : INT := INT#5;
END_VAR
Second := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    FirstValue : INT;
    SecondValue : INT;
END_VAR
FirstValue := First();
SecondValue := Second();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("FirstValue"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("SecondValue"), Some(Value::Int(5)));
}

#[test]
fn pou_initializer_dependency_lookup_is_case_insensitive() {
    let harness = run(r#"
FUNCTION Read : INT
VAR_INPUT
    Value : INT := defaultvalue;
END_VAR
VAR CONSTANT
    DefaultValue : INT := INT#19;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Read();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(19)));
}

#[test]
fn pou_initializer_dependency_reverse_multi_hop_graph_is_resolved() {
    let harness = run(r#"
FUNCTION Read : INT
VAR_INPUT
    Value : INT := Top;
END_VAR
VAR CONSTANT
    Top : INT := Middle + INT#1;
    Middle : INT := Base + INT#1;
    Base : INT := INT#5;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    Observed : INT;
END_VAR
Observed := Read();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_dependency_multi_name_output_gets_independent_initialized_slots() {
    let harness = run(r#"
FUNCTION Produce : INT
VAR_OUTPUT
    First, Second : INT := InitialValue;
END_VAR
VAR CONSTANT
    InitialValue : INT := INT#4;
END_VAR
First := First + INT#1;
Produce := INT#0;
END_FUNCTION
PROGRAM Main
VAR
    Ignored : INT;
END_VAR
VAR_OUTPUT
    FirstObserved : INT;
    SecondObserved : INT;
END_VAR
Ignored := Produce(First => FirstObserved, Second => SecondObserved);
END_PROGRAM
"#);

    assert_eq!(harness.get_output("FirstObserved"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("SecondObserved"), Some(Value::Int(4)));
}

#[test]
fn pou_initializer_dependency_rejects_var_in_out_initializer() {
    let error = compile_error(&[(
        "in_out_initializer.st",
        r#"
FUNCTION Read : INT
VAR_IN_OUT
    Value : INT := INT#3;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    )]);

    let lower = error.to_ascii_lowercase();
    assert!(
        lower.contains("var_in_out") || lower.contains("initializer") || lower.contains("expected"),
        "{error}"
    );
}

#[test]
fn pou_initializer_dependency_rejects_var_external_initializer() {
    let error = compile_error(&[(
        "external_initializer.st",
        r#"
VAR_GLOBAL
    Shared : INT;
END_VAR
FUNCTION Read : INT
VAR_EXTERNAL
    Shared : INT := INT#3;
END_VAR
Read := Shared;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    )]);

    let lower = error.to_ascii_lowercase();
    assert!(
        lower.contains("var_external")
            || lower.contains("initializer")
            || lower.contains("initial value"),
        "{error}"
    );
}

#[test]
fn pou_initializer_dependency_rejects_mutable_local_operand() {
    let error = compile_error(&[(
        "mutable_operand.st",
        r#"
FUNCTION Read : INT
VAR
    MutableValue : INT := INT#3;
    Initialized : INT := MutableValue;
END_VAR
Read := Initialized;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    )]);

    let lower = error.to_ascii_lowercase();
    assert!(
        lower.contains("constant") && lower.contains("mutablevalue"),
        "{error}"
    );
}

#[test]
fn pou_initializer_dependency_rejects_undefined_operand_with_source_label() {
    let error = compile_error(&[
        (
            "function.st",
            r#"
FUNCTION Read : INT
VAR_INPUT
    Value : INT := MissingValue;
END_VAR
Read := Value;
END_FUNCTION
"#,
        ),
        (
            "program.st",
            r#"
PROGRAM Main
END_PROGRAM
"#,
        ),
    ]);

    let lower = error.to_ascii_lowercase();
    assert!(error.contains("function.st:"), "{error}");
    assert!(
        lower.contains("missingvalue")
            && (lower.contains("undefined")
                || lower.contains("unknown")
                || lower.contains("cannot resolve")),
        "{error}"
    );
}

#[test]
fn pou_initializer_dependency_rejects_local_constant_cycle() {
    let error = compile_error(&[(
        "cycle.st",
        r#"
FUNCTION Read : INT
VAR_INPUT
    Value : INT := First;
END_VAR
VAR CONSTANT
    First : INT := Second + INT#1;
    Second : INT := First - INT#1;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(error.to_ascii_lowercase().contains("cyclic"), "{error}");
}

#[test]
fn pou_initializer_dependency_rejects_divide_by_zero() {
    let error = compile_error(&[(
        "divide_by_zero.st",
        r#"
FUNCTION Read : INT
VAR_OUTPUT
    Value : INT := INT#8 / Zero;
END_VAR
VAR CONSTANT
    Zero : INT := INT#0;
END_VAR
Read := INT#0;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    )]);

    let lower = error.to_ascii_lowercase();
    assert!(
        lower.contains("divides by zero") || lower.contains("division by zero"),
        "{error}"
    );
}

#[test]
fn pou_initializer_dependency_rejects_integer_overflow() {
    let error = compile_error(&[(
        "overflow.st",
        r#"
FUNCTION Read : LINT
VAR
    Value : LINT := Maximum + LINT#1;
END_VAR
VAR CONSTANT
    Maximum : LINT := LINT#9223372036854775807;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    )]);

    assert!(error.to_ascii_lowercase().contains("overflow"), "{error}");
}

#[test]
fn pou_initializer_dependency_rejects_ambiguous_using_operand() {
    let error = compile_error(&[(
        "ambiguous.st",
        r#"
NAMESPACE Left
VAR_GLOBAL CONSTANT
    InitialValue : INT := INT#2;
END_VAR
END_NAMESPACE
NAMESPACE Right
VAR_GLOBAL CONSTANT
    InitialValue : INT := INT#4;
END_VAR
END_NAMESPACE
FUNCTION Read : INT
USING Left, Right;
VAR_INPUT
    Value : INT := InitialValue;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    )]);

    let lower = error.to_ascii_lowercase();
    assert!(
        lower.contains("ambiguous") && lower.contains("initialvalue"),
        "{error}"
    );
}

#[test]
fn pou_initializer_dependency_does_not_leak_unimported_namespace_operand() {
    let error = compile_error(&[(
        "missing_import.st",
        r#"
NAMESPACE Limits
VAR_GLOBAL CONSTANT
    InitialValue : INT := INT#4;
END_VAR
END_NAMESPACE
FUNCTION Read : INT
VAR_INPUT
    Value : INT := InitialValue;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
END_PROGRAM
"#,
    )]);

    let lower = error.to_ascii_lowercase();
    assert!(
        lower.contains("initialvalue")
            && (lower.contains("undefined")
                || lower.contains("unknown")
                || lower.contains("cannot resolve")),
        "{error}"
    );
}
