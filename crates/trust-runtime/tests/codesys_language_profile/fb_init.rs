//! CODESYS's implicit `FB_init` (specification 10 §4.8): when a restart calls it, in which order and with what, in a
//! module of its own so that the CODESYS profile's test program stays below the 1,000 lines of a file.

use super::*;

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
