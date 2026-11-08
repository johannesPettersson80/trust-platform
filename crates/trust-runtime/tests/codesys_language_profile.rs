use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;
use trust_runtime::RestartMode;

#[path = "codesys_language_profile/call_after_init.rs"]
mod call_after_init;
#[path = "codesys_language_profile/instance_path.rs"]
mod instance_path;
#[path = "codesys_language_profile/noinit.rs"]
mod noinit;

// Specification 10 §4.8: under the CODESYS profiles a restart calls FB_init of every instance whose
// type declares it, before the first cycle; under the IEC profile nothing calls it. Catches FB_init
// never called (the CODESYS report's A4: the lamp would read FALSE), and called under the IEC
// profile, where a valid program may declare a method of that name.
const FB_INIT_LAMP: &str = r#"
FUNCTION_BLOCK FB_Lamp
VAR_OUTPUT
    initialized : BOOL;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
END_VAR
initialized := TRUE;
FB_init := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    lamp : FB_Lamp;
    seen : BOOL;
END_VAR
seen := lamp.initialized;
END_PROGRAM
"#;

#[test]
fn fb_init_runs_before_the_first_cycle_under_the_codesys_profile_only() {
    for (profile, expected) in [
        (ConversionProfile::Codesys, true),
        (ConversionProfile::Iec, false),
    ] {
        let session = CompileSession::from_source(FB_INIT_LAMP).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{profile:?}: {:?}", cycle.errors);
        assert_eq!(
            harness.get_output("seen"),
            Some(Value::Bool(expected)),
            "{profile:?}"
        );
    }
}

// Specification 10 §4.8: the order of the implicit calls and what they are passed. Each FB_init
// writes its digit into `trace`: FB_Leaf 1, FB_Base 2, FB_Derived 3, FB_Kept 4. Worked by hand from
// the declarations: the retained global `kept` (4), then the global `gLeaf` (1); then Main's
// variables in order: `derived` calls its base part (2), its own nested `leaf` (1) and its own
// FB_init (3); each of the two `children`, whose type inherits FB_Base's FB_init and declares none,
// calls it once (2, 2); the four elements of `grid` (1, 1, 1, 1); the leaf in the structure
// `holder` (1); the leaf in `plain`, a block without FB_init (1). So 4121322111111, thirteen calls.
// A cold start passes bInitRetains TRUE to all thirteen, a warm restart FALSE, neither passes
// bInCopyCode TRUE. Catches an instance left out (a retained or a plain global, a nested one, an
// array element, a structure member), an inherited FB_init called twice, a base after its derived
// part or members after their block, and the two flags swapped or fixed.
const FB_INIT_ORDER: &str = r#"
VAR_GLOBAL RETAIN
    kept : FB_Kept;
END_VAR
VAR_GLOBAL
    gLeaf : FB_Leaf;
    trace : LINT;
    calls : INT;
    retaining : INT;
    copying : INT;
END_VAR

FUNCTION Note : BOOL
VAR_INPUT
    digit : LINT;
    retains : BOOL;
    copies : BOOL;
END_VAR
trace := trace * 10 + digit;
calls := calls + 1;
IF retains THEN
    retaining := retaining + 1;
END_IF
IF copies THEN
    copying := copying + 1;
END_IF
Note := TRUE;
END_FUNCTION

FUNCTION_BLOCK FB_Leaf
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
END_VAR
FB_init := Note(1, bInitRetains, bInCopyCode);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Kept
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
END_VAR
FB_init := Note(4, bInitRetains, bInCopyCode);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Base
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
END_VAR
FB_init := Note(2, bInitRetains, bInCopyCode);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    leaf : FB_Leaf;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
END_VAR
FB_init := Note(3, bInitRetains, bInCopyCode);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Child EXTENDS FB_Base
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Plain
VAR
    inner : FB_Leaf;
END_VAR
END_FUNCTION_BLOCK

TYPE S_Holder :
STRUCT
    leaf : FB_Leaf;
END_STRUCT
END_TYPE

