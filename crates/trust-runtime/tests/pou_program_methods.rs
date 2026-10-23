use trust_runtime::harness::TestHarness;

// e!COCKPIT dialect: program-scoped METHODs and textual ACTIONs (spec 04 §4,
// §8). They lower as sub-bodies of the program singleton and dispatch
// through the program's method table — bare name from the program body, or
// qualified through the program name.

#[test]
fn program_method_bare_call_returns_value() {
    let source = r#"
PROGRAM Main
VAR
    flag : BOOL := FALSE;
    out : BOOL := FALSE;
END_VAR
    flag := StateToBool('1');
    out := flag;
METHOD PUBLIC StateToBool : BOOL
VAR_INPUT
    sInput : STRING;
END_VAR
    StateToBool := sInput = '1';
END_METHOD
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out", true);
}

#[test]
fn program_method_qualified_call_reads_program_state() {
    let source = r#"
PROGRAM Main
VAR
    count : INT := INT#20;
    out : INT := INT#0;
END_VAR
    out := Bump();
METHOD Bump : INT
    count := count + INT#1;
    Bump := count;
END_METHOD
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out", 21i16);
}

#[test]
fn program_action_bare_call_mutates_program_variables() {
    let source = r#"
PROGRAM Main
VAR
    fired : BOOL := FALSE;
    out : BOOL := FALSE;
END_VAR
    Reset();
    out := fired;
ACTION Reset:
    fired := TRUE;
END_ACTION
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out", true);
}

#[test]
fn fb_action_bare_call_runs_against_instance_variables() {
    let source = r#"
FUNCTION_BLOCK Muxer
VAR
    count : INT := INT#0;
END_VAR
    Bump();
    Bump();
ACTION Bump:
    count := count + INT#1;
END_ACTION
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    muxer : Muxer;
    out : INT := INT#0;
END_VAR
    muxer();
    out := muxer.count;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out", 2i16);
}
