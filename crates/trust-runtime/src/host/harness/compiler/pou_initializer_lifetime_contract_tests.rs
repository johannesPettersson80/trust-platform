use crate::harness::TestHarness;
use crate::value::{ArrayValue, Value};

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

fn assert_struct_output_fields(harness: &TestHarness, output: &str, expected: &[(&str, Value)]) {
    let Some(Value::Struct(value)) = harness.get_output(output) else {
        panic!("expected STRUCT output {output}");
    };
    for (field, expected_value) in expected {
        assert_eq!(
            value.field(field),
            Some(expected_value),
            "unexpected {output}.{field}"
        );
    }
}

#[test]
fn pou_initializer_lifetime_function_var_reinitializes_each_call() {
    let harness = run(r#"
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
"#);

    assert_eq!(harness.get_output("First"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(6)));
}

#[test]
fn pou_initializer_lifetime_function_temp_reinitializes_each_call() {
    let harness = run(r#"
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
"#);

    assert_eq!(harness.get_output("First"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(8)));
}

#[test]
fn pou_initializer_lifetime_function_output_reinitializes_each_call() {
    let harness = run(r#"
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
"#);

    assert_eq!(harness.get_output("First"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(6)));
}

#[test]
fn pou_initializer_lifetime_function_static_initializes_once_and_persists() {
    let mut harness = run(r#"
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
"#);

    assert_eq!(harness.get_output("First"), Some(Value::Int(11)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(12)));
    cycle(&mut harness);
    assert_eq!(harness.get_output("First"), Some(Value::Int(13)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(14)));
}

#[test]
fn pou_initializer_lifetime_supplied_function_input_overrides_default_per_call() {
    let harness = run(r#"
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
"#);

    assert_eq!(harness.get_output("Supplied"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("Omitted"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_lifetime_function_aggregate_input_uses_recursive_type_default() {
    let harness = run(r#"
TYPE Pair : STRUCT
    Left : INT := INT#7;
    Right : INT := INT#9;
END_STRUCT
END_TYPE
FUNCTION ReadDefault : INT
VAR_INPUT
    Value : Pair;
END_VAR
ReadDefault := Value.Left + Value.Right;
END_FUNCTION
PROGRAM Main
VAR_OUTPUT
    Result : INT;
END_VAR
Result := ReadDefault();
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Result"), Some(Value::Int(16)));
}

#[test]
fn pou_initializer_lifetime_function_aggregate_output_reinitializes_each_call() {
    let harness = run(r#"
TYPE Pair : STRUCT
    Left : INT := INT#5;
    Right : INT := INT#7;
    Label : STRING := 'implicit';
    Samples : ARRAY[1..2] OF INT := [INT#3, INT#4];
END_STRUCT
END_TYPE
FUNCTION Produce : INT
VAR_OUTPUT
    Value : Pair;
END_VAR
Value.Left := Value.Left + INT#1;
Produce := INT#0;
END_FUNCTION
PROGRAM Main
VAR
    Ignored : INT;
END_VAR
VAR_OUTPUT
    First : Pair;
    Second : Pair;
END_VAR
Ignored := Produce(Value => First);
Ignored := Produce(Value => Second);
END_PROGRAM
"#);

    assert_struct_output_fields(
        &harness,
        "First",
        &[
            ("Left", Value::Int(6)),
            ("Right", Value::Int(7)),
            ("Label", Value::String("implicit".into())),
            (
                "Samples",
                Value::Array(Box::new(ArrayValue::from_canonical_parts(
                    vec![Value::Int(3), Value::Int(4)],
                    vec![(1, 2)],
                ))),
            ),
        ],
    );
    assert_struct_output_fields(
        &harness,
        "Second",
        &[
            ("Left", Value::Int(6)),
            ("Right", Value::Int(7)),
            ("Label", Value::String("implicit".into())),
            (
                "Samples",
                Value::Array(Box::new(ArrayValue::from_canonical_parts(
                    vec![Value::Int(3), Value::Int(4)],
                    vec![(1, 2)],
                ))),
            ),
        ],
    );
}

#[test]
fn pou_initializer_lifetime_function_array_output_reinitializes_each_call() {
    let harness = run(r#"
FUNCTION Produce : INT
VAR_OUTPUT
    Values : ARRAY[1..2] OF INT;
END_VAR
Values[1] := Values[1] + INT#1;
Produce := INT#0;
END_FUNCTION
PROGRAM Main
VAR
    Ignored : INT;
END_VAR
VAR_OUTPUT
    First : ARRAY[1..2] OF INT;
    Second : ARRAY[1..2] OF INT;
END_VAR
Ignored := Produce(Values => First);
Ignored := Produce(Values => Second);
END_PROGRAM
"#);

    let expected = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(1), Value::Int(0)],
        vec![(1, 2)],
    )));
    assert_eq!(harness.get_output("First"), Some(expected.clone()));
    assert_eq!(harness.get_output("Second"), Some(expected));
}

#[test]
fn pou_initializer_lifetime_function_union_output_reinitializes_each_call() {
    let harness = run(r#"
TYPE Choice : UNION
    Count : INT := INT#7;
    Ready : BOOL := TRUE;
END_UNION
END_TYPE
FUNCTION Produce : INT
VAR_OUTPUT
    Value : Choice;
END_VAR
Value.Count := Value.Count + INT#1;
Produce := INT#0;
END_FUNCTION
PROGRAM Main
VAR
    Ignored : INT;
END_VAR
VAR_OUTPUT
    First : Choice;
    Second : Choice;
END_VAR
Ignored := Produce(Value => First);
Ignored := Produce(Value => Second);
END_PROGRAM
"#);

    for output in ["First", "Second"] {
        assert_struct_output_fields(
            &harness,
            output,
            &[("Count", Value::Int(8)), ("Ready", Value::Bool(true))],
        );
    }
}

#[test]
fn pou_initializer_lifetime_function_wildcard_array_input_accepts_supplied_shape() {
    let harness = run(r#"
FUNCTION ReadFirst : INT
VAR_INPUT
    Values : ARRAY[*] OF INT;
END_VAR
ReadFirst := Values[1];
END_FUNCTION
PROGRAM Main
VAR
    Values : ARRAY[1..2] OF INT := [INT#7, INT#9];
END_VAR
VAR_OUTPUT
    Result : INT;
END_VAR
Result := ReadFirst(Values := Values);
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Result"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_lifetime_function_reference_parameters_default_to_null_each_call() {
    let harness = run(r#"
FUNCTION Inspect : BOOL
VAR_INPUT
    Source : REF_TO INT;
END_VAR
VAR_OUTPUT
    Result : REF_TO INT;
END_VAR
Inspect := (Source = NULL) AND (Result = NULL);
END_FUNCTION
PROGRAM Main
VAR
    Ignored : BOOL;
    OutputRef : REF_TO INT;
END_VAR
VAR_OUTPUT
    WasNull : BOOL;
END_VAR
Ignored := Inspect(Result => OutputRef);
WasNull := OutputRef = NULL;
END_PROGRAM
"#);

    assert_eq!(harness.get_output("WasNull"), Some(Value::Bool(true)));
}

#[test]
fn pou_initializer_lifetime_method_accepts_supplied_function_block_input() {
    let harness = run(r#"
FUNCTION_BLOCK Source
VAR_INPUT
    Value : INT;
END_VAR
END_FUNCTION_BLOCK
CLASS Reader
METHOD PUBLIC Read : INT
VAR_INPUT
    Input : Source;
END_VAR
Read := Input.Value;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    Input : Source := (Value := INT#7);
    Instance : Reader;
END_VAR
VAR_OUTPUT
    Result : INT;
END_VAR
Result := Instance.Read(Input := Input);
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Result"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_lifetime_method_accepts_supplied_interface_input() {
    let harness = run(r#"
INTERFACE ISource
METHOD Read : INT
END_METHOD
END_INTERFACE
CLASS Source IMPLEMENTS ISource
METHOD PUBLIC Read : INT
Read := INT#9;
END_METHOD
END_CLASS
CLASS Consumer
METHOD PUBLIC Apply : INT
VAR_INPUT
    Input : ISource;
END_VAR
Apply := Input.Read();
END_METHOD
END_CLASS
PROGRAM Main
VAR
    SourceInstance : Source;
    Contract : ISource;
    ConsumerInstance : Consumer;
END_VAR
VAR_OUTPUT
    Result : INT;
END_VAR
Contract := SourceInstance;
Result := ConsumerInstance.Apply(Input := Contract);
END_PROGRAM
"#);

    assert_eq!(harness.get_output("Result"), Some(Value::Int(9)));
}

#[test]
fn pou_initializer_lifetime_method_automatic_locals_reinitialize_each_call() {
    let harness = run(r#"
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
"#);

    assert_eq!(harness.get_output("First"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(7)));
}

#[test]
fn pou_initializer_lifetime_method_output_reinitializes_each_call() {
    let harness = run(r#"
CLASS Producer
METHOD PUBLIC Produce : INT
VAR_OUTPUT
    Value : INT := INT#5;
END_VAR
Value := Value + INT#1;
Produce := INT#0;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    Instance : Producer;
    Ignored : INT;
END_VAR
VAR_OUTPUT
    First : INT;
    Second : INT;
END_VAR
Ignored := Instance.Produce(Value => First);
Ignored := Instance.Produce(Value => Second);
END_PROGRAM
"#);

    assert_eq!(harness.get_output("First"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(6)));
}

#[test]
fn pou_initializer_lifetime_method_aggregate_parameters_use_explicit_defaults() {
    let harness = run(r#"
TYPE Pair : STRUCT
    Left : INT;
    Right : INT;
    Label : STRING;
END_STRUCT
END_TYPE
CLASS Worker
METHOD PUBLIC Apply : INT
VAR_INPUT
    Source : Pair := Pair(Left := INT#3, Right := INT#4, Label := 'source');
END_VAR
VAR_OUTPUT
    Result : Pair := Pair(Left := INT#5, Right := INT#6, Label := 'result');
END_VAR
Result.Left := Result.Left + Source.Left;
Apply := Source.Right;
END_METHOD
END_CLASS
PROGRAM Main
VAR
    Instance : Worker;
END_VAR
VAR_OUTPUT
    FirstReturn : INT;
    SecondReturn : INT;
    First : Pair;
    Second : Pair;
END_VAR
FirstReturn := Instance.Apply(Result => First);
SecondReturn := Instance.Apply(Result => Second);
END_PROGRAM
"#);

    assert_eq!(harness.get_output("FirstReturn"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("SecondReturn"), Some(Value::Int(4)));
    assert_struct_output_fields(
        &harness,
        "First",
        &[
            ("Left", Value::Int(8)),
            ("Right", Value::Int(6)),
            ("Label", Value::String("result".into())),
        ],
    );
    assert_struct_output_fields(
        &harness,
        "Second",
        &[
            ("Left", Value::Int(8)),
            ("Right", Value::Int(6)),
            ("Label", Value::String("result".into())),
        ],
    );
}

#[test]
fn pou_initializer_lifetime_method_static_initializes_once_per_receiver() {
    let harness = run(r#"
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
"#);

    assert_eq!(harness.get_output("First"), Some(Value::Int(12)));
    assert_eq!(harness.get_output("Second"), Some(Value::Int(11)));
}

#[test]
fn pou_initializer_lifetime_function_block_state_persists_and_temp_reinitializes() {
    let harness = run(r#"
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
"#);

    assert_eq!(harness.get_output("Observed"), Some(Value::Int(30)));
}

#[test]
fn pou_initializer_lifetime_function_block_instances_initialize_independently() {
    let harness = run(r#"
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
"#);

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
