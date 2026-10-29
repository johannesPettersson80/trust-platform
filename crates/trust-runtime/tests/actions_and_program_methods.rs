//! Textual ACTIONs and a program's METHODs in a compiled program (specification 04 §4 and §8,
//! specification 44 §6.4): an action is a method of its POU without parameters or result, run on
//! the owner's variables only when it is called; a program's method runs on the variables of the
//! program instance it is called on. The values are worked out by hand.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::{CompileSession, TestHarness};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Value;

/// The program of `session` run for `cycles` cycles, each without a runtime error.
fn run(session: CompileSession, cycles: usize) -> TestHarness {
    let mut harness = TestHarness::from_session(session).expect("the program builds");
    for _ in 0..cycles {
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
    }
    harness
}

// Specification 04 §8: the block's body calls `Bump` twice and the empty `Nothing` once; `Bump`
// adds 1 to `count` and sets `last` through `THIS` (rule 6). Worked by hand: each call of the block
// adds 2, so after two cycles count = 4 and last = 4 * 10 = 40. Catches the action not run (0), its
// statements also appended to the block's body (3 per cycle, 6), the instance's variables not kept
// between calls (2), and an empty action refused by the build.
#[test]
fn an_action_runs_on_its_function_block_instance_when_the_body_calls_it() {
    let harness = run(
        CompileSession::from_source(
            r#"
FUNCTION_BLOCK Muxer
VAR_OUTPUT
    count : INT;
    last : INT;
END_VAR
Bump();
Nothing();
Bump();
ACTION Bump:
    count := count + 1;
    THIS.last := count * 10;
END_ACTION
ACTION Nothing:
END_ACTION
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    muxer : Muxer;
    seenCount : INT;
    seenLast : INT;
END_VAR
muxer();
seenCount := muxer.count;
seenLast := muxer.last;
END_PROGRAM
"#,
        ),
        2,
    );
    assert_eq!(harness.get_output("seenCount"), Some(Value::Int(4)));
    assert_eq!(harness.get_output("seenLast"), Some(Value::Int(40)));
}