PROGRAM Main
VAR
    derived : FB_Derived;
    children : ARRAY[1..2] OF FB_Child;
    grid : ARRAY[1..2, 1..2] OF FB_Leaf;
    holder : S_Holder;
    plain : FB_Plain;
    traceSeen : LINT;
    callsSeen : INT;
    retainingSeen : INT;
    copyingSeen : INT;
END_VAR
traceSeen := trace;
callsSeen := calls;
retainingSeen := retaining;
copyingSeen := copying;
END_PROGRAM
"#;

#[test]
fn fb_init_runs_for_every_instance_in_order_with_the_restart_flags() {
    let session = CompileSession::from_source(FB_INIT_ORDER)
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    for (restart, retaining) in [("cold", 13), ("warm", 0)] {
        if restart == "warm" {
            harness
                .restart(RestartMode::Warm)
                .expect("the warm restart runs");
        }
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{restart}: {:?}", cycle.errors);
        assert_eq!(
            harness.get_output("traceSeen"),
            Some(Value::LInt(4_121_322_111_111)),
            "{restart}"
        );
        assert_eq!(
            harness.get_output("callsSeen"),
            Some(Value::Int(13)),
            "{restart}"
        );
        assert_eq!(
            harness.get_output("retainingSeen"),
            Some(Value::Int(retaining)),
            "{restart}"
        );
        assert_eq!(
            harness.get_output("copyingSeen"),
            Some(Value::Int(0)),
            "{restart}"
        );
    }
}

// Specification 10 §4.8: the inputs of FB_init after the first two take the arguments of the
// instance's declaration, by name or all in order, else their initial value, else their type's default;
// a member's declaration in a function block and a global's declaration give theirs, and a base
// part's FB_init takes the same arguments. Worked by hand from the declarations: `named` gets port
// 3, the default speed 50 and fast TRUE; `positional` port 7, speed 80 and fast FALSE; the
// holder's `inner` port 9; the global `gPort` port 11 and speed 20; `derived`, whose FB_init
// repeats FB_Port's inputs and adds a name code, has its base part set by FB_Port's FB_init to port
// 5 and its own code 42. Catches the arguments ignored (port 0), bound one input too early (to
// bInCopyCode), the default lost, and a base part called without them.
const FB_INIT_ARGUMENTS: &str = r#"
FUNCTION_BLOCK FB_Port
VAR_OUTPUT
    port : INT;
    speed : INT;
    fast : BOOL;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
    iPort : INT;
    nSpeed : INT := 50;
    bFast : BOOL;
END_VAR
port := iPort;
speed := nSpeed;
fast := bFast;
FB_init := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Coded EXTENDS FB_Port
VAR_OUTPUT
    code : DINT;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
    iPort : INT;
    nSpeed : INT := 50;
    bFast : BOOL;
    nCode : DINT;
END_VAR
code := nCode;
FB_init := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Holder
VAR
    inner : FB_Port(iPort := 9);
END_VAR
END_FUNCTION_BLOCK

VAR_GLOBAL
    gPort : FB_Port(11, 20, FALSE);
END_VAR

PROGRAM Main
VAR
    named : FB_Port(bFast := TRUE, iPort := 3);
    positional : FB_Port(7, 80, FALSE);
    holder : FB_Holder;
    derived : FB_Coded(iPort := 5, nCode := 42);
    s1, s2, s3, s4, s5, s6, s7, s8, s9, s10 : DINT;
END_VAR
s1 := named.port;
s2 := named.speed;
s3 := BOOL_TO_DINT(named.fast);
s4 := positional.port;
s5 := positional.speed;
s6 := BOOL_TO_DINT(positional.fast);
s7 := holder.inner.port;
s8 := gPort.port * 100 + gPort.speed;
s9 := derived.port;
s10 := derived.code;
END_PROGRAM
"#;

#[test]
fn fb_init_takes_the_arguments_of_the_declaration() {
    let harness = codesys_run(FB_INIT_ARGUMENTS, 1);
    let seen: Vec<Value> = (1..=10)
        .map(|index| {
            harness
                .get_output(&format!("s{index}"))
                .unwrap_or(Value::Null)
        })
        .collect();
    assert_eq!(
        seen,
        [3, 50, 1, 7, 80, 0, 9, 1120, 5, 42]
            .into_iter()
            .map(Value::DInt)
            .collect::<Vec<_>>()
    );
}

