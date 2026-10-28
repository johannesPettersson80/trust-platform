//! The result of a call whose value is a reference, used where a reference is expected, in a
//! compiled program (IEC 61131-3 §6.4.4.10.3 and Table 12, §6.6.6.7 and Table 52; specification
//! 44 §6.4, §7.3): a method's result taken from `SUPER`'s, as the plant library's `GetAlarm` takes
//! it (`examples/room_heating/libraries/plant/src/drive.st`), and a result assigned, dereferenced,
//! compared, passed as an input, returned and taken by `?=`. Every expected value is worked out by
//! hand from the standard's rules: a reference assigned or passed names the variable the
//! reference it was given names, and is copied without a dereference, `NULL` too (§6.4.4.10.3);
//! two references are equal when they name the same variable or are both `NULL`.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

/// An alarm that counts its raises, an alarm derived from it with a level, and a device whose
/// `GetAlarm` names its alarm of an index, `NULL` past them; a drive adds one alarm of its own and
/// leaves the device's to `SUPER.GetAlarm`, as the plant library's variable-speed motor does.
/// `CountOf` reads the count of the alarm a reference names, -1 for `NULL`.
const BLOCKS: &str = "
FUNCTION_BLOCK Alarm
VAR_OUTPUT
    Count : INT;
END_VAR
METHOD PUBLIC Raise
Count := Count + 1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK LevelAlarm EXTENDS Alarm
VAR_OUTPUT
    Level : INT;
END_VAR
METHOD PUBLIC SetLevel
VAR_INPUT
    Value : INT;
END_VAR
Level := Value;
END_METHOD
END_FUNCTION_BLOCK

INTERFACE ISource
    METHOD GetAlarm : REF_TO Alarm
    VAR_INPUT
        Index : INT;
    END_VAR
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Device IMPLEMENTS ISource
VAR
    Trip : Alarm;
    Fail : Alarm;
END_VAR
METHOD PUBLIC GetAlarm : REF_TO Alarm
VAR_INPUT
    Index : INT;
END_VAR
CASE Index OF
    1: GetAlarm := REF(Trip);
    2: GetAlarm := REF(Fail);
ELSE
    GetAlarm := NULL;
END_CASE;
END_METHOD
METHOD PUBLIC Holds : BOOL
VAR_INPUT
    Entry : REF_TO Alarm;
END_VAR
Holds := Entry = REF(Trip) OR Entry = REF(Fail);
END_METHOD
METHOD PUBLIC Itself : ISource
Itself := THIS;
END_METHOD
METHOD PUBLIC Unit : Device
Unit := THIS;
END_METHOD
METHOD PUBLIC Third : REF_TO Alarm
Third := THIS.GetAlarm(3);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Drive EXTENDS Device
VAR
    Deviation : LevelAlarm;
END_VAR
METHOD PUBLIC OVERRIDE GetAlarm : REF_TO Alarm
VAR_INPUT
    Index : INT;
END_VAR
IF Index = 3 THEN
    GetAlarm := REF(Deviation);
ELSE
    GetAlarm := SUPER.GetAlarm(Index);
END_IF;
END_METHOD
METHOD PUBLIC LevelOf : REF_TO LevelAlarm
LevelOf := REF(Deviation);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION CountOf : INT
VAR_INPUT
    Entry : REF_TO Alarm;
END_VAR
IF Entry = NULL THEN
    CountOf := -1;
ELSE
    CountOf := Entry^.Count;
END_IF;
END_FUNCTION
";

/// The program `program`, with [`BLOCKS`] before it, built for this computer, cold-started and run
/// for one scan without an error.
fn scanned(program: &str) -> TestHarness {
    let source = format!("{BLOCKS}{program}");
    let mut harness = TestHarness::from_source(&source).expect("the program builds");
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
}

fn assert_values(harness: &TestHarness, expected: &[(&str, Value)]) {
    for (name, value) in expected {
        assert_eq!(harness.get_output(name), Some(value.clone()), "{name}");
    }
}

