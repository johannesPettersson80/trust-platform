use trust_runtime::harness::TestHarness;

// CODESYS textual ACTION sub-bodies (spec 04 §8): an `ACTION x: ...` inside
// a FUNCTION_BLOCK runs against the FB instance's own variables when
// invoked by bare name from the FB body, or qualified through an instance.

#[test]
fn fb_action_bare_call_runs_against_instance_variables() {
    let source = r#"
FUNCTION_BLOCK Muxer
VAR
    count : INT := INT#0;
    last : INT := INT#0;
END_VAR
    Bump();
    Bump();
ACTION Bump:
    count := count + INT#1;
    last := count;
END_ACTION
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    muxer : Muxer;
    out_count : INT := INT#0;
    out_last : INT := INT#0;
END_VAR
    muxer();
    out_count := muxer.count;
    out_last := muxer.last;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out_count", 2i16);
    harness.assert_eq("out_last", 2i16);
}

#[test]
fn fb_action_qualified_call_from_program() {
    let source = r#"
FUNCTION_BLOCK Payload
VAR
    fired : BOOL := FALSE;
END_VAR
ACTION Day:
    fired := TRUE;
END_ACTION
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    payload : Payload;
    out_fired : BOOL := FALSE;
END_VAR
    payload.Day();
    out_fired := payload.fired;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out_fired", true);
}

#[test]
fn fb_action_body_sees_owner_fields_directly() {
    let source = r#"
FUNCTION_BLOCK Timers
VAR
    ticks : INT := INT#1;
    done : BOOL := FALSE;
END_VAR
    tick();
ACTION tick:
    ticks := ticks - INT#1;
    IF ticks <= INT#0 THEN
        done := TRUE;
    END_IF;
END_ACTION
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    timers : Timers;
    out_done : BOOL := FALSE;
END_VAR
    timers();
    out_done := timers.done;
END_PROGRAM
"#;

    let mut harness = TestHarness::from_source(source).unwrap();
    let result = harness.cycle();
    assert!(result.errors.is_empty(), "{:?}", result.errors);
    harness.assert_eq("out_done", true);
}
