use crate::harness::TestHarness;
use crate::value::Value;

fn harness(source: &str) -> TestHarness {
    TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("POU initializer lifetime fixture must compile: {error}"))
}

fn run(source: &str) -> TestHarness {
    let mut harness = harness(source);
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn cycle(harness: &mut TestHarness) {
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
}

#[test]
fn pou_initializer_lifetime_function_var_reinitializes_each_call() {
    let harness = run(
        r#"
FUNCTION Next : INT
VAR
    Value : INT := INT#5;
END_VAR
Value := Value + INT#1;
Next := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
First := Next();
Second := Next();
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("First"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(6)));
}

#[test]
fn pou_initializer_lifetime_function_temp_reinitializes_each_call() {
    let harness = run(
        r#"
FUNCTION Next : INT
VAR_TEMP
    Value : INT := INT#7;
END_VAR
Value := Value + INT#1;
Next := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
First := Next();
Second := Next();
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("First"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(8)));
}

#[test]
fn pou_initializer_lifetime_function_output_reinitializes_each_call() {
    let harness = run(
        r#"
FUNCTION Produce : INT
VAR_OUTPUT
    Value : INT := INT#5;
END_VAR
Value := Value + INT#1;
Produce := INT#0;
END_FUNCTION
PROGRAM Main
VAR
    Ignored : INT;
END_VAR
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
Ignored := Produce(Value => First);
Ignored := Produce(Value => Second);
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("First"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(6)));
}

#[test]
fn pou_initializer_lifetime_function_static_initializes_once_and_persists() {
    let mut harness = run(
        r#"
FUNCTION Next : INT
VAR_STAT
    Value : INT := INT#10;
END_VAR
Value := Value + INT#1;
Next := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
First := Next();
Second := Next();
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("First"), Some(Value::Int(11)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(12)));
    cycle(&mut harness);
    assert_eq!(harness.get_output("First"), Some(Value::Int(13)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(14)));
}

#[test]
fn pou_initializer_lifetime_supplied_function_input_overrides_default_per_call() {
    let harness = run(
        r#"
FUNCTION Read : INT
VAR_INPUT
    Value : INT := INT#7;
END_VAR
Read := Value;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    Supplied : INT;
    Omitted : INT;
END_VAR
Supplied := Read(Value := INT#3);
Omitted := Read();
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("Supplied"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("Omitted"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_lifetime_method_automatic_locals_reinitialize_each_call() {
    let harness = run(
        r#"
CLASS Calculator
METHOD PUBLIC Next : INT
VAR
    LocalValue : INT := INT#2;
END_VAR
VAR_TEMP
    TempValue : INT := INT#3;
END_VAR
LocalValue := LocalValue + INT#1;
TempValue := TempValue + INT#1;
Next := LocalValue + TempValue;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    Instance : Calculator;
END_VAR
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
First := Instance.Next();
Second := Instance.Next();
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("First"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_lifetime_method_static_initializes_once_per_receiver() {
    let harness = run(
        r#"
CLASS Counter
METHOD PUBLIC Next : INT
VAR_STAT
    Value : INT := INT#10;
END_VAR
Value := Value + INT#1;
Next := Value;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    FirstCounter : Counter;
    SecondCounter : Counter;
END_VAR
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
FirstCounter.Next();
First := FirstCounter.Next();
Second := SecondCounter.Next();
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("First"), Some(Value::Int(12)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(11)));
}

#[test]
fn pou_initializer_lifetime_function_block_state_persists_and_temp_reinitializes() {
    let harness = run(
        r#"
FUNCTION_BLOCK Accumulate
VAR_OUTPUT
    Total : INT := INT#1;
END_VAR
VAR
    Stored : INT := INT#10;
END_VAR
VAR_TEMP
    Scratch : INT := INT#2;
END_VAR
Scratch := Scratch + INT#1;
Stored := Stored + Scratch;
Total := Total + Stored;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    Instance : Accumulate;
END_VAR
VAR_OUTPUT
    Observed : INT;
END_VAR
Instance();
Instance();
Observed := Instance.Total;
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(30)));
}

#[test]
fn pou_initializer_lifetime_function_block_instances_initialize_independently() {
    let harness = run(
        r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT
    Value : INT := INT#5;
END_VAR
Value := Value + INT#1;
END_FUNCTION_BLOCK
PROGRAM Main
VAR
    FirstCounter : Counter;
    SecondCounter : Counter;
END_VAR
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
FirstCounter();
FirstCounter();
SecondCounter();
First := FirstCounter.Value;
Second := SecondCounter.Value;
END_PROGRAM
"#,
    );

    assert_eq!(harness.get_output("First"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(6)));
}

#[test]
fn pou_initializer_lifetime_program_state_persists_and_temp_reinitializes_each_cycle() {
    let mut harness = harness(
        r#"
PROGRAM Main
VAR_OUTPUT
    Observed : INT := INT#1;
END_VAR
VAR
    Stored : INT := INT#10;
END_VAR
VAR_TEMP
    Scratch : INT := INT#2;
END_VAR
Scratch := Scratch + INT#1;
Stored := Stored + Scratch;
Observed := Observed + Stored;
END_PROGRAM
"#,
    );

    cycle(&mut harness);
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(14)));
    cycle(&mut harness);
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(30)));
}

#[test]
fn pou_initializer_lifetime_multi_name_program_storage_is_independent() {
    let mut harness = harness(
        r#"
PROGRAM Main
VAR
    First, Second : INT := INT#5;
END_VAR
VAR_OUTPUT
    Observed : INT;
END_VAR
First := First + INT#1;
Observed := First * INT#10 + Second;
END_PROGRAM
"#,
    );

    cycle(&mut harness);
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(65)));
    cycle(&mut harness);
    assert_eq!(harness.get_output("Observed"), Some(Value::Int(75)));
}
