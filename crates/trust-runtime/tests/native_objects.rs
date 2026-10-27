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

/// A method given a function block instance as an input, which picks a child of it through an
/// interface and keeps it after the call (IEC §6.6.3.4.1, Table 42 feature 9a).
const INSTANCE_INPUT: &str = "
INTERFACE INode
    METHOD Value : INT
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Node IMPLEMENTS INode
VAR
    v : INT;
END_VAR
METHOD PUBLIC Value : INT
Value := v;
END_METHOD
METHOD PUBLIC Set
VAR_INPUT
    n : INT;
END_VAR
v := n;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Pair
VAR_OUTPUT
    Count : INT := 2;
END_VAR
VAR
    first : Node;
    second : Node;
END_VAR
METHOD PUBLIC Child : INode
VAR_INPUT
    index : INT;
END_VAR
IF index = 1 THEN
    Child := first;
ELSE
    Child := second;
END_IF;
END_METHOD
METHOD PUBLIC Fill
first.Set(n := 10);
second.Set(n := 20);
END_METHOD
METHOD PUBLIC Raise
second.Set(n := second.Value() + 10);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Picker
VAR
    picked : INode;
END_VAR
METHOD PUBLIC Pick : INT
VAR_INPUT
    p : Pair;
END_VAR
picked := p.Child(index := p.Count);
Pick := picked.Value();
END_METHOD
METHOD PUBLIC Again : INT
Again := picked.Value();
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    pair : Pair;
    picker : Picker;
    now : INT;
    later : INT;
END_VAR
pair.Fill();
now := picker.Pick(p := pair);
pair.Raise();
later := picker.Again();
END_PROGRAM
";

/// A builder whose methods return the builder itself, `THIS`, so that calls chain (IEC
/// §6.6.5.7.2).
const CHAINED: &str = "
TYPE Recipe :
STRUCT
    Volume : REAL;
    Speed : REAL;
    Valid : BOOL;
END_STRUCT
END_TYPE

FUNCTION_BLOCK Builder
VAR
    r : Recipe;
END_VAR
METHOD PUBLIC Volume : Builder
VAR_INPUT
    v : REAL;
END_VAR
r.Volume := v;
Volume := THIS;
END_METHOD
METHOD PUBLIC Speed : Builder
VAR_INPUT
    s : REAL;
END_VAR
r.Speed := s;
Speed := THIS;
END_METHOD
METHOD PUBLIC Build : Recipe
r.Valid := r.Volume > 0.0 AND r.Speed > 0.0;
Build := r;
END_METHOD
METHOD PUBLIC CurrentSpeed : REAL
CurrentSpeed := r.Speed;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    b : Builder;
    made : Recipe;
    volume : REAL;
    speed : REAL;
    valid : BOOL;
    kept : REAL;
END_VAR
made := b.Volume(v := 10.0).Speed(s := 45.0).Build();
volume := made.Volume;
speed := made.Speed;
valid := made.Valid;
kept := b.CurrentSpeed();
END_PROGRAM
";

/// The program of `source`, compiled for this computer, cold-started.
fn program(source: &str) -> NativeProgram<'static, trust_runtime::native::HostCode> {
    static BUILDS: AtomicUsize = AtomicUsize::new(0);
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
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
        trusted_keys: &[],
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

/// A function block instance given as an input is the caller's instance (IEC §6.6.3.4.1, Table 42
/// feature 9a; specification 44 §6.4): `Pick` reads `pair.Count`, 2, picks `pair.second`, whose
/// value `Fill` made 20, and keeps it; after `pair.Raise()` makes it 30, `Again` reads 30 through
/// the kept interface. A copy of `pair` would give `Again` the copy's child, which no longer exists
/// after the call: the build refused it before, and a copy would end the scan with
/// `ReferenceLifetime` where `Pick` takes the reference.
#[test]
fn an_instance_given_as_an_input_is_the_callers_instance() {
    let mut program = program(INSTANCE_INPUT);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    for (variable, expected) in [("MAIN.NOW", Value::Int(20)), ("MAIN.LATER", Value::Int(30))] {
        assert_eq!(program.value(variable), Some(expected), "{variable}");
    }
}

