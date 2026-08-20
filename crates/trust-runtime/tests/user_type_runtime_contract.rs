use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::value::Value;

fn run(source: &str) -> TestHarness {
    let mut harness =
        TestHarness::from_source(source).unwrap_or_else(|error| panic!("fixture: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn array_elements(harness: &TestHarness, name: &str) -> Vec<Value> {
    let Some(Value::Array(values)) = harness.get_output(name) else {
        panic!("{name} must be an array");
    };
    values.elements().to_vec()
}

#[test]
fn user_type_runtime_ordinary_enum_defaults_to_first_literal() {
    let harness = run(r#"
TYPE Color : (Red, Green, Blue); END_TYPE
PROGRAM Main
VAR color : Color; isRed : BOOL; END_VAR
isRed := color = Color#Red;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("isRed"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_ordinary_enum_uses_explicit_type_default() {
    let harness = run(r#"
TYPE Color : (Red, Green, Blue) := Blue; END_TYPE
PROGRAM Main
VAR color : Color; isBlue : BOOL; END_VAR
isBlue := color = Color#Blue;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("isBlue"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_enum_assignment_preserves_declared_identity() {
    let harness = run(r#"
TYPE Color : (Red, Green, Blue); END_TYPE
PROGRAM Main
VAR color : Color; isGreen : BOOL; END_VAR
color := Color#Green;
isGreen := color = Color#Green;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("isGreen"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_named_values_retain_integer_base_range() {
    let harness = run(r#"
TYPE Signal : INT (Low := 1, High := 2); END_TYPE
PROGRAM Main
VAR signal : Signal; result : INT; END_VAR
signal := 27;
result := signal + Signal#High;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(29)));
}

#[test]
fn user_type_runtime_subrange_without_initializer_defaults_to_lower_bound() {
    let harness = run(r#"
TYPE Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR value : Limited; observed : INT; END_VAR
observed := value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(4)));
}

#[test]
fn user_type_runtime_subrange_uses_explicit_type_default() {
    let harness = run(r#"
TYPE Limited : INT (4..6) := 5; END_TYPE
PROGRAM Main
VAR value : Limited; observed : INT; END_VAR
observed := value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(5)));
}

#[test]
fn user_type_runtime_rejects_above_subrange_without_mutation() {
    let mut harness = TestHarness::from_source(
        r#"
TYPE Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR source : INT := 7; value : Limited := 5; END_VAR
value := source;
END_PROGRAM
"#,
    )
    .expect("dynamic subrange fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(harness.get_output("value"), Some(Value::Int(5)));
}

#[test]
fn user_type_runtime_rejects_below_subrange_without_mutation() {
    let mut harness = TestHarness::from_source(
        r#"
TYPE Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR source : INT := 3; value : Limited := 5; END_VAR
value := source;
END_PROGRAM
"#,
    )
    .expect("dynamic subrange fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(harness.get_output("value"), Some(Value::Int(5)));
}

#[test]
fn user_type_runtime_rejects_out_of_range_parameter_copy_in_before_body() {
    let mut harness = TestHarness::from_source(
        r#"
TYPE Limited : INT (4..6); END_TYPE
VAR_GLOBAL trace : INT; END_VAR
FUNCTION Read : INT
VAR_INPUT value : Limited; END_VAR
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + 1; Read := value;
END_FUNCTION
PROGRAM Main
VAR source : INT := 7; result : INT := 9; END_VAR
result := Read(value := source);
END_PROGRAM
"#,
    )
    .expect("dynamic parameter fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(harness.get_output("result"), Some(Value::Int(9)));
    assert_eq!(
        harness.runtime().storage().get_global("trace"),
        Some(&Value::Int(0))
    );
}

#[test]
fn user_type_runtime_partial_array_initializer_default_fills_rightmost_values() {
    let harness =
        run("PROGRAM Main\nVAR values : ARRAY[1..4] OF INT := [1, 2]; END_VAR\nEND_PROGRAM");
    assert_eq!(
        array_elements(&harness, "values"),
        [Value::Int(1), Value::Int(2), Value::Int(0), Value::Int(0)]
    );
}

#[test]
fn user_type_runtime_excess_array_initializer_ignores_rightmost_values() {
    let harness =
        run("PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [1, 2, 3, 4]; END_VAR\nEND_PROGRAM");
    assert_eq!(
        array_elements(&harness, "values"),
        [Value::Int(1), Value::Int(2)]
    );
}

#[test]
fn user_type_runtime_zero_repetition_contributes_no_elements() {
    let harness =
        run("PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [0(9), 1, 2]; END_VAR\nEND_PROGRAM");
    assert_eq!(
        array_elements(&harness, "values"),
        [Value::Int(1), Value::Int(2)]
    );
}

#[test]
fn user_type_runtime_nested_repetition_expands_in_source_order() {
    let harness =
        run("PROGRAM Main\nVAR values : ARRAY[1..8] OF INT := [2(2(1, 2))]; END_VAR\nEND_PROGRAM");
    assert_eq!(
        array_elements(&harness, "values"),
        [
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(2),
            Value::Int(1),
            Value::Int(2),
        ]
    );
}

#[test]
fn user_type_runtime_multidimensional_initializer_varies_rightmost_index_fastest() {
    let harness = run(r#"
PROGRAM Main
VAR
    values : ARRAY[1..2, 10..11] OF INT := [1, 2, 3, 4];
    a : INT; b : INT; c : INT; d : INT;
END_VAR
a := values[1, 10];
b := values[1, 11];
c := values[2, 10];
d := values[2, 11];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("a"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("b"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("c"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("d"), Some(Value::Int(4)));
}

#[test]
fn user_type_runtime_whole_array_assignment_is_an_independent_copy() {
    let harness = run(r#"
PROGRAM Main
VAR source : ARRAY[1..2] OF INT := [1, 2]; target : ARRAY[1..2] OF INT; observed : INT; END_VAR
target := source;
source[1] := 9;
observed := target[1];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(1)));
}

#[test]
fn user_type_runtime_whole_structure_assignment_is_an_independent_copy() {
    let harness = run(r#"
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE
PROGRAM Main
VAR source : Point := (x := 1, y := 2); target : Point; observed : INT; END_VAR
target := source;
source.x := 9;
observed := target.x;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(1)));
}

#[test]
fn user_type_runtime_failed_computed_array_read_preserves_destination() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR values : ARRAY[1..2] OF INT := [1, 2]; index : INT := 3; result : INT := 9; END_VAR
result := values[index];
END_PROGRAM
"#,
    )
    .expect("computed-index fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(harness.get_output("result"), Some(Value::Int(9)));
}

#[test]
fn user_type_runtime_failed_computed_array_write_preserves_every_element() {
    let mut harness = TestHarness::from_source(
        r#"
PROGRAM Main
VAR values : ARRAY[1..2] OF INT := [1, 2]; index : INT := 3; END_VAR
values[index] := 9;
END_PROGRAM
"#,
    )
    .expect("computed-index fixture must compile");
    let cycle = harness.cycle();
    assert_eq!(cycle.errors.len(), 1, "{:?}", cycle.errors);
    assert_eq!(
        array_elements(&harness, "values"),
        [Value::Int(1), Value::Int(2)]
    );
}

#[test]
fn user_type_runtime_array_of_enum_defaults_each_element_to_first_literal() {
    let harness = run(r#"
TYPE Color : (Red, Green); END_TYPE
PROGRAM Main
VAR values : ARRAY[1..2] OF Color; bothRed : BOOL; END_VAR
bothRed := (values[1] = Color#Red) AND (values[2] = Color#Red);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("bothRed"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_array_of_subrange_defaults_each_element_to_lower_bound() {
    let harness = run(r#"
TYPE Limited : INT (4..6); END_TYPE
PROGRAM Main
VAR values : ARRAY[1..2] OF Limited; sum : INT; END_VAR
sum := values[1] + values[2];
END_PROGRAM
"#);
    assert_eq!(harness.get_output("sum"), Some(Value::Int(8)));
}

#[test]
fn user_type_runtime_structure_initializer_precedence_is_recursive() {
    let harness = run(r#"
TYPE
Inner : STRUCT value : INT := 1; flag : BOOL := TRUE; END_STRUCT;
DefaultInner : Inner := (value := 2);
Outer : STRUCT nested : DefaultInner; other : INT := 3; END_STRUCT;
END_TYPE
PROGRAM Main
VAR value : Outer := (nested := (value := 4)); observed : INT; flag : BOOL; other : INT; END_VAR
observed := value.nested.value;
flag := value.nested.flag;
other := value.other;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("flag"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("other"), Some(Value::Int(3)));
}

#[test]
fn user_type_runtime_union_materializes_every_variant_default() {
    let harness = run(r#"
TYPE Choice : UNION count : INT := 7; ready : BOOL := TRUE; END_UNION; END_TYPE
PROGRAM Main
VAR value : Choice; count : INT; ready : BOOL; END_VAR
count := value.count;
ready := value.ready;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("count"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("ready"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_union_partial_initializer_preserves_omitted_variant_default() {
    let harness = run(r#"
TYPE Choice : UNION count : INT := 1; ready : BOOL := TRUE; END_UNION; END_TYPE
PROGRAM Main
VAR value : Choice := (count := 9); count : INT; ready : BOOL; END_VAR
count := value.count;
ready := value.ready;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("count"), Some(Value::Int(9)));
    assert_eq!(harness.get_output("ready"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_union_variant_write_does_not_alias_other_variants() {
    let harness = run(r#"
TYPE Choice : UNION wide : DWORD := DWORD#16#AABBCCDD; low : WORD := WORD#16#1234; END_UNION; END_TYPE
PROGRAM Main
VAR value : Choice; observedWide : DWORD; observedLow : WORD; END_VAR
value.low := WORD#16#5678;
observedWide := value.wide;
observedLow := value.low;
END_PROGRAM
"#);
    assert_eq!(
        harness.get_output("observedWide"),
        Some(Value::DWord(0xAABBCCDD))
    );
    assert_eq!(harness.get_output("observedLow"), Some(Value::Word(0x5678)));
}

#[test]
fn user_type_runtime_whole_union_assignment_is_an_independent_copy() {
    let harness = run(r#"
TYPE Choice : UNION count : INT; ready : BOOL; END_UNION; END_TYPE
PROGRAM Main
VAR
    source : Choice := (count := 1, ready := TRUE);
    target : Choice;
    observedCount : INT;
    observedReady : BOOL;
END_VAR
target := source;
source.count := 9;
source.ready := FALSE;
observedCount := target.count;
observedReady := target.ready;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observedCount"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("observedReady"), Some(Value::Bool(true)));
}

#[test]
fn user_type_runtime_overlap_fields_share_backing_storage() {
    let harness = run(r#"
TYPE Overlay : STRUCT OVERLAP
    wide AT %B0 : DWORD;
    low AT %B0 : WORD;
END_STRUCT;
END_TYPE
PROGRAM Main
VAR value : Overlay; observed : WORD; observedWide : DWORD; END_VAR
value.wide := DWORD#0;
value.low := WORD#16#1234;
observed := value.low;
observedWide := value.wide;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("observed"), Some(Value::Word(0x1234)));
    assert_eq!(
        harness.get_output("observedWide"),
        Some(Value::DWord(0x1234))
    );
}

#[test]
fn user_type_runtime_invalid_excess_initializer_is_not_hidden() {
    let error = CompileSession::from_source(
        "PROGRAM Main\nVAR values : ARRAY[1..2] OF INT := [1, 2, missing]; END_VAR\nEND_PROGRAM",
    )
    .build_runtime()
    .expect_err("invalid excess initializer expression must reject compilation");
    assert!(
        error.to_string().contains("missing"),
        "primary missing-name diagnostic must remain visible: {error}"
    );
}