/// The plant library's construct: `GetAlarm := SUPER.GetAlarm(Index)` in an override gives the
/// override's result the reference the base's method returns (IEC §6.6.5.7.3: `SUPER` calls the
/// base's own method on the instance), so `d.GetAlarm(1)` and `d.GetAlarm(2)` name `d`'s `Trip`
/// and `Fail`, which `Raise` counts once and twice, and `d.GetAlarm(3)`, the drive's own index,
/// names `Deviation`, raised three times; index 4 is `NULL`, and so is index 3 of `e`, a plain
/// device; `e`'s alarms stay at 0. `Third`, the device's `THIS.GetAlarm(3)`, is bound by the
/// instance's class (IEC §6.6.5.6): `d.Third()` names `d.Deviation`, `e.Third()` is `NULL`.
/// Catches the build refusing the `SUPER` call's result (the example scan of 26 October 2026), a
/// result not taken over, which leaves `r` `NULL` and ends the scan at `r^`, a `SUPER` call bound
/// to the override again, which recurses until the call depth is used up, a `THIS` call bound to
/// the device's own method, and a reference that names an alarm of another instance or index.
#[test]
fn a_method_result_is_the_result_of_super() {
    let harness = scanned(
        "
PROGRAM Main
VAR
    d : Drive;
    e : Device;
    r : REF_TO Alarm;
    missing : BOOL;
    plainMissing : BOOL;
    viaThis : BOOL;
    plainViaThis : BOOL;
END_VAR
r := d.GetAlarm(1);
r^.Raise();
r := d.GetAlarm(2);
r^.Raise();
r^.Raise();
r := d.GetAlarm(3);
r^.Raise();
r^.Raise();
r^.Raise();
r := d.GetAlarm(4);
missing := r = NULL;
r := e.GetAlarm(3);
plainMissing := r = NULL;
viaThis := d.Third() = d.GetAlarm(3);
plainViaThis := e.Third() = NULL;
END_PROGRAM
",
    );
    assert_values(
        &harness,
        &[
            ("d.Trip.Count", Value::Int(1)),
            ("d.Fail.Count", Value::Int(2)),
            ("d.Deviation.Count", Value::Int(3)),
            ("e.Trip.Count", Value::Int(0)),
            ("e.Fail.Count", Value::Int(0)),
            ("missing", Value::Bool(true)),
            ("plainMissing", Value::Bool(true)),
            ("viaThis", Value::Bool(true)),
            ("plainViaThis", Value::Bool(true)),
        ],
    );
}

/// A call's result dereferenced where it stands and compared (IEC Table 12 features 4 and 2c):
/// `d.GetAlarm(3)^.Raise()` raises `d.Deviation` once and `src.GetAlarm(Index := 1)^.Raise()`,
/// through the interface `src` that names `d`, raises `d.Trip` twice; `d.GetAlarm(2)` and
/// `src.GetAlarm(Index := 2)` both name `d.Fail`, so they are equal; `Trip` and `Fail` differ;
/// index 3 through `src` reaches the drive's override and is no `NULL`, index 0 is `NULL`, with
/// `NULL` on the left too. Catches a dereference or a comparison refused for a call's value, and a
/// comparison of the results' storage instead of the variables they name.
#[test]
fn a_call_result_is_dereferenced_and_compared() {
    let harness = scanned(
        "
PROGRAM Main
VAR
    d : Drive;
    src : ISource;
    sameAlarm : BOOL;
    otherAlarm : BOOL;
    present : BOOL;
    absent : BOOL;
END_VAR
src := d;
d.GetAlarm(3)^.Raise();
src.GetAlarm(Index := 1)^.Raise();
src.GetAlarm(Index := 1)^.Raise();
sameAlarm := d.GetAlarm(2) = src.GetAlarm(Index := 2);
otherAlarm := d.GetAlarm(1) <> d.GetAlarm(2);
present := src.GetAlarm(Index := 3) <> NULL;
absent := NULL = src.GetAlarm(Index := 0);
END_PROGRAM
",
    );
    assert_values(
        &harness,
        &[
            ("d.Deviation.Count", Value::Int(1)),
            ("d.Trip.Count", Value::Int(2)),
            ("d.Fail.Count", Value::Int(0)),
            ("sameAlarm", Value::Bool(true)),
            ("otherAlarm", Value::Bool(true)),
            ("present", Value::Bool(true)),
            ("absent", Value::Bool(true)),
        ],
    );
}

