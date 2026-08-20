use crate::harness::TestHarness;
use crate::value::Value;

fn run(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source)
        .unwrap_or_else(|error| panic!("call-binding fixture must compile: {error}"));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

#[test]
fn pou_call_runtime_evaluates_named_actuals_left_to_right_in_source_order() {
    let harness = run(r#"
VAR_GLOBAL trace : INT; END_VAR
FUNCTION Next : INT
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + 1;
Next := trace;
END_FUNCTION
FUNCTION Encode : INT
VAR_INPUT first : INT; second : INT; END_VAR
Encode := first * 10 + second;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Encode(second := Next(), first := Next());
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(21)));
    assert_eq!(
        harness.runtime().storage().get_global("trace"),
        Some(&Value::Int(2))
    );
}

#[test]
fn pou_call_runtime_evaluates_each_actual_exactly_once() {
    let harness = run(r#"
VAR_GLOBAL trace : INT; END_VAR
FUNCTION Next : INT
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + 1;
Next := trace;
END_FUNCTION
FUNCTION Sum : INT
VAR_INPUT left : INT; right : INT; END_VAR
Sum := left + right;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Sum(left := Next(), right := Next());
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(3)));
    assert_eq!(
        harness.runtime().storage().get_global("trace"),
        Some(&Value::Int(2))
    );
}

#[test]
fn pou_call_runtime_evaluates_hybrid_prefix_and_suffix_in_source_order() {
    let harness = run(r#"
VAR_GLOBAL trace : INT; END_VAR
FUNCTION Next : INT
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + 1;
Next := trace;
END_FUNCTION
FUNCTION Encode : INT
VAR_INPUT first : INT; second : INT; third : INT; END_VAR
Encode := first * 100 + second * 10 + third;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Encode(Next(), third := Next(), second := Next());
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(132)));
}

#[test]
fn pou_call_runtime_copies_output_and_in_out_after_normal_return() {
    let harness = run(r#"
FUNCTION Transfer : INT
VAR_INPUT source : INT; END_VAR
VAR_OUTPUT copied : INT; END_VAR
VAR_IN_OUT target : INT; END_VAR
copied := source + 1;
target := target + source;
Transfer := target;
END_FUNCTION
PROGRAM Main
VAR copied : INT; target : INT := 5; result : INT; END_VAR
result := Transfer(source := 3, copied => copied, target := target);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("copied"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("target"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("result"), Some(Value::Int(8)));
}

#[test]
fn pou_call_runtime_maps_complete_positional_inputs_outputs_and_in_outs() {
    let harness = run(r#"
FUNCTION Transfer : INT
VAR_INPUT source : INT; END_VAR
VAR_OUTPUT copied : INT; END_VAR
VAR_IN_OUT target : INT; END_VAR
copied := source + 1;
target := target + source;
Transfer := target;
END_FUNCTION
PROGRAM Main
VAR copied : INT; target : INT := 5; result : INT; END_VAR
result := Transfer(3, copied, target);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("copied"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("target"), Some(Value::Int(8)));
}

#[test]
fn pou_call_runtime_uses_function_input_initializer_on_every_omitted_call() {
    let harness = run(r#"
FUNCTION Defaulted : INT
VAR_INPUT value : INT := INT#7; END_VAR
Defaulted := value + INT#1;
END_FUNCTION
PROGRAM Main
VAR first : INT; second : INT; END_VAR
first := Defaulted();
second := Defaulted();
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(8)));
}

#[test]
fn pou_call_runtime_reinitializes_function_output_for_every_call() {
    let harness = run(r#"
FUNCTION OutputDefault : INT
VAR_INPUT write : BOOL; END_VAR
VAR_OUTPUT value : INT := INT#4; END_VAR
IF write THEN value := INT#9; END_IF;
OutputDefault := value;
END_FUNCTION
PROGRAM Main
VAR first : INT; second : INT; output : INT; END_VAR
first := OutputDefault(write := TRUE, value => output);
second := OutputDefault(write := FALSE, value => output);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(9)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("output"), Some(Value::Int(4)));
}

#[test]
fn pou_call_runtime_reuses_function_block_input_when_omitted() {
    let harness = run(r#"
FUNCTION_BLOCK Accumulator
VAR_INPUT amount : INT := 1; END_VAR
VAR_OUTPUT total : INT; END_VAR
total := total + amount;
END_FUNCTION_BLOCK
PROGRAM Main
VAR fb : Accumulator; first : INT; second : INT; END_VAR
fb(amount := 3, total => first);
fb(total => second);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("first"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(6)));
}

#[test]
fn pou_call_runtime_false_en_skips_body_and_sets_connected_eno_false() {
    let harness = run(r#"
VAR_GLOBAL trace : INT; END_VAR
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; END_VAR
VAR_OUTPUT ENO : BOOL; END_VAR
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + 1;
Controlled := trace;
END_FUNCTION
PROGRAM Main
VAR ok : BOOL := TRUE; result : INT := 9; END_VAR
result := Controlled(EN := FALSE, ENO => ok);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(0)));
    assert_eq!(harness.get_output("ok"), Some(Value::Bool(false)));
    assert_eq!(
        harness.runtime().storage().get_global("trace"),
        Some(&Value::Int(0))
    );
}

#[test]
fn pou_call_runtime_false_en_suppresses_non_en_actual_declared_before_en() {
    let harness = run(r#"
VAR_GLOBAL trace : INT; END_VAR
FUNCTION Next : INT
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + 1;
Next := trace;
END_FUNCTION
FUNCTION Controlled : INT
VAR_INPUT value : INT; EN : BOOL; END_VAR
Controlled := value;
END_FUNCTION
PROGRAM Main
VAR result : INT; END_VAR
result := Controlled(value := Next(), EN := FALSE);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(0)));
    assert_eq!(
        harness.runtime().storage().get_global("trace"),
        Some(&Value::Int(0))
    );
}