// Specification 44 §6.7: an input of a type that is no scalar, here a STRING, from the instance's
// declaration is not compiled yet (NX-09), so the build refuses it rather than pass something else.
// Catches the argument dropped and the input left empty.
#[test]
fn an_fb_init_string_argument_is_refused_at_build() {
    let source = r#"
FUNCTION_BLOCK FB_Named
VAR
    title : STRING;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
    sTitle : STRING;
END_VAR
title := sTitle;
FB_init := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    named : FB_Named(sTitle := 'pump');
END_VAR
END_PROGRAM
"#;
    let session =
        CompileSession::from_source(source).with_conversion_profile(ConversionProfile::Codesys);
    let error = TestHarness::from_session(session)
        .err()
        .expect("the build refuses it");
    assert!(
        error
            .to_string()
            .contains("FB_Named.FB_init's input sTitle, of a type that is no scalar"),
        "{error}"
    );
}

// Specification 14: the project's `vendor_profile` selects the language rules of the runtime build
// with its conversion rules. A function block that assigns its own input in its body is refused
// under the IEC rules (IEC 61131-3 Ed.3 Figure 13 rule 2a) and built under the CODESYS profile
// (docs/IEC_DEVIATIONS.md DEV-029).
const SOURCE: &str = r#"
FUNCTION_BLOCK Counter
VAR_INPUT
    increment : INT;
END_VAR
VAR_OUTPUT
    total : INT;
END_VAR
total := total + increment;
increment := 0;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    counter : Counter;
    first : INT;
    second : INT;
END_VAR
counter(increment := 5);
first := counter.total;
counter();
second := counter.total;
END_PROGRAM
"#;