/// A call's result given to an input of a function block, a function and a method (IEC Table 12
/// feature 2b): `raiser`, called twice with `d.GetAlarm(2)`, raises `d.Fail` twice and sees 2;
/// `CountOf` reads 2 through it and -1 for `e`'s index 7, `NULL`; `d.Holds` finds `d`'s own `Trip`
/// among its alarms and not `e`'s, a variable of the same name in another instance. Catches an
/// input refused for a call's value, a copy of the alarm instead of a reference to it (which
/// `raiser` would raise without changing `d.Fail`), and a reference that names a member without its
/// instance.
#[test]
fn a_call_result_is_passed_as_an_input() {
    let harness = scanned(
        "
FUNCTION_BLOCK Raiser
VAR_INPUT
    Target : REF_TO Alarm;
END_VAR
VAR_OUTPUT
    Seen : INT;
END_VAR
IF Target <> NULL THEN
    Target^.Raise();
    Seen := Target^.Count;
END_IF;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    d : Drive;
    e : Device;
    raiser : Raiser;
    seen : INT;
    counted : INT;
    none : INT;
    own : BOOL;
    foreign : BOOL;
END_VAR
raiser(Target := d.GetAlarm(2));
raiser(Target := d.GetAlarm(2));
seen := raiser.Seen;
counted := CountOf(d.GetAlarm(Index := 2));
none := CountOf(Entry := e.GetAlarm(7));
own := d.Holds(Entry := d.GetAlarm(1));
foreign := d.Holds(Entry := e.GetAlarm(1));
END_PROGRAM
",
    );
    assert_values(
        &harness,
        &[
            ("d.Fail.Count", Value::Int(2)),
            ("seen", Value::Int(2)),
            ("counted", Value::Int(2)),
            ("none", Value::Int(-1)),
            ("own", Value::Bool(true)),
            ("foreign", Value::Bool(false)),
        ],
    );
}

/// A call's result returned as a function's result, by assignment to the function's name and by
/// `RETURN expr` (IEC §6.6.2.2; specification 06, DEV-022): `FirstOf(d)` names `d.Trip`, raised
/// once through `r`, `FirstOf(Source := e)` names `e.Trip`, raised twice where it stands, and
/// `ThirdOf(d)` names `d.Deviation`, raised three times; `ThirdOf(e)` is `NULL`. Catches a result
/// refused for a call's value and a result that is not the reference the inner call returned.
#[test]
fn a_call_result_is_returned() {
    let harness = scanned(
        "
FUNCTION FirstOf : REF_TO Alarm
VAR_INPUT
    Source : ISource;
END_VAR
FirstOf := Source.GetAlarm(Index := 1);
END_FUNCTION

FUNCTION ThirdOf : REF_TO Alarm
VAR_INPUT
    Source : ISource;
END_VAR
RETURN Source.GetAlarm(Index := 3);
END_FUNCTION

PROGRAM Main
VAR
    d : Drive;
    e : Device;
    r : REF_TO Alarm;
    plainThird : BOOL;
END_VAR
r := FirstOf(d);
r^.Raise();
FirstOf(Source := e)^.Raise();
FirstOf(Source := e)^.Raise();
ThirdOf(d)^.Raise();
ThirdOf(d)^.Raise();
ThirdOf(d)^.Raise();
plainThird := ThirdOf(e) = NULL;
END_PROGRAM
",
    );
    assert_values(
        &harness,
        &[
            ("d.Trip.Count", Value::Int(1)),
            ("e.Trip.Count", Value::Int(2)),
            ("d.Deviation.Count", Value::Int(3)),
            ("plainThird", Value::Bool(true)),
        ],
    );
}