/// A method's result of a function block's type names the instance assigned to it, here `THIS`
/// (IEC §6.6.5.7.2; specification 44 §6.4): `b.Volume(10.0).Speed(45.0).Build()` sets the volume
/// and the speed of `b` itself and builds the recipe 10.0, 45.0, valid; `b` then holds the speed
/// 45.0. A result that copied the instance would leave `b`'s speed 0.0, as `Speed` would set it on
/// the copy.
#[test]
fn a_method_that_returns_this_chains_on_the_instance() {
    let mut program = program(CHAINED);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    for (variable, expected) in [
        ("MAIN.VOLUME", Value::Real(10.0)),
        ("MAIN.SPEED", Value::Real(45.0)),
        ("MAIN.VALID", Value::Bool(true)),
        ("MAIN.KEPT", Value::Real(45.0)),
    ] {
        assert_eq!(program.value(variable), Some(expected), "{variable}");
    }
}

/// A derived function block that its call gives an inherited input, an input of its own, and whose
/// inherited output it reads.
const INHERITED_INPUTS: &str = "
FUNCTION_BLOCK Scale
VAR_INPUT
    Gain : DINT;
END_VAR
VAR_OUTPUT
    Out : DINT;
END_VAR
Out := Gain * 2;
END_FUNCTION_BLOCK

FUNCTION_BLOCK Shifted EXTENDS Scale
VAR_INPUT
    Bias : DINT;
END_VAR
Out := Gain * 2 + Bias;
END_FUNCTION_BLOCK

PROGRAM Calls
VAR
    s : Shifted;
    gain : DINT;
    result : DINT;
END_VAR
s(Gain := 5, Bias := 3, Out => result);
gain := s.Gain;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Calls;
END_RESOURCE
END_CONFIGURATION
";

/// A derived function block inherits every variable of its base (IEC §6.6.7.2.7, §6.6.5), so its
/// call connects the inputs and outputs it inherits as its own: `Gain := 5` sets the inherited
/// input, `Bias := 3` its own, and `Out => result` reads the inherited output, 5 × 2 + 3 = 13, on
/// the kernel and through the test harness. Catches a call refused with "expected 0 arguments" or "unknown parameter", which
/// trust-hir gave before 23 October 2026, and an inherited input left unset.
#[test]
fn a_derived_function_block_call_connects_its_inherited_inputs_and_outputs() {
    let mut program = program(INHERITED_INPUTS);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    let mut runtime = trust_runtime::harness::TestHarness::from_source(INHERITED_INPUTS)
        .expect("the runtime builds the program");
    runtime.advance_time(Duration::from_millis(10));
    let result = runtime.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    for (native, interpreted, expected) in [
        ("MAIN.S.GAIN", "s.Gain", 5),
        ("MAIN.S.BIAS", "s.Bias", 3),
        ("MAIN.S.OUT", "s.Out", 13),
        ("MAIN.RESULT", "result", 13),
        ("MAIN.GAIN", "gain", 5),
    ] {
        assert_eq!(
            program.value(native),
            Some(Value::DInt(expected)),
            "{native}"
        );
        assert_eq!(
            runtime.get_output(interpreted),
            Some(Value::DInt(expected)),
            "{interpreted}"
        );
    }
}

/// A function whose `VAR_IN_OUT` is a `Valve`, given a `MotorValve`, a type derived from it that adds
/// an input and overrides `Name`, and given a `Valve`.
const DERIVED_IN_OUT: &str = "
FUNCTION_BLOCK Valve
VAR_OUTPUT
    Open : BOOL;
END_VAR
METHOD PUBLIC Command
Open := TRUE;
END_METHOD
METHOD PUBLIC Name : INT
Name := 1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK MotorValve EXTENDS Valve
VAR_INPUT
    Speed : INT;
END_VAR
METHOD PUBLIC OVERRIDE Name : INT
Name := 2;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION Operate : INT
VAR_IN_OUT
    v : Valve;
END_VAR
v.Command();
Operate := v.Name();
END_FUNCTION