// Worked by hand: the first call adds 5 and the body clears `increment`; the second call assigns no
// input, so the block keeps the 0 its body left and the total stays 5. Catches the CODESYS profile
// not reaching the runtime build (refused as under IEC), and a body's input assignment lost (the
// second call would add 5 again, giving 10).
#[test]
fn the_codesys_profile_builds_a_block_that_assigns_its_own_input() {
    let session =
        CompileSession::from_source(SOURCE).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    harness.cycle();
    assert_eq!(harness.get_output("first"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("second"), Some(Value::Int(5)));
}

// Catches the CODESYS rule applied to a project under the IEC rules.
#[test]
fn the_iec_rules_refuse_a_block_that_assigns_its_own_input() {
    let Err(error) = TestHarness::from_session(CompileSession::from_source(SOURCE)) else {
        panic!("the IEC rules refuse the input assignment");
    };
    let text = error.to_string();
    assert!(
        text.contains("cannot assign to input parameter 'increment'"),
        "{text}"
    );
}

/// The program of `source` built under the CODESYS profile and run for `cycles` cycles.
// CG-A1: the dotted form `E_Mode.AUTO` of an enumeration value, which CODESYS writes everywhere and
// requires for a `qualified_only` type, compiles wherever a value does: an initializer, a
// comparison, a CASE label, a call argument and an assignment, under the CODESYS and the IEC
// profiles (two-part access is IEC §6.4.4.2's type prefix). Worked by hand: the mode starts AUTO,
// so the comparison is TRUE, CASE takes its second label (2), the call is TRUE, and SERVICE is
// assigned. Catches the build refusing the type name as a variable ("the name E_Mode is not
// compiled yet") and a member taken for another value.
const DOTTED_ENUM: &str = r#"
{attribute 'qualified_only'}
TYPE E_Mode : (MANUAL, AUTO, SERVICE); END_TYPE

FUNCTION IsAuto : BOOL
VAR_INPUT
    mode : E_Mode;
END_VAR
IsAuto := mode = E_Mode.AUTO;
END_FUNCTION

PROGRAM Main
VAR
    mode : E_Mode := E_Mode.AUTO;
    viaCompare : BOOL;
    viaCase : INT;
    viaCall : BOOL;
    assigned : E_Mode;
    isService : BOOL;
END_VAR
viaCompare := mode = E_Mode.AUTO;
CASE mode OF
    E_Mode.MANUAL: viaCase := 1;
    E_Mode.AUTO: viaCase := 2;
ELSE
    viaCase := 3;
END_CASE
viaCall := IsAuto(E_Mode.AUTO);
assigned := E_Mode.SERVICE;
isService := assigned = E_Mode#SERVICE;
END_PROGRAM
"#;

#[test]
fn a_dotted_enumeration_value_compiles_wherever_a_value_does() {
    for profile in [ConversionProfile::Codesys, ConversionProfile::Iec] {
        let session = CompileSession::from_source(DOTTED_ENUM).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{profile:?}: {:?}", cycle.errors);
        assert_eq!(
            harness.get_output("viaCompare"),
            Some(Value::Bool(true)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("viaCase"),
            Some(Value::Int(2)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("viaCall"),
            Some(Value::Bool(true)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("isService"),
            Some(Value::Bool(true)),
            "{profile:?}"
        );
    }
}

fn codesys_run(source: &str, cycles: usize) -> TestHarness {
    let session =
        CompileSession::from_source(source).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    for _ in 0..cycles {
        harness.cycle();
    }
    harness
}

// Specification 04: under the CODESYS profile a method or function that never assigns its result
// returns the default of its result type: 0, the first enumerator, the empty string, and 0 after a
// bare `RETURN`. The variables start elsewhere (7, 'x', 9, 1.5), so a call that left them as they
// were, or returned anything but the default, is caught; so is the build refusing the POUs (E206).
#[test]
fn an_unassigned_result_is_the_default_of_its_type() {
    let harness = codesys_run(
        r#"
TYPE E_Mode : (MANUAL, AUTO); END_TYPE
FUNCTION_BLOCK FB_A
METHOD GetCount : INT
END_METHOD
METHOD GetMode : E_Mode
END_METHOD
METHOD GetText : STRING
END_METHOD
METHOD Early : DINT
RETURN;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION Pick : REAL
END_FUNCTION

PROGRAM Main
VAR
    fb : FB_A;
    count : INT := 7;
    isManual : BOOL;
    text : STRING := 'x';
    early : DINT := 9;
    picked : REAL := 1.5;
END_VAR
count := fb.GetCount();
isManual := fb.GetMode() = E_Mode#MANUAL;
text := fb.GetText();
early := fb.Early();
picked := Pick();
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("count"), Some(Value::Int(0)));
    assert_eq!(harness.get_output("isManual"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("text"), Some(Value::String("".into())));
    assert_eq!(harness.get_output("early"), Some(Value::DInt(0)));
    assert_eq!(harness.get_output("picked"), Some(Value::Real(0.0)));
}

// DEV-028: under the CODESYS profile another POU writes and reads a function block's `VAR` member
// through the instance. Worked by hand: Main sets `stored` to 10, the call adds 1, Main reads 11.
// Catches the write not reaching the instance (the call would read 0 and give 1) and the read
// refused by the build.
#[test]
fn a_function_block_member_is_written_and_read_from_outside() {
    let harness = codesys_run(
        r#"
FUNCTION_BLOCK Device
VAR
    stored : INT;
END_VAR
stored := stored + 1;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    device : Device;
    seen : INT;
END_VAR
device.stored := 10;
device();
seen := device.stored;
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("seen"), Some(Value::Int(11)));
}

// DEV-030: under the CODESYS profile a program is one instance named by its type. Without a
// CONFIGURATION each program runs as one instance of its name (specification 18 §2.2), and
// `Producer.counter` is that instance's variable. Worked by hand: Producer's body is empty, so
// each of Main's cycles adds 10: 10 after one cycle, 20 after two. Catches `Producer.counter`
// built as a variable of its own (Main would see its own 10 each cycle) and the access refused.
#[test]
fn a_program_variable_is_reached_by_the_program_name() {
    let harness = codesys_run(
        r#"
PROGRAM Producer
VAR
    counter : INT;
END_VAR
END_PROGRAM

PROGRAM Main
VAR
    seen : INT;
END_VAR
Producer.counter := Producer.counter + 10;
seen := Producer.counter;
END_PROGRAM
"#,
        2,
    );
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(20)));
    assert_eq!(harness.get_output("Producer.counter"), Some(Value::Int(20)));
}

// The `REF=` rebinding of specification 06 with its source through the program's name: the shape a
// configuration uses to hand each consumer the storage it binds behind its reference.
const CROSS_PROGRAM_REF_SOURCE: &str = r#"
PROGRAM Producer
VAR
    n : INT := 5;
END_VAR
END_PROGRAM

PROGRAM Main
VAR
    r : REFERENCE TO INT;
    seen : INT;
END_VAR
r REF= Producer.n;
seen := r^;
r^ := 7;
END_PROGRAM
"#;

// DEV-030 with the `REF=` extension (specification 06): the rebinding names the other program's
// variable, not a copy of it. Worked by hand: Main binds r to Producer.n (5), reads 5, and the
// write through r^ lands in Producer's own storage. Catches the source built as a copy (Producer.n
// stays 5) and the qualified source refused by the build.
#[test]
fn a_reference_rebinds_to_another_programs_variable() {
    let harness = codesys_run(CROSS_PROGRAM_REF_SOURCE, 1);
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(5)));
    assert_eq!(harness.get_output("Producer.n"), Some(Value::Int(7)));
}

// The segmented shapes: an indexed function-block array element's member as the source, and a
// global array element's reference-typed member as the target (the event-wiring idiom). Worked by
// hand: afb[2].stClick.count = 42 and stLocal.count = 41, so Main reads 42 through rLocal and 41
// through the global holder's reference. Catches a segmented or indexed source refused, or the
// global target's rebinding stored anywhere but the global's own storage.
#[test]
fn a_reference_rebinds_through_segmented_and_indexed_paths() {
    let harness = codesys_run(
        r#"
TYPE Event : STRUCT count : INT; END_STRUCT END_TYPE
TYPE Click : STRUCT stClick : Event; END_STRUCT END_TYPE
TYPE Holder : STRUCT pEvent : REFERENCE TO Event; END_STRUCT END_TYPE

VAR_GLOBAL
    entities : ARRAY[0..1] OF Holder;
END_VAR

PROGRAM Producer
VAR
    stLocal : Event;
    afb : ARRAY[0..3] OF Click;
END_VAR
stLocal.count := 41;
afb[2].stClick.count := 42;
END_PROGRAM

PROGRAM Main
VAR
    rLocal : REFERENCE TO Event;
    seen : INT;
    viaGlobal : INT;
END_VAR
rLocal REF= Producer.afb[2].stClick;
seen := rLocal^.count;
entities[0].pEvent REF= Producer.stLocal;
viaGlobal := entities[0].pEvent^.count;
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(42)));
    assert_eq!(harness.get_output("Main.viaGlobal"), Some(Value::Int(41)));
}

