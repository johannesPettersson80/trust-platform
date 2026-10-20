//! An online change carries every variable's value into the new program (specification 11 §6.7.1;
//! specification 34 §11.3; decision OC-1 and the owner's "keep what fits", 20 October 2026).
//!
//! Each test installs a program, runs it, changes it online on a computer's installed runtime and
//! reads what the new program holds before and after its first scan. Every expected value is worked
//! out in the test from the rules: a variable at the same place with the same name keeps its value,
//! retained or not; a changed declaration keeps what fits; a reference names the same variable or
//! becomes NULL; a constant takes the new program's value; each variable that does not keep its value
//! as it was is recorded as an event, at most 32 of them and then their count.

use trust_runtime::debug::{DebugControl, RuntimeEvent};
use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::{bytecode_bytes_from_source, TestHarness};
use trust_runtime::runtime_core::vm::CarriedValueEvent;
use trust_runtime::value::{Duration, Value};

/// A harness running `source` on the bytecode VM, after a cold start.
fn installed(source: &str) -> TestHarness {
    let mut harness = TestHarness::from_source(source).expect("the program compiles");
    let bytes = bytecode_bytes_from_source(source).expect("the program builds");
    harness
        .runtime_mut()
        .apply_bytecode_bytes(&bytes, None)
        .expect("the program installs");
    harness
        .runtime_mut()
        .set_execution_backend(ExecutionBackend::BytecodeVm)
        .expect("the VM runs it");
    harness
        .runtime_mut()
        .restart(trust_runtime::RestartMode::Cold)
        .expect("the program starts");
    harness
}

/// Runs one scan, which must succeed.
fn scan(harness: &mut TestHarness) {
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "the scan runs: {cycle:?}");
}

/// Changes the installed program to `source` online.
fn change(harness: &mut TestHarness, source: &str) {
    let bytes = bytecode_bytes_from_source(source).expect("the new program builds");
    harness
        .runtime_mut()
        .apply_online_change_bytes(&bytes, None)
        .expect("the online change is applied");
}

/// The value of the variable `name` of the program instance `program`, or of the global `name`.
fn value(harness: &TestHarness, program: Option<&str>, name: &str) -> Option<Value> {
    let storage = harness.runtime().storage();
    match program {
        Some(program) => match storage.get_global(program) {
            Some(Value::Instance(id)) => storage.get_instance_var(*id, name).cloned(),
            _ => None,
        },
        None => storage.get_global(name).cloned(),
    }
}

/// The value of member `member` of the function-block instance `block` of program `program`.
fn member(harness: &TestHarness, program: &str, block: &str, member: &str) -> Option<Value> {
    let storage = harness.runtime().storage();
    let Some(Value::Instance(program)) = storage.get_global(program) else {
        return None;
    };
    let Some(Value::Instance(block)) = storage.get_instance_var(*program, block) else {
        return None;
    };
    storage.get_instance_var(*block, member).cloned()
}

/// The events of the variables an online change did not give their running values as they were.
fn carried_events(debug: &DebugControl) -> Vec<CarriedValueEvent> {
    debug
        .drain_runtime_events()
        .into_iter()
        .filter_map(|event| match event {
            RuntimeEvent::OnlineChangeValue { event, .. } => Some(event),
            _ => None,
        })
        .collect()
}

fn new(name: &str) -> CarriedValueEvent {
    CarriedValueEvent::New { name: name.into() }
}

fn converted(name: &str) -> CarriedValueEvent {
    CarriedValueEvent::Converted { name: name.into() }
}

fn not_kept(name: &str) -> CarriedValueEvent {
    CarriedValueEvent::NotKept { name: name.into() }
}

const COUNTING: &str = r#"
CONFIGURATION Conf
VAR_GLOBAL
    g_plain : INT := 2;
END_VAR
VAR_GLOBAL RETAIN
    g_ret : INT := 1;
END_VAR
RESOURCE R ON CPU
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK Counter
VAR_INPUT
    delta : INT := 1;
END_VAR
VAR_OUTPUT
    count : INT := 0;
