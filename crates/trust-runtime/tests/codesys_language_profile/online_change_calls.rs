//! CODESYS's calls at an online change (specification 10 §4.8, specification 44 §12.3), in a module of
//! their own so that the CODESYS profile's test program stays below the 1,000 lines of a file.

use super::*;

/// The running program: `FB_Leaf` takes its `id` from its `FB_init` argument and writes it into the
/// carried `trace` when it leaves, counting the calls that pass `bInCopyCode` TRUE.
const RUNNING: &str = r#"
VAR_GLOBAL
    trace : LINT;
    exitCopies : INT;
    g : FB_Leaf(nId := 3);
END_VAR

FUNCTION_BLOCK FB_Leaf
VAR_INPUT
    id : LINT;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
    nId : LINT;
END_VAR
id := nId;
FB_init := TRUE;
END_METHOD
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
trace := trace * 10 + id;
IF bInCopyCode THEN
    exitCopies := exitCopies + 1;
END_IF
FB_exit := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    a : FB_Leaf(nId := 1);
    b : FB_Leaf(nId := 2);
    gone : FB_Leaf(nId := 7);
END_VAR
END_PROGRAM
"#;

/// A changed program: `FB_Leaf` with the `FB_exit` of the running one, an `FB_init` and an
/// `FB_reinit` that write into globals only this program declares, which the carry leaves alone,
/// `members` as its declaration's further members (none, or a new one), and `program` as Main's
/// instances. Its `FB_init` sets `id` to its argument plus one, so that `FB_reinit`, which reads
/// `id`, sees the carried id only after the carry.
fn changed(members: &str, program: &str) -> String {
    format!(
        r#"
VAR_GLOBAL
    trace : LINT;
    exitCopies : INT;
    initTrace : LINT;
    initCopies : INT;
    initRetains : INT;
    reinitTrace : LINT;
    g : FB_Leaf(nId := 3);
END_VAR

FUNCTION_BLOCK FB_Leaf
VAR_INPUT
    id : LINT;
END_VAR
{members}
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
    nId : LINT;
END_VAR
id := nId + 1;
initTrace := initTrace * 10 + nId;
IF bInCopyCode THEN
    initCopies := initCopies + 1;
END_IF
IF bInitRetains THEN
    initRetains := initRetains + 1;
END_IF
FB_init := TRUE;
END_METHOD
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
trace := trace * 10 + id;
IF bInCopyCode THEN
    exitCopies := exitCopies + 1;
END_IF
FB_exit := TRUE;
END_METHOD
METHOD FB_reinit : BOOL
reinitTrace := reinitTrace * 10 + id;
FB_reinit := TRUE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
{program}
    traceSeen : LINT;
    exitCopiesSeen : INT;
    initSeen : LINT;
    initCopiesSeen : INT;
    initRetainsSeen : INT;
    reinitSeen : LINT;
END_VAR
traceSeen := trace;
exitCopiesSeen := exitCopies;
initSeen := initTrace;
initCopiesSeen := initCopies;
initRetainsSeen := initRetains;
reinitSeen := reinitTrace;
END_PROGRAM
"#
    )
}

/// Main's instances as the running program declares them.
const SAME_INSTANCES: &str =
    "    a : FB_Leaf(nId := 1);\n    b : FB_Leaf(nId := 2);\n    gone : FB_Leaf(nId := 7);";

/// What the changed program saw of the calls in its first cycle after the change from `RUNNING`
/// to `source`: the exit trace and its copies, the init trace, its copies and its retains, and
/// the reinit trace.
fn calls_at_change(source: &str) -> [Option<Value>; 6] {
    let session =
        CompileSession::from_source(RUNNING).with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    harness
        .reload_source(source)
        .expect("the online change runs");
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "{:?}", cycle.errors);
    [
        harness.get_output("traceSeen"),
        harness.get_output("exitCopiesSeen"),
        harness.get_output("initSeen"),
        harness.get_output("initCopiesSeen"),
        harness.get_output("initRetainsSeen"),
        harness.get_output("reinitSeen"),
    ]
}

