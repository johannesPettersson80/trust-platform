use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

// docs/specs/02-data-types.md §3.4: array elements can be function blocks
// (`Timers: ARRAY[1..50] OF TON`). Each element is its own instance with its own state.
const SOURCE: &str = r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    inc : INT;
END_VAR
VAR_OUTPUT
    total : INT;
END_VAR
total := total + inc;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Bank
VAR
    counters : ARRAY[1..2] OF Counter;
END_VAR
VAR_OUTPUT
    sum : INT;
END_VAR
counters[1](inc := 100);
counters[2](inc := 1000);
sum := counters[1].total + counters[2].total;
END_FUNCTION_BLOCK

VAR_GLOBAL
    g : ARRAY[1..2] OF Counter;
END_VAR

PROGRAM Main
VAR
    local : ARRAY[0..2] OF Counter;
    edges : ARRAY[1..2] OF R_TRIG;
    bank : Bank;
    i : INT;
    g1 : INT;
    g2 : INT;
    l2 : INT;
    edge : BOOL;
    banked : INT;
END_VAR
g[1](inc := 1);
g[2](inc := 10);
FOR i := 0 TO 2 DO
    local[i](inc := i + 1);
END_FOR
edges[2](CLK := TRUE);
bank();
g1 := g[1].total;
g2 := g[2].total;
l2 := local[2].total;
edge := edges[2].Q;
banked := bank.sum;
END_PROGRAM
"#;

#[test]
fn function_block_array_elements_are_separate_instances() {
    let mut harness = TestHarness::from_source(SOURCE).unwrap();
    harness.cycle();

    // global array, program local array, array inside a function block
    assert_eq!(harness.get_output("g1"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("g2"), Some(Value::Int(10)));
    assert_eq!(harness.get_output("l2"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("edge"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("banked"), Some(Value::Int(1100)));

    // state is kept per element across cycles
    harness.cycle();
    assert_eq!(harness.get_output("g1"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("g2"), Some(Value::Int(20)));
    assert_eq!(harness.get_output("l2"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("edge"), Some(Value::Bool(false)));
    assert_eq!(harness.get_output("banked"), Some(Value::Int(2200)));
}

#[test]
fn cold_restart_creates_function_block_array_instances_anew() {
    let mut harness = TestHarness::from_source(SOURCE).unwrap();
    harness.cycle();
    harness.cycle();
    assert_eq!(harness.get_output("g2"), Some(Value::Int(20)));
    harness.restart(trust_runtime::RestartMode::Cold).unwrap();
    harness.cycle();
    assert_eq!(harness.get_output("g1"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("g2"), Some(Value::Int(10)));
    assert_eq!(harness.get_output("l2"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("banked"), Some(Value::Int(1100)));
}

const LOCALS_AND_CLASSES: &str = r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    inc : INT;
END_VAR
VAR_OUTPUT
    total : INT;
END_VAR
total := total + inc;
END_FUNCTION_BLOCK

CLASS Acc
VAR
    n : INT;
END_VAR
METHOD PUBLIC Add : INT
VAR_INPUT
    x : INT;
END_VAR
n := n + x;
Add := n;
END_METHOD
END_CLASS

TYPE Pair : ARRAY[1..2] OF Counter; END_TYPE

FUNCTION Twice : INT
VAR_INPUT
    x : INT;
END_VAR
VAR
    tmp : ARRAY[1..2] OF Counter;
END_VAR
tmp[1](inc := x);
tmp[2](inc := tmp[1].total);
Twice := tmp[1].total + tmp[2].total;
END_FUNCTION

FUNCTION_BLOCK Holder
VAR
    accs : ARRAY[1..3] OF Acc;
END_VAR
METHOD PUBLIC Feed : INT
VAR
    scratch : ARRAY[1..2] OF Counter;
END_VAR
scratch[2](inc := 5);
Feed := accs[2].Add(x := scratch[2].total);
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    grid : ARRAY[1..2, 1..3] OF Counter;
    pair : Pair;
    holder : Holder;
    doubled : INT;
    fed : INT;
    corner : INT;
    second : INT;
END_VAR
doubled := Twice(x := 4);
fed := holder.Feed();
grid[2, 3](inc := 7);
grid[1, 1](inc := 1);
corner := grid[2, 3].total;
pair[2](inc := 9);
second := pair[2].total;
END_PROGRAM
"#;

#[test]
fn function_block_arrays_in_functions_methods_classes_and_aliases() {
    let mut harness = TestHarness::from_source(LOCALS_AND_CLASSES).unwrap();
    harness.cycle();
    // function and method locals are new instances on every call
    assert_eq!(harness.get_output("doubled"), Some(Value::Int(8)));
    assert_eq!(harness.get_output("fed"), Some(Value::Int(5)));
    // multidimensional array, array type alias
    assert_eq!(harness.get_output("corner"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(9)));
    harness.cycle();
    assert_eq!(harness.get_output("doubled"), Some(Value::Int(8)));
    // the class instance in the array keeps its state
    assert_eq!(harness.get_output("fed"), Some(Value::Int(10)));
    assert_eq!(harness.get_output("corner"), Some(Value::Int(14)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(18)));
}

const STATICS: &str = r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    inc : INT;
END_VAR
VAR_OUTPUT
    total : INT;
END_VAR
total := total + inc;
END_FUNCTION_BLOCK

FUNCTION Tally : INT
VAR_INPUT
    x : INT;
END_VAR
VAR_STAT
    kept : ARRAY[1..2] OF Counter;
END_VAR
kept[2](inc := x);
Tally := kept[2].total;
END_FUNCTION

FUNCTION_BLOCK Owner
METHOD PUBLIC Advance : INT
VAR_STAT
    kept : ARRAY[1..2] OF Counter;
END_VAR
kept[1](inc := 3);
Advance := kept[1].total;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    owner : Owner;
    tallied : INT;
    stepped : INT;
END_VAR
tallied := Tally(x := 2);
stepped := owner.Advance();
END_PROGRAM
"#;

#[test]
fn function_block_arrays_in_static_variables_keep_their_state() {
    let mut harness = TestHarness::from_source(STATICS).unwrap();
    harness.cycle();
    assert_eq!(harness.get_output("tallied"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("stepped"), Some(Value::Int(3)));
    harness.cycle();
    assert_eq!(harness.get_output("tallied"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("stepped"), Some(Value::Int(6)));
}
