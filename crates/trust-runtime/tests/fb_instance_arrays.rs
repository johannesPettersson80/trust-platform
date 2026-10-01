//! Arrays of function block and class instances: IEC 61131-3 Ed.3 Table 11 feature 5 (FB types and classes as array
//! elements) and 6.6.3.4.1 example e) (FB call using an instance array); docs/specs/02-data-types.md §3.4 and
//! docs/specs/04-pou-declarations.md (Table 41 and Table 42).

use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

const ACCUMULATOR: &str = r#"
FUNCTION_BLOCK Accumulator
VAR_INPUT
    Increment : DINT;
END_VAR
VAR_OUTPUT
    Total : DINT;
END_VAR
Total := Total + Increment;
END_FUNCTION_BLOCK
"#;

fn run(source: &str, cycles: usize) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("compile and initialize");
    for _ in 0..cycles {
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
    harness
}

#[test]
fn every_element_of_an_instance_array_keeps_its_own_state() {
    // A call with a variable subscript in a loop and one with a constant subscript: each element accumulates only its own
    // steps, and its output is read through the subscript.
    let source = format!(
        "{ACCUMULATOR}
PROGRAM Main
VAR
    Sums : ARRAY[0..2] OF Accumulator;
    I : DINT;
    First : DINT;
    Second : DINT;
    Third : DINT;
END_VAR
FOR I := 0 TO 2 DO
    Sums[I](Increment := I + 1);
END_FOR;
Sums[1](Increment := 10);
First := Sums[0].Total;
Second := Sums[1].Total;
Third := Sums[2].Total;
END_PROGRAM
"
    );
    let harness = run(&source, 2);
    assert_eq!(harness.get_output("First"), Some(Value::DInt(2)));
    assert_eq!(harness.get_output("Second"), Some(Value::DInt(24)));
    assert_eq!(harness.get_output("Third"), Some(Value::DInt(6)));
}

#[test]
fn a_separate_input_assignment_takes_effect_with_the_next_call_of_the_element() {
    // Table 42 feature 6a on an element: the input assigned before the call is the one the call uses.
    let source = format!(
        "{ACCUMULATOR}
PROGRAM Main
VAR
    Sums : ARRAY[1..2] OF Accumulator;
    Result : DINT;
    Other : DINT;
END_VAR
Sums[2].Increment := 7;
Sums[2]();
Result := Sums[2].Total;
Other := Sums[1].Total;
END_PROGRAM
"
    );
    let harness = run(&source, 3);
    assert_eq!(harness.get_output("Result"), Some(Value::DInt(21)));
    assert_eq!(harness.get_output("Other"), Some(Value::DInt(0)));
}

#[test]
fn each_element_owns_its_nested_instances() {
    // An element whose type declares an instance of another function block gets its own nested instance.
    let source = format!(
        "{ACCUMULATOR}
FUNCTION_BLOCK Pair
VAR_INPUT
    Increment : DINT;
END_VAR
VAR_OUTPUT
    Total : DINT;
END_VAR
VAR
    Inner : Accumulator;
END_VAR
Inner(Increment := Increment);
Total := Inner.Total;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Pairs : ARRAY[1..2] OF Pair;
    Left : DINT;
    Right : DINT;
END_VAR
Pairs[1](Increment := 1);
Pairs[2](Increment := 5);
Left := Pairs[1].Total;
Right := Pairs[2].Total;
END_PROGRAM
"
    );
    let harness = run(&source, 2);
    assert_eq!(harness.get_output("Left"), Some(Value::DInt(2)));
    assert_eq!(harness.get_output("Right"), Some(Value::DInt(10)));
}