// DEV-030: the IEC rules have no access to a program through its name (IEC 61131-3 Ed.3 §6.6.4
// item 3), so its `REF=` source is refused too — the CODESYS twin above builds the same source.
// Catches the profile rule bypassed by the rebinding.
#[test]
fn the_iec_rules_refuse_a_reference_bound_to_another_programs_variable() {
    let Err(error) =
        TestHarness::from_session(CompileSession::from_source(CROSS_PROGRAM_REF_SOURCE))
    else {
        panic!("the IEC rules refuse `r REF= Producer.n`");
    };
    let text = error.to_string();
    assert!(text.contains("Producer"), "{text}");
}

// DEV-030: under the CODESYS profile `Doubler(x := 21)` runs Doubler's body with that input.
// Worked by hand: y = 21 * 2 = 42 whichever program the cycle runs first (Doubler's own run keeps
// the input 21 Main passed). Catches the call refused, or run without its argument (y = 0).
#[test]
fn a_program_is_called_by_its_name() {
    let harness = codesys_run(
        r#"
PROGRAM Doubler
VAR_INPUT
    x : INT;
END_VAR
VAR_OUTPUT
    y : INT;
END_VAR
y := x * 2;
END_PROGRAM

PROGRAM Main
VAR
    seen : INT;
END_VAR
Doubler(x := 21);
seen := Doubler.y;
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(42)));
}

