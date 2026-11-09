//! Instances that live for one call (specification 10 §4.8): a function's or a method's locals and a program's
//! `VAR_TEMP`, in a module of their own so that the CODESYS profile's test program stays below the 1,000 lines of a
//! file.

use super::*;

/// `FB_Local` writes its FB_init argument into `trace` when it starts and that id plus five when it leaves, and
/// counts the calls that pass `bInitRetains` or `bInCopyCode` TRUE.
const FB_LOCAL: &str = r#"
VAR_GLOBAL
    trace : LINT;
    retaining : INT;
    copying : INT;
END_VAR

FUNCTION_BLOCK FB_Local
VAR
    id : LINT;
END_VAR
METHOD FB_init : BOOL
VAR_INPUT
    bInitRetains : BOOL;
    bInCopyCode : BOOL;
    nId : LINT;
END_VAR
id := nId;
trace := trace * 10 + nId;
IF bInitRetains THEN
    retaining := retaining + 1;
END_IF
IF bInCopyCode THEN
    copying := copying + 1;
END_IF
FB_init := TRUE;
END_METHOD
METHOD FB_exit : BOOL
VAR_INPUT
    bInCopyCode : BOOL;
END_VAR
trace := trace * 10 + id + 5;
IF bInCopyCode THEN
    copying := copying + 1;
END_IF
FB_exit := TRUE;
END_METHOD
END_FUNCTION_BLOCK
"#;

/// The program of `main` with `FB_Local`.
fn with_local(main: &str) -> String {
    format!("{FB_LOCAL}\n{main}")
}

/// A function whose two local instances start and leave at each call, also through `RETURN`.
const TWICE: &str = r#"
FUNCTION Twice : INT
VAR_INPUT
    early : BOOL;
END_VAR
VAR
    a : FB_Local(nId := 1);
    b : FB_Local(nId := 2);
END_VAR
IF early THEN
    Twice := 1;
    RETURN;
END_IF
Twice := 2;
END_FUNCTION

PROGRAM Main
VAR
    r : INT;
    traceSeen : LINT;
    retainingSeen : INT;
    copyingSeen : INT;
END_VAR
trace := 0;
r := Twice(early := FALSE);
r := Twice(early := TRUE);
traceSeen := trace;
retainingSeen := retaining;
copyingSeen := copying;
END_PROGRAM
"#;

// Specification 10 §4.8: an instance a function declares lives for one call: under the CODESYS profiles each call
// calls its FB_init(bInitRetains := FALSE, bInCopyCode := FALSE) after its starting values and its FB_exit(FALSE)
// when the call returns, at its end or at a RETURN, in the reverse order. Worked by hand: each call of `Twice` starts
// a (1) and b (2) and leaves b (7) and a (6), so a cycle that calls it once to its end and once through RETURN writes
// 12761276, with no retains and no copies, and the second cycle the same. Catches a local instance started once and
// kept (a second cycle without 1, 2), FB_exit skipped at a RETURN (127612 at most), the exits in the starts' order
// (12671267), and flags of a start of the program (retains 4).
#[test]
fn a_function_starts_and_leaves_its_local_instances_at_each_call() {
    let session = CompileSession::from_source(with_local(TWICE))
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    for cycle in 1..=2 {
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(
            [
                harness.get_output("traceSeen"),
                harness.get_output("retainingSeen"),
                harness.get_output("copyingSeen")
            ],
            [
                Some(Value::LInt(12_761_276)),
                Some(Value::Int(0)),
                Some(Value::Int(0))
            ],
            "cycle {cycle}"
        );
    }
}

// Specification 10 §4.8: under the IEC profile FB_init and FB_exit are ordinary methods, so the same program's
// function locals start and leave without them. Worked by hand: trace stays 0. Catches the implicit calls made under
// every profile.
#[test]
fn local_instances_call_nothing_under_the_iec_profile() {
    // The IEC rules take no FB_init arguments in a declaration, so the instances here have none.
    let source = with_local(TWICE)
        .replace("FB_Local(nId := 1)", "FB_Local")
        .replace("FB_Local(nId := 2)", "FB_Local");
    let session = CompileSession::from_source(source);
    let mut harness = TestHarness::from_session(session).expect("the IEC profile builds it");
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    assert_eq!(harness.get_output("traceSeen"), Some(Value::LInt(0)));
}

// Specification 10 §4.8: a program's VAR_TEMP instance starts at each scan with its FB_init and leaves at the scan's
// end with its FB_exit, and `{attribute 'no-exit'}` leaves a marked one out. Worked by hand: the first scan starts t
// (3) and q (4), so Main sees 34, and leaves t (8) but not q; the second scan starts both again, so Main sees 34834.
// Catches VAR_TEMP instances started once (348 at most), t's exit lost at the scan's end (3434), and q's exit made
// despite its mark (a 9).
#[test]
fn a_var_temp_instance_starts_and_leaves_at_each_scan() {
    let main = r#"
PROGRAM Main
VAR_TEMP
    t : FB_Local(nId := 3);
    {attribute 'no-exit'}
    q : FB_Local(nId := 4);
END_VAR
VAR
    traceSeen : LINT;
END_VAR
traceSeen := trace;
END_PROGRAM
"#;
    let session = CompileSession::from_source(with_local(main))
        .with_conversion_profile(ConversionProfile::Codesys);
    let mut harness = TestHarness::from_session(session).expect("the CODESYS profile builds it");
    for (cycle, seen) in [(1, 34), (2, 34_834)] {
        let result = harness.cycle();
        assert!(result.errors.is_empty(), "{:?}", result.errors);
        assert_eq!(
            harness.get_output("traceSeen"),
            Some(Value::LInt(seen)),
            "cycle {cycle}"
        );
    }
}