END_VAR
count := count + delta;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    c : Counter;
    plain : DINT := 0;
    text : STRING[10] := 'a';
    table : ARRAY[1..3] OF INT;
END_VAR
c();
plain := plain + DINT#1;
text := 'scanned';
table[2] := c.count;
END_PROGRAM
"#;

#[test]
fn an_online_change_keeps_every_variable_at_the_same_place() {
    // Specification 11 §6.7.1 (decision OC-1): every variable the new program declares at the same
    // place and by the same name keeps the running value, retained or not. The program runs three
    // scans: the instance `c` counts to 3, `plain` is 3, `text` is 'scanned', `table[2]` is 3. The
    // test then sets the global `g_plain` to 40 and the retained `g_ret` to 30. The new program has
    // the same declarations and a body that adds 10: right after the change it holds 3, 3,
    // 'scanned', 3, 40 and 30, and records no event; its first scan counts on from there (c 13,
    // plain 13). Wrong behaviour caught: variables that are not retained reset to their initial
    // values (plain 0, text 'a', g_plain 2), an instance built anew (count 0), and an event for a
    // variable that kept its value.
    let mut harness = installed(COUNTING);
    let debug = harness.runtime_mut().enable_debug();
    for _ in 0..3 {
        harness.advance_time(Duration::from_millis(10));
        scan(&mut harness);
    }
    harness.set_input("g_plain", Value::Int(40));
    harness.set_input("g_ret", Value::Int(30));
    assert_eq!(member(&harness, "P1", "c", "count"), Some(Value::Int(3)));
    let _ = debug.drain_runtime_events();

    change(
        &mut harness,
        &COUNTING
            .replace("count := count + delta;", "count := count + delta * 10;")
            .replace("plain := plain + DINT#1;", "plain := plain + DINT#10;"),
    );

    assert_eq!(member(&harness, "P1", "c", "count"), Some(Value::Int(3)));
    assert_eq!(value(&harness, Some("P1"), "plain"), Some(Value::DInt(3)));
    assert_eq!(
        value(&harness, Some("P1"), "text"),
        Some(Value::String("scanned".into()))
    );
    assert_eq!(value(&harness, None, "g_plain"), Some(Value::Int(40)));
    assert_eq!(value(&harness, None, "g_ret"), Some(Value::Int(30)));
    let Some(Value::Array(table)) = value(&harness, Some("P1"), "table") else {
        panic!("table is an array");
    };
    assert_eq!(table.elements()[1], Value::Int(3));
    assert_eq!(carried_events(&debug), Vec::new(), "every value was kept");

    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    assert_eq!(member(&harness, "P1", "c", "count"), Some(Value::Int(13)));
    assert_eq!(value(&harness, Some("P1"), "plain"), Some(Value::DInt(13)));
}

const DECLARATIONS: &str = r#"
TYPE Color : (Red, Green, Blue); END_TYPE
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT; END_TYPE

CONFIGURATION Conf
RESOURCE R ON CPU
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_RESOURCE
END_CONFIGURATION

PROGRAM Main
VAR
    a : INT;
    b : DINT;
    b2 : DINT;
    s : STRING[10];
    t : STRING[10];
    lim : INT;
    e : Color := Color#Green;
    table : ARRAY[1..3] OF INT;
    st : Point;
END_VAR
a := 5;
b := 70000;
b2 := 5;
s := 'abcdefgh';
t := 'abc';
lim := 100;
e := Color#Red;
table[1] := 1;
table[2] := 2;
table[3] := 3;
st.x := 1;
st.y := 2;
END_PROGRAM
"#;

const CHANGED_DECLARATIONS: &str = r#"
TYPE Color : (Blue, Green, Red); END_TYPE
TYPE Point : STRUCT y : INT; z : INT := 9; END_STRUCT; END_TYPE

CONFIGURATION Conf
RESOURCE R ON CPU
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_RESOURCE
END_CONFIGURATION

PROGRAM Main
VAR
    a : DINT;
    b : INT;
    b2 : INT;
    s : STRING[5];
    t : STRING[5];
    lim : INT(0..10);
    e : Color := Color#Green;
    table : ARRAY[2..4] OF INT;
    st : Point;