/// The assignment attempt `?=` of a call's result (IEC §6.6.6.7, Table 52 feature 2, EXAMPLE 3):
/// `d.GetAlarm(3)` names `d.Deviation`, a `LevelAlarm`, so `level` takes it and sets its level 7;
/// `d.GetAlarm(1)` names `d.Trip`, an `Alarm` only, so `level` becomes `NULL`. `d.Itself()`, an
/// interface naming `d`, a drive, gives `drive` a reference, `e.Itself()` a `NULL`. A call's
/// result assigned to an interface: `e.Unit()`, of the device's type, names `e` (specification 44
/// §6.4), through which `Fail` of `e` is raised once, and `d.Itself()` names `d`, whose `Trip` is
/// raised once. Catches `?=` refused for a call's value, a cast that ignores the class of the
/// instance named, and an interface that names another instance than the result's.
#[test]
fn an_assignment_attempt_and_an_interface_take_a_call_result() {
    let harness = scanned(
        "
PROGRAM Main
VAR
    d : Drive;
    e : Device;
    level : REF_TO LevelAlarm;
    drive : REF_TO Drive;
    src : ISource;
    isLevel : BOOL;
    tripIsLevel : BOOL;
    isDrive : BOOL;
    plainIsDrive : BOOL;
END_VAR
level ?= d.GetAlarm(3);
isLevel := level <> NULL;
IF isLevel THEN
    level^.SetLevel(Value := 7);
END_IF;
level ?= d.GetAlarm(1);
tripIsLevel := level <> NULL;
drive ?= d.Itself();
isDrive := drive <> NULL;
drive ?= e.Itself();
plainIsDrive := drive <> NULL;
src := e.Unit();
src.GetAlarm(Index := 2)^.Raise();
src := d.Itself();
src.GetAlarm(Index := 1)^.Raise();
END_PROGRAM
",
    );
    assert_values(
        &harness,
        &[
            ("isLevel", Value::Bool(true)),
            ("d.Deviation.Level", Value::Int(7)),
            ("tripIsLevel", Value::Bool(false)),
            ("isDrive", Value::Bool(true)),
            ("plainIsDrive", Value::Bool(false)),
            ("e.Fail.Count", Value::Int(1)),
            ("e.Trip.Count", Value::Int(0)),
            ("d.Trip.Count", Value::Int(1)),
        ],
    );
}

/// A reference to an instance of a derived type assigned to a reference of its base type names
/// the same instance and is bound to the base type (IEC §6.4.4.10.3, Table 12 feature 2a):
/// `d.LevelOf()`, a `REF_TO LevelAlarm`, taken by `r`, a `REF_TO Alarm`, raises `d.Deviation`
/// once, the variable `exact` of the derived type taken by `r` raises it again, and `CountOf`,
/// whose input is a `REF_TO Alarm`, reads 2 through `d.LevelOf()`; `exact` sets the level 4, and
/// `r` names the alarm `d.GetAlarm(3)` names, `REF` of it taken as an `Alarm`. Catches a reference that keeps the extent of the
/// derived type, which a dereference as the base refuses with `TypeMismatch`, ending the scan.
#[test]
fn a_reference_of_a_derived_type_is_taken_as_its_base() {
    let harness = scanned(
        "
PROGRAM Main
VAR
    d : Drive;
    r : REF_TO Alarm;
    exact : REF_TO LevelAlarm;
    counted : INT;
    same : BOOL;
END_VAR
r := d.LevelOf();
r^.Raise();
exact := d.LevelOf();
exact^.SetLevel(Value := 4);
r := exact;
r^.Raise();
counted := CountOf(d.LevelOf());
same := r = d.GetAlarm(3);
END_PROGRAM
",
    );
    assert_values(
        &harness,
        &[
            ("d.Deviation.Count", Value::Int(2)),
            ("d.Deviation.Level", Value::Int(4)),
            ("counted", Value::Int(2)),
            ("same", Value::Bool(true)),
        ],
    );
}