PROGRAM Plant
VAR
    m : MotorValve;
    plain : Valve;
    viaDerived : INT;
    viaBase : INT;
END_VAR
viaDerived := Operate(v := m);
viaBase := Operate(v := plain);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Plant;
END_RESOURCE
END_CONFIGURATION
";

/// An in-out of a function block type takes an instance of a derived type that adds no in-out
/// (IEC §6.6.8.3; additional inputs are allowed, `docs/IEC_DECISIONS.md` 2026-10-23), and a call
/// through it binds by the instance's class: `Operate(v := m)` sets `m.Open` and gets
/// `MotorValve.Name`, 2; `Operate(v := plain)` gets `Valve.Name`, 1; on the kernel and through the
/// test harness. Catches the
/// call refused with E205, which trust-hir gave before 23 October 2026, and a call bound by the
/// in-out's declared type (`viaDerived` 1).
#[test]
fn a_base_in_out_takes_a_derived_instance() {
    let mut program = program(DERIVED_IN_OUT);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    let mut runtime = trust_runtime::harness::TestHarness::from_source(DERIVED_IN_OUT)
        .expect("the runtime builds the program");
    runtime.advance_time(Duration::from_millis(10));
    let result = runtime.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    for (native, interpreted, expected) in [
        ("MAIN.VIADERIVED", "viaDerived", Value::Int(2)),
        ("MAIN.VIABASE", "viaBase", Value::Int(1)),
        ("MAIN.M.OPEN", "m.Open", Value::Bool(true)),
        ("MAIN.PLAIN.OPEN", "plain.Open", Value::Bool(true)),
    ] {
        assert_eq!(program.value(native), Some(expected.clone()), "{native}");
        assert_eq!(
            runtime.get_output(interpreted),
            Some(expected),
            "{interpreted}"
        );
    }
}

/// Interfaces compared for equality: `i` and `j` name `a`, `k` names `b`, `n1` and `n2` are `NULL`.
const INTERFACE_EQUALITY: &str = "
INTERFACE IShape
    METHOD Area : REAL
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Sq IMPLEMENTS IShape
METHOD PUBLIC Area : REAL
Area := 4.0;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Compare
VAR
    a : Sq;
    b : Sq;
    i : IShape;
    j : IShape;
    k : IShape;
    n1 : IShape;
    n2 : IShape;
    same : BOOL;
    differ : BOOL;
    nulls : BOOL;
    mixed : BOOL;
END_VAR
i := a;
j := a;
k := b;
same := i = j;
differ := i <> k;
nulls := n1 = n2;
mixed := i = n1;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Compare;
END_RESOURCE
END_CONFIGURATION
";

/// Two values of one interface type compare for equality (IEC §6.6.6.5.1; specification 04 §6 rule
/// 7): equal when they name the same instance or are both `NULL`, so `same`, `differ` and `nulls`
/// are TRUE and `mixed` FALSE, on the kernel and through the test harness. Catches the comparison refused with E201, which
/// trust-hir gave before 23 October 2026, and one that compares the instances' values.
#[test]
fn interfaces_compare_for_equality() {
    let mut program = program(INTERFACE_EQUALITY);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    let mut runtime = trust_runtime::harness::TestHarness::from_source(INTERFACE_EQUALITY)
        .expect("the runtime builds the program");
    runtime.advance_time(Duration::from_millis(10));
    let result = runtime.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    for (name, expected) in [
        ("same", true),
        ("differ", true),
        ("nulls", true),
        ("mixed", false),
    ] {
        assert_eq!(
            program.value(&format!("MAIN.{}", name.to_ascii_uppercase())),
            Some(Value::Bool(expected)),
            "{name}"
        );
        assert_eq!(
            runtime.get_output(name),
            Some(Value::Bool(expected)),
            "{name}"
        );
    }
}

/// `SUPER` through three levels: each `Step` adds its digit and calls its base's, and the base's
/// `Step` reads `THIS.Name()`. `z` is a `Level3`, `w` a `Level2`, `x` a `B` (of `BINDING`'s kind).
const SUPER_CHAIN: &str = "
FUNCTION_BLOCK Level1
VAR_OUTPUT
    trace : DINT;
    named : INT;
