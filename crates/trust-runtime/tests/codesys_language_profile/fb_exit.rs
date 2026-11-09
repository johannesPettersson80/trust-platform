//! CODESYS's implicit `FB_exit` (specification 10 §4.8, specification 44 §6.7), in a module of its own so
//! that the CODESYS profile's test program stays below the 1,000 lines of a file.

use super::*;

// Specification 10 §4.8: under the CODESYS profiles a restart of a program that has run first calls
// FB_exit(bInCopyCode := FALSE) of every instance whose type declares it, in the reverse of
// FB_init's order, before the memory starts again, so that what FB_exit writes into a `noinit`
// variable stays. Each FB_exit writes a digit into the `noinit` global `trace`: FB_Leaf its `id`,
// FB_Base 2, FB_Derived 3, FB_Kept 4. Worked by hand from the declarations: Main's variables in
// reverse, `grid` in reverse row-major order with the ids the scan gives it (9, 8, 7, 6), then
// `derived`: its own FB_exit (3), its own nested `leaf` declared with id 5 (5), its base part (2);
// then the globals in reverse, `gLeaf` (1) and the retained `kept` (4). So 987635214, nine calls,
// none with bInCopyCode TRUE. The starts that bring the program up, the load's power-on start and
// the cold start after it, call nothing before its first scan (0); the warm restart gives 987635214
// and the cold restart after it appends the same nine digits. Catches
// FB_init's order kept (412536789), an array walked forward (6789), a block's members or base
// before its own FB_exit, an instance left out (a retained or a plain global, the nested one, an
// element, the base part), the exit after the memory started again (the grid's ids back at 1), the
// exit's writes lost to the `noinit` bytes taken before it (0), a start before the first scan that
// exits (it found 111135214 with the grid's ids still 1), a restart that does not, and bInCopyCode
// passed TRUE.
const FB_EXIT_ORDER: &str = r#"
VAR_GLOBAL RETAIN
    kept : FB_Kept;
END_VAR
VAR_GLOBAL
    gLeaf : FB_Leaf;
    {attribute 'noinit'}
    trace : LINT;
    {attribute 'noinit'}
    calls : INT;
    {attribute 'noinit'}
    copying : INT;
END_VAR

FUNCTION Note : BOOL
VAR_INPUT
    digit : LINT;
    copies : BOOL;
END_VAR
trace := trace * 10 + digit;
calls := calls + 1;
IF copies THEN
    copying := copying + 1;
END_IF
Note := TRUE;
END_FUNCTION

FUNCTION_BLOCK FB_Leaf
VAR_INPUT
    id : LINT := 1;
END_VAR
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
FB_exit := Note(id, bInCopyCode);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Kept
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
FB_exit := Note(4, bInCopyCode);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Base
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
FB_exit := Note(2, bInCopyCode);
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    leaf : FB_Leaf := (id := 5);
END_VAR
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
FB_exit := Note(3, bInCopyCode);
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    derived : FB_Derived;
    grid : ARRAY[1..2, 1..2] OF FB_Leaf;
    traceSeen : LINT;
    callsSeen : INT;
    copyingSeen : INT;
END_VAR
grid[1, 1].id := 6;
grid[1, 2].id := 7;
grid[2, 1].id := 8;
grid[2, 2].id := 9;
traceSeen := trace;
callsSeen := calls;
copyingSeen := copying;
END_PROGRAM
"#;

/// What Main saw of the exits in its last cycle: the trace, the calls and the calls that passed
/// bInCopyCode TRUE.
fn exits_seen(harness: &mut TestHarness) -> [Option<Value>; 3] {
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    [
        harness.get_output("traceSeen"),
        harness.get_output("callsSeen"),
        harness.get_output("copyingSeen"),
    ]
}

#[test]
fn fb_exit_runs_for_every_instance_in_reverse_order_before_a_restart() {
    let session = CompileSession::from_source(FB_EXIT_ORDER)
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    assert_eq!(
        exits_seen(&mut harness),
        [
            Some(Value::LInt(0)),
            Some(Value::Int(0)),
            Some(Value::Int(0))
        ],
        "the first start after the install"
    );
    harness
        .restart(RestartMode::Warm)
        .expect("the warm restart runs");
    assert_eq!(
        exits_seen(&mut harness),
        [
            Some(Value::LInt(987_635_214)),
            Some(Value::Int(9)),
            Some(Value::Int(0))
        ],
        "the warm restart"
    );
    harness
        .restart(RestartMode::Cold)
        .expect("the cold restart runs");
    assert_eq!(
        exits_seen(&mut harness),
        [
            Some(Value::LInt(987_635_214_987_635_214)),
            Some(Value::Int(18)),
            Some(Value::Int(0))
        ],
        "the cold restart"
    );
}

// Specification 10 §4.8: a download calls the running program's FB_exit before the installed one
// replaces it, as a restart does. Worked by hand as above: after the first scan the exit writes
// 987635214 in nine calls, none with bInCopyCode TRUE, and the program, which this test keeps,
// sees them in its next scan. Catches a download that drops the program without its exit.
#[test]
fn a_download_leaves_the_running_program_first() {
    let session = CompileSession::from_source(FB_EXIT_ORDER)
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    exits_seen(&mut harness);
    harness
        .runtime_mut()
        .leave_program()
        .expect("the exit runs");
    assert_eq!(
        exits_seen(&mut harness),
        [
            Some(Value::LInt(987_635_214)),
            Some(Value::Int(9)),
            Some(Value::Int(0))
        ]
    );
}