END_VAR
END_PROGRAM
"#;

#[test]
fn a_changed_declaration_keeps_what_fits() {
    // Specification 11 §6.7.1 ("keep what fits", owner 20 October 2026): a value is converted when the
    // new type holds it, that is when it converts to the new type and back unchanged; a structure keeps
    // its fields by name, an array the elements at the indices both have; the rest keeps its initial
    // value, and no value refuses the change. The running program holds a 5 (INT), b 70,000 and b2 5
    // (DINT), s 'abcdefgh' and t 'abc' (STRING[10]), lim 100 (INT), e Red of (Red, Green, Blue),
    // table [1, 2, 3] at 1..3, and st (x 1, y 2). The new program declares a DINT, b and b2 INT, s
    // and t STRING[5], lim INT(0..10), e of (Blue, Green, Red), table at 2..4, and Point (y, z := 9):
    //   a 5 (converted), b 0 (70,000 does not fit an INT), b2 5 (converted), s '' (eight
    //   characters do not fit five), t 'abc' (converted), lim 0 (100 is outside 0..10; the
    //   subrange's initial value is its lower bound), e Red by its name (now the value 2), table
    //   [2, 3, 0] at 2..4 (the element 1 is gone and 4 is new), st.y 2 and st.z 9 (a new field takes
    //   its initializer).
    // The events name each variable that did not keep its value as it was, in declaration order.
    // Wrong behaviour caught: a change refused because a value does not fit (the old retained
    // subrange refusal), a value cut or wrapped to fit (s 'abcde', b 4,464), an enumeration value kept
    // by its number (Blue), an array copied by position (table [1, 2, 3]), a structure reset as a whole
    // (st.y 0), and a converted value without an event.
    let mut harness = installed(DECLARATIONS);
    let debug = harness.runtime_mut().enable_debug();
    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    let _ = debug.drain_runtime_events();

    change(&mut harness, CHANGED_DECLARATIONS);

    let p1 = |name: &str| value(&harness, Some("P1"), name);
    assert_eq!(p1("a"), Some(Value::DInt(5)));
    assert_eq!(p1("b"), Some(Value::Int(0)));
    assert_eq!(p1("b2"), Some(Value::Int(5)));
    assert_eq!(p1("s"), Some(Value::String("".into())));
    assert_eq!(p1("t"), Some(Value::String("abc".into())));
    assert_eq!(p1("lim"), Some(Value::Int(0)));
    let Some(Value::Enum(e)) = p1("e") else {
        panic!("e is an enumeration value");
    };
    assert_eq!((e.variant_name().as_str(), e.numeric_value()), ("Red", 2));
    let Some(Value::Array(table)) = p1("table") else {
        panic!("table is an array");
    };
    assert_eq!(
        table.elements(),
        [Value::Int(2), Value::Int(3), Value::Int(0)].as_slice()
    );
    let Some(Value::Struct(st)) = p1("st") else {
        panic!("st is a structure");
    };
    let field = |name: &str| {
        st.fields()
            .iter()
            .find(|(field, _)| field.eq_ignore_ascii_case(name))
            .map(|(_, value)| value.clone())
    };
    assert_eq!(field("y"), Some(Value::Int(2)));
    assert_eq!(field("z"), Some(Value::Int(9)));
    assert_eq!(field("x"), None);
    assert_eq!(
        carried_events(&debug),
        [
            converted("P1.a"),
            not_kept("P1.b"),
            converted("P1.b2"),
            not_kept("P1.s"),
            converted("P1.t"),
            not_kept("P1.lim"),
            converted("P1.e"),
            converted("P1.table"),
            new("P1.st.z"),
        ]
    );
}

const REFERENCES: &str = r#"
CONFIGURATION Conf
VAR_GLOBAL
    target : INT;
    other : INT;
    r : REF_TO INT;
    q : REF_TO INT;
END_VAR
RESOURCE R ON CPU
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK Counter
VAR_OUTPUT
    count : INT;
END_VAR
count := count + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    c : Counter;
    pc : REF_TO INT;
END_VAR
c();
target := 5;
other := 6;
r := REF(target);
q := REF(other);
pc := REF(c.count);
END_PROGRAM
"#;