END_VAR
METHOD PUBLIC Name : INT
Name := 1;
END_METHOD
METHOD PUBLIC Advance
trace := trace * 10 + 1;
named := THIS.Name();
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Level2 EXTENDS Level1
METHOD PUBLIC OVERRIDE Advance
trace := trace * 10 + 2;
SUPER.Advance();
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Level3 EXTENDS Level2
METHOD PUBLIC OVERRIDE Name : INT
Name := 3;
END_METHOD
METHOD PUBLIC OVERRIDE Advance
trace := trace * 10 + 3;
SUPER.Advance();
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

PROGRAM Chain
VAR
    z : Level3;
    w : Level2;
    x : B;
END_VAR
z.Advance();
w.Advance();
x.Run();
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Chain;
END_RESOURCE
END_CONFIGURATION
";

/// `SUPER` binds statically to the base's method and `THIS` stays the own instance (IEC
/// §6.6.5.7.2, §6.6.5.7.3), whose class `THIS.Name()` binds to dynamically (§6.6.5.6), on the
/// kernel and through the test harness: `z.Advance()` runs `Level3`'s, `Level2`'s and `Level1`'s steps, so `z.trace` is 321, and
/// `THIS.Name()` in `Level1.Advance` is `z`'s, a `Level3`, so 3; `w`, a `Level2`, gives 21 and
/// `Level1`'s 1; `THIS.Name()` in `A.Run` reached from `B.Run` is `x`'s, so 2. Catches `THIS` bound
/// to the base part `SUPER` named (`z.named` 1, `x.r` 1), as the bytecode engine did before 23
/// October 2026, and a `SUPER` taken from the instance instead of the running method's class, which
/// would call `Level2.Advance` again without end.
#[test]
fn this_stays_the_own_instance_through_super() {
    let mut program = program(SUPER_CHAIN);
    program
        .scan(Duration::from_millis(10), &mut NoHooks)
        .expect("the scan runs");
    let mut runtime = trust_runtime::harness::TestHarness::from_source(SUPER_CHAIN)
        .expect("the runtime builds the program");
    runtime.advance_time(Duration::from_millis(10));
    let result = runtime.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    for (native, interpreted, expected) in [
        ("MAIN.Z.TRACE", "z.trace", Value::DInt(321)),
        ("MAIN.Z.NAMED", "z.named", Value::Int(3)),
        ("MAIN.W.TRACE", "w.trace", Value::DInt(21)),
        ("MAIN.W.NAMED", "w.named", Value::Int(1)),
        ("MAIN.X.R", "x.r", Value::Int(2)),
    ] {
        assert_eq!(program.value(native), Some(expected.clone()), "{native}");
        assert_eq!(
            runtime.get_output(interpreted),
            Some(expected),
            "{interpreted}"
        );
    }
}

/// A call through an interface that names no instance.
const NULL_CALL: &str = "
INTERFACE ICounter
    METHOD Inc : INT
        VAR_INPUT
            delta : INT;
        END_VAR
    END_METHOD
END_INTERFACE

PROGRAM Main
VAR
    j : ICounter;
    first : INT;
END_VAR
first := j.Inc(delta := 1);
END_PROGRAM
";

/// A call through an interface that names no instance ends the scan with the error of a null
/// dereference (specification 10) through the test harness too, as the kernel reports it
/// (`a_call_through_null_is_a_null_dereference`). Catches the `type mismatch` the bytecode engine
/// reported before 24 October 2026.
#[test]
fn a_call_through_null_is_a_null_dereference_through_the_harness() {
    let mut runtime = trust_runtime::harness::TestHarness::from_source(NULL_CALL)
        .expect("the runtime builds the program");
    runtime.advance_time(Duration::from_millis(10));
    let result = runtime.cycle();
    assert!(
        matches!(
            result.errors.as_slice(),
            [trust_runtime::error::RuntimeError::NullReference]
        ),
        "{:?}",
        result.errors
    );
}