// Specification 10 §4.8: an online change that changes a function block's declaration copies every
// variable of that type, and calls FB_exit(bInCopyCode := TRUE) of each in the running program in
// the reverse of FB_init's order, then FB_init(bInitRetains := FALSE, bInCopyCode := TRUE) of each
// in the new program in FB_init's order, then the carry, then FB_reinit() of each in FB_init's
// order (CODESYS help: "old_inst.FB_Exit(bInCopyCode := TRUE); new_inst.FB_Init(bInitRetains :=
// FALSE, bInCopyCode := TRUE); copy(&old_inst, &new_inst); new_inst.FB_Reinit();"). FB_Leaf gains
// `extra`. Worked by hand: the exits run over Main's variables in reverse, then the globals: gone
// 7, b 2, a 1, g 3, so the carried trace is 7213 with four copies; the inits run over the globals,
// then Main's variables: g 3, a 1, b 2, gone 7, so 3127, four copies, no retains; the reinits read
// the carried ids in the same order, 3127. Catches a change that calls nothing, FB_init's flags of
// a start (TRUE, FALSE), the exits in FB_init's order (3127) or the inits reversed (7213), FB_reinit
// before the carry (it would read the new FB_init's ids, 4238), and an instance left out.
#[test]
fn a_changed_declaration_calls_exit_init_and_reinit_of_each_copied_variable() {
    let source = changed("VAR\n    extra : INT;\nEND_VAR", SAME_INSTANCES);
    assert_eq!(
        calls_at_change(&source),
        [
            Some(Value::LInt(7213)),
            Some(Value::Int(4)),
            Some(Value::LInt(3127)),
            Some(Value::Int(4)),
            Some(Value::Int(0)),
            Some(Value::LInt(3127)),
        ]
    );
}

// Specification 10 §4.8: a change of the code only, whose declarations are the same, copies no
// variable, so nothing is called ("If no changes were made to the declaration part of a function
// block ..., but in the implementation only, then the data areas are not replaced. ... Then the
// methods FB_Exit, FB_Init, and FB_Reinit are not called."). The changed FB_Leaf has new method
// bodies and FB_reinit, and the program new globals and outputs, but FB_Leaf's members are the same.
// Worked by hand: every trace and count stays 0. Catches the cold-init's FB_init of every instance
// at a change (3127 with four retains), and a change that takes a new method or a new variable of
// the program for a changed declaration.
#[test]
fn a_change_of_the_code_only_calls_nothing() {
    let source = changed("", SAME_INSTANCES);
    assert_eq!(
        calls_at_change(&source),
        [
            Some(Value::LInt(0)),
            Some(Value::Int(0)),
            Some(Value::LInt(0)),
            Some(Value::Int(0)),
            Some(Value::Int(0)),
            Some(Value::LInt(0)),
        ]
    );
}

// Specification 10 §4.8: a variable only the running program declares is left with
// FB_exit(bInCopyCode := FALSE), and one only the new program declares starts with
// FB_init(bInitRetains := TRUE, bInCopyCode := FALSE), as at a download; neither is reinitialized,
// and the variables whose declaration is the same see nothing. Main drops `gone` and adds `fresh`.
// Worked by hand: trace 7 with no copy, init trace 8 with one retain and no copy, no reinit. Catches
// a dropped variable left without its exit or told it is copied, an added variable started as a
// copy or without its FB_init, and a reinit of an added variable.
#[test]
fn a_dropped_variable_leaves_and_an_added_one_starts_as_at_a_download() {
    let source = changed(
        "",
        "    a : FB_Leaf(nId := 1);\n    b : FB_Leaf(nId := 2);\n    fresh : FB_Leaf(nId := 8);",
    );
    assert_eq!(
        calls_at_change(&source),
        [
            Some(Value::LInt(7)),
            Some(Value::Int(0)),
            Some(Value::LInt(8)),
            Some(Value::Int(0)),
            Some(Value::Int(1)),
            Some(Value::LInt(0)),
        ]
    );
}