#[test]
fn pou_call_runtime_false_en_preserves_ordinary_output_target() {
    let harness = run(r#"
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; END_VAR
VAR_OUTPUT value : INT; ENO : BOOL; END_VAR
value := 9; Controlled := value;
END_FUNCTION
PROGRAM Main
VAR output : INT := 5; ok : BOOL := TRUE; result : INT; END_VAR
result := Controlled(EN := FALSE, value => output, ENO => ok);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("output"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("ok"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("result"), Some(Value::Int(0)));
}

#[test]
fn pou_call_runtime_true_en_initializes_eno_true_before_body() {
    let harness = run(r#"
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; END_VAR
VAR_OUTPUT ENO : BOOL; END_VAR
IF ENO THEN Controlled := 1; ELSE Controlled := 2; END_IF;
END_FUNCTION
PROGRAM Main
VAR ok : BOOL; result : INT; END_VAR
result := Controlled(EN := TRUE, ENO => ok);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("ok"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("result"), Some(Value::Int(1)));
}

#[test]
fn pou_call_runtime_body_may_set_eno_false_and_still_transfer_outputs() {
    let harness = run(r#"
FUNCTION Controlled : INT
VAR_INPUT EN : BOOL; END_VAR
VAR_OUTPUT value : INT; ENO : BOOL; END_VAR
value := 7; ENO := FALSE; Controlled := 8;
END_FUNCTION
PROGRAM Main
VAR output : INT; ok : BOOL := TRUE; result : INT; END_VAR
result := Controlled(EN := TRUE, value => output, ENO => ok);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("output"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("ok"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("result"), Some(Value::Int(8)));
}

#[test]
fn pou_call_runtime_input_snapshot_is_stable_when_output_reuses_storage() {
    let harness = run(r#"
FUNCTION Copy : INT
VAR_INPUT source : INT; END_VAR
VAR_OUTPUT target : INT; END_VAR
target := source + 1;
Copy := source;
END_FUNCTION
PROGRAM Main
VAR value : INT := 5; result : INT; END_VAR
result := Copy(source := value, target => value);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("value"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("result"), Some(Value::Int(5)));
}

#[test]
fn pou_call_runtime_transfers_distinct_array_element_outputs() {
    let harness = run(r#"
FUNCTION Pair : INT
VAR_OUTPUT left : INT; right : INT; END_VAR
left := 4; right := 5; Pair := 0;
END_FUNCTION
PROGRAM Main
VAR values : ARRAY[0..1] OF INT; END_VAR
Pair(left => values[0], right => values[1]);
END_PROGRAM
"#);
    let Some(Value::Array(values)) = harness.get_output("values") else {
        panic!("values must remain an array");
    };
    assert_eq!(values.elements(), &[Value::Int(4), Value::Int(5)]);
}

#[test]
fn pou_call_runtime_evaluates_method_actuals_left_to_right() {
    let harness = run(r#"
VAR_GLOBAL trace : INT; END_VAR
FUNCTION Next : INT
VAR_EXTERNAL trace : INT; END_VAR
trace := trace + 1; Next := trace;
END_FUNCTION
CLASS Encoder
METHOD PUBLIC Encode : INT
VAR_INPUT first : INT; second : INT; END_VAR
Encode := first * 10 + second;
END_METHOD
END_CLASS
PROGRAM Main
VAR encoder : Encoder; result : INT; END_VAR
result := encoder.Encode(second := Next(), first := Next());
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(21)));
}

#[test]
fn pou_call_runtime_method_copies_output_and_in_out_after_return() {
    let harness = run(r#"
CLASS Worker
METHOD PUBLIC Transfer : INT
VAR_INPUT source : INT; END_VAR
VAR_OUTPUT copied : INT; END_VAR
VAR_IN_OUT target : INT; END_VAR
copied := source + 1; target := target + source; Transfer := target;
END_METHOD
END_CLASS
PROGRAM Main
VAR worker : Worker; copied : INT; target : INT := 5; result : INT; END_VAR
result := worker.Transfer(source := 3, copied => copied, target := target);
END_PROGRAM
"#);
    assert_eq!(harness.get_output("copied"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("target"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("result"), Some(Value::Int(8)));
}

#[test]
fn pou_call_runtime_error_does_not_copy_function_output() {
    let mut harness = TestHarness::from_source(
        r#"
FUNCTION Fail : INT
VAR_OUTPUT output : INT; END_VAR
output := 9;
Fail := 1 / 0;
END_FUNCTION
PROGRAM Main
VAR target : INT := 5; result : INT; END_VAR
result := Fail(output => target);
END_PROGRAM
"#,
    )
    .expect("runtime-error fixture must compile");
    let cycle = harness.cycle();
    assert!(!cycle.errors.is_empty(), "division by zero must fail");
    assert_eq!(harness.get_output("target"), Some(Value::Int(5)));
}

#[test]
fn pou_call_runtime_error_does_not_copy_function_in_out() {
    let mut harness = TestHarness::from_source(
        r#"
FUNCTION Fail : INT
VAR_IN_OUT target : INT; END_VAR
target := 9;
Fail := 1 / 0;
END_FUNCTION
PROGRAM Main
VAR target : INT := 5; result : INT; END_VAR
result := Fail(target := target);
END_PROGRAM
"#,
    )
    .expect("runtime-error fixture must compile");
    let cycle = harness.cycle();
    assert!(!cycle.errors.is_empty(), "division by zero must fail");
    assert_eq!(harness.get_output("target"), Some(Value::Int(5)));
}

#[test]
fn pou_call_runtime_function_block_omitted_output_keeps_instance_output_accessible() {
    let harness = run(r#"
FUNCTION_BLOCK Counter
VAR_OUTPUT value : INT; END_VAR
value := value + 1;
END_FUNCTION_BLOCK
PROGRAM Main
VAR counter : Counter; result : INT; END_VAR
counter();
result := counter.value;
END_PROGRAM
"#);
    assert_eq!(harness.get_output("result"), Some(Value::Int(1)));
}