// Specification 04 §8 rule 10 and compilation boundary 2: `payload.Day()` runs only the action on
// the instance, and `payload()` runs only the block's own statements. Worked by hand: two action
// calls give days = 2, one block call gives calls = 1. Catches an action call that runs the block's
// body (calls = 3), the action's statements run as part of the body (days = 3), and the call
// through the instance refused.
#[test]
fn an_action_is_called_through_an_instance_apart_from_the_body() {
    let harness = run(
        CompileSession::from_source(
            r#"
FUNCTION_BLOCK Payload
VAR_OUTPUT
    calls : INT;
    days : INT;
END_VAR
calls := calls + 1;
ACTION Day:
    days := days + 1;
END_ACTION
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    payload : Payload;
    seenCalls : INT;
    seenDays : INT;
END_VAR
payload.Day();
payload.Day();
payload();
seenCalls := payload.calls;
seenDays := payload.days;
END_PROGRAM
"#,
        ),
        1,
    );
    assert_eq!(harness.get_output("seenCalls"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("seenDays"), Some(Value::Int(2)));
}

// Specification 04 §8: a block's statements before and after an action are its body, run in
// source order; the action between them runs only when called. Worked by hand: one call gives
// before = 1 and after = 1 + 2 = 3. Catches the statements after the action dropped (after = 0)
// and the uncalled action run with the body (before = 100).
#[test]
fn the_statements_around_an_action_are_the_function_block_body() {
    let harness = run(
        CompileSession::from_source(
            r#"
FUNCTION_BLOCK Split
VAR_OUTPUT
    before : INT;
    after : INT;
END_VAR
before := before + 1;
ACTION Unused:
    before := 100;
END_ACTION
after := before + 2;
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    split : Split;
    seenBefore : INT;
    seenAfter : INT;
END_VAR
split();
seenBefore := split.before;
seenAfter := split.after;
END_PROGRAM
"#,
        ),
        1,
    );
    assert_eq!(harness.get_output("seenBefore"), Some(Value::Int(1)));
    assert_eq!(harness.get_output("seenAfter"), Some(Value::Int(3)));
}

// Specification 04 §4 and §8 under the IEC rules: the program's body calls its action and its
// methods by their bare names, on its own variables. Worked by hand: `Reset` adds 1 to fired and,
// in the first cycle only, 10 to marker before its `RETURN` ends it; `Bump(5)` adds 5 to count,
// which starts at 20; `IsOne('1')` is TRUE; `Tally`'s `VAR_STAT` starts at 100 and keeps its value
// between calls (specification 01 §3). After two cycles fired = 2, marker = 10,
// count = 20 + 5 + 5 = 30, tally = 102. Catches a method's result lost (0), the method working on
// a copy of the program's variables (count = 25), the `VAR_STAT` started anew at each call (101)
// or without its starting value (2), the action not run, its `RETURN` ending the program's body
// too (seenFired would stay 1), and the action or the methods run with the program's body.
#[test]
fn a_program_calls_its_action_and_methods_by_their_names() {
    let harness = run(
        CompileSession::from_source(
            r#"
PROGRAM Main
VAR
    fired : INT;
    marker : INT;
    count : INT := 20;
    flag : BOOL;
    seenFired : INT;
    seenCount : INT;
    seenTally : INT;
END_VAR
Reset();
seenFired := fired;
seenCount := Bump(5);
flag := IsOne('1');
seenTally := Tally();
METHOD Bump : INT
VAR_INPUT
    amount : INT;
END_VAR
    count := count + amount;
    Bump := count;
END_METHOD
METHOD PUBLIC IsOne : BOOL
VAR_INPUT
    text : STRING;
END_VAR
    IsOne := text = '1';
END_METHOD
METHOD Tally : INT
VAR_STAT
    calls : INT := 100;
END_VAR
    calls := calls + 1;
    Tally := calls;
END_METHOD
ACTION Reset:
    fired := fired + 1;
    IF fired > 1 THEN
        RETURN;
    END_IF;
    marker := marker + 10;
END_ACTION
END_PROGRAM
"#,
        ),
        2,
    );
    assert_eq!(harness.get_output("seenFired"), Some(Value::Int(2)));
    assert_eq!(harness.get_output("marker"), Some(Value::Int(10)));
    assert_eq!(harness.get_output("seenCount"), Some(Value::Int(30)));
    assert_eq!(harness.get_output("count"), Some(Value::Int(30)));
    assert_eq!(harness.get_output("flag"), Some(Value::Bool(true)));
    assert_eq!(harness.get_output("seenTally"), Some(Value::Int(102)));
}

/// Another program calls Counter's method and action through Counter's name.
const CROSS_PROGRAM: &str = r#"
PROGRAM Counter
VAR
    count : INT;
END_VAR
METHOD Add : INT
VAR_INPUT
    amount : INT;
END_VAR
    count := count + amount;
    Add := count;
END_METHOD
ACTION Clear:
    count := 0;
END_ACTION
END_PROGRAM

PROGRAM Main
VAR
    seen : INT;
    cleared : INT;
END_VAR
seen := Counter.Add(3);
Counter.Add(4);
cleared := Counter.count;
Counter.Clear();
END_PROGRAM
"#;

// DEV-030: under the CODESYS profile `Counter.Add(…)` and `Counter.Clear()` run on the program
// instance named Counter (specification 18 §2.2: without a CONFIGURATION each program runs as one
// instance of its name). Counter's own body is empty, so the order of the programs in a cycle does
// not matter. Worked by hand: Add(3) returns 3, Add(4) makes count 7, Main reads 7, Clear sets 0.
// Catches the calls landing on a copy of Counter's variables (Main would read 0, or Counter.count
// stay 7) and the calls refused under the CODESYS profile.
#[test]
fn the_codesys_profile_calls_another_programs_method_and_action_by_its_name() {
    let harness = run(
        CompileSession::from_source(CROSS_PROGRAM)
            .with_conversion_profile(ConversionProfile::Codesys),
        1,
    );
    assert_eq!(harness.get_output("Main.seen"), Some(Value::Int(3)));
    assert_eq!(harness.get_output("Main.cleared"), Some(Value::Int(7)));
    assert_eq!(harness.get_output("Counter.count"), Some(Value::Int(0)));
}

// DEV-030: the IEC rules have no access to a program through its name (IEC 61131-3 Ed.3 §6.6.4
// item 3), and the CODESYS twin above builds the same source, so the refusal is the profile's.
// Catches the CODESYS rule applied to an IEC project.
#[test]
fn the_iec_rules_refuse_another_programs_method_by_its_name() {
    let Err(error) = TestHarness::from_session(CompileSession::from_source(CROSS_PROGRAM)) else {
        panic!("the IEC rules refuse `Counter.Add(3)`");
    };
    let text = error.to_string();
    assert!(text.contains("Counter"), "{text}");
}