// DEV-031: under the CODESYS profile a reference in value position is the value it references.
// Worked by hand: `r.x := 7` writes p.x, read back 7; `ra[2]` is a[2] = 3; `copied := rn` reads
// n = 4; `rn := 9` writes n; `sum := rn + 1` is 9 + 1 = 10. Catches a reference copied instead of
// dereferenced, a write landing in the reference instead of the variable, and the build refusing.
#[test]
fn references_are_dereferenced_without_a_caret() {
    let harness = codesys_run(
        r#"
TYPE Point : STRUCT x : INT; y : INT; END_STRUCT END_TYPE
TYPE Triple : ARRAY[0..2] OF INT; END_TYPE

PROGRAM Main
VAR
    p : Point;
    r : REFERENCE TO Point;
    a : Triple := [1, 2, 3];
    ra : REFERENCE TO Triple;
    n : INT := 4;
    rn : REF_TO INT;
    viaField : INT;
    viaIndex : INT;
    copied : INT;
    sum : INT;
END_VAR
r REF= p;
r.x := 7;
viaField := r.x;
ra REF= a;
viaIndex := ra[2];
rn REF= n;
copied := rn;
rn := 9;
sum := rn + 1;
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("viaField"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("p.x"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("viaIndex"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("copied"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("n"), Some(Value::Int(9)));
    assert_eq!(harness.get_output("sum"), Some(Value::Int(10)));
}

// DEV-032: under the CODESYS profile an input bound to nothing keeps its value and an output bound
// to nothing is discarded. Worked by hand: `factor` keeps its initial 3, so result = 2 * 3 = 6;
// `doubled` goes nowhere, so `d` keeps -1. Catches the empty input taken as 0 (result 0) and the
// build refusing the empty bindings.
#[test]
fn an_empty_binding_leaves_the_input_and_discards_the_output() {
    let harness = codesys_run(
        r#"
FUNCTION_BLOCK Scale
VAR_INPUT
    value : INT;
    factor : INT := 3;
END_VAR
VAR_OUTPUT
    result : INT;
    doubled : INT;
END_VAR
result := value * factor;
doubled := value * 2;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    scale : Scale;
    r : INT;
    d : INT := -1;
END_VAR
scale(value := 2, factor := , result => r, doubled => );
END_PROGRAM
"#,
        1,
    );
    assert_eq!(harness.get_output("r"), Some(Value::Int(6)));
    assert_eq!(harness.get_output("d"), Some(Value::Int(-1)));
}

// PR #140's extensions that change no IEC program build under both profiles: `REF=`, the
// `REFERENCE TO` spelling, `EnumType.Member`, an enum-typed subscript and `TYPE … EXTENDS` on a
// structure. Worked by hand: `Axis.Y` is the second value, so `table[which]` is table[1] = 20;
// `base := derived` copies the base part, a = 5. Catches an extension that only the analysis
// accepts and the build refuses or computes differently.
#[test]
fn the_extensions_build_and_run_under_both_profiles() {
    let source = r#"
TYPE Axis : (X, Y, Z); END_TYPE
TYPE Base : STRUCT a : INT; END_STRUCT END_TYPE
TYPE Derived EXTENDS Base : STRUCT b : INT; END_STRUCT END_TYPE

PROGRAM Main
VAR
    table : ARRAY[0..2] OF INT := [10, 20, 30];
    which : Axis := Axis.Y;
    picked : INT;
    derived : Derived;
    base : Base;
    gotA : INT;
    n : INT := 4;
    rn : REFERENCE TO INT;
    viaDeref : INT;
END_VAR
picked := table[which];
derived.a := 5;
derived.b := 6;
base := derived;
gotA := base.a;
rn REF= n;
viaDeref := rn^;
END_PROGRAM
"#;
    for conversions in [ConversionProfile::Iec, ConversionProfile::Codesys] {
        let session = CompileSession::from_source(source).with_conversion_profile(conversions);
        let mut harness = TestHarness::from_session(session).expect("both profiles build it");
        harness.cycle();
        assert_eq!(
            harness.get_output("picked"),
            Some(Value::Int(20)),
            "{conversions:?}"
        );
        assert_eq!(
            harness.get_output("gotA"),
            Some(Value::Int(5)),
            "{conversions:?}"
        );
        assert_eq!(
            harness.get_output("viaDeref"),
            Some(Value::Int(4)),
            "{conversions:?}"
        );
    }
}

// Specification 02 §4.1: `__ISVALIDREF(ref)` is TRUE once a reference is bound and FALSE while it is
// NULL, for a `REFERENCE TO` and a `REF_TO`, under the CODESYS and the IEC profiles. Worked by hand:
// both references start NULL (FALSE, FALSE over flags that started TRUE), are bound in the cycle
// (TRUE, TRUE), and `byRef := NULL` unbinds one again (FALSE). Catches the test inverted, NULL taken
// as bound, and the result fixed by the first binding.
const IS_VALID_REF: &str = r#"
PROGRAM Main
VAR
    value : INT;
    byName : REFERENCE TO INT;
    byRef : REF_TO INT;
    nameBefore : BOOL := TRUE;
    refBefore : BOOL := TRUE;
    nameAfter : BOOL;
    refAfter : BOOL;
    refCleared : BOOL := TRUE;
END_VAR
nameBefore := __ISVALIDREF(byName);
refBefore := __ISVALIDREF(byRef);
byName REF= value;
byRef := REF(value);
nameAfter := __ISVALIDREF(byName);
refAfter := __ISVALIDREF(byRef);
byRef := NULL;
refCleared := __ISVALIDREF(byRef);
END_PROGRAM
"#;

#[test]
fn is_valid_ref_tells_a_bound_reference_from_null() {
    for profile in [ConversionProfile::Codesys, ConversionProfile::Iec] {
        let session = CompileSession::from_source(IS_VALID_REF).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{profile:?}: {:?}", cycle.errors);
        for (name, expected) in [
            ("nameBefore", false),
            ("refBefore", false),
            ("nameAfter", true),
            ("refAfter", true),
            ("refCleared", false),
        ] {
            assert_eq!(
                harness.get_output(name),
                Some(Value::Bool(expected)),
                "{profile:?}: {name}"
            );
        }
    }
}

// Specification 04: `__QUERYINTERFACE(source, target)` is `target ?= source` then `target <> NULL`,
// under the CODESYS and the IEC profiles. Worked by hand: the lamp implements IDimmer, so the first
// query is TRUE and the dimmer gives the lamp's level 3; it does not implement IFan, so the second
// query is FALSE and the fan, which named another block, is NULL afterwards. Catches the answer
// inverted, the target left unchanged on a failed query (CODESYS sets it to 0), and a query that
// never names the instance.
const QUERY_INTERFACE: &str = r#"
INTERFACE ILamp
METHOD Toggle : BOOL
END_METHOD
END_INTERFACE

INTERFACE IDimmer
METHOD Dim : INT
END_METHOD
END_INTERFACE

INTERFACE IFan
METHOD Spin : BOOL
END_METHOD
END_INTERFACE

FUNCTION_BLOCK FB_Lamp IMPLEMENTS ILamp, IDimmer
VAR
    level : INT := 3;
END_VAR
METHOD PUBLIC Toggle : BOOL
Toggle := TRUE;
END_METHOD
METHOD PUBLIC Dim : INT
Dim := level;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Fan IMPLEMENTS IFan
METHOD PUBLIC Spin : BOOL
Spin := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    lamp : FB_Lamp;
    fanBlock : FB_Fan;
    lampItf : ILamp;
    dimmer : IDimmer;
    fan : IFan;
    found : BOOL;
    level : INT;
    missing : BOOL := TRUE;
    fanCleared : BOOL;
END_VAR
lampItf := lamp;
found := __QUERYINTERFACE(lampItf, dimmer);
IF found THEN
    level := dimmer.Dim();
END_IF
fan := fanBlock;
missing := __QUERYINTERFACE(lampItf, fan);
fanCleared := fan = NULL;
END_PROGRAM
"#;

#[test]
fn query_interface_answers_and_sets_the_target() {
    for profile in [ConversionProfile::Codesys, ConversionProfile::Iec] {
        let session = CompileSession::from_source(QUERY_INTERFACE).with_conversion_profile(profile);
        let mut harness = TestHarness::from_session(session)
            .unwrap_or_else(|error| panic!("{profile:?}: {error}"));
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{profile:?}: {:?}", cycle.errors);
        assert_eq!(
            harness.get_output("found"),
            Some(Value::Bool(true)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("level"),
            Some(Value::Int(3)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("missing"),
            Some(Value::Bool(false)),
            "{profile:?}"
        );
        assert_eq!(
            harness.get_output("fanCleared"),
            Some(Value::Bool(true)),
            "{profile:?}"
        );
    }
}

// Specification 10 §4.8: under the CODESYS profiles the members an instance's declaration
// initializes hold their type's initial values while FB_init runs and take the declaration's values
// right after it, as CODESYS orders them (its help, Example 259: `fbDoIt.FB_Init(); fbDoIt.in1 :=
// 99; fbDoIt.FB_AfterInit();`; Beckhoff: "Such assignments are only executed after FB_init has been
// called"). Worked by hand: `in1` starts at 5 in its type and the declaration gives 99. At the cold
// restart FB_init sees 5 and sets 105, and the declaration's 99 replaces it; at the warm restart
// FB_init sees 5 again and, as `bInitRetains` is FALSE, leaves it, and the declaration's 99 follows.
// `other`, which the declaration does not name, keeps the 7 FB_init writes. `kept`, a RETAIN
// instance, holds the declaration's 99 when its FB_init runs at the cold restart, as Beckhoff orders
// retained variables, so FB_init sees 99; it keeps the 42 the program's first scan gives it over the
// warm restart, whose FB_init then sees 42. Catches FB_init seeing the declaration's value (99), the
// declaration's value lost to FB_init's write (199 or 105), a retained member written around
// FB_init (5 seen), and a warm restart that resets a kept member (99).
const DECLARATION_AFTER_FB_INIT: &str = r#"
FUNCTION_BLOCK FB_Order
VAR_INPUT
    in1 : INT := 5;
    other : INT := 1;
END_VAR
VAR_OUTPUT
    seenInInit : INT;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
END_VAR
seenInInit := in1;
IF bInitRetains THEN
    in1 := in1 + 100;
END_IF;
other := 7;
FB_init := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    fb : FB_Order := (in1 := 99);
    seen : INT;
    in1Final : INT;
    otherFinal : INT;
    keptSeen : INT;
    keptFinal : INT;
END_VAR
VAR RETAIN
    kept : FB_Order := (in1 := 99);
    scans : INT;
END_VAR
scans := scans + 1;
IF scans = 1 THEN
    kept.in1 := 42;
END_IF;
seen := fb.seenInInit;
in1Final := fb.in1;
otherFinal := fb.other;
keptSeen := kept.seenInInit;
keptFinal := kept.in1;
END_PROGRAM
"#;

#[test]
fn the_declaration_initializes_its_members_after_fb_init() {
    let session = CompileSession::from_source(DECLARATION_AFTER_FB_INIT)
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    for (restart, kept_seen) in [("cold", 99), ("warm", 42)] {
        if restart == "warm" {
            harness
                .restart(RestartMode::Warm)
                .expect("the warm restart runs");
        }
        let cycle = harness.cycle();
        assert!(cycle.errors.is_empty(), "{restart}: {:?}", cycle.errors);
        assert_eq!(harness.get_output("seen"), Some(Value::Int(5)), "{restart}");
        assert_eq!(
            harness.get_output("in1Final"),
            Some(Value::Int(99)),
            "{restart}"
        );
        assert_eq!(
            harness.get_output("otherFinal"),
            Some(Value::Int(7)),
            "{restart}"
        );
        assert_eq!(
            harness.get_output("keptSeen"),
            Some(Value::Int(kept_seen)),
            "{restart}: what the RETAIN instance's FB_init sees"
        );
        assert_eq!(
            harness.get_output("keptFinal"),
            Some(Value::Int(42)),
            "{restart}: the RETAIN instance keeps its member"
        );
    }
}