const CHANGED_REFERENCES: &str = r#"
CONFIGURATION Conf
VAR_GLOBAL
    target : INT;
    r : REF_TO INT;
    q : REF_TO INT;
    seen_r : INT;
    seen_pc : INT;
    q_null : BOOL;
END_VAR
RESOURCE R ON CPU
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK Counter
VAR_OUTPUT
    count : INT;
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    c : Counter;
    pc : REF_TO INT;
END_VAR
IF r <> NULL THEN
    seen_r := r^;
END_IF;
IF pc <> NULL THEN
    seen_pc := pc^;
END_IF;
q_null := q = NULL;
END_PROGRAM
"#;

#[test]
fn a_reference_names_the_same_variable_or_becomes_null() {
    // Specification 11 §6.7.1: a reference names the same variable in the new program when it has it
    // with the type the reference names, and is NULL otherwise. The running program sets target 5 and
    // other 6, points r at target, q at other and pc at the member count of its instance c, which one
    // scan leaves at 1. The new program drops `other` and reads through the references in its first
    // scan: r reads 5 and pc reads 1 in the new program's storage, and q is NULL, recorded as cleared;
    // the new globals seen_r, seen_pc and q_null start new. Wrong behaviour caught: a reference that
    // still names the old program's storage (its first dereference would fault), a reference to a
    // removed variable left dangling, and a member reference that loses its instance.
    let mut harness = installed(REFERENCES);
    let debug = harness.runtime_mut().enable_debug();
    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    let _ = debug.drain_runtime_events();

    change(&mut harness, CHANGED_REFERENCES);
    assert_eq!(
        carried_events(&debug),
        [
            new("seen_r"),
            new("seen_pc"),
            new("q_null"),
            CarriedValueEvent::Cleared { name: "q".into() },
        ],
        "the new globals in declaration order, then the reference cleared once every instance is known"
    );

    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    assert_eq!(value(&harness, None, "seen_r"), Some(Value::Int(5)));
    assert_eq!(value(&harness, None, "seen_pc"), Some(Value::Int(1)));
    assert_eq!(value(&harness, None, "q_null"), Some(Value::Bool(true)));
}

#[test]
fn a_constant_takes_the_new_programs_value() {
    // Specification 11 §6.7.1: constants keep the candidate's values. The running program's constant
    // k is 5 and its scan copies it to x. The new program declares k := 7: right after the change k is
    // 7, x keeps its running 5, and the first scan copies 7. Wrong behaviour caught: a constant carried
    // from the running program, which would keep 5 against the new declaration.
    let source = |k: i32| {
        format!(
            r#"
VAR_GLOBAL CONSTANT
    k : INT := {k};
END_VAR
VAR_GLOBAL
    x : INT;
END_VAR

PROGRAM Main
x := k;
END_PROGRAM
"#
        )
    };
    let mut harness = installed(&source(5));
    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    assert_eq!(value(&harness, None, "x"), Some(Value::Int(5)));

    change(&mut harness, &source(7));
    assert_eq!(value(&harness, None, "k"), Some(Value::Int(7)));
    assert_eq!(value(&harness, None, "x"), Some(Value::Int(5)));
    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    assert_eq!(value(&harness, None, "x"), Some(Value::Int(7)));
}

#[test]
fn the_events_of_one_change_are_bounded() {
    // Specification 11 §6.7.1: a change records at most 32 events and then one with the number of the
    // others. The new program adds 40 globals, g0 to g39, each new: the change records g0 to g31 as
    // new and then that 8 more variables did not keep their running values. Wrong behaviour caught:
    // an unbounded list, whose memory grows with the program on a chip, and a count that loses
    // variables.
    let globals = |count: usize| {
        (0..count)
            .map(|index| format!("    g{index} : INT;\n"))
            .collect::<String>()
    };
    let source = |count: usize| {
        format!("VAR_GLOBAL\n    base : INT;\n{}END_VAR\n\nPROGRAM Main\nbase := base + 1;\nEND_PROGRAM\n", globals(count))
    };
    let mut harness = installed(&source(0));
    let debug = harness.runtime_mut().enable_debug();
    harness.advance_time(Duration::from_millis(10));
    scan(&mut harness);
    let _ = debug.drain_runtime_events();

    change(&mut harness, &source(40));
    let mut expected: Vec<CarriedValueEvent> =
        (0..32).map(|index| new(&format!("g{index}"))).collect();
    expected.push(CarriedValueEvent::More { count: 8 });
    assert_eq!(carried_events(&debug), expected);
    assert_eq!(value(&harness, None, "base"), Some(Value::Int(1)));
}