// Specification 10 §4.8: under the IEC profile FB_exit is an ordinary method that nothing calls
// implicitly, since a valid IEC program may declare a method of that name. The same program built
// under the IEC profile keeps `trace` at 0 over a warm and a cold restart. Catches the implicit
// call made under every profile.
#[test]
fn fb_exit_is_an_ordinary_method_under_the_iec_profile() {
    let session = CompileSession::from_source(FB_EXIT_ORDER);
    let mut harness = TestHarness::from_session(session).expect("the IEC profile builds it");
    exits_seen(&mut harness);
    for mode in [RestartMode::Warm, RestartMode::Cold] {
        harness.restart(mode).expect("the restart runs");
        assert_eq!(
            exits_seen(&mut harness),
            [
                Some(Value::LInt(0)),
                Some(Value::Int(0)),
                Some(Value::Int(0))
            ],
            "{mode:?}"
        );
    }
}

// Specification 10 §4.8: a declaration marked {attribute 'no-exit'} leaves out the FB_exit of the
// instances it declares, their base parts' too (CODESYS help, "Attribute: no-exit"); an instance
// nested in one of them keeps the mark of its own declaration. Worked by hand at a restart, Main's
// variables in reverse: `holder`, whose block declares no FB_exit, leaves its members in reverse,
// `loud` (7) and the marked `quiet` (none); the marked array `hiddenRow` (none for either
// element); the marked `hidden`, whose own FB_exit (3) and base part (2) are left out, while its
// nested `leaf` (5) leaves; `shown` (1); then the marked global `gQuiet` (none). So 751. Catches
// the mark ignored (a 4, 6, 3, 2 or an element's 1), a mark in a block's type ignored (6), a mark
// taken for the instances nested in the marked one (no 5), and a marked instance's base part called
// (2).
#[test]
fn no_exit_leaves_out_the_instances_its_declaration_declares() {
    let source = r#"
VAR_GLOBAL
    {attribute 'noinit'}
    trace : LINT;
    {attribute 'no-exit'}
    gQuiet : FB_Leaf := (id := 4);
END_VAR

FUNCTION_BLOCK FB_Leaf
VAR_INPUT
    id : LINT := 1;
END_VAR
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
trace := trace * 10 + id;
FB_exit := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Base
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
trace := trace * 10 + 2;
FB_exit := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
VAR
    leaf : FB_Leaf := (id := 5);
END_VAR
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
trace := trace * 10 + 3;
FB_exit := TRUE;
END_METHOD
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Holder
VAR
    {attribute 'no-exit'}
    quiet : FB_Leaf := (id := 6);
    loud : FB_Leaf := (id := 7);
END_VAR
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    shown : FB_Leaf;
    {attribute 'no-exit'}
    hidden : FB_Derived;
    {attribute 'no-exit'}
    hiddenRow : ARRAY[1..2] OF FB_Leaf;
    holder : FB_Holder;
    traceSeen : LINT;
END_VAR
traceSeen := trace;
END_PROGRAM
"#;
    let session =
        CompileSession::from_source(source).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    harness.cycle();
    harness
        .restart(RestartMode::Warm)
        .expect("the warm restart runs");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    assert_eq!(harness.get_output("traceSeen"), Some(Value::LInt(751)));
}

// Specification 10 §4.8: a fault in FB_exit fails the restart, like a fault of the initialization.
// FB_exit divides by a variable that holds 0, so the warm restart of the started program reports a
// division by zero. Catches the exit's fault dropped at a restart.
#[test]
fn a_fault_in_fb_exit_fails_the_restart() {
    let source = r#"
FUNCTION_BLOCK FB_Faulty
VAR
    zero : INT;
    ratio : INT;
END_VAR
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
ratio := 10 / zero;
FB_exit := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    faulty : FB_Faulty;
END_VAR
END_PROGRAM
"#;
    let session =
        CompileSession::from_source(source).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    harness.cycle();
    let error = harness
        .restart(RestartMode::Warm)
        .expect_err("the exit's fault fails the restart");
    assert!(
        error
            .to_string()
            .to_lowercase()
            .contains("division by zero"),
        "{error}"
    );
}

// Specification 10 §4.8: FB_exit declares the one BOOL input bInCopyCode and returns a BOOL or
// nothing; the build refuses another signature (specification 44 NX-09) rather than call it with
// an input left empty. Catches the second input ignored.
#[test]
fn a_build_refuses_an_fb_exit_with_another_signature() {
    let source = r#"
FUNCTION_BLOCK FB_Wide
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
    nReason : INT;
END_VAR
FB_exit := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    wide : FB_Wide;
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
            .contains("FB_Wide.FB_exit with other inputs than the BOOL one bInCopyCode"),
        "{error}"
    );
}

// Specification 10 §4.8, specification 44 §6.7: an online change takes over the running program's
// instances, which have lived, so a restart right after it, before any scan, leaves them first.
// Worked by hand as in the first test: the first scan gives the grid its ids, the online change to
// the same program carries every value, and the cold restart that follows at once calls the nine
// FB_exit, 987635214. Catches the instances taken for new ones after an online change (0).
#[test]
fn a_restart_after_an_online_change_leaves_the_instances_taken_over() {
    let session = CompileSession::from_source(FB_EXIT_ORDER)
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    exits_seen(&mut harness);
    harness
        .reload_source(FB_EXIT_ORDER)
        .expect("the online change runs");
    harness
        .restart(RestartMode::Cold)
        .expect("the cold restart runs");
    assert_eq!(
        exits_seen(&mut harness),
        [
            Some(Value::LInt(987_635_214)),
            Some(Value::Int(9)),
            Some(Value::Int(0))
        ]
    );
}