/// Results that a method leaves `NULL`: `Registry.Find` of a `REF_TO` type when its item is not
/// asked for, and `Builder.Empty` of the block's own type unless the builder is `Ready`, which
/// `Builder.Relay` returns as its own result. `probe` dereferences the `NULL` reference, `relay`
/// calls a method through the `NULL` interface, `chained` calls a method on `Relay`'s result.
const NULL_RESULTS: &str = "
INTERFACE IBump
    METHOD Bump
    END_METHOD
END_INTERFACE

FUNCTION_BLOCK Builder IMPLEMENTS IBump
VAR_INPUT
    Ready : BOOL;
END_VAR
VAR_OUTPUT
    Count : INT;
END_VAR
METHOD PUBLIC Empty : Builder
IF Ready THEN
    Empty := THIS;
END_IF;
END_METHOD
METHOD PUBLIC Relay : Builder
Relay := THIS.Empty();
END_METHOD
METHOD PUBLIC Bump
Count := Count + 1;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK Registry
VAR
    Item : Builder;
END_VAR
METHOD PUBLIC Find : REF_TO Builder
VAR_INPUT
    Present : BOOL;
END_VAR
IF Present THEN
    Find := REF(Item);
END_IF;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    reg : Registry;
    b : Builder;
    r : REF_TO Builder;
    held : IBump;
    refNull : BOOL;
    empty : BOOL;
    named : BOOL;
    probe : BOOL;
    relay : BOOL;
    chained : BOOL;
END_VAR
r := reg.Find(Present := FALSE);
refNull := r = NULL;
held := b.Relay();
empty := held = NULL;
b.Ready := TRUE;
held := b.Relay();
named := held <> NULL;
held.Bump();
b.Ready := FALSE;
IF probe THEN
    r^.Bump();
END_IF;
IF relay THEN
    held := b.Relay();
    held.Bump();
END_IF;
IF chained THEN
    b.Relay().Bump();
END_IF;
END_PROGRAM
";

/// An assignment copies a reference and never dereferences it (IEC §6.4.4.10.3: only `^`
/// dereferences; specification 44 §7.3): `reg.Find(Present := FALSE)`, a `REF_TO` result its
/// method leaves `NULL`, assigned to `r` stores `NULL`; `b.Relay()` returns `THIS.Empty()`, which
/// is `NULL` while `b` is not `Ready`, so `held` takes `NULL`, and the scan ends without an error;
/// once `b` is `Ready`, `held` names `b`, whose `Bump` counts 1. Dereferenced, a `NULL` faults:
/// `r^.Bump()`, a call through `held` taken from the `NULL` result, and the chained
/// `b.Relay().Bump()` each end their scan with `NullReference`. Catches an assignment that
/// dereferences a result of a block's type, which ended the scan in `Relay` with `NullReference`
/// before 26 October 2026, and a dereference of `NULL` that does not fault.
#[test]
fn a_null_result_is_assigned_as_null_and_faults_only_when_dereferenced() {
    let mut harness = TestHarness::from_source(NULL_RESULTS).expect("the program builds");
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_values(
        &harness,
        &[
            ("refNull", Value::Bool(true)),
            ("empty", Value::Bool(true)),
            ("named", Value::Bool(true)),
            ("b.Count", Value::Int(1)),
        ],
    );
    assert!(
        matches!(harness.get_output("r"), Some(Value::Reference(None))),
        "{:?}",
        harness.get_output("r")
    );
    for probe in ["probe", "relay", "chained"] {
        let mut harness = TestHarness::from_source(NULL_RESULTS).expect("the program builds");
        harness.set_input(probe, true);
        harness.advance_time(Duration::from_millis(10));
        let cycle = harness.cycle();
        assert!(
            matches!(cycle.errors.as_slice(), [RuntimeError::NullReference]),
            "{probe}: {:?}",
            cycle.errors
        );
    }
}