#[test]
fn standard_timer_elements_time_independently() {
    let source = r#"
PROGRAM Main
VAR
    Timers : ARRAY[1..2] OF TON;
    Done1 : BOOL;
    Done2 : BOOL;
END_VAR
Timers[1](IN := TRUE, PT := T#200ms);
Timers[2](IN := FALSE, PT := T#200ms);
Done1 := Timers[1].Q;
Done2 := Timers[2].Q;
END_PROGRAM
"#;
    let mut harness = run(source, 1);
    harness.advance_time(Duration::from_millis(250));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("Done1"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("Done2"), Some(Value::Bool(false)));
}

#[test]
fn a_repeated_element_initializer_initializes_every_instance() {
    // docs/specs/02-data-types.md §3.4: Timers: ARRAY[1..50] OF TON := [50(PT := T#100ms)].
    let source = r#"
PROGRAM Main
VAR
    Timers : ARRAY[1..3] OF TON := [3(PT := T#100ms)];
    I : DINT;
    Preset : TIME;
    AllDone : BOOL;
END_VAR
AllDone := TRUE;
FOR I := 1 TO 3 DO
    Timers[I](IN := TRUE);
    AllDone := AllDone AND Timers[I].Q;
END_FOR;
Preset := Timers[3].PT;
END_PROGRAM
"#;
    let mut harness = run(source, 1);
    assert_eq!(
        harness.get_output("Preset"),
        Some(Value::Time(Duration::from_millis(100)))
    );
    assert_eq!(harness.get_output("AllDone"), Some(Value::Bool(false)));
    harness.advance_time(Duration::from_millis(150));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("AllDone"), Some(Value::Bool(true)));
}

#[test]
fn element_initializers_initialize_each_instance_in_order() {
    // The Annex A form: each element's initializer in its own parentheses, a repetition n((...)), and elements after the list
    // keeping their declared initial values.
    let source = r#"
PROGRAM Main
VAR
    Listed : ARRAY[1..3] OF TON := [(PT := T#100ms), (PT := T#300ms)];
    Repeated : ARRAY[0..2] OF TON := [2((PT := T#50ms)), (PT := T#70ms)];
    Listed1 : TIME;
    Listed2 : TIME;
    Listed3 : TIME;
    Repeated0 : TIME;
    Repeated1 : TIME;
    Repeated2 : TIME;
END_VAR
Listed1 := Listed[1].PT;
Listed2 := Listed[2].PT;
Listed3 := Listed[3].PT;
Repeated0 := Repeated[0].PT;
Repeated1 := Repeated[1].PT;
Repeated2 := Repeated[2].PT;
END_PROGRAM
"#;
    let harness = run(source, 1);
    let time = |ms| Some(Value::Time(Duration::from_millis(ms)));
    assert_eq!(harness.get_output("Listed1"), time(100));
    assert_eq!(harness.get_output("Listed2"), time(300));
    assert_eq!(harness.get_output("Listed3"), time(0));
    assert_eq!(harness.get_output("Repeated0"), time(50));
    assert_eq!(harness.get_output("Repeated1"), time(50));
    assert_eq!(harness.get_output("Repeated2"), time(70));
}

#[test]
fn a_named_array_type_of_function_blocks_declares_instances() {
    // Table 11 feature 5a: an array type whose elements are function blocks.
    let source = format!(
        "{ACCUMULATOR}
TYPE Accumulators : ARRAY[0..1] OF Accumulator;
END_TYPE

PROGRAM Main
VAR
    Sums : Accumulators;
    First : DINT;
    Second : DINT;
END_VAR
Sums[0](Increment := 3);
Sums[1](Increment := 4);
First := Sums[0].Total;
Second := Sums[1].Total;
END_PROGRAM
"
    );
    let harness = run(&source, 2);
    assert_eq!(harness.get_output("First"), Some(Value::DInt(6)));
    assert_eq!(harness.get_output("Second"), Some(Value::DInt(8)));
}

#[test]
fn excess_element_initializers_are_ignored() {
    // docs/specs/02-data-types.md §3.4: excess rightmost initial values are ignored.
    let source = r#"
PROGRAM Main
VAR
    Listed : ARRAY[1..2] OF TON := [(PT := T#1ms), (PT := T#2ms), (PT := T#3ms)];
    Last : TIME;
END_VAR
Last := Listed[2].PT;
END_PROGRAM
"#;
    let harness = run(source, 1);
    assert_eq!(
        harness.get_output("Last"),
        Some(Value::Time(Duration::from_millis(2)))
    );
}

#[test]
fn a_named_array_type_initializer_initializes_every_variable_of_the_type() {
    // Table 11 feature 5b: TYPE TONS : ARRAY[1..50] OF TON := [50(PT := T#100ms)]; each variable of the type gets it.
    let source = r#"
TYPE Delays : ARRAY[1..3] OF TON := [3(PT := T#100ms)];
END_TYPE

PROGRAM Main
VAR
    First : Delays;
    Second : Delays;
    Preset1 : TIME;
    Preset2 : TIME;
END_VAR
Preset1 := First[1].PT;
Preset2 := Second[3].PT;
END_PROGRAM
"#;
    let harness = run(source, 1);
    assert_eq!(
        harness.get_output("Preset1"),
        Some(Value::Time(Duration::from_millis(100)))
    );
    assert_eq!(
        harness.get_output("Preset2"),
        Some(Value::Time(Duration::from_millis(100)))
    );
}

#[test]
fn a_function_block_may_own_an_instance_array() {
    let source = format!(
        "{ACCUMULATOR}
FUNCTION_BLOCK Bank
VAR_OUTPUT
    Total : DINT;
END_VAR
VAR
    Sums : ARRAY[0..1] OF Accumulator;
END_VAR
Sums[0](Increment := 1);
Sums[1](Increment := 2);
Total := Sums[0].Total + Sums[1].Total;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    First : Bank;
    Second : Bank;
    FirstTotal : DINT;
    SecondTotal : DINT;
END_VAR
First();
First();
Second();
FirstTotal := First.Total;
SecondTotal := Second.Total;
END_PROGRAM
"
    );
    let harness = run(&source, 1);
    assert_eq!(harness.get_output("FirstTotal"), Some(Value::DInt(6)));
    assert_eq!(harness.get_output("SecondTotal"), Some(Value::DInt(3)));
}

#[test]
fn class_elements_keep_their_own_state_through_method_calls() {
    // Table 11 feature 5a also allows classes as array elements.
    let source = r#"
CLASS Counter
VAR
    Count : DINT;
END_VAR
METHOD PUBLIC Bump : DINT
VAR_INPUT
    Amount : DINT;
END_VAR
Count := Count + Amount;
Bump := Count;
END_METHOD
END_CLASS

PROGRAM Main
VAR
    Counters : ARRAY[0..1] OF Counter;
    First : DINT;
    Second : DINT;
END_VAR
First := Counters[0].Bump(Amount := 1);
Second := Counters[1].Bump(Amount := 5);
END_PROGRAM
"#;
    let harness = run(source, 2);
    assert_eq!(harness.get_output("First"), Some(Value::DInt(2)));
    assert_eq!(harness.get_output("Second"), Some(Value::DInt(10)));
}

#[test]
fn a_cold_restart_recreates_program_instance_arrays() {
    // A cold restart re-creates every element with its initial state.
    let source = format!(
        "{ACCUMULATOR}
PROGRAM Main
VAR
    Sums : ARRAY[0..1] OF Accumulator;
    First : DINT;
    Second : DINT;
END_VAR
Sums[0](Increment := 1);
Sums[1](Increment := 2);
First := Sums[0].Total;
Second := Sums[1].Total;
END_PROGRAM
"
    );
    let mut harness = run(&source, 3);
    assert_eq!(harness.get_output("Second"), Some(Value::DInt(6)));
    harness
        .restart(trust_runtime::RestartMode::Cold)
        .expect("cold restart");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("First"), Some(Value::DInt(1)));
    assert_eq!(harness.get_output("Second"), Some(Value::DInt(2)));
}

#[test]
fn a_cold_restart_recreates_global_instance_arrays_without_aliasing_other_instances() {
    // A global instance array is re-created with fresh elements; it shares no element with the program's own instances.
    let source = format!(
        "{ACCUMULATOR}
PROGRAM Main
VAR_EXTERNAL
    Shared : ARRAY[0..1] OF Accumulator;
END_VAR
VAR
    Own : Accumulator;
    First : DINT;
    Second : DINT;
    Mine : DINT;
END_VAR
Shared[0](Increment := 1);
Shared[1](Increment := 2);
Own(Increment := 100);
First := Shared[0].Total;
Second := Shared[1].Total;
Mine := Own.Total;
END_PROGRAM

CONFIGURATION Config
VAR_GLOBAL
    Shared : ARRAY[0..1] OF Accumulator;
END_VAR
RESOURCE Res ON PLC
    TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"
    );
    let mut harness = TestHarness::from_source(&source).expect("compile and initialize");
    for _ in 0..2 {
        harness.advance_time(Duration::from_millis(10));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
    assert_eq!(harness.get_output("First"), Some(Value::DInt(2)));
    assert_eq!(harness.get_output("Second"), Some(Value::DInt(4)));
    assert_eq!(harness.get_output("Mine"), Some(Value::DInt(200)));
    harness
        .restart(trust_runtime::RestartMode::Cold)
        .expect("cold restart");
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("First"), Some(Value::DInt(1)));
    assert_eq!(harness.get_output("Second"), Some(Value::DInt(2)));
    assert_eq!(harness.get_output("Mine"), Some(Value::DInt(100)));
}

#[test]
fn a_warm_restart_rebuilds_retained_global_instance_arrays_and_keeps_retained_values() {
    // docs/specs/10-runtime-semantics.md §4.8: instance identities are never retained, including inside an aggregate; the
    // warm restart reconstructs the instances, and retained values keep their values.
    let source = format!(
        "{ACCUMULATOR}
PROGRAM Main
VAR_EXTERNAL
    Shared : ARRAY[0..1] OF Accumulator;
    Kept : DINT;
END_VAR
VAR
    Own : Accumulator;
    First : DINT;
    Mine : DINT;
END_VAR
Shared[0](Increment := 1);
Own(Increment := 100);
Kept := Kept + 1;
First := Shared[0].Total;
Mine := Own.Total;
END_PROGRAM

CONFIGURATION Config
VAR_GLOBAL RETAIN
    Shared : ARRAY[0..1] OF Accumulator;
    Kept : DINT;
END_VAR
RESOURCE Res ON PLC
    TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"
    );
    let mut harness = TestHarness::from_source(&source).expect("compile and initialize");
    for _ in 0..2 {
        harness.advance_time(Duration::from_millis(10));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
    assert_eq!(harness.get_output("Kept"), Some(Value::DInt(2)));
    harness
        .restart(trust_runtime::RestartMode::Warm)
        .expect("warm restart");
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("Kept"), Some(Value::DInt(3)));
    assert_eq!(harness.get_output("First"), Some(Value::DInt(1)));
    assert_eq!(harness.get_output("Mine"), Some(Value::DInt(100)));
}

#[test]
fn a_cold_restart_reapplies_global_element_initializers() {
    // docs/specs/10-runtime-semantics.md §4.8: a cold restart uses the declaration initializers.
    let source = r#"
PROGRAM Main
VAR_EXTERNAL
    Delays : ARRAY[0..1] OF TON;
END_VAR
VAR
    Before : TIME;
    After : TIME;
END_VAR
Before := Delays[1].PT;
Delays[1](IN := FALSE, PT := T#99ms);
After := Delays[1].PT;
END_PROGRAM

CONFIGURATION Config
VAR_GLOBAL
    Delays : ARRAY[0..1] OF TON := [(PT := T#20ms), (PT := T#40ms)];
END_VAR
RESOURCE Res ON PLC
    TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"#;
    let time = |ms| Some(Value::Time(Duration::from_millis(ms)));
    let mut harness = TestHarness::from_source(source).expect("compile and initialize");
    for _ in 0..2 {
        harness.advance_time(Duration::from_millis(10));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
    assert_eq!(harness.get_output("Before"), time(99));
    harness
        .restart(trust_runtime::RestartMode::Cold)
        .expect("cold restart");
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("Before"), time(40));
    assert_eq!(harness.get_output("After"), time(99));
}

#[test]
fn a_method_local_instance_array_takes_its_element_initializers() {
    let source = r#"
FUNCTION_BLOCK Holder
METHOD PUBLIC Presets : TIME
VAR
    Timers : ARRAY[0..1] OF TON := [(PT := T#15ms), (PT := T#25ms)];
END_VAR
Presets := Timers[0].PT + Timers[1].PT;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Keeper : Holder;
    Total : TIME;
END_VAR
Total := Keeper.Presets();
END_PROGRAM
"#;
    let harness = run(source, 1);
    assert_eq!(
        harness.get_output("Total"),
        Some(Value::Time(Duration::from_millis(40)))
    );
}

#[test]
fn a_warm_restart_rebuilds_a_retained_global_function_block_instance() {
    // docs/specs/10-runtime-semantics.md §4.8: the instance identity of a RETAIN function-block global is not retained.
    let source = format!(
        "{ACCUMULATOR}
PROGRAM Main
VAR_EXTERNAL
    Shared : Accumulator;
END_VAR
VAR
    Own : Accumulator;
    Total : DINT;
    Mine : DINT;
END_VAR
Shared(Increment := 1);
Own(Increment := 100);
Total := Shared.Total;
Mine := Own.Total;
END_PROGRAM

CONFIGURATION Config
VAR_GLOBAL RETAIN
    Shared : Accumulator;
END_VAR
RESOURCE Res ON PLC
    TASK Fast(INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"
    );
    let mut harness = TestHarness::from_source(&source).expect("compile and initialize");
    for _ in 0..2 {
        harness.advance_time(Duration::from_millis(10));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    }
    harness
        .restart(trust_runtime::RestartMode::Warm)
        .expect("warm restart");
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "cycle errors: {:?}", cycle.errors);
    assert_eq!(harness.get_output("Total"), Some(Value::DInt(1)));
    assert_eq!(harness.get_output("Mine"), Some(Value::DInt(100)));
}
