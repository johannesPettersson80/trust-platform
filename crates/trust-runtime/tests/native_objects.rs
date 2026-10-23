//! Classes, function blocks that extend others, interfaces and properties in a compiled program
//! (IEC §6.6.5 to §6.6.8; specification 44 §6.4): every call reaches the method the standard binds
//! it to, and the values are worked out by hand from the standard's examples.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::sync::atomic::{AtomicUsize, Ordering};

use trust_plc_abi::FaultCode;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::{
    build, load, Device, NativeProgram, NoHooks, Options, Target, Toolchain,
};
use trust_runtime::value::{Duration, Value};

/// The lighting example of IEC §6.6.8.3 and §6.6.8.4, `THIS` of §6.6.8.5, and `SUPER` of §6.6.5.7.
const BINDING: &str = "
CLASS LIGHTROOM
VAR PUBLIC
    LIGHT : BOOL;
END_VAR
METHOD PUBLIC SET_DAYTIME
VAR_INPUT
    DAYTIME : BOOL;
END_VAR
LIGHT := NOT DAYTIME;
END_METHOD
END_CLASS

CLASS LIGHT2ROOM EXTENDS LIGHTROOM
VAR PUBLIC
    LIGHT2 : BOOL;
END_VAR
METHOD PUBLIC OVERRIDE SET_DAYTIME
VAR_INPUT
    DAYTIME : BOOL;
END_VAR
SUPER.SET_DAYTIME(DAYTIME := DAYTIME);
LIGHT2 := NOT DAYTIME;
END_METHOD
END_CLASS

FUNCTION_BLOCK ROOM_CTRL
VAR_IN_OUT
    RM : LIGHTROOM;
END_VAR
VAR_INPUT
    DAYTIME : BOOL;
END_VAR
RM.SET_DAYTIME(DAYTIME := DAYTIME);
END_FUNCTION_BLOCK

FUNCTION_BLOCK Shape
VAR_OUTPUT
    Sides : INT;
END_VAR
METHOD PUBLIC Count : INT
Count := 0;
END_METHOD
METHOD PUBLIC Describe
Sides := THIS.Count();
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Square EXTENDS Shape
METHOD PUBLIC OVERRIDE Count : INT
Count := 4;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK A
VAR_OUTPUT
    r : INT;
END_VAR
METHOD PUBLIC Name : INT
Name := 1;
END_METHOD
METHOD PUBLIC Run
r := THIS.Name();
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK B EXTENDS A
METHOD PUBLIC OVERRIDE Name : INT
Name := 2;
END_METHOD
METHOD PUBLIC OVERRIDE Run
SUPER.Run();
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    MyRoom1 : LIGHTROOM;
    MyRoom2 : LIGHT2ROOM;
    Ctrl : ROOM_CTRL;
    Room : REF_TO LIGHTROOM;
    s : Square;
    b : Shape;
    x : B;
    y : A;
END_VAR
Ctrl(RM := MyRoom1, DAYTIME := FALSE);
Room := REF(MyRoom2);
Room^.SET_DAYTIME(DAYTIME := FALSE);
s.Describe();
b.Describe();
x.Run();
y.Run();
END_PROGRAM
";

/// Two classes that implement one interface differently, a property through the interface, and a
/// variable of the interface that names no instance (IEC §6.6.6).
const INTERFACES: &str = "
INTERFACE ICounter
METHOD Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
END_METHOD
PROPERTY Total : INT
GET
END_GET
END_PROPERTY
END_INTERFACE

CLASS Counter IMPLEMENTS ICounter
VAR
    value : INT;
END_VAR
METHOD PUBLIC Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
value := value + delta;
Inc := value;
END_METHOD
PUBLIC PROPERTY Total : INT
GET
    Total := value;
END_GET
END_PROPERTY
END_CLASS

CLASS Meter IMPLEMENTS ICounter
METHOD PUBLIC Inc : INT
VAR_INPUT
    delta : INT;
END_VAR
Inc := delta * 10;
END_METHOD
PUBLIC PROPERTY Total : INT
GET
    Total := -1;
END_GET
END_PROPERTY
END_CLASS

PROGRAM Main
VAR
    c : Counter;
    m : Meter;
    i : ICounter;
    k : ICounter;
    j : ICounter;
    first : INT;
    second : INT;
    total : INT;
    isnull : BOOL;
    fail : BOOL;
END_VAR
i := c;
k := m;
first := i.Inc(delta := 2);
second := k.Inc(delta := 3);
total := i.Total;
isnull := j = NULL;
IF fail THEN
    first := j.Inc(delta := 1);
END_IF;
END_PROGRAM
";

/// A property with `GET` and `SET`, and a method with outputs.
const MEMBERS: &str = "
FUNCTION_BLOCK Tank
VAR
    stored : REAL;
END_VAR
PUBLIC PROPERTY Level : REAL
GET
    Level := stored;
END_GET
SET
    stored := Level;
END_SET
END_PROPERTY
METHOD PUBLIC Split : INT
VAR_INPUT
    v : INT;
END_VAR
VAR_OUTPUT
    tens : INT;
    ones : INT;