const BLOCK: &str = r#"
CONFIGURATION Conf
RESOURCE R ON CPU
TASK T (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM P1 WITH T : Main;
END_RESOURCE
END_CONFIGURATION

FUNCTION_BLOCK Counter
VAR_INPUT
    delta : INT := 1;
END_VAR
VAR
    count : INT;
END_VAR
count := count + delta;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    c : Counter;
END_VAR
c(delta := 2);
END_PROGRAM
"#;

#[test]
fn an_instance_keeps_its_members_by_name_when_its_block_changes() {
    // Specification 11 §6.7.1: an instance keeps its members by name, recursively, and a changed
    // member keeps what fits. Three scans leave c.count 6 and the input c.delta 2. The new block
    // declares count as a DINT and adds `limit : INT := 9`: count is 6 converted, delta stays 2, limit
    // starts at 9, and the events name c.count as converted and c.limit as new. Wrong behaviour
    // caught: an instance of a changed block built anew (count 0, delta 1).
    let mut harness = installed(BLOCK);
    let debug = harness.runtime_mut().enable_debug();
    for _ in 0..3 {
        harness.advance_time(Duration::from_millis(10));
        scan(&mut harness);
    }
    let _ = debug.drain_runtime_events();

    change(
        &mut harness,
        &BLOCK.replace(
            "VAR\n    count : INT;\nEND_VAR",
            "VAR\n    count : DINT;\n    limit : INT := 9;\nEND_VAR",
        ),
    );
    assert_eq!(member(&harness, "P1", "c", "count"), Some(Value::DInt(6)));
    assert_eq!(member(&harness, "P1", "c", "delta"), Some(Value::Int(2)));
    assert_eq!(member(&harness, "P1", "c", "limit"), Some(Value::Int(9)));
    assert_eq!(
        carried_events(&debug),
        [converted("P1.c.count"), new("P1.c.limit")]
    );
}

const TIMER: &str = r#"
PROGRAM Main
VAR
    t : TON;
    done : BOOL;
    elapsed : TIME;
END_VAR
t(IN := TRUE, PT := T#100ms);
done := t.Q;
elapsed := t.ET;
END_PROGRAM
"#;

#[test]
fn a_timer_keeps_running_through_an_online_change() {
    // Specification 11 §6.7.1: an instance of the same standard block keeps its internal state, and an
    // input its last value. The timer starts at the first scan, 10 ms, so at 60 ms its elapsed time is
    // 50 ms. The program is changed online to itself with another statement, and the scans go on every
    // 10 ms: at 110 ms the timer has run 100 ms and its output is TRUE. Wrong behaviour caught: a timer
    // that starts again at the change, at the first scan after it, 70 ms (at 110 ms it would show
    // 40 ms and FALSE), which delays every timed output of a machine by the time it had already run.
    let mut harness = installed(TIMER);
    for _ in 0..6 {
        harness.advance_time(Duration::from_millis(10));
        scan(&mut harness);
    }
    assert_eq!(
        harness.get_output("elapsed"),
        Some(Value::Time(Duration::from_millis(50)))
    );
    change(
        &mut harness,
        &TIMER.replace(
            "elapsed := t.ET;",
            "elapsed := t.ET;\ndone := done OR FALSE;",
        ),
    );
    for _ in 0..5 {
        harness.advance_time(Duration::from_millis(10));
        scan(&mut harness);
    }
    assert_eq!(
        harness.get_output("elapsed"),
        Some(Value::Time(Duration::from_millis(100)))
    );
    assert_eq!(harness.get_output("done"), Some(Value::Bool(true)));
}