END_VAR
tens := v / 10;
ones := v MOD 10;
Split := tens + ones;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    t : Tank;
    level : REAL;
    sum : INT;
    tens : INT;
    ones : INT;
END_VAR
t.Level := REAL#2.5;
level := t.Level;
sum := t.Split(v := 47, tens => tens, ones => ones);
END_PROGRAM
";

/// The program of `source`, compiled for this computer, cold-started.
fn program(source: &str) -> NativeProgram<'static, trust_runtime::native::HostCode> {
    static BUILDS: AtomicUsize = AtomicUsize::new(0);
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let host = Target::this_cpu().expect("a computer target");
    let work = std::env::temp_dir().join(format!(
        "trust-native-objects-{}-{}",
        std::process::id(),
        BUILDS.fetch_add(1, Ordering::Relaxed)
    ));
    let built = build(
        &[SourceFile::new(source)],
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the program compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target: host,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 10_000,
    };
    let image: &'static [u8] = Box::leak(built.image.into_boxed_slice());
    let mut program = load(image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    program
}

/// Each call reaches the method of the instance's class (IEC §6.6.8): through a `VAR_IN_OUT`
/// `RM` names `MyRoom1`, so `LIGHT` is `NOT FALSE`; `Room`, a reference to a `LIGHTROOM`, names
/// `MyRoom2`, a `LIGHT2ROOM`, so the override sets `LIGHT2` and, through `SUPER`, `LIGHT`;
/// `THIS.Count()` in `Describe` reaches `Square`'s 4 for `s` and `Shape`'s 0 for `b`; and
/// `THIS.Name()` in `A.Run`, reached from `B.Run` through `SUPER`, is still `x`'s, a `B`, so 2
/// (§6.6.8.5), while `y`, an `A`, gets 1. A call bound by the variable's declared type would leave
/// `LIGHT2` FALSE, `s.Sides` 0 and `x.r` 1.
#[test]
fn calls_bind_to_the_class_of_the_instance() {
    let mut program = program(BINDING);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    for (variable, expected) in [
        ("MAIN.MYROOM1.LIGHT", Value::Bool(true)),
        ("MAIN.MYROOM2.LIGHT", Value::Bool(true)),
        ("MAIN.MYROOM2.LIGHT2", Value::Bool(true)),
        ("MAIN.S.SIDES", Value::Int(4)),
        ("MAIN.B.SIDES", Value::Int(0)),
        ("MAIN.X.R", Value::Int(2)),
        ("MAIN.Y.R", Value::Int(1)),
    ] {
        assert_eq!(program.value(variable), Some(expected), "{variable}");
    }
}

/// A call through an interface reaches the class of the instance assigned to it (IEC §6.6.6,
/// §6.6.8.2): `i` names the `Counter`, whose `Inc(2)` makes its value 2 and whose `Total` reads it;
/// `k` names the `Meter`, whose `Inc(3)` is 30; `j` was never assigned and is `NULL`. A dispatch
/// that took the first implementing class for every call would give `second` 5.
#[test]
fn interfaces_reach_the_class_assigned_to_them() {
    let mut program = program(INTERFACES);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    for (variable, expected) in [
        ("MAIN.FIRST", Value::Int(2)),
        ("MAIN.SECOND", Value::Int(30)),
        ("MAIN.TOTAL", Value::Int(2)),
        ("MAIN.ISNULL", Value::Bool(true)),
    ] {
        assert_eq!(program.value(variable), Some(expected), "{variable}");
    }
}

/// A call through an interface that names no instance ends the scan with `NullReference`, the
/// error of a null dereference (specification 10; specification 44 §6.5), at the statement of the
/// call. A dispatch that read the class of whatever lies at offset 0 of the area would call a
/// method on a foreign instance instead.
#[test]
fn a_call_through_null_is_a_null_dereference() {
    let mut program = program(INTERFACES);
    let (fail, _) = program
        .image()
        .find_symbol("MAIN.FAIL")
        .expect("the symbol of fail");
    program
        .write_symbol(fail, &Value::Bool(true))
        .expect("fail is written");
    assert!(program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .is_err());
    assert_eq!(program.last_fault().code, FaultCode::NullReference as u32);
}

/// `t.Level := 2.5` calls the property's `SET`, which stores 2.5, and reading `t.Level` calls its
/// `GET`; `Split(47)` writes its outputs 4 and 7 to `tens` and `ones` after the call and returns
/// their sum, 11 (IEC §6.6.1.4). An assignment that wrote a member instead of calling `SET`, or
/// outputs that were not copied back, would leave `level`, `tens` and `ones` 0.
#[test]
fn properties_and_method_outputs() {
    let mut program = program(MEMBERS);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    for (variable, expected) in [
        ("MAIN.LEVEL", Value::Real(2.5)),
        ("MAIN.SUM", Value::Int(11)),
        ("MAIN.TENS", Value::Int(4)),
        ("MAIN.ONES", Value::Int(7)),
    ] {
        assert_eq!(program.value(variable), Some(expected), "{variable}");
    }
}
